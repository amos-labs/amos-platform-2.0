# Multi-stage build for AMOS Platform
# Central token economics, governance, billing

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

# Build only the platform binary
RUN cargo build --release --bin amos-platform

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
COPY --from=builder /app/target/release/amos-platform /usr/local/bin/

# Create non-root user
RUN useradd -m -u 1000 amos && \
    mkdir -p /app/data /app/keys && \
    chown -R amos:amos /app

USER amos
WORKDIR /app

# Expose HTTP and gRPC ports
EXPOSE 4000 4001

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:4000/health || exit 1

# Run the platform
CMD ["amos-platform"]
