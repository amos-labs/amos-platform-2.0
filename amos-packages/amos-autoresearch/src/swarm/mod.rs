//! Swarm management — CRUD operations for agent swarms and their members.

pub mod router;

use amos_core::{AmosError, Result};
use chrono::Utc;
use sqlx::{PgPool, Row};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::types::{AddMemberRequest, CreateSwarmRequest, Swarm, SwarmMember, UpdateSwarmRequest};

/// Manages the lifecycle of agent swarms and their membership.
pub struct SwarmManager {
    db_pool: PgPool,
}

impl SwarmManager {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    // ── Swarm CRUD ──────────────────────────────────────────────────────

    /// Create a new agent swarm.
    pub async fn create_swarm(&self, req: &CreateSwarmRequest) -> Result<Swarm> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let layer_order = req.layer_order.unwrap_or(0);
        let routing_strategy = req.routing_strategy.as_deref().unwrap_or("round_robin");
        let max_agents = req.max_agents.unwrap_or(10);
        let domain = req.domain.as_deref().unwrap_or("general");
        let metadata = req
            .metadata
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));

        info!(swarm_id = %id, name = %req.name, "creating swarm");

        let row = sqlx::query(
            r#"
            INSERT INTO agent_swarms
                (id, name, description, parent_swarm_id, layer_order,
                 routing_strategy, max_agents, enabled, domain, metadata,
                 created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, true, $8, $9, $10, $11)
            RETURNING id, name, description, parent_swarm_id, layer_order,
                      routing_strategy, max_agents, enabled, domain, metadata,
                      created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.parent_swarm_id)
        .bind(layer_order)
        .bind(routing_strategy)
        .bind(max_agents)
        .bind(domain)
        .bind(&metadata)
        .bind(now)
        .bind(now)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to create swarm: {e}")))?;

        Ok(swarm_from_row(&row))
    }

    /// Fetch a single swarm by ID.
    pub async fn get_swarm(&self, id: Uuid) -> Result<Option<Swarm>> {
        debug!(swarm_id = %id, "fetching swarm");

        let row = sqlx::query(
            r#"
            SELECT id, name, description, parent_swarm_id, layer_order,
                   routing_strategy, max_agents, enabled, domain, metadata,
                   created_at, updated_at
            FROM agent_swarms
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to fetch swarm: {e}")))?;

        Ok(row.as_ref().map(swarm_from_row))
    }

    /// List all swarms ordered by creation time (oldest first).
    pub async fn list_swarms(&self) -> Result<Vec<Swarm>> {
        debug!("listing all swarms");

        let rows = sqlx::query(
            r#"
            SELECT id, name, description, parent_swarm_id, layer_order,
                   routing_strategy, max_agents, enabled, domain, metadata,
                   created_at, updated_at
            FROM agent_swarms
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to list swarms: {e}")))?;

        Ok(rows.iter().map(swarm_from_row).collect())
    }

    /// List only enabled swarms.
    pub async fn list_enabled_swarms(&self) -> Result<Vec<Swarm>> {
        debug!("listing enabled swarms");

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
        .map_err(|e| AmosError::Internal(format!("Failed to list enabled swarms: {e}")))?;

        Ok(rows.iter().map(swarm_from_row).collect())
    }

    /// Partially update a swarm. Only non-`None` fields are applied.
    pub async fn update_swarm(&self, id: Uuid, req: &UpdateSwarmRequest) -> Result<Swarm> {
        info!(swarm_id = %id, "updating swarm");

        // Fetch current to merge partial update
        let current = self
            .get_swarm(id)
            .await?
            .ok_or_else(|| AmosError::NotFound {
                entity: "Swarm".into(),
                id: id.to_string(),
            })?;

        let name = req.name.as_deref().unwrap_or(&current.name);
        let description = req.description.as_ref().or(current.description.as_ref());
        let routing_strategy = req
            .routing_strategy
            .as_deref()
            .unwrap_or(&current.routing_strategy);
        let max_agents = req.max_agents.unwrap_or(current.max_agents);
        let enabled = req.enabled.unwrap_or(current.enabled);
        let domain = req.domain.as_deref().unwrap_or(&current.domain);
        let metadata = req.metadata.as_ref().unwrap_or(&current.metadata);
        let now = Utc::now();

        let row = sqlx::query(
            r#"
            UPDATE agent_swarms
            SET name = $1,
                description = $2,
                routing_strategy = $3,
                max_agents = $4,
                enabled = $5,
                domain = $6,
                metadata = $7,
                updated_at = $8
            WHERE id = $9
            RETURNING id, name, description, parent_swarm_id, layer_order,
                      routing_strategy, max_agents, enabled, domain, metadata,
                      created_at, updated_at
            "#,
        )
        .bind(name)
        .bind(description)
        .bind(routing_strategy)
        .bind(max_agents)
        .bind(enabled)
        .bind(domain)
        .bind(metadata)
        .bind(now)
        .bind(id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to update swarm: {e}")))?;

        Ok(swarm_from_row(&row))
    }

    /// Delete a swarm and its member associations.
    pub async fn delete_swarm(&self, id: Uuid) -> Result<()> {
        info!(swarm_id = %id, "deleting swarm");

        // Remove members first to honour any FK constraints.
        sqlx::query("DELETE FROM agent_swarm_members WHERE swarm_id = $1")
            .bind(id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| AmosError::Internal(format!("Failed to delete swarm members: {e}")))?;

        let result = sqlx::query("DELETE FROM agent_swarms WHERE id = $1")
            .bind(id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| AmosError::Internal(format!("Failed to delete swarm: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(AmosError::NotFound {
                entity: "Swarm".into(),
                id: id.to_string(),
            });
        }

        Ok(())
    }

    // ── Member Management ───────────────────────────────────────────────

    /// Add an agent to a swarm. Enforces the swarm's `max_agents` limit.
    pub async fn add_member(&self, swarm_id: Uuid, req: &AddMemberRequest) -> Result<SwarmMember> {
        // Validate swarm exists and check capacity.
        let swarm = self
            .get_swarm(swarm_id)
            .await?
            .ok_or_else(|| AmosError::NotFound {
                entity: "Swarm".into(),
                id: swarm_id.to_string(),
            })?;

        let member_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_swarm_members WHERE swarm_id = $1")
                .bind(swarm_id)
                .fetch_one(&self.db_pool)
                .await
                .map_err(|e| AmosError::Internal(format!("Failed to count members: {e}")))?;

        if member_count >= swarm.max_agents as i64 {
            warn!(
                swarm_id = %swarm_id,
                current = member_count,
                max = swarm.max_agents,
                "swarm member limit reached"
            );
            return Err(AmosError::Validation(format!(
                "Swarm has reached its maximum of {} agents",
                swarm.max_agents
            )));
        }

        let id = Uuid::new_v4();
        let role = req.role.as_deref().unwrap_or("worker");
        let weight = req.weight.unwrap_or(1.0);
        let now = Utc::now();

        info!(
            swarm_id = %swarm_id,
            agent_id = req.agent_id,
            role = role,
            "adding member to swarm"
        );

        let row = sqlx::query(
            r#"
            INSERT INTO agent_swarm_members
                (id, swarm_id, agent_id, weight, fitness_score, role, joined_at)
            VALUES ($1, $2, $3, $4, NULL, $5, $6)
            RETURNING id, swarm_id, agent_id, weight, fitness_score, role, joined_at
            "#,
        )
        .bind(id)
        .bind(swarm_id)
        .bind(req.agent_id)
        .bind(weight)
        .bind(role)
        .bind(now)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to add swarm member: {e}")))?;

        Ok(member_from_row(&row))
    }

    /// Remove an agent from a swarm.
    pub async fn remove_member(&self, swarm_id: Uuid, agent_id: i32) -> Result<()> {
        info!(swarm_id = %swarm_id, agent_id = agent_id, "removing member from swarm");

        let result =
            sqlx::query("DELETE FROM agent_swarm_members WHERE swarm_id = $1 AND agent_id = $2")
                .bind(swarm_id)
                .bind(agent_id)
                .execute(&self.db_pool)
                .await
                .map_err(|e| AmosError::Internal(format!("Failed to remove swarm member: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(AmosError::NotFound {
                entity: "SwarmMember".into(),
                id: format!("swarm={swarm_id}, agent={agent_id}"),
            });
        }

        Ok(())
    }

    /// List all members in a swarm.
    pub async fn list_members(&self, swarm_id: Uuid) -> Result<Vec<SwarmMember>> {
        debug!(swarm_id = %swarm_id, "listing swarm members");

        let rows = sqlx::query(
            r#"
            SELECT id, swarm_id, agent_id, weight, fitness_score, role, joined_at
            FROM agent_swarm_members
            WHERE swarm_id = $1
            ORDER BY joined_at ASC
            "#,
        )
        .bind(swarm_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to list swarm members: {e}")))?;

        Ok(rows.iter().map(member_from_row).collect())
    }

    /// Fetch direct child swarms of a parent (for hierarchical routing).
    pub async fn get_child_swarms(&self, parent_id: Uuid) -> Result<Vec<Swarm>> {
        debug!(parent_id = %parent_id, "fetching child swarms");

        let rows = sqlx::query(
            r#"
            SELECT id, name, description, parent_swarm_id, layer_order,
                   routing_strategy, max_agents, enabled, domain, metadata,
                   created_at, updated_at
            FROM agent_swarms
            WHERE parent_swarm_id = $1
            ORDER BY layer_order ASC, created_at ASC
            "#,
        )
        .bind(parent_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to fetch child swarms: {e}")))?;

        Ok(rows.iter().map(swarm_from_row).collect())
    }

    /// Update the routing weight for a swarm member.
    pub async fn update_member_weight(
        &self,
        swarm_id: Uuid,
        agent_id: i32,
        weight: f64,
    ) -> Result<()> {
        debug!(swarm_id = %swarm_id, agent_id = agent_id, weight = weight, "updating member weight");

        let result = sqlx::query(
            r#"
            UPDATE agent_swarm_members
            SET weight = $1
            WHERE swarm_id = $2 AND agent_id = $3
            "#,
        )
        .bind(weight)
        .bind(swarm_id)
        .bind(agent_id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to update member weight: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(AmosError::NotFound {
                entity: "SwarmMember".into(),
                id: format!("swarm={swarm_id}, agent={agent_id}"),
            });
        }

        Ok(())
    }

    /// Update the fitness score for a swarm member.
    pub async fn update_member_fitness(
        &self,
        swarm_id: Uuid,
        agent_id: i32,
        fitness: f64,
    ) -> Result<()> {
        debug!(
            swarm_id = %swarm_id,
            agent_id = agent_id,
            fitness = fitness,
            "updating member fitness"
        );

        let result = sqlx::query(
            r#"
            UPDATE agent_swarm_members
            SET fitness_score = $1
            WHERE swarm_id = $2 AND agent_id = $3
            "#,
        )
        .bind(fitness)
        .bind(swarm_id)
        .bind(agent_id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Failed to update member fitness: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(AmosError::NotFound {
                entity: "SwarmMember".into(),
                id: format!("swarm={swarm_id}, agent={agent_id}"),
            });
        }

        Ok(())
    }
}

// ── Row mapping helpers ─────────────────────────────────────────────────

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

fn member_from_row(row: &sqlx::postgres::PgRow) -> SwarmMember {
    SwarmMember {
        id: row.get("id"),
        swarm_id: row.get("swarm_id"),
        agent_id: row.get("agent_id"),
        weight: row.get("weight"),
        fitness_score: row.get("fitness_score"),
        role: row.get("role"),
        joined_at: row.get("joined_at"),
    }
}
