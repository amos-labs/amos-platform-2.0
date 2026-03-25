//! Tool system for agent execution
//!
//! Tools are the primary way the agent interacts with the world.

pub mod app_tools;
pub mod automation_tools;
pub mod canvas_tools;
pub mod credential_tools;
pub mod document_tools;
pub mod image_gen_tools;
pub mod integration_tools;
pub mod knowledge_tools;
pub mod memory_tools;
pub mod openclaw_tools;
// orchestration_tools removed — external agent work delegation is now handled
// by task_tools (create_bounty, get_task_result) and openclaw_tools (agent management).
pub mod platform_tools;
pub mod revision_tools;
pub mod schema_tools;
pub mod site_tools;
pub mod system_tools;
pub mod task_tools;
pub mod web_tools;
pub mod workspace_tools;

use crate::automations::engine::AutomationEngine;
use crate::embeddings::EmbeddingService;
use crate::integrations::{etl::EtlPipeline, executor::ApiExecutor};
use crate::task_queue::TaskQueue;
use amos_core::{AmosError, AppConfig, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};

/// Result of tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool execution was successful
    pub success: bool,

    /// Result data (if successful)
    pub data: Option<JsonValue>,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Additional metadata
    pub metadata: Option<JsonValue>,
}

impl ToolResult {
    /// Create a success result
    pub fn success(data: JsonValue) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            metadata: None,
        }
    }

    /// Create an error result
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            metadata: None,
        }
    }

    /// Create a success result with metadata
    pub fn success_with_metadata(data: JsonValue, metadata: JsonValue) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            metadata: Some(metadata),
        }
    }
}

/// Tool trait that all tools must implement
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name
    fn name(&self) -> &str;

    /// Get the tool description
    fn description(&self) -> &str;

    /// Get the JSON schema for tool parameters
    fn parameters_schema(&self) -> JsonValue;

    /// Execute the tool with the given parameters
    async fn execute(&self, params: JsonValue) -> Result<ToolResult>;

    /// Get tool category for organization
    fn category(&self) -> ToolCategory {
        ToolCategory::Other
    }
}

/// Tool category for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolCategory {
    Platform,
    Canvas,
    Apps,
    Web,
    System,
    Memory,
    Knowledge,
    OpenClaw,
    Integration,
    Schema,
    TaskQueue,
    Document,
    ImageGen,
    Automation,
    Other,
}

impl ToolCategory {
    pub fn as_str(&self) -> &str {
        match self {
            ToolCategory::Platform => "platform",
            ToolCategory::Canvas => "canvas",
            ToolCategory::Apps => "apps",
            ToolCategory::Web => "web",
            ToolCategory::System => "system",
            ToolCategory::Memory => "memory",
            ToolCategory::Knowledge => "knowledge",
            ToolCategory::OpenClaw => "openclaw",
            ToolCategory::Integration => "integration",
            ToolCategory::Schema => "schema",
            ToolCategory::TaskQueue => "task_queue",
            ToolCategory::Document => "document",
            ToolCategory::ImageGen => "image_gen",
            ToolCategory::Automation => "automation",
            ToolCategory::Other => "other",
        }
    }
}

/// Tool registry manages all available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
    db_pool: PgPool,
    config: Arc<AppConfig>,
}

