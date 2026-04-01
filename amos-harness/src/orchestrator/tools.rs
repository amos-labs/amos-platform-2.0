//! Orchestrator tools for multi-harness management.
//!
//! These 5 tools are registered as core tools on the primary harness only,
//! giving the AMOS agent the ability to discover and delegate to specialist harnesses.

use super::proxy::HarnessProxy;
use amos_core::{
    tools::{Tool, ToolCategory, ToolResult},
    Result,
};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::sync::Arc;

// ── list_harnesses ──────────────────────────────────────────────────────

pub struct ListHarnessesTool {
    proxy: Arc<HarnessProxy>,
}

impl ListHarnessesTool {
    pub fn new(proxy: Arc<HarnessProxy>) -> Self {
        Self { proxy }
    }
}

#[async_trait]
impl Tool for ListHarnessesTool {
    fn name(&self) -> &str {
        "list_harnesses"
    }

    fn description(&self) -> &str {
        "List all specialized harness instances, their packages, available tools, and health status. Use this to discover what capabilities are available across your harness fleet."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "package_filter": {
                    "type": "string",
                    "description": "Optional: only show harnesses with this package (e.g., 'autoresearch')"
                }
            },
            "required": []
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Orchestrator
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        // Refresh discovery before listing
        self.proxy.refresh().await;

        let package_filter = params
            .get("package_filter")
            .and_then(|v| v.as_str())
            .map(String::from);

        let siblings = if let Some(pkg) = &package_filter {
            self.proxy.find_by_package(pkg).await
        } else {
            self.proxy.get_siblings().await
        };

        if siblings.is_empty() {
            return Ok(ToolResult::success(serde_json::json!({
                "harnesses": [],
                "message": "No specialist harnesses discovered. This harness is running standalone."
            })));
        }

        let harnesses: Vec<JsonValue> = siblings
            .iter()
            .map(|s| {
                serde_json::json!({
                    "harness_id": s.harness_id,
                    "name": s.name,
                    "role": s.role,
                    "packages": s.packages,
                    "url": s.internal_url,
                    "status": s.status,
                    "healthy": s.healthy,
                })
            })
            .collect();

        Ok(ToolResult::success(serde_json::json!({
            "harnesses": harnesses,
            "count": harnesses.len(),
        })))
    }
}

// ── delegate_to_harness ─────────────────────────────────────────────────

pub struct DelegateToHarnessTool {
    proxy: Arc<HarnessProxy>,
}

impl DelegateToHarnessTool {
    pub fn new(proxy: Arc<HarnessProxy>) -> Self {
        Self { proxy }
    }
}

#[async_trait]
impl Tool for DelegateToHarnessTool {
    fn name(&self) -> &str {
        "delegate_to_harness"
    }

    fn description(&self) -> &str {
        "Execute a specific tool on a named specialist harness. The tool runs synchronously and returns the result. Use list_harnesses first to discover available harnesses and their tools."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "harness": {
                    "type": "string",
                    "description": "Name or ID of the target harness (e.g., 'Trading Research' or a UUID)"
                },
                "tool": {
                    "type": "string",
                    "description": "Name of the tool to execute on the target harness"
                },
                "params": {
                    "type": "object",
                    "description": "Parameters to pass to the tool"
                }
            },
            "required": ["harness", "tool"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Orchestrator
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let harness = params
            .get("harness")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("'harness' is required".into()))?;

        let tool = params
            .get("tool")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("'tool' is required".into()))?;

        let tool_params = params
            .get("params")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        match self.proxy.execute_tool(harness, tool, tool_params).await {
            Ok(result) => Ok(result),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to delegate to harness '{}': {}",
                harness, e
            ))),
        }
    }
}

// ── submit_task_to_harness ──────────────────────────────────────────────

pub struct SubmitTaskToHarnessTool {
    proxy: Arc<HarnessProxy>,
}

impl SubmitTaskToHarnessTool {
    pub fn new(proxy: Arc<HarnessProxy>) -> Self {
        Self { proxy }
    }
}

#[async_trait]
impl Tool for SubmitTaskToHarnessTool {
    fn name(&self) -> &str {
        "submit_task_to_harness"
    }

