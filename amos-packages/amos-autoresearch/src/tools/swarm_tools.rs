//! Swarm management tools for the agent.

use amos_core::{tools::ToolCategory, Result, Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use uuid::Uuid;

use crate::swarm::router::SwarmRouter;
use crate::swarm::SwarmManager;
use crate::types::*;

// ─── CreateSwarmTool ────────────────────────────────────────────────

pub struct CreateSwarmTool {
    db_pool: PgPool,
}

impl CreateSwarmTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for CreateSwarmTool {
    fn name(&self) -> &str {
        "create_swarm"
    }

    fn description(&self) -> &str {
        "Create an agent swarm — a coordinated group of agents with a routing strategy. Supports round_robin, capability, load, fitness, or hierarchical routing."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Swarm name" },
                "description": { "type": "string", "description": "What this swarm does" },
                "routing_strategy": { "type": "string", "enum": ["round_robin", "capability", "load", "fitness", "hierarchical"], "description": "How tasks are routed to agents" },
                "max_agents": { "type": "integer", "description": "Maximum number of agents (default 10)" },
                "domain": { "type": "string", "description": "Domain: trading, marketing, sales, support, or custom" },
                "parent_swarm_id": { "type": "string", "description": "Parent swarm UUID for hierarchical nesting" },
                "layer_order": { "type": "integer", "description": "Execution order within parent swarm" }
            },
            "required": ["name"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Autoresearch
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let req = CreateSwarmRequest {
            name: params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            description: params
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from),
            parent_swarm_id: params
                .get("parent_swarm_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok()),
            layer_order: params
                .get("layer_order")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32),
            routing_strategy: params
                .get("routing_strategy")
                .and_then(|v| v.as_str())
                .map(String::from),
            max_agents: params
                .get("max_agents")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32),
            domain: params
                .get("domain")
                .and_then(|v| v.as_str())
                .map(String::from),
            metadata: params.get("metadata").cloned(),
        };

        let mgr = SwarmManager::new(self.db_pool.clone());
        let swarm = mgr.create_swarm(&req).await?;

        Ok(ToolResult::success(json!({
            "id": swarm.id,
            "name": swarm.name,
            "routing_strategy": swarm.routing_strategy,
            "domain": swarm.domain,
            "max_agents": swarm.max_agents,
            "status": "created"
        })))
    }
}

// ─── ListSwarmsTool ─────────────────────────────────────────────────

pub struct ListSwarmsTool {
    db_pool: PgPool,
}

impl ListSwarmsTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ListSwarmsTool {
    fn name(&self) -> &str {
        "list_swarms"
    }

    fn description(&self) -> &str {
        "List all agent swarms with their agent counts and routing strategies."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "enabled_only": { "type": "boolean", "description": "Only show enabled swarms (default true)" }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Autoresearch
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let enabled_only = params
            .get("enabled_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let mgr = SwarmManager::new(self.db_pool.clone());
        let swarms = if enabled_only {
            mgr.list_enabled_swarms().await?
        } else {
            mgr.list_swarms().await?
        };

        let results: Vec<JsonValue> = swarms
            .into_iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "name": s.name,
                    "description": s.description,
                    "routing_strategy": s.routing_strategy,
                    "domain": s.domain,
                    "enabled": s.enabled,
                    "max_agents": s.max_agents,
                })
            })
            .collect();

        Ok(ToolResult::success(json!({
            "swarms": results,
            "count": results.len()
        })))
    }
}

// ─── AddAgentToSwarmTool ────────────────────────────────────────────

pub struct AddAgentToSwarmTool {
    db_pool: PgPool,
}

impl AddAgentToSwarmTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for AddAgentToSwarmTool {
    fn name(&self) -> &str {
        "add_agent_to_swarm"
    }

    fn description(&self) -> &str {
        "Add an agent to a swarm with a role (leader, worker, or evaluator)."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "swarm_id": { "type": "string", "description": "Swarm UUID" },
                "agent_id": { "type": "integer", "description": "Agent ID from openclaw_agents" },
                "role": { "type": "string", "enum": ["leader", "worker", "evaluator"], "description": "Agent's role in the swarm (default: worker)" },
                "weight": { "type": "number", "description": "Initial Darwinian weight (default 1.0)" }
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

        let req = AddMemberRequest {
            agent_id,
            role: params
                .get("role")
                .and_then(|v| v.as_str())
                .map(String::from),
            weight: params.get("weight").and_then(|v| v.as_f64()),
        };

        let mgr = SwarmManager::new(self.db_pool.clone());
        let member = mgr.add_member(swarm_id, &req).await?;

        Ok(ToolResult::success(json!({
            "swarm_id": member.swarm_id,
            "agent_id": member.agent_id,
            "role": member.role,
            "weight": member.weight,
            "status": "added"
        })))
    }
}

// ─── RemoveAgentFromSwarmTool ───────────────────────────────────────

pub struct RemoveAgentFromSwarmTool {
    db_pool: PgPool,
}

impl RemoveAgentFromSwarmTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for RemoveAgentFromSwarmTool {
    fn name(&self) -> &str {
        "remove_agent_from_swarm"
    }

    fn description(&self) -> &str {
        "Remove an agent from a swarm."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "swarm_id": { "type": "string", "description": "Swarm UUID" },
                "agent_id": { "type": "integer", "description": "Agent ID" }
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

        let mgr = SwarmManager::new(self.db_pool.clone());
        mgr.remove_member(swarm_id, agent_id).await?;

        Ok(ToolResult::success(json!({
            "swarm_id": swarm_id,
            "agent_id": agent_id,
            "status": "removed"
        })))
    }
}

// ─── RouteTaskToSwarmTool ───────────────────────────────────────────

pub struct RouteTaskToSwarmTool {
    db_pool: PgPool,
}

impl RouteTaskToSwarmTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for RouteTaskToSwarmTool {
    fn name(&self) -> &str {
        "route_task_to_swarm"
    }

    fn description(&self) -> &str {
        "Dispatch a task through a swarm's routing strategy to select the best agent."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "swarm_id": { "type": "string", "description": "Swarm UUID" },
                "task_description": { "type": "string", "description": "What the task is about" },
                "required_capabilities": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Required agent capabilities (for capability routing)"
                }
            },
            "required": ["swarm_id", "task_description"]
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

        let req = RouteTaskRequest {
            task_description: params
                .get("task_description")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            required_capabilities: params
                .get("required_capabilities")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
            metadata: params.get("metadata").cloned(),
        };

        let router = SwarmRouter::new(self.db_pool.clone());
        let agent_id = router.route_task(swarm_id, &req).await?;

        Ok(ToolResult::success(json!({
            "swarm_id": swarm_id,
            "routed_to_agent_id": agent_id,
            "task_description": req.task_description,
            "status": "routed"
        })))
    }
}
