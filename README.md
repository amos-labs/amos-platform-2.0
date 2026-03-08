# AMOS

**Autonomous Management Operating System** -- an AI-native business operating system written in pure Rust.

AMOS is a per-customer AI harness that serves as a single conversational + canvas interface for managing an entire business. The AI agent builds workflows, automations, integrations, websites, and applications on the fly through natural language.

## Architecture

```
amos-automate/
├── amos-core          # Shared types, config, errors, token economics
├── amos-harness       # The main runtime: agent loop, tools, UI, canvas engine
├── amos-platform      # Multi-tenant platform layer (harness orchestration, provisioning)
├── amos-cli           # Command-line interface
├── amos-solana/       # On-chain programs (treasury, bounties, governance)
└── docs/              # Whitepaper, token economics documentation
```

### amos-core

The foundation crate. Every other crate depends on it. Contains:

- **AppConfig** -- Hierarchical configuration (env vars, files, defaults) using `AMOS__` prefix with `__` as nested separator
- **DeploymentMode** -- Managed (AMOS cloud) vs self-hosted (customer hardware) deployment configuration
- **CustomModelsConfig** -- Sovereign AI support: customer-provisioned OpenAI-compatible model endpoints
- **PlatformConfig** -- Harness-to-platform sync settings (heartbeat, config pull, activity reporting)
- **AmosError / Result** -- Unified error types across the workspace
- **Token Economics** -- AMOS token distribution, vesting, and reward calculations
- **Domain Types** -- Shared types used across crates

### amos-harness

The per-customer runtime. This is where the core product lives:

- **Agent Loop** (`src/agent/`) -- Event-driven V3 agent loop with model escalation, streaming responses via SSE, and iterative tool execution against AWS Bedrock (Claude)
- **Canvas Engine** (`src/canvas/`) -- Dynamic UI generation. The agent creates interactive dashboards, forms, and visualizations rendered in sandboxed iframes
- **Tool System** (`src/tools/`) -- 30+ tools the agent can invoke:
  - `platform_tools` -- Query, create, update, execute against the platform database
  - `canvas_tools` -- Create, update, publish canvases
  - `web_tools` -- Web search, page scraping
  - `system_tools` -- File read, bash execution
  - `memory_tools` -- Remember and recall with salience-based attention
  - `openclaw_tools` -- Register, manage, activate, and assign tasks to autonomous AI agents
  - `schema_tools` -- Define collections, CRUD records (dynamic data layer)
  - `site_tools` -- Create multi-page websites and landing pages
- **OpenClaw Agent Management** (`src/openclaw/`) -- Unified autonomous AI agent management. Agents register with AMOS, are managed via a single control plane, and can be activated, stopped, and assigned tasks. Communicates via WebSocket gateway protocol
- **Schema** (`src/schema/`) -- Runtime-defined collections and records (JSONB-backed, validated, queryable). No migrations needed per customer request
- **Sites** (`src/sites/`) -- Multi-page public websites served at `/s/{slug}` with form submission into schema collections
- **Memory** (`src/memory/`) -- Working memory with semantic search and salience scoring
- **Platform Sync** (`src/platform_sync.rs`) -- Background client that syncs with the AMOS platform: heartbeat, config pull, and usage reporting. Works for both managed and self-hosted deployments
- **Model Registry** (`src/agent/model_registry.rs`) -- Dynamic model registry supporting AWS Bedrock (Anthropic Claude) and customer-provisioned OpenAI-compatible models (Qwen, etc.)

### amos-platform

The multi-tenant control plane. Manages harness lifecycle and billing:

- **Provisioning** (`src/provisioning/`) -- Docker-based harness provisioning. Creates, starts, stops, and deprovisions per-customer harness containers via the Docker API (bollard)
- **Solana Integration** (`src/solana/`) -- Optional on-chain token operations, treasury management
- **Sync API** (`src/routes/sync.rs`) -- Harness sync endpoints: heartbeat, config distribution, activity ingest, version checks
- **Billing** (`src/billing/`) -- Subscription plans, compute cost tracking with 20% markup (waived for sovereign AI), AMOS token discount
- **Routes** -- REST API for harness lifecycle management, health checks, token economics, governance

### UI

The harness serves a single-page application from `amos-harness/static/`. The interface has three modes:

1. **Full chat** -- Sidebar + full-width conversation
2. **Chat + Canvas** -- Sidebar collapses, chat shrinks to 1/3, canvas fills 2/3
3. **Navigation views** -- Canvases, Agents, Integrations, Settings

