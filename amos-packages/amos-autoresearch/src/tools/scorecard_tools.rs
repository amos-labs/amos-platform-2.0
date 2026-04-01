//! Scorecard and fitness tools for the agent.

use amos_core::{tools::ToolCategory, Result, Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use uuid::Uuid;

use crate::fitness::collector::ScorecardCollector;
use crate::fitness::FitnessEngine;
use crate::types::*;

// ─── DefineFitnessFunctionTool ──────────────────────────────────────

pub struct DefineFitnessFunctionTool {
    db_pool: PgPool,
}

impl DefineFitnessFunctionTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for DefineFitnessFunctionTool {
    fn name(&self) -> &str {
        "define_fitness_function"
    }

    fn description(&self) -> &str {
        "Configure a fitness metric for a swarm. Supports internal (SQL/collection), external (HTTP API), or webhook metric sources. Multiple fitness functions per swarm are combined as a weighted average."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "swarm_id": { "type": "string", "description": "Swarm UUID" },
                "name": { "type": "string", "description": "Metric name (e.g., 'Sharpe Ratio', 'Task Completion Rate')" },
                "metric_source": { "type": "string", "enum": ["internal", "external", "webhook"], "description": "Where the metric comes from" },
                "metric_type": {
                    "type": "string",
                    "enum": ["task_completion_rate", "quality_score", "custom", "engagement", "conversion", "revenue", "profit", "sharpe_ratio", "sortino_ratio", "max_drawdown", "total_return", "win_rate"],
                    "description": "Type of metric"
                },
                "metric_query": { "type": "string", "description": "Raw SQL query for internal/custom metrics (must return a single float)" },
                "metric_endpoint": { "type": "string", "description": "HTTP endpoint for external metrics" },
                "metric_config": { "type": "object", "description": "Additional config (collection, auth, JSONPath, etc.)" },
                "window_days": { "type": "integer", "description": "Rolling window in days (default 60)" },
                "weight": { "type": "number", "description": "Weight in composite score (default 1.0)" }
            },
            "required": ["swarm_id", "name", "metric_source", "metric_type"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Autoresearch
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let req = CreateFitnessFunctionRequest {
            swarm_id: params
                .get("swarm_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| amos_core::AmosError::Validation("swarm_id is required".into()))?,
            name: params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            metric_source: params
                .get("metric_source")
                .and_then(|v| v.as_str())
                .unwrap_or("internal")
                .to_string(),
            metric_type: params
                .get("metric_type")
                .and_then(|v| v.as_str())
                .unwrap_or("task_completion_rate")
                .to_string(),
            metric_query: params
                .get("metric_query")
                .and_then(|v| v.as_str())
                .map(String::from),
            metric_endpoint: params
                .get("metric_endpoint")
                .and_then(|v| v.as_str())
                .map(String::from),
            metric_config: params.get("metric_config").cloned(),
            window_days: params
                .get("window_days")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32),
            weight: params.get("weight").and_then(|v| v.as_f64()),
        };

        let http_client = reqwest::Client::new();
        let engine = FitnessEngine::new(self.db_pool.clone(), http_client);
        let func = engine.create_function(&req).await?;

        Ok(ToolResult::success(json!({
            "id": func.id,
            "swarm_id": func.swarm_id,
            "name": func.name,
            "metric_source": func.metric_source,
            "metric_type": func.metric_type,
            "window_days": func.window_days,
            "weight": func.weight,
            "status": "created"
        })))
    }
}

// ─── ComputeFitnessTool ─────────────────────────────────────────────

pub struct ComputeFitnessTool {
    db_pool: PgPool,
}

impl ComputeFitnessTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ComputeFitnessTool {
    fn name(&self) -> &str {
        "compute_fitness"
    }

    fn description(&self) -> &str {
        "Manually trigger fitness computation for all agents in a swarm. Updates scorecards and member fitness scores."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "swarm_id": { "type": "string", "description": "Swarm UUID" },
                "window_days": { "type": "integer", "description": "Override window days (default: use function settings)" }
            },
            "required": ["swarm_id"]
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

        let window_days = params
            .get("window_days")
            .and_then(|v| v.as_i64())
            .unwrap_or(60) as i32;

        let http_client = reqwest::Client::new();
        let collector = ScorecardCollector::new(self.db_pool.clone(), http_client);
        let scorecards = collector.collect_scorecards(swarm_id, window_days).await?;

        let results: Vec<JsonValue> = scorecards
            .iter()
            .map(|s| {
                json!({
                    "agent_id": s.agent_id,
                    "fitness_score": s.fitness_score,
                    "tasks_completed": s.tasks_completed,
                    "tasks_failed": s.tasks_failed,
                    "total_tokens_used": s.total_tokens_used,
                    "total_cost_usd": s.total_cost_usd,
                })
            })
            .collect();

        Ok(ToolResult::success(json!({
            "swarm_id": swarm_id,
            "scorecards": results,
            "agents_scored": results.len()
        })))
    }
}

