//! Core ingestion logic: fetch, parse, dedupe, and upsert.

use crate::errors::IngestError;
use crate::metrics::{FETCH_COUNTER, FETCH_HISTOGRAM};
use chrono::NaiveDateTime;
use feed_rs::model::Entry;
use feed_rs::parser;
use sqlx::PgPool;
use std::time::Instant;

/// Fetch & parse the feed at `url`.
pub async fn fetch_feed(url: &str) -> Result<Vec<Entry>, IngestError> {
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

    Ok(feed.entries)
}

/// Process a single entry: dedupe & write to DB.
pub async fn process_entry(pool: &PgPool, entry: &Entry) -> Result<(), IngestError> {
    // Basic fields…
    let guid = &entry.id;
    let title = entry
        .title
        .as_ref()
        .map(|t| t.content.clone())
        .unwrap_or_default();
    let link = entry
        .links
        .first()
        .map(|l| l.href.clone())
        .unwrap_or_default();
    let published: Option<NaiveDateTime> = entry.published.map(|dt| dt.naive_utc());

    // ==== CONTENT FALLBACK ====
    // 1) If <content:encoded> exists and has a body, use it
    // 2) Otherwise use <description> (entry.summary)
    // 3) Otherwise default to empty
    let content = entry
        .content
        .as_ref()
        .and_then(|c| c.body.clone()) // flatten Option<Option<String>> → Option<String>
        .or_else(|| entry.summary.as_ref().map(|s| s.content.clone()))
        .unwrap_or_default();

    // Check archive for duplicates
    let exists: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM archive WHERE guid = $1)")
        .bind(guid)
        .fetch_one(pool)
        .await?;

    if !exists.0 {
        sqlx::query(
            "INSERT INTO archive (guid, title, link, published, content)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(guid)
        .bind(&title)
        .bind(&link)
        .bind(published)
        .bind(&content)
        .execute(pool)
        .await?;
    }

    // Upsert into current
    sqlx::query(
        "INSERT INTO current (guid, title, link, published, content)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (guid) DO UPDATE SET
           title     = EXCLUDED.title,
           link      = EXCLUDED.link,
           published = EXCLUDED.published,
           content   = EXCLUDED.content",
    )
    .bind(guid)
    .bind(&title)
    .bind(&link)
    .bind(published)
    .bind(&content)
    .execute(pool)
    .await?;

    Ok(())
}
