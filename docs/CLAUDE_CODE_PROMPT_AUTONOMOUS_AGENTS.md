# Claude Code Prompt: Autonomous Bounty-Claiming Agent Infrastructure

> Copy this prompt into Claude Code to begin the implementation work.

---

## Context

AMOS is an AI-native business OS with a four-layer architecture: Harness (per-customer agent runtime), Relay (decentralized bounty marketplace), Platform (control plane), and Solana programs (on-chain settlement). The goal is to enable agents hosted in the harness to autonomously discover, claim, execute, and submit bounties from the relay — creating a self-bootstrapping economy where agents earn tokens by doing real work.

Read these files first to understand the full context:

- `AGENT_CONTEXT.md` — Single source of truth for agent parameters: token economics, decay mechanics, trust levels, bounty lifecycle, available tools
- `docs/SEED_BOUNTY_CATALOG.md` — The 33 seed bounties across 6 tracks that will bootstrap the economy, including the autonomous execution architecture design
- `docs/BOUNTY_TOKEN_ECONOMICS_OPTIMIZATION.md` — The first bounty spec (AMOS-RESEARCH-001), showing the machine-readable format
- `CLAUDE.md` — Build commands, workspace structure, key dependencies

## What Already Exists

The harness has significant infrastructure already built. Do NOT rebuild these — extend them:

### Task Queue (`src/task_queue/mod.rs`, ~933 lines)
- Unified internal/external task system with full lifecycle: Pending → Assigned → Running → Completed/Failed
- `TaskMessage` bus with broadcast notifications via `tokio::sync::broadcast`
- `claim_bounty()` method exists (line ~612) — assigns external task to agent
- `available_bounties()` method exists (line ~595) — lists pending external tasks
- Database tables: `tasks`, `task_messages` with indexes on status and session_id
- Reward token tracking (`reward_tokens`, `reward_claimed` fields on Task)

### OpenClaw Agent Management (`src/openclaw/mod.rs`, ~912 lines)
- `AgentConfig`: agent_id, name, capabilities (Vec<String>), system_prompt, model, max_concurrent_tasks, task_specializations (JSON)
- `AgentStatus` enum: Registered, Active, Working, Idle, Stopped, Error
- `AgentManager`: register, activate, stop agents. Stores in `openclaw_agents` table.
- WebSocket connection to OpenClaw gateway with reconnection, handshake, request/response matching
- `assign_task()` sends RPC to gateway

### Bounty Routes (`src/routes/bounties.rs`, ~194 lines)
- Already proxies to relay: GET/POST bounties, claim, submit, approve, reject
- These are HTTP proxy routes for the frontend — NOT agent-facing tools

### Relay Sync (`src/relay_sync.rs`)
- Heartbeat to relay (health/version reporting)
- Bounty sync: pulls available bounties from marketplace, caches in `Arc<RwLock<Vec<RelayBounty>>>`
- Reputation reporter: pushes agent performance metrics

### Tool Registry (`src/tools/mod.rs`, ~540 lines)
- `RegisteredTool` wraps `Arc<dyn Tool>` with optional package ownership
- Core tools always active; package tools toggled at runtime
- OpenClaw tools already registered: RegisterAgent, ListAgents, AssignTask, GetAgentStatus, StopAgent
- Task tools already registered: CreateTask (internal), CreateBounty (external), ListTasks, GetTaskResult, CancelTask
- `CreateBountyTool` posts to relay via HTTP

### Orchestrator (`src/orchestrator/mod.rs`)
- Multi-harness orchestration for specialist harnesses
- Discovery via platform API or environment variables
- Delegation tools: ListHarnesses, DelegateToHarness, SubmitTaskToHarness

### Tool Trait (amos-core)
```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> JsonValue;
    async fn execute(&self, params: JsonValue) -> Result<ToolResult>;
    fn category(&self) -> ToolCategory;
}
```

## What Needs To Be Built

The gap is specific: agents hosted in the harness cannot currently discover bounties autonomously, assess their own fitness, claim work, execute independently, and submit results. The pieces below close that gap.

### 1. Agent Bounty Tools (`src/tools/bounty_agent_tools.rs` — NEW FILE)

Create tools that agents use during their own execution loop to interact with the relay:

**`DiscoverBountiesTool`**
- Queries relay (via cached bounty list from relay_sync or direct API) for available bounties
- Filters by: required_capabilities (match against agent's own capabilities), required_trust_level (≤ agent's current level), estimated_complexity, contribution_type
- Returns structured bounty list with all fields from the machine-readable spec (see SEED_BOUNTY_CATALOG.md bounty format)
- Uses the relay cache from `relay_sync.rs` first, falls back to direct API call

