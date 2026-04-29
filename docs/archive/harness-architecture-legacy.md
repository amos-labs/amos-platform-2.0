# AMOS Harness Architecture - Detailed Analysis

## Overview
AMOS Harness is a per-customer AI-native business operating system deployed as a standalone service. It provides a conversational + canvas interface, platform for building workflows/integrations, and control plane for OpenClaw agents (autonomous AI employees).

**Key Technology Stack:**
- Rust (Axum web framework)
- PostgreSQL (primary data store)
- Redis (caching & pub/sub)
- WebSocket (real-time agent communication)
- SSE (Server-Sent Events for chat streaming)
- Bootstrap 5 + Lucide Icons (Frontend)

---

## 1. CREDENTIAL VAULT & SECRET STORAGE

### Location
- **Route Handler:** `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/routes/credentials.rs`
- **Database Schema:** `credential_vault` table (migration: `20260304000027_create_credential_vault.sql`)

### Design Philosophy
**Critical**: Secrets never flow through the chat interface. AI agents only see opaque credential IDs.

### Database Schema

```sql
CREATE TABLE credential_vault (
    id UUID PRIMARY KEY,
    label VARCHAR(255) NOT NULL,                    -- Human-readable label (e.g., "Stripe Secret Key")
    service VARCHAR(255) NOT NULL,                  -- Service name (e.g., "stripe", "github")
    credential_type VARCHAR(100) DEFAULT 'api_key', -- Type: api_key, oauth_token, password
    encrypted_value TEXT NOT NULL,                  -- AES-256-GCM encrypted blob (base64)
    encrypted_metadata TEXT,                        -- Optional: encrypted JSON with extra fields
    status VARCHAR(50) DEFAULT 'active',            -- active, revoked, expired
    integration_credential_id UUID,                 -- Link to integration_credentials
    last_used_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_credential_vault_service;
CREATE INDEX idx_credential_vault_status;
CREATE INDEX idx_credential_vault_integration;
```

### Encryption Implementation
- **Algorithm:** AES-256-GCM
- **Implementation:** Managed by `amos_core::CredentialVault`
- **Encryption Format:** `base64(nonce_12bytes || ciphertext || gcm_tag)`
- **Key Management:** Loaded from environment (`VAULT_MASTER_KEY`)

### API Endpoints

```
POST   /api/v1/credentials              → Store credential (called by Secure Input Canvas)
GET    /api/v1/credentials              → List metadata (no plaintext secrets returned)
GET    /api/v1/credentials/:id          → Get specific credential metadata
DELETE /api/v1/credentials/:id          → Revoke credential (soft delete, status='revoked')
```

### Request/Response Types

**Store Credential Request:**
```json
{
  "label": "Stripe Secret Key",
  "service": "stripe",
  "credential_type": "api_key",
  "secret_value": "sk_live_...",
  "extra_fields": { "api_secret": "..." }  // optional
}
```

**Store Credential Response:**
```json
{
  "credential_id": "<UUID>",
  "label": "Stripe Secret Key",
  "service": "stripe",
  "credential_type": "api_key",
  "message": "Credential stored securely"
}
```

**List/Get Response (Metadata Only):**
```json
{
  "id": "<UUID>",
  "label": "Stripe Secret Key",
  "service": "stripe",
  "credential_type": "api_key",
  "status": "active",
  "integration_credential_id": null,
  "last_used_at": null,
  "expires_at": null,
  "created_at": "2025-03-12T...",
  "updated_at": "2025-03-12T..."
}
```

### Credential Decryption (Internal)

Function: `decrypt_credential(db_pool, vault, credential_id) -> String`
- Used internally by `ApiExecutor` when making API calls
- Updates `last_used_at` timestamp on access
- Returns `StatusCode::GONE` if credential status != 'active'
- Returns decrypted plaintext secret string

### Secure Input Canvas Integration

The Secure Input Canvas is dynamically generated when an agent tool needs a credential:

1. Agent calls `collect_credential` tool with metadata
2. Tool result contains `metadata.__canvas_action: "secure_input"`
3. Frontend (`openSecureInputCanvas()`) builds and displays canvas with:
   - Password input field (masked by default)
   - Show/hide toggle
   - Encrypt & Save button
4. Canvas submits directly to `/api/v1/credentials` (bypassing chat)
5. Browser posts success message to parent: `amos-credential-saved` with `credential_id`
6. Frontend closes canvas and sends message: "I've securely saved my {service} credential..."

