//! Agent chat routes (streaming and synchronous) with session persistence

use crate::{
    agent::{bedrock::BedrockClient, loop_runner::{AgentLoop, LoopConfig}, AgentEvent},
    routes::uploads::load_upload_data,
    sessions,
    state::AppState,
};
use axum::{
    extract::{Path, Query, State, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::StatusCode,
    response::{sse::{Event, KeepAlive}, IntoResponse, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use futures::stream::{self, StreamExt};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

/// Chat request — includes session_id for multi-turn and attachments for file uploads
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub session_id: Option<String>,
    pub user_context: Option<serde_json::Value>,
    /// Upload IDs to attach to this message (images become vision content blocks)
    pub attachments: Option<Vec<String>>,
}

/// Chat response
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub session_id: String,
    pub message: String,
}

/// Cancel request
#[derive(Debug, Deserialize)]
pub struct CancelRequest {
    pub chat_id: String,
}

/// Query params for list_sessions
#[derive(Debug, Deserialize)]
pub struct ListSessionsQuery {
    pub limit: Option<i64>,
}

/// Build agent routes
pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/chat", post(chat_stream))
        .route("/chat/sync", post(chat_sync))
        .route("/chat/cancel", post(chat_cancel))
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session))
        .route("/sessions/{id}", delete(delete_session))
        .route("/ws/chat", get(ws_chat_handler))
}

