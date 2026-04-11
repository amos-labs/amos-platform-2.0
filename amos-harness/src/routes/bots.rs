//! OpenClaw agent management routes + External Agent Protocol (EAP) endpoints
//!
//! EAP endpoints allow the sidecar agent to register with the harness,
//! discover available tools, execute them, and send heartbeats.

use crate::{openclaw::AgentConfig, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    pub name: String,
    pub display_name: String,
    pub role: String,
    pub capabilities: Vec<String>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAgentRequest {
    pub role: Option<String>,
    pub capabilities: Option<Vec<String>>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
}

// ── EAP types ────────────────────────────────────────────────────────────

/// EAP registration request from the sidecar agent.
#[derive(Debug, Deserialize)]
pub struct EapRegisterRequest {
    pub name: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub agent_card_url: Option<String>,
    pub version: Option<String>,
}

/// EAP registration response — returns an agent ID, token, and available tools.
#[derive(Debug, Serialize)]
pub struct EapRegisterResponse {
    pub agent_id: String,
    pub token: String,
    pub harness_tools: Vec<EapHarnessTool>,
}

/// Tool descriptor sent to the agent during registration.
#[derive(Debug, Serialize)]
pub struct EapHarnessTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// EAP tool execution request from the agent.
#[derive(Debug, Deserialize)]
pub struct EapToolExecuteRequest {
    pub tool_name: String,
    pub input: serde_json::Value,
    pub task_id: Option<String>,
}

/// EAP tool execution response.
#[derive(Debug, Serialize)]
pub struct EapToolExecuteResponse {
    pub content: String,
    pub is_error: bool,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// EAP task response (returned from task polling).
#[derive(Debug, Serialize)]
pub struct EapTaskResponse {
    pub task_id: String,
    pub title: String,
    pub description: String,
    pub context: serde_json::Value,
    pub category: String,
    pub priority: i32,
    pub reward_tokens: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline_at: Option<String>,
    pub created_at: String,
}

/// EAP task result submission request.
#[derive(Debug, Deserialize)]
pub struct EapTaskResultRequest {
    pub status: String, // "completed" or "failed"
    pub result: serde_json::Value,
    pub error_message: Option<String>,
    pub execution_time_ms: Option<i64>,
    pub tools_used: Option<Vec<String>>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_agents).post(register_agent))
        // EAP endpoints (sidecar agent protocol)
        .route("/register", post(eap_register))
        .route("/{id}/heartbeat", post(eap_heartbeat))
        .route("/{id}/tools/execute", post(eap_tool_execute))
        .route("/{id}/tasks", get(eap_poll_tasks))
        // EAP task result submission
        .route("/{id}/tasks/{task_id}/result", post(eap_submit_task_result))
        // OpenClaw management endpoints
        .route("/{id}", get(get_agent).put(update_agent))
        .route("/{id}/activate", post(activate_agent))
        .route("/{id}/stop", post(stop_agent))
}

async fn list_agents(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AgentConfig>>, StatusCode> {
    let agents = state
        .agent_manager
        .list_agents()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(agents))
}

async fn register_agent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterAgentRequest>,
) -> Result<Json<AgentConfig>, StatusCode> {
    let agent = state
        .agent_manager
        .register_agent(
            req.name,
            req.display_name,
            req.role,
            req.capabilities,
            req.system_prompt,
            req.model,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(agent))
}

async fn get_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let status = state
        .agent_manager
        .get_status(id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(serde_json::json!({
        "agent_id": id,
        "status": status
    })))
}

async fn update_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
    Json(req): Json<UpdateAgentRequest>,
) -> Result<Json<AgentConfig>, StatusCode> {
    let updates = crate::openclaw::AgentConfigUpdate {
        role: req.role,
        capabilities: req.capabilities,
        system_prompt: req.system_prompt,
        model: req.model,
    };

    let agent = state
        .agent_manager
        .update_agent(id, updates)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(agent))
}

