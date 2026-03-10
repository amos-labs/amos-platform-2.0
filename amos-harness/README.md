# amos-harness

The per-customer AI runtime. This is where the core AMOS product lives.

## Binary

```bash
AMOS__DATABASE__URL=postgres://user@localhost:5432/amos_dev cargo run --bin amos-harness
# → http://localhost:3000
```

## Architecture

```
src/
├── agent/              # AI agent loop (the brain)
│   ├── loop_runner.rs  # Event-driven think-act-observe cycle
│   ├── bedrock.rs      # AWS Bedrock ConverseStream client
│   ├── provider.rs     # Model provider abstraction
│   ├── prompt_builder.rs # System prompt construction
│   └── model_registry.rs # Dynamic model registry (Bedrock + custom)
├── tools/              # Tool system (30+ tools)
│   ├── mod.rs          # Tool trait + ToolRegistry
│   ├── platform_tools.rs # Database CRUD
│   ├── canvas_tools.rs # Canvas create/update/publish
│   ├── web_tools.rs    # Web search, page scraping
│   ├── system_tools.rs # File read, bash execution
│   ├── memory_tools.rs # Remember/recall with salience
│   ├── openclaw_tools.rs # Agent management
│   ├── schema_tools.rs # Dynamic collections/records
│   └── site_tools.rs   # Public website generation
├── canvas/             # Canvas engine (dynamic UI in iframes)
├── routes/             # HTTP route handlers
├── openclaw/           # Autonomous agent management
├── schema/             # Runtime-defined collections + records
├── sites/              # Public websites served at /s/{slug}
├── memory/             # Working memory with semantic search
├── documents/          # Document processing (PDF, DOCX)
├── integrations/       # Third-party service connections
├── sessions/           # Conversation history
├── revisions/          # Content revision tracking
├── task_queue/         # Background task processing
├── platform_sync.rs    # Heartbeat + config sync with platform
├── server.rs           # Axum server setup + route registration
├── state.rs            # Shared application state
└── main.rs             # Entry point
```

## Key Concepts

**Agent Loop**: Send conversation + tool schemas to Bedrock (Claude), receive response, execute tool calls, feed results back. Supports model escalation and SSE streaming.

**Tool System**: Tools implement the `Tool` trait. They're registered in `ToolRegistry` and their JSON schemas are sent to the LLM. To add a tool: create struct, implement trait, register it.

**Canvas Engine**: The agent creates HTML/CSS/JS canvases rendered in sandboxed iframes. All canvas data is DB-backed.

**Schema System**: Runtime-defined collections and records using JSONB. Validated against JSON Schema. Customers define data structures through conversation.

## Static Assets

The chat UI lives in `static/`. Plain JS + Tailwind CSS + Lucide icons. No build step.
