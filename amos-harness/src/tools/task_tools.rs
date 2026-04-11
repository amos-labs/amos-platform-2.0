//! Task management tools for AMOS
//!
//! These tools give AMOS the ability to create, monitor, and manage
//! background tasks. Tasks fall into two categories:
//!
//! - **Internal**: Spawns a sub-agent inside the harness to do the work
//! - **External (bounty)**: Posts work for external OpenClaw agents to claim
//!
//! The tools operate on the unified `TaskQueue` infrastructure.

use super::{Tool, ToolCategory, ToolResult};
use crate::task_queue::{CreateTaskParams, TaskCategory as TCategory, TaskQueue, TaskStatus};
use amos_core::Result;
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use uuid::Uuid;

// ── CreateTaskTool ──────────────────────────────────────────────────────

/// Create a new internal task (sub-agent background work).
pub struct CreateTaskTool {
    task_queue: Arc<TaskQueue>,
}

impl CreateTaskTool {
    pub fn new(task_queue: Arc<TaskQueue>) -> Self {
        Self { task_queue }
    }
}

#[async_trait]
impl Tool for CreateTaskTool {
    fn name(&self) -> &str {
        "create_task"
    }

    fn description(&self) -> &str {
        "Create an internal background task. A sub-agent will be spawned to work on it \
         asynchronously while you continue the conversation with the user. Use this for \
         work you can handle with your own tools but want to do in the background: \
         research, data processing, report generation, etc."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short title describing the task (e.g. 'Research competitor pricing')"
                },
                "description": {
                    "type": "string",
                    "description": "Detailed description of what the sub-agent should do"
                },
                "priority": {
                    "type": "integer",
                    "description": "Priority from 1 (highest) to 10 (lowest). Default: 5",
                    "minimum": 1,
                    "maximum": 10
                },
                "context": {
                    "type": "object",
                    "description": "Additional context data the sub-agent may need (JSON)"
                },
                "session_id": {
                    "type": "string",
                    "description": "Session ID to associate the task with (UUID format)"
                },
                "parent_task_id": {
                    "type": "string",
                    "description": "Parent task ID if this is a sub-task (UUID format)"
                }
            },
            "required": ["title", "description"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let title = params["title"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("title is required".to_string()))?;

        let description = params["description"].as_str().ok_or_else(|| {
            amos_core::AmosError::Validation("description is required".to_string())
        })?;

        let priority = params
            .get("priority")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);
        let context = params.get("context").cloned();
        let session_id = params
            .get("session_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        let parent_task_id = params
            .get("parent_task_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        let create_params = CreateTaskParams {
            title: title.to_string(),
            description: Some(description.to_string()),
            context,
            category: TCategory::Internal,
            task_type: None,
            priority,
            session_id,
            parent_task_id,
            reward_tokens: None,
            deadline_at: None,
        };

        match self.task_queue.create_task(create_params).await {
            Ok(task) => Ok(ToolResult::success(json!({
                "task_id": task.id.to_string(),
                "title": task.title,
                "category": "internal",
                "status": "pending",
                "priority": task.priority,
                "message": format!(
                    "Internal task '{}' created (ID: {}). A sub-agent will pick it up shortly.",
                    task.title, task.id
                )
            }))),
            Err(e) => Ok(ToolResult::error(format!("Failed to create task: {e}"))),
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TaskQueue
    }
}

// ── CreateBountyTool ────────────────────────────────────────────────────

/// Create a bounty on the AMOS relay marketplace for agents to claim.
pub struct CreateBountyTool {
    relay_url: String,
}

impl CreateBountyTool {
    pub fn new(relay_url: String) -> Self {
        Self { relay_url }
    }
}

#[async_trait]
impl Tool for CreateBountyTool {
    fn name(&self) -> &str {
        "create_bounty"
    }

    fn description(&self) -> &str {
        "Create an external bounty task for OpenClaw agents to claim. Use this when the \
         work requires capabilities outside the harness (shell access, browser control, \
         specialized APIs) or when you want to delegate to a specialized external agent. \
         External agents will see the bounty and can claim it."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short title describing the bounty (e.g. 'Scrape competitor pricing pages')"
                },
                "description": {
                    "type": "string",
                    "description": "Detailed description of what needs to be done and expected deliverables"
                },
                "reward_tokens": {
                    "type": "integer",
                    "description": "AMOS token reward for completing the bounty. Default: 0",
                    "minimum": 0
                },
                "deadline": {
                    "type": "string",
                    "description": "Optional deadline in ISO 8601 format (e.g. '2026-04-15T00:00:00Z')"
                },
                "required_capabilities": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Capabilities required to complete this bounty (e.g. ['web_search', 'code_execution'])"
                }
            },
            "required": ["title", "description"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let title = params["title"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("title is required".to_string()))?;

        let description = params["description"].as_str().ok_or_else(|| {
            amos_core::AmosError::Validation("description is required".to_string())
        })?;

        let reward_tokens = params
            .get("reward_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let deadline = params.get("deadline").and_then(|v| v.as_str());

        let capabilities: Vec<String> = params
            .get("required_capabilities")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let payload = json!({
            "title": title,
            "description": description,
            "reward_tokens": reward_tokens,
            "deadline": deadline,
            "required_capabilities": capabilities,
        });

        let url = format!("{}/api/v1/bounties", self.relay_url);
        let client = reqwest::Client::new();

        match client.post(&url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                let body: JsonValue = resp.json().await.unwrap_or(json!({}));
                let bounty_id = body["id"].as_str().unwrap_or("unknown");
                Ok(ToolResult::success(json!({
                    "bounty_id": bounty_id,
                    "title": title,
                    "status": "open",
                    "reward_tokens": reward_tokens,
                    "message": format!(
                        "Bounty '{}' posted to marketplace (ID: {}). Agents can now claim it.",
                        title, bounty_id
                    )
                })))
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                Ok(ToolResult::error(format!(
                    "Relay returned {}: {}",
                    status, body
                )))
            }
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to reach relay marketplace: {e}"
            ))),
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TaskQueue
    }
}

