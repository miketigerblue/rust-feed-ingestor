# TigerBlue Feed‑Ingestor

_A Rust async micro‑service that collects and stores cyber‑security RSS/Atom feeds with built‑in metrics and structured tracing._

This repository is the ingestion layer used by the TigerBlue OSINT dashboard.  Its remit is deliberately narrow: **pull trusted feeds on a schedule, deduplicate items, and persist them to PostgreSQL while exposing operational telemetry**.

---

## Core design points

| Aspect | Implementation |
|--------|----------------|
| **Feed management** | Feed URLs and poll interval are declared in `Config.toml` (or environment).  A Tokio task scheduler issues parallel `reqwest` GETs, honouring each channel’s `<ttl>` to avoid over‑pulling. |
| **Parsing** | `feed-rs` and `quick-xml` translate RSS/Atom into strongly‑typed structs.  Items are deduplicated by GUID/hash before they touch the database. |
| **Persistence** | Async inserts via `sqlx` into PostgreSQL.  Duplicates are ignored with `ON CONFLICT DO NOTHING`; a single table `items` stores the canonical copy. |
| **Observability** | Prometheus metrics (`/metrics`) track ingest counts, HTTP status codes and processing latency.  `/healthz` performs a live DB ping and returns JSON. |
| **Runtime profile** | One Tokio runtime, bounded‑concurrency semaphore (defaults to 32 simultaneous HTTP requests).  Back‑pressure kicks in when either the DB pool or HTTP queue is saturated. |
| **Deployment** | Multi‑stage Docker build (~18 MB compressed).  `docker‑compose.yml` launches Postgres, the ingestor, and Prometheus in three containers. |

---

## Data flow

```
┌────────────┐   HTTP/HTTPS   ┌──────────────┐     SQL    ┌────────────┐
│  Fetcher   │ ─────────────▶ │ Deduper      │ ────────▶ │  Postgres  │
└────────────┘                └──────────────┘            └────────────┘
   async reqwest                 in‑memory set               sqlx/SQL
```

1. **Fetcher** – parallel HTTP GET with timeout and per‑host rate‑limit.
2. **Deduper** – GUID or link hash checked against an LRU cache; unseen items forwarded.
3. **Store** – `sqlx` executes batch inserts inside a transaction; duplicate GUIDs are dropped via `ON CONFLICT`.

If any stage errors, the task retries with exponential back‑off (configurable), and a Prometheus counter increments `ingest_failures_total{step="fetch"|"parse"|"store"}`.

---

## Quick start

```bash
# Clone
$ git clone https://github.com/miketigerblue/rust-feed-ingestor.git
$ cd rust-feed-ingestor

# Launch stack
$ docker compose up -d --build

# Verify
$ curl http://localhost:9100/healthz   # "OK"
$ curl http://localhost:9100/metrics   # plain-text Prometheus page
```

### Containers in the default `docker‑compose.yml`

| Name | Image (tag) | Responsibility | Persistent data | Exposed port(s) | Key environment / args |
|------|-------------|----------------|-----------------|-----------------|-------------------------|
| **db** | `postgres:16-alpine` | Primary relational store for ingested feed items. Starts with the `uuid-ossp` extension enabled. | `db-data` volume at `/var/lib/postgresql/data` | **5432/tcp** | `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB` |
| **ingestor** | Local multi‑stage build (`rust:1.78` → `gcr.io/distroless/cc`) | Polls feeds, deduplicates, stores items, and serves `/metrics` & `/healthz`. | None (stateless) | **9100/tcp** | `DATABASE_URL`, `INGEST_INTERVAL`, `RUST_LOG` |
| **prometheus** | `prom/prometheus:v2` | Scrapes `ingestor` every 15 s; retains a 15‑day time‑series window; ships a basic alert rule for ingest downtime. | `prom-data` volume | **9090/tcp** | Prom config file path |
| **grafana** | `grafana/grafana:10` | Visualises Prometheus data using dashboards in `grafana/`. | `grafana-data` volume | **3000/tcp** | `GF_SECURITY_ADMIN_PASSWORD`, etc. |
| **pgadmin** | `dpage/pgadmin4:8` | Web UI for inspecting Postgres. | `pgadmin-data` volume | **5050/tcp** | `PGADMIN_DEFAULT_EMAIL`, `PGADMIN_DEFAULT_PASSWORD` |
| **postgrest** | `postgrest/postgrest:v11` | REST API over Postgres schema. | None | **3001/tcp** | `PGRST_DB_URI`, `PGRST_JWT_SECRET` |

---

## Configuration reference (`Config.toml`)

```toml
database_url    = "postgres://postgres:postgres@db:5432/osint"
ingest_interval = "1h"
server_bind     = "0.0.0.0:9100"

[[feeds]]
name = "CISA Alerts"
url  = "https://us-cert.cisa.gov/ncas/alerts.xml"
```

---

## Build & test

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Integration tests spin up Postgres via Testcontainers and execute the full ingest pipeline against fixture feeds.

---

## Continuous Integration (GitHub Actions)

File: `.github/workflows/ci.yml`

### Workflow overview

1. Provision Postgres 13 service.
2. Cache Cargo registry & index.
3. Run rustfmt & clippy.
4. Security scans: `cargo-audit`, `cargo-deny`, `cargo-geiger`, CodeQL.
5. Run tests.
6. Apply SQLx migrations.
7. Build multi-arch image with Buildx.
8. Trivy scan + SBOM generation.
9. Upload binary & SBOM artefacts.

### Triggers & environment

* Runs on push and PR to `main`.
* Nightly cron job refreshes images.
* Secrets: `GHCR_PAT`, optional `SLACK_WEBHOOK_URL`.

---

## Roadmap

* Configurable dedup window.
* TLS for feed sources.
* Helm chart.
* Grafana dashboard JSON.

---

## Licence

Code: MIT — Docs: CC BY‑SA 4.0
