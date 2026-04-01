//! Fitness engine — central coordinator for computing agent fitness scores.
//!
//! Evaluates configurable fitness functions (internal metrics, external APIs,
//! webhooks) and produces weighted composite scores used by the Darwinian
//! optimization layer to rank, prune, and evolve agents within a swarm.

pub mod collector;
pub mod external;
pub mod metrics;
pub mod trading;

use crate::types::*;
use amos_core::{AmosError, Result};
use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Central coordinator that creates, manages, and evaluates fitness functions
/// for agent swarms. Each fitness function defines a metric source (internal
/// database query, external HTTP endpoint, or webhook push), a weight, and a
/// rolling evaluation window.
pub struct FitnessEngine {
    db_pool: PgPool,
    http_client: reqwest::Client,
}

impl FitnessEngine {
    /// Create a new `FitnessEngine` backed by the given database pool and HTTP
    /// client.
    pub fn new(db_pool: PgPool, http_client: reqwest::Client) -> Self {
        Self {
            db_pool,
            http_client,
        }
    }

    // ── CRUD ────────────────────────────────────────────────────────────

    /// Insert a new fitness function definition and return the created record.
    pub async fn create_function(
        &self,
        req: &CreateFitnessFunctionRequest,
    ) -> Result<FitnessFunction> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let metric_config = req.metric_config.clone().unwrap_or(serde_json::json!({}));
        let window_days = req.window_days.unwrap_or(30);
        let weight = req.weight.unwrap_or(1.0);