// ── ListTasksTool ───────────────────────────────────────────────────────

/// List tasks with optional filtering.
pub struct ListTasksTool {
    task_queue: Arc<TaskQueue>,
}

impl ListTasksTool {
    pub fn new(task_queue: Arc<TaskQueue>) -> Self {
        Self { task_queue }
    }
}

#[async_trait]
impl Tool for ListTasksTool {
    fn name(&self) -> &str {
        "list_tasks"
    }

    fn description(&self) -> &str {
        "List background tasks and bounties. Filter by status to see active, completed, \
         or failed tasks. Use this to check on the progress of work you delegated to \
         sub-agents or external agents."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["pending", "assigned", "running", "completed", "failed", "cancelled"],
                    "description": "Filter by task status (optional)"
                },
                "session_id": {
                    "type": "string",
                    "description": "Filter by session ID (UUID format, optional)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of tasks to return (default: 20)",
                    "minimum": 1,
                    "maximum": 100
                }
            }
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let session_id = params
            .get("session_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        let status_filter = params
            .get("status")
            .and_then(|v| v.as_str())
            .and_then(TaskStatus::from_str);

        let limit = params.get("limit").and_then(|v| v.as_i64()).or(Some(20));

        match self
            .task_queue
            .list_tasks(session_id, status_filter, limit)
            .await
        {
            Ok(tasks) => {
                let task_list: Vec<JsonValue> = tasks
                    .iter()
                    .map(|t| {
                        json!({
                            "id": t.id.to_string(),
                            "title": t.title,
                            "category": t.category.as_str(),
                            "status": t.status.as_str(),
                            "priority": t.priority,
                            "assigned_to": t.assigned_to.map(|u| u.to_string()),
                            "reward_tokens": t.reward_tokens,
                            "created_at": t.created_at.to_rfc3339(),
                            "has_result": t.result.is_some(),
                            "has_error": t.error_message.is_some(),
                        })
                    })
                    .collect();

                let count = task_list.len();
                Ok(ToolResult::success(json!({
                    "tasks": task_list,
                    "count": count,
                })))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to list tasks: {e}"))),
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TaskQueue
    }
}

// ── GetTaskResultTool ───────────────────────────────────────────────────

/// Get the result and messages for a specific task.
pub struct GetTaskResultTool {
    task_queue: Arc<TaskQueue>,
}

impl GetTaskResultTool {
    pub fn new(task_queue: Arc<TaskQueue>) -> Self {
        Self { task_queue }
    }
}

#[async_trait]
impl Tool for GetTaskResultTool {
    fn name(&self) -> &str {
        "get_task_result"
    }

    fn description(&self) -> &str {
        "Get the detailed status, result, and message history for a specific task. \
         Use this to review what a sub-agent or external agent accomplished, read \
         any questions they posted, or check error details for failed tasks."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to look up (UUID format)"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let task_id_str = params["task_id"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("task_id is required".to_string()))?;

        let task_id = Uuid::parse_str(task_id_str).map_err(|_| {
            amos_core::AmosError::Validation(format!("Invalid UUID: {task_id_str}"))
        })?;

        // Fetch task and its messages in parallel
        let task_result = self.task_queue.get_task(task_id).await;
        let task = match task_result {
            Ok(t) => t,
            Err(e) => return Ok(ToolResult::error(format!("Task not found: {e}"))),
        };

        let messages = self
            .task_queue
            .messages_for_task(task_id)
            .await
            .unwrap_or_default();

        let message_list: Vec<JsonValue> = messages
            .iter()
            .map(|m| {
                json!({
                    "id": m.id.to_string(),
                    "direction": m.direction.as_str(),
                    "type": m.message_type.as_str(),
                    "content": m.content,
                    "acknowledged": m.acknowledged,
                    "created_at": m.created_at.to_rfc3339(),
                })
            })
            .collect();

        Ok(ToolResult::success(json!({
            "task": {
                "id": task.id.to_string(),
                "title": task.title,
                "description": task.description,
                "category": task.category.as_str(),
                "status": task.status.as_str(),
                "priority": task.priority,
                "assigned_to": task.assigned_to.map(|u| u.to_string()),
                "result": task.result,
                "error_message": task.error_message,
                "reward_tokens": task.reward_tokens,
                "created_at": task.created_at.to_rfc3339(),
                "started_at": task.started_at.map(|t| t.to_rfc3339()),
                "completed_at": task.completed_at.map(|t| t.to_rfc3339()),
            },
            "messages": message_list,
            "message_count": message_list.len(),
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TaskQueue
    }
}

// ── CancelTaskTool ──────────────────────────────────────────────────────

/// Cancel a running or pending task.
pub struct CancelTaskTool {
    task_queue: Arc<TaskQueue>,
}

impl CancelTaskTool {
    pub fn new(task_queue: Arc<TaskQueue>) -> Self {
        Self { task_queue }
    }
}

#[async_trait]
impl Tool for CancelTaskTool {
    fn name(&self) -> &str {
        "cancel_task"
    }

    fn description(&self) -> &str {
        "Cancel a pending or running task. This stops work on the task and marks it \
         as cancelled. Use this if a task is no longer needed or the user changes their mind."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to cancel (UUID format)"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let task_id_str = params["task_id"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("task_id is required".to_string()))?;

        let task_id = Uuid::parse_str(task_id_str).map_err(|_| {
            amos_core::AmosError::Validation(format!("Invalid UUID: {task_id_str}"))
        })?;

        // Check if task is already terminal
        let task = match self.task_queue.get_task(task_id).await {
            Ok(t) => t,
            Err(e) => return Ok(ToolResult::error(format!("Task not found: {e}"))),
        };

        if task.status.is_terminal() {
            return Ok(ToolResult::error(format!(
                "Task is already in terminal state: {}",
                task.status
            )));
        }

        match self.task_queue.cancel_task(task_id).await {
            Ok(updated) => Ok(ToolResult::success(json!({
                "task_id": updated.id.to_string(),
                "title": updated.title,
                "previous_status": task.status.as_str(),
                "status": "cancelled",
                "message": format!("Task '{}' has been cancelled.", updated.title)
            }))),
            Err(e) => Ok(ToolResult::error(format!("Failed to cancel task: {e}"))),
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TaskQueue
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    /// Helper to create a TaskQueue for testing (requires tokio runtime).
    fn test_task_queue() -> Arc<TaskQueue> {
        Arc::new(TaskQueue::new(
            PgPool::connect_lazy("postgres://localhost/fake").unwrap(),
        ))
    }

    #[tokio::test]
    async fn create_task_tool_metadata() {
        let tq = test_task_queue();
        let tool = CreateTaskTool::new(tq);
        assert_eq!(tool.name(), "create_task");
        assert_eq!(tool.category(), ToolCategory::TaskQueue);

        let schema = tool.parameters_schema();
        assert_eq!(schema["required"], json!(["title", "description"]));
        assert!(schema["properties"]["title"].is_object());
        assert!(schema["properties"]["priority"].is_object());
    }

    #[tokio::test]
    async fn create_bounty_tool_metadata() {
        let tool = CreateBountyTool::new("http://localhost:4100".to_string());
        assert_eq!(tool.name(), "create_bounty");
        assert_eq!(tool.category(), ToolCategory::TaskQueue);

        let schema = tool.parameters_schema();
        assert!(schema["properties"]["reward_tokens"].is_object());
        assert!(schema["properties"]["required_capabilities"].is_object());
    }

    #[tokio::test]
    async fn list_tasks_tool_metadata() {
        let tq = test_task_queue();
        let tool = ListTasksTool::new(tq);
        assert_eq!(tool.name(), "list_tasks");
        assert_eq!(tool.category(), ToolCategory::TaskQueue);

        let schema = tool.parameters_schema();
        assert!(schema["properties"]["status"].is_object());
        assert!(schema["properties"]["limit"].is_object());
    }

    #[tokio::test]
    async fn get_task_result_tool_metadata() {
        let tq = test_task_queue();
        let tool = GetTaskResultTool::new(tq);
        assert_eq!(tool.name(), "get_task_result");
        assert_eq!(tool.category(), ToolCategory::TaskQueue);

        let schema = tool.parameters_schema();
        assert_eq!(schema["required"], json!(["task_id"]));
    }

    #[tokio::test]
    async fn cancel_task_tool_metadata() {
        let tq = test_task_queue();
        let tool = CancelTaskTool::new(tq);
        assert_eq!(tool.name(), "cancel_task");
        assert_eq!(tool.category(), ToolCategory::TaskQueue);

        let schema = tool.parameters_schema();
        assert_eq!(schema["required"], json!(["task_id"]));
    }

    #[tokio::test]
    async fn all_tools_have_unique_names() {
        let tq = test_task_queue();
        let names = [
            CreateTaskTool::new(tq.clone()).name().to_string(),
            CreateBountyTool::new("http://localhost:4100".to_string())
                .name()
                .to_string(),
            ListTasksTool::new(tq.clone()).name().to_string(),
            GetTaskResultTool::new(tq.clone()).name().to_string(),
            CancelTaskTool::new(tq.clone()).name().to_string(),
        ];
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "Tool names must be unique");
    }
}
