//! Scorecard collector — periodically computes and persists rolling
//! performance snapshots for every agent in a swarm.
//!
//! A [`Scorecard`] captures the fitness score, task-level statistics, cost
//! data, and per-metric breakdowns at a point in time. The collector is
//! intended to be invoked on a schedule (e.g. via a background task) and
//! also updates the denormalized `fitness_score` column on
//! `agent_swarm_members` so that routing decisions can use the latest value
//! without recomputing.

use crate::fitness::FitnessEngine;
use crate::types::Scorecard;
use amos_core::Result;
use chrono::{Duration, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Collects scorecards for agents within swarms.
pub struct ScorecardCollector {
    db_pool: PgPool,
    http_client: reqwest::Client,
}

impl ScorecardCollector {
    /// Create a new `ScorecardCollector`.
    pub fn new(db_pool: PgPool, http_client: reqwest::Client) -> Self {
        Self {
            db_pool,
            http_client,
        }
    }

    /// Compute and persist scorecards for every agent in the given swarm.
    ///
    /// Returns the freshly-created scorecards ordered by fitness (descending).
    pub async fn collect_scorecards(
        &self,
        swarm_id: Uuid,
        window_days: i32,
    ) -> Result<Vec<Scorecard>> {
        let engine = FitnessEngine::new(self.db_pool.clone(), self.http_client.clone());

        // Fetch all members of the swarm.
        let members =
            sqlx::query("SELECT agent_id, weight FROM agent_swarm_members WHERE swarm_id = $1")
                .bind(swarm_id)
                .fetch_all(&self.db_pool)
                .await?;

        let window_start = Utc::now() - Duration::days(window_days as i64);
        let window_end = Utc::now();

        let mut scorecards = Vec::with_capacity(members.len());

        for member_row in &members {
            let agent_id: i32 = member_row.get("agent_id");
            let weight: f64 = member_row.get("weight");

            // 1. Compute composite fitness score.
            let fitness_score = engine
                .compute_agent_fitness(swarm_id, agent_id)
                .await
                .unwrap_or_else(|e| {
                    tracing::error!(agent_id, %swarm_id, error = %e, "Fitness computation failed");
                    0.0
                });

            // 2. Gather task attribution stats for the window.
            let stats_row = sqlx::query(
                r#"
                SELECT
                    COUNT(*) FILTER (WHERE quality_score IS NOT NULL) AS completed,
                    COUNT(*) FILTER (WHERE quality_score IS NULL)     AS failed,
                    AVG(duration_ms)::bigint                          AS avg_duration_ms,
                    COALESCE(SUM(tokens_used), 0)                     AS total_tokens,
                    COALESCE(SUM(cost_usd), 0.0)                      AS total_cost
                FROM agent_task_attribution
                WHERE agent_id = $1
                  AND swarm_id = $2
                  AND created_at >= $3
                "#,
            )
            .bind(agent_id)
            .bind(swarm_id)
            .bind(window_start)
            .fetch_one(&self.db_pool)
            .await?;

            let tasks_completed: i64 = stats_row.get("completed");
            let tasks_failed: i64 = stats_row.get("failed");
            let avg_task_duration_ms: Option<i64> = stats_row.get("avg_duration_ms");
            let total_tokens_used: i64 = stats_row.get("total_tokens");
            let total_cost_usd: f64 = stats_row.get("total_cost");

            // 3. Build per-metric breakdown for the scorecard.
            let functions = engine.list_functions(swarm_id).await.unwrap_or_default();
            let mut metric_scores = serde_json::Map::new();
            for func in &functions {
                if let Some(v) = func.last_value {
                    metric_scores.insert(
                        func.name.clone(),
                        serde_json::json!({
                            "value": v,
                            "weight": func.weight,
                            "metric_type": func.metric_type,
                        }),
                    );
                }
            }

            // 4. Insert the scorecard.
            let scorecard_id = Uuid::new_v4();
            let now = Utc::now();

            let sc_row = sqlx::query(
                r#"
                INSERT INTO agent_scorecards
                    (id, agent_id, swarm_id, fitness_score, tasks_completed,
                     tasks_failed, avg_task_duration_ms, total_tokens_used,
                     total_cost_usd, metric_scores, window_start, window_end,
                     weight_at_snapshot, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                RETURNING id, agent_id, swarm_id, fitness_score, tasks_completed,
                          tasks_failed, avg_task_duration_ms, total_tokens_used,
                          total_cost_usd, metric_scores, window_start, window_end,
                          weight_at_snapshot, created_at
                "#,
            )
            .bind(scorecard_id)
            .bind(agent_id)
            .bind(swarm_id)
            .bind(fitness_score)
            .bind(tasks_completed as i32)
            .bind(tasks_failed as i32)
            .bind(avg_task_duration_ms)
            .bind(total_tokens_used)
            .bind(total_cost_usd)
            .bind(serde_json::Value::Object(metric_scores))
            .bind(window_start)
            .bind(window_end)
            .bind(weight)
            .bind(now)
            .fetch_one(&self.db_pool)
            .await?;

            // 5. Update the denormalized fitness score on the swarm member.
            let _ = sqlx::query(
                r#"
                UPDATE agent_swarm_members
                SET fitness_score = $1
                WHERE swarm_id = $2 AND agent_id = $3
                "#,
            )
            .bind(fitness_score)
            .bind(swarm_id)
            .bind(agent_id)
            .execute(&self.db_pool)
            .await;

            scorecards.push(row_to_scorecard(&sc_row));
        }

        // Sort descending by fitness score before returning.
        scorecards.sort_by(|a, b| {
            b.fitness_score
                .partial_cmp(&a.fitness_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(scorecards)
    }

    /// Retrieve the most recent scorecard for a specific agent in a swarm.
    pub async fn get_latest_scorecard(
        &self,
        agent_id: i32,
        swarm_id: Uuid,
    ) -> Result<Option<Scorecard>> {
        let row = sqlx::query(
            r#"
            SELECT id, agent_id, swarm_id, fitness_score, tasks_completed,
                   tasks_failed, avg_task_duration_ms, total_tokens_used,
                   total_cost_usd, metric_scores, window_start, window_end,
                   weight_at_snapshot, created_at
            FROM agent_scorecards
            WHERE agent_id = $1 AND swarm_id = $2
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(agent_id)
        .bind(swarm_id)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(row.as_ref().map(row_to_scorecard))
    }

    /// Retrieve the latest scorecard per agent for the entire swarm.
    ///
    /// Uses `DISTINCT ON` to pick the most recent scorecard for each agent.
    pub async fn get_swarm_scorecards(&self, swarm_id: Uuid) -> Result<Vec<Scorecard>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT ON (agent_id)
                   id, agent_id, swarm_id, fitness_score, tasks_completed,
                   tasks_failed, avg_task_duration_ms, total_tokens_used,
                   total_cost_usd, metric_scores, window_start, window_end,
                   weight_at_snapshot, created_at
            FROM agent_scorecards
            WHERE swarm_id = $1
            ORDER BY agent_id, created_at DESC
            "#,
        )
        .bind(swarm_id)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(rows.iter().map(row_to_scorecard).collect())
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn row_to_scorecard(row: &sqlx::postgres::PgRow) -> Scorecard {
    Scorecard {
        id: row.get("id"),
        agent_id: row.get("agent_id"),
        swarm_id: row.get("swarm_id"),
        fitness_score: row.get("fitness_score"),
        tasks_completed: row.get("tasks_completed"),
        tasks_failed: row.get("tasks_failed"),
        avg_task_duration_ms: row.get("avg_task_duration_ms"),
        total_tokens_used: row.get("total_tokens_used"),
        total_cost_usd: row.get("total_cost_usd"),
        metric_scores: row.get("metric_scores"),
        window_start: row.get("window_start"),
        window_end: row.get("window_end"),
        weight_at_snapshot: row.get("weight_at_snapshot"),
        created_at: row.get("created_at"),
    }
}
