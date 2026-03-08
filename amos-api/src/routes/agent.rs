use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Sse},
    Json,
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use crate::state::AppState;

/// Request body for chat endpoints
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    /// The user's message
    pub message: String,

    /// Optional session ID to continue existing conversation
    #[serde(default)]
    pub session_id: Option<String>,

    /// Optional context for the conversation
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}

/// Response for synchronous chat endpoint
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    /// Session ID for this conversation
    pub session_id: String,

    /// Agent's response message
    pub response: String,

    /// Token usage information
    pub tokens_used: u32,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// Session state response
#[derive(Debug, Serialize)]
pub struct SessionState {
    /// Session ID
    pub session_id: String,

    /// Number of messages in conversation
    pub message_count: usize,

    /// Total tokens used in session
    pub total_tokens: u32,

    /// Session creation timestamp
    pub created_at: String,

    /// Last activity timestamp
    pub last_activity: String,

    /// Session status
    pub status: String,
}

/// Streaming chat handler
/// Accepts a chat message and returns SSE stream of agent responses
pub async fn chat_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatRequest>,
) -> Result<Sse<impl Stream<Item = Result<axum::response::sse::Event, Infallible>>>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Chat request: session_id={:?}, message_length={}",
        request.session_id,
        request.message.len()
    );

    // Generate or use existing session ID
    let session_id = request.session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // TODO: Create/retrieve agent loop from runtime
    // TODO: Process message through agent loop
    // TODO: Stream responses

    // Placeholder: Create a simple stream
    let stream = stream::iter(vec![
        Ok(axum::response::sse::Event::default()
            .event("start")
            .data(json!({"session_id": session_id}).to_string())),
        Ok(axum::response::sse::Event::default()
            .event("message")
            .data(json!({"text": "Processing your request..."}).to_string())),
        Ok(axum::response::sse::Event::default()
            .event("message")
            .data(json!({"text": format!("You said: {}", request.message)}).to_string())),
        Ok(axum::response::sse::Event::default()
            .event("complete")
            .data(json!({"tokens_used": 150, "processing_time_ms": 500}).to_string())),
    ]);

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("keep-alive"),
    ))
}

/// Synchronous chat handler
/// Accepts a chat message and returns complete response as JSON
pub async fn chat_sync_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Sync chat request: session_id={:?}, message_length={}",
        request.session_id,
        request.message.len()
    );

    let start_time = std::time::Instant::now();

    // Generate or use existing session ID
    let session_id = request.session_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // TODO: Create/retrieve agent loop from runtime
    // TODO: Process message through agent loop
    // TODO: Collect complete response

    // Placeholder response
    let response = ChatResponse {
        session_id,
        response: format!("Echo: {}", request.message),
        tokens_used: 150,
        processing_time_ms: start_time.elapsed().as_millis() as u64,
    };

    Ok(Json(response))
}

/// Get session state handler
/// Returns current state of a conversation session
pub async fn get_session_handler(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionState>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Get session request: session_id={}", session_id);

    // TODO: Retrieve session from storage
    // TODO: Build session state response

    // Placeholder response
    let session_state = SessionState {
        session_id: session_id.clone(),
        message_count: 5,
        total_tokens: 750,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_activity: chrono::Utc::now().to_rfc3339(),
        status: "active".to_string(),
    };

    Ok(Json(session_state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_request_deserialize() {
        let json = r#"{"message": "Hello", "session_id": "123"}"#;
        let request: ChatRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.message, "Hello");
        assert_eq!(request.session_id, Some("123".to_string()));
    }

    #[test]
    fn test_chat_response_serialize() {
        let response = ChatResponse {
            session_id: "123".to_string(),
            response: "Hi there".to_string(),
            tokens_used: 50,
            processing_time_ms: 100,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("session_id"));
    }
}
