# -----------------------------------------------------------------
# Build stage: compile Rust binary
# -----------------------------------------------------------------
    FROM rust:latest AS builder

    # Set working directory inside the container
    WORKDIR /usr/src/app
    
    # 1) Copy only Cargo manifest files to leverage Docker layer caching
    COPY Cargo.toml Cargo.lock ./
    
    # 2) Copy source code to the working directory
    COPY src ./src
    
    # 3) Download Rust dependencies (cached unless Cargo.toml or Cargo.lock change)
    RUN cargo fetch
    
    # 4) Copy the rest of the project files (e.g., config, templates)
    COPY . .
    
    # 5) Build the Rust application in release mode for optimized binary
    RUN cargo build --release
    
    # -----------------------------------------------------------------
    # Runtime stage: prepare minimal container to run the binary
    # -----------------------------------------------------------------
    FROM debian:bookworm
    
    # Update package lists and install necessary packages:
    # - libssl3 and ca-certificates for SSL support
    # - chromium for headless browser functionality required by headless_chrome crate
    RUN apt-get update && \
        apt-get install -y --no-install-recommends \
            libssl3 \
            ca-certificates \
            chromium && \
        rm -rf /var/lib/apt/lists/*
    
    # Create a non-root system user and group 'app' for safer execution
    RUN addgroup --system app && adduser --system --ingroup app app
    
    # Switch to the non-root user
    USER app:app
    
    # Set working directory for the app user
    WORKDIR /home/app
    
    # Copy the compiled Rust binary from the build stage
    COPY --from=builder /usr/src/app/target/release/rust_feed_ingestor /usr/local/bin/rust_feed_ingestor
    
    # Optionally copy default configuration file
    COPY Config.toml /home/app/Config.toml
    
    # Expose the port your app listens on (matches server_bind in config)
    EXPOSE 9100
    
    # Set the binary as the container entrypoint
    ENTRYPOINT ["rust_feed_ingestor"]