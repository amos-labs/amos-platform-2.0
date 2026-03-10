# AMOS

**Autonomous Management Operating System** -- an AI-native business operating system written in pure Rust.

AMOS provides a per-customer AI harness (the "operating system") that hosts tools, canvases, schemas, and data -- while autonomous agents connect via the External Agent Protocol to do the thinking. The harness never runs its own agent loop; agents are independent processes that register, pull tasks, call harness tools over HTTP, and report results.

## Architecture

```
amos-automate/
├── amos-core       Shared types, config, errors, token economics
├── amos-harness    Per-customer OS (tools, canvas engine, schemas, sites, agent registry)
├── amos-agent      Default autonomous agent (Bedrock, model registry, task consumer)
├── amos-platform   Multi-tenant control plane (provisioning, billing, governance)
├── amos-cli        Command-line interface for both harness and platform
├── amos-solana/    On-chain programs (treasury, bounties, governance) -- built via Anchor
├── docker/         Production Dockerfiles (harness, platform, agent)
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
│      (per-customer OS / tool host / marketplace)     │
│                                                       │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐ │
│  │  Canvas   │  │  Schema   │  │      Tools         │ │
│  │  Engine   │  │ (runtime  │  │  (54+ tools:       │ │
│  │  (iframe) │  │  defined) │  │   db, web, files,  │ │
│  └──────────┘  └──────────┘  │   canvas, agents)   │ │
│  ┌──────────┐  ┌──────────┐  └────────────────────┘ │
│  │ Sessions  │  │   Sites   │  ┌──────────────────┐  │
│  │ Memory    │  │  (public) │  │  Agent Registry   │  │
│  └──────────┘  └──────────┘  └──────────────────┘  │
└──────────────────────┬──────────────────────────────┘
                       │ External Agent Protocol (register, tasks, tools, heartbeat)
          ┌────────────┴────────────┐
          ▼                         ▼
┌──────────────────┐  ┌──────────────────────────────┐
│   amos-agent     │  │  External / 3rd-party agents  │
│  (default agent) │  │  (same protocol, same access) │
│                  │  │                                │
│  Agent Loop      │  │  Any language / framework      │
│  Bedrock/OpenAI  │  │  EAP-compatible                │
│  Model Registry  │  │  /.well-known/agent.json       │
│  Local Tools     │  │                                │
│  Task Consumer   │  │                                │
└──────────────────┘  └──────────────────────────────┘
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

# Run tests (~300 tests)
cargo test --workspace

# Run the harness (terminal 1)
AMOS__DATABASE__URL=postgres://user@localhost:5432/amos_dev \
  cargo run --bin amos-harness
# → http://localhost:3000

# Run the platform (terminal 2)
AMOS__DATABASE__URL=postgres://user@localhost:5432/amos_platform_dev \
  AMOS__SERVER__PORT=4000 \
  cargo run --bin amos-platform
# → http://localhost:4000

# Run the agent (terminal 3)
cargo run --bin amos-agent
# → Interactive mode, type messages to chat

# Or in service mode (HTTP API + task consumer):
AMOS_SERVE=true cargo run --bin amos-agent
# → http://localhost:3100 (auto-registers with harness)
```

### Docker Development

```bash
# Start everything (postgres, redis, localstack, platform, harness, agent)
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

### Agent Chat (SSE streaming) -- amos-agent :3100

```
POST /api/v1/chat
Content-Type: application/json

{"message": "Create a dashboard showing monthly revenue"}
```

Returns Server-Sent Events: `text_delta`, `tool_start`, `tool_end`, `error`, `done`.

### Harness Endpoints -- amos-harness :3000

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/canvases` | List canvases |
| `GET` | `/api/v1/agents` | List registered agents |
| `POST` | `/api/v1/agents/register` | Register an agent |
| `GET` | `/api/v1/sessions` | List chat sessions |
| `POST` | `/api/v1/tools/{name}/execute` | Execute a harness tool |
| `GET` | `/api/v1/tasks/next` | Pull next pending task (agent polling) |
| `POST` | `/api/v1/tasks/{id}/result` | Report task result |
| `GET` | `/c/{slug}` | Public canvas |
| `GET` | `/s/{slug}` | Public site |
| `GET` | `/health` | Health check |

### Agent Endpoints -- amos-agent :3100

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/chat` | Chat with agent (SSE) |
| `GET` | `/.well-known/agent.json` | Agent Card (A2A discovery) |
| `GET` | `/health` | Health check |

## Deployment Modes

**Managed** (default): AMOS provisions and manages harness containers. Compute costs include 20% markup.

**Self-Hosted**: Customers run AMOS on their own infrastructure with their own models. No compute markup on customer-owned models. Supports air-gapped operation.

## Token Economics

AMOS uses a Solana-based SPL token with a decay-based ownership model. 100M fixed supply. 50% of platform revenue distributed to token holders.

See [docs/whitepaper_technical.md](docs/whitepaper_technical.md) for the full specification.

## License

Apache-2.0
