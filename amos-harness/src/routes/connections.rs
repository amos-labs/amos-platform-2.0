//! HTTP routes for the Connections canvas.
//!
//! The canvas is read-only for secrets (we never return client_secret or
//! access_token), but exposes:
//!   - GET /api/v1/connections                         — list all credentials
//!   - POST /api/v1/connections/:id/revoke             — revoke a credential
//!   - GET /api/v1/connections/providers               — list OAuth directory
//!
//! For creating new connections, the canvas drives the agent to call
//! `initiate_oauth_connection` (OAuth) or `store_credential` (API key).
//! The agent owns the creation flow because it can guide the user
//! conversationally with provider-specific instructions.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_connections))
        .route("/providers", get(list_providers))
        .route("/{id}/revoke", post(revoke_connection))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ConnectionView {
    id: Uuid,
    integration_id: Uuid,
    integration_name: Option<String>,
    auth_type: String,
    status: String,
    label: Option<String>,
    oauth_scopes: Option<String>,
    token_expires_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct ProviderView {
    slug: String,
    name: String,
    auth_url: String,
    token_url: String,
    default_scopes: Option<String>,
    app_creation_url: Option<String>,
    docs_url: Option<String>,
    icon_url: Option<String>,
    setup_instructions: Option<String>,
}

async fn list_connections(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ConnectionView>>, StatusCode> {
    let rows = sqlx::query_as::<_, ConnectionView>(
        r#"SELECT c.id, c.integration_id, i.name AS integration_name,
                  c.auth_type, c.status, c.label, c.oauth_scopes,
                  c.token_expires_at, c.last_used_at, c.created_at, c.updated_at
             FROM integration_credentials c
             LEFT JOIN integrations i ON i.id = c.integration_id
         ORDER BY c.created_at DESC"#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list connections: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}

async fn list_providers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ProviderView>>, StatusCode> {
    let rows = sqlx::query_as::<_, ProviderView>(
        r#"SELECT slug, name, auth_url, token_url, default_scopes,
                  app_creation_url, docs_url, icon_url, setup_instructions
             FROM oauth_providers
         ORDER BY name ASC"#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list oauth providers: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}

async fn revoke_connection(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result = sqlx::query(
        r#"UPDATE integration_credentials
              SET status = 'revoked',
                  access_token = NULL,
                  refresh_token = NULL,
                  token_expires_at = NULL,
                  updated_at = NOW()
            WHERE id = $1"#,
    )
    .bind(id)
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to revoke connection: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(serde_json::json!({ "revoked": true, "id": id })))
}
