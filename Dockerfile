# Multi-stage Dockerfile for AMOS Rust API
# ==========================================
# Stage 1: Build dependencies and application
# Stage 2: Minimal runtime image with only the binary

# Stage 1: Builder
# ----------------
FROM rust:1.80-bookworm as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy workspace crates
COPY amos-core ./amos-core
COPY amos-solana ./amos-solana
COPY amos-agent-runtime ./amos-agent-runtime
COPY amos-api ./amos-api
COPY amos-cli ./amos-cli

# Build dependencies in release mode
# This layer is cached unless Cargo.toml or Cargo.lock changes
RUN cargo build --release --locked

# The binary is now at /app/target/release/amos

# Stage 2: Runtime
# ----------------
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -m -u 1000 amos

# Set working directory
WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/amos /usr/local/bin/amos

# Create directories for keys and cache
RUN mkdir -p /app/keys /app/cache && \
    chown -R amos:amos /app

# Switch to non-root user
USER amos

# Expose ports
# 4000: HTTP API (Axum)
# 4001: gRPC API (Tonic)
EXPOSE 4000 4001

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:4000/health || exit 1

# Default command
CMD ["amos", "serve"]

# Build args for metadata
ARG GIT_COMMIT=unknown
ARG BUILD_DATE=unknown
ARG VERSION=unknown

# Labels
LABEL org.opencontainers.image.title="AMOS Platform API"
LABEL org.opencontainers.image.description="Agent Management and Orchestration System - Rust API Server"
LABEL org.opencontainers.image.vendor="AMOS"
LABEL org.opencontainers.image.version="${VERSION}"
LABEL org.opencontainers.image.created="${BUILD_DATE}"
LABEL org.opencontainers.image.revision="${GIT_COMMIT}"
LABEL org.opencontainers.image.licenses="MIT"

# Build instructions:
# ===================
# Development:
#   docker build -t amos-api:dev .
#   docker run -p 4000:4000 -p 4001:4001 --env-file .env amos-api:dev
#
# Production with metadata:
#   docker build \
#     --build-arg VERSION=$(git describe --tags --always) \
#     --build-arg GIT_COMMIT=$(git rev-parse HEAD) \
#     --build-arg BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ") \
#     -t amos-api:latest \
#     -t ghcr.io/your-org/amos-api:latest \
#     .
#
# Push to GitHub Container Registry:
#   docker push ghcr.io/your-org/amos-api:latest
#
# Run with Docker Compose:
#   docker-compose up rust_api
#
# Cache optimization notes:
# - Cargo.toml and Cargo.lock are copied first for dependency caching
# - Source code is copied after dependencies are built
# - Use BuildKit for better caching: DOCKER_BUILDKIT=1 docker build
# - Layer sizes:
#   * Builder: ~2GB (includes Rust toolchain)
#   * Runtime: ~100MB (only binary + minimal deps)
