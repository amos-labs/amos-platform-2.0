//! Task routing — selects an agent from a swarm based on its routing strategy.

use amos_core::{AmosError, Result};
use rand::Rng;
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::types::{RouteTaskRequest, RoutingStrategy};

use super::SwarmManager;

/// Routes incoming tasks to the most appropriate agent within a swarm.
pub struct SwarmRouter {
    db_pool: PgPool,
}

impl SwarmRouter {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    /// Select an agent from the swarm to handle a task.
    ///
    /// The selection algorithm depends on the swarm's configured `routing_strategy`.
    /// Uses `Box::pin` internally to support recursive hierarchical routing.
    pub fn route_task<'a>(
        &'a self,
        swarm_id: Uuid,
        req: &'a RouteTaskRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<i32>> + Send + 'a>> {
        Box::pin(self.route_task_inner(swarm_id, req))
    }

    async fn route_task_inner(&self, swarm_id: Uuid, req: &RouteTaskRequest) -> Result<i32> {
        let manager = SwarmManager::new(self.db_pool.clone());

        let swarm = manager
            .get_swarm(swarm_id)
            .await?
            .ok_or_else(|| AmosError::NotFound {
                entity: "Swarm".into(),
                id: swarm_id.to_string(),
            })?;

        if !swarm.enabled {
            return Err(AmosError::Validation(format!(
                "Swarm {} is disabled",
                swarm_id
            )));
        }

        let strategy = RoutingStrategy::parse(&swarm.routing_strategy).ok_or_else(|| {
            AmosError::Validation(format!(
                "Unknown routing strategy: {}",
                swarm.routing_strategy
            ))
        })?;

        info!(
            swarm_id = %swarm_id,
            strategy = swarm.routing_strategy.as_str(),
            task = %req.task_description,
            "routing task"
        );

        match strategy {
            RoutingStrategy::RoundRobin => self.route_round_robin(swarm_id).await,
            RoutingStrategy::Capability => {
                self.route_by_capability(swarm_id, req.required_capabilities.as_deref())
                    .await
            }
            RoutingStrategy::Load => self.route_by_load(swarm_id).await,
            RoutingStrategy::Fitness => self.route_by_fitness(swarm_id).await,
            RoutingStrategy::Hierarchical => self.route_hierarchical(swarm_id, req).await,
        }
    }

    // ── Round-Robin ─────────────────────────────────────────────────────

