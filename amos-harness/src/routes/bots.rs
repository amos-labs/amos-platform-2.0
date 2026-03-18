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
}

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_agents).post(register_agent))
        // EAP endpoints (sidecar agent protocol)
        .route("/register", post(eap_register))
        .route("/{id}/heartbeat", post(eap_heartbeat))
        .route("/{id}/tools/execute", post(eap_tool_execute))
        .route("/{id}/tasks", get(eap_poll_tasks))
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

    // Generate a simple agent ID for this session
    let agent_id = uuid::Uuid::new_v4().to_string();

    info!(
        agent = %req.name,
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
async fn eap_tool_execute(
    State(state): State<Arc<AppState>>,
    Path(_id): Path<String>,
    Json(req): Json<EapToolExecuteRequest>,
) -> Result<Json<EapToolExecuteResponse>, StatusCode> {
    let start = std::time::Instant::now();

    info!(tool = %req.tool_name, "EAP tool execution request");

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
            }))
        }
        Err(e) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            Ok(Json(EapToolExecuteResponse {
                content: format!("Tool execution error: {e}"),
                is_error: true,
                duration_ms,
            }))
        }
    }
}

/// `GET /api/v1/agents/{id}/tasks` — Poll for available tasks (returns empty for now).
async fn eap_poll_tasks(Path(_id): Path<String>) -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}
