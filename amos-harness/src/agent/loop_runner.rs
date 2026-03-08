//! V3 Agent Loop - Event-driven agent execution with streaming
//!
//! Inspired by Pi's architecture, this implements:
//! - Event-driven streaming via broadcast channels
//! - Tool execution with timeout and cancellation
//! - Model escalation on failure
//! - Hallucination detection
//! - Conversation compaction
//! - Steering messages

use super::{bedrock::BedrockClient, model_registry::ModelRegistry, prompt_builder};
use crate::tools::ToolRegistry;
use amos_core::{
    types::{ContentBlock, Message, Role},
    AmosError, Result,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::broadcast,
    time::timeout,
};
use tracing::{error, info, warn};

/// Configuration for the agent loop
#[derive(Debug, Clone)]
pub struct LoopConfig {
    /// Maximum number of iterations before forcing stop
    pub max_iterations: usize,

    /// Context window threshold for triggering compaction (in tokens)
    pub compaction_threshold: usize,

    /// Timeout for individual tool executions
    pub tool_timeout: Duration,

    /// Enable hallucination detection
    pub detect_hallucinations: bool,

    /// Enable model escalation on errors
    pub enable_escalation: bool,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 25,
            compaction_threshold: 100_000,
            tool_timeout: Duration::from_secs(30),
            detect_hallucinations: true,
            enable_escalation: true,
        }
    }
}

/// Events emitted during agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// A new turn has started
    TurnStart {
        iteration: usize,
        model: String,
    },

    /// A new message is being generated
    MessageStart {
        role: String,
    },

    /// Delta of message content (streaming)
    MessageDelta {
        content: String,
    },

    /// Message generation completed
    MessageEnd {
        message: Message,
    },

    /// A tool is being executed
    ToolStart {
        tool_name: String,
        tool_id: String,
    },

    /// Tool execution completed
    ToolEnd {
        tool_name: String,
        tool_id: String,
        result: serde_json::Value,
        duration_ms: u64,
    },

    /// A turn has completed
    TurnEnd {
        iteration: usize,
        tokens_used: u64,
    },

    /// Agent has completed execution
    AgentEnd {
        total_iterations: usize,
        total_tokens: u64,
        reason: String,
    },

    /// An error occurred
    Error {
        message: String,
        recoverable: bool,
    },

    /// Model escalation occurred
    ModelEscalation {
        from_model: String,
        to_model: String,
        reason: String,
    },

    /// Hallucination detected
    HallucinationDetected {
        phrase: String,
    },

    /// Conversation compacted
    ConversationCompacted {
        original_tokens: usize,
        new_tokens: usize,
    },
}

/// The agent loop executor
pub struct AgentLoop {
    config: LoopConfig,
    tool_registry: Arc<ToolRegistry>,
    model_registry: ModelRegistry,
    bedrock_client: BedrockClient,
    conversation: Vec<Message>,
    current_model: String,
    total_tokens: u64,
    iteration: usize,
    event_tx: broadcast::Sender<AgentEvent>,
    /// Shared cancellation flag — set to `true` to stop the loop
    cancelled: Arc<AtomicBool>,
}

impl AgentLoop {
    /// Create a new agent loop.
    ///
    /// Returns `(Self, Arc<AtomicBool>)` — the caller keeps the `Arc<AtomicBool>`
    /// and can set it to `true` to cancel the running loop.
    pub fn new(
        config: LoopConfig,
        tool_registry: Arc<ToolRegistry>,
        bedrock_client: BedrockClient,
    ) -> (Self, Arc<AtomicBool>) {
        let (event_tx, _) = broadcast::channel(1000);
        let model_registry = ModelRegistry::new();
        let current_model = model_registry.get_cheapest().id.clone();
        let cancelled = Arc::new(AtomicBool::new(false));

        let agent = Self {
            config,
            tool_registry,
            model_registry,
            bedrock_client,
            conversation: Vec::new(),
            current_model,
            total_tokens: 0,
            iteration: 0,
            event_tx,
            cancelled: cancelled.clone(),
        };

        (agent, cancelled)
    }

