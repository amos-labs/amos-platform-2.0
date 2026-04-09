# Multi-stage build for AMOS Relay
# Network relay / bounty marketplace

# Stage 1: Builder
FROM rust:bookworm AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./

# Copy all workspace crate sources (cargo requires all members for workspace resolution)
COPY amos-core ./amos-core
COPY amos-harness ./amos-harness
COPY amos-cli ./amos-cli
COPY amos-agent ./amos-agent
COPY amos-relay ./amos-relay
COPY amos-packages ./amos-packages

# Build only the relay binary
RUN SQLX_OFFLINE=true cargo build --release --bin amos-relay

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/amos-relay /usr/local/bin/

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

# Run the relay
CMD ["amos-relay"]
