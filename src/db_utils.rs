use sqlx::PgPool;
use anyhow::Result;
use chrono::{DateTime, Utc};

/// Retrieves the current failed fetch count for a given GUID from the archive table.
///
/// # Arguments
///
/// * `pool` - Reference to the Postgres connection pool.
/// * `guid` - The unique identifier for the feed item.
///
/// # Returns
///
/// * `Ok(i32)` - The current failed fetch count, or 0 if none found.
/// * `Err` - If the database query fails.
///
/// # Notes
///
/// Using `fetch_optional` to handle cases where the GUID might not exist yet.
/// Defaults to 0 failures if not found.
pub async fn get_failed_fetch_count(pool: &PgPool, guid: &str) -> Result<i32> {
    let rec = sqlx::query_scalar!(
        // Simple select to get the failed fetch count for this GUID
        "SELECT failed_fetch_count FROM archive WHERE guid = $1",
        guid
    )
    .fetch_optional(pool)
    .await?;

    // Return the count or zero if no record found
    Ok(rec.unwrap_or(0))
}

/// Updates the failed fetch count for a given GUID in the archive table.
///
/// # Arguments
///
/// * `pool` - Reference to the Postgres connection pool.
/// * `guid` - The unique identifier for the feed item.
/// * `count` - The new failed fetch count to set.
///
/// # Returns
///
/// * `Ok(())` - On successful update.
/// * `Err` - If the update query fails.
///
/// # Notes
///
/// Uses positional parameters for safety and clarity.
pub async fn update_failed_fetch_count(pool: &PgPool, guid: &str, count: i32) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE archive
        SET failed_fetch_count = $1
        WHERE guid = $2
        "#,
        count,
        guid
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Disables a feed item by setting the `disabled` flag to true in the archive table.
///
/// # Arguments
///
/// * `pool` - Reference to the Postgres connection pool.
/// * `guid` - The unique identifier for the feed item.
///
/// # Returns
///
/// * `Ok(())` - On successful update.
/// * `Err` - If the update query fails.
///
/// # Notes
///
/// Marks the feed as disabled to prevent further processing.
pub async fn disable_feed(pool: &PgPool, guid: &str) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE archive
        SET disabled = true
        WHERE guid = $1
        "#,
        guid
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Retrieves the last fetch attempt timestamp for a given GUID from the archive table.
///
/// # Arguments
///
/// * `pool` - Reference to the Postgres connection pool.
/// * `guid` - The unique identifier for the feed item.
///
/// # Returns
///
/// * `Ok(Some(DateTime<Utc>))` - Timestamp of last fetch attempt if present.
/// * `Ok(None)` - If no timestamp is recorded.
/// * `Err` - If the database query fails.
pub async fn get_last_fetch_attempt(pool: &PgPool, guid: &str) -> Result<Option<DateTime<Utc>>> {
    let rec = sqlx::query_scalar!(
        "SELECT last_fetch_attempt FROM archive WHERE guid = $1",
        guid
    )
    .fetch_optional(pool)
    .await?;

    // Flatten Option<Option<T>> into Option<T>
    Ok(rec.flatten())
}

/// Updates the last fetch attempt timestamp for a given GUID in the archive table.
///
/// # Arguments
///
/// * `pool` - Reference to the Postgres connection pool.
/// * `guid` - The unique identifier for the feed item.
/// * `timestamp` - The new timestamp to set.
///
/// # Returns
///
/// * `Ok(())` - On successful update.
/// * `Err` - If the update query fails.
pub async fn update_last_fetch_attempt(pool: &PgPool, guid: &str, timestamp: DateTime<Utc>) -> Result<()> {
    sqlx::query!(
        r#"
        UPDATE archive
        SET last_fetch_attempt = $1
        WHERE guid = $2
        "#,
        timestamp,
        guid
    )
    .execute(pool)
    .await?;

    Ok(())
}