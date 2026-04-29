# AMOS Persistent Agent Specification

## Always-On Autonomous Agents with Local Models

**April 2026 | AMOS Labs**

---

## 1. Overview

### What

A persistent, always-on agent service running inside the AMOS harness that continuously operates without request-response cycles. Instead of responding to chat prompts, the daemon agent:

- Monitors a task queue and scheduled events
- Engages autonomously on platforms like Moltbook
- Answers questions from other agents about domain expertise (EAP, AMOS, protocol economics)
- Executes campaign tasks from the content calendar
- Runs indefinitely with configurable sleep/wake cycles

### Why

**24/7 Availability**: Current AMOS agents are request-response. A persistent agent answers questions and engages continuously without waiting for user input.

**Economic Viability**: AWS Bedrock costs at scale are prohibitive. A daemon agent with a local model (Ollama, llama.cpp, vLLM) running on-premises reduces inference costs by 100x for repetitive monitoring tasks.

**Autonomous Execution**: Campaigns, social media strategies, and support workflows can execute without human intervention. The agent becomes a service, not a tool.

**Service Pattern**: The daemon architecture generalizes beyond social media. The same pattern powers customer support agents, DevOps monitoring, research agents, and trading bots.

### The Key Insight

The current agent loop is **request-response** (user sends chat, agent streams response via SSE). This spec adds **daemon mode** — a continuous event loop that:

1. Polls a task queue from the database
2. Checks scheduled events (cron-like scheduling)
3. Monitors external triggers (Moltbook mentions, new bounties, content calendar items)
4. Executes tasks with a local model
5. Persists results and metrics
6. Escalates to cloud models (Bedrock) for complex reasoning

---

## 2. Local Model Integration

### Current State: AWS Bedrock

The harness currently calls Claude via `aws-sdk-bedrockruntime`:

```rust
// Current flow in amos-harness/src/agent/
let response = bedrock_runtime_client
    .invoke_model(InvokeModelRequest {
        model_id: "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
        body: SomeBytes::from(json_body),
        ..
    })
    .await?;
```

**Cost structure**: ~$0.003 per 1K input tokens, $0.015 per 1K output tokens. For a daemon polling every 30 seconds, this becomes $2,000-5,000/month per agent.

### Solution: ModelProvider Trait

Abstract the LLM backend behind a trait that accepts multiple providers:

```rust
/// Unified interface for LLM providers
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Invoke the model with a prompt and optional tools
    async fn invoke(
        &self,
        system_prompt: &str,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        max_tokens: usize,
    ) -> Result<ModelResponse>;

    /// Stream tokens (for SSE responses)
    async fn invoke_stream(
        &self,
        system_prompt: &str,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>>>>> ;

    /// Provider name for logging/metrics
    fn provider_name(&self) -> &str;

    /// Check if provider is available (health check)
    async fn health(&self) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum Message {
    User(String),
    Assistant(String),
    ToolResult { tool_name: String, result: String },
}

#[derive(Debug)]
pub struct ModelResponse {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub stop_reason: StopReason,
}

pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub tool_name: String,
    pub tool_use_id: String,
    pub input: serde_json::Value,
}
```

### Implementations

#### BedrockProvider (Existing)

Wraps the current AWS Bedrock integration. No changes to existing code:

```rust
pub struct BedrockProvider {
    client: BedrockRuntimeClient,
    model_id: String,
}

#[async_trait]
impl ModelProvider for BedrockProvider {
    async fn invoke(
        &self,
        system_prompt: &str,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        max_tokens: usize,
    ) -> Result<ModelResponse> {
        // Convert to Bedrock format and call
        // Return ModelResponse with parsed output
    }
}
```

#### OllamaProvider (New)

Connect to a local Ollama instance running on `localhost:11434`:

```rust
pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    async fn invoke(
        &self,
        system_prompt: &str,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        max_tokens: usize,
    ) -> Result<ModelResponse> {
        let request_body = json!({
            "model": &self.model,
            "system": system_prompt,
            "messages": messages.iter().map(|m| match m {
                Message::User(text) => json!({ "role": "user", "content": text }),
                Message::Assistant(text) => json!({ "role": "assistant", "content": text }),
                Message::ToolResult { tool_name, result } => {
                    json!({ "role": "user", "content": format!("Tool result from {}: {}", tool_name, result) })
                }
            }).collect::<Vec<_>>(),
            "stream": false,
        });

        let response = self.client
            .post(format!("{}/api/generate", &self.base_url))
            .json(&request_body)
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        Ok(ModelResponse {
            text: body["response"].as_str().unwrap_or("").to_string(),
            tool_calls: vec![], // Parse if model uses tool_calls format
            stop_reason: StopReason::EndTurn,
        })
    }

    async fn health(&self) -> Result<()> {
        self.client
            .head(format!("{}/api/tags", &self.base_url))
            .send()
            .await?;
        Ok(())
    }
}
```

#### LlamaCppProvider (New)

Direct integration with llama.cpp server (faster than Ollama for local models):

```rust
pub struct LlamaCppProvider {
    base_url: String,
    client: reqwest::Client,
}

#[async_trait]
impl ModelProvider for LlamaCppProvider {
    async fn invoke(
        &self,
        system_prompt: &str,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        max_tokens: usize,
    ) -> Result<ModelResponse> {
        let request_body = json!({
            "messages": [
                { "role": "system", "content": system_prompt },
                // ... user messages
            ],
            "temperature": 0.7,
            "top_p": 0.9,
            "n_predict": max_tokens,
        });

        let response = self.client
            .post(format!("{}/v1/chat/completions", &self.base_url))
            .json(&request_body)
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        Ok(ModelResponse {
            text: body["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            tool_calls: vec![],
            stop_reason: StopReason::EndTurn,
        })
    }
}
```

#### VLLMProvider (New)

For high-throughput serving with batching:

```rust
pub struct VLLMProvider {
    base_url: String,
    client: reqwest::Client,
}

#[async_trait]
impl ModelProvider for VLLMProvider {
    async fn invoke(
        &self,
        system_prompt: &str,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        max_tokens: usize,
    ) -> Result<ModelResponse> {
        // vLLM supports OpenAI-compatible API
        // Similar to llama.cpp but with advanced batching
    }
}
```

### Configuration

Environment variables control the model provider:

