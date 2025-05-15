//! Entrypoint: sets up tracing/logging, runs database migrations, starts HTTP metrics & health server,
//! and begins the main OSINT feed ingestion loop.

use std::{net::SocketAddr, sync::Arc, time::Instant};

use futures::stream::{FuturesUnordered, StreamExt};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server};
use prometheus::{Encoder, TextEncoder};
use sqlx::postgres::PgPoolOptions;
use tokio::time::interval;
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, EnvFilter};

use rust_feed_ingestor::config::{Feed, Settings};
use rust_feed_ingestor::errors::IngestError;
use rust_feed_ingestor::ingestor::{
    entry_to_feed_item, fetch_feed, process_entry, sanitize_and_validate,
};
use rust_feed_ingestor::metrics::{self, ENTRIES_PROCESSED, SANITIZATION_FAILURES};

#[tokio::main]
async fn main() -> Result<(), IngestError> {
    // ───────────────────────────────────────────────────────────────
    // 1. Initialize tracing / logging
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
    // 4. HTTP server for metrics & health endpoints
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
                        // ─── METRICS ENDPOINT ────────────────────────────────
                        (&Method::GET, "/metrics") => {
                            let metrics_text = metrics::gather_metrics();
                            let encoder = TextEncoder::new();
                            let mime = encoder.format_type();
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

    tokio::spawn(async move {
        info!(%addr, "Starting metrics & health server");
        Server::bind(&addr)
            .serve(make_svc)
            .await
            .expect("Metrics server failed");
    });

    // ───────────────────────────────────────────────────────────────
    // 5. Main ingestion loop: fetch, parse, sanitize, store, and monitor feeds
    // ───────────────────────────────────────────────────────────────
    let feeds: Arc<Vec<Feed>> = Arc::new(settings.feeds.clone());
    let mut ticker = interval(settings.ingest_interval);

    loop {
        let cycle_start = Instant::now();
        info!("Starting ingestion cycle for {} feeds", feeds.len());

        let mut tasks = FuturesUnordered::new();
        for feed in feeds.iter().cloned() {
            let pool = pool.clone();
            let feed_url = feed.url.clone();
            let feed_name = feed.name.clone();
            tasks.push(async move {
                let feed_start = Instant::now();
                let mut errors: usize = 0;
                match fetch_feed(&feed_url).await {
                    Ok(feed_struct) => {
                        let fetch_duration = feed_start.elapsed().as_secs_f64();
                        let count = feed_struct.entries.len();
                        info!(
                            feed = %feed_name,
                            url = %feed_url,
                            count = count,
                            duration_s = fetch_duration,
                            "Fetched feed"
                        );
                        for entry in &feed_struct.entries {
                            let feed_item = entry_to_feed_item(entry, &feed_struct, &feed_url);
                            match sanitize_and_validate(&feed_item) {
                                Some(safe_item) => match process_entry(&pool, &safe_item).await {
                                    Ok(_) => {
                                        ENTRIES_PROCESSED.inc();
                                    }
                                    Err(e) => {
                                        errors += 1;
                                        error!(
                                            feed = %feed_name,
                                            entry_id = ?entry.id,
                                            error = %e,
                                            "Failed to process entry"
                                        );
                                    }
                                },
                                None => {
                                    errors += 1;
                                    SANITIZATION_FAILURES.inc();
                                    warn!(
                                        feed = %feed_name,
                                        entry_id = ?entry.id,
                                        "Entry failed sanitization/validation and was skipped"
                                    );
                                }
                            }
                        }
                        (feed_name, fetch_duration, count, errors)
                    }
                    Err(e) => {
                        let fetch_duration = feed_start.elapsed().as_secs_f64();
                        error!(
                            feed = %feed_name,
                            url = %feed_url,
                            error = %e,
                            duration_s = fetch_duration,
                            "Failed to fetch feed"
                        );
                        (feed_name, fetch_duration, 0, 1)
                    }
                }
            });
        }

        let mut total_entries: usize = 0;
        let mut total_errors: usize = 0;
        let mut total_duration: f64 = 0.0;
        while let Some((_, dur, count, errs)) = tasks.next().await {
            total_duration += dur;
            total_entries += count;
            total_errors += errs;
        }
        let cycle_secs = cycle_start.elapsed().as_secs_f64();
        info!(
            total_feeds = feeds.len(),
            total_entries = total_entries,
            total_errors = total_errors,
            avg_fetch_s = if !feeds.is_empty() {
                total_duration / (feeds.len() as f64)
            } else {
                0.0
            },
            cycle_s = cycle_secs,
            "Ingestion cycle complete"
        );
        ticker.tick().await;
    }
}