/// SSE streaming chat endpoint with session persistence
pub async fn chat_stream(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    let user_context = req.user_context.unwrap_or(json!({}));
    let db_pool = state.db_pool.clone();

    // ── Resolve or create session ────────────────────────────────────────
    let (session_id, prior_messages, prior_count) = match &req.session_id {
        Some(sid) => {
            // Continuing an existing session
            match Uuid::parse_str(sid) {
                Ok(uuid) => {
                    match sessions::load_messages(&db_pool, uuid).await {
                        Ok(msgs) => {
                            let count = msgs.len() as i32;
                            (uuid, msgs, count)
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load session {sid}: {e}, creating new");
                            match sessions::create_session(&db_pool, None).await {
                                Ok(s) => (s.id, Vec::new(), 0),
                                Err(e) => {
                                    tracing::error!("Failed to create session: {e}");
                                    return (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(json!({"error": "Failed to create session"})),
                                    ).into_response();
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    tracing::warn!("Invalid session_id: {sid}, creating new");
                    match sessions::create_session(&db_pool, None).await {
                        Ok(s) => (s.id, Vec::new(), 0),
                        Err(e) => {
                            tracing::error!("Failed to create session: {e}");
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"error": "Failed to create session"})),
                            ).into_response();
                        }
                    }
                }
            }
        }
        None => {
            // First message — create a new session
            // Use first ~60 chars of the user message as a title
            let title: String = req.message.chars().take(60).collect();
            match sessions::create_session(&db_pool, Some(&title)).await {
                Ok(s) => (s.id, Vec::new(), 0),
                Err(e) => {
                    tracing::error!("Failed to create session: {e}");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": "Failed to create session"})),
                    ).into_response();
                }
            }
        }
    };

    // ── Create agent loop ────────────────────────────────────────────────
    let bedrock_client = match BedrockClient::new(None, None, None) {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("Failed to create Bedrock client: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to initialize Bedrock client: {}", e)})),
            ).into_response();
        }
    };
    let config = LoopConfig::default();
    let (mut agent_loop, cancel_flag) = AgentLoop::new(config, state.tool_registry.clone(), bedrock_client);

    // Pre-seed with prior conversation history
    if !prior_messages.is_empty() {
        agent_loop.set_conversation(prior_messages);
    }

    // Generate a unique chat_id for cancellation and store the flag
    let chat_id = Uuid::new_v4().to_string();
    state.active_chats.insert(chat_id.clone(), cancel_flag);

    // ── Resolve attachments into content blocks ─────────────────────────
    let mut attachment_blocks: Vec<amos_core::types::ContentBlock> = Vec::new();
    if let Some(ref attachment_ids) = req.attachments {
        for att_id_str in attachment_ids {
            if let Ok(att_uuid) = Uuid::parse_str(att_id_str) {
                match load_upload_data(&db_pool, &state.storage, att_uuid).await {
                    Ok((ct, filename, data)) => {
                        if ct.starts_with("image/") {
                            // Image → ContentBlock::Image with base64
                            let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                            attachment_blocks.push(amos_core::types::ContentBlock::Image {
                                source: amos_core::types::ImageSource {
                                    source_type: "base64".to_string(),
                                    media_type: ct,
                                    data: b64,
                                },
                            });
                        } else if is_text_content_type(&ct) {
                            // Text-based file → include actual content inline
                            match String::from_utf8(data.clone()) {
                                Ok(text_content) => {
                                    const MAX_TEXT_SIZE: usize = 100_000;
                                    let content = if text_content.len() > MAX_TEXT_SIZE {
                                        format!(
                                            "{}...\n[Truncated — showing first ~100KB of {} total bytes]",
                                            &text_content[..MAX_TEXT_SIZE],
                                            text_content.len()
                                        )
                                    } else {
                                        text_content
                                    };
                                    attachment_blocks.push(amos_core::types::ContentBlock::Text {
                                        text: format!(
                                            "=== Attached file: {} ({}) ===\n{}\n=== End of file ===",
                                            filename, ct, content
                                        ),
                                    });
                                }
                                Err(_) => {
                                    attachment_blocks.push(amos_core::types::ContentBlock::Text {
                                        text: format!(
                                            "[Attached binary file: {} ({}, {} bytes) — content not displayable as text]",
                                            filename, ct, data.len()
                                        ),
                                    });
                                }
                            }
                        } else {
                            // Try document extraction (PDF, DOCX, etc.)
                            let extraction = state.document_processor.extract(&data, &filename, &ct).await;
                            match extraction {
                                crate::documents::ExtractionResult::Text(text) => {
                                    const MAX_TEXT_SIZE: usize = 100_000;
                                    let content = if text.len() > MAX_TEXT_SIZE {
                                        format!(
                                            "{}...\n[Truncated — showing first ~100KB of {} total bytes]",
                                            &text[..MAX_TEXT_SIZE],
                                            text.len()
                                        )
                                    } else {
                                        text
                                    };
                                    attachment_blocks.push(amos_core::types::ContentBlock::Text {
                                        text: format!(
                                            "=== Attached file: {} ({}) ===\n{}\n=== End of file ===",
                                            filename, ct, content
                                        ),
                                    });
                                }
                                crate::documents::ExtractionResult::Pages(pages) => {
                                    let combined = pages.iter()
                                        .map(|p| format!("--- Page {} ---\n{}", p.page_number, p.text))
                                        .collect::<Vec<_>>()
                                        .join("\n\n");
                                    const MAX_TEXT_SIZE: usize = 100_000;
                                    let content = if combined.len() > MAX_TEXT_SIZE {
                                        format!(
                                            "{}...\n[Truncated — showing first ~100KB of {} total bytes]",
                                            &combined[..MAX_TEXT_SIZE],
                                            combined.len()
                                        )
                                    } else {
                                        combined
                                    };
                                    attachment_blocks.push(amos_core::types::ContentBlock::Text {
                                        text: format!(
                                            "=== Attached file: {} ({}, {} pages) ===\n{}\n=== End of file ===",
                                            filename, ct, pages.len(), content
                                        ),
                                    });
                                }
                                crate::documents::ExtractionResult::RenderedPages(images) => {
                                    // Scanned PDF → send page images to Claude Vision
                                    for (media_type, img_bytes) in &images {
                                        let b64 = base64::engine::general_purpose::STANDARD.encode(img_bytes);
                                        attachment_blocks.push(amos_core::types::ContentBlock::Image {
                                            source: amos_core::types::ImageSource {
                                                source_type: "base64".to_string(),
                                                media_type: media_type.clone(),
                                                data: b64,
                                            },
                                        });
                                    }
                                    attachment_blocks.push(amos_core::types::ContentBlock::Text {
                                        text: format!(
                                            "[Attached scanned document: {} — {} page image(s) included above for visual analysis]",
                                            filename, images.len()
                                        ),
                                    });
                                }
                                crate::documents::ExtractionResult::RawDocument(format, doc_name, raw_bytes) => {
                                    // Image-heavy or scanned PDF → send raw document to
                                    // Claude's native document content block for vision.
                                    let b64 = base64::engine::general_purpose::STANDARD.encode(&raw_bytes);
                                    tracing::info!(
                                        "Sending raw document '{}' ({} format, {} bytes) to Claude native document API",
                                        doc_name, format, raw_bytes.len()
                                    );
                                    attachment_blocks.push(amos_core::types::ContentBlock::Document {
                                        source: amos_core::types::DocumentSource {
                                            format,
                                            name: doc_name,
                                            data: b64,
                                        },
                                    });
                                    attachment_blocks.push(amos_core::types::ContentBlock::Text {
                                        text: format!(
                                            "[Attached document: {} — sent as native document for visual analysis]",
                                            filename
                                        ),
                                    });
                                }
                                crate::documents::ExtractionResult::Unsupported => {
                                    // Truly unknown binary format
                                    attachment_blocks.push(amos_core::types::ContentBlock::Text {
                                        text: format!(
                                            "[Attached binary file: {} ({}, {} bytes) — binary format, content not included]",
                                            filename, ct, data.len()
                                        ),
                                    });
                                }
                            }
                        }
                    }
                    Err(_) => {
                        tracing::warn!("Failed to load attachment {att_id_str}");
                    }
                }
            }
        }
    }

    // Subscribe to events before spawning
    let event_rx = agent_loop.subscribe();

    // Start agent in background — saves conversation after loop ends
    let message = req.message.clone();
    let active_chats = state.active_chats.clone();
    let chat_id_for_cleanup = chat_id.clone();
    let db_for_save = db_pool.clone();
    let sess_id = session_id;
    let prior_cnt = prior_count;

    tokio::spawn(async move {
        if let Err(e) = agent_loop.run_with_attachments(message, user_context, attachment_blocks).await {
            tracing::error!("Agent loop error: {:?}", e);
        }

        // ── Persist new messages ─────────────────────────────────────────
        let conversation = agent_loop.get_conversation();
        // Only save the NEW messages (after prior_cnt)
        let new_messages = &conversation[prior_cnt as usize..];
        if !new_messages.is_empty() {
            if let Err(e) = sessions::save_messages(&db_for_save, sess_id, new_messages, prior_cnt).await {
                tracing::error!("Failed to save messages for session {sess_id}: {e}");
            }

            // Touch session stats
            let _ = sessions::touch_session(
                &db_for_save,
                sess_id,
                new_messages.len() as i32,
                0, // TODO: track tokens when available
                0,
            ).await;
        }

        // Clean up the cancellation flag
        active_chats.remove(&chat_id_for_cleanup);
    });

    // ── Build SSE stream ─────────────────────────────────────────────────
    // First event contains both chat_id (for cancellation) and session_id (for continuity)
    let meta_event = Event::default()
        .event("chat_meta")
        .data(json!({
            "chat_id": chat_id,
            "session_id": session_id.to_string(),
        }).to_string());

    let broadcast_stream = BroadcastStream::new(event_rx);
    let agent_stream = stream::unfold(broadcast_stream, |mut stream| async move {
        match stream.next().await {
            Some(Ok(event)) => {
                let event_json = serde_json::to_string(&event).unwrap_or_default();
                Some((Ok::<_, axum::Error>(Event::default().data(event_json)), stream))
            }
            _ => None,
        }
    });

    let full_stream = stream::once(async move { Ok(meta_event) }).chain(agent_stream);

    Sse::new(full_stream).keep_alive(KeepAlive::default()).into_response()
}

