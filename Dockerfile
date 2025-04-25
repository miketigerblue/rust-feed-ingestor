# Build stage
FROM rust:1.70 as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo install --path . --locked --root /usr/local

# Runtime stage
FROM debian:bullseye-slim
env RUST_LOG=info
COPY --from=builder /usr/local/bin/rust_feed_ingestor /usr/local/bin/

# Config file (optional)
COPY Config.toml /
EXPOSE 9100
ENTRYPOINT ["rust_feed_ingestor"]