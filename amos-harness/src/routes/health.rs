//! Health check endpoints

use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

/// Health check endpoint
pub async fn health_check() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION")
        })),
    )
}

/// Readiness check endpoint
pub async fn readiness_check() -> (StatusCode, Json<Value>) {
    // TODO: Check database and Redis connectivity
    (
        StatusCode::OK,
        Json(json!({
            "status": "ready",
            "checks": {
                "database": "ok",
                "redis": "ok"
            }
        })),
    )
}
