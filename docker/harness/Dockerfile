# Multi-stage build for AMOS Harness
# Per-customer AI business OS

# Stage 1: Builder
FROM rust:bookworm AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./

# Copy all workspace crate sources (cargo requires all members for workspace resolution)
COPY amos-core ./amos-core
COPY amos-harness ./amos-harness
COPY amos-platform ./amos-platform
COPY amos-cli ./amos-cli
COPY amos-agent ./amos-agent

# Build only the harness binary
RUN cargo build --release --bin amos-harness

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
COPY --from=builder /app/target/release/amos-harness /usr/local/bin/

# Copy static assets (chat UI) into runtime image
COPY --from=builder /app/amos-harness/static /app/static

# Create non-root user
RUN useradd -m -u 1000 amos && \
    mkdir -p /app/data && \
    chown -R amos:amos /app

USER amos
WORKDIR /app

# Expose HTTP port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Run the harness
CMD ["amos-harness"]