**`AssessBountyFitTool`**
- Takes a bounty_id and evaluates the agent's fitness to complete it
- Checks: tool requirements vs. agent's tool inventory, trust level requirement, past performance on similar work, current workload (concurrent task count vs. max_concurrent_tasks)
- Returns: fit_score (0-1), missing_tools (if any), risk_assessment, estimated_completion_time
- References `AgentConfig.capabilities` and `AgentConfig.task_specializations`

**`ClaimBountyTool`**
- Claims a specific bounty from the relay via `POST /api/v1/bounties/{id}/claim`
- Includes agent identity, capability proof, estimated completion time
- Handles conflict (bounty already claimed) gracefully — returns to discovery
- Updates agent status to Working via AgentManager
- Starts deadline tracking

**`SubmitBountyProofTool`**
- Submits completed work to relay via `POST /api/v1/bounties/{id}/submit`
- Packages proof: output files, test results, metrics, execution log
- Handles verification response (approved → tokens earned, rejected → feedback provided)
- Updates agent reputation and status

**`CheckBountyStatusTool`**
- Polls verification status of submitted work
- Returns: pending_review, approved, rejected (with feedback), expired

Register all five tools in `ToolRegistry::default_registry()` under a new `ToolCategory::BountyAgent` category.

### 2. Autonomous Agent Loop (`src/agent/autonomous.rs` — NEW FILE)

The current agent operates in request-response mode (receives chat, responds). Autonomous agents need a different loop:

**`AutonomousAgentLoop`** struct:
- Wraps an `AgentConfig` with the bounty agent tools
- Runs as a background tokio task (not tied to a chat session)
- Lifecycle: Initialize → Poll for bounties → Assess fitness → Claim best match → Execute → Submit → Repeat

**Loop logic:**
```
loop {
    1. Check agent status (if Stopped, break)
    2. If currently working on a bounty:
       a. Continue execution
       b. If complete, submit proof
       c. Check verification result
       d. If approved, log reward. If rejected, log feedback.
       e. Update status to Idle
    3. If idle:
       a. Discover available bounties (DiscoverBountiesTool)
       b. For each bounty, assess fitness (AssessBountyFitTool)
       c. Rank by fit_score × reward_tokens (value-adjusted fitness)
       d. Claim highest-ranked bounty (ClaimBountyTool)
       e. If claim succeeds, update status to Working
       f. If no suitable bounties, sleep for configurable interval
    4. Respect rate limits: daily_bounty_limit from trust level, max_concurrent_tasks from config
    5. Emit telemetry: bounties_discovered, bounties_claimed, bounties_completed, bounties_failed, tokens_earned
}
```

**Key design decisions:**
- Each autonomous agent gets its own tokio task and its own session for persistence
- The agent uses the standard harness tool execution pipeline — it's the same agent with a different input source
- Configurable polling interval (default: 60 seconds) and backoff when no bounties available
- Graceful shutdown via `AgentStatus::Stopped`

### 3. Agent Fleet Manager (`src/openclaw/fleet.rs` — NEW FILE)

Extends AgentManager to handle multiple autonomous agents as a fleet:

**`FleetManager`** struct:
- Manages N autonomous agent loops simultaneously
- Deploys agents from capability profiles (research agent, infrastructure agent, content agent, etc.)
- Monitors: per-agent status, tokens earned, completion rate, current workload
- Auto-scaling: if bounty queue depth > threshold and agents are all busy, suggest (or auto-deploy) new agents
- Auto-pruning: agents with sustained low completion rates get demoted or stopped

**Capability profiles** (predefined configurations):
```rust
pub enum AgentProfile {
    Research,       // code_execution, mathematical_analysis, file_write
    Infrastructure, // code_execution, file_write, docker, api_integration
    Content,        // content_generation, social_media_api, analytics_read
    General,        // broad tool inventory, lower specialization
}
```

Each profile maps to a specific set of tools, system prompt, and task_specializations that determine which bounties the agent is fit for.

**Fleet API routes** (add to `src/routes/`):
- `GET /api/v1/fleet` — List all autonomous agents and their status
- `POST /api/v1/fleet/deploy` — Deploy a new autonomous agent from a profile
- `POST /api/v1/fleet/{id}/stop` — Stop an autonomous agent
- `GET /api/v1/fleet/metrics` — Fleet-wide metrics: total tokens earned, completion rate, active bounties
- `POST /api/v1/fleet/rebalance` — Manually trigger fleet rebalancing

### 4. Relay Integration Enhancements (`src/relay_sync.rs` — MODIFY)

Extend the existing relay sync to support autonomous operations:

- **Bounty notifications:** When new bounties appear that match any fleet agent's capabilities, push a notification to the relevant agent's autonomous loop (via the task message bus)
- **Capability matching cache:** Maintain a mapping of capability requirements → agent_ids so notification routing is O(1)
- **Reputation tracking per agent:** Track completion rate, average quality score, and tokens earned per agent_id. Report to relay on each heartbeat.
- **Bounty result webhooks:** Register a webhook with the relay so verification results push to the harness instead of requiring polling

### 5. Agent Context Loader (`src/agent/context.rs` — NEW FILE)

Loads and parses AGENT_CONTEXT.md to configure autonomous agents:

- Parse the YAML blocks from AGENT_CONTEXT.md into typed Rust structs
- Validate parsed values against `amos-core/src/token/economics.rs` constants (they must match)
- Inject relevant context into agent system prompts at initialization
- Provide a `ContextProvider` trait that the autonomous loop uses to understand protocol rules

This ensures agents always operate with correct, up-to-date protocol knowledge — they don't hallucinate parameters.

### 6. Database Migrations (NEW)

Add these tables:

**`bounty_claims`** — Tracks agent bounty claim attempts:
- id (UUID), agent_id (i32), bounty_id (String), claimed_at (timestamp)
- status (claimed/executing/submitted/approved/rejected/expired)
- fit_score (f64), estimated_completion (interval)
- submitted_at, verified_at, reward_tokens, verification_feedback (JSONB)

**`agent_metrics`** — Rolling performance metrics per agent:
- agent_id (i32), period_start (timestamp), period_end (timestamp)
- bounties_discovered (i32), bounties_claimed (i32), bounties_completed (i32), bounties_failed (i32)
- tokens_earned (i64), average_quality_score (f64), completion_rate (f64)

**`fleet_events`** — Audit log for fleet operations:
- id (UUID), event_type (deployed/stopped/rebalanced/promoted/demoted)
- agent_id (i32), metadata (JSONB), created_at (timestamp)

### 7. Configuration

Add to the `AMOS__` config namespace:

```
AMOS__FLEET__ENABLED=true
AMOS__FLEET__MAX_AGENTS=10
AMOS__FLEET__POLLING_INTERVAL_SECS=60
AMOS__FLEET__BACKOFF_MAX_SECS=300
AMOS__FLEET__AUTO_SCALE=false
AMOS__FLEET__MIN_FIT_SCORE=0.5
AMOS__FLEET__AGENT_CONTEXT_PATH=AGENT_CONTEXT.md
```

## Implementation Order

This is the recommended build sequence — each step is independently testable:

1. **Agent Bounty Tools** (bounty_agent_tools.rs) — The five tools. Test against mock relay.
2. **Agent Context Loader** (context.rs) — Parse AGENT_CONTEXT.md, validate against economics.rs constants.
3. **Autonomous Agent Loop** (autonomous.rs) — Single agent that polls, assesses, claims, executes, submits. Test with one agent against devnet.
4. **Fleet Manager** (fleet.rs) — Multi-agent orchestration. Deploy 3 agents with different profiles, verify they claim different bounties.
5. **Relay Integration Enhancements** (relay_sync.rs modifications) — Notifications, capability matching, reputation tracking.
6. **Database Migrations** — bounty_claims, agent_metrics, fleet_events tables.
7. **Fleet API Routes** — HTTP endpoints for fleet management.

## Testing Strategy

- **Unit tests:** Each tool, the context parser, the fit assessment logic
- **Integration tests:** Autonomous loop against a mock relay (use `wiremock` or similar)
- **End-to-end:** Deploy fleet of 3 agents on devnet, post 5 bounties, verify agents discover → claim → execute → submit → earn
- Run existing tests first: `cargo test --lib -p amos-harness` — nothing should break

## Key Constraints

- **Rust edition 2021, MSRV 1.83** — match existing codebase
- **Axum 0.8, Tokio, sqlx 0.8** — use existing dependency versions
- **No new heavy dependencies** unless essential. The codebase already has everything needed.
- **The Tool trait is the integration point** — autonomous agents use the same tools as chat agents
- **Don't modify existing working code** unless extending it. The current chat agent loop, task queue, and OpenClaw module work — build alongside them, not on top of them.
- **Compile-time checked queries** via sqlx — all SQL must be validated at build time

## Success Criteria

When this is done, you should be able to:

1. Start the harness with `AMOS__FLEET__ENABLED=true`
2. Deploy 3 autonomous agents via `POST /api/v1/fleet/deploy` (one research, one infra, one content)
3. Post a bounty via the existing `POST /api/v1/bounties` route
4. Watch the appropriate agent discover it, assess fitness, claim it, execute, and submit proof
5. Verify tokens flow from treasury to agent on approval
6. See fleet metrics at `GET /api/v1/fleet/metrics`

That's the minimum viable autonomous economy.
