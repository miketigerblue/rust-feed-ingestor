//! Centralised error type for the ingestor.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum IngestError {
    #[error("HTTP error fetching {0}: {1}")]
    Fetch(String, #[source] reqwest::Error),

    #[error("Parse error for {0}: {1}")]
    Parse(String, #[source] feed_rs::parser::ParseFeedError),

    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),
}