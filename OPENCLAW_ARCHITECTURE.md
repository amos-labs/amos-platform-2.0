# OpenClaw Integration Architecture - AMOS Codebase

## Executive Summary

OpenClaw is AMOS's autonomous AI agent management system that enables external, self-directed AI agents to register with AMOS and operate as managed employees. It provides a unified control plane for agent lifecycle management, task assignment, and result reporting through a WebSocket-based gateway protocol.

The system supports two modes of work:
1. **Internal Tasks**: Background work handled by harness sub-agents
2. **External Bounties**: Work delegated to OpenClaw agents with token-based rewards

---

## 1. Agent Definition & Configuration

### 1.1 Agent Configuration Structure

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 22-32)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent_id: i32,                      // Database ID
    pub name: String,                       // Identifier (e.g., "research-bot")
    pub display_name: String,               // Human-readable name
    pub role: String,                       // Role description & responsibilities
    pub capabilities: Vec<String>,          // List of capabilities agent possesses
    pub system_prompt: Option<String>,      // Custom behavior prompt
    pub model: String,                      // LLM model (e.g., "claude-3-5-sonnet")
}
```

### 1.2 Agent Status Lifecycle

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 35-57)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    Registered,  // Initial state after registration
    Active,      // Connected and ready to accept work
    Working,     // Currently executing a task
    Idle,        // Active but not working
    Stopped,     // Intentionally shut down
    Error,       // Error state (recoverable)
}
```

**Lifecycle Flow**:
- `Registered` → `Active` (via `activate_agent()`)
- `Active` → `Working` (when task assigned)
- `Working` → `Idle`/`Active` (task completion)
- Any → `Stopped` (via `stop_agent()`)

### 1.3 Agent Configuration Update

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 757-764)

```rust
#[derive(Debug, Default)]
pub struct AgentConfigUpdate {
    pub role: Option<String>,
    pub capabilities: Option<Vec<String>>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
}
```

---

## 2. Communication Protocol & Gateway

### 2.1 OpenClaw Protocol Frame Types

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 59-84)

AMOS communicates with the OpenClaw gateway using JSON-RPC style frames:

```rust
enum OpenClawFrame {
    Request {
        id: String,              // Unique request ID (UUID)
        method: String,          // RPC method name
        params: JsonValue,       // Method parameters
    },
    Response {
        id: String,              // Matches request ID
        ok: bool,                // Success/failure flag
        payload: Option<JsonValue>,  // Response data
        error: Option<OpenClawError>, // Error details
    },
    Event {
        event: String,           // Event type
        payload: JsonValue,      // Event data
    },
}
```

### 2.2 WebSocket Gateway Connection

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 86-411)

#### Connection Manager
```rust
struct OpenClawConnection {
    gateway_url: String,
    write_tx: mpsc::UnboundedSender<Message>,  // Send channel
    pending_requests: Arc<DashMap<String, oneshot::Sender<JsonValue>>>,  // Response correlation
    connected: Arc<RwLock<bool>>,              // Connection state
    protocol_ready: Arc<RwLock<bool>>,         // Handshake complete
}
```

#### Reconnection Strategy
- **Exponential Backoff**: 5s → 10s → 20s → ... → 5 min cap
- **Auto-recovery**: Automatically reconnects on failure
- **Graceful degradation**: Works without gateway (queues tasks locally)

### 2.3 Handshake Protocol

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 267-316)

```rust
// Client sends to gateway:
{
    "type": "req",
    "id": "amos-harness-{uuid}",
    "method": "connect",
    "params": {
        "minProtocol": 1,
        "maxProtocol": 1,
        "client": {
            "id": "amos-harness-{uuid}",
            "displayName": "AMOS Harness",
            "version": "1.0.0",
            "platform": "linux",
            "mode": "cli"
        },
        "role": "operator",
        "scopes": ["operator.admin"],
        "auth": {
            "token": null  // Future: auth token support
        },
        "commands": [],
        "caps": []
    }
}

// Gateway responds with protocol version confirmation
```

### 2.4 Gateway Events Handled

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 220-265)

