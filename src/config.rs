//! Type-safe configuration loader using the `config` crate,
//! with manual environment-variable overrides for core settings.

use serde::Deserialize;
use humantime_serde;
use humantime;
use std::{env, time::Duration};
use config::{Config, ConfigError, File};

/// Top-level application settings loaded from `Config.toml`
/// and then overridden (where applicable) by environment variables.
#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    /// Postgres connection URL
    pub database_url: String,

    /// Interval between each ingestion run (e.g. "30m", "1h")
    #[serde(with = "humantime_serde")]
    pub ingest_interval: Duration,

    /// HTTP bind address for metrics & health endpoints
    pub server_bind: String,

    /// List of all RSS/Atom sources to ingest, each carrying metadata.
    pub feeds: Vec<Feed>,
}

/// Represents one RSS/Atom feed source and its metadata.
#[derive(Debug, Deserialize, Clone)]
pub struct Feed {
    /// Human-friendly name of this feed (e.g. "Krebs on Security")
    pub name: String,

    /// The actual RSS/Atom URL to pull down
    pub url: String,

    /// Category or origin (e.g. "official", "independent", "community")
    #[serde(default)]
    pub feed_type: Option<String>,

    /// Tags to help you filter or group feeds in your code
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Settings {
    /// Load settings from `Config.toml` (if present),
    /// then apply any overrides from these environment variables:
    ///
    /// - `APP__DATABASE_URL`
    /// - `APP__INGEST_INTERVAL`
    /// - `APP__SERVER_BIND`
    pub fn new() -> Result<Self, ConfigError> {
        // 1) Base defaults from Config.toml
        let cfg = Config::builder()
            .add_source(File::with_name("Config").required(false))
            .build()?;

        // Deserialize everything straight away
        let mut settings: Settings = cfg.try_deserialize()?;

        // 2) Manual overrides for core settings
        if let Ok(val) = env::var("APP__DATABASE_URL") {
            settings.database_url = val;
        }
        if let Ok(val) = env::var("APP__INGEST_INTERVAL") {
            settings.ingest_interval = humantime::parse_duration(&val)
                .map_err(|e| ConfigError::Foreign(Box::new(e)))?;
        }
        if let Ok(val) = env::var("APP__SERVER_BIND") {
            settings.server_bind = val;
        }

        Ok(settings)
    }
}
