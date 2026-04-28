# Multi-stage build for AMOS Agent
# Autonomous AI agent that registers with harness and runs tasks
#
# Builder stage is byte-identical across all four service Dockerfiles.

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

# Install runtime dependencies + security tooling
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    iptables \
    gosu \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from the builder's /out (cache-mount-safe extraction)
COPY --from=builder /out/amos-agent /usr/local/bin/

# Create non-root user with workspace directories
# sandbox (uid 1001) is used for bash tool subprocesses (process isolation)
RUN useradd -m -u 1000 amos && \
    useradd -m -u 1001 -s /bin/sh sandbox && \
    mkdir -p /app/data /workspace /tmp/amos && \
    chown -R amos:amos /app /workspace /tmp/amos && \
    chown sandbox:sandbox /workspace /tmp/amos

# Copy entrypoint (runs as root for iptables, then drops to amos)
COPY docker/agent/entrypoint.sh /entrypoint.sh

WORKDIR /app

# Expose HTTP port
EXPOSE 3100

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:3100/health || exit 1

# Entrypoint applies network hardening then drops to amos user
ENTRYPOINT ["/entrypoint.sh"]
CMD ["amos-agent", "--serve"]
