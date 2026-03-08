# AMOS

**Autonomous Management Operating System** -- an AI-native business operating system written in pure Rust.

AMOS is a per-customer AI harness that serves as a single conversational + canvas interface for managing an entire business. The AI agent builds workflows, automations, integrations, websites, and applications on the fly through natural language.

## Architecture

```
amos-automate/
├── amos-core          # Shared types, config, errors, token economics
├── amos-harness       # The main runtime: agent loop, tools, UI, canvas engine
├── amos-platform      # Multi-tenant platform layer (harness orchestration)
├── amos-cli           # Command-line interface
└── amos-solana/       # On-chain programs (treasury, bounties, governance)
```

### amos-core

The foundation crate. Every other crate depends on it. Contains:

- **AppConfig** -- Hierarchical configuration (env vars, files, defaults) using `AMOS__` prefix with `__` as nested separator
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
  - `openclaw_tools` -- Register, manage, and assign tasks to autonomous AI agents
  - `orchestration_tools` -- Delegate work to external AI agents
  - `schema_tools` -- Define collections, CRUD records (dynamic data layer)
  - `site_tools` -- Create multi-page websites and landing pages
- **OpenClaw** (`src/openclaw/`) -- Autonomous AI agent management and orchestration via WebSocket gateway
- **Orchestration** (`src/orchestration/`) -- External Agent Protocol (EAP) for registering, delegating to, and auditing third-party AI agents with trust levels
- **Schema** (`src/schema/`) -- Runtime-defined collections and records (JSONB-backed, validated, queryable). No migrations needed per customer request
- **Sites** (`src/sites/`) -- Multi-page public websites served at `/s/{slug}` with form submission into schema collections
- **Memory** (`src/memory/`) -- Working memory with semantic search and salience scoring

### UI

The harness serves a single-page application from `amos-harness/static/`. The interface has three modes:

1. **Full chat** -- Sidebar + full-width conversation
2. **Chat + Canvas** -- Sidebar collapses, chat shrinks to 1/3, canvas fills 2/3
3. **Navigation views** -- Canvases, Agents, Integrations, Settings

No JavaScript framework. Plain JS + Tailwind CSS + Lucide icons.

## Prerequisites

- **Rust** >= 1.80
- **PostgreSQL** >= 15
- **Redis**
- **AWS credentials** configured for Bedrock (Claude model access)

## Quick Start

```bash
# Clone the repository
git clone https://github.com/amos-labs/amos-platform.git
cd amos-platform

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

The harness starts on `http://localhost:3000`.

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

## Database

The harness uses PostgreSQL with sqlx migrations. Migrations run automatically on startup. The schema includes:

- `sessions` / `messages` -- Conversation history
- `canvases` -- Dynamic UI canvases (HTML/CSS/JS + data bindings)
- `openclaw_agents` -- OpenClaw agent configurations and state
- `external_agents` -- EAP agent registry with trust levels
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

### Agents (OpenClaw)

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

### Health

```
GET /health    # Liveness check
GET /ready     # Readiness check (includes DB connectivity)
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
- External agent orchestration (EAP)
- Memory system

## License

Apache-2.0
