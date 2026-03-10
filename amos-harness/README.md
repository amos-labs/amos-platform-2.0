# amos-harness

The per-customer AI operating system. The harness hosts tools, canvases, schemas, sites, and data -- it never runs its own agent loop. Autonomous agents (including the bundled `amos-agent`) connect externally via the **External Agent Protocol** to register, pull tasks, call harness tools over HTTP, and report results.

## Binary

```bash
AMOS__DATABASE__URL=postgres://user@localhost:5432/amos_dev cargo run --bin amos-harness
# -> http://localhost:3000
```

## Architecture

```
src/
├── tools/                # Tool system (54+ tools exposed to agents via EAP)
│   ├── mod.rs            # Tool trait + ToolRegistry
│   ├── canvas_tools.rs   # Canvas create/update/publish
│   ├── credential_tools.rs # Credential vault operations
│   ├── document_tools.rs # Document processing (PDF, DOCX)
│   ├── image_gen_tools.rs # AI image generation
│   ├── integration_tools.rs # Third-party API integrations
│   ├── memory_tools.rs   # Remember/recall with semantic search
│   ├── openclaw_tools.rs # Agent management tools
│   ├── platform_tools.rs # Database CRUD
│   ├── revision_tools.rs # Content revision tracking
│   ├── schema_tools.rs   # Dynamic collections/records
│   ├── site_tools.rs     # Public website generation
│   ├── system_tools.rs   # File read, bash execution
│   ├── task_tools.rs     # Background task management
│   └── web_tools.rs      # Web search, page scraping
├── canvas/               # Canvas engine (dynamic UI in iframes)
│   ├── generator.rs      # AI-powered canvas generation (Bedrock)
│   ├── renderer.rs       # Canvas rendering
│   ├── templates.rs      # Built-in canvas templates
│   └── types.rs          # Canvas data types
├── routes/               # HTTP route handlers
│   ├── bots.rs           # Agent registration + management (EAP)
│   ├── canvas.rs         # Canvas CRUD + public serving
│   ├── credentials.rs    # Credential vault API
│   ├── health.rs         # Health + readiness checks
│   ├── integrations.rs   # Third-party service connections
│   ├── revisions.rs      # Content revision API
│   ├── sites.rs          # Site management + public serving
│   └── uploads.rs        # File upload handling
├── openclaw/             # Agent registry and lifecycle management
├── memory/               # Working memory with semantic search
├── documents/            # Document processing (PDF, DOCX extract/export)
├── integrations/         # ETL pipelines + API executor
├── task_queue/           # Background task processing + sub-agent dispatch
├── middleware/           # Auth middleware (JWT validation)
├── bedrock.rs            # AWS Bedrock client (canvas generation only)
├── geo.rs                # Geolocation services
├── image_gen.rs          # Image generation client
├── schema.rs             # Runtime-defined collections + JSON Schema validation
├── sessions.rs           # Conversation history
├── sites.rs              # Site data layer
├── revisions.rs          # Revision data layer
├── storage.rs            # S3-compatible object storage
├── platform_sync.rs      # Heartbeat + config sync with amos-platform
├── server.rs             # Axum server setup + route registration
├── state.rs              # Shared application state (AppState)
├── lib.rs                # Crate root (re-exports)
└── main.rs               # Entry point (DB migrations, server start)

static/                   # Chat UI (plain JS + Tailwind CSS + Lucide icons, no build step)
```

## Key Concepts

**External Agent Protocol (EAP)**: Agents connect to the harness over HTTP. They register at `/api/v1/agents/register`, poll for tasks at `/api/v1/tasks/next`, execute harness tools at `/api/v1/tools/{name}/execute`, and report results at `/api/v1/tasks/{id}/result`. The harness does not run its own agent loop -- all intelligence comes from external agents.

**Tool System**: Tools implement the `Tool` trait. They're registered in `ToolRegistry` and their JSON schemas are sent to agents. To add a tool: create struct, implement trait, register it in `mod.rs`.

**Canvas Engine**: Agents create HTML/CSS/JS canvases rendered in sandboxed iframes. Canvas data is DB-backed. The harness uses Bedrock directly (via `bedrock.rs`) only for AI-assisted canvas generation -- this is the sole use of an LLM inside the harness.

**Schema System**: Runtime-defined collections and records using JSONB. Validated against JSON Schema. Customers define data structures through conversation with an agent.

**Credential Vault**: AES-256-GCM encrypted storage for API keys and secrets. Agents and integrations resolve credentials securely without exposing plaintext.

**Agent Registry**: Tracks registered agents, their capabilities, and heartbeat status. Supports both the bundled `amos-agent` and any third-party agent that speaks EAP.

## HTTP Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/ready` | Readiness check |
| `GET` | `/login` | Login page (system canvas) |
| `GET` | `/register` | Registration page (system canvas) |
| `GET` | `/forgot-password` | Password reset page (system canvas) |
| `GET/POST` | `/api/v1/canvases` | Canvas CRUD |
| `GET` | `/c/{slug}` | Public canvas |
| `GET/POST` | `/api/v1/agents` | Agent registration + management |
| `POST` | `/api/v1/uploads` | File uploads (25 MB limit) |
| `GET/POST` | `/api/v1/integrations` | Integration management |
| `GET/POST` | `/api/v1/credentials` | Credential vault |
| `GET/POST` | `/api/v1/revisions` | Content revisions |
| `GET/POST` | `/api/v1/sites` | Site management |
| `GET` | `/s/{slug}` | Public site (index page) |
| `GET` | `/s/{slug}/{path}` | Public site (sub-pages) |
| `POST` | `/s/{slug}/submit/{collection}` | Public form submission |

## Static Assets

The chat UI lives in `static/`. Plain JS + Tailwind CSS + Lucide icons. No build step.
