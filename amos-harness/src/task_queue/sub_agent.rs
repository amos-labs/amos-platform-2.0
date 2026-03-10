//! Sub-agent system stub.
//!
//! Sub-agents have been removed from the harness. Tasks should now be
//! dispatched to registered agents via the harness API instead.
//!
//! This stub maintains the original public API for backwards compatibility
//! but logs a deprecation warning instead of spawning internal agent loops.

use super::{Task, TaskQueue};
use crate::tools::ToolRegistry;
use amos_core::{AmosError, Result};
use std::sync::Arc;
use tracing::warn;

/// Deprecated handle to a sub-agent task.
pub struct SubAgentHandle {
    pub task_id: uuid::Uuid,
    pub join_handle: tokio::task::JoinHandle<()>,
}

/// Spawn a sub-agent to work on an internal task.
///
/// DEPRECATED: This function no longer spawns internal agent loops.
/// Tasks should be dispatched to registered agents via the harness API.
/// This stub logs a warning and immediately fails the task.
pub async fn spawn_sub_agent(
    task: Task,
    task_queue: Arc<TaskQueue>,
    _tool_registry: Arc<ToolRegistry>,
    _bedrock_client: crate::bedrock::BedrockClient,
) -> Result<SubAgentHandle> {
    let task_id = task.id;
    let task_title = task.title.clone();

    warn!(
        task_id = %task_id,
        task_title = %task_title,
        "Sub-agent spawning is deprecated. Tasks should be dispatched to registered agents instead."
    );

    // Create a dummy handle that immediately completes with an error
    let tq = task_queue.clone();
    let join_handle = tokio::spawn(async move {
        let error_msg = "Sub-agent spawning is deprecated. Please dispatch this task to a registered agent via the harness API.";

        // Fail the task
        if let Err(e) = tq
            .update_task_status(
                task_id,
                super::TaskStatus::Failed,
                None,
                Some(error_msg.to_string()),
            )
            .await
        {
            tracing::error!("Failed to mark deprecated sub-agent task as failed: {}", e);
        }
    });

    Ok(SubAgentHandle {
        task_id,
        join_handle,
    })
}