async fn activate_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    state
        .agent_manager
        .activate_agent(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

async fn stop_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    state
        .agent_manager
        .stop_agent(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

// ── EAP (External Agent Protocol) handlers ───────────────────────────────

/// `POST /api/v1/agents/register` — Agent registers and receives available tools.
async fn eap_register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EapRegisterRequest>,
) -> Result<Json<EapRegisterResponse>, StatusCode> {
    // Build the list of harness tools from the tool registry.
    // The harness ToolRegistry stores tools with Bedrock-style schemas;
    // we flatten them to the simple {name, description, input_schema} format.
    let harness_tools: Vec<EapHarnessTool> = state
        .tool_registry
        .list_tools()
        .iter()
        .filter_map(|name| {
            let tool = state.tool_registry.get(name)?;
            let schema = tool.parameters_schema();
            Some(EapHarnessTool {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: schema,
            })
        })
        .collect();

    // Insert the agent into external_agents so trust-level gating works.
    // The built-in sidecar ("amos-agent") gets trust level 5 (full access).
    // External agents registering via this endpoint start at level 1.
    let is_sidecar = req.name == "amos-agent";
    let trust_level: i16 = if is_sidecar { 5 } else { 1 };

    let agent_id: String = sqlx::query_scalar(
        "INSERT INTO external_agents (name, description, endpoint_url, trust_level, capabilities, status)
         VALUES ($1, $2, 'local://sidecar', $3, $4, 'active')
         ON CONFLICT (name) DO UPDATE SET
             trust_level = EXCLUDED.trust_level,
             last_seen_at = NOW(),
             status = 'active'
         RETURNING id::text",
    )
    .bind(&req.name)
    .bind(format!("Registered agent v{}", req.version.as_deref().unwrap_or("unknown")))
    .bind(trust_level)
    .bind(serde_json::json!(&req.capabilities))
    .fetch_one(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to register agent: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        agent = %req.name,
        agent_id = %agent_id,
        trust_level,
        version = ?req.version,
        tools = harness_tools.len(),
        "EAP agent registered, {} harness tools available",
        harness_tools.len()
    );

    Ok(Json(EapRegisterResponse {
        agent_id,
        token: "eap-internal".to_string(),
        harness_tools,
    }))
}

/// `POST /api/v1/agents/{id}/heartbeat` — Agent keepalive.
async fn eap_heartbeat(Path(_id): Path<String>) -> StatusCode {
    StatusCode::OK
}

/// `POST /api/v1/agents/{id}/tools/execute` — Execute a harness tool.
///
/// Enforces trust-level gating: agents must have sufficient trust level
/// to access tools in higher permission tiers.
async fn eap_tool_execute(
    State(state): State<Arc<AppState>>,
    Path(agent_id): Path<String>,
    Json(req): Json<EapToolExecuteRequest>,
) -> Result<Json<EapToolExecuteResponse>, StatusCode> {
    let start = std::time::Instant::now();

    info!(tool = %req.tool_name, agent = %agent_id, "EAP tool execution request");

    // Check trust level gating
    if let Some(tool) = state.tool_registry.get(&req.tool_name) {
        let required_trust = super::trust_level_for_category(tool.category());

        // Look up agent trust level from external_agents table.
        // The built-in sidecar agent registers via eap_register() which gives
        // it a random UUID that is NOT in external_agents. If the agent_id is
        // not found in the table, it's the sidecar and gets full access (level 5).
        // External agents that ARE in the table use their stored trust_level.
        let agent_trust: i16 = {
            sqlx::query_scalar::<_, i16>(
                "SELECT trust_level FROM external_agents WHERE id::text = $1 OR name = $1",
            )
            .bind(&agent_id)
            .fetch_optional(&state.db_pool)
            .await
            .ok()
            .flatten()
            .unwrap_or(1) // Unknown agent = minimum access
        };

        if (agent_trust as u8) < required_trust {
            return Ok(Json(EapToolExecuteResponse {
                content: format!(
                    "Insufficient trust level: tool '{}' requires level {}, agent has level {}",
                    req.tool_name, required_trust, agent_trust
                ),
                is_error: true,
                duration_ms: start.elapsed().as_millis() as u64,
                metadata: Some(serde_json::json!({
                    "error_code": "INSUFFICIENT_TRUST",
                    "required_trust_level": required_trust,
                    "agent_trust_level": agent_trust,
                })),
            }));
        }
    }

    match state.tool_registry.execute(&req.tool_name, req.input).await {
        Ok(result) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let content = if result.success {
                serde_json::to_string(&result.data.unwrap_or(serde_json::json!({})))
                    .unwrap_or_default()
            } else {
                result.error.unwrap_or_else(|| "Unknown error".to_string())
            };

            Ok(Json(EapToolExecuteResponse {
                content,
                is_error: !result.success,
                duration_ms,
                metadata: result.metadata,
            }))
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            Ok(Json(EapToolExecuteResponse {
                content: format!("Tool execution error: {e}"),
                is_error: true,
                duration_ms,
                metadata: None,
            }))
        }
    }
}

