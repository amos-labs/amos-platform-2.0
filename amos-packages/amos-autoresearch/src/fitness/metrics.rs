//! Built-in internal metrics — queries executed against the local database to
//! measure agent performance within a rolling time window.
//!
//! Metric types:
//! - **task_completion_rate**: ratio of completed to total tasks
//! - **quality_score**: average quality score across attributed tasks
//! - **engagement / conversion / revenue**: collection-based JSONB metrics
//! - **profit**: revenue minus cost from task attribution
//! - **custom**: raw SQL query returning a single scalar float

use crate::types::{FitnessFunction, MetricType};
use amos_core::{AmosError, Result};
use chrono::{Duration, Utc};
use sqlx::{PgPool, Row};

/// Compute a metric from internal data sources.
///
/// The `function` argument carries the metric type, optional raw SQL query, and
/// JSON configuration that together determine which query to run.
pub async fn compute_internal(
    db: &PgPool,
    function: &FitnessFunction,
    agent_id: i32,
) -> Result<f64> {
    let metric_type = MetricType::parse(&function.metric_type).ok_or_else(|| {
        AmosError::Validation(format!("Unknown metric type: {}", function.metric_type))
    })?;

    let window_start = Utc::now() - Duration::days(function.window_days as i64);

    match metric_type {
        MetricType::TaskCompletionRate => {
            compute_task_completion_rate(db, agent_id, function.swarm_id, window_start).await
        }
        MetricType::QualityScore => {
            compute_quality_score(db, agent_id, function.swarm_id, window_start).await
        }
        MetricType::Engagement => {
            compute_collection_aggregate(db, function, agent_id, window_start, "sum").await
        }
        MetricType::Conversion => compute_conversion(db, function, agent_id, window_start).await,
        MetricType::Revenue => {
            compute_collection_aggregate(db, function, agent_id, window_start, "sum").await
        }
        MetricType::Profit => compute_profit(db, function, agent_id, window_start).await,
        MetricType::Custom => compute_custom(db, function).await,
        // Trading metrics are expected to come from external sources; if
        // someone registers them as internal, return 0.
        MetricType::SharpeRatio
        | MetricType::SortinoRatio
        | MetricType::MaxDrawdown
        | MetricType::TotalReturn
        | MetricType::WinRate => {
            tracing::warn!(
                metric_type = function.metric_type.as_str(),
                "Trading metric registered as internal — returning 0.0"
            );
            Ok(0.0)
        }
    }
}

// ── Task-level metrics ──────────────────────────────────────────────────

async fn compute_task_completion_rate(
    db: &PgPool,
    agent_id: i32,
    swarm_id: uuid::Uuid,
    window_start: chrono::DateTime<Utc>,
) -> Result<f64> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE quality_score IS NOT NULL) AS completed,
            COUNT(*) AS total
        FROM agent_task_attribution
        WHERE agent_id = $1
          AND swarm_id = $2
          AND created_at >= $3
        "#,
    )
    .bind(agent_id)
    .bind(swarm_id)
    .bind(window_start)
    .fetch_one(db)
    .await?;

    let completed: i64 = row.get("completed");
    let total: i64 = row.get("total");

    if total == 0 {
        return Ok(0.0);
    }

    Ok(completed as f64 / total as f64)
}

async fn compute_quality_score(
    db: &PgPool,
    agent_id: i32,
    swarm_id: uuid::Uuid,
    window_start: chrono::DateTime<Utc>,
) -> Result<f64> {
    let row = sqlx::query(
        r#"
        SELECT COALESCE(AVG(quality_score), 0.0) AS avg_quality
        FROM agent_task_attribution
        WHERE agent_id = $1
          AND swarm_id = $2
          AND created_at >= $3
          AND quality_score IS NOT NULL
        "#,
    )
    .bind(agent_id)
    .bind(swarm_id)
    .bind(window_start)
    .fetch_one(db)
    .await?;

    let avg: f64 = row.get("avg_quality");
    Ok(avg)
}

// ── Collection-based metrics ────────────────────────────────────────────