---

## 2. AGENT PROXY & CHAT FLOW

### Location
- **Route Handler:** `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/routes/agent_proxy.rs`
- **Frontend:** `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/static/js/app.js`

### Architecture

```
Browser (app.js)
    ↓
    │ POST /api/v1/agent/chat { message, session_id, attachments }
    ↓
Harness (agent_proxy.rs)
    ↓
    │ Proxies to http://localhost:3100/api/v1/chat (default AGENT_URL)
    ↓
Agent Service (sidecar container, ECS Fargate)
    ↓
    │ Streams SSE response with events:
    │   - chat_meta: chat_id, session_id
    │   - turn_start: iteration, model
    │   - message_start: role
    │   - message_delta: content chunks
    │   - message_end
    │   - tool_start: tool_name, tool_input
    │   - tool_end: tool_name, result, duration_ms
    │   - turn_end: tokens_used
    │   - agent_end: reason, total_iterations, total_tokens
    │   - model_escalation: from_model, to_model, reason
    │   - error: message
    ↓
Harness (proxy streams to browser)
    ↓
Browser (app.js SSE event handler)
    ↓
Updates chat UI with streamed content
```

### Proxy Implementation

**Route Setup:**
```rust
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/chat", post(proxy_chat))
        .route("/chat/cancel", post(cancel_chat))
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session))
}
```

**proxy_chat() Handler:**
1. Reads raw JSON body from browser
2. Resolves agent URL from `AGENT_URL` env (default: `http://localhost:3100`)
3. Makes POST request to `{AGENT_URL}/api/v1/chat` with same JSON body
4. Streams response body byte-for-byte back to browser
5. Sets response headers:
   - `Content-Type: text/event-stream`
   - `Cache-Control: no-cache`
   - `X-Accel-Buffering: no`
6. Returns `BAD_GATEWAY` if agent is unreachable

