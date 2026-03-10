# amos-agent

Standalone autonomous agent for the AMOS ecosystem. Connects to the harness using the same protocol as any external agent ("One Protocol" principle).

## Binary

```bash
# With AWS Bedrock (default)
AMOS_HARNESS_URL=http://localhost:3000 cargo run --bin amos-agent

# With OpenAI-compatible API
AMOS_MODEL_PROVIDER=openai \
  AMOS_API_BASE=http://localhost:8000/v1 \
  AMOS_API_KEY=your-key \
  AMOS_MODEL_ID=Qwen/Qwen3-Next-80B \
  cargo run --bin amos-agent
```

## Architecture

```
src/
├── agent_loop.rs       # Think-act-observe cycle
├── config.rs           # CLI args + configuration (clap)
├── provider.rs         # ModelProvider trait (Bedrock + OpenAI)
├── harness_client/     # HTTP client for harness registration + tool execution
├── agent_card/         # A2A Agent Card server (/.well-known/agent.json)
├── memory/             # SQLite-backed persistent memory (remember/recall)
├── tools/              # Local tools
│   ├── think.rs        # Internal reasoning (no side effects)
│   ├── memory_tools.rs # Remember + recall
│   ├── plan.rs         # Task planning
│   ├── web_search.rs   # Brave Search API
│   └── file_tools.rs   # File read/write (sandboxed to work_dir)
└── main.rs             # Entry point (interactive REPL)
```

## Key Concepts

**One Protocol**: This agent connects to the harness via HTTP, registers itself, discovers harness tools, and executes them remotely. There is no privileged internal API.

**Local + Remote Tools**: Local tools (think, remember, plan, web_search, file I/O) run on the agent's machine. Harness tools (prefixed `harness_`) are executed via HTTP on the harness server.

**Agent Card**: Serves an A2A-compatible agent card at `/.well-known/agent.json` for discovery.

**Memory**: SQLite-backed persistent memory with keyword search. Survives restarts.

## Configuration

| Env Var | Default | Description |
|---------|---------|-------------|
| `AMOS_HARNESS_URL` | `http://localhost:3000` | Harness to connect to |
| `AMOS_AGENT_NAME` | `amos-agent` | Registration name |
| `AMOS_AGENT_PORT` | `3100` | Agent Card server port |
| `AMOS_MODEL_PROVIDER` | `bedrock` | `bedrock` or `openai` |
| `AMOS_MODEL_ID` | `anthropic.claude-sonnet-4-20250514-v1:0` | Model to use |
| `AMOS_API_BASE` | -- | OpenAI-compatible API base URL |
| `AMOS_API_KEY` | -- | API key for model provider |
| `AMOS_MEMORY_DB` | `amos_agent_memory.db` | SQLite memory path |
| `BRAVE_API_KEY` | -- | Brave Search API key |
| `AMOS_MAX_ITERATIONS` | `25` | Max agent loop iterations |
| `AMOS_WORK_DIR` | `.` | Working directory for file tools |
