//! OpenClaw agent management routes

use crate::{openclaw::AgentConfig, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

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

pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_agents).post(register_agent))
        .route("/{id}", get(get_agent).put(update_agent))
        .route("/{id}/activate", post(activate_agent))
        .route("/{id}/stop", post(stop_agent))
}

async fn list_agents(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AgentConfig>>, StatusCode> {
    let agents = state.agent_manager.list_agents().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(agents))
}

async fn register_agent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterAgentRequest>,
) -> Result<Json<AgentConfig>, StatusCode> {
    let agent = state.agent_manager.register_agent(
        req.name,
        req.display_name,
        req.role,
        req.capabilities,
        req.system_prompt,
        req.model,
    ).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(agent))
}

async fn get_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let status = state.agent_manager.get_status(id).await
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

    let agent = state.agent_manager.update_agent(id, updates).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(agent))
}

async fn activate_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    state.agent_manager.activate_agent(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

async fn stop_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    state.agent_manager.stop_agent(id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}
