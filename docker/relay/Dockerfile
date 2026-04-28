# Multi-stage build for AMOS Relay
# Network relay / bounty marketplace
#
# Builder stage is byte-identical across all four service Dockerfiles
# (harness/agent/relay/oracle). Combined with shared BuildKit cache mounts
# on the cargo registry + target dir, the heavy dep compilation runs once
# and is reused across all four images. Each runtime stage just copies the
# already-built binary out of the cache mount via /out.

# syntax=docker/dockerfile:1.7

# Stage 1: Builder (shared across all four service images)
FROM rust:bookworm AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./

# Copy all workspace crate sources (cargo requires all members for workspace resolution)
COPY amos-core ./amos-core
COPY amos-harness ./amos-harness
COPY amos-cli ./amos-cli
COPY amos-agent ./amos-agent
COPY amos-oracle ./amos-oracle
COPY amos-relay ./amos-relay
COPY amos-packages ./amos-packages

# Build all four service binaries in one RUN with cache mounts on cargo
# registry + target. Cache mounts don't survive into the image layer, so
# we copy the binaries to /out before exiting the RUN.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/app/target,sharing=locked \
    RUST_MIN_STACK=16777216 SQLX_OFFLINE=true cargo build --release \
        --bin amos-harness --features "pkg-education,pkg-autoresearch" \
    && cargo build --release --bin amos-agent \
    && cargo build --release --bin amos-relay \
    && cargo build --release --bin amos-oracle-agent \
    && mkdir -p /out \
    && cp target/release/amos-harness /out/ \
    && cp target/release/amos-agent /out/ \
    && cp target/release/amos-relay /out/ \
    && cp target/release/amos-oracle-agent /out/

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from the builder's /out (cache-mount-safe extraction)
COPY --from=builder /out/amos-relay /usr/local/bin/

# Create non-root user
RUN useradd -m -u 1000 amos && \
    mkdir -p /app/data && \
    chown -R amos:amos /app

USER amos
WORKDIR /app

# Expose HTTP port
EXPOSE 4100

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:4100/health || exit 1

# Entrypoint: write oracle keypair from env var to file, then run relay
COPY docker/relay/entrypoint.sh /usr/local/bin/entrypoint.sh

# Run the relay
CMD ["/usr/local/bin/entrypoint.sh"]
