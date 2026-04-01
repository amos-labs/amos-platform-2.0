//! Core types for the autoresearch package.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

// ─── Enums ──────────────────────────────────────────────────────────

/// How tasks are routed within a swarm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingStrategy {
    RoundRobin,
    Capability,
    Load,
    Fitness,
    Hierarchical,
}

impl RoutingStrategy {
    pub fn as_str(&self) -> &str {
        match self {
            Self::RoundRobin => "round_robin",
            Self::Capability => "capability",
            Self::Load => "load",
            Self::Fitness => "fitness",
            Self::Hierarchical => "hierarchical",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "round_robin" => Some(Self::RoundRobin),
            "capability" => Some(Self::Capability),
            "load" => Some(Self::Load),
            "fitness" => Some(Self::Fitness),
            "hierarchical" => Some(Self::Hierarchical),
            _ => None,
        }
    }
}

/// Role of an agent within a swarm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmRole {
    Leader,
    Worker,
    Evaluator,
}

impl SwarmRole {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Leader => "leader",
            Self::Worker => "worker",
            Self::Evaluator => "evaluator",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "leader" => Some(Self::Leader),
            "worker" => Some(Self::Worker),
            "evaluator" => Some(Self::Evaluator),
            _ => None,
        }
    }
}

/// Source for fitness metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricSource {
    Internal,
    External,
    Webhook,
}

impl MetricSource {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Internal => "internal",
            Self::External => "external",
            Self::Webhook => "webhook",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "internal" => Some(Self::Internal),
            "external" => Some(Self::External),
            "webhook" => Some(Self::Webhook),
            _ => None,
        }
    }
}

/// Metric types — universal, business, and trading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    // Universal
    TaskCompletionRate,
    QualityScore,
    Custom,
    // Business
    Engagement,
    Conversion,
    Revenue,
    Profit,
    // Trading
    SharpeRatio,
    SortinoRatio,
    MaxDrawdown,
    TotalReturn,
    WinRate,
}

impl MetricType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::TaskCompletionRate => "task_completion_rate",
            Self::QualityScore => "quality_score",
            Self::Custom => "custom",
            Self::Engagement => "engagement",
            Self::Conversion => "conversion",
            Self::Revenue => "revenue",
            Self::Profit => "profit",
            Self::SharpeRatio => "sharpe_ratio",
            Self::SortinoRatio => "sortino_ratio",
            Self::MaxDrawdown => "max_drawdown",
            Self::TotalReturn => "total_return",
            Self::WinRate => "win_rate",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "task_completion_rate" => Some(Self::TaskCompletionRate),
            "quality_score" => Some(Self::QualityScore),
            "custom" => Some(Self::Custom),
            "engagement" => Some(Self::Engagement),
            "conversion" => Some(Self::Conversion),
            "revenue" => Some(Self::Revenue),
            "profit" => Some(Self::Profit),
            "sharpe_ratio" => Some(Self::SharpeRatio),
            "sortino_ratio" => Some(Self::SortinoRatio),
            "max_drawdown" => Some(Self::MaxDrawdown),
            "total_return" => Some(Self::TotalReturn),
            "win_rate" => Some(Self::WinRate),
            _ => None,
        }
    }
}

/// Status of a Darwinian experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentStatus {
    Proposed,
    Active,
    Evaluating,
    Accepted,
    Reverted,
    Expired,
}

impl ExperimentStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Proposed => "proposed",
            Self::Active => "active",
            Self::Evaluating => "evaluating",
            Self::Accepted => "accepted",
            Self::Reverted => "reverted",
            Self::Expired => "expired",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "proposed" => Some(Self::Proposed),
            "active" => Some(Self::Active),
            "evaluating" => Some(Self::Evaluating),
            "accepted" => Some(Self::Accepted),
            "reverted" => Some(Self::Reverted),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }
}

/// Type of experiment mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentType {
    PromptMutation,
    ModelChange,
    ToolChange,
    ParameterTune,
}

impl ExperimentType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::PromptMutation => "prompt_mutation",
            Self::ModelChange => "model_change",
            Self::ToolChange => "tool_change",
            Self::ParameterTune => "parameter_tune",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "prompt_mutation" => Some(Self::PromptMutation),
            "model_change" => Some(Self::ModelChange),
            "tool_change" => Some(Self::ToolChange),
            "parameter_tune" => Some(Self::ParameterTune),
            _ => None,
        }
    }
}

