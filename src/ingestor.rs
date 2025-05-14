//! Core ingestion logic: fetch, parse, dedupe, sanitize, and upsert.
use crate::errors::IngestError;
use crate::metrics::{FETCH_COUNTER, FETCH_HISTOGRAM};
use chrono::NaiveDateTime;
use feed_rs::model::{Entry, Feed};
use feed_rs::parser;
use sqlx::PgPool;
use std::time::Instant;
use ammonia::clean;
use url::Url;
use tracing::warn;

/// Unified model: All entry and feed metadata for powerful OSINT queries.
#[derive(Debug, Clone)]
pub struct FeedItem {
    // Entry fields
    pub guid: String,
    pub title: String,
    pub link: String,
    pub published: Option<NaiveDateTime>,
    pub content: Option<String>,
    pub summary: Option<String>,
    pub author: Option<String>,
    pub categories: Option<Vec<String>>,
    pub entry_updated: Option<NaiveDateTime>,
    // Feed source fields
    pub feed_url: String,
    pub feed_title: Option<String>,
    pub feed_description: Option<String>,
    pub feed_language: Option<String>,
    pub feed_icon: Option<String>,
    pub feed_updated: Option<NaiveDateTime>,
}

/// Convert an entry and its parent feed into a FeedItem with provenance.
/// This will also resolve relative links to absolute URLs using the feed's URL.
pub fn entry_to_feed_item(entry: &Entry, feed: &Feed, feed_url: &str) -> FeedItem {
    // If possible, resolve relative links to absolute using feed_url as base
    let link_raw = entry.links.get(0).map(|l| l.href.clone()).unwrap_or_default();
    let link = match Url::parse(&link_raw) {
        Ok(_) => link_raw.clone(),
        Err(_) => {
            // Try to join with feed_url base if link_raw is relative
            Url::parse(feed_url)
                .and_then(|base| base.join(&link_raw))
                .map(|u| u.to_string())
                .unwrap_or(link_raw.clone())
        }
    };

    FeedItem {
        guid: entry.id.clone(),
        title: entry.title.as_ref().map(|t| t.content.clone()).unwrap_or_default(),
        link,
        published: entry.published.map(|dt| dt.naive_utc()),
        content: entry.content.as_ref().and_then(|c| c.body.clone()),
        summary: entry.summary.as_ref().map(|s| s.content.clone()),
        author: entry.authors.get(0).map(|a| a.name.clone()),
        categories: if entry.categories.is_empty() {
            None
        } else {
            Some(entry.categories.iter().map(|c| c.term.clone()).collect())
        },
        entry_updated: entry.updated.map(|dt| dt.naive_utc()),
        feed_url: feed_url.to_string(),
        feed_title: feed.title.as_ref().map(|t| t.content.clone()),
        feed_description: feed.description.as_ref().map(|d| d.content.clone()),
        feed_language: feed.language.clone(),
        feed_icon: feed.icon.as_ref().map(|i| i.uri.clone()), // FIXED: .uri not .href
        feed_updated: feed.updated.map(|dt| dt.naive_utc()),
    }
}

/// Sanitize and validate a FeedItem.
/// Returns Some(sanitized_item) if valid, None if invalid.
pub fn sanitize_and_validate(item: &FeedItem) -> Option<FeedItem> {
    // Validate title: required and max 1024 chars
    let title = item.title.trim();
    if title.is_empty() || title.len() > 1024 {
        warn!("Sanitization failed: title missing/too long: {:?}", item);
        return None;
    }
    // Validate summary: optional, max 200,000 chars
    let summary = item.summary.as_deref().map(str::trim);
    if let Some(s) = summary {
        if s.len() > 200_000 {
            warn!("Sanitization failed: summary too long: {:?}", item);
            return None;
        }
    }
    // Validate content: optional, max 500,000 chars
    let content = item.content.as_deref().map(str::trim);
    if let Some(c) = content {
        if c.len() > 500_000 {
            warn!("Sanitization failed: content too long: {:?}", item);
            return None;
        }
    }
    // Validate URL (must be absolute)
    if Url::parse(&item.link).is_err() {
        warn!("Sanitization failed: invalid link: {:?}", item.link);
        return None;
    }
    // Sanitize all HTML/text fields
    let sanitized_title = clean(title).to_string();
    let sanitized_summary = summary.map(|s| clean(s).to_string());
    let sanitized_content = content.map(|c| clean(c).to_string());
    Some(FeedItem {
        title: sanitized_title,
        summary: sanitized_summary,
        content: sanitized_content,
        ..item.clone()
    })
}