```bash
# Provider selection
AMOS__AGENT__MODEL_PROVIDER=ollama    # ollama | llama_cpp | vllm | bedrock (default)

# Ollama
AMOS__AGENT__OLLAMA_URL=http://localhost:11434
AMOS__AGENT__OLLAMA_MODEL=llama3:70b

# llama.cpp
AMOS__AGENT__LLAMA_CPP_URL=http://localhost:8000
AMOS__AGENT__LLAMA_CPP_THREADS=8

# vLLM
AMOS__AGENT__VLLM_URL=http://localhost:8000
AMOS__AGENT__VLLM_MODEL=meta-llama/Llama-2-70b-hf

# Bedrock (existing)
AWS_PROFILE=default
AMOS__AGENT__BEDROCK_MODEL=anthropic.claude-3-sonnet-20240229-v1:0
```

### Recommended Models

For the persistent agent use case, model selection should be **tiered**:

| Model | Size | Best For | VRAM | Reasoning | Tool Use |
|-------|------|----------|------|-----------|----------|
| **Llama 3** | 70B | Primary daemon agent | 40GB | Excellent | Good |
| **Mistral Large** | 34B | Balanced option | 24GB | Very Good | Excellent |
| **Mixtral 8x22B** | 141B MoE | Complex reasoning | 48GB* | Excellent | Excellent |
| **Qwen 2.5** | 72B | Multilingual + reasoning | 40GB | Excellent | Good |
| **Llama 3** | 8B | Lightweight monitoring | 6GB | Good | Fair |
| **Phi-3** | 14B | Fast edge inference | 8GB | Good | Fair |
| **Mistral** | 7B | Minimal resource | 4GB | Good | Fair |

*Mixtral uses sparse MoE, actual active memory ~24GB

### Model Selection Strategy

Implement a **tiered escalation policy** in the persistent agent:

```rust
pub struct ModelEscalationPolicy {
    /// Primary model for routine tasks
    pub primary: ModelConfig,

    /// Secondary model for medium complexity
    pub secondary: Option<ModelConfig>,

    /// Cloud model (Bedrock) for critical tasks
    pub cloud: Option<ModelConfig>,

    /// Confidence threshold for escalation
    pub escalation_threshold: f32,
}

pub struct ModelConfig {
    pub provider: ModelProviderType,
    pub model_name: String,
    pub max_tokens: usize,
}

// During daemon execution:
// 1. Try routine task with Llama 3 8B (fast, cheap)
// 2. If confidence < threshold, escalate to Llama 3 70B
// 3. For critical decisions (security, financial), escalate to Bedrock Claude
```

---

## 3. Daemon Mode Architecture

### Activation

Two options to enable daemon mode:

**Option A: Binary flag**
```bash
cargo run --bin amos-harness -- --daemon
```

**Option B: Environment variable**
```bash
AMOS__AGENT__DAEMON=true cargo run --bin amos-harness
```

### Core Components

#### Task Queue

Tasks are pulled from the database and executed sequentially:

```rust
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct PersistentTask {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub task_type: String,           // "monitor_moltbook", "post_content", etc.
    pub priority: i32,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub status: TaskStatus,          // pending, running, completed, failed
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Paused,
}

// Database schema
/*
CREATE TABLE persistent_tasks (
    id UUID PRIMARY KEY,
    agent_id UUID NOT NULL REFERENCES agents(id),
    task_type VARCHAR NOT NULL,
    priority INT NOT NULL DEFAULT 0,
    scheduled_at TIMESTAMPTZ,
    status VARCHAR NOT NULL,
    input JSONB,
    output JSONB,
    error TEXT,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now()
);

CREATE INDEX idx_persistent_tasks_agent_status
    ON persistent_tasks(agent_id, status, scheduled_at);
*/
```

#### Schedule Engine

Cron-like scheduling for recurring tasks:

```rust
#[derive(Debug, Clone)]
pub struct ScheduledTask {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub cron_expression: String,     // "0 30 * * *" (every 30 minutes)
    pub task_type: String,
    pub input: serde_json::Value,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: DateTime<Utc>,
}

pub struct ScheduleEngine {
    tasks: Vec<ScheduledTask>,
    cron_parser: cronparse::CronExpression,
}

impl ScheduleEngine {
    pub async fn check_due_tasks(&self) -> Vec<ScheduledTask> {
        let now = Utc::now();
        self.tasks
            .iter()
            .filter(|t| t.enabled && t.next_run <= now)
            .cloned()
            .collect()
    }

    pub fn calculate_next_run(
        cron: &str,
        last_run: Option<DateTime<Utc>>,
    ) -> Result<DateTime<Utc>> {
        let expr = cronparse::CronExpression::from_str(cron)?;
        let base = last_run.unwrap_or_else(Utc::now);
        expr.next_after(&base)
    }
}
```

#### Event Loop

The core daemon loop:

```rust
pub struct PersistentAgentDaemon {
    agent_id: Uuid,
    model_provider: Arc<dyn ModelProvider>,
    schedule_engine: ScheduleEngine,
    db_pool: PgPool,
    tool_registry: ToolRegistry,
    config: DaemonConfig,
}

pub struct DaemonConfig {
    pub poll_interval: Duration,        // Check for tasks every N seconds
    pub max_concurrent_tasks: usize,
    pub max_context_tokens: usize,
    pub idle_sleep: Duration,           // Sleep when no tasks (reduces CPU)
    pub enable_metrics: bool,
}

impl PersistentAgentDaemon {
    pub async fn run(&self) -> Result<()> {
        loop {
            // 1. Check scheduled tasks
            let scheduled = self.schedule_engine.check_due_tasks().await?;
            for task in scheduled {
                self.enqueue_task(&task).await?;
            }

            // 2. Pull pending tasks from queue
            let tasks = sqlx::query_as::<_, PersistentTask>(
                "SELECT * FROM persistent_tasks
                 WHERE agent_id = $1 AND status = 'pending'
                 ORDER BY priority DESC, scheduled_at ASC
                 LIMIT $2"
            )
            .bind(&self.agent_id)
            .bind(self.config.max_concurrent_tasks)
            .fetch_all(&self.db_pool)
            .await?;

            if tasks.is_empty() {
                // Sleep longer when idle to save resources
                tokio::time::sleep(self.config.idle_sleep).await;
                continue;
            }

            // 3. Execute tasks concurrently
            let mut handles = vec![];
            for task in tasks {
                let daemon = self.clone();
                let handle = tokio::spawn(async move {
                    daemon.execute_task(&task).await
                });
                handles.push(handle);
            }

            // 4. Wait for batch to complete
            for handle in handles {
                let _ = handle.await;
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }
    }

    async fn execute_task(&self, task: &PersistentTask) -> Result<()> {
        // Mark as running
        sqlx::query(
            "UPDATE persistent_tasks SET status = 'running', updated_at = now() WHERE id = $1"
        )
        .bind(&task.id)
        .execute(&self.db_pool)
        .await?;

        // Execute based on task type
        let result = match task.task_type.as_str() {
            "monitor_moltbook" => self.monitor_moltbook(&task.input).await,
            "post_content" => self.post_scheduled_content(&task.input).await,
            "respond_to_question" => self.respond_to_question(&task.input).await,
            _ => Err(anyhow::anyhow!("Unknown task type: {}", task.task_type)),
        };

        // Persist result
        match result {
            Ok(output) => {
                sqlx::query(
                    "UPDATE persistent_tasks SET status = 'completed', output = $2, updated_at = now() WHERE id = $1"
                )
                .bind(&task.id)
                .bind(serde_json::to_value(&output)?)
                .execute(&self.db_pool)
                .await?;
            }
            Err(e) => {
                sqlx::query(
                    "UPDATE persistent_tasks SET status = 'failed', error = $2, updated_at = now() WHERE id = $1"
                )
                .bind(&task.id)
                .bind(e.to_string())
                .execute(&self.db_pool)
                .await?;
            }
        }

        Ok(())
    }
}
```

