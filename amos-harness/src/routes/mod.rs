//! HTTP routes and WebSocket handlers

pub mod agent_proxy;
pub mod bots;
pub mod canvas;
pub mod credentials;
pub mod data;
pub mod health;
pub mod integrations;
pub mod llm_providers;
pub mod revisions;
pub mod sites;
pub mod uploads;

use crate::state::AppState;
use axum::{extract::DefaultBodyLimit, routing::get, Router};
use std::sync::Arc;

/// Build all application routes
pub fn build_routes(state: Arc<AppState>) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness_check))
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