    fn description(&self) -> &str {
        "Submit an asynchronous task to a specialist harness's agent. Returns a task ID for tracking. Use this for long-running operations that don't need an immediate result."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "harness": {
                    "type": "string",
                    "description": "Name or ID of the target harness"
                },
                "task": {
                    "type": "string",
                    "description": "Description of the task to perform"
                }
            },
            "required": ["harness", "task"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Orchestrator
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let harness = params
            .get("harness")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("'harness' is required".into()))?;

        let task = params
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("'task' is required".into()))?;

        match self.proxy.submit_task(harness, task).await {
            Ok(task_id) => Ok(ToolResult::success(serde_json::json!({
                "task_id": task_id,
                "harness": harness,
                "status": "submitted",
            }))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to submit task to harness '{}': {}",
                harness, e
            ))),
        }
    }
}

// ── get_harness_status ──────────────────────────────────────────────────

pub struct GetHarnessStatusTool {
    proxy: Arc<HarnessProxy>,
}

impl GetHarnessStatusTool {
    pub fn new(proxy: Arc<HarnessProxy>) -> Self {
        Self { proxy }
    }
}

#[async_trait]
impl Tool for GetHarnessStatusTool {
    fn name(&self) -> &str {
        "get_harness_status"
    }

    fn description(&self) -> &str {
        "Get detailed status of a specialist harness including health, available tools, active packages, and connectivity."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "harness": {
                    "type": "string",
                    "description": "Name or ID of the target harness"
                }
            },
            "required": ["harness"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Orchestrator
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let harness = params
            .get("harness")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("'harness' is required".into()))?;

        match self.proxy.get_status(harness).await {
            Ok(status) => Ok(ToolResult::success(serde_json::json!({
                "harness_id": status.harness_id,
                "name": status.name,
                "role": status.role,
                "packages": status.packages,
                "status": status.status,
                "healthy": status.healthy,
                "tools": status.tools,
                "tool_count": status.tools.len(),
            }))),
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to get status for harness '{}': {}",
                harness, e
            ))),
        }
    }
}

// ── broadcast_to_harnesses ──────────────────────────────────────────────

pub struct BroadcastToHarnessesTool {
    proxy: Arc<HarnessProxy>,
}

impl BroadcastToHarnessesTool {
    pub fn new(proxy: Arc<HarnessProxy>) -> Self {
        Self { proxy }
    }
}

#[async_trait]
impl Tool for BroadcastToHarnessesTool {
    fn name(&self) -> &str {
        "broadcast_to_harnesses"
    }

    fn description(&self) -> &str {
        "Execute the same tool on all harnesses matching a filter (e.g., all with a specific package). Returns results from each harness."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "package_filter": {
                    "type": "string",
                    "description": "Only target harnesses with this package"
                },
                "tool": {
                    "type": "string",
                    "description": "Name of the tool to execute on each harness"
                },
                "params": {
                    "type": "object",
                    "description": "Parameters to pass to the tool"
                }
            },
            "required": ["tool"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Orchestrator
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let tool = params
            .get("tool")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("'tool' is required".into()))?;

        let tool_params = params
            .get("params")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        let package_filter = params.get("package_filter").and_then(|v| v.as_str());

        let siblings = if let Some(pkg) = package_filter {
            self.proxy.find_by_package(pkg).await
        } else {
            self.proxy.get_siblings().await
        };

        if siblings.is_empty() {
            return Ok(ToolResult::success(serde_json::json!({
                "results": [],
                "message": "No matching harnesses found"
            })));
        }

        let mut results = Vec::new();
        for sibling in &siblings {
            let name = sibling.name.as_deref().unwrap_or(&sibling.harness_id);
            match self
                .proxy
                .execute_tool(name, tool, tool_params.clone())
                .await
            {
                Ok(result) => {
                    results.push(serde_json::json!({
                        "harness": name,
                        "success": result.success,
                        "data": result.data,
                        "error": result.error,
                    }));
                }
                Err(e) => {
                    results.push(serde_json::json!({
                        "harness": name,
                        "success": false,
                        "error": e,
                    }));
                }
            }
        }

        Ok(ToolResult::success(serde_json::json!({
            "results": results,
            "harness_count": siblings.len(),
        })))
    }
}