No JavaScript framework. Plain JS + Tailwind CSS + Lucide icons.

## Token Economics

AMOS uses a Solana-based SPL token with a decay-based ownership model. Contributors earn tokens through work (code, sales, content), and token holders receive 50% of platform revenue.

Key properties:
- **Fixed supply**: 100M tokens, 9 decimals
- **Revenue share**: 50% to holders, 40% R&D, 5% treasury, 5% operations
- **Dynamic decay**: 2-25% annual, tied to platform profitability (more profitable = less decay)
- **Wealth preservation**: 12-month grace period, graduated floors, staking vaults

Full documentation:
- [Technical Whitepaper](docs/whitepaper_technical.md) -- Complete technical specification
- [Simple Whitepaper](docs/whitepaper_simple.md) -- Non-technical overview
- [Token Economy Math](docs/token_economy_math.md) -- Mathematical framework and formulas
- [Equation Cheat Sheet](docs/token_economy_equations.md) -- Quick reference for all equations

## Deployment Modes

AMOS supports two deployment modes:

### Managed (Default)

AMOS provisions and manages harness containers via Docker API. Customers connect to their harness through the AMOS platform.

- Compute costs include a 20% markup
- Updates pushed automatically
- Monitoring and billing handled by platform

### Self-Hosted (Sovereign AI)

Customers run AMOS on their own infrastructure with their own AI models. Ideal for organizations requiring data sovereignty, air-gapped environments, or custom model deployment.

```bash
# Self-hosted configuration (in .env or config file)
AMOS__DEPLOYMENT__MODE=self_hosted
AMOS__DEPLOYMENT__LICENSE_KEY=your-license-key
AMOS__DEPLOYMENT__AUTO_UPDATE=true

# Platform sync (optional, can be disabled for air-gapped)
AMOS__PLATFORM__URL=https://api.amos.ai
AMOS__PLATFORM__API_KEY=your-platform-api-key
AMOS__PLATFORM__TELEMETRY_ENABLED=true

# Custom model (OpenAI-compatible endpoint)
AMOS__CUSTOM_MODELS__ENABLED=true
```

For custom models, configure via `config/default.toml`:

```toml
[custom_models]
enabled = true

[[custom_models.providers]]
name = "qwen-local"
display_name = "Qwen3-Next 80B (Self-Hosted)"
api_base = "http://gpu-server:8000/v1"
model_id = "Qwen/Qwen3-Next-80B"
context_window = 131072
tier = 2
customer_owned = true
```

Key differences from managed:
- **No compute markup** on customer-owned models
- **Pull-based updates** instead of push (harness checks platform for new versions)
- **License validation** via platform API
- **Optional telemetry** (can be fully air-gapped)
- **Custom Qwen models** via vLLM, TGI, or Ollama (OpenAI-compatible API)

## Prerequisites

- **Rust** >= 1.83
- **PostgreSQL** >= 15 (with pgvector extension recommended)
- **Redis**
- **Docker** (for platform provisioning and dev environment)
- **AWS credentials** configured for Bedrock (Claude model access)

## Quick Start

### Local Development (without Docker)

```bash
# Clone the repository
git clone https://github.com/amos-labs/amos-platform-2.0.git
cd amos-platform-2.0

# Create and configure environment
cp .env.example .env
# Edit .env with your database URL, Redis URL, etc.

# Create the database
createdb amos_harness_development

# Build the workspace
cargo build

# Run the harness (migrations run automatically on startup)
cargo run --bin amos-harness
```

### Docker Development

```bash
# Start infrastructure (PostgreSQL + Redis)
docker compose up postgres redis -d

# Build and start everything
docker compose up --build

# Or build images separately
docker compose build harness
docker compose build platform
```

The harness starts on `http://localhost:3000`. The platform starts on `http://localhost:4000`.

## Configuration

Configuration uses the `AMOS__` prefix with `__` as the nested separator. Set via environment variables or `.env` file:

