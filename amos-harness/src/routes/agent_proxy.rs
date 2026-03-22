//! Reverse proxy for agent API endpoints.
//!
//! The harness frontend (app.js) sends chat requests to `/api/v1/agent/chat`,
//! but the actual agent service runs as a sidecar on a separate port (default 3100).
//! This module proxies those requests through to the agent, preserving the SSE
//! streaming response for chat.
//!
//! **BYOK injection**: Before forwarding to the agent, the proxy looks up the active
//! LLM provider from the database. If one is configured, it decrypts the API key
//! from the credential vault and injects `provider_type`, `api_base`, `api_key`,
//! and `model_id` into the JSON body. The agent then uses these to create a
//! per-request provider instead of its default Bedrock provider.
//!
//! Endpoints proxied:
//!   - `POST /api/v1/agent/chat`       → agent `POST /api/v1/chat` (with BYOK injection)
//!   - `POST /api/v1/agent/chat/cancel` → stub (agent doesn't support cancel yet)
//!   - `GET  /api/v1/agent/sessions`    → stub (agent doesn't persist sessions yet)
//!   - `GET  /api/v1/agent/sessions/:id` → stub

use crate::routes::credentials;
use crate::state::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures::TryStreamExt;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Build agent proxy routes.
///
/// All routes are relative — they get nested under `/api/v1/agent` in `build_routes()`.
pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/chat", post(proxy_chat))
        .route("/chat/cancel", post(cancel_chat))
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session))
}

/// Resolve the agent service URL from environment.
///
/// In ECS Fargate, the agent runs as a sidecar container in the same task,
/// so it's reachable at `localhost:3100`. In local dev, the agent may run
/// on any host/port.
fn agent_base_url() -> String {
    std::env::var("AGENT_URL").unwrap_or_else(|_| "http://localhost:3100".to_string())
}

/// Proxy `POST /api/v1/agent/chat` → agent `POST /api/v1/chat`.
///
/// This is an SSE streaming proxy: we forward the JSON body to the agent,
/// then stream the agent's SSE response byte-for-byte back to the browser.
///
/// **BYOK injection**: Before forwarding, we look up the active LLM provider
/// from the database. If one exists, we decrypt its API key and inject
/// `provider_type`, `api_base`, `api_key`, and `model_id` into the JSON body.
/// This lets the agent create a per-request provider instead of its default.
async fn proxy_chat(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<Response, StatusCode> {
    let agent_url = format!("{}/api/v1/chat", agent_base_url());

    // Try to inject BYOK provider config into the request body
    let enriched_body = match inject_byok_provider(&state, &body).await {
        Ok(b) => b,
        Err(e) => {
            // Non-fatal: if we can't look up the provider, forward the original body.
            // The agent will fall back to its default provider (Bedrock).
            warn!("BYOK injection skipped ({}), forwarding original body", e);
            body
        }
    };

    info!(url = %agent_url, byok = enriched_body.contains("provider_type"), "Proxying chat request to agent");

    let client = reqwest::Client::new();
    let agent_response = match client
        .post(&agent_url)
        .header("Content-Type", "application/json")
        .body(enriched_body)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to connect to agent service at {}: {}", agent_url, e);
            // Return an SSE error event so the frontend shows a proper message
            // instead of a raw 502.
            let error_event = "event: error\ndata: {\"type\":\"error\",\"message\":\"Agent service is not available. Please try again shortly or contact support.\"}\n\n".to_string();
            return Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream")
                .header(header::CACHE_CONTROL, "no-cache")
                .body(Body::from(error_event))
                .unwrap());
        }
    };

    let status = agent_response.status();

    if !status.is_success() {
        warn!(
            status = %status,
            "Agent returned non-success status"
        );
        // Forward the error response as-is
        let error_body = agent_response.text().await.unwrap_or_default();
        return Ok(Response::builder()
            .status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(error_body))
            .unwrap());
    }

    // Stream the SSE response back to the browser.
    // Convert reqwest's byte stream into an axum Body.
    let stream = agent_response.bytes_stream().map_err(std::io::Error::other);

    let body = Body::from_stream(stream);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(body)
        .unwrap())
}

/// Look up the active LLM provider from the database, decrypt its API key,
/// and inject `provider_type`, `api_base`, `api_key`, `model_id` into the
/// chat request JSON body.
///
/// Returns the enriched JSON string, or an error string if no provider is
/// configured or decryption fails.
async fn inject_byok_provider(state: &AppState, body: &str) -> Result<String, String> {
    // Parse the incoming JSON body
    let mut json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("invalid JSON: {e}"))?;

    // Don't override if the client already supplied provider config
    if json.get("provider_type").is_some() {
        return Ok(body.to_string());
    }

    // Look up the active LLM provider
    let provider = sqlx::query_as::<_, crate::routes::llm_providers::LlmProviderRow>(
        "SELECT * FROM llm_providers WHERE is_active = true LIMIT 1",
    )
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("DB error: {e}"))?
    .ok_or_else(|| "no active provider".to_string())?;

    let credential_id = provider
        .credential_id
        .ok_or_else(|| "active provider has no credential".to_string())?;

    // Decrypt the API key from the credential vault
    let api_key = credentials::decrypt_credential(&state.db_pool, &state.vault, credential_id)
        .await
        .map_err(|status| format!("decrypt failed: HTTP {}", status.as_u16()))?;

    // Inject BYOK fields into the JSON body
    let obj = json
        .as_object_mut()
        .ok_or_else(|| "body is not a JSON object".to_string())?;
    obj.insert(
        "provider_type".to_string(),
        serde_json::Value::String(provider.name.clone()),
    );
    obj.insert(
        "api_base".to_string(),
        serde_json::Value::String(provider.api_base.clone()),
    );
    obj.insert("api_key".to_string(), serde_json::Value::String(api_key));
    obj.insert(
        "model_id".to_string(),
        serde_json::Value::String(provider.default_model.clone()),
    );

    info!(
        provider = %provider.name,
        model = %provider.default_model,
        "Injected BYOK provider config into chat request"
    );

    serde_json::to_string(&json).map_err(|e| format!("JSON serialize: {e}"))
}

/// Stub for `POST /api/v1/agent/chat/cancel`.
///
/// The agent doesn't support cancellation yet. Return 200 so the frontend
/// doesn't show an error — the AbortController on the client side will
/// close the SSE stream regardless.
async fn cancel_chat() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "message": "Cancel acknowledged (client-side abort)"
    }))
}

/// Stub for `GET /api/v1/agent/sessions`.
///
/// The agent doesn't persist sessions yet. Return an empty list so the
/// sidebar renders correctly.
async fn list_sessions() -> impl IntoResponse {
    Json(serde_json::json!({
        "sessions": []
    }))
}

/// Stub for `GET /api/v1/agent/sessions/:id`.
///
/// Return 404 so the frontend clears the stale session ID and starts fresh.
async fn get_session() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "Session not found",
            "message": "Session persistence is not yet implemented"
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_base_url_defaults_to_localhost() {
        std::env::remove_var("AGENT_URL");
        assert_eq!(agent_base_url(), "http://localhost:3100");
    }

    #[test]
    fn agent_base_url_reads_env() {
        std::env::set_var("AGENT_URL", "http://agent-sidecar:3100");
        assert_eq!(agent_base_url(), "http://agent-sidecar:3100");
        std::env::remove_var("AGENT_URL");
    }
}