/// `GET /api/v1/agents/{id}/tasks` — Poll for available tasks.
///
/// Returns the next pending work item assigned to or available for this agent.
/// If no tasks are available, returns 204 No Content.
async fn eap_poll_tasks(
    State(state): State<Arc<AppState>>,
    Path(_agent_id): Path<String>,
) -> Result<Json<Vec<EapTaskResponse>>, StatusCode> {
    // Query for pending work items (assigned to this agent or unassigned)
    let tasks = sqlx::query_as::<_, (
        uuid::Uuid,
        String,
        String,
        Option<serde_json::Value>,
        String,
        i32,
        Option<i64>,
        Option<chrono::DateTime<chrono::Utc>>,
        chrono::DateTime<chrono::Utc>,
    )>(
        r#"
        SELECT
            id, title, description, input_data, task_type, priority,
            reward_tokens, deadline_at, created_at
        FROM work_items
        WHERE status = 'pending'
        ORDER BY priority ASC, created_at ASC
        LIMIT 5
        "#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::warn!("Failed to query work items: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if tasks.is_empty() {
        return Ok(Json(vec![]));
    }

    let responses: Vec<EapTaskResponse> = tasks
        .into_iter()
        .map(|(id, title, description, input_data, task_type, priority, reward_tokens, deadline_at, created_at)| {
            EapTaskResponse {
                task_id: id.to_string(),
                title,
                description,
                context: input_data.unwrap_or(serde_json::json!({})),
                category: task_type,
                priority,
                reward_tokens: reward_tokens.unwrap_or(0),
                deadline_at: deadline_at.map(|d| d.to_rfc3339()),
                created_at: created_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(responses))
}

/// `POST /api/v1/agents/{id}/tasks/{task_id}/result` — Submit task result.
async fn eap_submit_task_result(
    State(state): State<Arc<AppState>>,
    Path((_agent_id, task_id)): Path<(String, String)>,
    Json(req): Json<EapTaskResultRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let task_uuid = uuid::Uuid::parse_str(&task_id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let now = chrono::Utc::now();

    let new_status = match req.status.as_str() {
        "completed" => "completed",
        "failed" => "failed",
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let result = sqlx::query(
        r#"
        UPDATE work_items
        SET
            status = $1,
            output_data = $2,
            completed_at = $3,
            updated_at = $4,
            metadata = metadata || $5
        WHERE id = $6 AND status IN ('pending', 'assigned', 'in_progress')
        RETURNING id
        "#,
    )
    .bind(new_status)
    .bind(&req.result)
    .bind(now)
    .bind(now)
    .bind(serde_json::json!({
        "execution_time_ms": req.execution_time_ms,
        "tools_used": req.tools_used,
        "error_message": req.error_message,
    }))
    .bind(task_uuid)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::warn!("Failed to update work item {}: {}", task_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    info!(task_id = %task_id, status = %new_status, "Task result submitted");

    Ok(Json(serde_json::json!({
        "accepted": true,
        "quality_score": null,
        "reward_status": "pending"
    })))
}
