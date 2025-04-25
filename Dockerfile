# -----------------------------------------------------------------
# Build stage
# -----------------------------------------------------------------
    FROM rust:latest AS builder
    WORKDIR /usr/src/app
    
    # 1) Copy only manifest files
    COPY Cargo.toml Cargo.lock ./
    
    # 2) Copy source so Cargo can see your targets
    COPY src ./src
    
    # 3) Download dependencies (cached unless Cargo.toml/Cargo.lock change)
    RUN cargo fetch
    
    # 4) Now copy the rest of your code (e.g. config, templates, etc.)
    COPY . .
    
    # 5) Build in release mode
    RUN cargo build --release
    
# -----------------------------------------------------------------
# Runtime stage
# -----------------------------------------------------------------
    FROM debian:bookworm-slim

    # Install OpenSSL 3 and CA certs
    RUN apt-get update \
        && apt-get install -y --no-install-recommends libssl3 ca-certificates \
        && rm -rf /var/lib/apt/lists/*
   

    # Create a non-root user
    RUN addgroup --system app && adduser --system --ingroup app app
    USER app:app
    
    WORKDIR /home/app
    
    # Copy the statically-linked binary
    COPY --from=builder /usr/src/app/target/release/rust_feed_ingestor /usr/local/bin/rust_feed_ingestor
    
    # (Optional) Default config
    COPY Config.toml /home/app/Config.toml
    
    EXPOSE 9100
    ENTRYPOINT ["rust_feed_ingestor"]
    