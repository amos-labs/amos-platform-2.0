//! HTTP routes for the autoresearch package.
//!
//! Nested under `/api/v1/pkg/autoresearch/` by the harness.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value as JsonValue};
use uuid::Uuid;

use crate::darwinian::evaluator::Evaluator;
use crate::darwinian::mutator::Mutator;
use crate::fitness::collector::ScorecardCollector;
use crate::fitness::FitnessEngine;
use crate::swarm::router::SwarmRouter;
use crate::swarm::SwarmManager;
use crate::types::*;
use crate::AutoresearchState;

pub fn autoresearch_routes(state: AutoresearchState) -> Router {
    Router::new()
        // Swarm CRUD
        .route("/swarms", get(list_swarms).post(create_swarm))
        .route(
            "/swarms/{id}",
            get(get_swarm).put(update_swarm).delete(delete_swarm),
        )
        .route(
            "/swarms/{id}/members",
            get(list_members).post(add_member).delete(remove_member),
        )
        .route("/swarms/{id}/route", post(route_task))
        .route("/swarms/{id}/scorecards", get(swarm_scorecards))
        // Experiments
        .route(
            "/experiments",
            get(list_experiments).post(create_experiment),
        )
        .route("/experiments/{id}", get(get_experiment))
        .route("/experiments/{id}/revert", post(revert_experiment))
        // Fitness
        .route(
            "/fitness",
            get(list_fitness_functions).post(create_fitness_function),
        )
        .route("/fitness/{id}/compute", post(compute_fitness))
        .route("/fitness/{id}/report", post(webhook_report))
        // Agent
        .route("/agents/{id}/scorecard", get(agent_scorecard))
        // Dashboard
        .route("/dashboard", get(dashboard))
        .with_state(state)
}

// ─── Helpers ────────────────────────────────────────────────────────

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn ok_json(data: JsonValue) -> Response {
    (StatusCode::OK, Json(data)).into_response()
}

// ─── Swarm Routes ───────────────────────────────────────────────────

