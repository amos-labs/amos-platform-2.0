//! Integration management routes
//!
//! Full CRUD for integrations, connections, credentials, operations,
//! sync configs, and action execution.

use crate::{integrations::types::*, state::AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

pub fn routes(_state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Integrations
        .route("/", get(list_integrations))
        .route("/{id}", get(get_integration))
        .route("/{id}/operations", get(list_operations))
        // Connections
        .route(
            "/connections",
            get(list_connections).post(create_connection),
        )
        .route(
            "/connections/{id}",
            get(get_connection)
                .put(update_connection)
                .delete(delete_connection),
        )
        .route("/connections/{id}/test", post(test_connection))
        .route("/connections/{id}/execute", post(execute_action))
        .route("/connections/{id}/logs", get(list_connection_logs))
        // Sync configs
        .route(
            "/sync-configs",
            get(list_sync_configs).post(create_sync_config),
        )
        .route(
            "/sync-configs/{id}",
            get(get_sync_config).delete(delete_sync_config),
        )
        .route("/sync-configs/{id}/trigger", post(trigger_sync))
}

// ═══════════════════════════════════════════════════════════════════════════
// INTEGRATIONS
// ═══════════════════════════════════════════════════════════════════════════

async fn list_integrations(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<IntegrationRow>>, StatusCode> {
    let integrations =
        sqlx::query_as::<_, IntegrationRow>("SELECT * FROM integrations ORDER BY name")
            .fetch_all(&state.db_pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to list integrations: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    Ok(Json(integrations))
}

async fn get_integration(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<JsonValue>, StatusCode> {
    let integration =
        sqlx::query_as::<_, IntegrationRow>("SELECT * FROM integrations WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

    let operations = sqlx::query_as::<_, OperationRow>(
        "SELECT * FROM integration_operations WHERE integration_id = $1 ORDER BY operation_id",
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "integration": integration,
        "operations": operations,
    })))
}

async fn list_operations(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<OperationRow>>, StatusCode> {
    let operations = sqlx::query_as::<_, OperationRow>(
        "SELECT * FROM integration_operations WHERE integration_id = $1 ORDER BY operation_id",
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(operations))
}

// ═══════════════════════════════════════════════════════════════════════════
// CONNECTIONS
// ═══════════════════════════════════════════════════════════════════════════

async fn list_connections(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ConnectionRow>>, StatusCode> {
    let connections = sqlx::query_as::<_, ConnectionRow>(
        "SELECT * FROM integration_connections ORDER BY created_at DESC",
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(connections))
}

async fn create_connection(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateConnectionRequest>,
) -> Result<(StatusCode, Json<ConnectionRow>), StatusCode> {
    // First, create the credential
    let credential_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO integration_credentials
           (id, integration_id, auth_type, credentials_data, status, metadata)
           VALUES ($1, $2, $3, $4, 'active', '{}')"#,
    )
    .bind(credential_id)
    .bind(body.integration_id)
    .bind(&body.auth_type)
    .bind(&body.credentials)
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create credential: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Then create the connection
    let connection_id = Uuid::new_v4();
    let name = body
        .name
        .unwrap_or_else(|| format!("Connection {}", &connection_id.to_string()[..8]));
    let config = body.config.unwrap_or_else(|| serde_json::json!({}));

    sqlx::query(
        r#"INSERT INTO integration_connections
           (id, integration_id, credential_id, name, config, status, health, metadata)
           VALUES ($1, $2, $3, $4, $5, 'disconnected', 'unknown', '{}')"#,
    )
    .bind(connection_id)
    .bind(body.integration_id)
    .bind(credential_id)
    .bind(&name)
    .bind(&config)
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create connection: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Fetch and return the created connection
    let connection =
        sqlx::query_as::<_, ConnectionRow>("SELECT * FROM integration_connections WHERE id = $1")
            .bind(connection_id)
            .fetch_one(&state.db_pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(connection)))
}

async fn get_connection(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<ConnectionRow>, StatusCode> {
    let connection =
        sqlx::query_as::<_, ConnectionRow>("SELECT * FROM integration_connections WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(connection))
}

async fn update_connection(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateConnectionRequest>,
) -> Result<Json<ConnectionRow>, StatusCode> {
    // Check exists
    sqlx::query_as::<_, ConnectionRow>("SELECT * FROM integration_connections WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Update fields that are present
    if let Some(name) = &body.name {
        sqlx::query(
            "UPDATE integration_connections SET name = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(name)
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    if let Some(config) = &body.config {
        sqlx::query(
            "UPDATE integration_connections SET config = $1, updated_at = NOW() WHERE id = $2",
        )
        .bind(config)
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // Fetch and return updated
    let connection =
        sqlx::query_as::<_, ConnectionRow>("SELECT * FROM integration_connections WHERE id = $1")
            .bind(id)
            .fetch_one(&state.db_pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(connection))
}

async fn delete_connection(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let result = sqlx::query("DELETE FROM integration_connections WHERE id = $1")
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn test_connection(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<JsonValue>, StatusCode> {
    // Check connection exists
    sqlx::query_as::<_, ConnectionRow>("SELECT * FROM integration_connections WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    match state.api_executor.test_connection(id).await {
        Ok(result) => {
            // Update connection status
            let _ = sqlx::query(
                r#"UPDATE integration_connections
                   SET status = 'connected', health = 'healthy',
                       last_used_at = NOW(), error_message = NULL, consecutive_errors = 0
                   WHERE id = $1"#,
            )
            .bind(id)
            .execute(&state.db_pool)
            .await;

            Ok(Json(serde_json::json!({
                "success": true,
                "status_code": result.status_code,
                "duration_ms": result.duration_ms,
                "connection_id": id,
            })))
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            let _ = sqlx::query(
                r#"UPDATE integration_connections
                   SET status = 'error', health = 'failing',
                       error_message = $2, consecutive_errors = consecutive_errors + 1
                   WHERE id = $1"#,
            )
            .bind(id)
            .bind(&error_msg)
            .execute(&state.db_pool)
            .await;

            Ok(Json(serde_json::json!({
                "success": false,
                "error": error_msg,
                "connection_id": id,
            })))
        }
    }
}

async fn execute_action(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<ExecuteActionRequest>,
) -> Result<Json<JsonValue>, StatusCode> {
    // Check connection exists
    sqlx::query_as::<_, ConnectionRow>("SELECT * FROM integration_connections WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    match state
        .api_executor
        .execute(id, &body.operation_id, body.params)
        .await
    {
        Ok(result) => Ok(Json(
            serde_json::to_value(&result).unwrap_or(serde_json::json!({})),
        )),
        Err(e) => Ok(Json(serde_json::json!({
            "error": format!("{}", e),
        }))),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// LOGS
// ═══════════════════════════════════════════════════════════════════════════

async fn list_connection_logs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<LogRow>>, StatusCode> {
    let logs = sqlx::query_as::<_, LogRow>(
        "SELECT * FROM integration_logs WHERE connection_id = $1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(logs))
}

// ═══════════════════════════════════════════════════════════════════════════
// SYNC CONFIGS
// ═══════════════════════════════════════════════════════════════════════════

async fn list_sync_configs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<SyncConfigRow>>, StatusCode> {
    let configs = sqlx::query_as::<_, SyncConfigRow>(
        "SELECT * FROM integration_sync_configs ORDER BY created_at DESC",
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(configs))
}

async fn create_sync_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateSyncConfigRequest>,
) -> Result<(StatusCode, Json<SyncConfigRow>), StatusCode> {
    let config_id = Uuid::new_v4();
    let sync_mode = body.sync_mode.unwrap_or_else(|| "incremental".to_string());
    let sync_direction = body.sync_direction.unwrap_or_else(|| "inbound".to_string());
    let schedule_type = body.schedule_type.unwrap_or_else(|| "manual".to_string());
    let conflict_resolution = body
        .conflict_resolution
        .unwrap_or_else(|| "external_wins".to_string());
    let requires_approval = body.requires_approval.unwrap_or(false);
    let fetch_params = body.fetch_params.unwrap_or_else(|| serde_json::json!({}));

    sqlx::query(
        r#"INSERT INTO integration_sync_configs
           (id, connection_id, resource_type, target_collection, sync_mode, sync_direction,
            field_mappings, conflict_resolution, schedule_type, schedule_cron,
            fetch_operation_id, fetch_params, requires_approval, enabled, metadata)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, true, '{}')"#,
    )
    .bind(config_id)
    .bind(body.connection_id)
    .bind(&body.resource_type)
    .bind(&body.target_collection)
    .bind(&sync_mode)
    .bind(&sync_direction)
    .bind(&body.field_mappings)
    .bind(&conflict_resolution)
    .bind(&schedule_type)
    .bind(&body.schedule_cron)
    .bind(&body.fetch_operation_id)
    .bind(&fetch_params)
    .bind(requires_approval)
    .execute(&state.db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create sync config: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let config =
        sqlx::query_as::<_, SyncConfigRow>("SELECT * FROM integration_sync_configs WHERE id = $1")
            .bind(config_id)
            .fetch_one(&state.db_pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(config)))
}

async fn get_sync_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<SyncConfigRow>, StatusCode> {
    let config =
        sqlx::query_as::<_, SyncConfigRow>("SELECT * FROM integration_sync_configs WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(config))
}

async fn delete_sync_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let result = sqlx::query("DELETE FROM integration_sync_configs WHERE id = $1")
        .bind(id)
        .execute(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn trigger_sync(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_body): Json<TriggerSyncRequest>,
) -> Result<Json<JsonValue>, StatusCode> {
    // Check sync config exists
    sqlx::query_as::<_, SyncConfigRow>("SELECT * FROM integration_sync_configs WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    match state.etl_pipeline.run(id).await {
        Ok(result) => Ok(Json(
            serde_json::to_value(&result).unwrap_or(serde_json::json!({})),
        )),
        Err(e) => {
            tracing::error!("Sync trigger failed: {}", e);
            Ok(Json(serde_json::json!({
                "error": format!("{}", e),
                "status": "failed",
            })))
        }
    }
}