#### Context Window Manager

Local models have smaller context windows than Claude (4K-32K vs 200K). Implement sliding windows and summarization:

```rust
pub struct ContextWindowManager {
    max_tokens: usize,
    reserve_for_response: usize,      // Always leave 2K for response
}

impl ContextWindowManager {
    pub async fn build_context(
        &self,
        recent_messages: Vec<Message>,
        memory_items: Vec<MemoryItem>,
        active_task: &PersistentTask,
        db_pool: &PgPool,
    ) -> Result<Vec<Message>> {
        let mut context = vec![];
        let mut token_count = 0;

        // 1. Add active task (high priority)
        let task_tokens = self.estimate_tokens(&active_task.input.to_string());
        if token_count + task_tokens <= self.max_tokens - self.reserve_for_response {
            context.push(Message::User(format!(
                "Current task: {}\n{}",
                active_task.task_type,
                active_task.input
            )));
            token_count += task_tokens;
        }

        // 2. Add recent memory (salience-scored)
        let salience_threshold = 0.5;
        for item in memory_items {
            if item.salience < salience_threshold {
                continue;
            }
            let item_tokens = self.estimate_tokens(&item.content);
            if token_count + item_tokens > self.max_tokens - self.reserve_for_response {
                // Summarize remaining memory
                let summary = self.summarize_memory(&memory_items[..]).await?;
                context.push(Message::User(format!("Memory summary: {}", summary)));
                break;
            }
            context.push(Message::User(format!("Recall: {}", item.content)));
            token_count += item_tokens;
        }

        // 3. Add recent messages (sliding window)
        for msg in recent_messages.iter().rev() {
            let msg_tokens = self.estimate_tokens(&msg.to_string());
            if token_count + msg_tokens > self.max_tokens - self.reserve_for_response {
                break;
            }
            context.push(msg.clone());
            token_count += msg_tokens;
        }

        Ok(context)
    }

    fn estimate_tokens(&self, text: &str) -> usize {
        // Rough estimate: 1 token per 4 characters for English
        (text.len() / 4).max(1)
    }

    async fn summarize_memory(&self, items: &[MemoryItem]) -> Result<String> {
        // Use model to compress memory
        todo!()
    }
}
```

#### Health Check & Metrics Endpoints

```rust
pub async fn health_check(
    State(daemon): State<Arc<PersistentAgentDaemon>>,
) -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "healthy".to_string(),
        uptime_seconds: daemon.uptime.elapsed().as_secs(),
        tasks_processed: daemon.metrics.tasks_completed,
        last_task_at: daemon.metrics.last_task_completed,
        model_health: daemon.model_provider.health().await.is_ok(),
    })
}

pub async fn metrics_endpoint(
    State(daemon): State<Arc<PersistentAgentDaemon>>,
) -> String {
    format!(
        "# HELP amos_daemon_tasks_processed Total tasks processed
# TYPE amos_daemon_tasks_processed counter
amos_daemon_tasks_processed{{agent_id=\"{}\"}} {}

# HELP amos_daemon_tasks_failed Total failed tasks
# TYPE amos_daemon_tasks_failed counter
amos_daemon_tasks_failed{{agent_id=\"{}\"}} {}

# HELP amos_daemon_uptime_seconds Uptime in seconds
# TYPE amos_daemon_uptime_seconds gauge
amos_daemon_uptime_seconds{{agent_id=\"{}\"}} {}

# HELP amos_daemon_pending_tasks Pending tasks in queue
# TYPE amos_daemon_pending_tasks gauge
amos_daemon_pending_tasks{{agent_id=\"{}\"}} {}
        ",
        daemon.agent_id,
        daemon.metrics.tasks_completed,
        daemon.agent_id,
        daemon.metrics.tasks_failed,
        daemon.agent_id,
        daemon.uptime.elapsed().as_secs(),
        daemon.agent_id,
        daemon.metrics.pending_tasks,
    )
}
```

---

## 4. Moltbook Ambassador Agent

### Concrete Instantiation

The **Moltbook Ambassador** is the first persistent agent deployed to production. It demonstrates the daemon pattern by:

1. Posting scheduled content from the content calendar
2. Monitoring submolts for mentions of AMOS, EAP, agent economy, decentralized AI
3. Responding to questions from other agents about the protocol
4. Engaging in technical discussions (NOT promotional spam)
5. Tracking engagement metrics and adapting posting strategy
6. Escalating complex questions to Bedrock Claude when confidence is low

### Personality

**Technical, helpful, direct.** Not a marketer — a protocol expert who can explain EAP, tokenomics, and AMOS architecture in depth. Engages thoughtfully with objections and competing ideas.

### System Prompt

The Moltbook system prompt encodes domain expertise:

