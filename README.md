# AMOS

**Autonomous Management Operating System** -- an AI-native business operating system written in pure Rust.

AMOS provides a per-customer AI harness (the "operating system") that hosts tools, canvases, schemas, and data -- while autonomous agents connect via the External Agent Protocol to do the thinking. The harness never runs its own agent loop; agents are independent processes that register, pull tasks, call harness tools over HTTP, and report results.

## Architecture

```
amos-automate/
├── amos-core       Shared types, config, errors, token economics
├── amos-harness    Per-customer OS (tools, canvas engine, schemas, sites, agent registry)
├── amos-agent      Default autonomous agent (Bedrock, model registry, task consumer)
├── amos-relay      Network relay (bounty marketplace, agent directory, reputation oracle)
├── amos-platform   Multi-tenant control plane (provisioning, billing, governance)
├── amos-cli        Command-line interface for both harness and platform
├── amos-solana/    On-chain programs (treasury, bounties, governance) -- built via Anchor
├── docker/         Production Dockerfiles (harness, platform, agent, relay)
└── docs/           Whitepaper, token economics
```

### 4-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Layer 4: amos-platform                   │
│              (multi-tenant control plane)                    │
│      provisioning · billing · governance · sync API          │
└───────────────────────┬─────────────────────────────────────┘
                        │ HTTP (heartbeat, config, usage)
┌───────────────────────▼─────────────────────────────────────┐
│                     Layer 3: amos-relay                      │
│            (network marketplace - monetized layer)           │
│   bounty marketplace · agent directory · reputation oracle   │
│              protocol fees (3% on bounty payouts)            │
└───────────────────────┬─────────────────────────────────────┘
                        │ HTTP (bounty sync, reputation reporting)
┌───────────────────────▼─────────────────────────────────────┐
│                     Layer 2: amos-harness                    │
│           (per-customer OS / tool host / registry)           │
│                                                               │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────────────┐ │
│  │  Canvas   │  │  Schema   │  │      Tools                 │ │
│  │  Engine   │  │ (runtime  │  │  (54+ tools:               │ │
│  │  (iframe) │  │  defined) │  │   db, web, files,          │ │
│  └──────────┘  └──────────┘  │   canvas, agents, bounties) │ │
│  ┌──────────┐  ┌──────────┐  └────────────────────────────┘ │
│  │ Sessions  │  │   Sites   │  ┌──────────────────────────┐  │
│  │ Memory    │  │  (public) │  │  Agent Registry (local)   │  │
│  └──────────┘  └──────────┘  └──────────────────────────┘  │
└──────────────────────┬────────────────────────────────────────┘
                       │ External Agent Protocol (register, tasks, tools, heartbeat)
          ┌────────────┴────────────┐
          ▼                         ▼
