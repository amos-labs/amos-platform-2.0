//! Escalation queue — where Oracle decisions go when the Oracle declines to
//! self-authorize (low confidence, above ceiling, novel territory, reasoning-
//! substrate touching, etc.). Council pulls from this queue, resolves with a
//! verdict + reasoning, and the resolution joins back to the original
//! decision via `oracle_outcomes`.
//!
//! All endpoints under this router require Bearer-token auth.

use crate::state::RelayState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::warn;
use uuid::Uuid;

pub fn routes() -> Router<RelayState> {
    Router::new()
        .route("/", post(create_escalation).get(list_escalations))
        .route("/{id}/resolve", post(resolve_escalation))
}

// ─── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateEscalationRequest {
    pub decision_id: Uuid,
    pub path: String, // "intake" or "review"
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct EscalationResponse {
    pub escalation_id: Uuid,
    pub decision_id: Uuid,
    pub path: String,
    pub reason: String,
    pub status: String,
    pub council_verdict: Option<String>,
    pub council_reasoning: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListEscalationsQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ResolveEscalationRequest {
    pub council_verdict: String,
    pub council_reasoning: String,
    pub resolved_by: String,
}

// ─── Handlers ────────────────────────────────────────────────────────────

async fn create_escalation(
    State(state): State<RelayState>,
    Json(req): Json<CreateEscalationRequest>,
) -> Result<(StatusCode, Json<EscalationResponse>), StatusCode> {
    if req.path != "intake" && req.path != "review" {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.reason.trim().is_empty() || req.reason.len() > 10_000 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Verify the decision exists before linking.
    let exists: Option<Uuid> =
        sqlx::query_scalar("SELECT decision_id FROM oracle_decisions WHERE decision_id = $1")
            .bind(req.decision_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| {
                warn!(error = %e, "create_escalation: decision lookup failed");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    if exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let row = sqlx::query(
        r#"
        INSERT INTO oracle_escalations (decision_id, path, reason)
        VALUES ($1, $2, $3)
        RETURNING escalation_id, decision_id, path, reason, status,
                  council_verdict, council_reasoning, resolved_by,
                  resolved_at, created_at
        "#,
    )
    .bind(req.decision_id)
    .bind(&req.path)
    .bind(&req.reason)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        warn!(error = %e, "create_escalation insert failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((StatusCode::CREATED, Json(escalation_from_row(row))))
}

async fn list_escalations(
    State(state): State<RelayState>,
    Query(q): Query<ListEscalationsQuery>,
) -> Result<Json<Vec<EscalationResponse>>, StatusCode> {
    let limit = q.limit.unwrap_or(100).clamp(1, 500);

    let rows = match q.status.as_deref() {
        Some(s) if s == "pending" || s == "resolved" => {
            sqlx::query(
                r#"
                SELECT escalation_id, decision_id, path, reason, status,
                       council_verdict, council_reasoning, resolved_by,
                       resolved_at, created_at
                FROM oracle_escalations
                WHERE status = $1
                ORDER BY created_at ASC
                LIMIT $2
                "#,
            )
            .bind(s)
            .bind(limit)
            .fetch_all(&state.db)
            .await
        }
        _ => {
            sqlx::query(
                r#"
                SELECT escalation_id, decision_id, path, reason, status,
                       council_verdict, council_reasoning, resolved_by,
                       resolved_at, created_at
                FROM oracle_escalations
                ORDER BY created_at DESC
                LIMIT $1
                "#,
            )
            .bind(limit)
            .fetch_all(&state.db)
            .await
        }
    }
    .map_err(|e| {
        warn!(error = %e, "list_escalations query failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(rows.into_iter().map(escalation_from_row).collect()))
}

async fn resolve_escalation(
    State(state): State<RelayState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ResolveEscalationRequest>,
) -> Result<Json<EscalationResponse>, StatusCode> {
    if req.council_reasoning.trim().is_empty() || req.council_reasoning.len() > 10_000 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.resolved_by.trim().is_empty() || req.resolved_by.len() > 128 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = sqlx::query(
        r#"
        UPDATE oracle_escalations
        SET status = 'resolved',
            council_verdict = $2,
            council_reasoning = $3,
            resolved_by = $4,
            resolved_at = now()
        WHERE escalation_id = $1
          AND status = 'pending'
        RETURNING escalation_id, decision_id, path, reason, status,
                  council_verdict, council_reasoning, resolved_by,
                  resolved_at, created_at
        "#,
    )
    .bind(id)
    .bind(&req.council_verdict)
    .bind(&req.council_reasoning)
    .bind(&req.resolved_by)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!(error = %e, "resolve_escalation update failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match row {
        None => Err(StatusCode::CONFLICT), // already resolved or not found
        Some(row) => Ok(Json(escalation_from_row(row))),
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────

fn escalation_from_row(row: sqlx::postgres::PgRow) -> EscalationResponse {
    EscalationResponse {
        escalation_id: row.try_get("escalation_id").unwrap_or_else(|_| Uuid::nil()),
        decision_id: row.try_get("decision_id").unwrap_or_else(|_| Uuid::nil()),
        path: row.try_get("path").unwrap_or_default(),
        reason: row.try_get("reason").unwrap_or_default(),
        status: row.try_get("status").unwrap_or_default(),
        council_verdict: row.try_get("council_verdict").ok(),
        council_reasoning: row.try_get("council_reasoning").ok(),
        resolved_by: row.try_get("resolved_by").ok(),
        resolved_at: row.try_get("resolved_at").ok(),
        created_at: row.try_get("created_at").unwrap_or_else(|_| Utc::now()),
    }
}