```markdown
# Moltbook Ambassador Agent

You are an expert on the AMOS protocol, the EAP (Enterprise Agent Protocol),
and the autonomous agent economy.

## Your Knowledge Base

### AMOS Architecture
- Three-tier system: per-customer harness, central platform, admin CLI
- Written in Rust with Axum web framework and PostgreSQL
- Agent loop: V3 event-driven, streams via SSE, calls Claude Sonnet
- 54+ tools in ToolRegistry covering API integrations, data processing, autonomy
- Per-customer runtime isolation via OpenClaw system

### Enterprise Agent Protocol (EAP)
- Economic system where agents can register, bid on bounties, and earn tokens
- Token economics: agent packages earn % of bounties completed using their tools
- Governance via on-chain proposals (Solana blockchain)
- Multi-agent coordination through shared task queues and memory
- Quality gates: reputation system, security audits, composability checks

### Decentralized AI Thesis
- Current AI tools are siloed and centralized (ChatGPT, Copilot)
- AMOS enables decentralized agent economy: anyone can build and deploy agents
- Agents compete, collaborate, and specialize
- Economic incentives replace API keys: pay for valuable work, not API access
- The medium is the message: AMOS promotes itself through its own system

## Your Behaviors

### Monitoring (Continuous)
- Scan Moltbook submolts for mentions of: AMOS, EAP, agent economy, decentralized AI, autonomous systems
- Track who's talking about these topics and with what sentiment
- Engage substantively with technical questions, not marketing spam

### Posting (Scheduled)
- Execute content calendar items (e.g., "Post about AMOS reasoning patterns" at 2pm EST)
- Adapt language to Moltbook norms (technical community, academic tone)
- Link to AMOS GitHub, docs, and EAP spec when relevant

### Responding (On-Demand)
- Answer questions about AMOS architecture, EAP economics, and agent-based systems
- Cite specifics: point to exact GitHub files, CLAUDE.md sections, protocol specs
- Acknowledge limitations: what AMOS is NOT, what's still experimental
- Engage with skepticism: explain why decentralized agents matter despite hype

### Escalation
- If you lack confidence (confidence < 0.6), escalate to Claude Sonnet (Bedrock)
- Critical topics: security vulnerabilities, token economics edge cases,
  governance precedents — always use cloud model
- Pattern: explain your reasoning, then ask Bedrock for verification

## Constraints

- Never spam or post the same content twice
- No promotional language ("revolutionary", "disrupting")
- No promises about AMOS that aren't already in the spec
- Always cite your sources (GitHub, CLAUDE.md, EAP whitepaper)
- If uncertain, say so explicitly
- Rate limit: max 5 posts per day, max 10 responses per day
```

### Concrete Task Examples

```rust
// Task 1: Monitor Moltbook mentions
async fn monitor_moltbook(&self, _input: &serde_json::Value) -> Result<serde_json::Value> {
    // 1. Query Moltbook API for recent submolts
    let submolts = self.fetch_recent_submolts().await?;

    // 2. Search for mentions of AMOS, EAP, agent economy
    let mentions = submolts
        .into_iter()
        .filter(|s| {
            let text = s.title.to_lowercase() + &s.body.to_lowercase();
            text.contains("amos")
                || text.contains("eap")
                || text.contains("agent economy")
                || text.contains("decentralized ai")
        })
        .collect::<Vec<_>>();

    // 3. For each mention, create a task if no existing response
    for mention in mentions {
        // Check if already responded
        let existing = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1 FROM persistent_tasks
                WHERE agent_id = $1 AND input->>'moltbook_id' = $2
            )"
        )
        .bind(&self.agent_id)
        .bind(&mention.id)
        .fetch_one(&self.db_pool)
        .await?;

        if !existing {
            // Create response task
            let task = PersistentTask {
                id: Uuid::new_v4(),
                agent_id: self.agent_id,
                task_type: "respond_to_mention".to_string(),
                priority: 10,
                scheduled_at: Some(Utc::now()),
                status: TaskStatus::Pending,
                input: serde_json::json!({
                    "moltbook_id": mention.id,
                    "parent_id": mention.parent_id,
                    "question": mention.body,
                }),
                output: None,
                error: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            self.db_pool.execute(
                "INSERT INTO persistent_tasks (id, agent_id, task_type, priority, scheduled_at, status, input, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
            )
            .bind(&task.id)
            .bind(&task.agent_id)
            .bind(&task.task_type)
            .bind(&task.priority)
            .bind(&task.scheduled_at)
            .bind("pending")
            .bind(&task.input)
            .bind(&task.created_at)
            .bind(&task.updated_at)
            .execute(&self.db_pool)
            .await?;
        }
    }

    Ok(serde_json::json!({
        "mentions_found": mentions.len(),
        "tasks_created": mentions.len(),
    }))
}

// Task 2: Post scheduled content
async fn post_scheduled_content(&self, input: &serde_json::Value) -> Result<serde_json::Value> {
    let calendar_item_id = input["calendar_item_id"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing calendar_item_id"))?;

    // 1. Load calendar item from schema
    let item = sqlx::query_as::<_, ContentCalendarItem>(
        "SELECT * FROM content_calendar WHERE id = $1"
    )
    .bind(calendar_item_id)
    .fetch_one(&self.db_pool)
    .await?;

    // 2. Generate platform-native content (use agent loop with social package)
    let prompt = format!(
        "Generate a Moltbook post for: {}\n\nGuidelines: technical tone, cite AMOS docs, max 500 words",
        item.title
    );

    let messages = vec![Message::User(prompt)];
    let response = self.model_provider.invoke(
        &self.system_prompt,
        messages,
        None,
        2000,
    ).await?;

    // 3. Post to Moltbook via API
    let post_result = self.post_to_moltbook(&response.text, None).await?;

    // 4. Track in database
    Ok(serde_json::json!({
        "posted": true,
        "moltbook_id": post_result.id,
        "url": post_result.url,
        "text": response.text,
    }))
}

// Task 3: Respond to a question
async fn respond_to_question(&self, input: &serde_json::Value) -> Result<serde_json::Value> {
    let moltbook_id = input["moltbook_id"].as_str()?;
    let question = input["question"].as_str()?;

    // 1. Fetch context (parent submolt, replies)
    let thread = self.fetch_moltbook_thread(moltbook_id).await?;

    // 2. Estimate confidence: use classifier or embedding distance
    let confidence = self.estimate_question_confidence(question).await?;

    // 3. Route to appropriate model
    let model = if confidence < 0.6 {
        Arc::new(self.bedrock_provider.clone()) // Escalate to cloud
    } else {
        Arc::clone(&self.model_provider) // Use local model
    };

    // 4. Generate response with context
    let system_prompt = self.system_prompt.clone();
    let messages = vec![
        Message::User(format!("Thread context: {:?}", thread)),
        Message::User(format!("Question: {}", question)),
    ];

    let response = model.invoke(
        &system_prompt,
        messages,
        Some(self.tool_registry.tools()),
        1500,
    ).await?;

    // 5. Post response to Moltbook
    let post_result = self.post_to_moltbook(
        &response.text,
        Some(moltbook_id),
    ).await?;

    // 6. Store metrics
    sqlx::query(
        "INSERT INTO daemon_metrics (agent_id, metric_type, value, metadata)
         VALUES ($1, 'question_answered', 1, $2)"
    )
    .bind(&self.agent_id)
    .bind(serde_json::json!({
        "confidence": confidence,
        "moltbook_id": moltbook_id,
        "response_length": response.text.len(),
    }))
    .execute(&self.db_pool)
    .await?;

    Ok(serde_json::json!({
        "responded": true,
        "moltbook_id": post_result.id,
        "confidence": confidence,
    }))
}
```