/// Cancel a running chat
pub async fn chat_cancel(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CancelRequest>,
) -> impl IntoResponse {
    if let Some(flag) = state.active_chats.get(&req.chat_id) {
        flag.value().store(true, Ordering::Relaxed);
        tracing::info!("Cancelled chat {}", req.chat_id);
        (StatusCode::OK, Json(json!({"status": "cancelled", "chat_id": req.chat_id})))
    } else {
        (StatusCode::NOT_FOUND, Json(json!({"error": "Chat not found or already completed"})))
    }
}

/// Synchronous chat endpoint (also session-aware)
pub async fn chat_sync(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, StatusCode> {
    let user_context = req.user_context.unwrap_or(json!({}));
    let db_pool = state.db_pool.clone();

    // Resolve or create session
    let (session_id, prior_messages, prior_count) = match &req.session_id {
        Some(sid) => match Uuid::parse_str(sid) {
            Ok(uuid) => {
                let msgs = sessions::load_messages(&db_pool, uuid).await.unwrap_or_default();
                let count = msgs.len() as i32;
                (uuid, msgs, count)
            }
            Err(_) => {
                let s = sessions::create_session(&db_pool, None).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                (s.id, Vec::new(), 0)
            }
        },
        None => {
            let title: String = req.message.chars().take(60).collect();
            let s = sessions::create_session(&db_pool, Some(&title)).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            (s.id, Vec::new(), 0)
        }
    };

    let bedrock_client = BedrockClient::new(None, None, None)
        .map_err(|e| {
            tracing::error!("Failed to create Bedrock client: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let config = LoopConfig::default();
    let (mut agent_loop, _cancel_flag) = AgentLoop::new(config, state.tool_registry.clone(), bedrock_client);

    if !prior_messages.is_empty() {
        agent_loop.set_conversation(prior_messages);
    }

    let mut event_rx = agent_loop.subscribe();

    let message = req.message.clone();
    tokio::spawn(async move {
        if let Err(e) = agent_loop.run(message, user_context).await {
            tracing::error!("Agent loop error: {:?}", e);
        }

        // Persist new messages
        let conversation = agent_loop.get_conversation();
        let new_messages = &conversation[prior_count as usize..];
        if !new_messages.is_empty() {
            let _ = sessions::save_messages(&db_pool, session_id, new_messages, prior_count).await;
            let _ = sessions::touch_session(&db_pool, session_id, new_messages.len() as i32, 0, 0).await;
        }
    });

    // Collect final message
    let mut final_message = String::new();
    while let Ok(event) = event_rx.recv().await {
        match event {
            AgentEvent::MessageDelta { content } => {
                final_message.push_str(&content);
            }
            AgentEvent::AgentEnd { .. } => break,
            _ => {}
        }
    }

    Ok(Json(ChatResponse {
        session_id: session_id.to_string(),
        message: final_message,
    }))
}

/// Get session with full message history
pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let uuid = Uuid::parse_str(&session_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let session = sessions::get_session(&state.db_pool, uuid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let messages = sessions::load_messages(&state.db_pool, uuid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "session": session,
        "messages": messages,
    })))
}

/// List recent sessions for the sidebar
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListSessionsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = params.limit.unwrap_or(20);

    let sessions = sessions::list_sessions(&state.db_pool, limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "sessions": sessions })))
}