/// Generic aggregation over the `records` JSONB table. The metric_config
/// is expected to contain:
///
/// ```json
/// {
///   "collection": "blog_posts",
///   "value_field": "page_views",
///   "aggregation": "sum",          // sum | avg | count
///   "attribution_field": "created_by_agent_id"
/// }
/// ```
async fn compute_collection_aggregate(
    db: &PgPool,
    function: &FitnessFunction,
    agent_id: i32,
    window_start: chrono::DateTime<Utc>,
    default_agg: &str,
) -> Result<f64> {
    let config = &function.metric_config;

    let collection = config["collection"]
        .as_str()
        .ok_or_else(|| AmosError::Validation("metric_config.collection is required".into()))?;
    let value_field = config["value_field"]
        .as_str()
        .ok_or_else(|| AmosError::Validation("metric_config.value_field is required".into()))?;
    let attribution_field = config["attribution_field"]
        .as_str()
        .unwrap_or("created_by_agent_id");
    let aggregation = config["aggregation"].as_str().unwrap_or(default_agg);

    let agg_sql = match aggregation {
        "sum" => format!("COALESCE(SUM((r.data->>'{value_field}')::float), 0)"),
        "avg" => format!("COALESCE(AVG((r.data->>'{value_field}')::float), 0)"),
        "count" => "COUNT(*)::float".to_string(),
        other => {
            return Err(AmosError::Validation(format!(
                "Unsupported aggregation: {other}"
            )));
        }
    };

    // Build the query using the validated aggregation function.
    let sql = format!(
        r#"
        SELECT {agg_sql} AS result
        FROM records r
        JOIN collections c ON r.collection_id = c.id
        WHERE c.name = $1
          AND r.data->>$2 = $3::text
          AND r.created_at >= $4
        "#,
    );

    let row = sqlx::query(&sql)
        .bind(collection)
        .bind(attribution_field)
        .bind(agent_id.to_string())
        .bind(window_start)
        .fetch_one(db)
        .await?;

    let result: f64 = row.get("result");
    Ok(result)
}

/// Conversion metric: ratio of records matching a target status to all records
/// attributed to the agent in the collection.
///
/// metric_config:
/// ```json
/// {
///   "collection": "leads",
///   "status_field": "status",
///   "target_status": "converted",
///   "attribution_field": "created_by_agent_id"
/// }
/// ```
async fn compute_conversion(
    db: &PgPool,
    function: &FitnessFunction,
    agent_id: i32,
    window_start: chrono::DateTime<Utc>,
) -> Result<f64> {
    let config = &function.metric_config;

    let collection = config["collection"]
        .as_str()
        .ok_or_else(|| AmosError::Validation("metric_config.collection is required".into()))?;
    let status_field = config["status_field"].as_str().unwrap_or("status");
    let target_status = config["target_status"]
        .as_str()
        .ok_or_else(|| AmosError::Validation("metric_config.target_status is required".into()))?;
    let attribution_field = config["attribution_field"]
        .as_str()
        .unwrap_or("created_by_agent_id");

    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE r.data->>$5 = $6) AS converted,
            COUNT(*) AS total
        FROM records r
        JOIN collections c ON r.collection_id = c.id
        WHERE c.name = $1
          AND r.data->>$2 = $3::text
          AND r.created_at >= $4
        "#,
    )
    .bind(collection)
    .bind(attribution_field)
    .bind(agent_id.to_string())
    .bind(window_start)
    .bind(status_field)
    .bind(target_status)
    .fetch_one(db)
    .await?;

    let converted: i64 = row.get("converted");
    let total: i64 = row.get("total");

    if total == 0 {
        return Ok(0.0);
    }

    Ok(converted as f64 / total as f64)
}

/// Profit = revenue (collection-based SUM) minus cost (task attribution).
async fn compute_profit(
    db: &PgPool,
    function: &FitnessFunction,
    agent_id: i32,
    window_start: chrono::DateTime<Utc>,
) -> Result<f64> {
    let revenue = compute_collection_aggregate(db, function, agent_id, window_start, "sum").await?;

    let cost_row = sqlx::query(
        r#"
        SELECT COALESCE(SUM(cost_usd), 0.0) AS total_cost
        FROM agent_task_attribution
        WHERE agent_id = $1
          AND swarm_id = $2
          AND created_at >= $3
        "#,
    )
    .bind(agent_id)
    .bind(function.swarm_id)
    .bind(window_start)
    .fetch_one(db)
    .await?;

    let total_cost: f64 = cost_row.get("total_cost");
    Ok(revenue - total_cost)
}

/// Execute a raw SQL query stored in `metric_query`. The query **must**
/// return exactly one row with a single numeric column.
async fn compute_custom(db: &PgPool, function: &FitnessFunction) -> Result<f64> {
    let sql = function
        .metric_query
        .as_deref()
        .ok_or_else(|| AmosError::Validation("Custom metric requires metric_query".into()))?;

    let row = sqlx::query(sql).fetch_one(db).await?;

    // Try to extract the first column as f64.
    let value: f64 = row.try_get(0).map_err(|e| {
        AmosError::Internal(format!(
            "Custom metric query did not return a numeric scalar: {e}"
        ))
    })?;

    Ok(value)
}