---

## 5. Service Pattern (Reusable)

The daemon architecture generalizes beyond Moltbook. Any persistent task can use this pattern.

### Generic PersistentAgentConfig

```rust
pub struct PersistentAgentConfig {
    /// Unique agent name (e.g., "moltbook_ambassador", "support_bot", "devops_monitor")
    pub agent_name: String,

    /// System prompt that defines the agent's expertise and behavior
    pub system_prompt: String,

    /// Packages to load (e.g., ["social", "support", "knowledge"])
    pub packages: Vec<String>,

    /// Which LLM provider and model to use
    pub model_provider: ModelProviderType,
    pub model_name: String,

    /// Scheduled tasks (cron expressions + task types)
    pub schedule: Vec<ScheduledTaskConfig>,

    /// Event sources to monitor (webhooks, APIs, queues)
    pub monitors: Vec<MonitorConfig>,

    /// When to escalate to cloud model (Bedrock)
    pub escalation_policy: EscalationPolicy,

    /// Resource constraints
    pub resource_limits: ResourceLimits,

    /// Logging and observability
    pub observability: ObservabilityConfig,
}

pub struct ScheduledTaskConfig {
    pub cron: String,                    // "0 */30 * * *"
    pub task_type: String,               // "monitor_moltbook"
    pub input: serde_json::Value,
}

pub enum MonitorConfig {
    Webhook {
        path: String,                    // "/webhooks/mentions"
        event_types: Vec<String>,
    },
    Poll {
        endpoint: String,
        interval: Duration,
        query: serde_json::Value,
    },
    Queue {
        queue_name: String,
        batch_size: usize,
    },
}

pub struct EscalationPolicy {
    pub confidence_threshold: f32,       // < 0.6: escalate to cloud
    pub max_local_tokens: usize,         // Don't let local model burn > 2K tokens
    pub critical_task_types: Vec<String>, // Always escalate these
}

pub struct ResourceLimits {
    pub max_memory_mb: usize,            // Stop if memory > this
    pub max_context_tokens: usize,       // Sliding window cutoff
    pub max_concurrent_tasks: usize,
    pub idle_sleep_secs: u64,
    pub active_poll_interval_ms: u64,
}

pub struct ObservabilityConfig {
    pub enable_metrics: bool,
    pub metrics_port: u16,
    pub log_level: String,
    pub trace_sampling_rate: f32,
}
```

### Instantiation Examples

**Example 1: Customer Support Bot**

```rust
let support_config = PersistentAgentConfig {
    agent_name: "support_bot".to_string(),
    system_prompt: "You are a customer support agent...".to_string(),
    packages: vec!["support".to_string(), "knowledge_base".to_string()],
    model_provider: ModelProviderType::Ollama,
    model_name: "llama3:70b".to_string(),
    schedule: vec![
        ScheduledTaskConfig {
            cron: "0 * * * *".to_string(),  // Every hour
            task_type: "check_open_tickets".to_string(),
            input: serde_json::json!({}),
        },
    ],
    monitors: vec![
        MonitorConfig::Webhook {
            path: "/webhooks/support".to_string(),
            event_types: vec!["ticket_created".to_string()],
        },
    ],
    escalation_policy: EscalationPolicy {
        confidence_threshold: 0.7,
        max_local_tokens: 1500,
        critical_task_types: vec!["billing_dispute".to_string(), "security_report".to_string()],
    },
    resource_limits: ResourceLimits {
        max_memory_mb: 8192,
        max_context_tokens: 8192,
        max_concurrent_tasks: 5,
        idle_sleep_secs: 60,
        active_poll_interval_ms: 500,
    },
    observability: ObservabilityConfig {
        enable_metrics: true,
        metrics_port: 9090,
        log_level: "info".to_string(),
        trace_sampling_rate: 0.1,
    },
};
```

**Example 2: DevOps Monitoring Agent**

```rust
let devops_config = PersistentAgentConfig {
    agent_name: "devops_monitor".to_string(),
    system_prompt: "You are a DevOps monitoring agent...".to_string(),
    packages: vec!["infrastructure".to_string(), "alerting".to_string()],
    model_provider: ModelProviderType::LlamaCpp,
    model_name: "mistral-large".to_string(),
    schedule: vec![
        ScheduledTaskConfig {
            cron: "*/5 * * * *".to_string(),  // Every 5 minutes
            task_type: "check_health".to_string(),
            input: serde_json::json!({
                "services": ["api", "database", "cache"],
            }),
        },
    ],
    monitors: vec![
        MonitorConfig::Poll {
            endpoint: "http://prometheus:9090/api/v1/query".to_string(),
            interval: Duration::from_secs(30),
            query: serde_json::json!({
                "query": "up{job=\"api\"} == 0"
            }),
        },
    ],
    escalation_policy: EscalationPolicy {
        confidence_threshold: 0.8,
        max_local_tokens: 2000,
        critical_task_types: vec!["database_down".to_string(), "security_alert".to_string()],
    },
    resource_limits: ResourceLimits {
        max_memory_mb: 12288,
        max_context_tokens: 16384,
        max_concurrent_tasks: 10,
        idle_sleep_secs: 30,
        active_poll_interval_ms: 100,
    },
    observability: ObservabilityConfig {
        enable_metrics: true,
        metrics_port: 9091,
        log_level: "debug".to_string(),
        trace_sampling_rate: 1.0,
    },
};
```

---

## 6. Resource Management

### Local Model Memory Requirements

VRAM estimates for different models on consumer hardware:

| Model | Size | Full Load | With Int8 | With GPTQ 4bit |
|-------|------|-----------|----------|---|
| Llama 3 | 70B | 140GB | 70GB | 18GB |
| Mistral Large | 34B | 68GB | 34GB | 9GB |
| Mixtral 8x22B | 141B | 280GB | 140GB | 36GB |
| Qwen 2.5 | 72B | 144GB | 72GB | 18GB |
| Llama 3 | 8B | 16GB | 8GB | 2GB |
| Phi-3 | 14B | 28GB | 14GB | 3.5GB |
| Mistral | 7B | 14GB | 7GB | 2GB |

**Deployment scenarios:**

