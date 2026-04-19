//! Harness info endpoint for multi-harness discovery.
//!
//! Every harness exposes `/api/v1/harness/info` so the orchestrator on
//! the primary harness can understand each sibling's capabilities.

use crate::orchestrator::provisioning_tools::{find_catalog_entry, SPECIALIST_CATALOG};
use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
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
    Router::new()
        .route("/info", get(get_harness_info))
        .route("/specialists", get(get_specialists))
        .route("/update-status", get(get_update_status))
        .route("/update", post(trigger_update))
}

#[derive(Serialize)]
struct TriggerUpdateResponse {
    success: bool,
    new_version: Option<String>,
    error: Option<String>,
}

/// Kick off a self-update by calling the platform's
/// `POST /api/v1/sync/update-self` endpoint on the user's behalf. The
/// in-harness "Update now" banner button hits this so customers never
/// have to leave the harness to take a release.
///
/// The platform handles everything: stops the old task, registers a new
/// task def with the latest release image tags, starts the new task, and
/// re-registers the ALB target. This endpoint returns as soon as the
/// platform has kicked the process off — the harness itself will be
/// restarted within seconds, which terminates this very request.
async fn trigger_update(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TriggerUpdateResponse>, StatusCode> {
    let harness_id = match std::env::var("AMOS_HARNESS_ID") {
        Ok(id) => id,
        Err(_) => {
            return Ok(Json(TriggerUpdateResponse {
                success: false,
                new_version: None,
                error: Some(
                    "AMOS_HARNESS_ID env var not set — self-update requires a managed harness"
                        .to_string(),
                ),
            }))
        }
    };

    let platform_url = state.config.platform.url.trim_end_matches('/');
    if platform_url.is_empty() {
        return Ok(Json(TriggerUpdateResponse {
            success: false,
            new_version: None,
            error: Some("Platform URL not configured".to_string()),
        }));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let resp = client
        .post(format!("{}/api/v1/sync/update-self", platform_url))
        .json(&serde_json::json!({ "harness_id": harness_id }))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Self-update request to platform failed: {}", e);
            StatusCode::BAD_GATEWAY
        })?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_else(|_| serde_json::json!({}));

    if !status.is_success() {
        return Ok(Json(TriggerUpdateResponse {
            success: false,
            new_version: None,
            error: Some(
                body.get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Platform returned an error")
                    .to_string(),
            ),
        }));
    }

    Ok(Json(TriggerUpdateResponse {
        success: true,
        new_version: body
            .get("new_version")
            .and_then(|v| v.as_str())
            .map(String::from),
        error: None,
    }))
}

#[derive(Serialize)]
struct UpdateStatusResponse {
    current_version: String,
    latest_version: Option<String>,
    update_available: bool,
    /// URL of the platform dashboard where customers can click Update.
    platform_update_url: Option<String>,
}

/// Tells the frontend whether a newer release is available from the
/// platform. Read by the update banner in the harness SPA — polled
/// every ~5 minutes or rendered once on page load.
async fn get_update_status(State(state): State<Arc<AppState>>) -> Json<UpdateStatusResponse> {
    let current = state.config.deployment.harness_version.clone();

    let (latest, update_available) = match &state.platform_sync {
        Some(client) => {
            let latest = client.update_available().await;
            let available = latest.is_some();
            (latest, available)
        }
        None => (None, false),
    };

    let platform_update_url = if update_available {
        Some(format!(
            "{}/dashboard",
            state.config.platform.url.trim_end_matches('/')
        ))
    } else {
        None
    };

    Json(UpdateStatusResponse {
        current_version: current,
        latest_version: latest,
        update_available,
        platform_update_url,
    })
}

// Track startup time via lazy_static-style approach
static START_TIME: std::sync::OnceLock<SystemTime> = std::sync::OnceLock::new();

fn get_start_time() -> &'static SystemTime {
    START_TIME.get_or_init(SystemTime::now)
}

#[derive(Serialize)]
struct SpecialistInfo {
    friendly_name: String,
    slug: String,
    icon_hint: String,
    status: String,
    healthy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    harness_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Serialize)]
struct SpecialistsResponse {
    specialists: Vec<SpecialistInfo>,
    available: Vec<SpecialistInfo>,
}

async fn get_specialists(State(state): State<Arc<AppState>>) -> Json<SpecialistsResponse> {
    let orchestrator = match &state.orchestrator {
        Some(o) => o,
        None => {
            // No orchestrator — return empty lists with all catalog entries as available
            let available = SPECIALIST_CATALOG
                .iter()
                .map(|e| SpecialistInfo {
                    friendly_name: e.friendly_name.to_string(),
                    slug: e.slug.to_string(),
                    icon_hint: e.icon_hint.to_string(),
                    status: "available".to_string(),
                    healthy: false,
                    harness_id: None,
                    description: Some(e.description.to_string()),
                })
                .collect();
            return Json(SpecialistsResponse {
                specialists: vec![],
                available,
            });
        }
    };

    // Refresh discovery and get current siblings
    orchestrator.refresh_discovery().await;
    let siblings = orchestrator.proxy.get_siblings().await;

    let mut specialists = Vec::new();
    let mut available = Vec::new();

    for entry in SPECIALIST_CATALOG {
        let running = siblings
            .iter()
            .find(|s| s.packages.contains(&entry.slug.to_string()));

        if let Some(sibling) = running {
            specialists.push(SpecialistInfo {
                friendly_name: entry.friendly_name.to_string(),
                slug: entry.slug.to_string(),
                icon_hint: entry.icon_hint.to_string(),
                status: sibling.status.clone(),
                healthy: sibling.healthy.unwrap_or(false),
                harness_id: Some(sibling.harness_id.clone()),
                description: None,
            });
        } else {
            available.push(SpecialistInfo {
                friendly_name: entry.friendly_name.to_string(),
                slug: entry.slug.to_string(),
                icon_hint: entry.icon_hint.to_string(),
                status: "available".to_string(),
                healthy: false,
                harness_id: None,
                description: Some(entry.description.to_string()),
            });
        }
    }

    // Also include any running siblings that aren't in the catalog
    for sibling in &siblings {
        let in_catalog = SPECIALIST_CATALOG
            .iter()
            .any(|e| sibling.packages.contains(&e.slug.to_string()));

        if !in_catalog {
            let name = sibling
                .name
                .clone()
                .or_else(|| {
                    sibling
                        .packages
                        .first()
                        .and_then(|slug| find_catalog_entry(slug))
                        .map(|e| e.friendly_name.to_string())
                })
                .unwrap_or_else(|| sibling.harness_id.clone());

            specialists.push(SpecialistInfo {
                friendly_name: name,
                slug: sibling
                    .packages
                    .first()
                    .cloned()
                    .unwrap_or_else(|| sibling.harness_id.clone()),
                icon_hint: "cpu".to_string(),
                status: sibling.status.clone(),
                healthy: sibling.healthy.unwrap_or(false),
                harness_id: Some(sibling.harness_id.clone()),
                description: None,
            });
        }
    }

    Json(SpecialistsResponse {
        specialists,
        available,
    })
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
