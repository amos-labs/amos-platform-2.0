//! # Tool Registry & Implementations
//!
//! Central registry for V3's 12 composable tools. Each tool implements
//! the [`Tool`] trait and is registered at startup.
//!
//! Tools exposed to the LLM:
//!   - platform_create, platform_query, platform_update, platform_execute
//!   - web_search, view_web_page, read_file
//!   - bash, browser_use, load_canvas
//!   - remember_this, search_memory

pub mod platform_create;
pub mod platform_query;
pub mod platform_update;
pub mod platform_execute;
pub mod web_search;
pub mod web_page;
pub mod read_file;
pub mod bash;
pub mod browser_use;
pub mod load_canvas;
pub mod memory;

use amos_core::error::{AmosError, Result};
use amos_core::types::ToolDefinition;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

/// Trait that every tool must implement.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name as exposed to the LLM (e.g. "platform_create").
    fn name(&self) -> &str;

    /// JSON Schema definition for the LLM.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with the given input.
    async fn execute(&self, input: &Value) -> Result<String>;
}

/// Central registry of all available tools.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Build the default registry with all 12 V3 tools.
    pub fn default_registry(
        db_pool: Option<sqlx::PgPool>,
        redis_client: Option<redis::Client>,
        http_client: reqwest::Client,
    ) -> Self {
        let mut registry = Self::new();

        // Platform tools (delegate to Rails API or direct DB)
        registry.register(Arc::new(platform_create::PlatformCreateTool::new(
            http_client.clone(),
        )));
        registry.register(Arc::new(platform_query::PlatformQueryTool::new(
            http_client.clone(),
        )));
        registry.register(Arc::new(platform_update::PlatformUpdateTool::new(
            http_client.clone(),
        )));
        registry.register(Arc::new(platform_execute::PlatformExecuteTool::new(
            http_client.clone(),
        )));

        // Research tools
        registry.register(Arc::new(web_search::WebSearchTool::new(
            http_client.clone(),
        )));
        registry.register(Arc::new(web_page::ViewWebPageTool::new(
            http_client.clone(),
        )));
        registry.register(Arc::new(read_file::ReadFileTool::new(
            http_client.clone(),
        )));

        // System tools
        registry.register(Arc::new(bash::BashTool::new()));
        registry.register(Arc::new(browser_use::BrowserUseTool::new()));
        registry.register(Arc::new(load_canvas::LoadCanvasTool::new(
            http_client.clone(),
        )));

        // Memory tools
        registry.register(Arc::new(memory::RememberThisTool::new(
            http_client.clone(),
        )));
        registry.register(Arc::new(memory::SearchMemoryTool::new(
            http_client.clone(),
        )));

        registry
    }

    /// Register a tool.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        debug!(tool = %name, "Registering tool");
        self.tools.insert(name, tool);
    }

    /// Get all tool definitions for LLM invocation.
    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Execute a tool by name.
    pub async fn execute(&self, name: &str, input: &Value) -> Result<String> {
        let tool = self.tools.get(name).ok_or(AmosError::ToolNotFound {
            name: name.to_string(),
        })?;

        debug!(tool = %name, "Executing tool");
        tool.execute(input).await
    }

    /// List all registered tool names.
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }
}
