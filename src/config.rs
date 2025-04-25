//! Type-safe configuration loader using the `config` crate
//! with manual environment-variable overrides for all fields.

use serde::Deserialize;
use humantime_serde;
use humantime;
use std::{env, time::Duration};
use config::{Config, ConfigError, File};

#[derive(Deserialize, Debug)]
pub struct Settings {
    /// Postgres connection URL
    pub database_url: String,
    /// List of RSS/Atom feed URLs
    pub feed_urls: Vec<String>,
    /// Ingestion interval (e.g. "30m", "1h")
    #[serde(with = "humantime_serde")]
    pub ingest_interval: Duration,
    /// HTTP bind address for metrics & health endpoints
    pub server_bind: String,
}

impl Settings {
    /// Load defaults from `Config.toml` (if present), then override from these environment variables:
    ///
    /// - `APP__DATABASE_URL`
    /// - `APP__INGEST_INTERVAL`
    /// - `APP__SERVER_BIND`
    /// - `APP__FEED_URLS`  (comma-separated list of URLs)
    pub fn new() -> Result<Self, ConfigError> {
        // 1) Load from Config.toml only
        let cfg = Config::builder()
            .add_source(File::with_name("Config").required(false))
            .build()?;
        let mut settings: Settings = cfg.try_deserialize()?;

        // 2) Override individual fields from env vars
        if let Ok(db_url) = env::var("APP__DATABASE_URL") {
            settings.database_url = db_url;
        }
        if let Ok(interval_str) = env::var("APP__INGEST_INTERVAL") {
            settings.ingest_interval = humantime::parse_duration(&interval_str)
                .map_err(|e| ConfigError::Foreign(Box::new(e)))?;
        }
        if let Ok(bind) = env::var("APP__SERVER_BIND") {
            settings.server_bind = bind;
        }
        if let Ok(csv) = env::var("APP__FEED_URLS") {
            settings.feed_urls = csv
                .split(',')
                .filter_map(|s| {
                    let t = s.trim();
                    if t.is_empty() { None } else { Some(t.to_string()) }
                })
                .collect();
        }

        Ok(settings)
    }
}
