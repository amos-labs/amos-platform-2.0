//! Darwinian loop — background optimization cycle that evolves agent prompts
//! within swarms based on fitness scores, quartile-based weight adjustments,
//! and LLM-driven prompt mutations.

pub mod evaluator;
pub mod mutator;
pub mod weights;

use crate::fitness::{collector::ScorecardCollector, FitnessEngine};
use crate::types::Swarm;
use amos_core::{AmosError, Result};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// The main Darwinian optimization loop. Periodically iterates over every
/// enabled swarm, computes scorecards, evaluates mature experiments, adjusts
/// routing weights by fitness quartile, and proposes new prompt mutations for
/// the lowest-performing agents.
pub struct DarwinianLoop {
    db_pool: PgPool,
    #[allow(dead_code)]
    fitness_engine: Arc<FitnessEngine>,
    collector: Arc<ScorecardCollector>,
    interval_hours: u64,
}

impl DarwinianLoop {
    /// Create a new `DarwinianLoop`.
    ///
    /// * `db_pool` — shared PostgreSQL connection pool.
    /// * `fitness_engine` — the fitness computation engine.
    /// * `collector` — scorecard collector for snapshotting agent performance.
    /// * `interval_hours` — how often (in hours) the cycle runs.
    pub fn new(
        db_pool: PgPool,
        fitness_engine: Arc<FitnessEngine>,
        collector: Arc<ScorecardCollector>,
        interval_hours: u64,
    ) -> Self {
        Self {
            db_pool,
            fitness_engine,
            collector,
            interval_hours,
        }
    }

