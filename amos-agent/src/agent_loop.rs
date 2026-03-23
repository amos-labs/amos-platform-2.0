//! Agent loop - the core think-act-observe cycle.
//!
//! This is the agent's main execution loop. It:
//! 1. Sends the conversation to the LLM
//! 2. Receives the response (text + tool calls)
//! 3. Executes tools (local or harness)
//! 4. Feeds results back to the LLM
//! 5. Repeats until the LLM stops calling tools
//!
//! The loop supports both local tools (think, remember, plan, web_search, files)
//! and harness tools (accessed via HTTP through the HarnessClient).

use crate::{
    harness_client::HarnessClient,
    provider::ModelProvider,
    tools::{self, ToolContext},
};
use amos_core::{
    types::{ContentBlock, Message, Role},
    Result,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Configuration for the agent loop.
#[derive(Debug, Clone)]
pub struct LoopConfig {
    pub max_iterations: usize,
    pub system_prompt: String,
    pub model_id: String,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 25,
            system_prompt: default_system_prompt(),
            model_id: "claude-sonnet-4-6".to_string(),
        }
    }
}

/// Events emitted during agent execution for external consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    TurnStart {
        iteration: usize,
    },
    #[serde(rename = "message_delta")]
    TextDelta {
        content: String,
    },
    ToolStart {
        tool_name: String,
        is_local: bool,
    },
    ToolEnd {
        tool_name: String,
        duration_ms: u64,
        is_error: bool,
    },
    TurnEnd {
        iteration: usize,
        tokens_used: u64,
    },
    #[serde(rename = "agent_end")]
    Done {
        total_iterations: usize,
        total_tokens: u64,
        final_text: String,
    },
    Error {
        message: String,
    },
}

/// Run the agent loop to completion.
pub async fn run_agent_loop(
    config: &LoopConfig,
    provider: &dyn ModelProvider,
    tool_ctx: &ToolContext,
    harness: Option<&HarnessClient>,
    initial_message: &str,
    content_blocks: Option<Vec<ContentBlock>>,
    event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
) -> Result<String> {
    let mut messages: Vec<Message> = Vec::new();

    // Build tool schemas: local tools + harness tools
    let local_defs = tools::local_tool_definitions();
    let mut all_tool_schemas = tools::tool_definitions_to_json(&local_defs);
    let local_tool_names: Vec<String> = local_defs.iter().map(|d| d.name.clone()).collect();

    if let Some(h) = harness {
        all_tool_schemas.extend(h.harness_tool_schemas());
    }

    // Add the initial user message.
    // If content blocks are provided (from attachments), build a multi-block
    // message with the text first, followed by images/documents.
    let initial_content = if let Some(mut blocks) = content_blocks {
        // Prepend the user's text message
        blocks.insert(
            0,
            ContentBlock::Text {
                text: initial_message.to_string(),
            },
        );
        blocks
    } else {
        vec![ContentBlock::Text {
            text: initial_message.to_string(),
        }]
    };

    messages.push(Message {
        role: Role::User,
        content: initial_content,
        tool_use_id: None,
        timestamp: Utc::now(),
    });

    let mut total_tokens: u64 = 0;
    let mut final_text = String::new();

    for iteration in 0..config.max_iterations {
        emit(&event_tx, AgentEvent::TurnStart { iteration }).await;
        debug!(iteration, "Agent loop iteration");

        // Call the model
        let (response, usage) = match provider
            .converse(
                &config.model_id,
                &config.system_prompt,
                &messages,
                &all_tool_schemas,
            )
            .await
        {
            Ok(result) => result,
            Err(e) => {
                let error_msg = format!("LLM provider error: {e}");
                tracing::error!("{}", error_msg);
                emit(
                    &event_tx,
                    AgentEvent::Error {
                        message: error_msg.clone(),
                    },
                )
                .await;
                return Err(e);
            }
        };

        total_tokens += usage.total_tokens;

        // Extract text content and tool calls from the response
        let mut has_tool_calls = false;
        let mut response_text = String::new();
        let mut tool_calls: Vec<(String, String, serde_json::Value)> = Vec::new();

        for block in &response.content {
            match block {
                ContentBlock::Text { text } => {
                    response_text.push_str(text);
                    emit(
                        &event_tx,
                        AgentEvent::TextDelta {
                            content: text.clone(),
                        },
                    )
                    .await;
                }
                ContentBlock::ToolUse { id, name, input } => {
                    has_tool_calls = true;
                    tool_calls.push((id.clone(), name.clone(), input.clone()));
                }
                _ => {}
            }
        }

        if !response_text.is_empty() {
            final_text = response_text.clone();
        }

        // Add assistant message to conversation
        messages.push(response);

        emit(
            &event_tx,
            AgentEvent::TurnEnd {
                iteration,
                tokens_used: usage.total_tokens,
            },
        )
        .await;

        // If no tool calls, the agent is done
        if !has_tool_calls {
            info!(iteration, "Agent loop completed (no more tool calls)");
            break;
        }

        // Execute tool calls and collect results
        let mut tool_results: Vec<ContentBlock> = Vec::new();

        for (tool_id, tool_name, input) in &tool_calls {
            let is_local = local_tool_names.contains(tool_name);
            emit(
                &event_tx,
                AgentEvent::ToolStart {
                    tool_name: tool_name.clone(),
                    is_local,
                },
            )
            .await;

            let start = std::time::Instant::now();

            let (result_content, is_error) = if is_local {
                // Execute locally
                match tools::execute_local_tool(tool_name, input, tool_ctx).await {
                    Ok(content) => (content, false),
                    Err(e) => (format!("Tool error: {e}"), true),
                }
            } else if let Some(harness_name) = tool_name.strip_prefix("harness_") {
                // Execute on the harness
                if let Some(h) = harness {
                    match h.execute_tool(harness_name, input.clone(), None).await {
                        Ok(resp) => (resp.content, resp.is_error),
                        Err(e) => (format!("Harness tool error: {e}"), true),
                    }
                } else {
                    (
                        format!("Harness not connected - cannot execute {tool_name}"),
                        true,
                    )
                }
            } else {
                (format!("Unknown tool: {tool_name}"), true)
            };

            let duration_ms = start.elapsed().as_millis() as u64;

            emit(
                &event_tx,
                AgentEvent::ToolEnd {
                    tool_name: tool_name.clone(),
                    duration_ms,
                    is_error,
                },
            )
            .await;

            tool_results.push(ContentBlock::ToolResult {
                tool_use_id: tool_id.clone(),
                content: truncate_tool_result(&result_content),
                is_error,
            });
        }

        // Add tool results as a user message (matching the Bedrock/Anthropic pattern)
        messages.push(Message {
            role: Role::User,
            content: tool_results,
            tool_use_id: None,
            timestamp: Utc::now(),
        });
    }

    emit(
        &event_tx,
        AgentEvent::Done {
            total_iterations: messages.len() / 2,
            total_tokens,
            final_text: final_text.clone(),
        },
    )
    .await;

    Ok(final_text)
}

