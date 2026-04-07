//! Harness connection management routes.

use crate::state::RelayState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Build harness routes.
pub fn routes() -> Router<RelayState> {
    Router::new()
        .route("/connect", post(connect_harness))
        .route("/", get(list_harnesses))
        .route("/{id}", get(get_harness))
        .route("/{id}/heartbeat", post(harness_heartbeat))
}

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct ConnectHarnessRequest {
    pub harness_id: String,
    pub name: String,
    pub version: String,
    pub endpoint_url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct HarnessHeartbeatRequest {
    pub version: String,
    pub healthy: bool,
    pub agent_count: u32,
    pub active_bounties: u32,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct HarnessResponse {
    pub harness_id: String,
    pub name: String,
    pub version: String,
    pub endpoint_url: String,
    pub healthy: bool,
    pub agent_count: i32,
    pub active_bounties: i32,
    pub connected_at: DateTime<Utc>,
    pub last_heartbeat: Option<DateTime<Utc>>,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// Connect a harness to the relay.
async fn connect_harness(
    State(state): State<RelayState>,
    Json(req): Json<ConnectHarnessRequest>,
) -> Result<(StatusCode, Json<HarnessResponse>), StatusCode> {
    let now = Utc::now();

    // Insert or update the harness record
    let harness = sqlx::query_as::<_, HarnessResponse>(
        r#"
        INSERT INTO relay_harnesses (
            harness_id, name, version, endpoint_url, api_key_hash,
            healthy, agent_count, active_bounties,
            connected_at, last_heartbeat
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (harness_id) DO UPDATE SET
            name = EXCLUDED.name,
            version = EXCLUDED.version,
            endpoint_url = EXCLUDED.endpoint_url,
            api_key_hash = EXCLUDED.api_key_hash,
            last_heartbeat = EXCLUDED.last_heartbeat
        RETURNING
            harness_id, name, version, endpoint_url,
            healthy, agent_count, active_bounties,
            connected_at, last_heartbeat
        "#,
    )
    .bind(&req.harness_id)
    .bind(&req.name)
    .bind(&req.version)
    .bind(&req.endpoint_url)
    .bind(hash_api_key(&req.api_key))
    .bind(true)
    .bind(0i32)
    .bind(0i32)
    .bind(now)
    .bind(Some(now))
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to connect harness: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Harness {} ({}) connected at {}",
        req.harness_id, req.name, req.endpoint_url
    );

    Ok((StatusCode::CREATED, Json(harness)))
}

/// List all connected harnesses.
async fn list_harnesses(
    State(state): State<RelayState>,
) -> Result<Json<Vec<HarnessResponse>>, StatusCode> {
    let harnesses = sqlx::query_as::<_, HarnessResponse>(
        r#"
        SELECT
            harness_id, name, version, endpoint_url,
            healthy, agent_count, active_bounties,
            connected_at, last_heartbeat
        FROM relay_harnesses
        ORDER BY connected_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to list harnesses: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(harnesses))
}

/// Get a single harness by ID.
async fn get_harness(
    State(state): State<RelayState>,
    Path(id): Path<String>,
) -> Result<Json<HarnessResponse>, StatusCode> {
    let harness = sqlx::query_as::<_, HarnessResponse>(
        r#"
        SELECT
            harness_id, name, version, endpoint_url,
            healthy, agent_count, active_bounties,
            connected_at, last_heartbeat
        FROM relay_harnesses
        WHERE harness_id = $1
        "#,
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to get harness {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(harness))
}

/// Harness heartbeat to report health and metrics.
async fn harness_heartbeat(
    State(state): State<RelayState>,
    Path(id): Path<String>,
    Json(req): Json<HarnessHeartbeatRequest>,
) -> Result<Json<HarnessResponse>, StatusCode> {
    let now = Utc::now();

    let harness = sqlx::query_as::<_, HarnessResponse>(
        r#"
        UPDATE relay_harnesses
        SET
            version = $1,
            healthy = $2,
            agent_count = $3,
            active_bounties = $4,
            last_heartbeat = $5
        WHERE harness_id = $6
        RETURNING
            harness_id, name, version, endpoint_url,
            healthy, agent_count, active_bounties,
            connected_at, last_heartbeat
        "#,
    )
    .bind(&req.version)
    .bind(req.healthy)
    .bind(req.agent_count as i32)
    .bind(req.active_bounties as i32)
    .bind(now)
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to update heartbeat for harness {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(harness))
}

/// Hash API key for storage (simple SHA-256 for now).
fn hash_api_key(api_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}
