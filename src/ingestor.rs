//! Core ingestion logic: fetch, parse, dedupe, sanitize, and upsert.

use crate::errors::IngestError;
use crate::metrics::{FETCH_COUNTER, FETCH_HISTOGRAM, ENTRIES_PROCESSED, SANITIZATION_FAILURES};
use ammonia::clean;
use chrono::{NaiveDateTime, Utc};
use feed_rs::model::{Entry, Feed};
use feed_rs::parser;
use sqlx::PgPool;
use std::time::Instant;
use tracing::{warn, info, debug};
use url::Url;
use uuid::Uuid;

/// Represents all unified fields we store for each RSS/Atom article.
#[derive(Debug, Clone)]
pub struct FeedItem {
    // Core/primary fields
    pub id: Uuid,
    pub guid: String,
    pub title: String,
    pub link: String,
    pub published: Option<NaiveDateTime>,
    pub content: Option<String>,
    pub summary: Option<String>,
    pub author: Option<String>,
    pub categories: Option<Vec<String>>,
    pub entry_updated: Option<NaiveDateTime>,
    // Feed/source metadata
    pub feed_url: String,
    pub feed_title: Option<String>,
    pub feed_description: Option<String>,
    pub feed_language: Option<String>,
    pub feed_icon: Option<String>,
    pub feed_updated: Option<NaiveDateTime>,
    pub inserted_at: NaiveDateTime,
}

/// Given an entry and its feed metadata, map all fields, always preferring the most content-rich field available.
/// - If `entry.content` exists, use that (most feeds with `<content:encoded>` or `<content>`).
/// - Else, use `entry.summary` (maps to `<description>` or `<summary>`).
/// - Clean HTML for both, as per best practice.
pub fn entry_to_feed_item(entry: &Entry, feed: &Feed, feed_url: &str) -> FeedItem {
    // Compute the "best" link (resolve relative URLs if needed)
    let link_raw = entry
        .links
        .first()
        .map(|l| l.href.clone())
        .unwrap_or_default();
    let link = match Url::parse(&link_raw) {
        Ok(_) => link_raw.clone(),
        Err(_) => Url::parse(feed_url)
            .and_then(|base| base.join(&link_raw))
            .map(|u| u.to_string())
            .unwrap_or(link_raw.clone()),
    };

    // Prefer entry.content (usually from <content:encoded> or <content>), else summary/description
    let content = entry
        .content
        .as_ref()
        .and_then(|c| c.body.clone())
        .or_else(|| entry.summary.as_ref().map(|s| s.content.clone()));

    // Keep summary as original summary field (for metadata/teaser purposes)
    let summary = entry.summary.as_ref().map(|s| s.content.clone());

    // Log if both are None for visibility
    if content.is_none() && summary.is_none() {
        warn!(
            "Feed entry for '{}' [{}] has neither content nor summary.",
            entry.title.as_ref().map(|t| t.content.as_str()).unwrap_or("<no title>"),
            link
        );
    }

    FeedItem {
        id: Uuid::new_v4(),
        guid: entry.id.clone(),
        title: entry
            .title
            .as_ref()
            .map(|t| t.content.clone())
            .unwrap_or_default(),
        link,
        published: entry.published.map(|dt| dt.naive_utc()),
        content,
        summary,
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
        inserted_at: Utc::now().naive_utc(),
    }
}

/// Sanitize, validate, and log why an entry is skipped if it fails.
/// - Ensures title, summary, and content are within length limits and required fields are present.
/// - Sanitizes HTML for title, summary, and content.
pub fn sanitize_and_validate(item: &FeedItem) -> Option<FeedItem> {
    let title = item.title.trim();
    if title.is_empty() || title.len() > 1024 {
        SANITIZATION_FAILURES.inc();
        warn!("Sanitization failed: title missing/too long: {:?}", item);
        return None;
    }

    // Limit summary size
    let summary = item.summary.as_deref().map(str::trim);
    if let Some(s) = summary {
        if s.len() > 200_000 {
            SANITIZATION_FAILURES.inc();
            warn!("Sanitization failed: summary too long: {:?}", item);
            return None;
        }
    }

    // Limit content size
    let content = item.content.as_deref().map(str::trim);
    if let Some(c) = content {
        if c.len() > 500_000 {
            SANITIZATION_FAILURES.inc();
            warn!("Sanitization failed: content too long: {:?}", item);
            return None;
        }
    }

    // Validate link
    if Url::parse(&item.link).is_err() {
        SANITIZATION_FAILURES.inc();
        warn!("Sanitization failed: invalid link: {:?}", item.link);
        return None;
    }

    let sanitized_title = clean(title).to_string();
    let sanitized_summary = summary.map(|s| clean(s).to_string());
    let sanitized_content = content.map(|c| clean(c).to_string());

    ENTRIES_PROCESSED.inc();

    Some(FeedItem {
        title: sanitized_title,
        summary: sanitized_summary,
        content: sanitized_content,
        ..item.clone()
    })
}

/// Download and parse the feed.
/// - Tracks metrics and logs timing.
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
    debug!("Fetched and parsed feed {} in {:.2}s", url, elapsed);
    Ok(feed)
}

/// Write a FeedItem to the database, with dedupe logic.
/// - Logs when an insert or upsert occurs.
pub async fn process_entry(pool: &PgPool, item: &FeedItem) -> Result<(), IngestError> {
    // Dedupe in archive by GUID
    let exists: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM archive WHERE guid = $1)")
        .bind(&item.guid)
        .fetch_one(pool)
        .await?;
    if !exists.0 {
        sqlx::query(
            "INSERT INTO archive (
                id, guid, title, link, published, content, summary, author, categories, entry_updated,
                feed_url, feed_title, feed_description, feed_language, feed_icon, feed_updated, inserted_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
        )
        .bind(&item.id)
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
        .bind(item.inserted_at)
        .execute(pool)
        .await?;
        info!("Inserted new archive entry for GUID: {}", item.guid);
    }

    // Always upsert into current
    sqlx::query(
        "INSERT INTO current (
            id, guid, title, link, published, content, summary, author, categories, entry_updated,
            feed_url, feed_title, feed_description, feed_language, feed_icon, feed_updated, inserted_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
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
            feed_updated = EXCLUDED.feed_updated,
            inserted_at = EXCLUDED.inserted_at",
    )
    .bind(&item.id)
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
    .bind(item.inserted_at)
    .execute(pool)
    .await?;
    debug!("Upserted current entry for GUID: {}", item.guid);
    Ok(())
}
