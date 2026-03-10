//! Health check and API discovery endpoints

use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::state::AppState;

/// Root API catalog
pub async fn api_catalog() -> Json<Value> {
    Json(json!({
        "service": "amos-harness",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "ok",
        "endpoints": {
            "health":        "GET  /health",
            "ready":         "GET  /ready",
            "agent_chat":    "POST /api/v1/agent/chat",
            "agent_sync":    "POST /api/v1/agent/chat/sync",
            "sessions":      "GET  /api/v1/agent/sessions",
            "canvases":      "GET  /api/v1/canvases",
            "agents":        "GET  /api/v1/agents",
            "uploads":       "GET  /api/v1/uploads",
            "integrations":  "GET  /api/v1/integrations",
            "sites":         "GET  /api/v1/sites",
        }
    }))
}

/// Liveness check (always returns OK - proves the process is running)
pub async fn health_check() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION")
        })),
    )
}

/// Readiness check (verifies DB and Redis connectivity)
pub async fn readiness_check(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<Value>) {
    let mut db_status = "ok".to_string();
    let mut redis_status = "ok".to_string();
    let mut overall = "ready";

    // Check PostgreSQL
    match sqlx::query("SELECT 1").execute(&state.db_pool).await {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Database health check failed: {}", e);
            db_status = format!("error: {}", e);
            overall = "not_ready";
        }
    }

    // Check Redis
    match state.redis.get_connection() {
        Ok(mut conn) => {
            use redis::Commands;
            match conn.get::<&str, Option<String>>("__health__") {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("Redis health check failed: {}", e);
                    redis_status = format!("error: {}", e);
                    overall = "not_ready";
                }
            }
        }
        Err(e) => {
            tracing::error!("Redis connection failed: {}", e);
            redis_status = format!("error: {}", e);
            overall = "not_ready";
        }
    }

    let status_code = if overall == "ready" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(json!({
            "status": overall,
            "version": env!("CARGO_PKG_VERSION"),
            "checks": {
                "database": db_status,
                "redis": redis_status
            }
        })),
    )
}