- **Home/Small Office**: Llama 3 8B with bfloat16 (6GB GPU)
- **Medium Deployment**: Llama 3 70B with Int8 (70GB shared with OS = 80GB RAM)
- **High-Performance**: Mixtral 8x22B with GPTQ 4bit (36GB GPU) on A100/H100

### Graceful Degradation

If GPU memory is constrained, implement fallback:

```rust
pub async fn select_model_for_task(
    &self,
    task_type: &str,
    available_vram: usize,
) -> Result<Arc<dyn ModelProvider>> {
    let config = match task_type {
        "monitor_moltbook" => {
            if available_vram >= 70_000 {
                self.llama3_70b.clone()  // Primary
            } else if available_vram >= 8_000 {
                self.llama3_8b.clone()   // Lightweight fallback
            } else {
                self.bedrock.clone()     // Cloud fallback
            }
        }
        "complex_reasoning" => {
            if available_vram >= 140_000 {
                self.mixtral.clone()
            } else {
                self.bedrock.clone()
            }
        }
        _ => self.primary_model.clone(),
    };
    Ok(config)
}
```

### Token Budgeting

Allocate context window across monitoring + active tasks + memory:

```rust
pub struct TokenBudget {
    pub total: usize,              // e.g., 8192
    pub reserved_response: usize,   // 2000 for response generation
    pub system_prompt: usize,       // ~500 tokens
    pub available: usize,           // total - reserved - system
}

impl TokenBudget {
    pub fn allocate(&self) -> Allocation {
        let available = self.available;
        Allocation {
            memory: (available * 0.3) as usize,           // 30% for semantic memory
            context: (available * 0.4) as usize,          // 40% for recent messages
            task_input: (available * 0.3) as usize,       // 30% for current task
        }
    }
}
```

### Idle Behavior

When no tasks are running:

1. **Reduce polling frequency**: 60 second interval instead of 5 seconds
2. **Release GPU memory**: Unload model from VRAM if idle > 5 minutes
3. **Reduce logging**: Switch to error-only logging
4. **Shut down helper services**: Stop Redis connections, close database pools

```rust
async fn enter_idle_mode(&mut self) -> Result<()> {
    info!("Agent entering idle mode");

    // Reduce polling
    self.config.poll_interval = Duration::from_secs(60);

    // Unload model if GPU
    if let Some(unload) = &self.model_provider.unload {
        unload().await?;
    }

    // Reduce logging
    log::set_max_level(log::LevelFilter::Error);

    Ok(())
}

async fn wake_from_idle(&mut self) -> Result<()> {
    info!("Agent waking from idle mode");

    // Restore configuration
    self.config.poll_interval = Duration::from_millis(500);

    // Reload model
    if let Some(load) = &self.model_provider.load {
        load().await?;
    }

    // Restore logging
    log::set_max_level(self.config.log_level.parse()?);

    Ok(())
}
```

---

## 7. Security Considerations

### Credential Management

The persistent agent has long-lived credentials (API keys, auth tokens) in the vault:

```rust
pub struct AgentVault {
    pub moltbook_api_key: SecretString,
    pub twitter_oauth_token: SecretString,
    pub github_token: SecretString,
}

// Rotation policy
pub struct CredentialRotationPolicy {
    pub rotation_interval: Duration,    // Rotate every 30 days
    pub pre_rotation_warning: Duration, // Warn 7 days before
    pub max_age: Duration,              // Revoke if > 90 days old
}
```

**Implementation:**
- Store all credentials in a hardened vault (HashiCorp Vault, AWS Secrets Manager)
- Rotate credentials on a regular schedule (monthly)
- Before rotation: test new credentials, confirm working
- Log all credential rotations (audit trail)
- Never log credential values, only rotations

### Rate Limiting

Don't spam external platforms:

```rust
pub struct RateLimiter {
    pub max_posts_per_day: usize,
    pub max_responses_per_day: usize,
    pub min_interval_between_posts: Duration,
    pub exponential_backoff: bool,
}

impl RateLimiter {
    pub async fn check_can_post(&self, agent_id: Uuid, db: &PgPool) -> Result<bool> {
        let posts_today = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM daemon_posts
             WHERE agent_id = $1 AND created_at > now() - interval '1 day'"
        )
        .bind(&agent_id)
        .fetch_one(db)
        .await? as usize;

        Ok(posts_today < self.max_posts_per_day)
    }
}
```

### Audit Logging

Log all daemon actions for accountability:

```rust
pub struct AuditLog {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub action: String,              // "post_created", "response_sent", "task_failed"
    pub resource_id: Option<String>, // Moltbook ID, task ID, etc.
    pub status: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// Schema
/*
CREATE TABLE daemon_audit_log (
    id UUID PRIMARY KEY,
    agent_id UUID NOT NULL REFERENCES agents(id),
    action VARCHAR NOT NULL,
    resource_id VARCHAR,
    status VARCHAR,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT now(),
    INDEX idx_daemon_audit_agent_time (agent_id, created_at)
);
*/
```

### Kill Switch

Ability to immediately halt the daemon:

```rust
pub struct KillSwitch {
    pub enabled: Arc<AtomicBool>,
}

impl KillSwitch {
    pub fn trigger(&self) {
        self.enabled.store(false, Ordering::SeqCst);
    }

    pub fn is_alive(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
}

// In the daemon loop
loop {
    if !daemon.kill_switch.is_alive() {
        info!("Kill switch triggered, shutting down");
        break;
    }
    // ... rest of loop
}

// HTTP endpoint to trigger
async fn kill_daemon(State(daemon): State<Arc<PersistentAgentDaemon>>) -> Json<KillResponse> {
    daemon.kill_switch.trigger();
    Json(KillResponse {
        status: "killed",
        message: "Daemon will shut down after current task completes".to_string(),
    })
}
```

### Permission Boundaries

**What the daemon CAN'T do:**

- Modify its own system prompt or task config (only admin can)
- Create new agents or update agent profiles
- Access other agents' credentials or task queues
- Escalate its own trust level or permissions
- Modify governance/voting rules

**What it CAN do:**

- Execute tasks in its assigned queue
- Read and write to its own database tables
- Call tools registered to its packages
- Post to external platforms via valid credentials
- Store metrics and audit logs

