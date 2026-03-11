//! Agent tools for entity revision tracking and template management.
//!
//! Provides 5 tools:
//! - `list_revisions`: List revision history for an entity
//! - `get_revision`: Get a specific revision by version
//! - `revert_entity`: Revert an entity to a previous version
//! - `list_templates`: List available templates
//! - `check_template_updates`: Check if a subscribed template has updates

use crate::revisions::{RevertRequest, RevisionService, TemplateService};
use crate::tools::{Tool, ToolCategory, ToolResult};
use amos_core::Result;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

// ── ListRevisionsTool ──────────────────────────────────────────────────

pub struct ListRevisionsTool {
    db_pool: PgPool,
}

impl ListRevisionsTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ListRevisionsTool {
    fn name(&self) -> &str {
        "list_revisions"
    }

    fn description(&self) -> &str {
        "List revision history for an entity. Returns versions with timestamps, change types, and who made each change."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "entity_type": {
                    "type": "string",
                    "description": "Entity type: 'integration', 'canvas', 'collection', 'site', 'page'"
                },
                "entity_id": {
                    "type": "string",
                    "description": "UUID of the entity"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results to return (default 20)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Offset for pagination (default 0)"
                }
            },
            "required": ["entity_type", "entity_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Platform
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let entity_type = params["entity_type"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let entity_id = match params["entity_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            Some(id) => id,
            None => {
                return Ok(ToolResult::error(
                    "Invalid or missing entity_id".to_string(),
                ))
            }
        };
        let limit = params["limit"].as_i64().unwrap_or(20);
        let offset = params["offset"].as_i64().unwrap_or(0);

        let service = RevisionService::new(self.db_pool.clone());
        match service
            .list_revisions(&entity_type, entity_id, limit, offset)
            .await
        {
            Ok(response) => Ok(ToolResult::success(serde_json::json!({
                "revisions": response.revisions,
                "total": response.total,
            }))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}

// ── GetRevisionTool ────────────────────────────────────────────────────

pub struct GetRevisionTool {
    db_pool: PgPool,
}

impl GetRevisionTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for GetRevisionTool {
    fn name(&self) -> &str {
        "get_revision"
    }

    fn description(&self) -> &str {
        "Get a specific revision of an entity by version number. Returns the full snapshot, diff, and metadata."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "entity_type": {
                    "type": "string",
                    "description": "Entity type: 'integration', 'canvas', 'collection', 'site', 'page'"
                },
                "entity_id": {
                    "type": "string",
                    "description": "UUID of the entity"
                },
                "version": {
                    "type": "integer",
                    "description": "Version number (1-based). Omit to get the latest."
                }
            },
            "required": ["entity_type", "entity_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Platform
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let entity_type = params["entity_type"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let entity_id = match params["entity_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            Some(id) => id,
            None => {
                return Ok(ToolResult::error(
                    "Invalid or missing entity_id".to_string(),
                ))
            }
        };

        let service = RevisionService::new(self.db_pool.clone());

        let result = if let Some(version) = params["version"].as_i64() {
            service
                .get_revision(&entity_type, entity_id, version as i32)
                .await
                .map(Some)
        } else {
            service.get_latest_revision(&entity_type, entity_id).await
        };

        match result {
            Ok(Some(revision)) => Ok(ToolResult::success(
                serde_json::to_value(&revision).unwrap(),
            )),
            Ok(None) => Ok(ToolResult::error(format!(
                "No revisions found for {} {}",
                entity_type, entity_id
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}

// ── RevertEntityTool ───────────────────────────────────────────────────

pub struct RevertEntityTool {
    db_pool: PgPool,
}

impl RevertEntityTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for RevertEntityTool {
    fn name(&self) -> &str {
        "revert_entity"
    }

    fn description(&self) -> &str {
        "Revert an entity to a previous version. Creates a new revision with the old snapshot (non-destructive)."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "entity_type": {
                    "type": "string",
                    "description": "Entity type: 'integration', 'canvas', 'collection', 'site', 'page'"
                },
                "entity_id": {
                    "type": "string",
                    "description": "UUID of the entity"
                },
                "target_version": {
                    "type": "integer",
                    "description": "Version number to revert to"
                },
                "reason": {
                    "type": "string",
                    "description": "Optional reason for the revert"
                }
            },
            "required": ["entity_type", "entity_id", "target_version"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Platform
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let entity_type = params["entity_type"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let entity_id = match params["entity_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            Some(id) => id,
            None => {
                return Ok(ToolResult::error(
                    "Invalid or missing entity_id".to_string(),
                ))
            }
        };
        let target_version = match params["target_version"].as_i64() {
            Some(v) => v as i32,
            None => return Ok(ToolResult::error("Missing target_version".to_string())),
        };

        let service = RevisionService::new(self.db_pool.clone());
        let request = RevertRequest {
            entity_type,
            entity_id,
            target_version,
            changed_by: "ai_agent".to_string(),
        };

        match service.revert_to_version(request).await {
            Ok(revision) => Ok(ToolResult::success(serde_json::json!({
                "reverted": true,
                "new_version": revision.version,
                "reverted_to": target_version,
                "revision_id": revision.id,
            }))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}

// ── ListTemplatesTool ──────────────────────────────────────────────────

pub struct ListTemplatesTool {
    db_pool: PgPool,
}

impl ListTemplatesTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ListTemplatesTool {
    fn name(&self) -> &str {
        "list_templates"
    }

    fn description(&self) -> &str {
        "List available templates from the template registry. Optionally filter by entity type."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "entity_type": {
                    "type": "string",
                    "description": "Optional filter: 'integration', 'canvas', 'collection'"
                }
            },
            "required": []
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Platform
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let entity_type = params["entity_type"].as_str();

        let service = TemplateService::new(self.db_pool.clone());
        match service.list_templates(entity_type).await {
            Ok(templates) => Ok(ToolResult::success(serde_json::json!({
                "templates": templates.iter().map(|t| serde_json::json!({
                    "slug": t.slug,
                    "name": t.name,
                    "entity_type": t.entity_type,
                    "current_version": t.current_version,
                    "category": t.category,
                    "description": t.description,
                })).collect::<Vec<_>>(),
                "total": templates.len(),
            }))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}

// ── CheckTemplateUpdatesTool ───────────────────────────────────────────

pub struct CheckTemplateUpdatesTool {
    db_pool: PgPool,
}

impl CheckTemplateUpdatesTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for CheckTemplateUpdatesTool {
    fn name(&self) -> &str {
        "check_template_updates"
    }

    fn description(&self) -> &str {
        "Check if an entity's upstream template has available updates. Shows current vs latest version and whether the update is breaking."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "entity_type": {
                    "type": "string",
                    "description": "Entity type: 'integration', 'canvas', 'collection'"
                },
                "entity_id": {
                    "type": "string",
                    "description": "UUID of the entity to check"
                }
            },
            "required": ["entity_type", "entity_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Platform
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let entity_type = params["entity_type"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let entity_id = match params["entity_id"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            Some(id) => id,
            None => {
                return Ok(ToolResult::error(
                    "Invalid or missing entity_id".to_string(),
                ))
            }
        };

        let service = TemplateService::new(self.db_pool.clone());
        match service.check_for_updates(&entity_type, entity_id).await {
            Ok(Some(result)) => Ok(ToolResult::success(serde_json::to_value(&result).unwrap())),
            Ok(None) => Ok(ToolResult::success(serde_json::json!({
                "message": "Entity is not subscribed to any template",
                "has_update": false,
            }))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: create a PgPool via connect_lazy (requires tokio context).
    fn mock_pool() -> PgPool {
        use sqlx::postgres::PgPoolOptions;
        PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://localhost/nonexistent_test_db")
            .unwrap()
    }

    /// Helper: collect all 5 revision tools.
    fn all_tools() -> Vec<Box<dyn Tool>> {
        let pool = mock_pool();
        vec![
            Box::new(ListRevisionsTool::new(pool.clone())),
            Box::new(GetRevisionTool::new(pool.clone())),
            Box::new(RevertEntityTool::new(pool.clone())),
            Box::new(ListTemplatesTool::new(pool.clone())),
            Box::new(CheckTemplateUpdatesTool::new(pool)),
        ]
    }

    // ── Metadata Tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_revisions_tool_metadata() {
        let pool = mock_pool();
        let tool = ListRevisionsTool::new(pool);
        assert_eq!(tool.name(), "list_revisions");
        assert_eq!(tool.category(), ToolCategory::Platform);
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["entity_type"].is_object());
        assert!(schema["properties"]["entity_id"].is_object());
    }

    #[tokio::test]
    async fn test_get_revision_tool_metadata() {
        let pool = mock_pool();
        let tool = GetRevisionTool::new(pool);
        assert_eq!(tool.name(), "get_revision");
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["version"].is_object());
    }

    #[tokio::test]
    async fn test_revert_entity_tool_metadata() {
        let pool = mock_pool();
        let tool = RevertEntityTool::new(pool);
        assert_eq!(tool.name(), "revert_entity");
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["target_version"].is_object());
    }

    #[tokio::test]
    async fn test_list_templates_tool_metadata() {
        let pool = mock_pool();
        let tool = ListTemplatesTool::new(pool);
        assert_eq!(tool.name(), "list_templates");
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["entity_type"].is_object());
    }

    #[tokio::test]
    async fn test_check_template_updates_tool_metadata() {
        let pool = mock_pool();
        let tool = CheckTemplateUpdatesTool::new(pool);
        assert_eq!(tool.name(), "check_template_updates");
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["entity_id"].is_object());
    }

    // ── Unique Names ────────────────────────────────────────────────────

    #[tokio::test]
    async fn all_revision_tools_have_unique_names() {
        let tools = all_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        let mut deduped = names.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(names.len(), deduped.len(), "All tool names must be unique");
    }

    // ── All Tools Are Platform Category ─────────────────────────────────

    #[tokio::test]
    async fn all_revision_tools_are_platform_category() {
        for tool in all_tools() {
            assert_eq!(
                tool.category(),
                ToolCategory::Platform,
                "Tool '{}' should be Platform category",
                tool.name()
            );
        }
    }

    // ── All Tools Have Non-Empty Descriptions ───────────────────────────

    #[tokio::test]
    async fn all_revision_tools_have_descriptions() {
        for tool in all_tools() {
            assert!(
                !tool.description().is_empty(),
                "Tool '{}' should have a description",
                tool.name()
            );
            assert!(
                tool.description().len() > 20,
                "Tool '{}' description should be meaningful (>20 chars)",
                tool.name()
            );
        }
    }

    // ── Schema Required Fields ──────────────────────────────────────────

    #[tokio::test]
    async fn list_revisions_requires_entity_type_and_id() {
        let pool = mock_pool();
        let tool = ListRevisionsTool::new(pool);
        let schema = tool.parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("entity_type")));
        assert!(required.contains(&json!("entity_id")));
    }

    #[tokio::test]
    async fn get_revision_requires_entity_type_and_id() {
        let pool = mock_pool();
        let tool = GetRevisionTool::new(pool);
        let schema = tool.parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("entity_type")));
        assert!(required.contains(&json!("entity_id")));
    }

    #[tokio::test]
    async fn revert_entity_requires_all_three() {
        let pool = mock_pool();
        let tool = RevertEntityTool::new(pool);
        let schema = tool.parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("entity_type")));
        assert!(required.contains(&json!("entity_id")));
        assert!(required.contains(&json!("target_version")));
    }

    #[tokio::test]
    async fn list_templates_has_no_required_fields() {
        let pool = mock_pool();
        let tool = ListTemplatesTool::new(pool);
        let schema = tool.parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(
            required.is_empty(),
            "list_templates should have no required fields"
        );
    }

    #[tokio::test]
    async fn check_template_updates_requires_entity_type_and_id() {
        let pool = mock_pool();
        let tool = CheckTemplateUpdatesTool::new(pool);
        let schema = tool.parameters_schema();
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("entity_type")));
        assert!(required.contains(&json!("entity_id")));
    }

    // ── Schema Types ────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_revisions_schema_has_optional_pagination() {
        let pool = mock_pool();
        let tool = ListRevisionsTool::new(pool);
        let schema = tool.parameters_schema();
        assert_eq!(schema["properties"]["limit"]["type"], "integer");
        assert_eq!(schema["properties"]["offset"]["type"], "integer");
    }

    #[tokio::test]
    async fn get_revision_version_is_integer() {
        let pool = mock_pool();
        let tool = GetRevisionTool::new(pool);
        let schema = tool.parameters_schema();
        assert_eq!(schema["properties"]["version"]["type"], "integer");
    }

    #[tokio::test]
    async fn revert_entity_has_optional_reason() {
        let pool = mock_pool();
        let tool = RevertEntityTool::new(pool);
        let schema = tool.parameters_schema();
        assert!(
            schema["properties"]["reason"].is_object(),
            "Should have optional reason field"
        );
        assert_eq!(schema["properties"]["reason"]["type"], "string");
    }

    // ── Schema Is Object Type ───────────────────────────────────────────

    #[tokio::test]
    async fn all_schemas_are_object_type() {
        for tool in all_tools() {
            let schema = tool.parameters_schema();
            assert_eq!(
                schema["type"],
                "object",
                "Tool '{}' schema should be type: object",
                tool.name()
            );
        }
    }

    // ── Error on Invalid Params (no DB needed) ──────────────────────────

    #[tokio::test]
    async fn list_revisions_returns_error_on_invalid_uuid() {
        let pool = mock_pool();
        let tool = ListRevisionsTool::new(pool);
        let result = tool
            .execute(json!({
                "entity_type": "integration",
                "entity_id": "not-a-uuid"
            }))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("Invalid"));
    }

    #[tokio::test]
    async fn get_revision_returns_error_on_missing_entity_id() {
        let pool = mock_pool();
        let tool = GetRevisionTool::new(pool);
        let result = tool
            .execute(json!({
                "entity_type": "integration"
            }))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("Invalid"));
    }

    #[tokio::test]
    async fn revert_entity_returns_error_on_missing_target_version() {
        let pool = mock_pool();
        let tool = RevertEntityTool::new(pool);
        let result = tool
            .execute(json!({
                "entity_type": "integration",
                "entity_id": Uuid::new_v4().to_string()
            }))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("Missing target_version"));
    }

    #[tokio::test]
    async fn check_template_updates_returns_error_on_invalid_uuid() {
        let pool = mock_pool();
        let tool = CheckTemplateUpdatesTool::new(pool);
        let result = tool
            .execute(json!({
                "entity_type": "integration",
                "entity_id": "bad"
            }))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("Invalid"));
    }
}