// ─── ViewScorecardTool ──────────────────────────────────────────────

pub struct ViewScorecardTool {
    db_pool: PgPool,
}

impl ViewScorecardTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ViewScorecardTool {
    fn name(&self) -> &str {
        "view_scorecard"
    }

    fn description(&self) -> &str {
        "View an agent's performance scorecard including fitness, task stats, token usage, and cost."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "agent_id": { "type": "integer", "description": "Agent ID" },
                "swarm_id": { "type": "string", "description": "Swarm UUID" }
            },
            "required": ["agent_id", "swarm_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Autoresearch
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let agent_id = params
            .get("agent_id")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .ok_or_else(|| amos_core::AmosError::Validation("agent_id is required".into()))?;

        let swarm_id = params
            .get("swarm_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| amos_core::AmosError::Validation("swarm_id is required".into()))?;

        let http_client = reqwest::Client::new();
        let collector = ScorecardCollector::new(self.db_pool.clone(), http_client);
        let scorecard = collector.get_latest_scorecard(agent_id, swarm_id).await?;

        match scorecard {
            Some(sc) => Ok(ToolResult::success(json!({
                "agent_id": sc.agent_id,
                "swarm_id": sc.swarm_id,
                "fitness_score": sc.fitness_score,
                "tasks_completed": sc.tasks_completed,
                "tasks_failed": sc.tasks_failed,
                "avg_task_duration_ms": sc.avg_task_duration_ms,
                "total_tokens_used": sc.total_tokens_used,
                "total_cost_usd": sc.total_cost_usd,
                "metric_scores": sc.metric_scores,
                "weight_at_snapshot": sc.weight_at_snapshot,
                "window_start": sc.window_start.to_rfc3339(),
                "window_end": sc.window_end.to_rfc3339(),
            }))),
            None => Ok(ToolResult::success(json!({
                "agent_id": agent_id,
                "message": "No scorecard found. Run compute_fitness first."
            }))),
        }
    }
}

// ─── CompareAgentsTool ──────────────────────────────────────────────

pub struct CompareAgentsTool {
    db_pool: PgPool,
}

impl CompareAgentsTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for CompareAgentsTool {
    fn name(&self) -> &str {
        "compare_agents"
    }

    fn description(&self) -> &str {
        "Side-by-side comparison of agents within a swarm, showing fitness, task stats, weights, and roles."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "swarm_id": { "type": "string", "description": "Swarm UUID" }
            },
            "required": ["swarm_id"]
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

        let http_client = reqwest::Client::new();
        let collector = ScorecardCollector::new(self.db_pool.clone(), http_client);
        let scorecards = collector.get_swarm_scorecards(swarm_id).await?;

        use sqlx::Row;
        // Also get member info
        let members = sqlx::query(
            "SELECT m.agent_id, m.weight, m.role, m.fitness_score, a.name, a.model
             FROM agent_swarm_members m
             JOIN openclaw_agents a ON a.id = m.agent_id
             WHERE m.swarm_id = $1
             ORDER BY m.fitness_score DESC NULLS LAST",
        )
        .bind(swarm_id)
        .fetch_all(&self.db_pool)
        .await?;

        let agents: Vec<JsonValue> = members
            .iter()
            .map(|row| {
                let agent_id: i32 = row.get("agent_id");
                let sc = scorecards.iter().find(|s| s.agent_id == agent_id);
                json!({
                    "agent_id": agent_id,
                    "name": row.get::<String, _>("name"),
                    "model": row.get::<String, _>("model"),
                    "role": row.get::<String, _>("role"),
                    "weight": row.get::<f64, _>("weight"),
                    "fitness_score": row.get::<Option<f64>, _>("fitness_score"),
                    "tasks_completed": sc.map(|s| s.tasks_completed),
                    "tasks_failed": sc.map(|s| s.tasks_failed),
                    "total_tokens_used": sc.map(|s| s.total_tokens_used),
                    "total_cost_usd": sc.map(|s| s.total_cost_usd),
                })
            })
            .collect();

        Ok(ToolResult::success(json!({
            "swarm_id": swarm_id,
            "agents": agents,
            "count": agents.len()
        })))
    }
}
