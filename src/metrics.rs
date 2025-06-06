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

/// Total number of feed entries that failed sanitization/validation
pub static SANITIZATION_FAILURES: Lazy<IntCounter> = Lazy::new(|| {
    let opts = Opts::new(
        "sanitization_failures_total",
        "Total number of feed entries that failed sanitization/validation",
    );
    let c = IntCounter::with_opts(opts).expect("counter opts");
    REGISTRY.register(Box::new(c.clone())).unwrap();
    c
});

/// Total number of successfully processed entries
pub static ENTRIES_PROCESSED: Lazy<IntCounter> = Lazy::new(|| {
    let opts = Opts::new(
        "entries_processed_total",
        "Total number of feed entries successfully sanitized and processed",
    );
    let c = IntCounter::with_opts(opts).expect("counter opts");
    REGISTRY.register(Box::new(c.clone())).unwrap();
    c
});

// Track which content extraction branch is being used (for observability and tuning)
pub static CONTENT_ENCODED_COUNT: Lazy<IntCounter> = Lazy::new(|| {
    let opts = Opts::new(
        "feed_content_encoded_entries_total",
        "Entries populated using <content:encoded> RSS extension",
    );
    let c = IntCounter::with_opts(opts).expect("counter opts");
    REGISTRY.register(Box::new(c.clone())).unwrap();
    c
});

pub static CONTENT_FIELD_COUNT: Lazy<IntCounter> = Lazy::new(|| {
    let opts = Opts::new(
        "feed_content_field_entries_total",
        "Entries populated using <content> field",
    );
    let c = IntCounter::with_opts(opts).expect("counter opts");
    REGISTRY.register(Box::new(c.clone())).unwrap();
    c
});

pub static SUMMARY_FALLBACK_COUNT: Lazy<IntCounter> = Lazy::new(|| {
    let opts = Opts::new(
        "feed_summary_fallback_entries_total",
        "Entries populated using <summary> field (fallback)",
    );
    let c = IntCounter::with_opts(opts).expect("counter opts");
    REGISTRY.register(Box::new(c.clone())).unwrap();
    c
});

/// Encode all metrics as text
pub fn gather_metrics() -> String {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let mf = REGISTRY.gather();
    encoder.encode(&mf, &mut buffer).expect("failed to encode");
    String::from_utf8(buffer).expect("invalid utf8")
}
