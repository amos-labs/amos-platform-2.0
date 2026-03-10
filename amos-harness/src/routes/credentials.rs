//! Credential vault routes - secure storage for user secrets.
//!
//! These endpoints are called by the Secure Input Canvas (frontend) to store
//! credentials without them flowing through the chat. The AI agent never sees
//! the plaintext values; it only receives opaque credential IDs.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_credentials).post(store_credential))
        .route("/{id}", get(get_credential).delete(revoke_credential))
}

// ═══════════════════════════════════════════════════════════════════════════
// Request/Response types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct StoreCredentialRequest {
    /// Human-readable label (e.g. "Stripe Secret Key")
    pub label: String,
    /// Service name (e.g. "stripe", "github", "sendgrid")
    pub service: String,
    /// Credential type (e.g. "api_key", "oauth_token", "password")
    #[serde(default = "default_credential_type")]
    pub credential_type: String,
    /// The secret value to encrypt and store
    pub secret_value: String,
    /// Optional extra metadata fields to encrypt (e.g. {"api_secret": "..."})
    pub extra_fields: Option<JsonValue>,
}

fn default_credential_type() -> String {
    "api_key".to_string()
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CredentialRow {
    pub id: Uuid,
    pub label: String,
    pub service: String,
    pub credential_type: String,
    // encrypted_value intentionally omitted from response
    pub status: String,
    pub integration_credential_id: Option<Uuid>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct StoreCredentialResponse {
    pub credential_id: Uuid,
    pub label: String,
    pub service: String,
    pub credential_type: String,
    pub message: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════

/// POST /api/v1/credentials - Store a new encrypted credential.
/// Called by the Secure Input Canvas, NOT by the AI agent.
async fn store_credential(
    State(state): State<Arc<AppState>>,
    Json(body): Json<StoreCredentialRequest>,
) -> Result<(StatusCode, Json<StoreCredentialResponse>), StatusCode> {
    // Encrypt the secret value
    let encrypted_value = state.vault.encrypt_string(&body.secret_value).map_err(|e| {
        tracing::error!("Failed to encrypt credential: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Encrypt extra fields if provided
    let encrypted_metadata = match &body.extra_fields {
        Some(fields) => {
            let json_str = serde_json::to_string(fields).map_err(|_| StatusCode::BAD_REQUEST)?;
            Some(state.vault.encrypt_string(&json_str).map_err(|e| {
                tracing::error!("Failed to encrypt metadata: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?)
        }
        None => None,
    };

    let credential_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO credential_vault
           (id, label, service, credential_type, encrypted_value, encrypted_metadata, status)
           VALUES ($1, $2, $3, $4, $5, $6, 'active')"#,
    )
    .bind(credential_id)
    .bind(&body.label)
    .bind(&body.service)
    .bind(&body.credential_type)
    .bind(&encrypted_value)
    .bind(&encrypted_metadata)
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to store credential: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!(
        credential_id = %credential_id,
        service = %body.service,
        "Credential stored securely in vault"
    );

    Ok((
        StatusCode::CREATED,
        Json(StoreCredentialResponse {
            credential_id,
            label: body.label,
            service: body.service,
            credential_type: body.credential_type,
            message: "Credential stored securely".to_string(),
        }),
    ))
}

/// GET /api/v1/credentials - List stored credentials (metadata only, no secrets).
async fn list_credentials(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CredentialRow>>, StatusCode> {
    let credentials = sqlx::query_as::<_, CredentialRow>(
        r#"SELECT id, label, service, credential_type, status,
                  integration_credential_id, last_used_at, expires_at,
                  created_at, updated_at
           FROM credential_vault
           WHERE status = 'active'
           ORDER BY created_at DESC"#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list credentials: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(credentials))
}

/// GET /api/v1/credentials/:id - Get credential metadata (no secret).
async fn get_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<CredentialRow>, StatusCode> {
    let credential = sqlx::query_as::<_, CredentialRow>(
        r#"SELECT id, label, service, credential_type, status,
                  integration_credential_id, last_used_at, expires_at,
                  created_at, updated_at
           FROM credential_vault WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(credential))
}

/// DELETE /api/v1/credentials/:id - Revoke a credential (soft delete).
async fn revoke_credential(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let result = sqlx::query(
        "UPDATE credential_vault SET status = 'revoked', updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    tracing::info!(credential_id = %id, "Credential revoked");
    Ok(StatusCode::NO_CONTENT)
}

// ═══════════════════════════════════════════════════════════════════════════
// Internal functions (used by ApiExecutor and tools, not exposed as routes)
// ═══════════════════════════════════════════════════════════════════════════

/// Decrypt and return the secret value for a credential.
/// This is used internally by the ApiExecutor when making API calls.
pub async fn decrypt_credential(
    db_pool: &sqlx::PgPool,
    vault: &amos_core::CredentialVault,
    credential_id: Uuid,
) -> Result<String, StatusCode> {
    let row: (String, String) = sqlx::query_as(
        "SELECT encrypted_value, status FROM credential_vault WHERE id = $1",
    )
    .bind(credential_id)
    .fetch_optional(db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let (encrypted_value, status) = row;
    if status != "active" {
        tracing::warn!(credential_id = %credential_id, status = %status, "Attempted to use non-active credential");
        return Err(StatusCode::GONE);
    }

    // Update last_used_at
    let _ = sqlx::query("UPDATE credential_vault SET last_used_at = NOW() WHERE id = $1")
        .bind(credential_id)
        .execute(db_pool)
        .await;

    vault.decrypt_string(&encrypted_value).map_err(|e| {
        tracing::error!("Failed to decrypt credential {}: {}", credential_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}
