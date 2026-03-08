use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;

/// Basic health check endpoint
/// Returns 200 OK with simple status message
pub async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok"
    }))
}

/// Readiness check endpoint
/// Verifies database and Redis connectivity
/// Returns 200 if all dependencies are healthy, 503 otherwise
pub async fn readiness_check(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut errors = Vec::new();

    // Check database connection
    match state.check_db_health().await {
        Ok(_) => tracing::debug!("Database health check passed"),
        Err(e) => {
            tracing::error!("Database health check failed: {}", e);
            errors.push(format!("database: {}", e));
        }
    }

    // Check Redis connection
    match state.check_redis_health().await {
        Ok(_) => tracing::debug!("Redis health check passed"),
        Err(e) => {
            tracing::error!("Redis health check failed: {}", e);
            errors.push(format!("redis: {}", e));
        }
    }

    if errors.is_empty() {
        Ok(Json(json!({
            "status": "ready",
            "checks": {
                "database": "ok",
                "redis": "ok"
            }
        })))
    } else {
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "not_ready",
                "errors": errors
            })),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
