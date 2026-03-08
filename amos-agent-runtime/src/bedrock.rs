//! # AWS Bedrock Client
//!
//! Async client for invoking LLMs via AWS Bedrock Converse API.
//! Supports streaming, tool use orchestration, and prompt caching.

use amos_core::error::{AmosError, Result};
use amos_core::types::{ContentBlock, Message, ModelInfo, Role, ToolDefinition};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// A streaming chunk from the model.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Partial text output.
    TextDelta(String),
    /// Model wants to use a tool.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Model finished generating.
    Stop {
        stop_reason: StopReason,
        usage: TokenUsage,
    },
    /// Error during streaming.
    Error(String),
}

/// Why the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
}

/// Token usage for a single invocation.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cache_read_tokens: usize,
    pub cache_write_tokens: usize,
}

/// Configuration for a Bedrock invocation.
#[derive(Debug, Clone)]
pub struct InvokeConfig {
    pub model_id: String,
    pub messages: Vec<Message>,
    pub system_prompt: Option<String>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,
    /// Enable prompt caching for system prompt and tools.
    pub prompt_caching: bool,
}

impl Default for InvokeConfig {
    fn default() -> Self {
        Self {
            model_id: String::new(),
            messages: Vec::new(),
            system_prompt: None,
            tools: Vec::new(),
            max_tokens: 4096,
            temperature: 0.7,
            top_p: 0.95,
            prompt_caching: true,
        }
    }
}

/// Bedrock client wrapping the AWS Converse API.
///
/// In production this uses the `aws-sdk-bedrockruntime` crate.
/// The interface is abstracted here so tests can mock it.
pub struct BedrockClient {
    region: String,
    http_client: Client,
    /// Cumulative token usage across the session.
    total_usage: parking_lot::Mutex<TokenUsage>,
}

impl BedrockClient {
    /// Create a new client for the given AWS region.
    pub fn new(region: &str) -> Self {
        Self {
            region: region.to_string(),
            http_client: Client::new(),
            total_usage: parking_lot::Mutex::new(TokenUsage::default()),
        }
    }

    /// Invoke the model and stream results back through an mpsc channel.
    ///
    /// This is the primary interface used by the agent loop.
    /// The channel receives [`StreamEvent`] items until the model stops.
    pub async fn converse_stream(
        &self,
        config: InvokeConfig,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let (tx, rx) = mpsc::channel(256);

        let region = self.region.clone();
        let _http = self.http_client.clone();

        tokio::spawn(async move {
            // ─── Build Bedrock Converse request ─────────────────────
            //
            // In production, this constructs the full AWS SigV4-signed
            // request to the Bedrock Runtime ConverseStream endpoint.
            //
            // For this skeleton, we emit a placeholder response.
            // The real implementation will use:
            //   aws_sdk_bedrockruntime::Client::converse_stream()
            //
            // Key features to implement:
            // 1. Message format conversion (our types → Bedrock format)
            // 2. Tool schema injection
            // 3. System prompt with cache control blocks
            // 4. SSE stream parsing into StreamEvent
            // 5. Clock skew / SigV4 retry handling
            // 6. Cross-region inference support
            // ────────────────────────────────────────────────────────

            debug!(
                model = %config.model_id,
                messages = config.messages.len(),
                tools = config.tools.len(),
                region = %region,
                "Starting Bedrock converse stream"
            );

            // TODO: Replace with real Bedrock SDK call.
            // This placeholder sends a single text response.
            let _ = tx
                .send(StreamEvent::TextDelta(
                    "I'm the AMOS agent. How can I help you today?".into(),
                ))
                .await;

            let _ = tx
                .send(StreamEvent::Stop {
                    stop_reason: StopReason::EndTurn,
                    usage: TokenUsage {
                        input_tokens: config
                            .messages
                            .iter()
                            .map(|m| estimate_tokens(&m.content))
                            .sum(),
                        output_tokens: 15,
                        cache_read_tokens: 0,
                        cache_write_tokens: 0,
                    },
                })
                .await;
        });

        Ok(rx)
    }

    /// Non-streaming invocation. Collects the full response.
    pub async fn converse(&self, config: InvokeConfig) -> Result<ConversationResponse> {
        let mut rx = self.converse_stream(config).await?;
        let mut text = String::new();
        let mut tool_uses = Vec::new();
        let mut usage = TokenUsage::default();

        while let Some(event) = rx.recv().await {
            match event {
                StreamEvent::TextDelta(delta) => text.push_str(&delta),
                StreamEvent::ToolUse { id, name, input } => {
                    tool_uses.push(ToolUseRequest { id, name, input });
                }
                StreamEvent::Stop {
                    stop_reason,
                    usage: u,
                } => {
                    usage = u;
                    break;
                }
                StreamEvent::Error(e) => {
                    return Err(AmosError::ModelInvocationFailed {
                        model: "unknown".into(),
                        reason: e,
                    });
                }
            }
        }

        // Track cumulative usage
        {
            let mut total = self.total_usage.lock();
            total.input_tokens += usage.input_tokens;
            total.output_tokens += usage.output_tokens;
        }

        Ok(ConversationResponse {
            text,
            tool_uses,
            usage,
        })
    }

    /// Get cumulative token usage for this session.
    pub fn total_usage(&self) -> TokenUsage {
        self.total_usage.lock().clone()
    }
}

/// Full response from a non-streaming converse call.
#[derive(Debug)]
pub struct ConversationResponse {
    pub text: String,
    pub tool_uses: Vec<ToolUseRequest>,
    pub usage: TokenUsage,
}

/// A tool use request from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseRequest {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// Rough token estimate for content blocks.
fn estimate_tokens(content: &[ContentBlock]) -> usize {
    content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => text.len() / 4,
            ContentBlock::ToolUse { input, .. } => input.to_string().len() / 4,
            ContentBlock::ToolResult { content, .. } => content.len() / 4,
            ContentBlock::Image { .. } => 1000, // rough estimate for images
        })
        .sum()
}