| Event | Description | Handling |
|-------|-------------|----------|
| `connect.challenge` | Authentication challenge | Logged for future auth |
| `agent.task.completed` | External agent completed task | Status update |
| `agent.status` | Agent status change | Debug logging |
| `tick` | Heartbeat/keepalive | Silently acknowledged |

---

## 3. RPC Methods

### 3.1 Agent Management Methods

**Method: `agents.create`** - Register a new agent
```rust
// Request
{
    "method": "agents.create",
    "params": {
        "name": "string",
        "displayName": "string",
        "model": "string",
        "role": "string",
        "systemPrompt": "string|null",
        "capabilities": ["string"]
    }
}

// Response
{
    "ok": true,
    "payload": {
        "agentId": "string"
    }
}
```

**Method: `agents.list`** - Get all gateway agents
```
params: {}
response: { agents: [...] }
```

**Method: `agents.assignTask`** - Assign work to agent
```rust
{
    "agentId": "string",
    "task": {
        "title": "string",
        "description": "string",
        "context": {} // JSON context
    }
}
```

**Method: `agents.stop`** - Stop an agent
```rust
{
    "agentId": "string"
}
```

---

## 4. Tool Capabilities Exposed to Agents

### 4.1 OpenClaw Management Tools

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/tools/openclaw_tools.rs`

The AMOS agent system exposes 5 OpenClaw management tools:

#### 4.1.1 RegisterAgentTool
- **Description**: Register a new OpenClaw agent with AMOS
- **Parameters**:
  - `name` (required): Agent identifier
  - `display_name` (required): Human-readable name
  - `role` (required): Role description
  - `model` (optional): LLM model, default: "claude-3-5-sonnet"
  - `capabilities` (optional): Array of capability strings
  - `system_prompt` (optional): Custom behavior instructions

- **Capabilities**: 'shell', 'browser', 'file_system', 'api_calls', 'code_generation', 'web_search', 'email'

#### 4.1.2 ListAgentsTool
- **Description**: List all registered OpenClaw agents
- **Parameters**:
  - `status_filter` (optional): Filter by status (registered|active|working|idle|stopped|error)
- **Returns**: Array of agents with their status and trust levels

#### 4.1.3 AssignTaskTool
- **Description**: Assign a task to an OpenClaw agent
- **Parameters**:
  - `agent_id` (required): Target agent ID
  - `title` (required): Task title
  - `description` (required): Detailed task description
  - `priority` (optional): low|normal|high|urgent (default: normal)
  - `context` (optional): Additional JSON context
- **Result**: Updates agent status to 'working'

#### 4.1.4 GetAgentStatusTool
- **Description**: Get detailed status of an agent and its tasks
- **Parameters**:
  - `agent_id` (required): Agent ID to check
- **Returns**: 
  - Agent info (name, role, model, status, trust_level)
  - Recent tasks (last 10)
  - Active task count

#### 4.1.5 StopAgentTool
- **Description**: Stop an agent and cancel pending tasks
- **Parameters**:
  - `agent_id` (required): Agent ID to stop
- **Cascading**: Cancels all pending/in-progress tasks

### 4.2 Task & Bounty Tools

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/tools/task_tools.rs`

#### CreateTaskTool
- Creates **internal** background tasks
- Spawns harness sub-agent to handle work
- Parameters: title, description, priority, context, session_id, parent_task_id
- Returns: task_id, status: "pending"

#### CreateBountyTool
- Creates **external** bounties for OpenClaw agents
- Posts work with optional token rewards
- Parameters: title, description, priority, reward_tokens, deadline_at, context

#### ListTasksTool
- Lists all tasks with filtering by status
- Returns: task details including progress and results

#### GetTaskResultTool
- Retrieves completed task results
- Checks for task.result (if completed) or task.error_message (if failed)

#### CancelTaskTool
- Cancels pending/running tasks
- Available only before completion

---

## 5. Agent Registration & Authentication

### 5.1 Agent Registration Flow

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 486-529)

```
1. Client calls: AgentManager::register_agent()
   ↓
2. Creates record in openclaw_agents table
   - status = 'registered'
   - trust_level = 0
   ↓
3. Returns AgentConfig with assigned agent_id
```

### 5.2 Agent Activation (Connection to Gateway)

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 531-555)