    /// Spawn the background loop as a detached Tokio task. The loop fires
    /// every `interval_hours` hours, running a full Darwinian cycle across
    /// all enabled swarms. Errors within a single swarm are logged but do
    /// not halt the cycle for remaining swarms.
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(self.interval_hours * 3600));
            loop {
                interval.tick().await;
                if let Err(e) = self.run_cycle().await {
                    error!("Darwinian cycle error: {e}");
                }
            }
        });
    }

    /// Execute one full Darwinian cycle across every enabled swarm.
    ///
    /// For each swarm the cycle:
    /// 1. Computes and persists scorecards via the collector.
    /// 2. Evaluates any mature experiments (accept / revert).
    /// 3. Expires stale proposed experiments.
    /// 4. Adjusts member routing weights by fitness quartile.
    /// 5. Identifies the lowest-fitness mutation target.
    /// 6. Proposes and applies a prompt mutation via the LLM mutator.
    pub async fn run_cycle(&self) -> Result<()> {
        info!("Darwinian cycle starting");

        let swarms = self.get_enabled_swarms().await?;
        info!(swarm_count = swarms.len(), "found enabled swarms");

        let eval = evaluator::Evaluator::new(self.db_pool.clone());
        let mut_engine = mutator::Mutator::new(self.db_pool.clone(), reqwest::Client::new());

        for swarm in &swarms {
            if let Err(e) = self.process_swarm(swarm, &eval, &mut_engine).await {
                warn!(
                    swarm_id = %swarm.id,
                    swarm_name = %swarm.name,
                    error = %e,
                    "Darwinian cycle failed for swarm — continuing"
                );
            }
        }

        info!("Darwinian cycle complete");
        Ok(())
    }

    /// Process a single swarm through all Darwinian stages.
    async fn process_swarm(
        &self,
        swarm: &Swarm,
        eval: &evaluator::Evaluator,
        mut_engine: &mutator::Mutator,
    ) -> Result<()> {
        let swarm_id = swarm.id;
        info!(swarm_id = %swarm_id, name = %swarm.name, "processing swarm");

        // Step 1: Compute scorecards
        info!(swarm_id = %swarm_id, "step 1 — computing scorecards");
        if let Err(e) = self.collector.collect_scorecards(swarm_id, 60).await {
            warn!(swarm_id = %swarm_id, error = %e, "scorecard collection failed");
        }

        // Step 2: Evaluate mature experiments
        info!(swarm_id = %swarm_id, "step 2 — evaluating mature experiments");
        match eval.evaluate_mature_experiments(swarm_id).await {
            Ok(ids) => {
                if !ids.is_empty() {
                    info!(
                        swarm_id = %swarm_id,
                        evaluated = ids.len(),
                        "evaluated mature experiments"
                    );
                }
            }
            Err(e) => {
                warn!(swarm_id = %swarm_id, error = %e, "experiment evaluation failed");
            }
        }

        // Step 2b: Expire stale proposed experiments
        match eval.expire_stale_experiments().await {
            Ok(count) if count > 0 => {
                info!(swarm_id = %swarm_id, expired = count, "expired stale experiments");
            }
            Err(e) => {
                warn!(swarm_id = %swarm_id, error = %e, "experiment expiry failed");
            }
            _ => {}
        }

        // Step 3: Adjust weights by fitness quartile
        info!(swarm_id = %swarm_id, "step 3 — adjusting weights");
        let members = sqlx::query(
            r#"
            SELECT agent_id, weight, COALESCE(fitness_score, 0.0) AS fitness_score
            FROM agent_swarm_members
            WHERE swarm_id = $1
            ORDER BY agent_id
            "#,
        )
        .bind(swarm_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to fetch members for weights: {e}")))?;

        if members.len() >= 4 {
            let mut member_data: Vec<(i32, f64, f64)> = members
                .iter()
                .map(|r| {
                    let agent_id: i32 = r.get("agent_id");
                    let weight: f64 = r.get("weight");
                    let fitness: f64 = r.get("fitness_score");
                    (agent_id, weight, fitness)
                })
                .collect();

            let changed = weights::adjust_weights(&mut member_data);
            for (agent_id, new_weight) in &changed {
                sqlx::query(
                    "UPDATE agent_swarm_members SET weight = $1 WHERE swarm_id = $2 AND agent_id = $3",
                )
                .bind(new_weight)
                .bind(swarm_id)
                .bind(agent_id)
                .execute(&self.db_pool)
                .await
                .map_err(|e| {
                    AmosError::Internal(format!("Failed to update weight for agent {agent_id}: {e}"))
                })?;
            }

            if !changed.is_empty() {
                info!(
                    swarm_id = %swarm_id,
                    adjusted = changed.len(),
                    "adjusted member weights"
                );
            }
        } else {
            info!(
                swarm_id = %swarm_id,
                count = members.len(),
                "fewer than 4 members — skipping weight adjustment"
            );
        }

        // Step 4: Identify mutation target and propose mutation
        info!(swarm_id = %swarm_id, "step 4 — proposing mutation");
        match mut_engine.find_mutation_target(swarm_id).await? {
            Some(agent_id) => {
                info!(
                    swarm_id = %swarm_id,
                    agent_id = agent_id,
                    "mutation target identified"
                );
                match mut_engine.propose_mutation(agent_id, swarm_id).await {
                    Ok(experiment) => {
                        info!(
                            swarm_id = %swarm_id,
                            agent_id = agent_id,
                            experiment_id = %experiment.id,
                            status = %experiment.status,
                            "mutation proposed"
                        );
                    }
                    Err(e) => {
                        warn!(
                            swarm_id = %swarm_id,
                            agent_id = agent_id,
                            error = %e,
                            "mutation proposal failed"
                        );
                    }
                }
            }
            None => {
                info!(swarm_id = %swarm_id, "no eligible mutation target found");
            }
        }

        Ok(())
    }

    /// Fetch all swarms that are currently enabled for Darwinian optimization.
    async fn get_enabled_swarms(&self) -> Result<Vec<Swarm>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, description, parent_swarm_id, layer_order,
                   routing_strategy, max_agents, enabled, domain, metadata,
                   created_at, updated_at
            FROM agent_swarms
            WHERE enabled = true
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to fetch enabled swarms: {e}")))?;

        Ok(rows.iter().map(swarm_from_row).collect())
    }
}

// ── Row mapping helper ─────────────────────────────────────────────────

fn swarm_from_row(row: &sqlx::postgres::PgRow) -> Swarm {
    Swarm {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        parent_swarm_id: row.get("parent_swarm_id"),
        layer_order: row.get("layer_order"),
        routing_strategy: row.get("routing_strategy"),
        max_agents: row.get("max_agents"),
        enabled: row.get("enabled"),
        domain: row.get("domain"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