    /// Pick the member whose last task attribution is the oldest (or who has
    /// never been assigned). Falls back to random if no attribution history.
    async fn route_round_robin(&self, swarm_id: Uuid) -> Result<i32> {
        debug!(swarm_id = %swarm_id, "round-robin routing");

        // Find the member with the least recent task attribution.
        let row = sqlx::query(
            r#"
            SELECT m.agent_id
            FROM agent_swarm_members m
            LEFT JOIN agent_task_attribution ta
                ON ta.agent_id = m.agent_id AND ta.swarm_id = m.swarm_id
            WHERE m.swarm_id = $1
            ORDER BY ta.created_at ASC NULLS FIRST
            LIMIT 1
            "#,
        )
        .bind(swarm_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Round-robin query failed: {e}")))?;

        match row {
            Some(r) => Ok(r.get("agent_id")),
            None => {
                // No attribution history — pick a random member.
                self.random_member(swarm_id).await
            }
        }
    }

    // ── Capability Matching ─────────────────────────────────────────────

    /// Match `required_capabilities` against each agent's capabilities JSONB.
    /// If no capabilities are required, falls back to random selection.
    async fn route_by_capability(
        &self,
        swarm_id: Uuid,
        required: Option<&[String]>,
    ) -> Result<i32> {
        let caps = match required {
            Some(c) if !c.is_empty() => c,
            _ => {
                debug!(swarm_id = %swarm_id, "no capabilities required, random selection");
                return self.random_member(swarm_id).await;
            }
        };

        debug!(
            swarm_id = %swarm_id,
            required_capabilities = ?caps,
            "capability-based routing"
        );

        // Build a JSONB array of the required capabilities and check containment.
        let caps_json: JsonValue = serde_json::to_value(caps)
            .map_err(|e| AmosError::Internal(format!("Failed to serialize capabilities: {e}")))?;

        // agents table is assumed to have a `capabilities` JSONB column (array of strings).
        // We find members whose agent capabilities contain ALL required capabilities.
        let row = sqlx::query(
            r#"
            SELECT m.agent_id
            FROM agent_swarm_members m
            JOIN openclaw_agents a ON a.id = m.agent_id
            WHERE m.swarm_id = $1
              AND a.capabilities @> $2
            ORDER BY m.weight DESC
            LIMIT 1
            "#,
        )
        .bind(swarm_id)
        .bind(&caps_json)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Capability routing query failed: {e}")))?;

        match row {
            Some(r) => Ok(r.get("agent_id")),
            None => {
                warn!(
                    swarm_id = %swarm_id,
                    capabilities = ?caps,
                    "no agent matches required capabilities, falling back to highest weight"
                );
                self.random_member(swarm_id).await
            }
        }
    }

    // ── Load-Based ──────────────────────────────────────────────────────

    /// Pick the agent with the fewest in-progress tasks.
    async fn route_by_load(&self, swarm_id: Uuid) -> Result<i32> {
        debug!(swarm_id = %swarm_id, "load-based routing");

        let row = sqlx::query(
            r#"
            SELECT m.agent_id,
                   COUNT(t.id) AS in_progress
            FROM agent_swarm_members m
            LEFT JOIN tasks t
                ON t.assigned_agent_id = m.agent_id
               AND t.status = 'in_progress'
            WHERE m.swarm_id = $1
            GROUP BY m.agent_id
            ORDER BY in_progress ASC, m.weight DESC
            LIMIT 1
            "#,
        )
        .bind(swarm_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Load routing query failed: {e}")))?;

        match row {
            Some(r) => Ok(r.get("agent_id")),
            None => Err(AmosError::Validation(format!(
                "Swarm {swarm_id} has no members"
            ))),
        }
    }

    // ── Fitness-Weighted ────────────────────────────────────────────────

    /// Weighted random selection: higher fitness_score = higher probability.
    /// Members without a fitness score default to 1.0.
    async fn route_by_fitness(&self, swarm_id: Uuid) -> Result<i32> {
        debug!(swarm_id = %swarm_id, "fitness-weighted routing");

        let rows = sqlx::query(
            r#"
            SELECT agent_id, COALESCE(fitness_score, 1.0) AS fitness
            FROM agent_swarm_members
            WHERE swarm_id = $1
            "#,
        )
        .bind(swarm_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Fitness routing query failed: {e}")))?;

        if rows.is_empty() {
            return Err(AmosError::Validation(format!(
                "Swarm {swarm_id} has no members"
            )));
        }

        // Collect (agent_id, fitness) pairs, clamp negative to a small positive.
        let members: Vec<(i32, f64)> = rows
            .iter()
            .map(|r| {
                let agent_id: i32 = r.get("agent_id");
                let fitness: f64 = r.get("fitness");
                (agent_id, fitness.max(0.01))
            })
            .collect();

        let total_fitness: f64 = members.iter().map(|(_, f)| f).sum();

        let mut rng = rand::thread_rng();
        let mut pick: f64 = rng.gen_range(0.0..total_fitness);

        for (agent_id, fitness) in &members {
            pick -= fitness;
            if pick <= 0.0 {
                debug!(agent_id = agent_id, "fitness-weighted selection");
                return Ok(*agent_id);
            }
        }

        // Floating-point edge case — return last member.
        Ok(members.last().unwrap().0)
    }

    // ── Hierarchical ────────────────────────────────────────────────────

    /// Process child swarms in `layer_order`. Each sub-swarm routes internally
    /// using its own strategy; results bubble up to the parent. The first
    /// child swarm that successfully routes a task wins.
    async fn route_hierarchical(
        &self,
        parent_swarm_id: Uuid,
        req: &RouteTaskRequest,
    ) -> Result<i32> {
        debug!(parent_swarm_id = %parent_swarm_id, "hierarchical routing");

        let manager = SwarmManager::new(self.db_pool.clone());
        let children = manager.get_child_swarms(parent_swarm_id).await?;

        if children.is_empty() {
            // Leaf swarm — fall back to fitness-weighted selection among
            // direct members of this swarm.
            debug!(
                swarm_id = %parent_swarm_id,
                "no child swarms, falling back to fitness routing"
            );
            return self.route_by_fitness(parent_swarm_id).await;
        }

        // Try each child swarm in layer_order.
        for child in &children {
            if !child.enabled {
                debug!(child_swarm_id = %child.id, "skipping disabled child swarm");
                continue;
            }

            match self.route_task(child.id, req).await {
                Ok(agent_id) => {
                    info!(
                        parent_swarm_id = %parent_swarm_id,
                        child_swarm_id = %child.id,
                        agent_id = agent_id,
                        "hierarchical routing selected agent via child swarm"
                    );
                    return Ok(agent_id);
                }
                Err(e) => {
                    warn!(
                        child_swarm_id = %child.id,
                        error = %e,
                        "child swarm routing failed, trying next"
                    );
                    continue;
                }
            }
        }

        Err(AmosError::Internal(format!(
            "Hierarchical routing exhausted all child swarms of {parent_swarm_id}"
        )))
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    /// Pick a random member from a swarm.
    async fn random_member(&self, swarm_id: Uuid) -> Result<i32> {
        let row = sqlx::query(
            r#"
            SELECT agent_id
            FROM agent_swarm_members
            WHERE swarm_id = $1
            ORDER BY RANDOM()
            LIMIT 1
            "#,
        )
        .bind(swarm_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Random member query failed: {e}")))?;

        match row {
            Some(r) => Ok(r.get("agent_id")),
            None => Err(AmosError::Validation(format!(
                "Swarm {swarm_id} has no members"
            ))),
        }
    }
}
