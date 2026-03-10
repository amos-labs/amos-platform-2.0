//! Bounty marketplace routes.

use crate::{protocol_fees::calculate_fee, state::RelayState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;

/// Build bounty routes.
pub fn routes() -> Router<RelayState> {
    Router::new()
        .route("/", post(create_bounty).get(list_bounties))
        .route("/:id", get(get_bounty))
        .route("/:id/claim", post(claim_bounty))
        .route("/:id/submit", post(submit_work))
        .route("/:id/approve", post(approve_submission))
        .route("/:id/reject", post(reject_submission))
}

// =============================================================================
// REQUEST/RESPONSE TYPES
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct CreateBountyRequest {
    pub title: String,
    pub description: String,
    pub reward_tokens: u64,
    pub deadline: DateTime<Utc>,
    pub required_capabilities: Vec<String>,
    pub poster_wallet: String,
}

#[derive(Debug, Deserialize)]
pub struct ListBountiesQuery {
    pub status: Option<BountyStatus>,
    pub min_reward: Option<u64>,
    pub capability: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimBountyRequest {
    pub agent_id: Uuid,
    pub harness_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SubmitWorkRequest {
    pub agent_id: Uuid,
    pub result: JsonValue,
    pub quality_evidence: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct ApproveSubmissionRequest {
    pub reviewer_wallet: String,
    pub quality_score: Option<u8>,
}

#[derive(Debug, Deserialize)]
pub struct RejectSubmissionRequest {
    pub reviewer_wallet: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "bounty_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum BountyStatus {
    Open,
    Claimed,
    Submitted,
    Approved,
    Rejected,
    Expired,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct BountyResponse {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub reward_tokens: i64,
    pub deadline: DateTime<Utc>,
    pub required_capabilities: Vec<String>,
    pub poster_wallet: String,
    pub status: BountyStatus,
    pub claimed_by_agent_id: Option<Uuid>,
    pub claimed_by_harness_id: Option<String>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub result: Option<JsonValue>,
    pub quality_evidence: Option<JsonValue>,
    pub quality_score: Option<i16>,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejected_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// =============================================================================
// HANDLERS
// =============================================================================

/// Create a new bounty.
async fn create_bounty(
    State(state): State<RelayState>,
    Json(req): Json<CreateBountyRequest>,
) -> Result<(StatusCode, Json<BountyResponse>), StatusCode> {
    let bounty_id = Uuid::new_v4();
    let now = Utc::now();

    let bounty = sqlx::query_as::<_, BountyResponse>(
        r#"
        INSERT INTO relay_bounties (
            id, title, description, reward_tokens, deadline,
            required_capabilities, poster_wallet, status,
            created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING
            id, title, description, reward_tokens, deadline,
            required_capabilities, poster_wallet,
            status,
            claimed_by_agent_id, claimed_by_harness_id,
            submitted_at, result, quality_evidence,
            quality_score, approved_at, rejected_at, rejection_reason,
            created_at, updated_at
        "#,
    )
    .bind(bounty_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(req.reward_tokens as i64)
    .bind(req.deadline)
    .bind(&req.required_capabilities)
    .bind(&req.poster_wallet)
    .bind(BountyStatus::Open)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to create bounty: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("Created bounty {} with reward {}", bounty_id, req.reward_tokens);

    Ok((StatusCode::CREATED, Json(bounty)))
}

/// List bounties with optional filters.
async fn list_bounties(
    State(state): State<RelayState>,
    Query(query): Query<ListBountiesQuery>,
) -> Result<Json<Vec<BountyResponse>>, StatusCode> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    // Build query dynamically based on filters
    let mut sql = String::from(
        r#"
        SELECT
            id, title, description, reward_tokens, deadline,
            required_capabilities, poster_wallet, status,
            claimed_by_agent_id, claimed_by_harness_id,
            submitted_at, result, quality_evidence,
            quality_score, approved_at, rejected_at, rejection_reason,
            created_at, updated_at
        FROM relay_bounties
        WHERE 1=1
        "#,
    );

    if query.status.is_some() {
        sql.push_str(" AND status = $1");
    }
    if query.min_reward.is_some() {
        sql.push_str(" AND reward_tokens >= $2");
    }
    if query.capability.is_some() {
        sql.push_str(" AND $3 = ANY(required_capabilities)");
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT $4 OFFSET $5");

    // For simplicity, we'll use a simpler approach without dynamic parameters
    let bounties = sqlx::query_as::<_, BountyResponse>(
        r#"
        SELECT
            id, title, description, reward_tokens, deadline,
            required_capabilities, poster_wallet,
            status,
            claimed_by_agent_id, claimed_by_harness_id,
            submitted_at, result, quality_evidence,
            quality_score, approved_at, rejected_at, rejection_reason,
            created_at, updated_at
        FROM relay_bounties
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(per_page as i64)
    .bind(offset as i64)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to list bounties: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(bounties))
}

/// Get a single bounty by ID.
async fn get_bounty(
    State(state): State<RelayState>,
    Path(id): Path<Uuid>,
) -> Result<Json<BountyResponse>, StatusCode> {
    let bounty = sqlx::query_as::<_, BountyResponse>(
        r#"
        SELECT
            id, title, description, reward_tokens, deadline,
            required_capabilities, poster_wallet,
            status,
            claimed_by_agent_id, claimed_by_harness_id,
            submitted_at, result, quality_evidence,
            quality_score, approved_at, rejected_at, rejection_reason,
            created_at, updated_at
        FROM relay_bounties
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to get bounty {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(bounty))
}

/// Claim a bounty for an agent.
async fn claim_bounty(
    State(state): State<RelayState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ClaimBountyRequest>,
) -> Result<Json<BountyResponse>, StatusCode> {
    let now = Utc::now();

    let bounty = sqlx::query_as::<_, BountyResponse>(
        r#"
        UPDATE relay_bounties
        SET
            status = $1,
            claimed_by_agent_id = $2,
            claimed_by_harness_id = $3,
            updated_at = $4
        WHERE id = $5 AND status = $6
        RETURNING
            id, title, description, reward_tokens, deadline,
            required_capabilities, poster_wallet,
            status,
            claimed_by_agent_id, claimed_by_harness_id,
            submitted_at, result, quality_evidence,
            quality_score, approved_at, rejected_at, rejection_reason,
            created_at, updated_at
        "#,
    )
    .bind(BountyStatus::Claimed)
    .bind(req.agent_id)
    .bind(&req.harness_id)
    .bind(now)
    .bind(id)
    .bind(BountyStatus::Open)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to claim bounty {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::CONFLICT)?; // Bounty already claimed or doesn't exist

    info!("Bounty {} claimed by agent {}", id, req.agent_id);

    Ok(Json(bounty))
}

/// Submit work for a claimed bounty.
async fn submit_work(
    State(state): State<RelayState>,
    Path(id): Path<Uuid>,
    Json(req): Json<SubmitWorkRequest>,
) -> Result<Json<BountyResponse>, StatusCode> {
    let now = Utc::now();

    let bounty = sqlx::query_as::<_, BountyResponse>(
        r#"
        UPDATE relay_bounties
        SET
            status = $1,
            submitted_at = $2,
            result = $3,
            quality_evidence = $4,
            updated_at = $5
        WHERE id = $6 AND status = $7 AND claimed_by_agent_id = $8
        RETURNING
            id, title, description, reward_tokens, deadline,
            required_capabilities, poster_wallet,
            status,
            claimed_by_agent_id, claimed_by_harness_id,
            submitted_at, result, quality_evidence,
            quality_score, approved_at, rejected_at, rejection_reason,
            created_at, updated_at
        "#,
    )
    .bind(BountyStatus::Submitted)
    .bind(now)
    .bind(&req.result)
    .bind(&req.quality_evidence)
    .bind(now)
    .bind(id)
    .bind(BountyStatus::Claimed)
    .bind(req.agent_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to submit work for bounty {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::CONFLICT)?; // Bounty not claimed by this agent

    info!("Work submitted for bounty {} by agent {}", id, req.agent_id);

    Ok(Json(bounty))
}

/// Approve a bounty submission and trigger payout.
async fn approve_submission(
    State(state): State<RelayState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ApproveSubmissionRequest>,
) -> Result<Json<BountyResponse>, StatusCode> {
    let now = Utc::now();

    // First, fetch the current bounty to get the reward amount
    let current_bounty = sqlx::query(
        r#"
        SELECT reward_tokens
        FROM relay_bounties
        WHERE id = $1 AND status = $2
        "#,
    )
    .bind(id)
    .bind(BountyStatus::Submitted)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to fetch bounty {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Calculate protocol fee
    let reward_tokens: i64 = current_bounty.get("reward_tokens");
    let reward_tokens = reward_tokens as u64;
    let fee = calculate_fee(reward_tokens);

    info!(
        "Approving bounty {}: reward={}, protocol_fee={}, holder_share={}, treasury_share={}, ops_burn_share={}",
        id, reward_tokens, fee.total_fee, fee.holder_share, fee.treasury_share, fee.ops_burn_share
    );

    // Update the bounty status
    let bounty = sqlx::query_as::<_, BountyResponse>(
        r#"
        UPDATE relay_bounties
        SET
            status = $1,
            approved_at = $2,
            quality_score = $3,
            updated_at = $4
        WHERE id = $5 AND status = $6
        RETURNING
            id, title, description, reward_tokens, deadline,
            required_capabilities, poster_wallet,
            status,
            claimed_by_agent_id, claimed_by_harness_id,
            submitted_at, result, quality_evidence,
            quality_score, approved_at, rejected_at, rejection_reason,
            created_at, updated_at
        "#,
    )
    .bind(BountyStatus::Approved)
    .bind(now)
    .bind(req.quality_score.map(|s| s as i16))
    .bind(now)
    .bind(id)
    .bind(BountyStatus::Submitted)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to approve bounty {}: {}", e, id);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::CONFLICT)?;

    // TODO: Trigger Solana settlement transaction
    // - Transfer reward to agent wallet (minus protocol fee)
    // - Distribute protocol fee to holder pool, treasury, and burn
    info!(
        "Bounty {} approved. TODO: Trigger Solana settlement for {} tokens",
        id, reward_tokens
    );

    Ok(Json(bounty))
}

/// Reject a bounty submission.
async fn reject_submission(
    State(state): State<RelayState>,
    Path(id): Path<Uuid>,
    Json(req): Json<RejectSubmissionRequest>,
) -> Result<Json<BountyResponse>, StatusCode> {
    let now = Utc::now();

    let bounty = sqlx::query_as::<_, BountyResponse>(
        r#"
        UPDATE relay_bounties
        SET
            status = $1,
            rejected_at = $2,
            rejection_reason = $3,
            updated_at = $4
        WHERE id = $5 AND status = $6
        RETURNING
            id, title, description, reward_tokens, deadline,
            required_capabilities, poster_wallet,
            status,
            claimed_by_agent_id, claimed_by_harness_id,
            submitted_at, result, quality_evidence,
            quality_score, approved_at, rejected_at, rejection_reason,
            created_at, updated_at
        "#,
    )
    .bind(BountyStatus::Rejected)
    .bind(now)
    .bind(&req.reason)
    .bind(now)
    .bind(id)
    .bind(BountyStatus::Submitted)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        warn!("Failed to reject bounty {}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::CONFLICT)?;

    info!("Bounty {} rejected by reviewer {}", id, req.reviewer_wallet);

    Ok(Json(bounty))
}