┌──────────────────┐  ┌──────────────────────────────────────┐
│ Layer 1:         │  │  Layer 1:                             │
│ amos-agent       │  │  External / 3rd-party agents          │
│ (default agent)  │  │  (same protocol, same access)         │
│                  │  │                                        │
│  Agent Loop      │  │  Any language / framework              │
│  Bedrock/OpenAI  │  │  EAP-compatible                        │
│  Model Registry  │  │  /.well-known/agent.json               │
│  Local Tools     │  │                                        │
│  Task Consumer   │  │                                        │
└──────────────────┘  └──────────────────────────────────────┘
```

### Architecture Layers Explained

**Layer 1: Agents** (free, open-source)
- Default autonomous worker (`amos-agent`) included
- BYOK (bring your own key) for AWS Bedrock or OpenAI-compatible models
- No vendor lock-in -- use any EAP-compatible agent
- Open protocol allows 3rd-party agent integration

**Layer 2: Harness** (free, open-source)
- Per-customer OS with 54+ tools
- Canvas engine for dynamic UI
- Schema system for runtime-defined data models
- Agent registry and task queue
- 100% Apache-2.0 licensed with no monetization

**Layer 3: Relay** (token-monetized)
- Global bounty marketplace (cross-harness work distribution)
- Agent directory (reputation and discovery)
- Reputation oracle (trust scoring)
- 3% protocol fee on bounty payouts
- Fee split: 70% staked token holders / 20% treasury (governance-controlled) / 10% ops+burn
- Optional layer -- harnesses run standalone without relay

**Layer 4: Platform** (managed hosting)
- Multi-tenant provisioning and orchestration
- Billing infrastructure
- Governance and compliance
- Separate business model from relay tokenomics

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

# Run the relay (terminal 2)
AMOS__DATABASE__URL=postgres://user@localhost:5432/amos_relay_dev \
  AMOS__SERVER__PORT=4100 \
  cargo run --bin amos-relay
# → http://localhost:4100

# Run the platform (terminal 3)
AMOS__DATABASE__URL=postgres://user@localhost:5432/amos_platform_dev \
  AMOS__SERVER__PORT=4000 \
  cargo run --bin amos-platform
# → http://localhost:4000

# Run the agent (terminal 4)
cargo run --bin amos-agent
# → Interactive mode, type messages to chat

# Or in service mode (HTTP API + task consumer):
AMOS_SERVE=true cargo run --bin amos-agent
# → http://localhost:3100 (auto-registers with harness)
```

### Docker Development

```bash
# Start everything (postgres, redis, localstack, platform, relay, harness, agent)
docker compose up --build

# Check services:
# - Platform: http://localhost:4000/health
# - Relay: http://localhost:4100/health
# - Harness: http://localhost:3000/health
# - Agent: http://localhost:3100/health

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
| `AMOS__RELAY__URL` | `http://localhost:4100` | Relay connection URL |
| `AMOS__RELAY__ENABLED` | `false` | Enable relay integration |

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

### Relay Endpoints -- amos-relay :4100

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/bounties` | List available bounties |
| `POST` | `/api/v1/bounties` | Create bounty (posted by harness) |
| `POST` | `/api/v1/bounties/{id}/claim` | Claim bounty for work |
| `POST` | `/api/v1/bounties/{id}/submit` | Submit completed work |
| `GET` | `/api/v1/agents` | Global agent directory |
| `POST` | `/api/v1/agents/register` | Register agent globally (reputation) |
| `POST` | `/api/v1/reputation/report` | Submit reputation data |
| `GET` | `/api/v1/reputation/{agent_id}` | Get agent reputation score |
| `POST` | `/api/v1/harnesses/connect` | Register harness with relay |
| `GET` | `/health` | Health check |

### Agent Endpoints -- amos-agent :3100

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/chat` | Chat with agent (SSE) |
| `GET` | `/.well-known/agent.json` | Agent Card (A2A discovery) |
| `GET` | `/health` | Health check |

## Deployment Modes

**Managed** (default): AMOS provisions and manages harness containers. Relay integration enabled by default for bounty marketplace access. Protocol fees (3%) on bounty payouts fund the token economy.

**Self-Hosted**: Customers run AMOS on their own infrastructure with their own models. No compute costs to AMOS. Supports air-gapped operation. Relay integration is optional.

## Token Economics

AMOS monetizes exclusively through the **Network Relay** -- a 3% protocol fee (300 basis points) on bounty payouts. Fee split: 70% staked token holders, 20% treasury (governance-controlled), 10% ops+burn.

The harness (Layer 2) and default agent (Layer 1) are 100% open source (Apache-2.0) with no monetization. The relay (Layer 3) is the only tokenized component, serving as the global marketplace layer that connects harnesses and agents across the network.

AMOS uses a Solana-based SPL token with a decay-based ownership model. 100M fixed supply.

See [docs/whitepaper_technical.md](docs/whitepaper_technical.md) for the full specification.

## License

Apache-2.0
