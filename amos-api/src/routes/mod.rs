pub mod agent;
pub mod health;
pub mod solana;
pub mod token;

use axum::{
    Router,
    routing::{get, post, any},
};
use crate::state::AppState;
use crate::middleware::error_handler::handle_error;

/// Build the complete Axum router with all route groups
pub fn build_routes() -> Router<std::sync::Arc<AppState>> {
    Router::new()
        // Health routes
        .route("/health", get(health::health_check))
        .route("/health/ready", get(health::readiness_check))

        // Agent routes (main API)
        .route("/api/v1/agent/chat", post(agent::chat_handler))
        .route("/api/v1/agent/chat/sync", post(agent::chat_sync_handler))
        .route("/api/v1/agent/sessions/:id", get(agent::get_session_handler))

        // Token economics routes
        .route("/api/v1/token/stats", get(token::stats_handler))
        .route("/api/v1/token/decay-rate", get(token::decay_rate_handler))
        .route("/api/v1/token/calculate-decay", post(token::calculate_decay_handler))
        .route("/api/v1/token/revenue-split", post(token::revenue_split_handler))
        .route("/api/v1/token/emission", get(token::emission_handler))

        // Solana bridge routes
        .route("/api/v1/solana/treasury", get(solana::treasury_state_handler))
        .route("/api/v1/solana/stake/:wallet", get(solana::stake_record_handler))
        .route("/api/v1/solana/verify-wallet", post(solana::verify_wallet_handler))

        // Rails proxy routes (for gradual migration)
        .route("/rails/*path", any(rails_proxy_handler))
}

/// Proxy handler for Rails routes during hybrid operation
async fn rails_proxy_handler(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<AppState>>,
    axum::extract::Path(path): axum::extract::Path<String>,
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
    body: axum::body::Body,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    use axum::body::Body;
    use axum::response::IntoResponse;

    // Get Rails base URL from config
    let rails_url = &state.config.server.rails_url;
    if rails_url.is_empty() {
        return Err(axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    // Build target URL
    let target_url = format!("{}/{}", rails_url, path);

    tracing::debug!("Proxying {} request to Rails: {}", method, target_url);

    // Create HTTP client
    let client = reqwest::Client::new();

    // Build request
    let mut request_builder = match method {
        axum::http::Method::GET => client.get(&target_url),
        axum::http::Method::POST => client.post(&target_url),
        axum::http::Method::PUT => client.put(&target_url),
        axum::http::Method::DELETE => client.delete(&target_url),
        axum::http::Method::PATCH => client.patch(&target_url),
        _ => return Err(axum::http::StatusCode::METHOD_NOT_ALLOWED),
    };

    // Forward headers (except host)
    for (key, value) in headers.iter() {
        if key != "host" {
            if let Ok(value_str) = value.to_str() {
                request_builder = request_builder.header(key.as_str(), value_str);
            }
        }
    }

    // Convert axum body to bytes
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    // Send request to Rails
    let response = request_builder
        .body(body_bytes.to_vec())
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to proxy to Rails: {}", e);
            axum::http::StatusCode::BAD_GATEWAY
        })?;

    // Build response
    let status = response.status();
    let headers = response.headers().clone();
    let body_bytes = response.bytes().await.map_err(|_| axum::http::StatusCode::BAD_GATEWAY)?;

    let mut response_builder = axum::http::Response::builder().status(status);

    // Copy response headers
    for (key, value) in headers.iter() {
        response_builder = response_builder.header(key, value);
    }

    response_builder
        .body(Body::from(body_bytes))
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routes_build() {
        // Verify router can be constructed
        let router = build_routes();
        // Router type check is sufficient
    }
}