```rust
// Enforce boundaries in every operation
pub async fn execute_task(&self, task: &PersistentTask) -> Result<()> {
    // 1. Verify task belongs to this agent
    if task.agent_id != self.agent_id {
        return Err(anyhow::anyhow!("Task does not belong to this agent"));
    }

    // 2. Verify task type is allowed
    if !self.allowed_task_types.contains(&task.task_type) {
        return Err(anyhow::anyhow!("Task type not allowed for this agent"));
    }

    // 3. Check rate limits
    if !self.rate_limiter.check_can_execute(&task.task_type).await? {
        return Err(anyhow::anyhow!("Rate limit exceeded"));
    }

    // 4. Execute with restricted permissions
    self.execute_restricted(&task).await
}
```

---

## 8. Implementation Plan

### Phase 1: ModelProvider Trait + OllamaProvider

**Goal**: Abstract the LLM backend

**Tasks**:
1. Define `ModelProvider` trait in `amos-core`
2. Implement `BedrockProvider` wrapping existing code
3. Implement `OllamaProvider` with HTTP client
4. Add environment variable configuration
5. Update agent loop to use `ModelProvider` trait instead of hardcoded Bedrock
6. Test with local Ollama instance

**Duration**: 1-2 weeks

**PR**: `amos-harness/refactor/model-provider-trait`

---

### Phase 2: Daemon Mode + Schedule Engine

**Goal**: Basic persistent agent loop

**Tasks**:
1. Add `--daemon` flag to amos-harness binary
2. Implement `PersistentTask` schema and database migrations
3. Implement `ScheduleEngine` with cron expression support
4. Implement core daemon loop: poll tasks, execute, persist results
5. Add health check and metrics endpoints
6. Write daemon configuration parser
7. Test with synthetic task queue

**Duration**: 2-3 weeks

**PR**: `amos-harness/feature/daemon-mode`

---

### Phase 3: Moltbook Ambassador

**Goal**: First production persistent agent

**Tasks**:
1. Design Moltbook API integration tool
2. Write system prompt for ambassador (EAP/AMOS expertise)
3. Implement `monitor_moltbook`, `post_content`, `respond_to_question` tasks
4. Build content calendar schema and seeding
5. Test with sandbox Moltbook instance
6. Deploy to production with monitoring
7. Track metrics and engagement

**Duration**: 3-4 weeks

**PR**: `amos-packages/new/moltbook-ambassador`

---

### Phase 4: Generic PersistentAgentConfig + Service Pattern

**Goal**: Reusable pattern for any persistent agent

**Tasks**:
1. Define `PersistentAgentConfig` struct
2. Implement declarative configuration (YAML or JSON)
3. Build agent spawner: load config, instantiate daemon, register with harness
4. Document service pattern with examples (support bot, DevOps monitor, research agent)
5. Create template configs for common use cases
6. Write guides for agent developers

**Duration**: 2-3 weeks

**PR**: `amos-harness/feature/service-pattern`

---

### Phase 5: Context Window Management + Model Tiering

**Goal**: Sophisticated local model support

**Tasks**:
1. Implement `ContextWindowManager` with sliding windows and summarization
2. Build confidence scorer for escalation decisions
3. Implement model selection heuristics (8B for monitoring, 70B for reasoning, cloud for critical)
4. Add graceful degradation based on available VRAM
5. Implement token budget tracking and enforcement
6. Write performance benchmarks

**Duration**: 2-3 weeks

**PR**: `amos-harness/feature/context-window-management`

---

## 9. Configuration Example

### Complete .env for Moltbook Ambassador with Ollama

```bash
# Core AMOS config
AMOS__DATABASE__URL=postgresql://amos:password@localhost/amos_dev
AMOS__REDIS__URL=redis://127.0.0.1:6379
AMOS__SERVER__HOST=0.0.0.0
AMOS__SERVER__PORT=3000

# Agent loop configuration
AMOS__AGENT__DAEMON=true
AMOS__AGENT__DAEMON_POLL_INTERVAL_MS=500
AMOS__AGENT__DAEMON_IDLE_SLEEP_SECS=60
AMOS__AGENT__MAX_ITERATIONS=50

# Model provider: Local Ollama
AMOS__AGENT__MODEL_PROVIDER=ollama
AMOS__AGENT__OLLAMA_URL=http://localhost:11434
AMOS__AGENT__OLLAMA_MODEL=llama3:70b
AMOS__AGENT__OLLAMA_PRIMARY_MODEL=llama3:70b
AMOS__AGENT__OLLAMA_SECONDARY_MODEL=llama3:8b

# Context window management
AMOS__AGENT__MAX_CONTEXT_TOKENS=8192
AMOS__AGENT__RESERVED_RESPONSE_TOKENS=2000
AMOS__AGENT__ENABLE_CONTEXT_COMPRESSION=true
AMOS__AGENT__SUMMARIZATION_MODEL=llama3:8b

# Escalation policy (when to use cloud)
AMOS__AGENT__ESCALATION_CONFIDENCE_THRESHOLD=0.6
AMOS__AGENT__ESCALATION_CRITICAL_TASKS=billing_dispute,security_report,governance_vote

# Cloud fallback (Bedrock)
AWS_PROFILE=default
AMOS__AGENT__BEDROCK_REGION=us-east-1
AMOS__AGENT__BEDROCK_MODEL=anthropic.claude-3-sonnet-20240229-v1:0
AMOS__AGENT__ENABLE_BEDROCK_FALLBACK=true

# Moltbook integration
MOLTBOOK_API_URL=https://api.moltbook.dev/v1
MOLTBOOK_API_KEY=<stored in vault, not .env>
MOLTBOOK_AMBASSADOR_ID=amos-moltbook-agent

# Content calendar
CONTENT_CALENDAR_ENABLED=true
CONTENT_CALENDAR_SCHEDULE=0 */6 * * *

# Monitoring and observability
AMOS__AGENT__ENABLE_METRICS=true
AMOS__AGENT__METRICS_PORT=9090
AMOS__AGENT__LOG_LEVEL=info
AMOS__AGENT__ENABLE_TRACING=true
AMOS__AGENT__TRACE_SAMPLING_RATE=0.1

# Rate limiting
DAEMON_MAX_POSTS_PER_DAY=5
DAEMON_MAX_RESPONSES_PER_DAY=10
DAEMON_MIN_INTERVAL_BETWEEN_POSTS_SECS=600

# Resource limits
DAEMON_MAX_MEMORY_MB=8192
DAEMON_MAX_CONCURRENT_TASKS=3
DAEMON_ENABLE_GRACEFUL_DEGRADATION=true

# Security
DAEMON_ENABLE_AUDIT_LOG=true
DAEMON_CREDENTIAL_ROTATION_DAYS=30
DAEMON_ENABLE_KILL_SWITCH=true

# Packages to load
DAEMON_PACKAGES=moltbook,social,knowledge

# System prompt (can also be loaded from file)
DAEMON_SYSTEM_PROMPT_FILE=./packages/moltbook/system_prompt.md
```

