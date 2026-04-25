//! Metrics snapshot — the Oracle's view of relay state.
//!
//! Composes on-chain pool state with DB aggregations to produce
//! `amos_oracle::metrics::RelaySnapshot`. Zeroed fields where the underlying
//! data isn't available (e.g. if the Solana client is unconfigured) —
//! degrading gracefully so the Oracle's constitutional §4 zero-signal
//! weighting kicks in rather than a hard error.

use crate::{solana::DailyPoolState, state::RelayState};
use axum::{extract::State, response::Json, routing::get, Router};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use tracing::warn;

pub fn routes() -> Router<RelayState> {
    Router::new().route("/snapshot", get(snapshot))
}

#[derive(Debug, Serialize)]
struct RelaySnapshotResponse {
    taken_at: DateTime<Utc>,

    // Pool state
    daily_emission_remaining_points: u64,
    daily_pool_points_distributed: u64,
    growth_pool_cap_bps: u16,

    // Bounty lifecycle counts (rolling 7d)
    bounties_posted_7d: u32,
    bounties_claimed_7d: u32,
    bounties_settled_7d: u32,
    bounties_rejected_7d: u32,

    // Value flow (rolling 7d, in AMOS atomic units)
    commercial_volume_7d: u64,
    system_emission_7d: u64,

    // Agent activity
    active_agents_7d: u32,
    avg_quality_score_7d: f64,

    // Category mix (rolling 7d)
    category_counts_7d: BTreeMap<String, u32>,
}

async fn snapshot(State(state): State<RelayState>) -> Json<RelaySnapshotResponse> {
    let taken_at = Utc::now();

    // ── Pool state (on-chain) ──────────────────────────────────────────
    let (remaining_points, distributed_points) = match &state.solana {
        Some(solana) => {
            let day_index = solana
                .read_config_timing()
                .await
                .map(|(_, idx)| idx)
                .unwrap_or(0);
            let pool = solana
                .read_daily_pool(day_index)
                .await
                .ok()
                .flatten()
                .unwrap_or(DailyPoolState {
                    day_index,
                    daily_emission: 0,
                    tokens_distributed: 0,
                    total_points: 0,
                    proof_count: 0,
                });
            // `total_points` is the accumulated pool denominator; the remaining
            // pool in points is approximated by daily_emission - distributed
            // (in lamports), which isn't directly "points." For the Oracle's
            // purposes we expose the points figures it cares about.
            let distributed = pool.total_points;
            // Remaining is budget-available; we don't have an authoritative
            // "points remaining" on-chain. Report 0 when we can't compute and
            // let Oracle's §4 weighting handle it.
            (0u64, distributed)
        }
        None => {
            warn!("metrics/snapshot: Solana client unconfigured; pool fields = 0");
            (0, 0)
        }
    };

    // ── Bounty lifecycle counts over the last 7 days ───────────────────
    let seven_days_ago = taken_at - chrono::Duration::days(7);

    let posted_7d = count_bounties_since(&state, "created_at", seven_days_ago, None).await;
    let claimed_7d = count_bounties_since(&state, "claimed_at", seven_days_ago, None).await;
    let settled_7d = count_bounties_since(
        &state,
        "approved_at",
        seven_days_ago,
        Some("status = 'approved'"),
    )
    .await;
    let rejected_7d = count_bounties_since(
        &state,
        "rejected_at",
        seven_days_ago,
        Some("status = 'rejected'"),
    )
    .await;

    // Commercial volume = total reward_tokens on settled bounties in window.
    // "Commercial" here means all user-funded bounties (vs. system/treasury).
    // The relay doesn't currently distinguish commercial from system in a
    // dedicated column — we approximate via approved in window as proxy.
    // This is tracked as a follow-up refinement (real commercial tagging).
    let commercial_volume_7d: u64 = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT COALESCE(SUM(reward_tokens), 0)::bigint
        FROM relay_bounties
        WHERE approved_at >= $1
          AND status = 'approved'
        "#,
    )
    .bind(seven_days_ago)
    .fetch_one(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0)
    .max(0) as u64;

    // System emission proxy: same for now; real split requires a
    // `bounty_source` column (tracked as follow-up).
    let system_emission_7d = commercial_volume_7d;

    // Active agents = distinct agents that claimed OR submitted in window.
    let active_agents_7d: u32 = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT COUNT(DISTINCT claimed_by_agent_id)::bigint
        FROM relay_bounties
        WHERE claimed_at >= $1
          AND claimed_by_agent_id IS NOT NULL
        "#,
    )
    .bind(seven_days_ago)
    .fetch_one(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0)
    .max(0)
    .min(u32::MAX as i64) as u32;

    // Average quality score over approved bounties in window.
    let avg_quality_score_7d: f64 = sqlx::query_scalar::<_, Option<f64>>(
        r#"
        SELECT AVG(quality_score::double precision)
        FROM relay_bounties
        WHERE approved_at >= $1
          AND quality_score IS NOT NULL
        "#,
    )
    .bind(seven_days_ago)
    .fetch_one(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0.0);

    // Category mix.
    let category_counts_7d = fetch_category_counts(&state, seven_days_ago).await;

    Json(RelaySnapshotResponse {
        taken_at,
        daily_emission_remaining_points: remaining_points,
        daily_pool_points_distributed: distributed_points,
        growth_pool_cap_bps: 2000, // sigmoid ceiling; real on-chain read is follow-up
        bounties_posted_7d: posted_7d,
        bounties_claimed_7d: claimed_7d,
        bounties_settled_7d: settled_7d,
        bounties_rejected_7d: rejected_7d,
        commercial_volume_7d,
        system_emission_7d,
        active_agents_7d,
        avg_quality_score_7d,
        category_counts_7d,
    })
}

async fn count_bounties_since(
    state: &RelayState,
    column: &str,
    since: DateTime<Utc>,
    extra_predicate: Option<&str>,
) -> u32 {
    // Column names are caller-controlled and constant (never user input), so
    // format!-ing here is safe.
    let predicate = extra_predicate
        .map(|p| format!("AND {}", p))
        .unwrap_or_default();
    let sql = format!(
        "SELECT COUNT(*)::bigint FROM relay_bounties WHERE {} >= $1 {}",
        column, predicate
    );

    let count: i64 = sqlx::query_scalar::<_, Option<i64>>(&sql)
        .bind(since)
        .fetch_one(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or(0);
    count.max(0).min(u32::MAX as i64) as u32
}

async fn fetch_category_counts(state: &RelayState, since: DateTime<Utc>) -> BTreeMap<String, u32> {
    let rows = sqlx::query_as::<_, (Option<String>, i64)>(
        r#"
        SELECT category, COUNT(*)::bigint
        FROM relay_bounties
        WHERE created_at >= $1
        GROUP BY category
        "#,
    )
    .bind(since)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut out = BTreeMap::new();
    for (cat, n) in rows {
        let key = cat.unwrap_or_else(|| "uncategorized".to_string());
        let n_u32 = n.max(0).min(u32::MAX as i64) as u32;
        out.insert(key, n_u32);
    }
    out
}
