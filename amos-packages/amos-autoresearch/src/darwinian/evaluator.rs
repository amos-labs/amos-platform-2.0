//! Experiment evaluator — determines whether a Darwinian experiment improved
//! an agent's fitness, and either accepts the mutated prompt or reverts it.

use crate::types::Experiment;
use amos_core::{AmosError, Result};
use chrono::Utc;
use sqlx::{PgPool, Row};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Evaluates mature experiments by comparing post-mutation fitness against the
/// recorded baseline. Experiments that improved fitness are accepted; those
/// that degraded it are reverted (the original prompt is restored).
pub struct Evaluator {
    db_pool: PgPool,
}

impl Evaluator {
    /// Create a new `Evaluator`.
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    /// Find and evaluate all experiments in the given swarm whose evaluation
    /// window has elapsed (i.e. `started_at + evaluation_days <= NOW()` and
    /// `status = 'active'`).
    ///
    /// For each mature experiment:
    /// 1. Compute the agent's current fitness.
    /// 2. Compare against `baseline_fitness`.
    /// 3. If improved: accept (keep the mutated prompt).
    /// 4. If degraded: revert (restore the original prompt).
    /// 5. Record `final_fitness`, `fitness_delta`, and `completed_at`.
    ///
    /// Returns the IDs of all evaluated experiments.
    pub async fn evaluate_mature_experiments(&self, swarm_id: Uuid) -> Result<Vec<Uuid>> {
        debug!(swarm_id = %swarm_id, "evaluating mature experiments");

        let rows = sqlx::query(
            r#"
            SELECT id, swarm_id, agent_id, experiment_type, diff,
                   original_prompt, mutated_prompt, status,
                   baseline_fitness, final_fitness, fitness_delta,
                   evaluation_days, cooldown_days,
                   proposed_by, proposal_reasoning,
                   started_at, completed_at, created_at, updated_at
            FROM darwinian_experiments
            WHERE swarm_id = $1
              AND status = 'active'
              AND started_at IS NOT NULL
              AND started_at + (evaluation_days || ' days')::INTERVAL <= NOW()
            "#,
        )
        .bind(swarm_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to fetch mature experiments: {e}")))?;

        let mut evaluated_ids = Vec::new();

        for row in &rows {
            let experiment = experiment_from_row(row);
            match self.evaluate_single(&experiment).await {
                Ok(()) => {
                    evaluated_ids.push(experiment.id);
                }
                Err(e) => {
                    warn!(
                        experiment_id = %experiment.id,
                        agent_id = experiment.agent_id,
                        error = %e,
                        "failed to evaluate experiment — skipping"
                    );
                }
            }
        }

        Ok(evaluated_ids)
    }

    /// Manually revert an active experiment. Restores the original prompt to
    /// the agent and marks the experiment as `reverted`.
    pub async fn revert_experiment(&self, experiment_id: Uuid) -> Result<()> {
        info!(experiment_id = %experiment_id, "manually reverting experiment");

        let row = sqlx::query(
            r#"
            SELECT id, swarm_id, agent_id, experiment_type, diff,
                   original_prompt, mutated_prompt, status,
                   baseline_fitness, final_fitness, fitness_delta,
                   evaluation_days, cooldown_days,
                   proposed_by, proposal_reasoning,
                   started_at, completed_at, created_at, updated_at
            FROM darwinian_experiments
            WHERE id = $1
            "#,
        )
        .bind(experiment_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to fetch experiment for revert: {e}")))?;

        let row = row.ok_or_else(|| AmosError::NotFound {
            entity: "Experiment".into(),
            id: experiment_id.to_string(),
        })?;

        let experiment = experiment_from_row(&row);

        if experiment.status != "active" && experiment.status != "evaluating" {
            return Err(AmosError::Validation(format!(
                "Cannot revert experiment in status '{}' — only active or evaluating experiments can be reverted",
                experiment.status
            )));
        }

        // Restore original prompt
        if let Some(ref original) = experiment.original_prompt {
            sqlx::query("UPDATE openclaw_agents SET system_prompt = $1 WHERE id = $2")
                .bind(original)
                .bind(experiment.agent_id)
                .execute(&self.db_pool)
                .await
                .map_err(|e| AmosError::Internal(format!("Failed to restore agent prompt: {e}")))?;
        }

        // Mark as reverted
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE darwinian_experiments
            SET status = 'reverted',
                completed_at = $1,
                updated_at = $2
            WHERE id = $3
            "#,
        )
        .bind(now)
        .bind(now)
        .bind(experiment_id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to mark experiment as reverted: {e}")))?;

        info!(
            experiment_id = %experiment_id,
            agent_id = experiment.agent_id,
            "experiment reverted — original prompt restored"
        );

        Ok(())
    }

    /// Mark experiments stuck in `proposed` status for longer than 7 days as
    /// `expired`. Returns the number of experiments expired.
    pub async fn expire_stale_experiments(&self) -> Result<i64> {
        debug!("expiring stale proposed experiments");

        let now = Utc::now();
        let result = sqlx::query(
            r#"
            UPDATE darwinian_experiments
            SET status = 'expired',
                completed_at = $1,
                updated_at = $2
            WHERE status = 'proposed'
              AND created_at + INTERVAL '7 days' <= NOW()
            "#,
        )
        .bind(now)
        .bind(now)
        .execute(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to expire stale experiments: {e}")))?;

        let count = result.rows_affected() as i64;
        if count > 0 {
            info!(expired = count, "expired stale proposed experiments");
        }

        Ok(count)
    }

    // ── Private helpers ─────────────────────────────────────────────────

    /// Evaluate a single mature experiment.
    async fn evaluate_single(&self, experiment: &Experiment) -> Result<()> {
        let agent_id = experiment.agent_id;
        let swarm_id = experiment.swarm_id;

        // Compute current fitness
        let current_fitness = self.get_current_fitness(agent_id, swarm_id).await?;
        let baseline = experiment.baseline_fitness.unwrap_or(0.0);
        let delta = current_fitness - baseline;

        let (new_status, action) = if delta >= 0.0 {
            ("accepted", "keeping mutated prompt")
        } else {
            ("reverted", "restoring original prompt")
        };

        info!(
            experiment_id = %experiment.id,
            agent_id = agent_id,
            baseline = baseline,
            current = current_fitness,
            delta = delta,
            status = new_status,
            "{action}"
        );

        // If reverting, restore the original prompt
        if new_status == "reverted" {
            if let Some(ref original) = experiment.original_prompt {
                sqlx::query("UPDATE openclaw_agents SET system_prompt = $1 WHERE id = $2")
                    .bind(original)
                    .bind(agent_id)
                    .execute(&self.db_pool)
                    .await
                    .map_err(|e| {
                        AmosError::Internal(format!("Failed to restore agent prompt: {e}"))
                    })?;
            }
        }

        // Update the experiment record
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE darwinian_experiments
            SET status = $1,
                final_fitness = $2,
                fitness_delta = $3,
                completed_at = $4,
                updated_at = $5
            WHERE id = $6
            "#,
        )
        .bind(new_status)
        .bind(current_fitness)
        .bind(delta)
        .bind(now)
        .bind(now)
        .bind(experiment.id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to update experiment result: {e}")))?;

        // Update the member's fitness score
        sqlx::query(
            "UPDATE agent_swarm_members SET fitness_score = $1 WHERE swarm_id = $2 AND agent_id = $3",
        )
        .bind(current_fitness)
        .bind(swarm_id)
        .bind(agent_id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| {
            AmosError::Internal(format!("Failed to update member fitness: {e}"))
        })?;

        Ok(())
    }

    /// Fetch the current fitness score for an agent in a swarm from the
    /// most recent scorecard.
    async fn get_current_fitness(&self, agent_id: i32, swarm_id: Uuid) -> Result<f64> {
        let row = sqlx::query(
            r#"
            SELECT fitness_score
            FROM agent_scorecards
            WHERE agent_id = $1 AND swarm_id = $2
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(agent_id)
        .bind(swarm_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to fetch current fitness: {e}")))?;

        match row {
            Some(r) => Ok(r.get("fitness_score")),
            None => {
                // Fall back to the member's stored fitness_score
                let member = sqlx::query(
                    "SELECT fitness_score FROM agent_swarm_members WHERE agent_id = $1 AND swarm_id = $2",
                )
                .bind(agent_id)
                .bind(swarm_id)
                .fetch_optional(&self.db_pool)
                .await
                .map_err(|e| {
                    AmosError::Internal(format!("Failed to fetch member fitness: {e}"))
                })?;

                Ok(member
                    .and_then(|r| r.get::<Option<f64>, _>("fitness_score"))
                    .unwrap_or(0.0))
            }
        }
    }
}

// ── Row mapping helper ─────────────────────────────────────────────────

fn experiment_from_row(row: &sqlx::postgres::PgRow) -> Experiment {
    Experiment {
        id: row.get("id"),
        swarm_id: row.get("swarm_id"),
        agent_id: row.get("agent_id"),
        experiment_type: row.get("experiment_type"),
        diff: row.get("diff"),
        original_prompt: row.get("original_prompt"),
        mutated_prompt: row.get("mutated_prompt"),
        status: row.get("status"),
        baseline_fitness: row.get("baseline_fitness"),
        final_fitness: row.get("final_fitness"),
        fitness_delta: row.get("fitness_delta"),
        evaluation_days: row.get("evaluation_days"),
        cooldown_days: row.get("cooldown_days"),
        proposed_by: row.get("proposed_by"),
        proposal_reasoning: row.get("proposal_reasoning"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
