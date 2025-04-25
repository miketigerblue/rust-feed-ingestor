# Rust OSINT Feed Ingestor

A best-practice, secure-by-default Rust service for ingesting RSS/Atom feeds into PostgreSQL, enriched with Prometheus metrics and structured tracing.

---

## Features

- **Modular ingestion**: Fetch & parse any number of RSS/Atom feeds on a configurable interval
- **Deduplication**: Append new items to an `archive` table and upsert into a `current` table
- **Observability**:
  - Structured logs with `tracing`
  - Prometheus metrics exposed on `/metrics`
  - Health-check endpoint on `/healthz`
- **Configuration**: Flexible loading via `Config.toml` or environment variables (prefix `APP__`)
- **Containerised**: Multi-stage Docker build; orchestrated via Docker Compose
- **CI pipeline**: GitHub Actions for `cargo fmt`, `clippy`, tests & Docker image build

---

## Quickstart

### Prerequisites

- Docker & Docker Compose
- Rust toolchain (for local development)

### Clone & Run

```bash
git clone https://github.com/miketigerblue/rust-osint-feed-ingestor.git
cd rust-osint-feed-ingestor

docker-compose up -d --build
```

This brings up:
- **PostgreSQL** on `localhost:5432`
- **Feed Ingestor** service on `localhost:9100`
- **Prometheus** on `localhost:9090`

### Verify

- Health: `curl http://localhost:9100/healthz` → `OK`
- Metrics: `curl http://localhost:9100/metrics`
- Prometheus UI: Open http://localhost:9090 and see `rust_feed_ingestor` targets

---

## Configuration

Configuration may be supplied via `Config.toml` (in repo root) or environment variables with the prefix `APP__` and `__` as separator:

| Setting           | TOML (`Config.toml`)          | Env var                     | Default            |
|-------------------|--------------------------------|-----------------------------|--------------------|
| `database_url`    | `database_url = "..."`       | `APP__DATABASE_URL`         | —                  |
| `feed_urls`       | `feed_urls = ["...","..."]`  | `APP__FEED_URLS` (CSV)      | —                  |
| `ingest_interval` | `ingest_interval = "1h"`     | `APP__INGEST_INTERVAL`      | `1h` (code fallback)|
| `server_bind`     | `server_bind = "0.0.0.0:9100"`| `APP__SERVER_BIND`          | `0.0.0.0:9100`     |

---

## Development

### Local build & test

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### CI

On push/PR to `main`, GitHub Actions runs:
1. `cargo fmt -- --check`
2. `cargo clippy` with warnings as errors
3. `cargo test`
4. Docker image build

---

## Docker Compose Services

- **db**: `postgres:13` with volume `db_data`
- **rust_ingestor**: built from `Dockerfile`, env-configured
- **prometheus**: `prom/prometheus:latest` with `prometheus.yml` scraping `rust_ingestor`

---

## Known Issues

- A future‐incompat warning is emitted from [`quick-xml v0.20.0`](https://github.com/tafia/quick-xml) (used by `feed-rs`). This is an upstream dependency; it does not affect functionality. The issue will be resolved when `feed-rs` upgrades their dependency. You can safely ignore this warning in your local build.

---

## Next Steps

- Add Grafana for dashboarding and alerting
- Write integration tests (e.g. with Testcontainers)
- Harden with TLS, secrets management, and Kubernetes manifests
- Publish GitHub Release v0.1.0

---

© 2025 Mike Harris — MIT License
