//! Tool system types — the contract between packages and the harness.
//!
//! These types live in amos-core so that package crates can implement `Tool`
//! without depending on amos-harness (which would create a circular dependency).

use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Result of tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub data: Option<JsonValue>,
    pub error: Option<String>,
    pub metadata: Option<JsonValue>,
}

impl ToolResult {
    pub fn success(data: JsonValue) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            metadata: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            metadata: None,
        }
    }

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
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> JsonValue;
    async fn execute(&self, params: JsonValue) -> Result<ToolResult>;

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
    Education,
    Autoresearch,
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
            ToolCategory::Education => "education",
            ToolCategory::Autoresearch => "autoresearch",
            ToolCategory::Other => "other",
        }
    }
}
