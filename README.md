# AMOS

**Autonomous Management Operating System** -- an AI-native business operating system written in pure Rust.

AMOS provides a per-customer AI harness where an AI agent builds workflows, automations, integrations, websites, and applications through natural language conversation. The system combines a tool-driven agent loop with a dynamic canvas engine and runtime-defined data schemas.

## Architecture

```
amos-automate/
├── amos-core       Shared types, config, errors, token economics
├── amos-harness    Per-customer AI runtime (agent loop, tools, UI, canvas engine)
├── amos-platform   Multi-tenant control plane (provisioning, billing, governance)
├── amos-cli        Command-line interface for both harness and platform
├── amos-agent      Standalone autonomous agent (same protocol as external agents)
├── amos-solana/    On-chain programs (treasury, bounties, governance) -- built via Anchor
├── docker/         Production Dockerfiles
└── docs/           Whitepaper, token economics
```

### How it fits together

```
┌─────────────────────────────────────────────────────┐
│                  amos-platform                       │
│          (multi-tenant control plane)                │
│   provisioning · billing · governance · sync API     │
└───────────────┬─────────────────────────────────────┘
                │ HTTP (heartbeat, config, usage)
┌───────────────▼─────────────────────────────────────┐
│                  amos-harness                         │
│            (per-customer instance)                    │
│                                                       │
│  ┌─────────┐  ┌──────────┐  ┌────────────────────┐  │
│  │  Agent   │  │  Canvas   │  │      Tools         │  │
│  │  Loop    │→ │  Engine   │  │  (30+ tools:       │  │
│  │ (Bedrock)│  │  (iframe) │  │   db, web, files,  │  │
│  └─────────┘  └──────────┘  │   schema, agents)   │  │
│       ↕                      └────────────────────┘  │
│  ┌─────────┐  ┌──────────┐  ┌──────────────────┐    │
│  │ Sessions │  │  Schema   │  │    Sites          │    │
│  │ Memory   │  │ (runtime  │  │  (public pages)   │    │
│  │          │  │  defined) │  │                    │    │
│  └─────────┘  └──────────┘  └──────────────────┘    │
└──────────────────────────────────────────────────────┘
                ↕ same protocol
┌──────────────────────────────────────────────────────┐
│  amos-agent (standalone) / external agents            │
└──────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

- Rust >= 1.88
- PostgreSQL >= 15 (with pgvector recommended)
- Redis
- AWS credentials configured for Bedrock (Claude model access)

### Local Development

```bash
# Build the workspace
cargo build

# Run tests (346 tests)
cargo test --workspace

# Run the harness
AMOS__DATABASE__URL=postgres://user@localhost:5432/amos_dev \
  cargo run --bin amos-harness
# → http://localhost:3000

# Run the platform (separate terminal)
AMOS__DATABASE__URL=postgres://user@localhost:5432/amos_platform_dev \
  AMOS__SERVER__PORT=4000 \
  cargo run --bin amos-platform
# → http://localhost:4000
```

### Docker Development

```bash
# Start everything (postgres, redis, localstack, platform, harness)
docker compose up --build

# Or just infrastructure
docker compose up postgres redis -d
```

## Configuration

All config uses the `AMOS__` prefix with `__` as nested separator:

| Variable | Default | Description |
|----------|---------|-------------|
| `AMOS__DATABASE__URL` | -- | PostgreSQL connection string (required) |
| `AMOS__SERVER__PORT` | `3000` | HTTP bind port |
| `AMOS__REDIS__URL` | `redis://127.0.0.1:6379` | Redis connection string |
| `AMOS__AGENT__MAX_ITERATIONS` | `25` | Max agent loop iterations per request |
| `AMOS__DEPLOYMENT__MODE` | `managed` | `managed` or `self_hosted` |

AWS credentials for Bedrock are read from the standard AWS credential chain.

## API

### Chat (SSE streaming)

```
POST /api/v1/agent/chat
Content-Type: application/json

{"message": "Create a dashboard showing monthly revenue", "session_id": null}
```

Returns Server-Sent Events: `turn_start`, `message_delta`, `tool_start`, `tool_end`, `turn_end`, `agent_end`.

### Key Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/agent/chat` | Chat with agent (SSE) |
| `GET` | `/api/v1/canvases` | List canvases |
| `GET` | `/api/v1/agents` | List registered agents |
| `GET` | `/api/v1/sessions` | List chat sessions |
| `GET` | `/c/{slug}` | Public canvas |
| `GET` | `/s/{slug}` | Public site |
| `GET` | `/health` | Health check |

## Deployment Modes

**Managed** (default): AMOS provisions and manages harness containers. Compute costs include 20% markup.

**Self-Hosted**: Customers run AMOS on their own infrastructure with their own models. No compute markup on customer-owned models. Supports air-gapped operation.

## Token Economics

AMOS uses a Solana-based SPL token with a decay-based ownership model. 100M fixed supply. 50% of platform revenue distributed to token holders.

See [docs/whitepaper_technical.md](docs/whitepaper_technical.md) for the full specification.

## License

Apache-2.0
