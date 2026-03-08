//! # V3 Agent Loop
//!
//! Pi-inspired single-agent loop: trust the model, give it tools, let it work.
//!
//! ```text
//! Loop:
//!   1. Stream LLM response (text + tool calls)
//!   2. Execute tool calls (in order)
//!   3. Append tool results to conversation
//!   4. Detect escalation / hallucination
//!   5. Repeat until end_turn or max iterations
//! ```
//!
//! Key features ported from the Rails V3::AgentLoop:
//! - Model escalation (cheap → Sonnet → Opus) on failure/complexity
//! - Hallucination detection and grounding nudges
//! - Steering message support (user can interrupt mid-work)
//! - Research check-ins to prevent aimless cycling
//! - Smart tool failure handling with context
//! - Result truncation (50 KB max per tool result)
//! - Conversation compaction when context gets large

use crate::bedrock::{BedrockClient, InvokeConfig, StopReason, StreamEvent, TokenUsage};
use crate::model_registry::ModelRegistry;
use crate::prompt_builder::{self, PlatformSummary, UserContext};
use crate::tools::ToolRegistry;
use amos_core::error::{AmosError, Result};
use amos_core::types::{
    ContentBlock, EscalationReason, Message, Role, ToolDefinition, ToolResult,
};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Maximum size of a single tool result before truncation.
const MAX_TOOL_RESULT_BYTES: usize = 50 * 1024;

/// Maximum consecutive empty responses before escalation.
const MAX_EMPTY_RESPONSES: usize = 2;

/// Maximum consecutive tool loops (same tool, same args) before escalation.
const MAX_TOOL_LOOPS: usize = 3;

/// Callback for streaming events to the caller (e.g. SSE to frontend).
pub type StreamCallback = Box<dyn Fn(AgentEvent) + Send + Sync>;

/// Events emitted by the agent loop for external consumers.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Partial text from the model.
    TextDelta(String),
    /// A tool is about to be executed.
    ToolStart { name: String, id: String },
    /// A tool finished executing.
    ToolEnd { name: String, id: String, duration_ms: u64 },
    /// The model escalated to a more capable tier.
    ModelEscalated { from: String, to: String, reason: EscalationReason },
    /// The loop completed.
    Done { iterations: usize, usage: TokenUsage },
    /// An error occurred but the loop continues.
    Warning(String),
}

/// Configuration for the agent loop.
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    /// Maximum loop iterations before forced stop.
    pub max_iterations: usize,
    /// Maximum total tokens before conversation compaction.
    pub max_context_tokens: usize,
    /// Starting model ID (usually the cheapest).
    pub starting_model: String,
    /// Temperature for generation.
    pub temperature: f32,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 25,
            max_context_tokens: 200_000,
            starting_model: "us.anthropic.claude-3-5-haiku-20241022-v1:0".into(),
            temperature: 0.7,
        }
    }
}

/// The V3 single-agent loop.
///
/// Owns the conversation state, tool registry, and Bedrock client.
/// Call [`run`] to execute the loop for a user message.
pub struct AgentLoop {
    config: AgentLoopConfig,
    bedrock: Arc<BedrockClient>,
    model_registry: Arc<ModelRegistry>,
    tool_registry: Arc<ToolRegistry>,
    /// Conversation history.
    messages: Vec<Message>,
    /// System prompt (cached, rebuilt on context change).
    system_prompt: String,
    /// Current model being used (may escalate during the loop).
    current_model: String,
    /// Event callback for streaming to the frontend.
    on_event: Option<StreamCallback>,
}

impl AgentLoop {
    /// Create a new agent loop with the given configuration.
    pub fn new(
        config: AgentLoopConfig,
        bedrock: Arc<BedrockClient>,
        model_registry: Arc<ModelRegistry>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        let current_model = config.starting_model.clone();
        Self {
            config,
            bedrock,
            model_registry,
            tool_registry,
            messages: Vec::new(),
            system_prompt: String::new(),
            current_model,
            on_event: None,
        }
    }

    /// Set the callback for streaming events.
    pub fn on_event(&mut self, callback: StreamCallback) {
        self.on_event = Some(callback);
    }

    /// Build and cache the system prompt.
    pub fn set_context(
        &mut self,
        user_ctx: &UserContext,
        platform_summary: &PlatformSummary,
        canvas_context: Option<&str>,
        learned_behaviors: &[String],
    ) {
        self.system_prompt = prompt_builder::build_system_prompt(
            user_ctx,
            platform_summary,
            canvas_context,
            learned_behaviors,
        );
    }

