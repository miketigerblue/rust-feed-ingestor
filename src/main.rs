//! Entrypoint: setup tracing, run migrations, start metrics server & ingestion loop.

use rust_feed_ingestor::config::Settings;
use rust_feed_ingestor::errors::IngestError;
use rust_feed_ingestor::metrics;
use rust_feed_ingestor::ingestor::{fetch_feed, process_entry};
use sqlx::postgres::PgPoolOptions;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::sync::Arc;
use tokio::time::interval;

#[tokio::main]
async fn main() -> Result<(), IngestError> {
    // 1. Initialise tracing/logging
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // 2. Load configuration
    let settings = Settings::new()?;
    info!(?settings, "Loaded settings");

    // 3. Create Postgres connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&settings.database_url)
        .await?;

    // 4. Run any pending SQLx migrations from the `migrations/` directory
    info!("Running database migrations...");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    // 5. Start the metrics & health HTTP server
    let addr = settings.server_bind.parse().expect("invalid bind address");
    let make_svc = make_service_fn(move |_| {
        async move {
            Ok::<_, IngestError>(service_fn(|req: Request<Body>| async move {
                match (req.uri().path(), req.method()) {
                    ("/metrics", &hyper::Method::GET) => {
                        let body = metrics::gather_metrics();
                        Ok::<_, IngestError>(Response::new(Body::from(body)))
                    }
                    ("/healthz", &hyper::Method::GET) => {
                        Ok(Response::new(Body::from("OK")))
                    }
                    _ => Ok(Response::builder()
                        .status(404)
                        .body(Body::empty())
                        .unwrap()),
                }
            }))
        }
    });

    // Spawn the HTTP server in the background
    tokio::spawn(async move {
        info!(%addr, "Starting metrics server");
        Server::bind(&addr)
            .serve(make_svc)
            .await
            .expect("Metrics server failed");
    });

    // 6. Ingestion loop
    let feeds = Arc::new(settings.feed_urls.clone());
    let mut ticker = interval(settings.ingest_interval);

    loop {
        ticker.tick().await;
        for url in feeds.iter() {
            match fetch_feed(url).await {
                Ok(entries) => {
                    for entry in entries {
                        if let Err(e) = process_entry(&pool, &entry).await {
                            error!(feed = %url, error = %e, "Entry processing failed");
                        }
                    }
                }
                Err(e) => error!(feed = %url, error = %e, "Fetch failed"),
            }
        }
    }
}