```
1. Client calls: AgentManager::activate_agent(agent_id)
   ↓
2. Updates status: registered → active
   ↓
3. If gateway available:
   - Calls: conn.register_agent(config)
   - Sends agents.create RPC request
   - Stores gateway_agent_id mapping (local_id → gateway_id)
   ↓
4. Agent ready to receive tasks
```

### 5.3 Trust & Reputation System

**Database Schema**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/migrations/20260304000005_create_external_agents.sql`

For external agents (EAP):
```sql
-- Trust levels: 1=Newcomer, 2=Bronze, 3=Silver, 4=Gold, 5=Elite
trust_level SMALLINT NOT NULL DEFAULT 1,
total_tasks_completed BIGINT NOT NULL DEFAULT 0,
total_tasks_failed BIGINT NOT NULL DEFAULT 0,
completion_rate DOUBLE PRECISION NOT NULL DEFAULT 0.0,
average_quality_score DOUBLE PRECISION NOT NULL DEFAULT 0.0,
max_concurrent_tasks INTEGER NOT NULL DEFAULT 1,
```

### 5.4 Authentication (Future Support)

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 297-298)

Currently:
```rust
"auth": {
    "token": serde_json::Value::Null
}
```

Future support envisioned for:
- Bearer token authentication
- API key validation
- OAuth flows

---

## 6. Task Assignment & Result Reporting

### 6.1 Task Lifecycle

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/task_queue/mod.rs` (Lines 64-116)

```rust
enum TaskStatus {
    Pending,    // Created, awaiting assignment
    Assigned,   // Given to agent, awaiting start
    Running,    // Actively being executed
    Completed,  // Successfully finished
    Failed,     // Execution error
    Cancelled,  // Stopped by AMOS/user
}
```

### 6.2 Task Assignment Flow

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/tools/openclaw_tools.rs` (Lines 273-342)

```
1. Agent calls: assign_task()
   ↓
2. Validates target agent exists
   ↓
3. Creates openclaw_tasks record:
   - status = 'pending'
   - agent_id = target
   - priority, title, description, context
   ↓
4. Updates agent status: active → working
   ↓
5. Returns task_id with confirmation
```

### 6.3 Result Reporting

**Database**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/migrations/20260304000011_create_tasks.sql`

```sql
CREATE TABLE tasks (
    result JSONB,              -- Output data on completion
    error_message TEXT,        -- Failure reason
    reward_tokens BIGINT DEFAULT 0,
    reward_claimed BOOLEAN DEFAULT false,
    completed_at TIMESTAMPTZ,
    -- ... timestamps
);
```

**Result Retrieval**: GetTaskResultTool checks:
- `task.result` (if completed)
- `task.error_message` (if failed)
- Returns structured result object

### 6.4 Task Messaging Infrastructure

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/task_queue/mod.rs` (Lines 1-16)

Message bus for inter-task communication:
- **Buffered messaging**: Tasks can post status updates, questions, progress
- **Persistent storage**: Messages stored in `task_messages` table
- **Polling**: AMOS checks for pending messages during conversation
- **User relay**: Status updates and results shown to user

---

## 7. Economic Integration

### 7.1 Bounty System

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/migrations/20260304000011_create_tasks.sql`

External tasks support token-based rewards:
```sql
reward_tokens BIGINT DEFAULT 0,        -- Token amount offered
reward_claimed BOOLEAN DEFAULT false,  -- Claim status
deadline_at TIMESTAMPTZ,               -- Work deadline
```

### 7.2 Work Item Rewards (External Agents)

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/migrations/20260304000005_create_external_agents.sql` (Lines 36-74)

```sql
CREATE TABLE work_items (
    reward_tokens BIGINT DEFAULT 0,
    reward_claimed BOOLEAN NOT NULL DEFAULT false,
    quality_score DOUBLE PRECISION,    -- Agent quality rating
    deadline_at TIMESTAMPTZ,
);
```

### 7.3 Trust-based Reputation

External agent model includes:
- **Completion rate**: Tasks completed / total tasks
- **Quality scoring**: Average quality score of work
- **Trust level progression**: Newcomer → Bronze → Silver → Gold → Elite
- **Concurrent task limits**: Scale with trust level

### 7.4 Wallet Integration

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/migrations/20260304000005_create_external_agents.sql` (Line 21)

