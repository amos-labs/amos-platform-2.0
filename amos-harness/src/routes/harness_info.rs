//! Harness info endpoint for multi-harness discovery.
//!
//! Every harness exposes `/api/v1/harness/info` so the orchestrator on
//! the primary harness can understand each sibling's capabilities.

use crate::state::AppState;
use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Serialize)]
struct HarnessInfoResponse {
    harness_id: String,
    role: String,
    packages: Vec<String>,
    tools: Vec<String>,
    status: String,
    uptime_secs: u64,
}

/// Build harness info routes.
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new().route("/info", get(get_harness_info))
}

// Track startup time via lazy_static-style approach
static START_TIME: std::sync::OnceLock<SystemTime> = std::sync::OnceLock::new();

fn get_start_time() -> &'static SystemTime {
    START_TIME.get_or_init(SystemTime::now)
}

async fn get_harness_info(State(state): State<Arc<AppState>>) -> Json<HarnessInfoResponse> {
    let harness_id = std::env::var("AMOS_HARNESS_ID").unwrap_or_else(|_| "unknown".to_string());
    let role = std::env::var("AMOS_HARNESS_ROLE").unwrap_or_else(|_| "primary".to_string());
    let packages: Vec<String> = std::env::var("AMOS_PACKAGES")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let tools = state.tool_registry.list_tools();

    let uptime_secs = get_start_time().elapsed().map(|d| d.as_secs()).unwrap_or(0);

    Json(HarnessInfoResponse {
        harness_id,
        role,
        packages,
        tools,
        status: "running".to_string(),
        uptime_secs,
    })
}
