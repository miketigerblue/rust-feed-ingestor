//! Entrypoint: set up tracing, run database migrations, start HTTP metrics & health server,
//! and begin the feed ingestion loop.
//!
//! This application uses a strongly-typed configuration (`Settings`) defined in `config.rs`,
//! which provides:
//!  - `database_url`       – Postgres connection string
//!  - `ingest_interval`    – How often to poll each feed
//!  - `server_bind`        – HTTP bind address for metrics & health endpoints
//!  - `feeds: Vec<Feed>`   – Your list of RSS/Atom sources with metadata

use std::{net::SocketAddr, sync::Arc, time::Instant};

use futures::stream::{FuturesUnordered, StreamExt};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server};
use prometheus::{Encoder, TextEncoder}; // ← bring Encoder trait into scope
use sqlx::postgres::PgPoolOptions;
use tokio::time::interval;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use rust_feed_ingestor::config::{Feed, Settings};
use rust_feed_ingestor::errors::IngestError;
use rust_feed_ingestor::ingestor::{fetch_feed, process_entry};
use rust_feed_ingestor::metrics;

/// Application entrypoint for the OSINT feed ingestor.
///
/// **Workflow**:
/// 1. Initialise tracing/logging from `RUST_LOG` (or default to `info`).  
/// 2. Load `Config.toml` (and apply any `APP__…` env-var overrides).  
/// 3. Spin up a Postgres pool and run any pending SQLx migrations.  
/// 4. Launch a background HTTP server on `/metrics` and `/healthz`.  
/// 5. Enter the ingestion loop: every `ingest_interval`, fetch all feeds
///    concurrently and process each entry into the database.
#[tokio::main]
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
    //
    // We must set the `Content-Type` header on `/metrics` to:
    //     text/plain; version=0.0.4; charset=utf-8
    // Otherwise Prometheus (v3+) will reject the scrape.
    let addr: SocketAddr = settings
        .server_bind
        .parse()
        .expect("Invalid `server_bind` in configuration");

    let make_svc = make_service_fn(move |_conn| {
        async move {
            Ok::<_, IngestError>(service_fn(move |req: Request<Body>| {
                async move {
                    match (req.method(), req.uri().path()) {
                        // ─── METRICS ENDPOINT ────────────────────────────────
                        (&Method::GET, "/metrics") => {
                            // 1) Gather all metrics into a text body
                            let metrics_text = metrics::gather_metrics();

                            // 2) Create an encoder to retrieve the correct MIME string
                            let encoder = TextEncoder::new();
                            let mime = encoder.format_type();
                            //    => "text/plain; version=0.0.4; charset=utf-8"

                            // 3) Build a full HTTP response with header + body
                            let resp = Response::builder()
                                .header("Content-Type", mime)
                                .body(Body::from(metrics_text))
                                .expect("Failed to build /metrics response");

                            Ok::<Response<Body>, IngestError>(resp)
                        }

                        // ─── HEALTHCHECK ENDPOINT ───────────────────────────
                        (&Method::GET, "/healthz") => {
                            Ok::<Response<Body>, IngestError>(Response::new(Body::from("OK")))
                        }

                        // ─── ANY OTHER ROUTE ────────────────────────────────
                        _ => {
                            let not_found =
                                Response::builder().status(404).body(Body::empty()).unwrap();
                            Ok::<Response<Body>, IngestError>(not_found)
                        }
                    }
                }
            }))
        }
    });

    // Spawn the metrics & health HTTP server
    tokio::spawn(async move {
        info!(%addr, "Starting metrics & health server");
        Server::bind(&addr)
            .serve(make_svc)
            .await
            .expect("Metrics server failed");
    });

    // ───────────────────────────────────────────────────────────────
    // 5. Ingestion loop (with enhanced logging)
    // ───────────────────────────────────────────────────────────────
    let feeds: Arc<Vec<Feed>> = Arc::new(settings.feeds.clone());
    let mut ticker = interval(settings.ingest_interval);

    loop {
        // Mark the start of the cycle to time the whole ingestion
        let cycle_start = Instant::now();
        info!("Starting ingestion cycle for {} feeds", feeds.len());

        // Fire off one task per feed, each returning:
        // (feed_name, fetch_duration_s, entry_count, errors_occurred)
        let mut tasks = FuturesUnordered::new();
        for feed in feeds.iter().cloned() {
            let pool = pool.clone();
            tasks.push(async move {
                let feed_start = Instant::now();
                let mut errors = 0;

                match fetch_feed(&feed.url).await {
                    Ok(entries) => {
                        let fetch_duration = feed_start.elapsed().as_secs_f64();
                        let count = entries.len();

                        // Log how many entries we fetched and how long it took
                        info!(
                            feed      = %feed.name,
                            url       = %feed.url,
                            count     = count,
                            duration_s = fetch_duration,
                            "Fetched feed"
                        );

                        // Process each entry, tallying any errors
                        for entry in entries {
                            if let Err(e) = process_entry(&pool, &entry).await {
                                errors += 1;
                                error!(
                                    feed     = %feed.name,
                                    entry_id = ?entry.id,
                                    error    = %e,
                                    "Failed to process entry"
                                );
                            }
                        }

                        (feed.name, fetch_duration, count, errors)
                    }
                    Err(e) => {
                        let fetch_duration = feed_start.elapsed().as_secs_f64();
                        error!(
                            feed       = %feed.name,
                            url        = %feed.url,
                            error      = %e,
                            duration_s = fetch_duration,
                            "Failed to fetch feed"
                        );

                        // Treat as zero entries with one error
                        (feed.name, fetch_duration, 0, 1)
                    }
                }
            });
        }

        // Aggregate results from all feed tasks
        let mut total_entries = 0;
        let mut total_errors = 0;
        let mut total_duration = 0.0_f64;

        while let Some((_, dur, count, errs)) = tasks.next().await {
            total_duration += dur;
            total_entries += count;
            total_errors += errs;
        }

        // Log the cycle summary
        let cycle_secs = cycle_start.elapsed().as_secs_f64();
        info!(
            total_feeds = feeds.len(),
            total_entries = total_entries,
            total_errors = total_errors,
            avg_fetch_s = if feeds.len() > 0 {
                total_duration / (feeds.len() as f64)
            } else {
                0.0
            },
            cycle_s = cycle_secs,
            "Ingestion cycle complete"
        );

        // Await next tick
        ticker.tick().await;
    }
}