async fn list_swarms(State(state): State<AutoresearchState>) -> Response {
    let mgr = SwarmManager::new(state.db_pool);
    match mgr.list_swarms().await {
        Ok(swarms) => ok_json(json!({ "swarms": swarms })),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn create_swarm(
    State(state): State<AutoresearchState>,
    Json(req): Json<CreateSwarmRequest>,
) -> Response {
    let mgr = SwarmManager::new(state.db_pool);
    match mgr.create_swarm(&req).await {
        Ok(swarm) => (StatusCode::CREATED, Json(json!(swarm))).into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

async fn get_swarm(State(state): State<AutoresearchState>, Path(id): Path<Uuid>) -> Response {
    let mgr = SwarmManager::new(state.db_pool);
    match mgr.get_swarm(id).await {
        Ok(Some(swarm)) => ok_json(json!(swarm)),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Swarm not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn update_swarm(
    State(state): State<AutoresearchState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateSwarmRequest>,
) -> Response {
    let mgr = SwarmManager::new(state.db_pool);
    match mgr.update_swarm(id, &req).await {
        Ok(swarm) => ok_json(json!(swarm)),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

async fn delete_swarm(State(state): State<AutoresearchState>, Path(id): Path<Uuid>) -> Response {
    let mgr = SwarmManager::new(state.db_pool);
    match mgr.delete_swarm(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ─── Member Routes ──────────────────────────────────────────────────

async fn list_members(State(state): State<AutoresearchState>, Path(id): Path<Uuid>) -> Response {
    let mgr = SwarmManager::new(state.db_pool);
    match mgr.list_members(id).await {
        Ok(members) => ok_json(json!({ "members": members })),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn add_member(
    State(state): State<AutoresearchState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddMemberRequest>,
) -> Response {
    let mgr = SwarmManager::new(state.db_pool);
    match mgr.add_member(id, &req).await {
        Ok(member) => (StatusCode::CREATED, Json(json!(member))).into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

async fn remove_member(
    State(state): State<AutoresearchState>,
    Path(id): Path<Uuid>,
    Json(req): Json<RemoveMemberRequest>,
) -> Response {
    let mgr = SwarmManager::new(state.db_pool);
    match mgr.remove_member(id, req.agent_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ─── Task Routing ───────────────────────────────────────────────────

async fn route_task(
    State(state): State<AutoresearchState>,
    Path(id): Path<Uuid>,
    Json(req): Json<RouteTaskRequest>,
) -> Response {
    let router = SwarmRouter::new(state.db_pool);
    match router.route_task(id, &req).await {
        Ok(agent_id) => ok_json(json!({
            "swarm_id": id,
            "routed_to_agent_id": agent_id,
        })),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// ─── Experiment Routes ──────────────────────────────────────────────

async fn list_experiments(State(state): State<AutoresearchState>) -> Response {
    use sqlx::Row;
    let rows =
        sqlx::query("SELECT * FROM autoresearch_experiments ORDER BY created_at DESC LIMIT 50")
            .fetch_all(&state.db_pool)
            .await;

    match rows {
        Ok(rows) => {
            let experiments: Vec<JsonValue> = rows
                .iter()
                .map(|row| {
                    json!({
                        "id": row.get::<Uuid, _>("id"),
                        "swarm_id": row.get::<Uuid, _>("swarm_id"),
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
            ok_json(json!({ "experiments": experiments }))
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn create_experiment(
    State(state): State<AutoresearchState>,
    Json(req): Json<CreateExperimentRequest>,
) -> Response {
    let http_client = reqwest::Client::new();
    let mutator = Mutator::new(state.db_pool, http_client);
    match mutator.propose_mutation(req.agent_id, req.swarm_id).await {
        Ok(experiment) => (StatusCode::CREATED, Json(json!(experiment))).into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

async fn get_experiment(State(state): State<AutoresearchState>, Path(id): Path<Uuid>) -> Response {
    use sqlx::Row;
    let row = sqlx::query("SELECT * FROM autoresearch_experiments WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await;

    match row {
        Ok(Some(row)) => ok_json(json!({
            "id": row.get::<Uuid, _>("id"),
            "swarm_id": row.get::<Uuid, _>("swarm_id"),
            "agent_id": row.get::<i32, _>("agent_id"),
            "experiment_type": row.get::<String, _>("experiment_type"),
            "status": row.get::<String, _>("status"),
            "diff": row.get::<JsonValue, _>("diff"),
            "original_prompt": row.get::<Option<String>, _>("original_prompt"),
            "mutated_prompt": row.get::<Option<String>, _>("mutated_prompt"),
            "baseline_fitness": row.get::<Option<f64>, _>("baseline_fitness"),
            "final_fitness": row.get::<Option<f64>, _>("final_fitness"),
            "fitness_delta": row.get::<Option<f64>, _>("fitness_delta"),
            "evaluation_days": row.get::<i32, _>("evaluation_days"),
            "proposed_by": row.get::<Option<String>, _>("proposed_by"),
            "proposal_reasoning": row.get::<Option<String>, _>("proposal_reasoning"),
            "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
        })),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Experiment not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn revert_experiment(
    State(state): State<AutoresearchState>,
    Path(id): Path<Uuid>,
) -> Response {
    let evaluator = Evaluator::new(state.db_pool);
    match evaluator.revert_experiment(id).await {
        Ok(()) => ok_json(json!({ "id": id, "status": "reverted" })),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// ─── Fitness Routes ─────────────────────────────────────────────────

async fn list_fitness_functions(State(state): State<AutoresearchState>) -> Response {
    use sqlx::Row;
    let rows = sqlx::query("SELECT * FROM fitness_functions ORDER BY created_at DESC")
        .fetch_all(&state.db_pool)
        .await;

    match rows {
        Ok(rows) => {
            let funcs: Vec<JsonValue> = rows
                .iter()
                .map(|row| {
                    json!({
                        "id": row.get::<Uuid, _>("id"),
                        "swarm_id": row.get::<Uuid, _>("swarm_id"),
                        "name": row.get::<String, _>("name"),
                        "metric_source": row.get::<String, _>("metric_source"),
                        "metric_type": row.get::<String, _>("metric_type"),
                        "window_days": row.get::<i32, _>("window_days"),
                        "weight": row.get::<f64, _>("weight"),
                        "last_value": row.get::<Option<f64>, _>("last_value"),
                    })
                })
                .collect();
            ok_json(json!({ "fitness_functions": funcs }))
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn create_fitness_function(
    State(state): State<AutoresearchState>,
    Json(req): Json<CreateFitnessFunctionRequest>,
) -> Response {
    let http_client = reqwest::Client::new();
    let engine = FitnessEngine::new(state.db_pool, http_client);
    match engine.create_function(&req).await {
        Ok(func) => (StatusCode::CREATED, Json(json!(func))).into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

async fn compute_fitness(State(state): State<AutoresearchState>, Path(id): Path<Uuid>) -> Response {
    // id here is the fitness_function_id, but we need the swarm_id
    use sqlx::Row;
    let func_row = sqlx::query("SELECT swarm_id FROM fitness_functions WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await;

    match func_row {
        Ok(Some(row)) => {
            let swarm_id: Uuid = row.get("swarm_id");
            let http_client = reqwest::Client::new();
            let collector = ScorecardCollector::new(state.db_pool, http_client);
            match collector.collect_scorecards(swarm_id, 60).await {
                Ok(scorecards) => ok_json(json!({
                    "swarm_id": swarm_id,
                    "agents_scored": scorecards.len(),
                    "scorecards": scorecards,
                })),
                Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Fitness function not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn webhook_report(
    State(state): State<AutoresearchState>,
    Path(id): Path<Uuid>,
    Json(req): Json<WebhookReportRequest>,
) -> Response {
    let http_client = reqwest::Client::new();
    let engine = FitnessEngine::new(state.db_pool, http_client);
    match engine.record_webhook(id, &req).await {
        Ok(()) => ok_json(json!({ "status": "recorded", "function_id": id })),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// ─── Agent Scorecard ────────────────────────────────────────────────

async fn agent_scorecard(State(state): State<AutoresearchState>, Path(id): Path<i32>) -> Response {
    use sqlx::Row;
    let rows = sqlx::query(
        "SELECT * FROM agent_scorecards WHERE agent_id = $1 ORDER BY created_at DESC LIMIT 10",
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await;

    match rows {
        Ok(rows) => {
            let scorecards: Vec<JsonValue> = rows
                .iter()
                .map(|row| {
                    json!({
                        "id": row.get::<Uuid, _>("id"),
                        "swarm_id": row.get::<Uuid, _>("swarm_id"),
                        "fitness_score": row.get::<f64, _>("fitness_score"),
                        "tasks_completed": row.get::<i32, _>("tasks_completed"),
                        "tasks_failed": row.get::<i32, _>("tasks_failed"),
                        "total_tokens_used": row.get::<i64, _>("total_tokens_used"),
                        "total_cost_usd": row.get::<f64, _>("total_cost_usd"),
                        "metric_scores": row.get::<JsonValue, _>("metric_scores"),
                        "weight_at_snapshot": row.get::<f64, _>("weight_at_snapshot"),
                        "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339(),
                    })
                })
                .collect();
            ok_json(json!({ "agent_id": id, "scorecards": scorecards }))
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ─── Swarm Scorecards ───────────────────────────────────────────────

async fn swarm_scorecards(
    State(state): State<AutoresearchState>,
    Path(id): Path<Uuid>,
) -> Response {
    let http_client = reqwest::Client::new();
    let collector = ScorecardCollector::new(state.db_pool, http_client);
    match collector.get_swarm_scorecards(id).await {
        Ok(scorecards) => ok_json(json!({
            "swarm_id": id,
            "scorecards": scorecards,
        })),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ─── Dashboard ──────────────────────────────────────────────────────

async fn dashboard(State(state): State<AutoresearchState>) -> Response {
    let stats = async {
        let total_swarms: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_swarms")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

        let enabled_swarms: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_swarms WHERE enabled = true")
                .fetch_one(&state.db_pool)
                .await
                .unwrap_or(0);

        let total_agents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_swarm_members")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

        let active_experiments: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM autoresearch_experiments WHERE status = 'active'",
        )
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

        let accepted: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM autoresearch_experiments WHERE status = 'accepted'",
        )
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

        let reverted: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM autoresearch_experiments WHERE status = 'reverted'",
        )
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

        let avg_fitness: Option<f64> =
            sqlx::query_scalar("SELECT AVG(fitness_score) FROM agent_swarm_members")
                .fetch_one(&state.db_pool)
                .await
                .unwrap_or(None);

        DashboardStats {
            total_swarms,
            enabled_swarms,
            total_agents_in_swarms: total_agents,
            active_experiments,
            experiments_accepted: accepted,
            experiments_reverted: reverted,
            avg_fitness,
        }
    }
    .await;

    ok_json(json!(stats))
}
