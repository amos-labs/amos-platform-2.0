//! Integration management tools for AI agents
//!
//! Provides tools for managing third-party integrations including:
//! - Listing integrations and connections
//! - Creating and testing connections
//! - Executing integration operations
//! - Managing sync configurations and ETL pipelines

use crate::integrations::etl::EtlPipeline;
use crate::integrations::executor::ApiExecutor;
use crate::integrations::types::{IntegrationRow, OperationRow};
use crate::tools::{Tool, ToolCategory, ToolResult};
use amos_core::Result;
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
// List Integrations Tool
// ═══════════════════════════════════════════════════════════════════════════

/// Lists all available integrations in the system
pub struct ListIntegrationsTool {
    db_pool: PgPool,
}

impl ListIntegrationsTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ListIntegrationsTool {
    fn name(&self) -> &str {
        "list_integrations"
    }

    fn description(&self) -> &str {
        "List all available third-party integrations (CRM, email, payment, etc.)"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _params: JsonValue) -> Result<ToolResult> {
        let integrations: Vec<IntegrationRow> = sqlx::query_as(
            r#"
            SELECT id, name, connector_type, endpoint_url, status,
                   credentials, last_sync_at, error_message, sync_config,
                   available_actions, metadata, created_at, updated_at
            FROM integrations
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.db_pool)
        .await?;

        let result: Vec<JsonValue> = integrations
            .iter()
            .map(|i| {
                json!({
                    "id": i.id,
                    "name": i.name,
                    "connector_type": i.connector_type,
                    "endpoint_url": i.endpoint_url,
                    "status": i.status,
                    "last_sync_at": i.last_sync_at,
                    "error_message": i.error_message,
                    "created_at": i.created_at,
                    "updated_at": i.updated_at,
                })
            })
            .collect();

        let count = result.len();
        Ok(ToolResult::success(json!({
            "integrations": result,
            "count": count
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// List Connections Tool
// ═══════════════════════════════════════════════════════════════════════════

/// Lists active integration connections, optionally filtered by integration
pub struct ListConnectionsTool {
    db_pool: PgPool,
}

impl ListConnectionsTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

/// Helper row type for connection list queries (includes integration name join)
#[derive(Debug, Clone, sqlx::FromRow)]
struct ConnectionWithIntegration {
    id: Uuid,
    integration_id: Uuid,
    credential_id: Option<Uuid>,
    name: Option<String>,
    status: String,
    health: String,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    last_sync_at: Option<chrono::DateTime<chrono::Utc>>,
    error_message: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    integration_name: String,
}

#[async_trait]
impl Tool for ListConnectionsTool {
    fn name(&self) -> &str {
        "list_connections"
    }

    fn description(&self) -> &str {
        "List active integration connections, optionally filtered by integration_id"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "integration_id": {
                    "type": "string",
                    "description": "Optional: Filter connections by integration UUID"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let integration_id = params
            .get("integration_id")
            .and_then(|v| v.as_str())
            .map(|s| Uuid::from_str(s))
            .transpose()
            .map_err(|_| {
                amos_core::AmosError::Validation("Invalid integration_id UUID format".to_string())
            })?;

        let connections: Vec<ConnectionWithIntegration> = if let Some(int_id) = integration_id {
            sqlx::query_as(
                r#"
                SELECT c.id, c.integration_id, c.credential_id, c.name, c.status,
                       c.health, c.last_used_at, c.last_sync_at, c.error_message,
                       c.created_at, c.updated_at,
                       i.name as integration_name
                FROM integration_connections c
                JOIN integrations i ON c.integration_id = i.id
                WHERE c.integration_id = $1
                ORDER BY c.created_at DESC
                "#,
            )
            .bind(int_id)
            .fetch_all(&self.db_pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT c.id, c.integration_id, c.credential_id, c.name, c.status,
                       c.health, c.last_used_at, c.last_sync_at, c.error_message,
                       c.created_at, c.updated_at,
                       i.name as integration_name
                FROM integration_connections c
                JOIN integrations i ON c.integration_id = i.id
                ORDER BY c.created_at DESC
                "#,
            )
            .fetch_all(&self.db_pool)
            .await?
        };

        let result: Vec<JsonValue> = connections
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "integration_id": c.integration_id,
                    "integration_name": c.integration_name,
                    "credential_id": c.credential_id,
                    "name": c.name,
                    "status": c.status,
                    "health": c.health,
                    "last_used_at": c.last_used_at,
                    "last_sync_at": c.last_sync_at,
                    "error_message": c.error_message,
                    "created_at": c.created_at,
                    "updated_at": c.updated_at,
                })
            })
            .collect();

        let count = result.len();
        Ok(ToolResult::success(json!({
            "connections": result,
            "count": count
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Create Connection Tool
// ═══════════════════════════════════════════════════════════════════════════

/// Creates a new integration connection with credentials
pub struct CreateConnectionTool {
    db_pool: PgPool,
}

impl CreateConnectionTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

/// Minimal row returned from credential INSERT
#[derive(Debug, sqlx::FromRow)]
struct CredentialIdRow {
    id: Uuid,
}

/// Minimal row returned from connection INSERT
#[derive(Debug, sqlx::FromRow)]
struct NewConnectionRow {
    id: Uuid,
    integration_id: Uuid,
    credential_id: Option<Uuid>,
    name: Option<String>,
    status: String,
    health: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
impl Tool for CreateConnectionTool {
    fn name(&self) -> &str {
        "create_connection"
    }

    fn description(&self) -> &str {
        "Create a new integration connection with authentication credentials"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "integration_id": {
                    "type": "string",
                    "description": "UUID of the integration to connect to"
                },
                "auth_type": {
                    "type": "string",
                    "description": "Authentication type (api_key, bearer_token, basic_auth, oauth2, sso_key, no_auth, custom)",
                    "enum": ["api_key", "bearer_token", "basic_auth", "oauth2", "sso_key", "no_auth", "custom"]
                },
                "credentials": {
                    "type": "object",
                    "description": "Credentials data as JSON object (e.g., {\"api_key\": \"sk_123\"} or {\"username\": \"user\", \"password\": \"pass\"}). Not required if vault_credential_id is provided."
                },
                "vault_credential_id": {
                    "type": "string",
                    "description": "UUID of a credential stored in the encrypted vault (from collect_credential tool). Use this instead of passing plaintext credentials."
                },
                "name": {
                    "type": "string",
                    "description": "Optional: Friendly name for this connection"
                },
                "config": {
                    "type": "object",
                    "description": "Optional: Connection-specific configuration settings"
                }
            },
            "required": ["integration_id", "auth_type"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let integration_id = params["integration_id"]
            .as_str()
            .ok_or_else(|| {
                amos_core::AmosError::Validation("integration_id is required".to_string())
            })
            .and_then(|s| {
                Uuid::from_str(s).map_err(|_| {
                    amos_core::AmosError::Validation("Invalid integration_id UUID".to_string())
                })
            })?;

        let auth_type = params["auth_type"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("auth_type is required".to_string()))?
            .to_string();

        // Support vault_credential_id as an alternative to plaintext credentials.
        // When vault_credential_id is provided, store it as a reference in credentials_data
        // so the ApiExecutor can resolve it at runtime from the encrypted vault.
        let vault_credential_id = params
            .get("vault_credential_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        let credentials = if let Some(ref vault_id) = vault_credential_id {
            // Validate the UUID format
            Uuid::from_str(vault_id).map_err(|_| {
                amos_core::AmosError::Validation("Invalid vault_credential_id UUID".to_string())
            })?;
            // Store vault reference — ApiExecutor will decrypt at runtime
            json!({ "vault_credential_id": vault_id })
        } else {
            params
                .get("credentials")
                .ok_or_else(|| amos_core::AmosError::Validation(
                    "Either credentials or vault_credential_id is required".to_string(),
                ))?
                .clone()
        };

        let name = params.get("name").and_then(|v| v.as_str()).map(String::from);

        let config = params
            .get("config")
            .cloned()
            .unwrap_or_else(|| json!({}));

        // First, create the credential
        let credential: CredentialIdRow = sqlx::query_as(
            r#"
            INSERT INTO integration_credentials
                (integration_id, auth_type, credentials_data, status, metadata)
            VALUES ($1, $2, $3, 'active', '{}')
            RETURNING id
            "#,
        )
        .bind(integration_id)
        .bind(&auth_type)
        .bind(&credentials)
        .fetch_one(&self.db_pool)
        .await?;

        // Then create the connection
        let connection: NewConnectionRow = sqlx::query_as(
            r#"
            INSERT INTO integration_connections
                (integration_id, credential_id, name, status, health, config, metadata)
            VALUES ($1, $2, $3, 'disconnected', 'unknown', $4, '{}')
            RETURNING id, integration_id, credential_id, name, status, health,
                      created_at, updated_at
            "#,
        )
        .bind(integration_id)
        .bind(credential.id)
        .bind(&name)
        .bind(&config)
        .fetch_one(&self.db_pool)
        .await?;

        Ok(ToolResult::success(json!({
            "connection_id": connection.id,
            "integration_id": connection.integration_id,
            "credential_id": connection.credential_id,
            "name": connection.name,
            "status": connection.status,
            "health": connection.health,
            "created_at": connection.created_at,
            "updated_at": connection.updated_at,
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Test Connection Tool
// ═══════════════════════════════════════════════════════════════════════════

/// Tests if an integration connection is working
pub struct TestConnectionTool {
    db_pool: PgPool,
    api_executor: Arc<ApiExecutor>,
}

impl TestConnectionTool {
    pub fn new(db_pool: PgPool, api_executor: Arc<ApiExecutor>) -> Self {
        Self {
            db_pool,
            api_executor,
        }
    }
}

#[async_trait]
impl Tool for TestConnectionTool {
    fn name(&self) -> &str {
        "test_connection"
    }

    fn description(&self) -> &str {
        "Test if an integration connection is working by executing a test API call"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "connection_id": {
                    "type": "string",
                    "description": "UUID of the connection to test"
                }
            },
            "required": ["connection_id"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let connection_id = params["connection_id"]
            .as_str()
            .ok_or_else(|| {
                amos_core::AmosError::Validation("connection_id is required".to_string())
            })
            .and_then(|s| {
                Uuid::from_str(s).map_err(|_| {
                    amos_core::AmosError::Validation("Invalid connection_id UUID".to_string())
                })
            })?;

        // Call the API executor's test_connection method
        match self.api_executor.test_connection(connection_id).await {
            Ok(result) => {
                // Update connection status to connected and health to healthy
                sqlx::query(
                    r#"
                    UPDATE integration_connections
                    SET status = 'connected',
                        health = 'healthy',
                        last_used_at = NOW(),
                        error_message = NULL,
                        consecutive_errors = 0
                    WHERE id = $1
                    "#,
                )
                .bind(connection_id)
                .execute(&self.db_pool)
                .await?;

                Ok(ToolResult::success(json!({
                    "success": true,
                    "status_code": result.status_code,
                    "duration_ms": result.duration_ms,
                    "message": "Connection test successful"
                })))
            }
            Err(e) => {
                // Update connection status to error
                let error_msg = format!("{}", e);
                sqlx::query(
                    r#"
                    UPDATE integration_connections
                    SET status = 'error',
                        health = 'failing',
                        error_message = $2,
                        consecutive_errors = consecutive_errors + 1
                    WHERE id = $1
                    "#,
                )
                .bind(connection_id)
                .bind(&error_msg)
                .execute(&self.db_pool)
                .await?;

                Ok(ToolResult::error(format!("Connection test failed: {}", e)))
            }
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Execute Integration Action Tool
// ═══════════════════════════════════════════════════════════════════════════

/// Executes an API operation on an integration
pub struct ExecuteIntegrationActionTool {
    api_executor: Arc<ApiExecutor>,
}

impl ExecuteIntegrationActionTool {
    pub fn new(api_executor: Arc<ApiExecutor>) -> Self {
        Self { api_executor }
    }
}

#[async_trait]
impl Tool for ExecuteIntegrationActionTool {
    fn name(&self) -> &str {
        "execute_integration_action"
    }

    fn description(&self) -> &str {
        "Execute an API operation on an integration (e.g., create_contact, send_email, fetch_invoices)"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "connection_id": {
                    "type": "string",
                    "description": "UUID of the connection to use"
                },
                "operation_id": {
                    "type": "string",
                    "description": "ID of the operation to execute (e.g., 'create_contact', 'send_email')"
                },
                "params": {
                    "type": "object",
                    "description": "Optional: Parameters for the operation as JSON object"
                }
            },
            "required": ["connection_id", "operation_id"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let connection_id = params["connection_id"]
            .as_str()
            .ok_or_else(|| {
                amos_core::AmosError::Validation("connection_id is required".to_string())
            })
            .and_then(|s| {
                Uuid::from_str(s).map_err(|_| {
                    amos_core::AmosError::Validation("Invalid connection_id UUID".to_string())
                })
            })?;

        let operation_id = params["operation_id"]
            .as_str()
            .ok_or_else(|| {
                amos_core::AmosError::Validation("operation_id is required".to_string())
            })?;

        let operation_params = params
            .get("params")
            .cloned()
            .unwrap_or_else(|| json!({}));

        // Execute the operation
        match self
            .api_executor
            .execute(connection_id, operation_id, operation_params)
            .await
        {
            Ok(result) => Ok(ToolResult::success(json!({
                "success": true,
                "status_code": result.status_code,
                "body": result.body,
                "duration_ms": result.duration_ms,
                "operation_id": result.operation_id,
            }))),
            Err(e) => Ok(ToolResult::error(format!("Operation failed: {}", e))),
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// List Operations Tool
// ═══════════════════════════════════════════════════════════════════════════

/// Lists available operations for an integration
pub struct ListOperationsTool {
    db_pool: PgPool,
}

impl ListOperationsTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ListOperationsTool {
    fn name(&self) -> &str {
        "list_integration_operations"
    }

    fn description(&self) -> &str {
        "List all available operations for a specific integration"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "integration_id": {
                    "type": "string",
                    "description": "UUID of the integration"
                }
            },
            "required": ["integration_id"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let integration_id = params["integration_id"]
            .as_str()
            .ok_or_else(|| {
                amos_core::AmosError::Validation("integration_id is required".to_string())
            })
            .and_then(|s| {
                Uuid::from_str(s).map_err(|_| {
                    amos_core::AmosError::Validation("Invalid integration_id UUID".to_string())
                })
            })?;

        let operations: Vec<OperationRow> = sqlx::query_as(
            r#"
            SELECT id, integration_id, operation_id, name, description, http_method,
                   path_template, request_schema, response_schema, pagination_strategy,
                   requires_confirmation, is_destructive, status, examples, metadata,
                   created_at, updated_at
            FROM integration_operations
            WHERE integration_id = $1
            ORDER BY name ASC
            "#,
        )
        .bind(integration_id)
        .fetch_all(&self.db_pool)
        .await?;

        let result: Vec<JsonValue> = operations
            .iter()
            .map(|op| {
                json!({
                    "id": op.id,
                    "operation_id": op.operation_id,
                    "name": op.name,
                    "description": op.description,
                    "http_method": op.http_method,
                    "path_template": op.path_template,
                    "request_schema": op.request_schema,
                    "response_schema": op.response_schema,
                    "pagination_strategy": op.pagination_strategy,
                    "requires_confirmation": op.requires_confirmation,
                    "is_destructive": op.is_destructive,
                    "status": op.status,
                })
            })
            .collect();

        let count = result.len();
        Ok(ToolResult::success(json!({
            "operations": result,
            "count": count
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Create Sync Config Tool
// ═══════════════════════════════════════════════════════════════════════════

/// Creates a new ETL sync configuration
pub struct CreateSyncConfigTool {
    db_pool: PgPool,
}

impl CreateSyncConfigTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

/// Minimal row returned from sync config INSERT
#[derive(Debug, sqlx::FromRow)]
struct NewSyncConfigRow {
    id: Uuid,
    connection_id: Uuid,
    resource_type: String,
    target_collection: String,
    sync_mode: String,
    sync_direction: String,
    schedule_type: String,
    enabled: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
impl Tool for CreateSyncConfigTool {
    fn name(&self) -> &str {
        "create_sync_config"
    }

    fn description(&self) -> &str {
        "Create a sync configuration to automatically pull data from an integration into a collection"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "connection_id": {
                    "type": "string",
                    "description": "UUID of the connection to sync from"
                },
                "resource_type": {
                    "type": "string",
                    "description": "Type of resource to sync (e.g., 'contacts', 'invoices', 'products')"
                },
                "target_collection": {
                    "type": "string",
                    "description": "Name of the collection to store synced data"
                },
                "fetch_operation_id": {
                    "type": "string",
                    "description": "Operation ID to use for fetching data (e.g., 'list_contacts')"
                },
                "field_mappings": {
                    "type": "object",
                    "description": "JSON object mapping external fields to collection fields (e.g., {\"email\": \"contact_email\"})"
                },
                "sync_mode": {
                    "type": "string",
                    "description": "Sync mode: 'full' or 'incremental' (default: 'full')",
                    "enum": ["full", "incremental"]
                },
                "sync_direction": {
                    "type": "string",
                    "description": "Sync direction: 'inbound', 'outbound', or 'bidirectional' (default: 'inbound')",
                    "enum": ["inbound", "outbound", "bidirectional"]
                },
                "schedule_type": {
                    "type": "string",
                    "description": "Schedule type: 'manual', 'scheduled', or 'realtime' (default: 'manual')",
                    "enum": ["manual", "scheduled", "realtime"]
                },
                "schedule_cron": {
                    "type": "string",
                    "description": "Cron expression for scheduled syncs (e.g., '0 */6 * * *' for every 6 hours)"
                },
                "conflict_resolution": {
                    "type": "string",
                    "description": "Conflict resolution strategy: 'external_wins', 'internal_wins', 'manual', or 'newest' (default: 'external_wins')",
                    "enum": ["external_wins", "internal_wins", "manual", "newest"]
                }
            },
            "required": ["connection_id", "resource_type", "target_collection", "fetch_operation_id", "field_mappings"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let connection_id = params["connection_id"]
            .as_str()
            .ok_or_else(|| {
                amos_core::AmosError::Validation("connection_id is required".to_string())
            })
            .and_then(|s| {
                Uuid::from_str(s).map_err(|_| {
                    amos_core::AmosError::Validation("Invalid connection_id UUID".to_string())
                })
            })?;

        let resource_type = params["resource_type"]
            .as_str()
            .ok_or_else(|| {
                amos_core::AmosError::Validation("resource_type is required".to_string())
            })?
            .to_string();

        let target_collection = params["target_collection"]
            .as_str()
            .ok_or_else(|| {
                amos_core::AmosError::Validation("target_collection is required".to_string())
            })?
            .to_string();

        let fetch_operation_id = params["fetch_operation_id"]
            .as_str()
            .ok_or_else(|| {
                amos_core::AmosError::Validation("fetch_operation_id is required".to_string())
            })?
            .to_string();

        let field_mappings = params
            .get("field_mappings")
            .ok_or_else(|| {
                amos_core::AmosError::Validation("field_mappings is required".to_string())
            })?
            .clone();

        let sync_mode = params
            .get("sync_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("full")
            .to_string();

        let sync_direction = params
            .get("sync_direction")
            .and_then(|v| v.as_str())
            .unwrap_or("inbound")
            .to_string();

        let schedule_type = params
            .get("schedule_type")
            .and_then(|v| v.as_str())
            .unwrap_or("manual")
            .to_string();

        let schedule_cron: Option<String> = params
            .get("schedule_cron")
            .and_then(|v| v.as_str())
            .map(String::from);

        let conflict_resolution = params
            .get("conflict_resolution")
            .and_then(|v| v.as_str())
            .unwrap_or("external_wins")
            .to_string();

        let empty_json = json!({});

        // Create the sync config (no `name` column in the table)
        let sync_config: NewSyncConfigRow = sqlx::query_as(
            r#"
            INSERT INTO integration_sync_configs
                (connection_id, resource_type, target_collection, sync_mode, sync_direction,
                 field_mappings, conflict_resolution, schedule_type, schedule_cron,
                 fetch_operation_id, fetch_params, enabled, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, true, $12)
            RETURNING id, connection_id, resource_type, target_collection, sync_mode,
                      sync_direction, schedule_type, enabled, created_at, updated_at
            "#,
        )
        .bind(connection_id)
        .bind(&resource_type)
        .bind(&target_collection)
        .bind(&sync_mode)
        .bind(&sync_direction)
        .bind(&field_mappings)
        .bind(&conflict_resolution)
        .bind(&schedule_type)
        .bind(&schedule_cron)
        .bind(&fetch_operation_id)
        .bind(&empty_json) // fetch_params
        .bind(&empty_json) // metadata
        .fetch_one(&self.db_pool)
        .await?;

        Ok(ToolResult::success(json!({
            "sync_config_id": sync_config.id,
            "connection_id": sync_config.connection_id,
            "resource_type": sync_config.resource_type,
            "target_collection": sync_config.target_collection,
            "sync_mode": sync_config.sync_mode,
            "sync_direction": sync_config.sync_direction,
            "schedule_type": sync_config.schedule_type,
            "enabled": sync_config.enabled,
            "created_at": sync_config.created_at,
            "updated_at": sync_config.updated_at,
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Trigger Sync Tool
// ═══════════════════════════════════════════════════════════════════════════

/// Triggers an ETL sync job
pub struct TriggerSyncTool {
    etl_pipeline: Arc<EtlPipeline>,
}

impl TriggerSyncTool {
    pub fn new(etl_pipeline: Arc<EtlPipeline>) -> Self {
        Self { etl_pipeline }
    }
}

#[async_trait]
impl Tool for TriggerSyncTool {
    fn name(&self) -> &str {
        "trigger_sync"
    }

    fn description(&self) -> &str {
        "Manually trigger an ETL sync job to pull data from an integration into a collection"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "sync_config_id": {
                    "type": "string",
                    "description": "UUID of the sync configuration to run"
                }
            },
            "required": ["sync_config_id"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let sync_config_id = params["sync_config_id"]
            .as_str()
            .ok_or_else(|| {
                amos_core::AmosError::Validation("sync_config_id is required".to_string())
            })
            .and_then(|s| {
                Uuid::from_str(s).map_err(|_| {
                    amos_core::AmosError::Validation("Invalid sync_config_id UUID".to_string())
                })
            })?;

        // Run the ETL pipeline
        match self.etl_pipeline.run(sync_config_id).await {
            Ok(result) => Ok(ToolResult::success(json!({
                "success": result.status == "success" || result.status == "partial",
                "status": result.status,
                "extracted": result.extracted,
                "transformed": result.transformed,
                "loaded": result.loaded,
                "duration_ms": result.duration_ms,
                "errors": result.errors,
            }))),
            Err(e) => Ok(ToolResult::error(format!("Sync failed: {}", e))),
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }
}
