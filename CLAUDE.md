# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AMOS (Autonomous Management Operating System) is open infrastructure for autonomous work written in pure Rust. This repo contains the open-source harness, core, packages, and CLI. The managed platform lives in a separate private repo; the AMOS protocol (relay, Oracle, Solana programs, agent) — the long-game economic layer, actively developed as a side track — lives at https://github.com/amos-labs/amos-protocol (WS-4 extraction).

## Build & Run Commands

```bash
# Build entire workspace
cargo build
cargo build --release

# Run individual binaries
cargo run --bin amos-harness     # Port 3000
cargo run --bin amos-cli

# Tests (per crate)
cargo test --lib -p amos-harness
cargo test --lib -p amos-core
cargo test --lib -p amos-cli

# Lint & format
cargo clippy
cargo fmt
cargo check

# Docker
docker compose up --build                  # Full stack
docker compose up postgres redis -d        # Just infrastructure

# Dev setup (macOS native)
./scripts/dev-setup.sh           # Full setup
./scripts/dev-setup.sh start     # Start PostgreSQL + Redis
./scripts/dev-setup.sh stop      # Stop services
./scripts/dev-setup.sh reset     # Drop and recreate databases
```

## Workspace Structure

| Crate | Binary | Purpose |
|-------|--------|---------|
| `amos-core` | (library) | Shared config, errors, types, token economics |
| `amos-harness` | `amos-harness` | Per-customer AI runtime (agent loop, tools, canvas, sites) |
| `amos-cli` | `amos` | Admin CLI |

## Architecture

**amos-core** — Foundation crate all others depend on. Config loads from env vars with `AMOS__` prefix and `__` nested separator (e.g., `AMOS__DATABASE__URL`). Unified `AmosError`/`Result` types.

**amos-harness** — The core product. Key subsystems:
- `src/agent/` — V3 event-driven agent loop, streams via SSE, calls Claude on AWS Bedrock
- `src/tools/` — 30+ tools implementing the `Tool` trait, registered in `ToolRegistry::default_registry()`. To add a tool: implement `Tool`, register it there.
- `src/canvas/` — Dynamic UI generation rendered in sandboxed iframes (Tera templates)
- `src/schema/` — Runtime-defined collections/records backed by JSONB, no per-customer migrations
- `src/sites/` — Public multi-page websites served at `/s/{slug}`
- `src/openclaw/` — Autonomous agent management (register, activate, task assignment, WebSocket gateway)
- `src/memory/` — Semantic memory with salience scoring and pgvector search
- `src/routes/` — Axum HTTP handlers (chat SSE at `POST /api/v1/agent/chat`)
- `static/` — Frontend SPA (plain JS + Tailwind CSS + Lucide icons, no framework)
- `migrations/` — 26 sqlx migrations, run automatically on startup



## Key Dependencies

- **Web**: Axum 0.8, Tokio, Tower
- **Database**: sqlx 0.8 (compile-time checked queries), PostgreSQL, pgvector
- **Cache**: Redis 0.27
- **AI**: AWS Bedrock (Claude) via aws-sdk-bedrockruntime
- **Blockchain**: Solana SDK 2.1, Anchor 0.30.1
- **Templating**: Tera 1.20
- **Rust edition**: 2021, MSRV 1.88

## Protocol & Agent Economy

- **Protocol-era context (relay/bounty/token)** now lives in [amos-labs/amos-protocol](https://github.com/amos-labs/amos-protocol).
- **[docs/README.md](docs/README.md)** — Current docs index and reading paths.
- **[docs/protocol/proof-carrying-loop.md](docs/protocol/proof-carrying-loop.md)** — Proof receipt, Oracle, failure capsule, and self-modifying guardrails.

## Configuration

Environment variables use `AMOS__` prefix with `__` separator. See `.env.example`. Key vars:
- `AMOS__DATABASE__URL` — PostgreSQL connection string
- `AMOS__REDIS__URL` — Redis connection string (default: `redis://127.0.0.1:6379`)
- `AMOS__SERVER__HOST` / `AMOS__SERVER__PORT` — Bind address (default: `0.0.0.0:3000`)
- `AMOS__AGENT__MAX_ITERATIONS` — Max agent loop iterations (default: 25)
- `AWS_PROFILE` — AWS profile for Bedrock access