    /// Run the agent loop for a user message.
    ///
    /// Returns the final assistant text response.
    pub async fn run(&mut self, user_message: &str) -> Result<String> {
        // Add user message to conversation
        self.messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: user_message.to_string(),
            }],
            tool_use_id: None,
            timestamp: Utc::now(),
        });

        let mut iteration = 0;
        let mut consecutive_empty = 0;
        let mut last_tool_call: Option<(String, String)> = None;
        let mut tool_loop_count = 0;
        let mut final_text = String::new();

        // ── Pre-routing: detect complex requests that need Opus ─────
        if self.should_preroute(user_message) {
            self.escalate_model(EscalationReason::ComplexRequestNoTools)?;
        }

        loop {
            iteration += 1;
            if iteration > self.config.max_iterations {
                warn!(iterations = iteration, "Agent loop exceeded max iterations");
                return Err(AmosError::AgentLoopExceeded {
                    max: self.config.max_iterations,
                });
            }

            // ── Compact conversation if context is too large ────────
            let estimated_tokens: usize = self
                .messages
                .iter()
                .map(|m| estimate_message_tokens(m))
                .sum();
            if estimated_tokens > self.config.max_context_tokens * 80 / 100 {
                self.compact_conversation();
            }

            // ── Invoke the model ────────────────────────────────────
            let invoke_config = InvokeConfig {
                model_id: self.current_model.clone(),
                messages: self.messages.clone(),
                system_prompt: Some(self.system_prompt.clone()),
                tools: self.tool_registry.tool_definitions(),
                max_tokens: self
                    .model_registry
                    .get(&self.current_model)
                    .map(|m| m.max_output_tokens)
                    .unwrap_or(4096),
                temperature: self.config.temperature,
                top_p: 0.95,
                prompt_caching: true,
            };

            let mut rx = self.bedrock.converse_stream(invoke_config).await?;

            // ── Process stream ──────────────────────────────────────
            let mut response_text = String::new();
            let mut tool_uses = Vec::new();
            let mut usage = TokenUsage::default();

            while let Some(event) = rx.recv().await {
                match event {
                    StreamEvent::TextDelta(delta) => {
                        response_text.push_str(&delta);
                        self.emit(AgentEvent::TextDelta(delta));
                    }
                    StreamEvent::ToolUse { id, name, input } => {
                        tool_uses.push((id, name, input));
                    }
                    StreamEvent::Stop {
                        stop_reason,
                        usage: u,
                    } => {
                        usage = u;
                        break;
                    }
                    StreamEvent::Error(e) => {
                        warn!(error = %e, "Stream error, attempting escalation");
                        self.escalate_model(EscalationReason::EmptyResponse)?;
                        continue;
                    }
                }
            }

            // ── Empty response detection ────────────────────────────
            if response_text.trim().is_empty() && tool_uses.is_empty() {
                consecutive_empty += 1;
                if consecutive_empty >= MAX_EMPTY_RESPONSES {
                    self.escalate_model(EscalationReason::EmptyResponse)?;
                    consecutive_empty = 0;
                }
                continue;
            }
            consecutive_empty = 0;

            // ── Hallucination detection ─────────────────────────────
            if self.detect_hallucination(&response_text, &tool_uses) {
                self.messages.push(Message {
                    role: Role::User,
                    content: vec![ContentBlock::Text {
                        text: "[System] You claimed to perform an action but didn't use \
                               any tools. Please use the appropriate tool to actually \
                               perform the action, or clarify what you need."
                            .into(),
                    }],
                    tool_use_id: None,
                    timestamp: Utc::now(),
                });
                self.escalate_model(EscalationReason::HallucinationGuard)?;
                continue;
            }

            // ── Add assistant response to history ───────────────────
            let mut assistant_content = Vec::new();
            if !response_text.is_empty() {
                assistant_content.push(ContentBlock::Text {
                    text: response_text.clone(),
                });
            }
            for (id, name, input) in &tool_uses {
                assistant_content.push(ContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                });
            }
            self.messages.push(Message {
                role: Role::Assistant,
                content: assistant_content,
                tool_use_id: None,
                timestamp: Utc::now(),
            });

            // ── Execute tool calls ──────────────────────────────────
            if !tool_uses.is_empty() {
                for (id, name, input) in &tool_uses {
                    // Tool loop detection
                    let call_sig = (name.clone(), input.to_string());
                    if last_tool_call.as_ref() == Some(&call_sig) {
                        tool_loop_count += 1;
                        if tool_loop_count >= MAX_TOOL_LOOPS {
                            warn!(tool = %name, "Tool loop detected, escalating");
                            self.escalate_model(EscalationReason::ToolLoop)?;
                            tool_loop_count = 0;
                        }
                    } else {
                        tool_loop_count = 0;
                    }
                    last_tool_call = Some(call_sig);

                    self.emit(AgentEvent::ToolStart {
                        name: name.clone(),
                        id: id.clone(),
                    });

                    let start = std::time::Instant::now();
                    let result = self.tool_registry.execute(name, input).await;
                    let duration_ms = start.elapsed().as_millis() as u64;

                    self.emit(AgentEvent::ToolEnd {
                        name: name.clone(),
                        id: id.clone(),
                        duration_ms,
                    });

                    let tool_result = match result {
                        Ok(output) => {
                            let truncated = truncate_result(&output, MAX_TOOL_RESULT_BYTES);
                            ToolResult {
                                tool_use_id: id.clone(),
                                content: truncated,
                                is_error: false,
                                duration_ms,
                            }
                        }
                        Err(e) => {
                            warn!(tool = %name, error = %e, "Tool execution failed");
                            ToolResult {
                                tool_use_id: id.clone(),
                                content: format!("Error: {e}"),
                                is_error: true,
                                duration_ms,
                            }
                        }
                    };

                    // Add tool result to conversation
                    self.messages.push(Message {
                        role: Role::Tool,
                        content: vec![ContentBlock::ToolResult {
                            tool_use_id: tool_result.tool_use_id.clone(),
                            content: tool_result.content,
                            is_error: tool_result.is_error,
                        }],
                        tool_use_id: Some(id.clone()),
                        timestamp: Utc::now(),
                    });
                }

                // After tool execution, loop back for model's next response
                continue;
            }

            // ── No tool calls → model is done ───────────────────────
            final_text = response_text;
            self.emit(AgentEvent::Done {
                iterations: iteration,
                usage: self.bedrock.total_usage(),
            });
            break;
        }

        Ok(final_text)
    }

    /// Inject a steering message (user interrupt mid-work).
    pub fn steer(&mut self, message: &str) {
        self.messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: message.to_string(),
            }],
            tool_use_id: None,
            timestamp: Utc::now(),
        });
    }

    /// Detect if the model is hallucinating actions without tool use.
    ///
    /// Patterns: "I've created", "I've updated", "Done!", "Here's the result"
    /// when no tools were actually called.
    fn detect_hallucination(&self, text: &str, tool_uses: &[(String, String, serde_json::Value)]) -> bool {
        if !tool_uses.is_empty() {
            return false;
        }

        let action_phrases = [
            "i've created",
            "i've updated",
            "i've deleted",
            "i've sent",
            "i have created",
            "i have updated",
            "successfully created",
            "successfully updated",
            "successfully sent",
            "here's the result",
            "i've set up",
            "done!",
        ];

        let lower = text.to_lowercase();
        action_phrases.iter().any(|phrase| lower.contains(phrase))
    }

    /// Detect complex requests that should skip cheap models.
    fn should_preroute(&self, message: &str) -> bool {
        let lower = message.to_lowercase();
        let complex_patterns = [
            "analyze",
            "strategy",
            "comprehensive",
            "deep dive",
            "architecture",
            "refactor",
            "rewrite",
            "multi-step",
        ];
        let word_count = message.split_whitespace().count();

        // Long messages or complex keywords → preroute to capable model
        word_count > 200 || complex_patterns.iter().any(|p| lower.contains(p))
    }

    /// Escalate to the next model tier.
    fn escalate_model(&mut self, reason: EscalationReason) -> Result<()> {
        if let Some(next) = self.model_registry.escalate(&self.current_model) {
            let from = self.current_model.clone();
            let to = next.id.clone();
            info!(from = %from, to = %to, reason = ?reason, "Model escalation");
            self.current_model = to.clone();
            self.emit(AgentEvent::ModelEscalated {
                from,
                to,
                reason,
            });
            Ok(())
        } else {
            warn!(model = %self.current_model, reason = ?reason, "Already at top tier");
            // Don't error — just continue with current model
            Ok(())
        }
    }

    /// Compact the conversation by summarizing older messages.
    fn compact_conversation(&mut self) {
        if self.messages.len() <= 4 {
            return;
        }

        // Keep the first message (original request) and last 4 messages.
        // Summarize everything in between.
        let keep_start = 1;
        let keep_end = self.messages.len().saturating_sub(4);

        if keep_end <= keep_start {
            return;
        }

        let compacted_count = keep_end - keep_start;
        let summary = format!(
            "[System: {compacted_count} earlier messages compacted to save context. \
             The conversation covered tool executions and intermediate results.]"
        );

        let mut new_messages = Vec::new();
        new_messages.push(self.messages[0].clone()); // original user message
        new_messages.push(Message {
            role: Role::User,
            content: vec![ContentBlock::Text { text: summary }],
            tool_use_id: None,
            timestamp: Utc::now(),
        });
        new_messages.extend(self.messages[keep_end..].iter().cloned());

        info!(
            before = self.messages.len(),
            after = new_messages.len(),
            "Conversation compacted"
        );

        self.messages = new_messages;
    }

    /// Emit an event through the callback.
    fn emit(&self, event: AgentEvent) {
        if let Some(cb) = &self.on_event {
            cb(event);
        }
    }

    /// Get the current conversation history (for debugging).
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get the current model being used.
    pub fn current_model(&self) -> &str {
        &self.current_model
    }

    /// Reset the conversation.
    pub fn reset(&mut self) {
        self.messages.clear();
        self.current_model = self.config.starting_model.clone();
    }
}

/// Truncate a tool result to the maximum size.
fn truncate_result(content: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        content.to_string()
    } else {
        let truncated = &content[..max_bytes];
        format!(
            "{truncated}\n\n[... truncated, showing {max_bytes} of {} bytes]",
            content.len()
        )
    }
}

/// Rough token estimate for a message.
fn estimate_message_tokens(msg: &Message) -> usize {
    msg.content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => text.len() / 4,
            ContentBlock::ToolUse { input, .. } => input.to_string().len() / 4,
            ContentBlock::ToolResult { content, .. } => content.len() / 4,
            ContentBlock::Image { .. } => 1000,
        })
        .sum()
}
