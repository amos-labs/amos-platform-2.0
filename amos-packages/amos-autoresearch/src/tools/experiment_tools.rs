//! Darwinian experiment tools for the agent.

use amos_core::{tools::ToolCategory, Result, Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use uuid::Uuid;

use crate::darwinian::evaluator::Evaluator;
use crate::darwinian::mutator::Mutator;

// ─── ProposeExperimentTool ──────────────────────────────────────────

pub struct ProposeExperimentTool {
    db_pool: PgPool,
}

impl ProposeExperimentTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ProposeExperimentTool {
    fn name(&self) -> &str {
        "propose_experiment"
    }

    fn description(&self) -> &str {
        "Manually trigger a Darwinian prompt mutation experiment for an agent in a swarm. The system will use an LLM to propose a targeted change to the agent's system prompt."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "swarm_id": { "type": "string", "description": "Swarm UUID" },
                "agent_id": { "type": "integer", "description": "Target agent ID" },
                "evaluation_days": { "type": "integer", "description": "Days to evaluate before deciding (default 5)" },
                "cooldown_days": { "type": "integer", "description": "Days to wait before next experiment (default 5)" }
            },
            "required": ["swarm_id", "agent_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Autoresearch
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let swarm_id = params
            .get("swarm_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| amos_core::AmosError::Validation("swarm_id is required".into()))?;

        let agent_id = params
            .get("agent_id")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .ok_or_else(|| amos_core::AmosError::Validation("agent_id is required".into()))?;

        let http_client = reqwest::Client::new();
        let mutator = Mutator::new(self.db_pool.clone(), http_client);
        let experiment = mutator.propose_mutation(agent_id, swarm_id).await?;

        Ok(ToolResult::success(json!({
            "experiment_id": experiment.id,
            "agent_id": experiment.agent_id,
            "status": experiment.status,
            "experiment_type": experiment.experiment_type,
            "proposal_reasoning": experiment.proposal_reasoning,
            "evaluation_days": experiment.evaluation_days,
        })))
    }
}

// ─── ViewExperimentsTool ────────────────────────────────────────────

pub struct ViewExperimentsTool {
    db_pool: PgPool,
}

impl ViewExperimentsTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ViewExperimentsTool {
    fn name(&self) -> &str {
        "view_experiments"
    }

    fn description(&self) -> &str {
        "List Darwinian experiments with their status, fitness deltas, and reasoning."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "swarm_id": { "type": "string", "description": "Filter by swarm UUID" },
                "status": { "type": "string", "enum": ["proposed", "active", "evaluating", "accepted", "reverted", "expired"], "description": "Filter by status" },
                "limit": { "type": "integer", "description": "Max results (default 20)" }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Autoresearch
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let swarm_id = params
            .get("swarm_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        let status = params.get("status").and_then(|v| v.as_str());
        let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(20) as i32;

        use sqlx::Row;
        let rows = if let Some(sid) = swarm_id {
            if let Some(st) = status {
                sqlx::query(
                    "SELECT * FROM autoresearch_experiments WHERE swarm_id = $1 AND status = $2 ORDER BY created_at DESC LIMIT $3"
                )
                .bind(sid)
                .bind(st)
                .bind(limit)
                .fetch_all(&self.db_pool)
                .await?
            } else {
                sqlx::query(
                    "SELECT * FROM autoresearch_experiments WHERE swarm_id = $1 ORDER BY created_at DESC LIMIT $2"
                )
                .bind(sid)
                .bind(limit)
                .fetch_all(&self.db_pool)
                .await?
            }
        } else if let Some(st) = status {
            sqlx::query(
                "SELECT * FROM autoresearch_experiments WHERE status = $1 ORDER BY created_at DESC LIMIT $2"
            )
            .bind(st)
            .bind(limit)
            .fetch_all(&self.db_pool)
            .await?
        } else {
            sqlx::query("SELECT * FROM autoresearch_experiments ORDER BY created_at DESC LIMIT $1")
                .bind(limit)
                .fetch_all(&self.db_pool)
                .await?
        };

        let experiments: Vec<JsonValue> = rows
            .iter()
            .map(|row| {
                json!({
                    "id": row.get::<Uuid, _>("id").to_string(),
                    "swarm_id": row.get::<Uuid, _>("swarm_id").to_string(),
                    "agent_id": row.get::<i32, _>("agent_id"),
                    "experiment_type": row.get::<String, _>("experiment_type"),
                    "status": row.get::<String, _>("status"),
                    "baseline_fitness": row.get::<Option<f64>, _>("baseline_fitness"),
                    "final_fitness": row.get::<Option<f64>, _>("final_fitness"),
                    "fitness_delta": row.get::<Option<f64>, _>("fitness_delta"),
                    "proposal_reasoning": row.get::<Option<String>, _>("proposal_reasoning"),
                    "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
                })
            })
            .collect();

        Ok(ToolResult::success(json!({
            "experiments": experiments,
            "count": experiments.len()
        })))
    }
}

// ─── RevertExperimentTool ───────────────────────────────────────────

pub struct RevertExperimentTool {
    db_pool: PgPool,
}

impl RevertExperimentTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for RevertExperimentTool {
    fn name(&self) -> &str {
        "revert_experiment"
    }

    fn description(&self) -> &str {
        "Manually revert an active Darwinian experiment, restoring the agent's original system prompt."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "experiment_id": { "type": "string", "description": "Experiment UUID to revert" }
            },
            "required": ["experiment_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Autoresearch
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let experiment_id = params
            .get("experiment_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| amos_core::AmosError::Validation("experiment_id is required".into()))?;

        let evaluator = Evaluator::new(self.db_pool.clone());
        evaluator.revert_experiment(experiment_id).await?;

        Ok(ToolResult::success(json!({
            "experiment_id": experiment_id,
            "status": "reverted"
        })))
    }
}
