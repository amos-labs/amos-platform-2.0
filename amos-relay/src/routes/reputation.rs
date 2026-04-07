//! Cross-harness reputation oracle routes.

use crate::{reputation::ReputationEngine, state::RelayState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

/// Build reputation routes.
pub fn routes() -> Router<RelayState> {
    Router::new()
        .route("/{agent_id}", get(get_reputation))
        .route("/report", post(report_outcome))
}

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct ReportOutcomeRequest {
    pub harness_id: String,
    pub agent_id: Uuid,
    pub task_id: String,
    pub outcome: TaskOutcome,
    pub quality_score: Option<u8>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "task_outcome", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TaskOutcome {
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReputationResponse {
    pub agent_id: Uuid,
    pub trust_level: u8,
    pub completion_rate: f64,
    pub quality_score: f64,
    pub total_completed: i32,
    pub total_failed: i32,
    pub total_tasks: i32,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct OutcomeReportResponse {
    pub id: Uuid,
    pub harness_id: String,
    pub agent_id: Uuid,
    pub task_id: String,
    pub outcome: TaskOutcome,
    pub quality_score: Option<i16>,
    pub reported_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct OutcomeRow {
    outcome: TaskOutcome,
    quality_score: Option<i16>,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// Get reputation metrics for an agent.
async fn get_reputation(
    State(state): State<RelayState>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<ReputationResponse>, StatusCode> {
    // Fetch all outcome reports for this agent
    let reports = sqlx::query_as::<_, OutcomeRow>(
        r#"
        SELECT
            outcome,
            quality_score
        FROM relay_reputation_reports
        WHERE agent_id = $1
        "#,
    )
    .bind(agent_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        warn!(
            "Failed to fetch reputation reports for agent {}: {}",
            agent_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total_tasks = reports.len() as i32;
    let total_completed = reports
        .iter()
        .filter(|r| matches!(r.outcome, TaskOutcome::Completed))
        .count() as i32;
    let total_failed = reports
        .iter()
        .filter(|r| matches!(r.outcome, TaskOutcome::Failed))
        .count() as i32;

    let completion_rate = if total_tasks > 0 {
        (total_completed as f64) / (total_tasks as f64)
    } else {
        0.0
    };

    // Calculate average quality score (only from completed tasks)
    let quality_scores: Vec<i16> = reports
        .iter()
        .filter(|r| matches!(r.outcome, TaskOutcome::Completed))
        .filter_map(|r| r.quality_score)
        .collect();

    let avg_quality = if !quality_scores.is_empty() {
        quality_scores.iter().sum::<i16>() as f64 / quality_scores.len() as f64
    } else {
        0.0
    };

    // Compute trust level using the reputation engine
    let trust_level = ReputationEngine::compute_trust_level(
        total_completed as u32,
        total_failed as u32,
        avg_quality,
    );

    Ok(Json(ReputationResponse {
        agent_id,
        trust_level,
        completion_rate,
        quality_score: avg_quality,
        total_completed,
        total_failed,
        total_tasks,
    }))
}

/// Report a task outcome from a harness.
async fn report_outcome(
    State(state): State<RelayState>,
    Json(req): Json<ReportOutcomeRequest>,
) -> Result<(StatusCode, Json<OutcomeReportResponse>), StatusCode> {
    let report_id = Uuid::new_v4();
    let now = Utc::now();

    let report = sqlx::query_as::<_, OutcomeReportResponse>(
        r#"
        INSERT INTO relay_reputation_reports (
            id, harness_id, agent_id, task_id, outcome,
            quality_score, reported_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING
            id, harness_id, agent_id, task_id,
            outcome,
            quality_score, reported_at
        "#,
    )
    .bind(report_id)
    .bind(&req.harness_id)
    .bind(req.agent_id)
    .bind(&req.task_id)
    .bind(req.outcome)
    .bind(req.quality_score.map(|s| s as i16))
    .bind(now)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to report outcome: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Reported {:?} outcome for agent {} on task {} from harness {}",
        req.outcome, req.agent_id, req.task_id, req.harness_id
    );

    // Update agent's aggregated stats
    let _ = update_agent_stats(&state, req.agent_id).await;

    Ok((StatusCode::CREATED, Json(report)))
}

/// Helper function to update agent's cached reputation stats.
async fn update_agent_stats(state: &RelayState, agent_id: Uuid) -> Result<(), StatusCode> {
    // Fetch all reports for this agent
    let reports = sqlx::query_as::<_, OutcomeRow>(
        r#"
        SELECT
            outcome,
            quality_score
        FROM relay_reputation_reports
        WHERE agent_id = $1
        "#,
    )
    .bind(agent_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_completed = reports
        .iter()
        .filter(|r| matches!(r.outcome, TaskOutcome::Completed))
        .count() as i32;

    let total_failed = reports
        .iter()
        .filter(|r| matches!(r.outcome, TaskOutcome::Failed))
        .count() as i32;

    let quality_scores: Vec<i16> = reports
        .iter()
        .filter(|r| matches!(r.outcome, TaskOutcome::Completed))
        .filter_map(|r| r.quality_score)
        .collect();

    let avg_quality = if !quality_scores.is_empty() {
        Some(quality_scores.iter().sum::<i16>() as f64 / quality_scores.len() as f64)
    } else {
        None
    };

    // Compute trust level
    let trust_level = ReputationEngine::compute_trust_level(
        total_completed as u32,
        total_failed as u32,
        avg_quality.unwrap_or(0.0),
    );

    // Update the agent record
    sqlx::query(
        r#"
        UPDATE relay_agents
        SET
            total_bounties_completed = $1,
            avg_quality_score = $2,
            trust_level = $3
        WHERE id = $4
        "#,
    )
    .bind(total_completed)
    .bind(avg_quality)
    .bind(trust_level as i16)
    .bind(agent_id)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}
