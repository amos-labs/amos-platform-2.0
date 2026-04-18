//! Automation monitoring routes: failed runs and dead-letter retry queue.
//!
//! Agents and the settings/failures canvas use these to surface automation
//! issues to the customer.
//!
//! Endpoints:
//!   - `GET /api/v1/automations/failures`     - Recent failed runs
//!   - `GET /api/v1/automations/dead-letters` - Retries that permanently failed
//!   - `POST /api/v1/automations/dead-letters/:id/requeue` - Reset a dead-letter for retry

use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/failures", get(list_failures))
        .route("/dead-letters", get(list_dead_letters))
        .route("/dead-letters/{id}/requeue", post(requeue_dead_letter))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    automation_id: Option<Uuid>,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct FailureRun {
    id: Uuid,
    automation_id: Uuid,
    automation_name: Option<String>,
    status: String,
    error: Option<String>,
    trigger_data: JsonValue,
    duration_ms: Option<i32>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct RetryEntry {
    id: Uuid,
    automation_id: Uuid,
    automation_name: Option<String>,
    action_type: String,
    attempt: i32,
    max_attempts: i32,
    next_attempt_at: DateTime<Utc>,
    last_error: Option<String>,
    status: String,
    trigger_data: JsonValue,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

async fn list_failures(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<FailureRun>>, StatusCode> {
    let limit = q.limit.clamp(1, 500);

    let rows = if let Some(auto_id) = q.automation_id {
        sqlx::query_as::<_, FailureRun>(
            r#"SELECT r.id, r.automation_id, a.name AS automation_name, r.status,
                      r.error, r.trigger_data, r.duration_ms, r.created_at
                 FROM automation_runs r
                 LEFT JOIN automations a ON a.id = r.automation_id
                WHERE r.status = 'error' AND r.automation_id = $1
             ORDER BY r.created_at DESC
                LIMIT $2"#,
        )
        .bind(auto_id)
        .bind(limit)
        .fetch_all(&state.db_pool)
        .await
    } else {
        sqlx::query_as::<_, FailureRun>(
            r#"SELECT r.id, r.automation_id, a.name AS automation_name, r.status,
                      r.error, r.trigger_data, r.duration_ms, r.created_at
                 FROM automation_runs r
                 LEFT JOIN automations a ON a.id = r.automation_id
                WHERE r.status = 'error'
             ORDER BY r.created_at DESC
                LIMIT $1"#,
        )
        .bind(limit)
        .fetch_all(&state.db_pool)
        .await
    }
    .map_err(|e| {
        tracing::error!("Failed to query automation failures: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}

async fn list_dead_letters(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<RetryEntry>>, StatusCode> {
    let limit = q.limit.clamp(1, 500);

    let rows = sqlx::query_as::<_, RetryEntry>(
        r#"SELECT rq.id, rq.automation_id, a.name AS automation_name, rq.action_type,
                  rq.attempt, rq.max_attempts, rq.next_attempt_at, rq.last_error,
                  rq.status, rq.trigger_data, rq.created_at, rq.updated_at
             FROM automation_retry_queue rq
             LEFT JOIN automations a ON a.id = rq.automation_id
            WHERE rq.status = 'dead_letter'
         ORDER BY rq.updated_at DESC
            LIMIT $1"#,
    )
    .bind(limit)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to query dead-letter queue: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows))
}

async fn requeue_dead_letter(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result = sqlx::query(
        r#"UPDATE automation_retry_queue
              SET status = 'pending', attempt = 1, next_attempt_at = NOW(),
                  updated_at = NOW()
            WHERE id = $1 AND status = 'dead_letter'"#,
    )
    .bind(id)
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to requeue dead-letter: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(serde_json::json!({ "requeued": true, "id": id })))
}
