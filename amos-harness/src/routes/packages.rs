//! Package management routes — list, inspect, toggle, and update system prompts.
//!
//! Endpoints:
//!   - `GET  /api/v1/packages`              — List all packages with metadata
//!   - `GET  /api/v1/packages/{name}`       — Package detail + tool list
//!   - `POST /api/v1/packages/{name}/toggle` — Enable/disable at runtime
//!   - `PUT  /api/v1/packages/{name}/prompt` — Update system prompt injection

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_packages))
        .route("/{name}", get(get_package))
        .route("/{name}/toggle", post(toggle_package))
        .route("/{name}/prompt", put(update_prompt))
}

// ═══════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PackageRow {
    pub id: Uuid,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub system_prompt: Option<String>,
    pub tool_count: i32,
    pub tool_names: serde_json::Value,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePromptRequest {
    pub system_prompt: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ToggleResponse {
    pub name: String,
    pub enabled: bool,
    pub message: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════

/// GET /api/v1/packages — List all packages with metadata.
async fn list_packages(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PackageRow>>, StatusCode> {
    let packages = sqlx::query_as::<_, PackageRow>(
        r#"SELECT id, name, display_name, description, version, enabled,
                  system_prompt, tool_count, tool_names, metadata,
                  created_at, updated_at
           FROM packages
           ORDER BY enabled DESC, name ASC"#,
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list packages: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(packages))
}

/// GET /api/v1/packages/{name} — Package detail + tool list.
async fn get_package(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<PackageRow>, StatusCode> {
    let package = sqlx::query_as::<_, PackageRow>("SELECT * FROM packages WHERE name = $1")
        .bind(&name)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get package {}: {}", name, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(package))
}

/// POST /api/v1/packages/{name}/toggle — Enable/disable a package at runtime.
///
/// Toggles the `enabled` flag in the DB and in the in-memory ToolRegistry.
/// When disabled, the package's tools are hidden from agents immediately.
async fn toggle_package(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<ToggleResponse>, StatusCode> {
    // Get current state
    let current: (bool,) = sqlx::query_as("SELECT enabled FROM packages WHERE name = $1")
        .bind(&name)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to query package {}: {}", name, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let new_enabled = !current.0;

    // Update DB
    sqlx::query("UPDATE packages SET enabled = $1, updated_at = NOW() WHERE name = $2")
        .bind(new_enabled)
        .bind(&name)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to toggle package {}: {}", name, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Update in-memory ToolRegistry (takes effect immediately)
    if new_enabled {
        state.tool_registry.enable_package(&name);
    } else {
        state.tool_registry.disable_package(&name);
    }

    let message = if new_enabled {
        format!("Package '{}' enabled — tools now active", name)
    } else {
        format!("Package '{}' disabled — tools hidden from agents", name)
    };

    tracing::info!("{}", message);

    Ok(Json(ToggleResponse {
        name,
        enabled: new_enabled,
        message,
    }))
}

/// PUT /api/v1/packages/{name}/prompt — Update the system prompt injection.
///
/// Allows overriding the package's default system prompt with a custom one.
/// Set `system_prompt` to `null` to revert to the compiled-in default.
async fn update_prompt(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(body): Json<UpdatePromptRequest>,
) -> Result<Json<PackageRow>, StatusCode> {
    // Verify package exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM packages WHERE name = $1)")
        .bind(&name)
        .fetch_one(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !exists {
        return Err(StatusCode::NOT_FOUND);
    }

    sqlx::query("UPDATE packages SET system_prompt = $1, updated_at = NOW() WHERE name = $2")
        .bind(&body.system_prompt)
        .bind(&name)
        .execute(&state.db_pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update prompt for package {}: {}", name, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(
        package = %name,
        has_prompt = body.system_prompt.is_some(),
        "Updated package system prompt"
    );

    // Return updated package
    let package = sqlx::query_as::<_, PackageRow>("SELECT * FROM packages WHERE name = $1")
        .bind(&name)
        .fetch_one(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(package))
}
