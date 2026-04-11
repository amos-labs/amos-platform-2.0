//! Harness settings routes — model selection, provider mode, etc.
//!
//! Endpoints:
//!   - `GET  /api/v1/settings` — Get all settings + available models
//!   - `PUT  /api/v1/settings` — Update one or more settings

use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_settings).put(update_settings))
}

// ═══════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize)]
struct SettingsResponse {
    /// Current provider mode: "shared_bedrock" or "byok"
    llm_provider_mode: String,
    /// Current model ID (Bedrock model ID for shared, provider-specific for BYOK)
    llm_model: String,
    /// Whether shared Bedrock is available (false for self-hosted)
    shared_bedrock_available: bool,
    /// Available models for shared Bedrock
    available_models: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
struct ModelInfo {
    id: &'static str,
    display_name: &'static str,
    tier: &'static str,
    input_price_per_mtok: f64,
    output_price_per_mtok: f64,
}

#[derive(Debug, Deserialize)]
struct UpdateSettingsRequest {
    llm_provider_mode: Option<String>,
    llm_model: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════════

const AVAILABLE_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "us.anthropic.claude-haiku-4-5-20251001-v1:0",
        display_name: "Claude Haiku 4.5",
        tier: "fast",
        input_price_per_mtok: 0.80,
        output_price_per_mtok: 4.00,
    },
    ModelInfo {
        id: "us.anthropic.claude-sonnet-4-6-20250514-v1:0",
        display_name: "Claude Sonnet 4.6",
        tier: "balanced",
        input_price_per_mtok: 3.00,
        output_price_per_mtok: 15.00,
    },
    ModelInfo {
        id: "us.anthropic.claude-opus-4-6-20250514-v1:0",
        display_name: "Claude Opus 4.6",
        tier: "powerful",
        input_price_per_mtok: 15.00,
        output_price_per_mtok: 75.00,
    },
];

const DEFAULT_MODEL: &str = "us.anthropic.claude-sonnet-4-6-20250514-v1:0";

// ═══════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════

async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SettingsResponse>, StatusCode> {
    let shared_bedrock_available = std::env::var("SHARED_BEDROCK_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let provider_mode = get_setting(&state, "llm_provider_mode")
        .await
        .unwrap_or_else(|| {
            if shared_bedrock_available {
                "shared_bedrock".to_string()
            } else {
                "byok".to_string()
            }
        });

    let model = get_setting(&state, "llm_model")
        .await
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());

    Ok(Json(SettingsResponse {
        llm_provider_mode: provider_mode,
        llm_model: model,
        shared_bedrock_available,
        available_models: AVAILABLE_MODELS
            .iter()
            .map(|m| ModelInfo {
                id: m.id,
                display_name: m.display_name,
                tier: m.tier,
                input_price_per_mtok: m.input_price_per_mtok,
                output_price_per_mtok: m.output_price_per_mtok,
            })
            .collect(),
    }))
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let shared_bedrock_available = std::env::var("SHARED_BEDROCK_ENABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    if let Some(mode) = &req.llm_provider_mode {
        if mode != "shared_bedrock" && mode != "byok" {
            return Err(StatusCode::BAD_REQUEST);
        }
        if mode == "shared_bedrock" && !shared_bedrock_available {
            return Err(StatusCode::BAD_REQUEST);
        }
        set_setting(&state, "llm_provider_mode", mode).await?;
    }

    if let Some(model) = &req.llm_model {
        // Validate model ID is in our allowed list
        let valid = AVAILABLE_MODELS.iter().any(|m| m.id == model);
        if !valid {
            return Err(StatusCode::BAD_REQUEST);
        }
        set_setting(&state, "llm_model", model).await?;
    }

    Ok(Json(serde_json::json!({ "updated": true })))
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Read a setting from the harness_settings table.
pub(crate) async fn get_setting(state: &AppState, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT value FROM harness_settings WHERE key = $1",
    )
    .bind(key)
    .fetch_optional(&state.db_pool)
    .await
    .ok()
    .flatten()
    .and_then(|v| {
        // Values are stored as JSON strings (e.g. "\"shared_bedrock\"")
        v.as_str().map(|s| s.to_string())
    })
}

/// Write a setting to the harness_settings table.
async fn set_setting(state: &AppState, key: &str, value: &str) -> Result<(), StatusCode> {
    sqlx::query(
        "INSERT INTO harness_settings (key, value, updated_at) VALUES ($1, $2, NOW())
         ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()",
    )
    .bind(key)
    .bind(serde_json::json!(value))
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::warn!("Failed to update setting {}: {}", key, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(())
}
