//! Entrypoint: set up tracing, run database migrations, start HTTP metrics & health server,
//! and begin the feed ingestion loop.
//!
//! This application uses a strongly-typed configuration (`Settings`) defined in `config.rs`,
//! which provides:
//!  - `database_url`       – Postgres connection string
//!  - `ingest_interval`    – How often to poll each feed
//!  - `server_bind`        – HTTP bind address for metrics & health endpoints
//!  - `feeds: Vec<Feed>`   – Your list of RSS/Atom sources with metadata

use std::{net::SocketAddr, sync::Arc};

use futures::stream::{FuturesUnordered, StreamExt};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server};
use sqlx::postgres::PgPoolOptions;
use tokio::time::interval;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use rust_feed_ingestor::config::{Feed, Settings};
use rust_feed_ingestor::errors::IngestError;
use rust_feed_ingestor::ingestor::{fetch_feed, process_entry};
use rust_feed_ingestor::metrics;

#[tokio::main]
/// Application entrypoint for the OSINT feed ingestor.
///
/// **Workflow**:
/// 1. Initialise tracing/logging from `RUST_LOG` (or default to `info`).  
/// 2. Load `Config.toml` (and apply any `APP__…` env‐var overrides).  
/// 3. Spin up a Postgres pool and run any pending SQLx migrations.  
/// 4. Launch a background HTTP server on `/metrics` and `/healthz`.  
/// 5. Enter the ingestion loop: every `ingest_interval`, fetch all feeds
///    concurrently and process each entry into the database.
async fn main() -> Result<(), IngestError> {
    // ───────────────────────────────────────────────────────────────
    // 1. Initialise tracing / logging
    // ───────────────────────────────────────────────────────────────
    fmt().with_env_filter(EnvFilter::from_default_env()).init();
    info!("Starting OSINT feed ingestor…");

    // ───────────────────────────────────────────────────────────────
    // 2. Load configuration
    // ───────────────────────────────────────────────────────────────
    let settings = Settings::new()?;
    info!(?settings, "Loaded configuration");

    // ───────────────────────────────────────────────────────────────
    // 3. Database pool & migrations
    // ───────────────────────────────────────────────────────────────
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&settings.database_url)
        .await?;
    info!("Connected to Postgres");

    info!("Running database migrations…");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");
    info!("Migrations complete");

    // ───────────────────────────────────────────────────────────────
    // 4. HTTP server for metrics & health
    // ───────────────────────────────────────────────────────────────
    let addr: SocketAddr = settings
        .server_bind
        .parse()
        .expect("Invalid `server_bind` in configuration");

    let make_svc = make_service_fn(move |_conn| {
        async move {
            Ok::<_, IngestError>(service_fn(move |req: Request<Body>| {
                async move {
                    match (req.method(), req.uri().path()) {
                        (&Method::GET, "/metrics") => {
                            let body = metrics::gather_metrics();
                            // TURBOFISH to fix E0282:
                            Ok::<Response<Body>, IngestError>(Response::new(Body::from(body)))
                        }
                        (&Method::GET, "/healthz") => {
                            Ok::<Response<Body>, IngestError>(Response::new(Body::from("OK")))
                        }
                        _ => {
                            let nf = Response::builder().status(404).body(Body::empty()).unwrap();
                            Ok::<Response<Body>, IngestError>(nf)
                        }
                    }
                }
            }))
        }
    });

    tokio::spawn(async move {
        info!(%addr, "Starting metrics & health server");
        Server::bind(&addr)
            .serve(make_svc)
            .await
            .expect("Metrics server failed");
    });

    // ───────────────────────────────────────────────────────────────
    // 5. Ingestion loop
    // ───────────────────────────────────────────────────────────────
    let feeds: Arc<Vec<Feed>> = Arc::new(settings.feeds.clone());
    let mut ticker = interval(settings.ingest_interval);

    loop {
        ticker.tick().await;
        info!("Beginning ingestion cycle for {} feeds", feeds.len());

        let mut tasks = FuturesUnordered::new();
        for feed in feeds.iter().cloned() {
            let pool = pool.clone();
            tasks.push(async move {
                info!(feed = %feed.name, url = %feed.url, "Fetching feed");
                match fetch_feed(&feed.url).await {
                    Ok(entries) => {
                        for entry in entries {
                            if let Err(e) = process_entry(&pool, &entry).await {
                                error!(
                                    feed = %feed.url,
                                    entry_id = ?entry.id,
                                    error = %e,
                                    "Entry processing failed"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        error!(feed = %feed.url, error = %e, "Feed fetch failed");
                    }
                }
            });
        }

        while let Some(_) = tasks.next().await {}
        info!("Ingestion cycle complete, waiting for next tick…");
    }
}
