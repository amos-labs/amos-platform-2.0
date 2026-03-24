//! Webhook ingress route for automation triggers.
//!
//! Receives incoming webhooks at `/api/v1/hooks/{path}` and fires matching
//! automations with the request body as trigger data.

use crate::automations::{TriggerEvent, TriggerType};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new().route("/{path}", post(receive_webhook))
}

async fn receive_webhook(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    Json(body): Json<JsonValue>,
) -> impl IntoResponse {
    // Look up matching webhook automations
    let rows = match sqlx::query(
        r#"SELECT id FROM automations
           WHERE trigger_type = 'webhook'
             AND enabled = true
             AND trigger_config->>'path' = $1"#,
    )
    .bind(&path)
    .fetch_all(&state.db_pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("Failed to look up webhook automations: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Internal error" })),
            );
        }
    };

    if rows.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("No automation found for webhook path '{}'", path) })),
        );
    }

    // Fire event
    let event = TriggerEvent {
        event_type: TriggerType::Webhook,
        collection: None,
        record_id: None,
        data: json!({
            "webhook_path": path,
            "body": body,
        }),
    };

    state.automation_engine.fire_event(event).await;

    (StatusCode::OK, Json(json!({ "accepted": true })))
}
