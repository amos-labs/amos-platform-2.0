//! HTTP routes and WebSocket handlers

pub mod agent_proxy;
pub mod bots;
pub mod canvas;
pub mod credentials;
pub mod data;
pub mod harness_info;
pub mod health;
pub mod hooks;
pub mod integrations;
pub mod llm_providers;
pub mod packages;
pub mod revisions;
pub mod sites;
pub mod uploads;

use crate::state::AppState;
use axum::{extract::DefaultBodyLimit, extract::State, routing::get, Json, Router};
use std::sync::Arc;

/// Build all application routes
pub fn build_routes(state: Arc<AppState>) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness_check))
        // EAP discovery endpoints
        .route("/.well-known/agent.json", get(well_known_agent_json))
        .route("/api/v1/tools", get(list_tools))
        // Auth page routes (served as standalone HTML pages from system canvases)
        .route("/login", get(canvas::serve_login))
        .route("/register", get(canvas::serve_register))
        .route("/forgot-password", get(canvas::serve_forgot_password))
        // Canvas routes
        .nest("/api/v1/canvases", canvas::routes(state.clone()))
        // Public canvas route
        .route("/c/{slug}", get(canvas::serve_public_canvas))
        // OpenClaw agent management routes
        .nest("/api/v1/agents", bots::routes(state.clone()))
        // Agent proxy routes (forward chat to agent sidecar service)
        .nest("/api/v1/agent", agent_proxy::routes(state.clone()))
        // Upload routes (25 MB body limit for file uploads)
        .nest(
            "/api/v1/uploads",
            uploads::routes(state.clone()).layer(DefaultBodyLimit::max(25 * 1024 * 1024)),
        )
        // Integration routes
        .nest("/api/v1/integrations", integrations::routes(state.clone()))
        // Credential vault routes (Secure Input Canvas target)
        .nest("/api/v1/credentials", credentials::routes(state.clone()))
        // LLM Provider routes (BYOK - Bring Your Own Key)
        .nest(
            "/api/v1/llm-providers",
            llm_providers::routes(state.clone()),
        )
        // Revision and template routes
        .nest("/api/v1", revisions::routes(state.clone()))
        // Data API routes (collection/record CRUD for canvas components)
        .nest("/api/v1/data", data::routes(state.clone()))
        // Webhook ingress routes (automation triggers)
        .nest("/api/v1/hooks", hooks::routes(state.clone()))
        // Harness info route (multi-harness discovery)
        .nest("/api/v1/harness", harness_info::routes(state.clone()))
        // Package management routes
        .nest("/api/v1/packages", packages::routes(state.clone()))
        // Site management routes
        .nest("/api/v1/sites", sites::routes(state.clone()))
        // Public site serving
        .route("/s/{slug}", axum::routing::get(sites::serve_site_index))
        .route(
            "/s/{slug}/{*path}",
            axum::routing::get(sites::serve_site_page),
        )
        .route(
            "/s/{slug}/submit/{collection}",
            axum::routing::post(sites::handle_form_submit),
        )
        .with_state(state)
}

/// `GET /.well-known/agent.json` — EAP Agent Card discovery endpoint.
async fn well_known_agent_json(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let harness_role = std::env::var("AMOS_HARNESS_ROLE").unwrap_or_else(|_| "primary".into());
    let tool_count = state.tool_registry.list_tools().len();

    Json(serde_json::json!({
        "name": "amos-harness",
        "description": "AMOS Harness — per-customer AI operating system with tool execution",
        "url": format!("{}://{}:{}", "https", state.config.server.host, state.config.server.port),
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": "eap/1.0",
        "capabilities": {
            "streaming": true,
            "pushNotifications": false,
            "batchExecution": false
        },
        "skills": [],
        "provider": {
            "name": "AMOS Labs",
            "model": "multi-model (BYOK)"
        },
        "role": harness_role,
        "tools_available": tool_count,
        "contact": "https://amoslabs.com"
    }))
}

/// `GET /api/v1/tools` — EAP tool discovery endpoint.
///
/// Returns all available tools with their names, descriptions, categories,
/// and parameter schemas.
async fn list_tools(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let tools: Vec<serde_json::Value> = state
        .tool_registry
        .list_tools()
        .iter()
        .filter_map(|name| {
            let tool = state.tool_registry.get(name)?;
            Some(serde_json::json!({
                "name": tool.name(),
                "description": tool.description(),
                "category": format!("{:?}", tool.category()),
                "parameters_schema": tool.parameters_schema(),
                "required_trust_level": trust_level_for_category(tool.category()),
            }))
        })
        .collect();

    Json(serde_json::json!({
        "tools": tools,
        "count": tools.len()
    }))
}

/// Map tool categories to minimum trust levels per the EAP spec.
fn trust_level_for_category(category: amos_core::tools::ToolCategory) -> u8 {
    use amos_core::tools::ToolCategory;
    match category {
        ToolCategory::System | ToolCategory::Web | ToolCategory::Memory | ToolCategory::Knowledge => 1,
        ToolCategory::Schema | ToolCategory::Canvas | ToolCategory::Apps => 2,
        ToolCategory::Integration | ToolCategory::Automation | ToolCategory::TaskQueue => 3,
        ToolCategory::OpenClaw | ToolCategory::Document | ToolCategory::ImageGen => 3,
        ToolCategory::Platform => 4,
        _ => 2,
    }
}