    /// Subscribe to agent events
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_tx.subscribe()
    }

    /// Run the agent loop with the given user message
    pub async fn run(&mut self, user_message: String, user_context: serde_json::Value) -> Result<()> {
        self.run_with_attachments(user_message, user_context, Vec::new()).await
    }

    /// Run the agent loop with a user message plus additional content blocks (e.g. images).
    pub async fn run_with_attachments(
        &mut self,
        user_message: String,
        user_context: serde_json::Value,
        extra_blocks: Vec<ContentBlock>,
    ) -> Result<()> {
        // Build system prompt
        let system_prompt = prompt_builder::build_system_prompt(user_context)?;

        // Build the user message content blocks: text first, then any attachments
        let mut content = vec![ContentBlock::Text {
            text: user_message,
        }];
        content.extend(extra_blocks);

        // ── Smart model routing ────────────────────────────────────────
        // Route to the best model based on content type, user intent keywords,
        // and message complexity. Haiku for simple chat, Sonnet for tools/docs/
        // analytical tasks, Opus for expert-level requests.
        //
        // Check both the NEW message content AND the conversation history
        // (prior session messages may contain Document/Image blocks from
        // earlier uploads that still need a capable model).
        let has_documents = content.iter().any(|b| matches!(b, ContentBlock::Document { .. }))
            || self.conversation.iter().any(|m| m.content.iter().any(|b| matches!(b, ContentBlock::Document { .. })));
        let has_images = content.iter().any(|b| matches!(b, ContentBlock::Image { .. }))
            || self.conversation.iter().any(|m| m.content.iter().any(|b| matches!(b, ContentBlock::Image { .. })));
        let user_text = content.iter().find_map(|b| match b {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        }).unwrap_or("");

        let routing = self.model_registry.route(user_text, has_documents, has_images);
        if routing.model_id != self.current_model {
            info!(
                "Model router: {} → {} (reason: {})",
                self.current_model, routing.display_name, routing.reason
            );
            let _ = self.emit_event(AgentEvent::ModelEscalation {
                from_model: self.current_model.clone(),
                to_model: routing.model_id.clone(),
                reason: format!("Pre-routing: {}", routing.reason),
            }).await;
            self.current_model = routing.model_id;
        }

        // Add user message to conversation
        self.conversation.push(Message {
            role: Role::User,
            content,
            tool_use_id: None,
            timestamp: Utc::now(),
        });

        // Main agent loop
        while self.iteration < self.config.max_iterations {
            // ── Cancellation check ──────────────────────────────────────
            if self.cancelled.load(Ordering::Relaxed) {
                info!("Agent loop cancelled by user");
                self.emit_event(AgentEvent::AgentEnd {
                    total_iterations: self.iteration,
                    total_tokens: self.total_tokens,
                    reason: "cancelled".to_string(),
                })
                .await;
                return Ok(());
            }

            self.iteration += 1;

            self.emit_event(AgentEvent::TurnStart {
                iteration: self.iteration,
                model: self.current_model.clone(),
            })
            .await;

            // Check if we need to compact the conversation
            if self.should_compact() {
                self.compact_conversation().await?;
            }

            // Execute one turn
            match self.execute_turn(&system_prompt).await {
                Ok(should_continue) => {
                    if !should_continue {
                        self.emit_event(AgentEvent::AgentEnd {
                            total_iterations: self.iteration,
                            total_tokens: self.total_tokens,
                            reason: "natural_completion".to_string(),
                        })
                        .await;
                        break;
                    }
                }
                Err(e) => {
                    error!("Error in agent loop: {:?}", e);

                    // Try to escalate model if enabled
                    if self.config.enable_escalation {
                        if let Some(next_model) = self.model_registry.escalate(&self.current_model)
                        {
                            let next_model_id = next_model.id.clone();
                            self.emit_event(AgentEvent::ModelEscalation {
                                from_model: self.current_model.clone(),
                                to_model: next_model_id.clone(),
                                reason: format!("Error: {}", e),
                            })
                            .await;
                            self.current_model = next_model_id;
                            continue;
                        }
                    }

                    self.emit_event(AgentEvent::Error {
                        message: format!("{:?}", e),
                        recoverable: false,
                    })
                    .await;
                    return Err(e);
                }
            }
        }

        if self.iteration >= self.config.max_iterations {
            self.emit_event(AgentEvent::AgentEnd {
                total_iterations: self.iteration,
                total_tokens: self.total_tokens,
                reason: "max_iterations_reached".to_string(),
            })
            .await;
        }

        Ok(())
    }

    /// Execute a single turn of the agent loop
    async fn execute_turn(&mut self, system_prompt: &str) -> Result<bool> {
        // Stream response from model
        let mut stream_rx = self
            .bedrock_client
            .converse_stream(
                &self.current_model,
                system_prompt,
                &self.conversation,
                &self.tool_registry.get_tool_schemas(),
            )
            .await?;

        self.emit_event(AgentEvent::MessageStart {
            role: "assistant".to_string(),
        })
        .await;

        let mut current_text = String::new();
        let mut tool_uses = Vec::new();
        let mut turn_tokens = 0u64;

        // Process stream events
        while let Some(event) = stream_rx.recv().await {
            // Check cancellation inside the stream loop for fast abort
            if self.cancelled.load(Ordering::Relaxed) {
                info!("Agent cancelled during streaming");
                break;
            }

            match event {
                super::bedrock::StreamEvent::TextDelta(text) => {
                    current_text.push_str(&text);
                    self.emit_event(AgentEvent::MessageDelta { content: text })
                        .await;
                }
                super::bedrock::StreamEvent::ToolUse { id, name, input } => {
                    tool_uses.push((id, name, input));
                }
                super::bedrock::StreamEvent::Stop => {
                    break;
                }
                super::bedrock::StreamEvent::Error(e) => {
                    return Err(AmosError::Internal(format!(
                        "Stream error: {}",
                        e
                    )));
                }
                super::bedrock::StreamEvent::TokenUsage(usage) => {
                    turn_tokens = usage.total_tokens;
                    self.total_tokens += turn_tokens;
                }
            }
        }

        // If cancelled mid-stream, bail out — skip tool execution
        if self.cancelled.load(Ordering::Relaxed) {
            self.emit_event(AgentEvent::AgentEnd {
                total_iterations: self.iteration,
                total_tokens: self.total_tokens,
                reason: "cancelled".to_string(),
            })
            .await;
            return Ok(false);
        }

        // Build assistant message
        let mut content_blocks = Vec::new();
        if !current_text.is_empty() {
            content_blocks.push(ContentBlock::Text { text: current_text.clone() });
        }
        for (id, name, input) in &tool_uses {
            content_blocks.push(ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            });
        }

        let assistant_message = Message {
            role: Role::Assistant,
            content: content_blocks,
            tool_use_id: None,
            timestamp: Utc::now(),
        };

        self.emit_event(AgentEvent::MessageEnd {
            message: assistant_message.clone(),
        })
        .await;

        self.conversation.push(assistant_message);

        // Detect hallucinations if enabled
        if self.config.detect_hallucinations && !tool_uses.is_empty() {
            self.detect_hallucinations(&current_text);
        }

        // Execute tools if any
        if !tool_uses.is_empty() {
            let tool_results = self.execute_tools(&tool_uses).await?;

            // Add tool result message
            let tool_result_blocks: Vec<ContentBlock> = tool_results
                .into_iter()
                .map(|(tool_use_id, result)| ContentBlock::ToolResult {
                    tool_use_id,
                    content: result,
                    is_error: false,
                })
                .collect();

            self.conversation.push(Message {
                role: Role::User,
                content: tool_result_blocks,
                tool_use_id: None,
                timestamp: Utc::now(),
            });

            self.emit_event(AgentEvent::TurnEnd {
                iteration: self.iteration,
                tokens_used: turn_tokens,
            })
            .await;

            // Continue the loop if tools were executed
            Ok(true)
        } else {
            self.emit_event(AgentEvent::TurnEnd {
                iteration: self.iteration,
                tokens_used: turn_tokens,
            })
            .await;

            // No tools executed, conversation is complete
            Ok(false)
        }
    }

    /// Execute tools with timeout and cancellation support
    async fn execute_tools(&mut self, tool_uses: &[(String, String, serde_json::Value)]) -> Result<Vec<(String, String)>> {
        let mut results = Vec::new();

        for (tool_id, tool_name, input) in tool_uses {
            let tool_id_clone = tool_id.clone();
            let tool_name_clone = tool_name.clone();

            self.emit_event(AgentEvent::ToolStart {
                tool_name: tool_name_clone.clone(),
                tool_id: tool_id_clone.clone(),
            })
            .await;

            let start = std::time::Instant::now();

            // Execute with timeout and abort handle for cancellation
            let tool_registry = self.tool_registry.clone();
            let params = input.clone();
            let tool_name_for_exec = tool_name.clone();

            let execution_future = async move {
                tool_registry.execute(&tool_name_for_exec, params).await
            };

            let result = match timeout(self.config.tool_timeout, execution_future).await {
                Ok(Ok(tool_result)) => {
                    serde_json::to_string(&tool_result).unwrap_or_else(|_| {
                        format!("{{\"error\": \"Failed to serialize tool result\"}}")
                    })
                }
                Ok(Err(e)) => {
                    warn!("Tool {} failed: {:?}", tool_name_clone, e);
                    format!("{{\"error\": \"{}\"}}", e)
                }
                Err(_) => {
                    warn!("Tool {} timed out", tool_name_clone);
                    format!("{{\"error\": \"Tool execution timed out\"}}")
                }
            };

            let duration = start.elapsed();

            self.emit_event(AgentEvent::ToolEnd {
                tool_name: tool_name_clone,
                tool_id: tool_id_clone.clone(),
                result: serde_json::from_str(&result).unwrap_or(serde_json::json!({})),
                duration_ms: duration.as_millis() as u64,
            })
            .await;

            results.push((tool_id_clone, result));
        }

        Ok(results)
    }

    /// Detect hallucinations (action phrases without corresponding tool calls)
    fn detect_hallucinations(&mut self, text: &str) {
        let hallucination_phrases = [
            "I will",
            "I'll",
            "Let me",
            "I'm going to",
            "I am going to",
            "I'm now",
            "I am now",
        ];

        for phrase in &hallucination_phrases {
            if text.contains(phrase) {
                warn!("Possible hallucination detected: '{}'", phrase);
                let _ = self.event_tx.send(AgentEvent::HallucinationDetected {
                    phrase: phrase.to_string(),
                });
            }
        }
    }

    /// Check if conversation should be compacted
    fn should_compact(&self) -> bool {
        // Rough token estimation: 1 token ≈ 4 characters
        let estimated_tokens: usize = self
            .conversation
            .iter()
            .map(|msg| {
                msg.content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => text.len() / 4,
                        ContentBlock::ToolUse { .. } => 100, // Rough estimate
                        ContentBlock::ToolResult { content, .. } => content.len() / 4,
                        ContentBlock::Image { .. } => 200, // Rough estimate
                        ContentBlock::Document { .. } => 2000, // PDF pages ~2k tokens
                    })
                    .sum::<usize>()
            })
            .sum();

        estimated_tokens > self.config.compaction_threshold
    }

    /// Compact the conversation by summarizing earlier messages
    async fn compact_conversation(&mut self) -> Result<()> {
        let original_tokens = self.estimate_tokens();

        // Keep the last 5 messages, summarize the rest
        if self.conversation.len() > 5 {
            let to_summarize = &self.conversation[..self.conversation.len() - 5];

            // Build summary prompt
            let summary_text = to_summarize
                .iter()
                .map(|msg| {
                    let role = format!("{:?}", msg.role);
                    let content = msg
                        .content
                        .iter()
                        .map(|block| match block {
                            ContentBlock::Text { text } => text.clone(),
                            _ => "[tool interaction]".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("{}: {}", role, content)
                })
                .collect::<Vec<_>>()
                .join("\n");

            let summary = format!(
                "Previous conversation summary:\n{}\n\n[Conversation continues below]",
                summary_text
            );

            // Replace old messages with summary
            let mut new_conversation = vec![Message {
                role: Role::User,
                content: vec![ContentBlock::Text { text: summary }],
                tool_use_id: None,
                timestamp: Utc::now(),
            }];

            new_conversation.extend_from_slice(&self.conversation[self.conversation.len() - 5..]);
            self.conversation = new_conversation;

            let new_tokens = self.estimate_tokens();

            self.emit_event(AgentEvent::ConversationCompacted {
                original_tokens,
                new_tokens,
            })
            .await;

            info!(
                "Compacted conversation from {} to {} tokens",
                original_tokens, new_tokens
            );
        }

        Ok(())
    }

    /// Estimate total tokens in conversation
    fn estimate_tokens(&self) -> usize {
        self.conversation
            .iter()
            .map(|msg| {
                msg.content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => text.len() / 4,
                        ContentBlock::ToolUse { .. } => 100,
                        ContentBlock::ToolResult { content, .. } => content.len() / 4,
                        ContentBlock::Image { .. } => 200,
                        ContentBlock::Document { .. } => 2000, // PDF pages ~2k tokens
                    })
                    .sum::<usize>()
            })
            .sum()
    }

    /// Emit an event to all subscribers
    async fn emit_event(&self, event: AgentEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Add a steering message to interrupt current flow
    pub fn steer(&mut self, message: String) {
        self.conversation.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: message }],
            tool_use_id: None,
            timestamp: Utc::now(),
        });
    }

    /// Get the current conversation history
    pub fn get_conversation(&self) -> &[Message] {
        &self.conversation
    }

    /// Pre-seed the conversation with prior history (for session continuity).
    /// Call this *before* `run()` to restore a previous session.
    pub fn set_conversation(&mut self, messages: Vec<Message>) {
        self.conversation = messages;
    }
}
