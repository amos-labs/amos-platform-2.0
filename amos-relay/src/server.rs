//! HTTP server configuration and routing.

use crate::{routes, state::RelayState, Result, VERSION};
use axum::{
    extract::State,
    http::{header, Method, StatusCode},
    response::Json,
    routing::get,
    Router,
};
use serde_json::{json, Value};
use std::time::Duration;
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer,
};
use tracing::info;

/// Start the HTTP server.
pub async fn start_http_server(state: RelayState) -> Result<()> {
    let app = build_http_router(state.clone());

    let addr = format!("{}:{}", state.config.server.host, state.config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| amos_core::AmosError::Internal(format!("Failed to bind to {}: {}", addr, e)))?;

    info!("AMOS Network Relay listening on {}", addr);

    axum::serve(listener, app)
        .await
        .map_err(|e| amos_core::AmosError::Internal(format!("Server error: {}", e)))?;

    Ok(())
}

/// Build the HTTP router with all routes and middleware.
pub fn build_http_router(state: RelayState) -> Router {
    Router::new()
        .route("/health", get(health))
        .nest("/api/v1", routes::api_routes())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    Duration::from_secs(30),
                ))
                .layer(
                    CorsLayer::new()
                        .allow_origin(tower_http::cors::Any)
                        .allow_methods([
                            Method::GET,
                            Method::POST,
                            Method::PUT,
                            Method::DELETE,
                        ])
                        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]),
                ),
        )
        .with_state(state)
}

/// Health check endpoint.
async fn health(State(state): State<RelayState>) -> (StatusCode, Json<Value>) {
    // Optionally perform a deeper health check
    match state.health_check().await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "status": "ok",
                "version": VERSION,
                "service": "amos-relay"
            })),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "error",
                "version": VERSION,
                "service": "amos-relay"
            })),
        ),
    }
}