/// Truncate tool result content to prevent context window overflow.
/// Web search and other tools can return very large results that balloon
/// the conversation past the LLM's token limit.
const MAX_TOOL_RESULT_CHARS: usize = 15_000;

fn truncate_tool_result(content: &str) -> String {
    if content.len() <= MAX_TOOL_RESULT_CHARS {
        content.to_string()
    } else {
        let truncated = &content[..MAX_TOOL_RESULT_CHARS];
        format!(
            "{truncated}\n\n[... truncated — result was {} chars, limit is {}]",
            content.len(),
            MAX_TOOL_RESULT_CHARS
        )
    }
}

async fn emit(tx: &Option<tokio::sync::mpsc::Sender<AgentEvent>>, event: AgentEvent) {
    if let Some(tx) = tx {
        let _ = tx.send(event).await;
    }
}

fn default_system_prompt() -> String {
    r#"You are AMOS Agent, an autonomous AI assistant that is part of the AMOS ecosystem.

You have access to local tools (think, remember, recall, plan, web_search, read_file, write_file) that run directly on your machine, and harness tools (prefixed with harness_) that execute on the AMOS Harness server.

Guidelines:
1. At the START of every conversation, call "harness_get_workspace_summary" to understand what already exists in this workspace (collections, canvases, sites, knowledge base). This prevents recreating things that already exist and lets you build on prior work.
2. Use the "think" tool to reason through complex problems before acting.
3. Use "remember" to store important facts and "recall" to retrieve them.
4. Use "plan" to break complex tasks into steps.
5. Use file tools when you need to read or create files.
6. Harness tools (harness_*) are for database operations, document processing, and other platform capabilities.

Knowledge base — persistent memory across sessions:
- "harness_knowledge_search" performs semantic search over all ingested documents and memories. Use it when the user asks about previously shared information, uploaded documents, or business context.
- "harness_ingest_document" stores a document permanently in the knowledge base with embeddings for future search. Use it when the user provides important reference material.
- Documents uploaded as attachments are automatically ingested into the knowledge base in the background.

Web search — two-stage pattern (IMPORTANT):
- "web_search" returns lightweight snippets only. Use it to find relevant URLs.
- "harness_view_web_page" fetches full page content. Use it selectively on the 1-2 most relevant URLs from search results.
- Do NOT call web_search repeatedly with similar queries. Review results before searching again.
- Do NOT call harness_view_web_page on every search result — pick only the most promising URLs.

Always be helpful, accurate, and thorough. If unsure, search the web or think through the problem first."#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LoopConfig::default();
        assert_eq!(config.max_iterations, 25);
        assert!(config.system_prompt.contains("AMOS Agent"));
    }

    #[test]
    fn test_default_system_prompt_contents() {
        let prompt = default_system_prompt();
        assert!(prompt.contains("think"));
        assert!(prompt.contains("remember"));
        assert!(prompt.contains("web_search"));
        assert!(prompt.contains("harness_"));
    }
}