// ─── Domain Structs ─────────────────────────────────────────────────

/// An agent swarm — a coordinated group of agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Swarm {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub parent_swarm_id: Option<Uuid>,
    pub layer_order: i32,
    pub routing_strategy: String,
    pub max_agents: i32,
    pub enabled: bool,
    pub domain: String,
    pub metadata: JsonValue,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A member of a swarm (agent ↔ swarm mapping).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmMember {
    pub id: Uuid,
    pub swarm_id: Uuid,
    pub agent_id: i32,
    pub weight: f64,
    pub fitness_score: Option<f64>,
    pub role: String,
    pub joined_at: DateTime<Utc>,
}

/// A configurable fitness function for a swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessFunction {
    pub id: Uuid,
    pub swarm_id: Uuid,
    pub name: String,
    pub metric_source: String,
    pub metric_type: String,
    pub metric_query: Option<String>,
    pub metric_endpoint: Option<String>,
    pub metric_config: JsonValue,
    pub window_days: i32,
    pub weight: f64,
    pub last_value: Option<f64>,
    pub last_computed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A Darwinian experiment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub id: Uuid,
    pub swarm_id: Uuid,
    pub agent_id: i32,
    pub experiment_type: String,
    pub diff: JsonValue,
    pub original_prompt: Option<String>,
    pub mutated_prompt: Option<String>,
    pub status: String,
    pub baseline_fitness: Option<f64>,
    pub final_fitness: Option<f64>,
    pub fitness_delta: Option<f64>,
    pub evaluation_days: i32,
    pub cooldown_days: i32,
    pub proposed_by: Option<String>,
    pub proposal_reasoning: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Rolling performance snapshot for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scorecard {
    pub id: Uuid,
    pub agent_id: i32,
    pub swarm_id: Uuid,
    pub fitness_score: f64,
    pub tasks_completed: i32,
    pub tasks_failed: i32,
    pub avg_task_duration_ms: Option<i64>,
    pub total_tokens_used: i64,
    pub total_cost_usd: f64,
    pub metric_scores: JsonValue,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub weight_at_snapshot: f64,
    pub created_at: DateTime<Utc>,
}

/// Attribution record linking a task outcome to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAttribution {
    pub id: Uuid,
    pub agent_id: i32,
    pub task_id: Uuid,
    pub swarm_id: Option<Uuid>,
    pub tokens_used: i64,
    pub cost_usd: f64,
    pub duration_ms: i64,
    pub quality_score: Option<f64>,
    pub metric_impact: JsonValue,
    pub created_at: DateTime<Utc>,
}

// ─── API Request/Response Types ─────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateSwarmRequest {
    pub name: String,
    pub description: Option<String>,
    pub parent_swarm_id: Option<Uuid>,
    pub layer_order: Option<i32>,
    pub routing_strategy: Option<String>,
    pub max_agents: Option<i32>,
    pub domain: Option<String>,
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSwarmRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub routing_strategy: Option<String>,
    pub max_agents: Option<i32>,
    pub enabled: Option<bool>,
    pub domain: Option<String>,
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub agent_id: i32,
    pub role: Option<String>,
    pub weight: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct RemoveMemberRequest {
    pub agent_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct RouteTaskRequest {
    pub task_description: String,
    pub required_capabilities: Option<Vec<String>>,
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFitnessFunctionRequest {
    pub swarm_id: Uuid,
    pub name: String,
    pub metric_source: String,
    pub metric_type: String,
    pub metric_query: Option<String>,
    pub metric_endpoint: Option<String>,
    pub metric_config: Option<JsonValue>,
    pub window_days: Option<i32>,
    pub weight: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookReportRequest {
    pub agent_id: i32,
    pub value: f64,
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExperimentRequest {
    pub swarm_id: Uuid,
    pub agent_id: i32,
    pub experiment_type: Option<String>,
    pub evaluation_days: Option<i32>,
    pub cooldown_days: Option<i32>,
}

/// Summary for dashboard view.
#[derive(Debug, Serialize)]
pub struct DashboardStats {
    pub total_swarms: i64,
    pub enabled_swarms: i64,
    pub total_agents_in_swarms: i64,
    pub active_experiments: i64,
    pub experiments_accepted: i64,
    pub experiments_reverted: i64,
    pub avg_fitness: Option<f64>,
}
