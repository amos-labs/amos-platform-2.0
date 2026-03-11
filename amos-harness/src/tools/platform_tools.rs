//! Platform CRUD tools for business data

use super::{Tool, ToolCategory, ToolResult};
use amos_core::Result;
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::{Column, PgPool, Row};

/// Query records from any module
pub struct PlatformQueryTool {
    db_pool: PgPool,
}

impl PlatformQueryTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for PlatformQueryTool {
    fn name(&self) -> &str {
        "platform_query"
    }

    fn description(&self) -> &str {
        "Query records from any platform module with filters and sorting"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "module": {
                    "type": "string",
                    "description": "Module name (table name)"
                },
                "filters": {
                    "type": "object",
                    "description": "Filter conditions (e.g., {\"status\": \"active\"})"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of records to return",
                    "default": 50
                },
                "offset": {
                    "type": "integer",
                    "description": "Number of records to skip",
                    "default": 0
                },
                "order_by": {
                    "type": "string",
                    "description": "Field to sort by"
                },
                "order_direction": {
                    "type": "string",
                    "enum": ["asc", "desc"],
                    "description": "Sort direction"
                }
            },
            "required": ["module"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let module = params["module"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("module is required".to_string()))?;

        let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);
        let offset = params.get("offset").and_then(|v| v.as_i64()).unwrap_or(0);

        // Build query (simplified - in production would use the module system)
        let query = format!(
            "SELECT * FROM {} ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            module
        );

        let rows = sqlx::query(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db_pool)
            .await
            .map_err(|e| {
                amos_core::AmosError::Internal(format!(
                    "Database: {}",
                    format!("Query failed: {}", e)
                ))
            })?;

        // Convert rows to JSON
        let mut records = Vec::new();
        for row in rows {
            let mut record = serde_json::Map::new();
            for (i, column) in row.columns().iter().enumerate() {
                let name = column.name();
                if let Ok(value) = row.try_get::<String, _>(i) {
                    record.insert(name.to_string(), JsonValue::String(value));
                } else if let Ok(value) = row.try_get::<i64, _>(i) {
                    record.insert(name.to_string(), JsonValue::Number(value.into()));
                } else if let Ok(value) = row.try_get::<bool, _>(i) {
                    record.insert(name.to_string(), JsonValue::Bool(value));
                }
            }
            records.push(JsonValue::Object(record));
        }

        Ok(ToolResult::success(json!({
            "records": records,
            "count": records.len()
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Platform
    }
}

/// Create a record in any module
pub struct PlatformCreateTool {
    db_pool: PgPool,
}

impl PlatformCreateTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for PlatformCreateTool {
    fn name(&self) -> &str {
        "platform_create"
    }

    fn description(&self) -> &str {
        "Create a new record in any platform module"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "module": {
                    "type": "string",
                    "description": "Module name (table name)"
                },
                "data": {
                    "type": "object",
                    "description": "Record data to create"
                }
            },
            "required": ["module", "data"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let module = params["module"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("module is required".to_string()))?;

        let _data = params
            .get("data")
            .ok_or_else(|| amos_core::AmosError::Validation("data is required".to_string()))?;

        // In production, this would use the module system to validate and create records
        // For now, return a stub response
        Ok(ToolResult::success(json!({
            "id": 1,
            "module": module,
            "created": true,
            "message": "Record created successfully (stub)"
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Platform
    }
}

/// Update a record in any module
pub struct PlatformUpdateTool {
    db_pool: PgPool,
}

impl PlatformUpdateTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for PlatformUpdateTool {
    fn name(&self) -> &str {
        "platform_update"
    }

    fn description(&self) -> &str {
        "Update an existing record in any platform module"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "module": {
                    "type": "string",
                    "description": "Module name (table name)"
                },
                "id": {
                    "type": "integer",
                    "description": "Record ID to update"
                },
                "data": {
                    "type": "object",
                    "description": "Fields to update"
                }
            },
            "required": ["module", "id", "data"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let module = params["module"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("module is required".to_string()))?;

        let id = params["id"]
            .as_i64()
            .ok_or_else(|| amos_core::AmosError::Validation("id is required".to_string()))?;

        Ok(ToolResult::success(json!({
            "id": id,
            "module": module,
            "updated": true,
            "message": "Record updated successfully (stub)"
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Platform
    }
}

/// Execute a module action
pub struct PlatformExecuteTool {
    db_pool: PgPool,
}

impl PlatformExecuteTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for PlatformExecuteTool {
    fn name(&self) -> &str {
        "platform_execute"
    }

    fn description(&self) -> &str {
        "Execute a custom action on a module or record"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "module": {
                    "type": "string",
                    "description": "Module name"
                },
                "action": {
                    "type": "string",
                    "description": "Action name to execute"
                },
                "record_id": {
                    "type": "integer",
                    "description": "Record ID (if action is record-specific)"
                },
                "params": {
                    "type": "object",
                    "description": "Action parameters"
                }
            },
            "required": ["module", "action"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let module = params["module"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("module is required".to_string()))?;

        let action = params["action"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("action is required".to_string()))?;

        Ok(ToolResult::success(json!({
            "module": module,
            "action": action,
            "executed": true,
            "message": "Action executed successfully (stub)"
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Platform
    }
}
