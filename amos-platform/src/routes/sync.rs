//! Sync API endpoints for harness↔platform communication.
//!
//! These endpoints are called by the PlatformSyncClient running inside each
//! harness container (both managed and self-hosted deployments).

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::state::PlatformState;

pub fn routes() -> Router<PlatformState> {
    Router::new()
        .route("/sync/heartbeat", post(receive_heartbeat))
        .route("/sync/config", get(get_config))
        .route("/sync/activity", post(receive_activity))
        .route("/sync/version", get(get_latest_version))
}

// ── Heartbeat ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct HeartbeatPayload {
    harness_version: String,
    deployment_mode: String,
    uptime_secs: u64,
    healthy: bool,
    timestamp: String,
}

#[derive(Serialize)]
struct HeartbeatResponse {
    acknowledged: bool,
    server_time: DateTime<Utc>,
}

async fn receive_heartbeat(
    State(_state): State<PlatformState>,
    Json(payload): Json<HeartbeatPayload>,
) -> impl IntoResponse {
    debug!(
        "Heartbeat received: version={}, mode={}, uptime={}s, healthy={}",
        payload.harness_version, payload.deployment_mode,
        payload.uptime_secs, payload.healthy,
    );

    // TODO: Update harness status in database (last_seen, version, health)

    Json(HeartbeatResponse {
        acknowledged: true,
        server_time: Utc::now(),
    })
}

// ── Config Distribution ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ConfigQuery {
    version: Option<String>,
}

#[derive(Serialize)]
struct RemoteConfig {
    /// Latest available harness version.
    latest_version: Option<String>,
    /// Whether this harness instance is enabled.
    enabled: bool,
    /// Model overrides (empty for now).
    model_overrides: Vec<ModelOverride>,
    /// Feature flags.
    feature_flags: std::collections::HashMap<String, bool>,
    /// Sync timestamp.
    synced_at: String,
}

#[derive(Serialize)]
struct ModelOverride {
    name: String,
    model_id: String,
    tier: u8,
}

async fn get_config(
    State(_state): State<PlatformState>,
    axum::extract::Query(query): axum::extract::Query<ConfigQuery>,
) -> impl IntoResponse {
    debug!("Config request: version={:?}", query.version);

    // TODO: Look up harness-specific config from database
    // TODO: Check if harness version is outdated
    // TODO: Return any model overrides or feature flags

    let mut feature_flags = std::collections::HashMap::new();
    feature_flags.insert("sovereign_ai".to_string(), true);
    feature_flags.insert("custom_models".to_string(), true);

    Json(RemoteConfig {
        latest_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        enabled: true,
        model_overrides: vec![],
        feature_flags,
        synced_at: Utc::now().to_rfc3339(),
    })
}

// ── Activity Ingest ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ActivityReport {
    period_start: String,
    period_end: String,
    conversations: u64,
    messages: u64,
    tokens_input: u64,
    tokens_output: u64,
    tools_executed: u64,
    models_used: Vec<String>,
    timestamp: String,
}

#[derive(Serialize)]
struct ActivityResponse {
    accepted: bool,
    server_time: DateTime<Utc>,
}

async fn receive_activity(
    State(_state): State<PlatformState>,
    Json(report): Json<ActivityReport>,
) -> impl IntoResponse {
    info!(
        "Activity report: {} convs, {} msgs, {} input tokens, {} output tokens over {}..{}",
        report.conversations, report.messages,
        report.tokens_input, report.tokens_output,
        report.period_start, report.period_end,
    );

    // TODO: Store activity in database for billing aggregation
    // TODO: Update usage metrics for the harness's customer

    Json(ActivityResponse {
        accepted: true,
        server_time: Utc::now(),
    })
}

// ── Version Check ───────────────────────────────────────────────────────

#[derive(Serialize)]
struct VersionInfo {
    latest_version: String,
    minimum_version: String,
    release_notes_url: Option<String>,
    update_required: bool,
}

async fn get_latest_version() -> impl IntoResponse {
    Json(VersionInfo {
        latest_version: env!("CARGO_PKG_VERSION").to_string(),
        minimum_version: "0.1.0".to_string(),
        release_notes_url: None,
        update_required: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_config_serialization() {
        let config = RemoteConfig {
            latest_version: Some("0.2.0".into()),
            enabled: true,
            model_overrides: vec![],
            feature_flags: std::collections::HashMap::new(),
            synced_at: "2025-01-01T00:00:00Z".into(),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("0.2.0"));
        assert!(json.contains("\"enabled\":true"));
    }

    #[test]
    fn test_version_info_serialization() {
        let info = VersionInfo {
            latest_version: "0.1.0".into(),
            minimum_version: "0.1.0".into(),
            release_notes_url: None,
            update_required: false,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("0.1.0"));
    }
}