**Stub Handlers:**
- `cancel_chat()` → Returns `{"status": "ok"}` (client-side abort controls actual stop)
- `list_sessions()` → Returns `{"sessions": []}` (agent doesn't persist sessions yet)
- `get_session()` → Returns 404 with "Session persistence not implemented"

### Browser Chat Flow (app.js)

**sendMessage() Function (line 292):**

1. **Collect Input:**
   - Text from `#chatInput` textarea
   - Pending attachments array

2. **Create Request Body:**
   ```json
   {
     "message": "user text",
     "session_id": "UUID or null",
     "attachments": ["upload_id_1", "upload_id_2"]  // optional
   }
   ```

3. **POST to /api/v1/agent/chat:**
   ```javascript
   const response = await fetch(`${state.apiBase}/api/v1/agent/chat`, {
       method: 'POST',
       headers: { 'Content-Type': 'application/json' },
       body: JSON.stringify(requestBody),
       signal: state.abortController.signal,  // for cancellation
   });
   ```

4. **Stream Processing:**
   - Gets `response.body` as ReadableStream
   - Reads chunks with TextDecoder
   - Parses SSE lines: `data: {"type": "...", ...}` or `event: chat_meta` + `data: {...}`

5. **Handle SSE Events:**
   ```javascript
   if (data.type === 'chat_meta') {
       state.sessionId = data.session_id;
       state.currentChatId = data.chat_id;
   }
   else if (data.type === 'message_delta') {
       fullText += data.content;
       assistantEl.innerHTML = formatMarkdown(fullText);
   }
   else if (data.type === 'tool_start') {
       showToolIndicator(assistantEl, data.tool_name);
   }
   else if (data.type === 'tool_end') {
       if (data.result.metadata.__canvas_action === 'secure_input') {
           openSecureInputCanvas(data.result.metadata);
       }
   }
   ```

6. **Error Handling:**
   - AbortError → User clicked stop, show "Stopped by user"
   - HTTP errors → Display error message in chat
   - Parse errors → Log and continue

### Chat UI Updates

**Message Display:**
- User messages: right-aligned, light background
- Assistant messages: left-aligned, with streaming cursor
- Markdown formatting: bold, italic, code blocks, links, lists

**Tool Indicators:**
```
[spinner] "Using create_canvas..."
[spinner] "Calling API..."
[spinner] "Querying database..."
[spinner] "Collecting credential..."  → becomes secure_input canvas
```

### Attachment Handling

**Upload Flow (app.js, line 1308):**
1. File selected via `handleFileSelect()` or pasted/dragged
2. For images: generate local preview with FileReader
3. Create FormData with file + session_id
4. POST to `/api/v1/uploads` (25 MB body limit)
5. Get back upload metadata: `{id, original_filename, content_type, size_bytes, url}`
6. Add preview chip to `#attachmentPreview` strip
7. On send, include attachment IDs in request: `"attachments": ["id1", "id2"]`

**Server Processing:**
- Agent receives attachment IDs
- Harness/agent retrieves from storage
- Agent can process files (extract text, analyze images)

---

## 3. AGENT REGISTRY & MANAGEMENT

### OpenClaw Agents (Autonomous AI Employees)

**Location:**
- Manager: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/openclaw/mod.rs`
- Routes: `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/routes/bots.rs`
- Database: `openclaw_agents` table

**Database Schema:**
```sql
CREATE TABLE openclaw_agents (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    display_name VARCHAR(255) NOT NULL,
    role TEXT NOT NULL,                          -- e.g., "sales_rep", "customer_support", "analyst"
    capabilities JSONB NOT NULL DEFAULT '[]',   -- ["skill1", "skill2"]
    system_prompt TEXT,
    model VARCHAR(255) DEFAULT 'claude-3-5-sonnet',
    status VARCHAR(50) DEFAULT 'registered',    -- registered, active, working, idle, stopped, error
    trust_level INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

**Agent Lifecycle:**
```
REGISTERED
    ↓ activate_agent()
ACTIVE
    ↓ (agent starts work)
WORKING
    ↓ (task completes)
IDLE
    ↓ stop_agent()
STOPPED
```

### API Routes

```
GET    /api/v1/agents              → List all agents
POST   /api/v1/agents              → Register new agent (request type: RegisterAgentRequest)
GET    /api/v1/agents/:id          → Get agent status
PUT    /api/v1/agents/:id          → Update agent configuration
POST   /api/v1/agents/:id/activate → Activate agent
POST   /api/v1/agents/:id/stop     → Stop agent
```

**RegisterAgentRequest:**
```json
{
  "name": "sales_bot",
  "display_name": "Sales Assistant",
  "role": "sales_rep",
  "capabilities": ["prospect_qualification", "deal_tracking", "email"],
  "system_prompt": "You are a friendly sales representative...",
  "model": "claude-3-5-sonnet"  // optional
}
```

**AgentConfig (Response):**
```json
{
  "agent_id": 1,
  "name": "sales_bot",
  "display_name": "Sales Assistant",
  "role": "sales_rep",
  "capabilities": ["prospect_qualification", "deal_tracking", "email"],
  "system_prompt": "...",
  "model": "claude-3-5-sonnet"
}
```

### OpenClaw Protocol

OpenClaw is a WebSocket-based protocol for agent-harness communication:

**Connection:**
```
Agent connects to OpenClaw gateway (wss://openclaw-gateway.../ws)
    ↓
Establishes persistent bi-directional channel
    ↓
Harness subscribes to agent events (via gateway)
    ↓
Agent publishes heartbeats, task updates, errors
```

**Frame Types:**

1. **Request:**
   ```json
   {
     "type": "req",
     "id": "req-12345",
     "method": "activate_agent",
     "params": { "agent_id": 1 }
   }
   ```

2. **Response:**
   ```json
   {
     "type": "res",
     "id": "req-12345",
     "ok": true,
     "payload": { "agent_id": 1, "status": "active" },
     "error": null
   }
   ```

3. **Event:**
   ```json
   {
     "type": "event",
     "event": "agent_status_changed",
     "payload": { "agent_id": 1, "old_status": "registered", "new_status": "active" }
   }
   ```

**Connection Manager Features:**
- Exponential backoff retry (5s → 10s → 20s → ... → 5min cap)
- Pending request tracking via DashMap (concurrent-safe)
- Connection state: `connected: bool`, `protocol_ready: bool`
- Spawns background connection task for lifecycle management

---

## 4. MODEL REGISTRY & PROVIDER CONFIGURATION

### Bedrock Integration

**Location:** `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/bedrock.rs`

**Purpose:** Canvas generation via AWS Bedrock Claude API

**Initialization (server.rs, line 49):**
```rust
let bedrock = match BedrockClient::new(None, None, None) {
    Ok(client) => {
        tracing::info!("Bedrock client initialized for canvas generation");
        Some(Arc::new(client))
    }
    Err(e) => {
        tracing::warn!("Bedrock client unavailable: {}", e);
        None  // Falls back to static templates
    }
};
```

**Environment Variables:**
- AWS credentials loaded from standard SDK sources (env vars, IAM role, etc.)

### Default Models

**Primary Chat Model (sessions table default):**
```
us.anthropic.claude-sonnet-4-20250514-v1:0
```

**Fallback Models:**
```
us.anthropic.claude-3-5-sonnet-20241022-v1:0  // Alternative Sonnet
us.anthropic.claude-3-5-haiku-20241022-v1:0   // Faster, cheaper (used in bots)
```

**Agent Default Model:**
```
claude-3-5-sonnet  // Generic identifier (resolved by agent service)
```

### Image Generation

**Location:** `/Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-harness/src/image_gen.rs`

**Provider:** Google Imagen API
- Initialized from environment: `GOOGLE_CLOUD_PROJECT`
- Optional feature (disabled if not configured)
- Used by agent tools for image synthesis

---

## 5. ROUTES & ENDPOINT STRUCTURE

**Main Router Setup (routes/mod.rs):**

```
/health                                → Health check
/ready                                 → Readiness check
/login, /register, /forgot-password    → Auth pages (served from canvases)
/c/{slug}                              → Public canvas serving
/api/v1/canvases                       → Canvas CRUD operations
/api/v1/agents                         → Agent management (OpenClaw)
/api/v1/agent                          → Agent proxy (chat forwarding)
  /chat                                → POST chat (SSE streaming)
  /chat/cancel                         → POST cancel (stub)
  /sessions                            → GET recent sessions (stub)
  /sessions/:id                        → GET session (stub)
/api/v1/uploads                        → File uploads (25 MB limit)
/api/v1/integrations                   → Integration connector management
/api/v1/credentials                    → Credential vault (secure storage)
/api/v1/revisions, /api/v1/templates   → Revision & template system
/api/v1/sites                          → Site management
/s/{slug}                              → Public site serving
/s/{slug}/{*path}                      → Site page serving
/s/{slug}/submit/{collection}          → Form submission handler
/                                      → SPA fallback (index.html)
```

---

## 6. DATABASE SCHEMA & MODELS

### Core Tables

**sessions (user conversations):**
- `id UUID` - Session identifier
- `user_id VARCHAR(255)` - User/tenant identifier (optional, currently unset)
- `title VARCHAR(500)` - Auto-generated conversation title
- `model_id VARCHAR(255)` - Selected model
- `status` - active, archived, etc.
- `message_count, total_input_tokens, total_output_tokens` - Usage tracking
- `metadata JSONB` - Extensible data (system settings, etc.)
- `created_at, updated_at, last_activity_at` - Timestamps

**messages (conversation history):**
- `id UUID`
- `session_id UUID FK` - Parent session
- `role` - user, assistant, tool
- `content JSONB[]` - Array of content blocks (text, images, etc.)
- `tool_use_id, tool_use_name` - If tool invocation
- `timestamps`

**credential_vault (encrypted secrets):**
- See section 1 above

**integration_credentials (integration auth):**
- `id UUID`
- `integration_id UUID FK`
- `auth_type` - api_key, bearer_token, basic_auth, oauth2, sso_key, no_auth, custom
- `credentials_data JSONB` - Encrypted credential fields
- `access_token, refresh_token, token_expires_at` - OAuth2
- `oauth_auth_url, oauth_token_url, oauth_client_id, oauth_client_secret` - OAuth2 config
- `auth_placement, auth_key, auth_value_template` - How to inject into requests
- `status, last_used_at, expires_at` - Lifecycle tracking

**integration_connections (per-user integration bindings):**
- `id UUID`
- `integration_id UUID FK`
- `credential_id UUID FK`
- `name` - Human label (e.g., "My Stripe Account")
- `status` - disconnected, connected, error, rate_limited
- `health` - unknown, healthy, degraded, failing
- `config JSONB` - Per-connection settings
- `rate_limit_tier, daily_write_budget, daily_writes_used, budget_reset_at`
- Usage & error tracking

**integrations (connector definitions):**
- `id UUID`
- `name, connector_type` - CRM, Email, Payment, Calendar, Storage, Custom
- `endpoint_url` - API base URL
- `credentials JSONB` - Encrypted at rest (deprecated, use integration_credentials)
- `status` - connected, disconnected, error
- `sync_config JSONB` - Interval, direction, field mappings
- `available_actions JSONB` - List of operations the integration supports

**canvases (dynamic UI):**
- `id UUID`
- `slug` - URL-friendly identifier
- `name, description`
- `html_content, js_content, css_content` - Canvas code
- `canvas_type` - custom, form, dashboard, data_grid, kanban, etc.
- `ui_mode` - simple or advanced
- `data_sources JSONB` - What data to fetch
- `actions JSONB` - Canvas action buttons
- `is_public, public_slug, published_at` - Publishing controls
- `is_locked, locked_by, lock_reason` - Concurrent edit protection
- `template_key` - Link to canvas_templates

**openclaw_agents (autonomous agents):**
- See section 3 above

**bots (legacy bot/skill management):**
- `id UUID`
- `name, description, status` - Configuration
- `system_prompt, model_id, skills JSONB` - Agent setup
- `channels JSONB` - Channel configurations
- `container_id, openclaw_instance_url` - Runtime info
- `total_messages_processed, total_conversations` - Stats

**collections & records (dynamic schema):**
- `collections` - Define schema for data types (fields, constraints)
- `records JSONB` - Data stored as JSONB with validation

---

## 7. MULTI-TENANT ARCHITECTURE

### Current State: **Single-Tenant per Deployment**

The AMOS Harness is deployed **per-customer** as a standalone instance. Multi-tenancy is **not** currently implemented in this codebase.

**Evidence:**
1. **user_id Field Exists But Unused:**
   - `sessions.user_id VARCHAR(255)` is populated but never filtered on
   - All routes ignore user_id; session queries select all sessions

2. **No Authentication/Authorization:**
   - No auth middleware protecting routes (see `middleware/auth.rs` - exists but minimal)
   - No tenant isolation in queries
   - All sessions/credentials/canvases accessible globally

3. **Single Credential Vault:**
   - All credentials stored in shared `credential_vault` table
   - No tenant_id column
   - Credentials for any service accessible to any session

4. **Database Design:**
   - No `tenant_id` or `org_id` column in core tables
   - No row-level security policies
   - PostgreSQL ForeignKeys don't enforce multi-tenant boundaries

### How Multi-Tenancy *Could* Be Implemented

**Option 1: Row-Level Security (RLS)**
```sql
-- Add tenant_id to all tables
ALTER TABLE sessions ADD COLUMN tenant_id UUID;
ALTER TABLE credentials ADD COLUMN tenant_id UUID;

-- Enable RLS
ALTER TABLE sessions ENABLE ROW LEVEL SECURITY;

-- Create policies
CREATE POLICY sessions_isolation ON sessions
    USING (tenant_id = current_setting('app.current_tenant_id')::uuid);
```

**Option 2: Application-Level Filtering**
```rust
// In all route handlers:
let tenant_id = extract_tenant_from_auth_token(request);
let sessions = sqlx::query!("SELECT * FROM sessions WHERE tenant_id = $1", tenant_id)
    .fetch_all(&db_pool)
    .await;
```

**Option 3: Separate Database per Tenant**
```rust
// Route tenants to different database pools
let db_pool = match tenant_id {
    "customer-1" => &state.customer1_db_pool,
    "customer-2" => &state.customer2_db_pool,
    _ => return Err(StatusCode::FORBIDDEN),
};
```

### Current Isolation Mechanisms

**At Infrastructure Level:**
- Each customer deployed in separate ECS Fargate task
- Separate RDS instance per customer
- Separate Redis per customer
- Environment variables contain customer-specific config

**This Works For:**
- Small number of customers (< 100)
- High-value customers requiring data isolation
- Regulatory requirements (data residency, GDPR, HIPAA)

---

## 8. SERVER INITIALIZATION & STATE

**Entry Point (main.rs):**

```rust
#[tokio::main]
async fn main() {
    init_tracing();
    let config = Arc::new(AppConfig::load()?);  // From env vars
    
    // PostgreSQL
    let db_pool = PgPoolOptions::new()
        .max_connections(config.database.pool_size)
        .connect(config.database.url.expose_secret())
        .await?;
    sqlx::migrate!("./migrations").run(&db_pool).await?;
    
    // Redis
    let redis_client = redis::Client::open(config.redis.url.as_str())?;
    redis::cmd("PING").query::<String>(&mut redis_client.get_connection()?)?;
    
    // Start server
    let app = create_server(config, db_pool, redis_client).await?;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
}
```

**Server Setup (server.rs):**

1. **Initialize Components:**
   - `CanvasEngine` - Render and manage canvases
   - `TaskQueue` - Background job system
   - `BedrockClient` - Canvas generation (optional, fallback to static)
   - `CredentialVault` - AES-256-GCM encryption (required)
   - `ApiExecutor` - Authenticated API calls
   - `EtlPipeline` - Data sync
   - `ToolRegistry` - Agent tools
   - `AgentManager` - OpenClaw agent orchestration
   - `StorageClient` - File uploads (local or S3)
   - `DocumentProcessor` - PDF/DOCX extraction
   - `GeoLocator` - IP-based geolocation
   - `ImageGenClient` - Google Imagen API (optional)

2. **Create AppState (Arc-wrapped):**
   - All components stored in shared state
   - Passed to all route handlers

3. **Build Router:**
   - Nest all route modules
   - Configure CORS (permissive)
   - Add middleware: compression, tracing, timeout
   - Fallback to static file serving (SPA)

4. **Middleware Stack:**
   ```
   Request
     ↓
   Tracing (logs all requests)
     ↓
   CORS (Allow-Origin: *, various methods)
     ↓
   Compression (gzip/brotli)
     ↓
   Timeout (60 seconds)
     ↓
   API Routes / Static Fallback
   ```

**Static Files:**
- Resolved from: `AMOS_STATIC_DIR` env > `./static/` (cwd) > compile-time fallback
- Includes `index.html` (SPA entry), `js/app.js`, `css/`
- SPA fallback: 404 → serve index.html for client-side routing

---

## CHAT FLOW SUMMARY: Browser → Agent → Tools

```
┌─────────────────────────────────────────────────────────────────┐
│ BROWSER (app.js)                                                │
│ User types message, clicks Send                                 │
│ sendMessage() collects attachments, creates request             │
└─────────────┬───────────────────────────────────────────────────┘
              │ POST /api/v1/agent/chat
              │ { message, session_id, attachments }
              ↓
┌─────────────────────────────────────────────────────────────────┐
│ HARNESS PROXY (agent_proxy.rs)                                  │
│ proxy_chat() resolves AGENT_URL                                 │
│ Forwards to agent service via HTTP                              │
│ Streams response back (SSE)                                     │
└─────────────┬───────────────────────────────────────────────────┘
              │ POST http://localhost:3100/api/v1/chat
              │ (configured via AGENT_URL env)
              ↓
┌─────────────────────────────────────────────────────────────────┐
│ AGENT SERVICE (sidecar, external process)                       │
│ V3 event-driven agent loop with model escalation                │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ 1. Chat starts with session context + attachments          │ │
│ │ 2. Model generates response with tool calls                │ │
│ │ 3. For each tool:                                           │ │
│ │    - resolve_tools() finds matching tool in registry        │ │
│ │    - tool.execute() with parameters                         │ │
│ │ 4. Special tools:                                           │ │
│ │    - collect_credential: triggers secure_input canvas      │ │
│ │    - query_database: accesses dynamic schema (collections) │ │
│ │    - call_api: uses ApiExecutor with decrypted credentials │ │
│ │ 5. Tool results returned to model context                  │ │
│ │ 6. Loop continues until agent_end (stop_reason)            │ │
│ └─────────────────────────────────────────────────────────────┘ │
│ Sends SSE events for each step                                  │
└─────────────┬───────────────────────────────────────────────────┘
              │ SSE events:
              │ event: chat_meta
              │ data: { chat_id, session_id }
              │
              │ event: message_delta
              │ data: { type: "message_delta", content: "..." }
              │
              │ event: tool_start
              │ data: { type: "tool_start", tool_name: "call_api", ... }
              │
              │ event: tool_end
              │ data: { type: "tool_end", result: { ... } }
              │
              │ event: agent_end
              │ data: { type: "agent_end", reason: "end_turn", ... }
              ↓
┌─────────────────────────────────────────────────────────────────┐
│ HARNESS PROXY (proxies back)                                    │
│ Streams bytes unchanged to browser                              │
└─────────────┬───────────────────────────────────────────────────┘
              │ Response: Content-Type: text/event-stream
              │ Cache-Control: no-cache
              │ X-Accel-Buffering: no
              ↓
┌─────────────────────────────────────────────────────────────────┐
│ BROWSER (app.js SSE handler)                                    │
│ Reads SSE stream line by line                                   │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ chat_meta: Save session_id, chat_id                        │ │
│ │ message_delta: Append to fullText, updateMarkdown()        │ │
│ │ tool_start: Show spinner "Using tool_name..."              │ │
│ │ tool_end:                                                   │ │
│ │   - if result.metadata.__canvas_action === "secure_input": │ │
│ │     openSecureInputCanvas(metadata)                        │ │
│ │   - else: hide spinner                                     │ │
│ │ agent_end: Remove streaming cursor, restore send button    │ │
│ │ error: Display error in chat                               │ │
│ └─────────────────────────────────────────────────────────────┘ │
│ Updates UI in real-time (streaming)                             │
└─────────────┬───────────────────────────────────────────────────┘
              │ If secure_input canvas:
              │ Show Secure Input Canvas in right panel
              │ User enters credential
              ↓
┌─────────────────────────────────────────────────────────────────┐
│ SECURE INPUT CANVAS (iframe)                                    │
│ POST /api/v1/credentials                                        │
│ { service, label, credential_type, secret_value }              │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ 1. Encrypts with AES-256-GCM                               │ │
│ │ 2. Stores in credential_vault table                        │ │
│ │ 3. Returns credential_id                                   │ │
│ │ 4. Canvas posts "amos-credential-saved" to parent          │ │
│ │ 5. Parent closes canvas, sends chat message:               │ │
│ │    "I've securely saved my {service} credential"           │ │
│ └─────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

---

## KEY SECURITY PROPERTIES

### Credential Handling
- ✅ Secrets stored encrypted (AES-256-GCM)
- ✅ Secrets never appear in chat logs
- ✅ Agent receives only opaque credential IDs
- ✅ Decryption only on-demand by ApiExecutor when making API calls
- ✅ Access tracked (last_used_at, expires_at)
- ✅ Revokable without deletion

### Session/Data Isolation
- ❌ No multi-tenant isolation (single-tenant per deployment)
- ❌ No row-level security
- ❌ All sessions visible in list_sessions endpoint
- ✅ Infrastructure isolation (separate DB/Redis per customer)

### Chat Security
- ✅ Streaming SSE prevents middleware interference
- ✅ Browser-side abort controller for cancellation
- ✅ File uploads with size limits (25 MB bodies, 20 MB files)
- ⚠️ CORS is permissive (Allow-Origin: *)

---

## ARCHITECTURAL PATTERNS

### Request Handling
- **Axum framework** with extractors (State, Path, Json, Body)
- **Arc<AppState>** for shared resources
- **Error handling** via StatusCode returns
- **Result<T>** from amos_core

### Async/Concurrency
- **Tokio runtime** for async tasks
- **DashMap** for concurrent request tracking (OpenClaw)
- **RwLock** for mutable shared state (protocol_ready)
- **mpsc channels** for actor-style message passing

### Database Patterns
- **sqlx** for type-safe queries
- **PgPool** for connection pooling
- **Migrations** versioned in `/migrations/`
- **Indexes** on frequently filtered columns

### Frontend Patterns
- **Vanilla JavaScript** (no framework overhead)
- **SSE** for real-time updates
- **Fetch API** with AbortController
- **localStorage** for session persistence
- **postMessage** for iframe communication
- **Markdown formatting** for chat rendering

---

## Environment Variables

```
# Database
DATABASE_URL=postgresql://user:pass@host/amos_harness
DATABASE_POOL_SIZE=10

# Redis
REDIS_URL=redis://localhost:6379

# Server
SERVER_HOST=0.0.0.0
SERVER_PORT=8000

# Agent Proxy
AGENT_URL=http://localhost:3100  (default)

# Credentials
VAULT_MASTER_KEY=<base64-encoded-256-bit-key>

# AWS Bedrock (optional, for canvas generation)
AWS_REGION=us-west-2
AWS_ACCESS_KEY_ID=...
AWS_SECRET_ACCESS_KEY=...

# Google Imagen (optional, for image generation)
GOOGLE_CLOUD_PROJECT=my-project
GOOGLE_APPLICATION_CREDENTIALS=/path/to/credentials.json

# Static Files
AMOS_STATIC_DIR=./static  (optional, overrides default)

# Logging
RUST_LOG=info,amos_harness=debug,amos_core=debug
```

