//! ingestor.rs
//!
//! Core ingestion logic: fetch, parse, dedupe, sanitize, enrich with live content,
//! and upsert into Postgres archive and current tables.

use crate::browser::Browser; // Our polite web fetcher and sanitizer
use crate::errors::IngestError;
use crate::metrics::{FETCH_COUNTER, FETCH_HISTOGRAM};
use ammonia::clean;
use chrono::NaiveDateTime;
use feed_rs::model::{Entry, Feed};
use feed_rs::parser;
use sqlx::PgPool;
use std::time::Instant;
use tracing::warn;
use url::Url;

/// Unified model representing a feed entry with provenance and metadata.
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

/// Convert an RSS entry and its parent feed into a FeedItem.
/// Resolves relative links to absolute URLs using the feed's URL.
pub fn entry_to_feed_item(entry: &Entry, feed: &Feed, feed_url: &str) -> FeedItem {
    // Extract the first link, resolve relative URLs against feed base URL
    let link_raw = entry
        .links
        .first()
        .map(|l| l.href.clone())
        .unwrap_or_default();

    let link = match Url::parse(&link_raw) {
        Ok(_) => link_raw.clone(),
        Err(_) => {
            // Attempt to join relative link with feed URL base
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
        author: entry.authors.first().map(|a| a.name.clone()),
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
        feed_icon: feed.icon.as_ref().map(|i| i.uri.clone()),
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

    // Sanitize all HTML/text fields to remove scripts, styles, etc.
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

/// Process a single FeedItem: dedupe, enrich with live content if missing, and write to DB.
pub async fn process_entry(pool: &PgPool, item: &FeedItem, browser: &Browser) -> Result<(), IngestError> {
    // Check if the feed-provided content is missing or empty (trimmed)
    // We only fetch live page content if we really need to.
    let fetched_content = if item.content.as_ref().map_or(true, |c| c.trim().is_empty()) {
        // Attempt to fetch and sanitize live content from the article's link.
        // This is our polite way of filling content gaps without spamming.
        match browser.fetch_and_clean(&item.link).await {
            Ok(content) if !content.trim().is_empty() => content,
            Ok(_) | Err(_) => {
                // If fetching failed or returned empty, fallback to feed content (even if empty)
                tracing::warn!("Live content fetch failed or empty for link: {}", &item.link);
                item.content.clone().unwrap_or_default()
            }
        }
    } else {
        // Content exists, no need to fetch again — save the bandwidth!
        item.content.clone().unwrap_or_default()
    };

    // Check if this entry already exists in archive to avoid duplicates
    let exists: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM archive WHERE guid = $1)")
        .bind(&item.guid)
        .fetch_one(pool)
        .await?;

    // Insert new archive entry with live content if missing
    if !exists.0 {
        sqlx::query(
            "INSERT INTO archive (
                guid, title, link, published, content, full_content, summary, author, categories, entry_updated,
                feed_url, feed_title, feed_description, feed_language, feed_icon, feed_updated
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"
        )
        .bind(&item.guid)
        .bind(&item.title)
        .bind(&item.link)
        .bind(item.published)
        .bind(&item.content)          // original feed content
        .bind(&fetched_content)       // live fetched or fallback content
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

    // Upsert into current table similarly, including live content
    sqlx::query(
        "INSERT INTO current (
            guid, title, link, published, content, full_content, summary, author, categories, entry_updated,
            feed_url, feed_title, feed_description, feed_language, feed_icon, feed_updated
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
        ON CONFLICT (guid) DO UPDATE SET
            title = EXCLUDED.title,
            link = EXCLUDED.link,
            published = EXCLUDED.published,
            content = EXCLUDED.content,
            full_content = EXCLUDED.full_content,
            summary = EXCLUDED.summary,
            author = EXCLUDED.author,
            categories = EXCLUDED.categories,
            entry_updated = EXCLUDED.entry_updated,
            feed_url = EXCLUDED.feed_url,
            feed_title = EXCLUDED.feed_title,
            feed_description = EXCLUDED.feed_description,
            feed_language = EXCLUDED.feed_language,
            feed_icon = EXCLUDED.feed_icon,
            feed_updated = EXCLUDED.feed_updated"
    )
    .bind(&item.guid)
    .bind(&item.title)
    .bind(&item.link)
    .bind(item.published)
    .bind(&item.content)
    .bind(&fetched_content)
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