| Variable | Default | Description |
|----------|---------|-------------|
| `AMOS__DATABASE__URL` | -- | PostgreSQL connection string |
| `AMOS__REDIS__URL` | `redis://127.0.0.1:6379` | Redis connection string |
| `AMOS__SERVER__HOST` | `0.0.0.0` | Bind address |
| `AMOS__SERVER__PORT` | `3000` | Bind port |
| `AMOS__AGENT__MAX_ITERATIONS` | `25` | Max agent loop iterations per request |
| `AMOS__AGENT__MAX_CONTEXT_TOKENS` | `200000` | Max context window tokens |
| `AWS_PROFILE` | `default` | AWS profile for Bedrock access |
| `AMOS__DEPLOYMENT__MODE` | `managed` | Deployment mode: `managed` or `self_hosted` |
| `AMOS__DEPLOYMENT__LICENSE_KEY` | -- | License key for self-hosted deployments |
| `AMOS__PLATFORM__URL` | `http://localhost:4000` | Platform API URL for sync |
| `AMOS__PLATFORM__API_KEY` | -- | API key for platform authentication |
| `AMOS__PLATFORM__HEARTBEAT_INTERVAL_SECS` | `30` | Heartbeat frequency (seconds) |
| `AMOS__PLATFORM__TELEMETRY_ENABLED` | `true` | Enable usage reporting to platform |
| `AMOS__CUSTOM_MODELS__ENABLED` | `false` | Enable custom model providers |

## Database

The harness uses PostgreSQL with sqlx migrations. Migrations run automatically on startup. The schema includes:

- `sessions` / `messages` -- Conversation history
- `canvases` -- Dynamic UI canvases (HTML/CSS/JS + data bindings)
- `openclaw_agents` -- Agent configurations, status, and lifecycle state
- `memory_items` -- Working memory with salience scores
- `integrations` -- Third-party service connections
- `collections` / `records` -- Dynamic schema system (runtime-defined data)
- `sites` / `pages` -- Public websites and landing pages

## API

### Chat (SSE streaming)

```
POST /api/v1/agent/chat
Content-Type: application/json

{"message": "Create a dashboard showing monthly revenue", "session_id": null}
```

Returns a Server-Sent Events stream with typed events: `turn_start`, `message_start`, `message_delta`, `tool_start`, `tool_end`, `turn_end`, `agent_end`, `model_escalation`, `error`.

### Canvases

```
GET    /api/v1/canvases          # List canvases
GET    /api/v1/canvases/:id      # Get canvas
DELETE /api/v1/canvases/:id      # Delete canvas
GET    /c/:slug                  # Public canvas (published)
```

### Agents

```
GET    /api/v1/agents              # List agents
POST   /api/v1/agents              # Register agent
GET    /api/v1/agents/:id          # Get agent status
PUT    /api/v1/agents/:id          # Update agent
POST   /api/v1/agents/:id/activate # Activate agent
POST   /api/v1/agents/:id/stop     # Stop agent
```

### Sites

```
GET    /s/:slug                  # Serve site index page
GET    /s/:slug/*path            # Serve site sub-page
POST   /s/:slug/submit/:collection  # Form submission into collection
```

### Provisioning (Platform)

```
POST   /provision/harness              # Provision new harness container
GET    /provision/harness/:id          # Get harness status
POST   /provision/harness/:id/start    # Start harness
POST   /provision/harness/:id/stop     # Stop harness
DELETE /provision/harness/:id          # Deprovision harness
GET    /provision/harness/:id/logs     # Get harness logs
```

### Health

```
GET /health    # Liveness check
GET /ready     # Readiness check (includes DB connectivity)
```

### Sync (Platform ↔ Harness)

```
POST /api/v1/sync/heartbeat     # Harness heartbeat (version, health, uptime)
GET  /api/v1/sync/config        # Pull configuration updates
POST /api/v1/sync/activity      # Push usage/activity metrics
GET  /api/v1/sync/version       # Check latest available version
```

## Tool System

Tools are the interface between the AI agent and the world. Each tool implements the `Tool` trait:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> JsonValue;
    async fn execute(&self, params: JsonValue) -> Result<ToolResult>;
    fn category(&self) -> ToolCategory;
}
```

Tools are registered in `ToolRegistry` and their JSON schemas are sent to Bedrock's ConverseStream API so the model can invoke them during conversation.

To add a new tool:

1. Create a struct implementing `Tool` in `src/tools/`
2. Register it in `ToolRegistry::default_registry()`
3. The agent will automatically discover and use it

## Project Status

This is an early-stage project under active development. The core architecture is functional:

- Chat pipeline with streaming SSE responses
- Agent loop with tool execution and model escalation
- Canvas creation and rendering
- Dynamic schema (collections + records)
- Public site generation
- Autonomous AI agent management (OpenClaw)
- Memory system
- Docker-based harness provisioning
- Token economics with on-chain revenue distribution
- Self-hosted deployment mode with license validation
- Sovereign AI: customer-provisioned Qwen models via OpenAI-compatible API
- Platform sync: heartbeat, config distribution, activity reporting
- Billing with sovereign AI support (no markup on customer-owned models)

## License

Apache-2.0
