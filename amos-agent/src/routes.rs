//! HTTP routes for the agent's API server.
//!
//! When running as a service (Docker deployment), the agent exposes:
//! - `POST /api/v1/chat` - SSE streaming chat endpoint
//! - `GET /health` - Health check
//! - `GET /.well-known/agent.json` - Agent Card (served separately)
//!
//! The chat endpoint accepts a JSON body and returns Server-Sent Events.

use crate::{
    agent_loop::{self, AgentEvent, LoopConfig},
    harness_client::HarnessClient,
    provider::ModelProvider,
    tools::ToolContext,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Shared state for route handlers.
#[derive(Clone)]
pub struct AgentState {
    pub provider: Arc<dyn ModelProvider>,
    pub tool_ctx: Arc<ToolContext>,
    pub harness: Arc<RwLock<HarnessClient>>,
    pub loop_config: LoopConfig,
}

/// Chat request body.
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Chat response for non-streaming mode.
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub text: String,
    pub session_id: Option<String>,
}

/// Create the agent HTTP router.
pub fn agent_router(state: AgentState) -> Router {
    Router::new()
        .route("/api/v1/chat", post(chat_sse))
        .route("/health", get(health))
        .with_state(state)
}

/// Health check endpoint.
async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "amos-agent",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// SSE streaming chat endpoint.
///
/// Accepts a chat message and returns a stream of Server-Sent Events
/// corresponding to the agent's think-act-observe loop.
async fn chat_sse(
    State(state): State<AgentState>,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, axum::Error>>>, StatusCode> {
    info!(message_len = req.message.len(), "Chat request received");

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<AgentEvent>(100);

    // Clone what we need for the spawned task
    let provider = state.provider.clone();
    let tool_ctx = state.tool_ctx.clone();
    let harness = state.harness.clone();
    let loop_config = state.loop_config.clone();
    let message = req.message.clone();

    // Run the agent loop in a background task
    tokio::spawn(async move {
        let h = harness.read().await;
        let result = agent_loop::run_agent_loop(
            &loop_config,
            provider.as_ref(),
            &tool_ctx,
            Some(&h),
            &message,
            Some(event_tx),
        )
        .await;

        if let Err(e) = result {
            error!("Agent loop error: {e}");
        }
    });

    // Convert the mpsc receiver into an SSE stream
    let stream = async_stream::stream! {
        while let Some(event) = event_rx.recv().await {
            let event_type = match &event {
                AgentEvent::TurnStart { .. } => "turn_start",
                AgentEvent::TextDelta { .. } => "message_delta",
                AgentEvent::ToolStart { .. } => "tool_start",
                AgentEvent::ToolEnd { .. } => "tool_end",
                AgentEvent::TurnEnd { .. } => "turn_end",
                AgentEvent::Done { .. } => "agent_end",
                AgentEvent::Error { .. } => "error",
            };

            let data = serde_json::to_string(&event).unwrap_or_default();
            yield Ok(Event::default().event(event_type).data(data));
        }
    };

    Ok(Sse::new(stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_request_deserialization() {
        let json = r#"{"message": "hello"}"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.message, "hello");
        assert!(req.session_id.is_none());
    }

    #[test]
    fn test_chat_request_with_session() {
        let json = r#"{"message": "hello", "session_id": "abc-123"}"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.session_id, Some("abc-123".to_string()));
    }
}