impl ToolRegistry {
    /// Create a new empty tool registry
    pub fn new(db_pool: PgPool, config: Arc<AppConfig>) -> Self {
        Self {
            tools: HashMap::new(),
            db_pool,
            config,
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Execute a tool by name
    pub async fn execute(&self, tool_name: &str, params: JsonValue) -> Result<ToolResult> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| AmosError::NotFound {
                entity: "Tool".to_string(),
                id: tool_name.to_string(),
            })?;

        tool.execute(params).await
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// List all tool names
    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get tools by category
    pub fn get_by_category(&self, category: ToolCategory) -> Vec<Arc<dyn Tool>> {
        self.tools
            .values()
            .filter(|tool| tool.category() == category)
            .cloned()
            .collect()
    }

    /// Get tool schemas for LLM (Bedrock ConverseStream format)
    ///
    /// Bedrock expects camelCase keys: `name`, `description`, `inputSchema`
    pub fn get_tool_schemas(&self) -> Vec<JsonValue> {
        self.tools
            .values()
            .map(|tool| {
                let mut schema = tool.parameters_schema();
                // Ensure inputSchema is never null — Bedrock requires it.
                // If a tool returns no schema, provide a minimal empty-object schema.
                if schema.is_null() {
                    schema = serde_json::json!({
                        "json": {
                            "type": "object",
                            "properties": {}
                        }
                    });
                }
                serde_json::json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "inputSchema": {
                        "json": schema
                    }
                })
            })
            .collect()
    }

    /// Create a registry with all default tools
    pub fn default_registry(
        db_pool: PgPool,
        config: Arc<AppConfig>,
        task_queue: Arc<TaskQueue>,
        bedrock: Option<Arc<crate::bedrock::BedrockClient>>,
        api_executor: Arc<ApiExecutor>,
        etl_pipeline: Arc<EtlPipeline>,
        embedding_service: Option<Arc<EmbeddingService>>,
        automation_engine: Arc<AutomationEngine>,
    ) -> Self {
        let mut registry = Self::new(db_pool.clone(), config.clone());

        // Register platform tools
        registry.register(Arc::new(platform_tools::PlatformQueryTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(platform_tools::PlatformCreateTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(platform_tools::PlatformUpdateTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(platform_tools::PlatformExecuteTool::new(
            db_pool.clone(),
        )));

        // Register canvas tools
        registry.register(Arc::new(canvas_tools::LoadCanvasTool::new(db_pool.clone())));
        registry.register(Arc::new(canvas_tools::CreateDynamicCanvasTool::new(
            db_pool.clone(),
            bedrock.clone(),
        )));
        registry.register(Arc::new(canvas_tools::CreateFreeformCanvasTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(canvas_tools::UpdateCanvasTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(canvas_tools::PublishCanvasTool::new(
            db_pool.clone(),
        )));

        // Register app tools (interactive multi-view applications)
        registry.register(Arc::new(app_tools::CreateAppTool::new(db_pool.clone())));
        registry.register(Arc::new(app_tools::UpdateAppViewTool::new(db_pool.clone())));

        // Register web tools
        // NOTE: WebSearchTool is intentionally NOT registered here — web search
        // is an agent-only tool (uses Brave API with separate billing). The agent
        // has its own local web_search tool with BRAVE_API_KEY.
        registry.register(Arc::new(web_tools::ViewWebPageTool::new()));

        // Register system tools
        registry.register(Arc::new(system_tools::ReadFileTool::new()));
        registry.register(Arc::new(system_tools::BashTool::new()));

        // Register memory tools (with optional embedding support)
        registry.register(Arc::new(memory_tools::RememberThisTool::new(
            db_pool.clone(),
            embedding_service.clone(),
        )));
        registry.register(Arc::new(memory_tools::SearchMemoryTool::new(
            db_pool.clone(),
            embedding_service.clone(),
        )));

        // Register knowledge base tools (RAG: ingest + semantic search)
        registry.register(Arc::new(knowledge_tools::IngestDocumentTool::new(
            db_pool.clone(),
            embedding_service.clone(),
        )));
        registry.register(Arc::new(knowledge_tools::KnowledgeSearchTool::new(
            db_pool.clone(),
            embedding_service.clone(),
        )));

        // Register workspace awareness tools
        registry.register(Arc::new(workspace_tools::GetWorkspaceSummaryTool::new(
            db_pool.clone(),
        )));

        // Register OpenClaw agent management tools
        registry.register(Arc::new(openclaw_tools::RegisterAgentTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(openclaw_tools::ListAgentsTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(openclaw_tools::AssignTaskTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(openclaw_tools::GetAgentStatusTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(openclaw_tools::StopAgentTool::new(
            db_pool.clone(),
        )));

        // Register schema tools (dynamic collections and records)
        registry.register(Arc::new(schema_tools::DefineCollectionTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(schema_tools::ListCollectionsTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(schema_tools::GetCollectionTool::new(
            db_pool.clone(),
        )));
        let event_tx = automation_engine.create_event_channel();
        registry.register(Arc::new(schema_tools::CreateRecordTool::new(
            db_pool.clone(),
            Some(event_tx.clone()),
        )));
        registry.register(Arc::new(schema_tools::QueryRecordsTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(schema_tools::UpdateRecordTool::new(
            db_pool.clone(),
            Some(event_tx.clone()),
        )));
        registry.register(Arc::new(schema_tools::DeleteRecordTool::new(
            db_pool.clone(),
            Some(event_tx),
        )));

        // Register automation tools
        registry.register(Arc::new(automation_tools::CreateAutomationTool::new(
            automation_engine.clone(),
        )));
        registry.register(Arc::new(automation_tools::ListAutomationsTool::new(
            automation_engine.clone(),
        )));
        registry.register(Arc::new(automation_tools::UpdateAutomationTool::new(
            automation_engine.clone(),
        )));
        registry.register(Arc::new(automation_tools::DeleteAutomationTool::new(
            automation_engine.clone(),
        )));
        registry.register(Arc::new(automation_tools::TestAutomationTool::new(
            automation_engine.clone(),
        )));

        // Register site tools (websites and landing pages)
        registry.register(Arc::new(site_tools::CreateSiteTool::new(db_pool.clone())));
        registry.register(Arc::new(site_tools::CreatePageTool::new(db_pool.clone())));
        registry.register(Arc::new(site_tools::UpdatePageTool::new(db_pool.clone())));
        registry.register(Arc::new(site_tools::PublishSiteTool::new(db_pool.clone())));
        registry.register(Arc::new(site_tools::ListSitesTool::new(db_pool.clone())));

        // Register task queue tools (background tasks and bounties)
        registry.register(Arc::new(task_tools::CreateTaskTool::new(
            task_queue.clone(),
        )));
        registry.register(Arc::new(task_tools::CreateBountyTool::new(
            task_queue.clone(),
        )));
        registry.register(Arc::new(task_tools::ListTasksTool::new(task_queue.clone())));
        registry.register(Arc::new(task_tools::GetTaskResultTool::new(
            task_queue.clone(),
        )));
        registry.register(Arc::new(task_tools::CancelTaskTool::new(
            task_queue.clone(),
        )));

        // Register document tools (export documents)
        registry.register(Arc::new(document_tools::GenerateDocumentTool::new(
            config.clone(),
        )));

        // Register image generation tools
        registry.register(Arc::new(image_gen_tools::GenerateImageTool::new()));

        // Register revision and template tools
        registry.register(Arc::new(revision_tools::ListRevisionsTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(revision_tools::GetRevisionTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(revision_tools::RevertEntityTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(revision_tools::ListTemplatesTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(revision_tools::CheckTemplateUpdatesTool::new(
            db_pool.clone(),
        )));

        // Register credential vault tools
        registry.register(Arc::new(credential_tools::CollectCredentialTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(credential_tools::ListVaultCredentialsTool::new(
            db_pool.clone(),
        )));

        // Register integration tools
        registry.register(Arc::new(integration_tools::ListIntegrationsTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(integration_tools::ListConnectionsTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(integration_tools::CreateConnectionTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(integration_tools::TestConnectionTool::new(
            db_pool.clone(),
            api_executor.clone(),
        )));
        registry.register(Arc::new(
            integration_tools::ExecuteIntegrationActionTool::new(api_executor.clone()),
        ));
        registry.register(Arc::new(integration_tools::ListOperationsTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(integration_tools::CreateSyncConfigTool::new(
            db_pool.clone(),
        )));
        registry.register(Arc::new(integration_tools::TriggerSyncTool::new(
            etl_pipeline.clone(),
        )));

        registry
    }
}

/// Helper macro to define tool parameter schema
#[macro_export]
macro_rules! tool_schema {
    ($($name:expr => $schema:tt),* $(,)?) => {
        serde_json::json!({
            "type": "object",
            "properties": {
                $(
                    $name: $schema
                ),*
            },
            "required": []
        })
    };
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── ToolResult ───────────────────────────────────────────────────

    #[test]
    fn tool_result_success() {
        let result = ToolResult::success(json!({"count": 42}));
        assert!(result.success);
        assert!(result.data.is_some());
        assert!(result.error.is_none());
        assert!(result.metadata.is_none());
    }

    #[test]
    fn tool_result_error() {
        let result = ToolResult::error("something went wrong".to_string());
        assert!(!result.success);
        assert!(result.data.is_none());
        assert_eq!(result.error.unwrap(), "something went wrong");
    }

    #[test]
    fn tool_result_success_with_metadata() {
        let result =
            ToolResult::success_with_metadata(json!({"items": []}), json!({"total": 0, "page": 1}));
        assert!(result.success);
        assert!(result.data.is_some());
        assert!(result.metadata.is_some());
        assert_eq!(result.metadata.unwrap()["total"], 0);
    }

    #[test]
    fn tool_result_serde_roundtrip() {
        let result = ToolResult::success(json!({"key": "value"}));
        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: ToolResult = serde_json::from_str(&serialized).unwrap();
        assert!(deserialized.success);
        assert_eq!(deserialized.data.unwrap()["key"], "value");
    }

    // ── ToolCategory ─────────────────────────────────────────────────

    #[test]
    fn tool_category_as_str() {
        assert_eq!(ToolCategory::Platform.as_str(), "platform");
        assert_eq!(ToolCategory::Canvas.as_str(), "canvas");
        assert_eq!(ToolCategory::Apps.as_str(), "apps");
        assert_eq!(ToolCategory::Web.as_str(), "web");
        assert_eq!(ToolCategory::System.as_str(), "system");
        assert_eq!(ToolCategory::Memory.as_str(), "memory");
        assert_eq!(ToolCategory::Knowledge.as_str(), "knowledge");
        assert_eq!(ToolCategory::OpenClaw.as_str(), "openclaw");
        assert_eq!(ToolCategory::Integration.as_str(), "integration");
        assert_eq!(ToolCategory::Schema.as_str(), "schema");
        assert_eq!(ToolCategory::TaskQueue.as_str(), "task_queue");
        assert_eq!(ToolCategory::Document.as_str(), "document");
        assert_eq!(ToolCategory::ImageGen.as_str(), "image_gen");
        assert_eq!(ToolCategory::Automation.as_str(), "automation");
        assert_eq!(ToolCategory::Other.as_str(), "other");
    }

    #[test]
    fn tool_category_equality() {
        assert_eq!(ToolCategory::Platform, ToolCategory::Platform);
        assert_ne!(ToolCategory::Platform, ToolCategory::Canvas);
    }
}
