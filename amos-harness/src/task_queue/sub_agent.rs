//! Sub-agent system for internal background task execution.
//!
//! When AMOS creates an internal task, the harness spawns a sub-agent:
//! a lightweight background agent loop that runs in its own tokio task.
//! Sub-agents share the harness's `ToolRegistry` and `BedrockClient` but
//! maintain their own conversation context. They communicate results and
//! questions back to AMOS via the task message bus.
//!
//! Think of sub-agents as AMOS's internal workforce -- they do the heavy
//! lifting in the background while AMOS continues talking to the user.

use crate::agent::{BedrockClient, provider::BedrockProvider, loop_runner::{AgentEvent, AgentLoop, LoopConfig}};
use crate::tools::ToolRegistry;
use super::{
    MessageDirection, MessageType, Task, TaskQueue, TaskStatus,
};
use amos_core::Result;
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Handle to a running sub-agent. Holds the JoinHandle so the harness can
/// await completion or cancel if needed.
pub struct SubAgentHandle {
    pub task_id: uuid::Uuid,
    pub join_handle: tokio::task::JoinHandle<()>,
}

/// Spawn a sub-agent to work on an internal task.
///
/// The sub-agent:
/// 1. Transitions the task to Running
/// 2. Builds a focused prompt for the task
/// 3. Runs an agent loop (tool execution until done)
/// 4. Captures the final output via the event stream
/// 5. Posts the result to the task message bus
/// 6. Transitions the task to Completed or Failed
pub async fn spawn_sub_agent(
    task: Task,
    task_queue: Arc<TaskQueue>,
    tool_registry: Arc<ToolRegistry>,
    bedrock_client: BedrockClient,
) -> Result<SubAgentHandle> {
    let task_id = task.id;
    let task_title = task.title.clone();

    // Transition to running
    task_queue
        .update_task_status(task_id, TaskStatus::Running, None, None)
        .await?;

    // Post a status update
    task_queue
        .post_message(
            task_id,
            MessageDirection::AgentToAmos,
            MessageType::StatusUpdate,
            json!({"text": format!("Started working on: {task_title}")}),
        )
        .await?;

    // Clone what the background task needs
    let tq = task_queue.clone();
    let tr = tool_registry;
    let bc = bedrock_client;
    let task_desc = task.description.clone().unwrap_or_default();
    let task_context = task.context.clone();

    let join_handle = tokio::spawn(async move {
        let result = run_sub_agent_loop(
            task_id,
            &task_title,
            &task_desc,
            &task_context,
            &tq,
            tr,
            bc,
        )
        .await;

        match result {
            Ok(output) => {
                // Post result message
                if let Err(e) = tq
                    .post_message(
                        task_id,
                        MessageDirection::AgentToAmos,
                        MessageType::Result,
                        json!({"text": format!("Completed: {task_title}"), "data": output}),
                    )
                    .await
                {
                    error!("Failed to post sub-agent result message: {e}");
                }

                // Transition to completed
                if let Err(e) = tq
                    .update_task_status(
                        task_id,
                        TaskStatus::Completed,
                        Some(output),
                        None,
                    )
                    .await
                {
                    error!("Failed to mark task completed: {e}");
                }

                info!(task_id = %task_id, "Sub-agent completed task: {task_title}");
            }
            Err(e) => {
                let err_msg = format!("{e}");

                // Post error message
                if let Err(post_err) = tq
                    .post_message(
                        task_id,
                        MessageDirection::AgentToAmos,
                        MessageType::Error,
                        json!({"text": format!("Failed: {task_title}"), "error": err_msg}),
                    )
                    .await
                {
                    error!("Failed to post sub-agent error message: {post_err}");
                }

                // Transition to failed
                if let Err(update_err) = tq
                    .update_task_status(
                        task_id,
                        TaskStatus::Failed,
                        None,
                        Some(err_msg.clone()),
                    )
                    .await
                {
                    error!("Failed to mark task failed: {update_err}");
                }

                warn!(task_id = %task_id, error = %err_msg, "Sub-agent failed task: {task_title}");
            }
        }
    });

    Ok(SubAgentHandle {
        task_id,
        join_handle,
    })
}

/// The actual agent loop for a sub-agent.
///
/// Creates a new AgentLoop with conservative settings, subscribes to its
/// event stream to capture the final output, and runs it to completion.
async fn run_sub_agent_loop(
    task_id: uuid::Uuid,
    title: &str,
    description: &str,
    context: &serde_json::Value,
    task_queue: &TaskQueue,
    tool_registry: Arc<ToolRegistry>,
    bedrock_client: BedrockClient,
) -> Result<serde_json::Value> {
    // Build a task-specific user message
    let user_message = format!(
        "You are a sub-agent working on a background task. Complete the following task and return \
         your results.\n\n\
         **Task**: {title}\n\
         **Description**: {description}\n\
         **Context**: {context}\n\n\
         Work through this step by step using available tools. When finished, provide a clear \
         summary of what you accomplished and any results."
    );

    // User context for the sub-agent system prompt
    let user_context = json!({
        "business_name": "AMOS Sub-Agent",
        "user_name": "Background Task",
    });

    // Create a sub-agent loop with conservative settings
    let config = LoopConfig {
        max_iterations: 15, // fewer iterations than main loop
        ..Default::default()
    };

    let provider: Box<dyn crate::agent::provider::ModelProvider> = Box::new(
        BedrockProvider::new(bedrock_client)
    );
    let (mut agent_loop, _cancel_flag) = AgentLoop::new(config, tool_registry, provider);

    // Subscribe to events to capture the final assistant output
    let mut event_rx = agent_loop.subscribe();
    let notify_tx = task_queue.notify_sender();
    let tid = task_id;

    // Spawn a listener that captures agent events and forwards notifications
    let output_handle = tokio::spawn(async move {
        let mut final_output = String::new();

        while let Ok(event) = event_rx.recv().await {
            match event {
                AgentEvent::MessageDelta { content } => {
                    final_output.push_str(&content);
                }
                AgentEvent::ToolStart { tool_name, .. } => {
                    let _ = notify_tx.send(super::TaskNotification {
                        task_id: tid,
                        message_type: MessageType::Progress,
                        summary: format!("Using tool: {tool_name}"),
                    });
                }
                AgentEvent::Error { message, .. } => {
                    let _ = notify_tx.send(super::TaskNotification {
                        task_id: tid,
                        message_type: MessageType::Error,
                        summary: message,
                    });
                }
                _ => {}
            }
        }

        final_output
    });

    // Run the agent loop (this blocks until the loop finishes)
    agent_loop.run(user_message, user_context).await?;

    // Collect the captured output
    let final_text = output_handle.await.unwrap_or_default();

    Ok(json!({
        "output": final_text,
        "task_id": task_id.to_string(),
    }))
}