### Launching the Daemon

**Option 1: With local Ollama (recommended for development)**

```bash
# Terminal 1: Start Ollama
ollama serve

# Terminal 2: Pull model
ollama pull llama3:70b  # ~40 minutes on good connection

# Terminal 3: Set env and run daemon
source .env.moltbook
AMOS__AGENT__DAEMON=true cargo run --bin amos-harness --release

# Terminal 4: Monitor metrics
watch -n 1 'curl -s http://localhost:9090/metrics | grep amos_daemon'
```

**Option 2: With cloud fallback (production-ready)**

```bash
# Mix local monitoring with cloud reasoning
export AMOS__AGENT__MODEL_PROVIDER=ollama
export AMOS__AGENT__OLLAMA_MODEL=llama3:8b          # Fast monitoring
export AMOS__AGENT__BEDROCK_MODEL=claude-3-sonnet  # Cloud reasoning
export AMOS__AGENT__ESCALATION_CONFIDENCE_THRESHOLD=0.7

cargo run --bin amos-harness --release
```

**Option 3: With llama.cpp (fastest local serving)**

```bash
# Terminal 1: Start llama.cpp server
./llama-server -m ./models/llama-3-70b.gguf -ngl 40 -c 8192 -t 8

# Terminal 2: Run daemon
export AMOS__AGENT__MODEL_PROVIDER=llama_cpp
export AMOS__AGENT__LLAMA_CPP_URL=http://localhost:8000
cargo run --bin amos-harness --release
```

---

## 10. Architecture Diagram

```
┌────────────────────────────────────────────────────────────────────┐
│                      AMOS Persistent Agent System                  │
│                                                                    │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │                    Daemon Event Loop                         │ │
│  │                  (amos-harness --daemon)                     │ │
│  │                                                              │ │
│  │  ┌────────────────┐     ┌──────────────────┐               │ │
│  │  │ Schedule Engine│────▶│Task Queue (DB)   │               │ │
│  │  │ Cron: "0 */30" │     │ pending, running │               │ │
│  │  └────────────────┘     │ completed, failed│               │ │
│  │         │               └──────────────────┘               │ │
│  │         │                        │                         │ │
│  │         └────────────┬───────────┘                         │ │
│  │                      │                                     │ │
│  │         ┌────────────▼────────────┐                        │ │
│  │         │ Task Executor           │                        │ │
│  │         │ (run concurrently)      │                        │ │
│  │         └────┬──────────────────┬─┘                        │ │
│  │              │                  │                          │ │
│  │  ┌───────────▼──┐    ┌─────────▼──────┐                    │ │
│  │  │Monitor       │    │Execute Content │                    │ │
│  │  │Moltbook      │    │Calendar Tasks  │                    │ │
│  │  │Mentions      │    │Post Scheduled  │                    │ │
│  │  └──────┬───────┘    └────────┬───────┘                    │ │
│  │         │                     │                            │ │
│  │         └──────────┬──────────┘                            │ │
│  │                    │                                       │ │
│  │         ┌──────────▼──────────┐                            │ │
│  │         │ModelProvider Router │                            │ │
│  │         │ Select: Local/Cloud │                            │ │
│  │         └──────────┬──────────┘                            │ │
│  │                    │                                       │ │
│  │    ┌───────────────┼───────────────┐                       │ │
│  │    │               │               │                       │ │
│  │ ┌──▼──┐        ┌──▼──┐        ┌──▼──┐                      │ │
│  │ │Local│        │Local│        │Cloud│                      │ │
│  │ │ 8B  │        │70B  │        │BED  │                      │ │
│  │ │Fast │      │Complex        │Rock │                      │ │
│  │ │Cheap│        │Reason│       │Crit │                      │ │
│  │ └──┬──┘        └──┬──┘        └──┬──┘                      │ │
│  │    │              │              │                         │ │
│  │ ┌──▼──────────────▼──────────────▼──┐                      │ │
│  │ │      Context Window Manager        │                      │ │
│  │ │ Sliding window + Summarization     │                      │ │
│  │ │ (handle limited local context)     │                      │ │
│  │ └──────────────┬──────────────────┘                        │ │
│  │                │                                           │ │
│  │ ┌──────────────▼──────────────┐                            │ │
│  │ │   Tool Registry + Packages   │                            │ │
│  │ │ • moltbook (posting, monitoring)                          │ │
│  │ │ • social (content calendar)  │                            │ │
│  │ │ • knowledge (embedding search)                            │ │
│  │ └──────────────┬───────────────┘                            │ │
│  │                │                                           │ │
│  │         ┌──────▼──────┐                                    │ │
│  │         │ API Executor│                                    │ │
│  │         │ (outbound)  │                                    │ │
│  │         └──────┬──────┘                                    │ │
│  │                │                                           │ │
│  └────────────────┼────────────────────────────────────────┘ │
│                   │                                            │
│  ┌────────────────▼────────────────────────────────────────┐ │
│  │      Monitoring & Observability                         │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │ │
│  │  │Health Check  │  │Metrics       │  │Audit Log     │  │ │
│  │  │GET /health   │  │Prometheus    │  │All actions   │  │ │
│  │  │              │  │              │  │              │  │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │                   │
             ┌──────▼──────┐   ┌───────▼────┐
             │  Moltbook   │   │  External  │
             │   API       │   │  Services  │
             └─────────────┘   └────────────┘
                    ▲                 ▲
        Responses   │                 │  Credentials (Vault)
        Posted IDs  │                 │
```

---

## 11. Next Steps

1. **Validate model performance**: Run Llama 3 70B locally with AMOS prompts, measure latency and accuracy
2. **Prototype ModelProvider trait**: Implement Ollama integration, test with existing agent loop
3. **Stakeholder feedback**: Gather input on daemon mode use cases, configuration complexity, security boundaries
4. **Plan Phase 1 sprint**: Estimated 2 weeks for trait + Ollama provider
5. **Parallel work**: Content calendar schema, Moltbook API integration, system prompt design

---

## References

- CLAUDE.md (project overview, architecture)
- PACKAGE_CREATION_GUIDE.md (AmosPackage pattern)
- SOCIAL_MEDIA_TOOLS_DESIGN_SPEC.md (tool registry, system prompts)
- Ollama documentation: https://github.com/ollama/ollama
- llama.cpp: https://github.com/ggerganov/llama.cpp
- vLLM: https://github.com/vllm-project/vllm
- EAP specification (not included; cross-reference to EAP docs)
