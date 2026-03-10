//! Health check routes.

use crate::{state::RelayState, VERSION};
use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde_json::{json, Value};

/// Build health check routes.
pub fn routes() -> Router<RelayState> {
    Router::new().route("/", get(health_check))
}

/// Simple health check handler.
async fn health_check(State(state): State<RelayState>) -> Result<Json<Value>, StatusCode> {
    // Perform a deep health check
    match state.health_check().await {
        Ok(_) => Ok(Json(json!({
            "status": "ok",
            "version": VERSION,
            "service": "amos-relay"
        }))),
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}
