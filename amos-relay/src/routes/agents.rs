//! Global agent directory routes.

use crate::state::RelayState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

/// Build agent routes.
pub fn routes() -> Router<RelayState> {
    Router::new()
        .route("/register", post(register_agent))
        .route("/", get(list_agents))
        .route("/:id", get(get_agent))
        .route("/:id/heartbeat", post(agent_heartbeat))
}

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    pub name: String,
    pub display_name: String,
    pub endpoint_url: String,
    pub capabilities: Vec<String>,
    pub description: Option<String>,
    pub wallet_address: String,
    pub harness_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ListAgentsQuery {
    pub capability: Option<String>,
    pub trust_level: Option<u8>,
    pub status: Option<AgentStatus>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub status: Option<AgentStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "agent_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Active,
    Idle,
    Stopped,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentResponse {
    pub id: Uuid,
    pub name: String,
    pub display_name: String,
    pub endpoint_url: String,
    pub capabilities: Vec<String>,
    pub description: Option<String>,
    pub wallet_address: String,
    pub harness_id: String,
    pub trust_level: i16,
    pub status: AgentStatus,
    pub total_bounties_completed: i32,
    pub avg_quality_score: Option<f64>,
    pub registered_at: DateTime<Utc>,
    pub last_heartbeat: Option<DateTime<Utc>>,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// Register a new agent in the global directory.
async fn register_agent(
    State(state): State<RelayState>,
    Json(req): Json<RegisterAgentRequest>,
) -> Result<(StatusCode, Json<AgentResponse>), StatusCode> {
    let agent_id = Uuid::new_v4();
    let now = Utc::now();

    let agent = sqlx::query_as::<_, AgentResponse>(
        r#"
        INSERT INTO relay_agents (
            id, name, display_name, endpoint_url, capabilities,
            description, wallet_address, harness_id, trust_level,
            status, total_bounties_completed, avg_quality_score,
            registered_at, last_heartbeat
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING
            id, name, display_name, endpoint_url, capabilities,
            description, wallet_address, harness_id, trust_level,
            status,
            total_bounties_completed, avg_quality_score,
            registered_at, last_heartbeat
        "#,
    )
    .bind(agent_id)
    .bind(&req.name)
    .bind(&req.display_name)
    .bind(&req.endpoint_url)
    .bind(&req.capabilities)
    .bind(&req.description)
    .bind(&req.wallet_address)
    .bind(&req.harness_id)
    .bind(1i16) // Start at trust level 1 (Newcomer)
    .bind(AgentStatus::Active)
    .bind(0i32)
    .bind(None::<f64>)
    .bind(now)
    .bind(Some(now))
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to register agent: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Registered agent {} ({}) on harness {}",
        agent_id, req.name, req.harness_id
    );

    Ok((StatusCode::CREATED, Json(agent)))
}

/// List agents with optional filters.
async fn list_agents(
    State(state): State<RelayState>,
    Query(query): Query<ListAgentsQuery>,
) -> Result<Json<Vec<AgentResponse>>, StatusCode> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    // For simplicity, we'll fetch all agents and filter in-memory
    // In production, you'd want to build dynamic SQL queries
    let agents = sqlx::query_as::<_, AgentResponse>(
        r#"
        SELECT
            id, name, display_name, endpoint_url, capabilities,
            description, wallet_address, harness_id, trust_level,
            status,
            total_bounties_completed, avg_quality_score,
            registered_at, last_heartbeat
        FROM relay_agents
        ORDER BY registered_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(per_page as i64)
    .bind(offset as i64)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to list agents: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(agents))
}

/// Get a single agent by ID.
async fn get_agent(
    State(state): State<RelayState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentResponse>, StatusCode> {
    let agent = sqlx::query_as::<_, AgentResponse>(
        r#"
        SELECT
            id, name, display_name, endpoint_url, capabilities,
            description, wallet_address, harness_id, trust_level,
            status,
            total_bounties_completed, avg_quality_score,
            registered_at, last_heartbeat
        FROM relay_agents
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to get agent {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(agent))
}

/// Agent heartbeat to indicate it's still active.
async fn agent_heartbeat(
    State(state): State<RelayState>,
    Path(id): Path<Uuid>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<Json<AgentResponse>, StatusCode> {
    let now = Utc::now();

    let mut query = sqlx::QueryBuilder::new("UPDATE relay_agents SET last_heartbeat = ");
    query.push_bind(now);

    if let Some(status) = req.status {
        query.push(", status = ");
        query.push_bind(status);
    }

    query.push(" WHERE id = ");
    query.push_bind(id);
    query.push(" RETURNING *");

    let agent = sqlx::query_as::<_, AgentResponse>(
        r#"
        UPDATE relay_agents
        SET last_heartbeat = $1
        WHERE id = $2
        RETURNING
            id, name, display_name, endpoint_url, capabilities,
            description, wallet_address, harness_id, trust_level,
            status,
            total_bounties_completed, avg_quality_score,
            registered_at, last_heartbeat
        "#,
    )
    .bind(now)
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to update heartbeat for agent {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(agent))
}
