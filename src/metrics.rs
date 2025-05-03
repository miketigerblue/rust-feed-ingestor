//! Prometheus metrics registry and metric definitions.

use once_cell::sync::Lazy;
use prometheus::{Encoder, Histogram, HistogramOpts, IntCounter, Opts, Registry, TextEncoder};

/// Global registry under crate namespace
pub static REGISTRY: Lazy<Registry> = Lazy::new(|| {
    Registry::new_custom(Some("rust_feed_ingestor".into()), None)
        .expect("failed to create Prometheus registry")
});

/// Total fetch attempts
pub static FETCH_COUNTER: Lazy<IntCounter> = Lazy::new(|| {
    let opts = Opts::new("feeds_fetched_total", "Total number of feed fetch attempts");
    let c = IntCounter::with_opts(opts).expect("counter opts");
    REGISTRY.register(Box::new(c.clone())).unwrap();
    c
});

/// Histogram of fetch+parse durations
pub static FETCH_HISTOGRAM: Lazy<Histogram> = Lazy::new(|| {
    let opts = HistogramOpts::new(
        "fetch_duration_seconds",
        "Duration of feed fetch+parse in seconds",
    );
    let h = Histogram::with_opts(opts).expect("histogram opts");
    REGISTRY.register(Box::new(h.clone())).unwrap();
    h
});

/// Encode all metrics as text
pub fn gather_metrics() -> String {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let mf = REGISTRY.gather();
    encoder.encode(&mf, &mut buffer).expect("failed to encode");
    String::from_utf8(buffer).expect("invalid utf8")
}
