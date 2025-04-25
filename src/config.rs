//! Type‑safe configuration loader using `config` crate.

use serde::Deserialize;
use std::time::Duration;
use config::{Config, ConfigError, Environment, File};

#[derive(Deserialize, Debug)]
pub struct Settings {
    pub database_url: String,
    pub feed_urls: Vec<String>,
    #[serde(with = "humantime_serde")]
    pub ingest_interval: Duration,
    pub server_bind: String,
}

impl Settings {
    /// Load configuration from `Config.toml` (optional) and environment variables (prefix APP__).
    pub fn new() -> Result<Self, ConfigError> {
        let builder = Config::builder()
            .add_source(File::with_name("Config").required(false))
            .add_source(Environment::with_prefix("APP").separator("__"));
        let cfg = builder.build()?;
        cfg.try_deserialize()
    }
}