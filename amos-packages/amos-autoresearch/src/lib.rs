//! # AMOS Autoresearch Package
//!
//! Extends the AMOS Harness with Darwinian optimization, swarm management,
//! and fitness-driven agent coordination:
//!
//! - **Swarm management** — Agent groups with routing (round-robin, capability, load, fitness, hierarchical)
//! - **Fitness engine** — Pluggable metrics: internal SQL, external API polling, inbound webhooks
//! - **Darwinian loop** — Background optimization: score agents, mutate worst performer's prompt, evaluate, keep/revert
//! - **Scorecards** — Rolling performance snapshots with per-agent attribution
//!
//! ## Usage
//!
//! Enable via environment variable:
//! ```bash
//! AMOS_PACKAGES=autoresearch
//! ```
//!
//! ## Tools (12 total)
//!
//! **Swarm**: create_swarm, list_swarms, add_agent_to_swarm, remove_agent_from_swarm, route_task_to_swarm
//! **Fitness**: define_fitness_function, compute_fitness, view_scorecard, compare_agents
//! **Experiments**: propose_experiment, view_experiments, revert_experiment

pub mod darwinian;
pub mod fitness;
pub mod routes;
pub mod swarm;
pub mod tools;
pub mod types;

use amos_core::{
    packages::{AmosPackage, PackageContext, PackageToolRegistry},
    Result,
};
use async_trait::async_trait;
use std::sync::Arc;

/// The autoresearch package — implements `AmosPackage` for harness loading.
pub struct AutoresearchPackage;

impl AutoresearchPackage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AutoresearchPackage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AmosPackage for AutoresearchPackage {
    fn name(&self) -> &str {
        "autoresearch"
    }

    fn display_name(&self) -> &str {
        "Autoresearch & Swarm Intelligence"
    }

    fn description(&self) -> &str {
        "Darwinian optimization, swarm management, and fitness-driven agent coordination"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn system_prompt(&self) -> Option<&str> {
        Some(
            r#"You have the Autoresearch & Swarm Intelligence package enabled. You can manage agent swarms, fitness metrics, and Darwinian optimization.

Key capabilities:
- **Swarms**: Use `create_swarm` to create agent groups with routing strategies (round_robin, capability, load, fitness, hierarchical). Use `add_agent_to_swarm` and `remove_agent_from_swarm` to manage membership. Use `route_task_to_swarm` to dispatch tasks through a swarm's router.
- **Fitness**: Use `define_fitness_function` to configure metrics (internal SQL, external API, webhook). Use `compute_fitness` to trigger scorecard computation. Use `view_scorecard` for individual agent performance and `compare_agents` for side-by-side comparison.
- **Darwinian optimization**: Use `propose_experiment` to trigger LLM-based prompt mutation for underperforming agents. Use `view_experiments` to track experiment status and fitness deltas. Use `revert_experiment` to manually undo a mutation.

The Darwinian loop runs automatically in the background, optimizing agent prompts every N hours. It scores agents, evaluates mature experiments, adjusts weights (top quartile get more work, bottom quartile less), and mutates the worst performer's prompt.

When users ask about agent coordination, swarm management, performance optimization, or self-improving agents, use these tools."#,
        )
    }

    fn register_tools(&self, registry: &mut dyn PackageToolRegistry, ctx: &PackageContext) {
        let db = ctx.db_pool.clone();
        let pkg = self.name();

        // Swarm tools (5)
        registry.register_package_tool(
            Arc::new(tools::swarm_tools::CreateSwarmTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::swarm_tools::ListSwarmsTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::swarm_tools::AddAgentToSwarmTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::swarm_tools::RemoveAgentFromSwarmTool::new(
                db.clone(),
            )),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::swarm_tools::RouteTaskToSwarmTool::new(db.clone())),
            pkg,
        );

        // Fitness/scorecard tools (4)
        registry.register_package_tool(
            Arc::new(tools::scorecard_tools::DefineFitnessFunctionTool::new(
                db.clone(),
            )),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::scorecard_tools::ComputeFitnessTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::scorecard_tools::ViewScorecardTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::scorecard_tools::CompareAgentsTool::new(db.clone())),
            pkg,
        );

        // Experiment tools (3)
        registry.register_package_tool(
            Arc::new(tools::experiment_tools::ProposeExperimentTool::new(
                db.clone(),
            )),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::experiment_tools::ViewExperimentsTool::new(
                db.clone(),
            )),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::experiment_tools::RevertExperimentTool::new(db)),
            pkg,
        );

        tracing::info!("Registered 12 autoresearch tools");
    }

    async fn on_activate(&self, ctx: &PackageContext) -> Result<()> {
        // Start the Darwinian background loop
        let interval_hours: u64 = std::env::var("AMOS_AUTORESEARCH_INTERVAL_HOURS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(6);

        let http_client = reqwest::Client::new();
        let fitness_engine = Arc::new(fitness::FitnessEngine::new(
            ctx.db_pool.clone(),
            http_client.clone(),
        ));
        let collector = Arc::new(fitness::collector::ScorecardCollector::new(
            ctx.db_pool.clone(),
            http_client,
        ));

        let darwin_loop = Arc::new(darwinian::DarwinianLoop::new(
            ctx.db_pool.clone(),
            fitness_engine,
            collector,
            interval_hours,
        ));
        darwin_loop.start();

        tracing::info!(
            "Autoresearch package activated — Darwinian loop running every {interval_hours}h"
        );
        Ok(())
    }
}

/// Shared state for autoresearch routes.
#[derive(Clone)]
pub struct AutoresearchState {
    pub db_pool: sqlx::PgPool,
}

/// Public route function — called by harness packages.rs (feature-gated).
pub fn package_routes(state: AutoresearchState) -> axum::Router {
    routes::autoresearch_routes(state)
}
