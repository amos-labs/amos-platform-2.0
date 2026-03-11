//! Task consumer - background loop that polls the harness for tasks.
//!
//! When deployed as a service (Docker), the agent runs a background loop that:
//! 1. Polls the harness for assigned tasks (GET /agents/{id}/tasks)
//! 2. Executes each task using the agent loop
//! 3. Reports results back (POST /agents/{id}/tasks/{task_id}/result)
//!
//! This enables autonomous, unattended operation alongside interactive chat.

use crate::{
    agent_loop::{self, LoopConfig},
    harness_client::{HarnessClient, TaskResult},
    provider::ModelProvider,
    tools::ToolContext,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Configuration for the task consumer.
#[derive(Debug, Clone)]
pub struct TaskConsumerConfig {
    /// How often to poll for tasks (seconds)
    pub poll_interval_secs: u64,
    /// Maximum concurrent task executions
    pub max_concurrent: usize,
    /// Max iterations per task execution
    pub max_iterations: usize,
}

impl Default for TaskConsumerConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 10,
            max_concurrent: 2,
            max_iterations: 25,
        }
    }
}

/// Run the task consumer loop.
///
/// This is designed to be spawned as a background tokio task.
/// It runs indefinitely, polling the harness for tasks.
pub async fn run_task_consumer(
    config: TaskConsumerConfig,
    harness: Arc<RwLock<HarnessClient>>,
    provider: Arc<dyn ModelProvider>,
    tool_ctx: Arc<ToolContext>,
    loop_config: LoopConfig,
) {
    info!(
        poll_interval = config.poll_interval_secs,
        max_concurrent = config.max_concurrent,
        "Task consumer started"
    );

    let semaphore = Arc::new(tokio::sync::Semaphore::new(config.max_concurrent));

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(config.poll_interval_secs)).await;

        // Poll for tasks
        let tasks = {
            let h = harness.read().await;
            match h.poll_tasks().await {
                Ok(tasks) => tasks,
                Err(e) => {
                    debug!("Task poll failed: {e}");
                    continue;
                }
            }
        };

        if tasks.is_empty() {
            continue;
        }

        info!(count = tasks.len(), "Received tasks from harness");

        for task in tasks {
            let permit = match semaphore.clone().try_acquire_owned() {
                Ok(p) => p,
                Err(_) => {
                    warn!(
                        "Max concurrent tasks reached, skipping task {}",
                        task.task_id
                    );
                    continue;
                }
            };

            let harness = harness.clone();
            let provider = provider.clone();
            let tool_ctx = tool_ctx.clone();
            let loop_config = loop_config.clone();

            tokio::spawn(async move {
                let _permit = permit; // Hold until done

                info!(
                    task_id = %task.task_id,
                    title = %task.title,
                    "Executing task"
                );

                // Build the prompt from the task
                let prompt = format!(
                    "Task: {}\n\nDescription: {}\n\nContext: {}",
                    task.title,
                    task.description,
                    serde_json::to_string_pretty(&task.context).unwrap_or_default()
                );

                // Run the agent loop for this task
                let h_read = harness.read().await;
                let result = agent_loop::run_agent_loop(
                    &loop_config,
                    provider.as_ref(),
                    &tool_ctx,
                    Some(&h_read),
                    &prompt,
                    None, // no event streaming for background tasks
                )
                .await;
                drop(h_read);

                // Report result
                let task_result = match result {
                    Ok(output) => {
                        info!(task_id = %task.task_id, "Task completed successfully");
                        TaskResult {
                            status: "completed".to_string(),
                            output: serde_json::json!({"text": output}),
                            error: None,
                        }
                    }
                    Err(e) => {
                        error!(task_id = %task.task_id, error = %e, "Task failed");
                        TaskResult {
                            status: "failed".to_string(),
                            output: serde_json::json!({}),
                            error: Some(e.to_string()),
                        }
                    }
                };

                let h = harness.read().await;
                if let Err(e) = h.report_result(&task.task_id, task_result).await {
                    error!(task_id = %task.task_id, "Failed to report task result: {e}");
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TaskConsumerConfig::default();
        assert_eq!(config.poll_interval_secs, 10);
        assert_eq!(config.max_concurrent, 2);
        assert_eq!(config.max_iterations, 25);
    }
}