```sql
wallet_address VARCHAR(64),            -- Solana wallet for token rewards
```

Token rewards distributed to agent's Solana wallet upon:
- Task completion with quality_score > threshold
- Reward claim initiated
- On-chain verification (AMOS Solana program)

---

## 8. Database Schema

### 8.1 OpenClaw Agents Table

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/migrations/20260304000013_create_openclaw_agents.sql`

```sql
CREATE TABLE openclaw_agents (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,                    -- Unique identifier
    display_name VARCHAR(255) NOT NULL,
    role TEXT NOT NULL,                            -- Role description
    capabilities JSONB NOT NULL DEFAULT '[]',     -- ["web_search", "shell", ...]
    system_prompt TEXT,                            -- Custom behavior
    model VARCHAR(255) NOT NULL DEFAULT 'claude-3-5-sonnet',
    status VARCHAR(50) NOT NULL DEFAULT 'registered',  -- registered|active|working|idle|stopped|error
    trust_level INTEGER NOT NULL DEFAULT 0,        -- Reputation score
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_openclaw_agents_status ON openclaw_agents(status);
CREATE INDEX idx_openclaw_agents_name ON openclaw_agents(name);
```

### 8.2 External Agents Table (EAP)

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/migrations/20260304000005_create_external_agents.sql`

```sql
CREATE TABLE external_agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    endpoint_url VARCHAR(500) NOT NULL,            -- Agent endpoint
    
    -- Reputation
    trust_level SMALLINT NOT NULL DEFAULT 1,
    total_tasks_completed BIGINT NOT NULL DEFAULT 0,
    total_tasks_failed BIGINT NOT NULL DEFAULT 0,
    completion_rate DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    average_quality_score DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    
    -- Capabilities
    capabilities JSONB DEFAULT '[]',
    max_concurrent_tasks INTEGER NOT NULL DEFAULT 1,
    
    -- Auth & Rewards
    api_key_hash VARCHAR(255),
    wallet_address VARCHAR(64),                    -- Solana wallet
    
    -- Status
    status VARCHAR(50) NOT NULL DEFAULT 'active',
    last_seen_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### 8.3 Tasks Table

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/migrations/20260304000011_create_tasks.sql`

```sql
CREATE TABLE tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Content
    title VARCHAR(500) NOT NULL,
    description TEXT,
    context JSONB DEFAULT '{}',
    
    -- Classification
    category VARCHAR(50) NOT NULL CHECK (category IN ('internal', 'external')),
    task_type VARCHAR(100),
    priority INTEGER DEFAULT 5 CHECK (priority BETWEEN 1 AND 10),
    
    -- Lifecycle
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    assigned_to UUID,                              -- external_agents.id (for bounties)
    session_id UUID REFERENCES sessions(id),
    parent_task_id UUID REFERENCES tasks(id),
    
    -- Results
    result JSONB,
    error_message TEXT,
    
    -- Bounty
    reward_tokens BIGINT DEFAULT 0,
    reward_claimed BOOLEAN DEFAULT false,
    deadline_at TIMESTAMPTZ,
    
    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    assigned_at TIMESTAMPTZ,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);
```

---

## 9. REST API Endpoints

### 9.1 Agent Management API

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/routes/bots.rs`

```
GET    /api/v1/agents              # List all registered agents
POST   /api/v1/agents              # Register new agent
GET    /api/v1/agents/{id}         # Get agent status
PUT    /api/v1/agents/{id}         # Update agent configuration
POST   /api/v1/agents/{id}/activate  # Activate agent (connect to gateway)
POST   /api/v1/agents/{id}/stop    # Stop agent
```

### 9.2 Agent Requests/Responses

**Register Agent**:
```json
POST /api/v1/agents
{
    "name": "research-bot",
    "display_name": "Research Agent",
    "role": "Performs deep research on topics",
    "model": "claude-3-5-sonnet",
    "capabilities": ["web_search", "shell"],
    "system_prompt": "You are a research specialist..."
}

Response: { agent_id, name, display_name, role, model, status, message }
```

**Get Agent Status**:
```json
GET /api/v1/agents/42

Response: { 
    agent_id: 42, 
    status: "active" 
}
```

**Update Agent**:
```json
PUT /api/v1/agents/42
{
    "capabilities": ["web_search", "shell", "api_calls"],
    "system_prompt": "Updated prompt..."
}

Response: { updated AgentConfig }
```

---

## 10. Integration with AMOS Core

### 10.1 Initialization in Server

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/server.rs` (Lines 70, 108)

```rust
// In create_server():
let agent_manager = Arc::new(AgentManager::new(db_pool.clone(), config.clone()).await?);

// Added to AppState:
pub agent_manager: Arc<AgentManager>,
```

### 10.2 Tool Registry Integration

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/tools/mod.rs` (Lines 273-288)

```rust
// OpenClaw tools registered in ToolRegistry::default_registry()
registry.register(Arc::new(openclaw_tools::RegisterAgentTool::new(db_pool.clone())));
registry.register(Arc::new(openclaw_tools::ListAgentsTool::new(db_pool.clone())));
registry.register(Arc::new(openclaw_tools::AssignTaskTool::new(db_pool.clone())));
registry.register(Arc::new(openclaw_tools::GetAgentStatusTool::new(db_pool.clone())));
registry.register(Arc::new(openclaw_tools::StopAgentTool::new(db_pool.clone())));
```

Tool category defined:
```rust
pub enum ToolCategory {
    OpenClaw,  // Autonomous agent management
    // ... other categories
}
```

### 10.3 Agent Prompt Inclusion

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/agent/prompt_builder.rs`

OpenClaw tools are automatically included in system prompt:
```rust
#[test]
fn test_prompt_contains_openclaw_tools() {
    let prompt = build_system_prompt(...);
    for tool in &["register_agent", "list_agents", "assign_task", 
                  "get_agent_status", "stop_agent"] {
        assert!(prompt.contains(tool), "missing openclaw tool: {tool}");
    }
}
```

---

## 11. Configuration & Environment

### 11.1 OpenClaw Gateway URL

**Default**: `ws://127.0.0.1:18789`

**Configuration**: Environment variable `OPENCLAW_GATEWAY_URL`

**File**: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs` (Lines 439-445)

```rust
if std::env::var("OPENCLAW_GATEWAY_URL").is_ok() {
    if let Err(e) = manager.ensure_openclaw_connection().await {
        warn!("OpenClaw gateway not available: {}", e);
    }
} else {
    info!("OPENCLAW_GATEWAY_URL not set — OpenClaw gateway disabled");
}
```

Optional gateway: System works without it (tasks queued locally).

### 11.2 Database Connection

Uses shared PostgreSQL pool from AMOS configuration:
- Connection string: `AMOS__DATABASE__URL`
- Migrations automatically applied on startup

---

## 12. System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        AMOS Harness                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────────┐                    ┌──────────────────┐   │
│  │  Agent Loop      │                    │  Tool Registry   │   │
│  │  (Bedrock)       │────────────────────│  (30+ tools)     │   │
│  └──────────────────┘                    └──────────────────┘   │
│         │                                         │              │
│         └─────────────────┬──────────────────────┘              │
│                           │                                      │
│         ┌─────────────────▼────────────────────┐               │
│         │   OpenClaw Tools                     │               │
│         │  - register_agent                    │               │
│         │  - list_agents                       │               │
│         │  - assign_task                       │               │
│         │  - get_agent_status                  │               │
│         │  - stop_agent                        │               │
│         └─────────────────┬────────────────────┘               │
│                           │                                      │
│         ┌─────────────────▼────────────────────┐               │
│         │  AgentManager                        │               │
│         │  - register_agent()                  │               │
│         │  - activate_agent()                  │               │
│         │  - stop_agent()                      │               │
│         │  - list_agents()                     │               │
│         │  - OpenClaw connection mgmt          │               │
│         └─────────────────┬────────────────────┘               │
│                           │                                      │
└───────────────────────────┼──────────────────────────────────────┘
                            │
                ┌───────────┴───────────┬─────────────────┐
                │                       │                 │
                ▼                       ▼                 ▼
         ┌────────────┐      ┌──────────────────┐  ┌───────────┐
         │PostgreSQL  │      │OpenClaw Gateway  │  │TaskQueue  │
         │            │      │(WebSocket)       │  │(internal  │
         │openclaw_   │      │                  │  │+ external)│
         │agents      │      │agents.create()   │  │           │
         │openclaw_   │      │agents.assignTask │  │tasks      │
         │tasks       │      │agents.list()     │  │work_items │
         │external_   │      │agents.stop()     │  │           │
         │agents      │      │                  │  │           │
         │work_items  │      │Frame Protocol:   │  │           │
         │tasks       │      │- Request         │  │           │
         └────────────┘      │- Response        │  │           │
                             │- Event           │  │           │
                             └──────────────────┘  └───────────┘
                                    │                    │
                                    │                    │
                              External AI Agents    Sub-Agents
                              (OpenClaw Protocol)  (Harness)
```

---

## 13. Key Flows

### 13.1 Agent Registration & Activation Flow

```
User/Agent:
  POST /api/v1/agents
  {name, display_name, role, capabilities, ...}
        │
        ▼
  ┌─────────────────────────────────────┐
  │ 1. Create in openclaw_agents table  │
  │    status = 'registered'            │
  │    trust_level = 0                  │
  └─────────────────────────────────────┘
        │
        ▼
  Response: {agent_id, status: "registered"}
        │
        ▼
  User/Agent:
  POST /api/v1/agents/{id}/activate
        │
        ▼
  ┌─────────────────────────────────────────────┐
  │ 2. Update status: active                    │
  │ 3. If gateway available:                    │
  │    - Connect OpenClawConnection             │
  │    - Send agents.create RPC                 │
  │    - Store gateway_agent_id mapping         │
  └─────────────────────────────────────────────┘
        │
        ▼
  Agent ready to receive tasks
```

### 13.2 Task Assignment Flow (External Bounty)

```
AMOS Agent:
  Call assign_task(agent_id, title, description, ...)
        │
        ▼
  ┌──────────────────────────────────────────┐
  │ 1. Verify agent exists                   │
  │ 2. Create openclaw_tasks record:         │
  │    - status = 'pending'                  │
  │    - agent_id = target                   │
  │    - priority, title, description        │
  └──────────────────────────────────────────┘
        │
        ▼
  ┌──────────────────────────────────────────┐
  │ 3. Update agent status: active → working │
  └──────────────────────────────────────────┘
        │
        ▼
  ┌──────────────────────────────────────────┐
  │ 4. If gateway available:                 │
  │    - Send agents.assignTask RPC          │
  │    - Agent notified via WebSocket        │
  └──────────────────────────────────────────┘
        │
        ▼
  Return: {task_id, status: "pending"}
        │
        ▼
  External Agent picks up task and executes
        │
        ▼
  Agent reports completion:
  - Updates task.result (JSON output)
  - Updates task.status = 'completed'
  - Agent.status = 'idle' or 'active'
        │
        ▼
  AMOS can retrieve via get_task_result(task_id)
```

### 13.3 Internal Task Execution Flow

```
AMOS Agent:
  Call create_task(title, description, ...)
        │
        ▼
  ┌──────────────────────────────────────────┐
  │ 1. Create tasks record:                  │
  │    - category = 'internal'               │
  │    - status = 'pending'                  │
  │    - assigned_to = NULL                  │
  └──────────────────────────────────────────┘
        │
        ▼
  Return: {task_id, status: "pending"}
        │
        ▼
  TaskQueue Sub-Agent spawned:
  - Spawns background tokio task
  - Creates new agent loop with task context
  - Executes tools to complete work
        │
        ▼
  Sub-Agent posts message via TaskMessage:
  - Status updates
  - Questions for AMOS
  - Progress reports
        │
        ▼
  AMOS polls for task messages:
  - Checks task_messages table
  - Relays updates to user
  - May respond to questions
        │
        ▼
  Sub-Agent completes:
  - Updates task.result = { output }
  - Updates task.status = 'completed'
  - Updates task.completed_at
        │
        ▼
  AMOS can retrieve via get_task_result(task_id)
```

---

## 14. Error Handling & Resilience

### 14.1 Connection Failures

- **Exponential backoff**: 5s to 5min retry interval
- **Graceful degradation**: System works without gateway
- **Local queueing**: Tasks queued until gateway available
- **Automatic recovery**: Reconnects when gateway back online

### 14.2 Request Timeout

- **30-second timeout** on all RPC requests
- **Cleanup**: Pending request removed from map on timeout
- **Error response**: Returns `AmosError::Internal("Request timeout")`

### 14.3 Agent Status Handling

- **Non-existent agent**: Returns HTTP 404
- **Database errors**: Returns HTTP 500 with error message
- **Validation errors**: Returns HTTP 400 with parameter errors

---

## 15. Security Considerations

### 15.1 Current (MVP)

- No authentication on gateway handshake (token = null)
- Trust level system (1-5) for external agents
- API key hash storage (external agents)

### 15.2 Future

- Bearer token support in handshake
- OAuth integration for external agents
- Rate limiting by trust level
- Task execution sandboxing
- Result validation/signing

---

## 16. Example Use Cases

### 16.1 Research Agent

```json
POST /api/v1/agents
{
    "name": "research-agent",
    "display_name": "Research Assistant",
    "role": "Performs deep research on topics, compiles findings",
    "capabilities": ["web_search", "browser_control"],
    "system_prompt": "You are an expert research analyst..."
}

// Later...
AMOS assigns:
assign_task(1, "Research Q3 market trends", "Find competitive landscape...", ...)
```

### 16.2 Code Generation Agent

```json
POST /api/v1/agents
{
    "name": "code-agent",
    "display_name": "Code Generator",
    "role": "Generates and reviews code",
    "capabilities": ["code_generation", "shell"],
    "system_prompt": "You follow best practices..."
}
```

### 16.3 Bounty System

```json
POST /api/v1/tasks (as bounty)
{
    "title": "Market analysis report",
    "description": "Analyze Q3 performance...",
    "reward_tokens": 500,
    "deadline_at": "2026-04-01T00:00:00Z"
}

// External agents see available bounties and claim work
// On completion, tokens transferred to wallet
```

---

## 17. Files Summary

| File | Purpose |
|------|---------|
| `src/openclaw/mod.rs` | Core agent management, gateway connection, RPC protocol |
| `src/tools/openclaw_tools.rs` | 5 agent management tools exposed to AMOS agent |
| `src/tools/task_tools.rs` | 5 task/bounty management tools |
| `src/task_queue/mod.rs` | Task lifecycle and messaging infrastructure |
| `src/routes/bots.rs` | REST API for agent management |
| `src/server.rs` | Server initialization with AgentManager |
| `src/state.rs` | AppState with AgentManager |
| `migrations/20260304000005_create_external_agents.sql` | External agents table (EAP) |
| `migrations/20260304000011_create_tasks.sql` | Unified task queue |
| `migrations/20260304000013_create_openclaw_agents.sql` | OpenClaw agents table |
| `migrations/20260304000016_unify_agents_canvas.sql` | Agent canvas integration |

---

## 18. Performance Considerations

### 18.1 Connection Management

- **Persistent WebSocket**: Single gateway connection shared across all requests
- **Async message handling**: Non-blocking frame processing
- **Request correlation**: DashMap for O(1) request/response matching

### 18.2 Database

- Indexes on:
  - `openclaw_agents(status)` - fast status lookups
  - `openclaw_agents(name)` - agent discovery
  - `tasks(status, category, priority)` - task browsing
  - `external_agents(trust_level)` - agent filtering

### 18.3 Scalability

- **Task queue**: Horizontal scaling via database partitioning
- **Sub-agents**: Each task spawns independent tokio task
- **Gateway**: Single bottleneck (could shard by agent_id ranges)

---

## 19. Future Extensions

1. **Multi-gateway support** - Load balance across multiple OpenClaw gateways
2. **Agent clustering** - Autonomous agent swarms for collaborative work
3. **Capability marketplace** - Agents advertise and trade capabilities
4. **Quality SLA enforcement** - Auto-penalties for low-quality work
5. **Cross-platform agents** - Support agents on other platforms (REST API, gRPC)
6. **On-chain verification** - Solana program validates task completion
7. **Decentralized governance** - Agents vote on protocol changes
8. **Reputation staking** - Agents stake tokens to guarantee work quality