/// Fetch & parse the feed at `url`.
/// Returns the full Feed struct (not just entries) for provenance.
pub async fn fetch_feed(url: &str) -> Result<Feed, IngestError> {
    FETCH_COUNTER.inc();
    let start = Instant::now();
    let bytes = reqwest::get(url)
        .await
        .map_err(|e| IngestError::Fetch(url.to_string(), e))?
        .bytes()
        .await
        .map_err(|e| IngestError::Fetch(url.to_string(), e))?;
    let feed = parser::parse(&bytes[..]).map_err(|e| IngestError::Parse(url.to_string(), e))?;
    let elapsed = start.elapsed().as_secs_f64();
    FETCH_HISTOGRAM.observe(elapsed);
    Ok(feed)
}

/// Process a single FeedItem: dedupe & write to DB (archive & current).
pub async fn process_entry(pool: &PgPool, item: &FeedItem) -> Result<(), IngestError> {
    // Check archive for duplicates
    let exists: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM archive WHERE guid = $1)")
        .bind(&item.guid)
        .fetch_one(pool)
        .await?;
    if !exists.0 {
        sqlx::query(
            "INSERT INTO archive (
                guid, title, link, published, content, summary, author, categories, entry_updated,
                feed_url, feed_title, feed_description, feed_language, feed_icon, feed_updated
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        )
        .bind(&item.guid)
        .bind(&item.title)
        .bind(&item.link)
        .bind(item.published)
        .bind(&item.content)
        .bind(&item.summary)
        .bind(&item.author)
        .bind(&item.categories)
        .bind(item.entry_updated)
        .bind(&item.feed_url)
        .bind(&item.feed_title)
        .bind(&item.feed_description)
        .bind(&item.feed_language)
        .bind(&item.feed_icon)
        .bind(item.feed_updated)
        .execute(pool)
        .await?;
    }
    // Upsert into current
    sqlx::query(
        "INSERT INTO current (
            guid, title, link, published, content, summary, author, categories, entry_updated,
            feed_url, feed_title, feed_description, feed_language, feed_icon, feed_updated
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        ON CONFLICT (guid) DO UPDATE SET
            title = EXCLUDED.title,
            link = EXCLUDED.link,
            published = EXCLUDED.published,
            content = EXCLUDED.content,
            summary = EXCLUDED.summary,
            author = EXCLUDED.author,
            categories = EXCLUDED.categories,
            entry_updated = EXCLUDED.entry_updated,
            feed_url = EXCLUDED.feed_url,
            feed_title = EXCLUDED.feed_title,
            feed_description = EXCLUDED.feed_description,
            feed_language = EXCLUDED.feed_language,
            feed_icon = EXCLUDED.feed_icon,
            feed_updated = EXCLUDED.feed_updated",
    )
    .bind(&item.guid)
    .bind(&item.title)
    .bind(&item.link)
    .bind(item.published)
    .bind(&item.content)
    .bind(&item.summary)
    .bind(&item.author)
    .bind(&item.categories)
    .bind(item.entry_updated)
    .bind(&item.feed_url)
    .bind(&item.feed_title)
    .bind(&item.feed_description)
    .bind(&item.feed_language)
    .bind(&item.feed_icon)
    .bind(item.feed_updated)
    .execute(pool)
    .await?;
    Ok(())
}