/// Delete a session
pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let uuid = Uuid::parse_str(&session_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let deleted = sessions::delete_session(&state.db_pool, uuid)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if deleted {
        Ok(Json(json!({"status": "deleted", "session_id": session_id})))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// WebSocket chat handler
pub async fn ws_chat_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

/// Handle WebSocket connection
async fn handle_websocket(mut socket: WebSocket, state: Arc<AppState>) {
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            // Parse message
            if let Ok(req) = serde_json::from_str::<ChatRequest>(&text) {
                let user_context = req.user_context.unwrap_or(json!({}));

                // Create agent loop
                let bedrock_client = match BedrockClient::new(None, None, None) {
                    Ok(client) => client,
                    Err(e) => {
                        tracing::error!("Failed to create Bedrock client: {:?}", e);
                        let error_msg = json!({"error": format!("Failed to initialize Bedrock client: {}", e)});
                        let _ = socket.send(Message::Text(error_msg.to_string().into())).await;
                        continue;
                    }
                };
                let config = LoopConfig::default();
                let (mut agent_loop, _cancel_flag) = AgentLoop::new(config, state.tool_registry.clone(), bedrock_client);

                let mut event_rx = agent_loop.subscribe();

                // Start agent
                let message = req.message.clone();
                tokio::spawn(async move {
                    if let Err(e) = agent_loop.run(message, user_context).await {
                        tracing::error!("Agent loop error: {:?}", e);
                    }
                });

                // Stream events back to client
                while let Ok(event) = event_rx.recv().await {
                    let event_json = serde_json::to_string(&event).unwrap_or_default();
                    if socket.send(Message::Text(event_json.into())).await.is_err() {
                        break;
                    }

                    // Break on agent end
                    if matches!(event, AgentEvent::AgentEnd { .. }) {
                        break;
                    }
                }
            }
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Check whether a MIME content type represents a text-based format
/// whose contents can be included inline as readable text.
fn is_text_content_type(ct: &str) -> bool {
    ct.starts_with("text/")
        || matches!(
            ct,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/typescript"
                | "application/x-yaml"
                | "application/yaml"
                | "application/toml"
                | "application/x-toml"
                | "application/sql"
                | "application/graphql"
                | "application/x-sh"
                | "application/xhtml+xml"
                | "application/ld+json"
                | "application/x-ndjson"
                | "application/csv"
                | "application/x-ruby"
                | "application/x-python"
                | "application/x-perl"
                | "application/x-httpd-php"
        )
}