        let row = sqlx::query(
            r#"
            INSERT INTO fitness_functions
                (id, swarm_id, name, metric_source, metric_type, metric_query,
                 metric_endpoint, metric_config, window_days, weight,
                 created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id, swarm_id, name, metric_source, metric_type, metric_query,
                      metric_endpoint, metric_config, window_days, weight,
                      last_value, last_computed_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(req.swarm_id)
        .bind(&req.name)
        .bind(&req.metric_source)
        .bind(&req.metric_type)
        .bind(&req.metric_query)
        .bind(&req.metric_endpoint)
        .bind(&metric_config)
        .bind(window_days)
        .bind(weight)
        .bind(now)
        .bind(now)
        .fetch_one(&self.db_pool)
        .await?;

        Ok(row_to_fitness_function(&row))
    }

    /// List all fitness functions belonging to a swarm.
    pub async fn list_functions(&self, swarm_id: Uuid) -> Result<Vec<FitnessFunction>> {
        let rows = sqlx::query(
            r#"
            SELECT id, swarm_id, name, metric_source, metric_type, metric_query,
                   metric_endpoint, metric_config, window_days, weight,
                   last_value, last_computed_at, created_at, updated_at
            FROM fitness_functions
            WHERE swarm_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(swarm_id)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(rows.iter().map(row_to_fitness_function).collect())
    }

    /// Get a single fitness function by id.
    pub async fn get_function(&self, id: Uuid) -> Result<Option<FitnessFunction>> {
        let row = sqlx::query(
            r#"
            SELECT id, swarm_id, name, metric_source, metric_type, metric_query,
                   metric_endpoint, metric_config, window_days, weight,
                   last_value, last_computed_at, created_at, updated_at
            FROM fitness_functions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(row.as_ref().map(row_to_fitness_function))
    }

    /// Delete a fitness function.
    pub async fn delete_function(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM fitness_functions WHERE id = $1")
            .bind(id)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }

    // ── Fitness computation ─────────────────────────────────────────────

    /// Evaluate every fitness function for the given swarm/agent pair and
    /// return a weighted composite score.
    ///
    /// The composite is: `sum(weight_i * value_i) / sum(weight_i)` across
    /// all functions whose metric could be resolved.
    pub async fn compute_agent_fitness(&self, swarm_id: Uuid, agent_id: i32) -> Result<f64> {
        let functions = self.list_functions(swarm_id).await?;
        if functions.is_empty() {
            return Ok(0.0);
        }

        let mut weighted_sum = 0.0_f64;
        let mut total_weight = 0.0_f64;

        for func in &functions {
            let value = match MetricSource::parse(&func.metric_source) {
                Some(MetricSource::Internal) => {
                    metrics::compute_internal(&self.db_pool, func, agent_id).await
                }
                Some(MetricSource::External) => {
                    external::fetch_external(&self.http_client, func, agent_id).await
                }
                Some(MetricSource::Webhook) => {
                    // For webhooks the value is pushed externally and stored on the row.
                    Ok(func.last_value.unwrap_or(0.0))
                }
                None => {
                    tracing::warn!(
                        function_id = %func.id,
                        source = %func.metric_source,
                        "Unknown metric source — skipping"
                    );
                    continue;
                }
            };

            match value {
                Ok(v) => {
                    weighted_sum += func.weight * v;
                    total_weight += func.weight;

                    // Persist the computed value back for observability.
                    let _ = sqlx::query(
                        "UPDATE fitness_functions SET last_value = $1, last_computed_at = $2 WHERE id = $3",
                    )
                    .bind(v)
                    .bind(Utc::now())
                    .bind(func.id)
                    .execute(&self.db_pool)
                    .await;
                }
                Err(e) => {
                    tracing::error!(
                        function_id = %func.id,
                        error = %e,
                        "Failed to compute metric — skipping"
                    );
                }
            }
        }

        if total_weight == 0.0 {
            return Ok(0.0);
        }

        Ok(weighted_sum / total_weight)
    }

    /// Compute fitness for every agent in the swarm.
    ///
    /// Returns a vec of `(agent_id, fitness)` pairs.
    pub async fn compute_swarm_fitness(&self, swarm_id: Uuid) -> Result<Vec<(i32, f64)>> {
        let members = sqlx::query("SELECT agent_id FROM agent_swarm_members WHERE swarm_id = $1")
            .bind(swarm_id)
            .fetch_all(&self.db_pool)
            .await?;

        let mut results = Vec::with_capacity(members.len());
        for row in &members {
            let agent_id: i32 = row.get("agent_id");
            let score = self.compute_agent_fitness(swarm_id, agent_id).await?;
            results.push((agent_id, score));
        }

        Ok(results)
    }

    /// Record an externally-pushed webhook metric value for a fitness function.
    pub async fn record_webhook(
        &self,
        function_id: Uuid,
        req: &WebhookReportRequest,
    ) -> Result<()> {
        let updated = sqlx::query(
            r#"
            UPDATE fitness_functions
            SET last_value = $1,
                last_computed_at = $2,
                metric_config = metric_config || $3
            WHERE id = $4
            "#,
        )
        .bind(req.value)
        .bind(Utc::now())
        .bind(serde_json::json!({
            "last_agent_id": req.agent_id,
            "last_metadata": req.metadata,
        }))
        .bind(function_id)
        .execute(&self.db_pool)
        .await?;

        if updated.rows_affected() == 0 {
            return Err(AmosError::NotFound {
                entity: "FitnessFunction".into(),
                id: function_id.to_string(),
            });
        }

        Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Map a `sqlx::Row` to a `FitnessFunction`.
fn row_to_fitness_function(row: &sqlx::postgres::PgRow) -> FitnessFunction {
    FitnessFunction {
        id: row.get("id"),
        swarm_id: row.get("swarm_id"),
        name: row.get("name"),
        metric_source: row.get("metric_source"),
        metric_type: row.get("metric_type"),
        metric_query: row.get("metric_query"),
        metric_endpoint: row.get("metric_endpoint"),
        metric_config: row.get("metric_config"),
        window_days: row.get("window_days"),
        weight: row.get("weight"),
        last_value: row.get("last_value"),
        last_computed_at: row.get("last_computed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
