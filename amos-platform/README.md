# AMOS Platform

The central service for the Autonomous Management Operating System (AMOS).

## Overview

This is the **single centralized platform** that all customer harnesses connect to. It provides:

- **Token Economics Engine**: Decay calculations, emission schedules, revenue distribution
- **Governance System**: On-chain proposals, voting, quality gates
- **Customer Billing**: Subscription management, usage tracking, plan limits
- **Harness Provisioning**: Docker-based container lifecycle management for customer harnesses
- **Solana Integration**: On-chain data queries and transaction submission
- **gRPC API**: Harness-to-platform communication
- **REST API**: Admin dashboard and operations

## Architecture

```
Customer Harness 1 ──┐
Customer Harness 2 ──┼──> [gRPC Server:4001]
Customer Harness N ──┘              │
                                    │
Admin Dashboard ──────────> [HTTP Server:4000]
                                    │
                                    ▼
                           ┌──────────────────┐
                           │  Platform State  │
                           │  (DB, Redis, RPC)│
                           └────────┬─────────┘
                    ┌───────────────┼───────────────┐
                    │               │               │
            ┌───────▼──────┐ ┌──────▼─────┐ ┌──────▼─────┐
            │ Token Econ.  │ │ Governance │ │ Provisioning│
            │ (amos-core)  │ │  Module    │ │   (Docker) │
            └──────────────┘ └────────────┘ └────────────┘
                    │
                    ▼
            [Solana Devnet/Mainnet]
```

## Components

### Core Modules

- **`billing/`**: Customer accounts, subscriptions, usage metrics, plan limits
- **`governance/`**: Proposals, voting, quality gates (benchmark, A/B test, feedback, steward)
- **`provisioning/`**: Harness container lifecycle using Docker API (bollard)
- **`solana/`**: RPC client for on-chain treasury, governance, and bounty programs
- **`middleware/`**: Authentication (API keys, harness tokens) and error handling

### API Routes

- **`/api/v1/health`**: Liveness check
- **`/api/v1/readiness`**: Readiness check (DB, Redis, Solana)
- **`/api/v1/token/*`**: Token economics endpoints (stats, decay, emission, revenue split)
- **`/api/v1/governance/*`**: Governance endpoints (proposals, votes, gates)
- **`/api/v1/billing/*`**: Customer management and billing
- **`/api/v1/provision/*`**: Harness provisioning and lifecycle

## Running

### Prerequisites

- PostgreSQL (connection pool for persistent state)
- Redis (session and cache)
- Docker (for harness provisioning)
- Solana RPC endpoint (devnet or mainnet)

### Configuration

Set environment variables (or use `.env` file):

```bash
AMOS_DATABASE__URL=postgres://user:pass@localhost/amos_platform
AMOS_REDIS__URL=redis://localhost:6379
AMOS_SOLANA__RPC_URL=https://api.devnet.solana.com
AMOS_SERVER__PORT=4000
AMOS_SERVER__GRPC_PORT=4001
```

### Start the server

```bash
cargo run --bin amos-platform
```

The platform will:
1. Load configuration from environment
2. Connect to PostgreSQL and Redis
3. Initialize Solana client (if available)
4. Run database migrations
5. Start HTTP server on port 4000
6. Start gRPC server on port 4001 (placeholder)

## Development Status

- **Token Economics API**: ✅ Complete (uses real amos-core functions)
- **Health Checks**: ✅ Complete (DB, Redis, Solana)
- **Governance API**: 🚧 Stub endpoints (database integration pending)
- **Billing API**: 🚧 Stub endpoints (payment provider integration pending)
- **Provisioning API**: 🚧 Stub endpoints (full Docker integration pending)
- **gRPC Server**: 📝 Placeholder (needs tonic service definitions)
- **Database Migrations**: 📝 Directory created (migrations pending)

## Testing

```bash
cargo test --lib -p amos-platform
```

## Dependencies

- **amos-core**: Shared types, config, error handling, token economics
- **axum**: HTTP framework
- **tonic**: gRPC framework (pending implementation)
- **sqlx**: PostgreSQL client with compile-time query verification
- **redis**: Redis client
- **bollard**: Docker API client
- **solana-client**: Solana RPC client
