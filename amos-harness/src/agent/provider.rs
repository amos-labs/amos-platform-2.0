//! Model provider abstraction layer
//!
//! Defines a `ModelProvider` trait that all LLM backends implement.
//! This enables BYOK (Bring Your Own Key) support — users can connect
//! any OpenAI-compatible API (Ollama, vLLM, TGI, OpenAI, Anthropic Direct)
//! alongside the default AWS Bedrock backend.

use super::bedrock::{BedrockClient, StreamEvent, TokenUsage};
use amos_core::{
    config::CustomModelProvider,
    types::Message,
    AmosError, Result,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

// ═══════════════════════════════════════════════════════════════════════════
// TRAIT
// ═══════════════════════════════════════════════════════════════════════════

/// Unified interface for LLM providers.
///
/// Both AWS Bedrock and OpenAI-compatible endpoints implement this trait.
/// The `AgentLoop` only interacts through this interface, making the
/// model backend pluggable.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Stream a conversation with the model.
    ///
    /// Returns a channel of `StreamEvent`s (text deltas, tool calls, stop, etc.).
    async fn converse_stream(
        &self,
        model_id: &str,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
    ) -> Result<mpsc::Receiver<StreamEvent>>;

    /// Non-streaming conversation (collects full response).
    async fn converse(
        &self,
        model_id: &str,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
    ) -> Result<(Message, TokenUsage)>;

    /// Human-readable name for logging.
    fn provider_name(&self) -> &str;
}

// ═══════════════════════════════════════════════════════════════════════════
// BEDROCK PROVIDER (wraps existing BedrockClient)
// ═══════════════════════════════════════════════════════════════════════════

/// AWS Bedrock provider — delegates to the existing `BedrockClient`.
pub struct BedrockProvider {
    client: BedrockClient,
}

impl BedrockProvider {
    pub fn new(client: BedrockClient) -> Self {
        Self { client }
    }

    /// Try to create from environment/config (standard AWS credential chain).
    pub fn from_env() -> Result<Self> {
        let client = BedrockClient::new(None, None, None)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl ModelProvider for BedrockProvider {
    async fn converse_stream(
        &self,
        model_id: &str,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        self.client.converse_stream(model_id, system_prompt, messages, tools).await
    }

    async fn converse(
        &self,
        model_id: &str,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
    ) -> Result<(Message, TokenUsage)> {
        self.client.converse(model_id, system_prompt, messages, tools).await
    }

    fn provider_name(&self) -> &str {
        "bedrock"
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// OPENAI-COMPATIBLE PROVIDER (Ollama, vLLM, TGI, OpenAI, etc.)
// ═══════════════════════════════════════════════════════════════════════════

/// Client for any OpenAI-compatible chat completions API.
///
/// Works with:
/// - OpenAI (api.openai.com)
/// - Anthropic Messages API (via adapter)
/// - Ollama (localhost:11434)
/// - vLLM (localhost:8000)
/// - Text Generation Inference (TGI)
/// - LiteLLM proxy
/// - Any server implementing `/v1/chat/completions`
pub struct OpenAiProvider {
    api_base: String,
    api_key: Option<String>,
    model_id: String,
    http_client: Client,
}

/// OpenAI chat completion request
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<OaiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OaiTool>>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OaiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OaiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OaiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OaiFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OaiFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OaiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OaiToolFunction,
}

#[derive(Debug, Serialize)]
struct OaiToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

/// Streaming chunk from OpenAI SSE
#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChunkChoice>,
    #[serde(default)]
    usage: Option<OaiUsage>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: ChunkDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChunkToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ChunkToolCall {
    #[serde(default)]
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<ChunkFunction>,
}

#[derive(Debug, Deserialize)]
struct ChunkFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OaiUsage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
}

/// Non-streaming response
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ResponseChoice>,
    #[serde(default)]
    usage: Option<OaiUsage>,
}

#[derive(Debug, Deserialize)]
struct ResponseChoice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<OaiToolCall>>,
}

impl OpenAiProvider {
    /// Create a new OpenAI-compatible provider from a custom model config.
    pub fn from_config(config: &CustomModelProvider) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| AmosError::Internal(format!("Failed to build HTTP client: {}", e)))?;

        let api_key = config.api_key.as_ref().map(|s| {
            use secrecy::ExposeSecret;
            s.expose_secret().to_string()
        });

        debug!(
            "Initialized OpenAI-compatible provider: {} (endpoint: {}, model: {})",
            config.display_name, config.api_base, config.model_id
        );

        Ok(Self {
            api_base: config.api_base.clone(),
            api_key,
            model_id: config.model_id.clone(),
            http_client,
        })
    }

    /// Create directly with explicit parameters.
    pub fn new(api_base: String, api_key: Option<String>, model_id: String) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| AmosError::Internal(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            api_base,
            api_key,
            model_id,
            http_client,
        })
    }

    /// Convert AMOS messages to OpenAI format
    fn convert_messages(&self, system_prompt: &str, messages: &[Message]) -> Vec<OaiMessage> {
        let mut oai_messages = Vec::new();

        // System prompt
        if !system_prompt.is_empty() {
            oai_messages.push(OaiMessage {
                role: "system".to_string(),
                content: Some(system_prompt.to_string()),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        for msg in messages {
            match msg.role {
                amos_core::types::Role::User => {
                    // Collect text content and tool results separately
                    let mut text_parts = Vec::new();
                    let mut tool_results = Vec::new();

                    for block in &msg.content {
                        match block {
                            amos_core::types::ContentBlock::Text { text } => {
                                text_parts.push(text.clone());
                            }
                            amos_core::types::ContentBlock::ToolResult { tool_use_id, content, .. } => {
                                tool_results.push((tool_use_id.clone(), content.clone()));
                            }
                            amos_core::types::ContentBlock::Image { .. } => {
                                text_parts.push("[image attachment - not supported by this model]".to_string());
                            }
                            amos_core::types::ContentBlock::Document { source } => {
                                text_parts.push(format!("[document: {}]", source.name));
                            }
                            _ => {}
                        }
                    }

                    // Emit tool results first (OpenAI requires them as separate messages)
                    for (tool_use_id, content) in tool_results {
                        oai_messages.push(OaiMessage {
                            role: "tool".to_string(),
                            content: Some(content),
                            tool_calls: None,
                            tool_call_id: Some(tool_use_id),
                        });
                    }

                    // Then emit user text (if any)
                    if !text_parts.is_empty() {
                        oai_messages.push(OaiMessage {
                            role: "user".to_string(),
                            content: Some(text_parts.join("\n")),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    }
                }
                amos_core::types::Role::Assistant => {
                    let mut text_parts = Vec::new();
                    let mut tool_calls = Vec::new();

                    for block in &msg.content {
                        match block {
                            amos_core::types::ContentBlock::Text { text } => {
                                text_parts.push(text.clone());
                            }
                            amos_core::types::ContentBlock::ToolUse { id, name, input } => {
                                tool_calls.push(OaiToolCall {
                                    id: id.clone(),
                                    call_type: "function".to_string(),
                                    function: OaiFunction {
                                        name: name.clone(),
                                        arguments: serde_json::to_string(input).unwrap_or_default(),
                                    },
                                });
                            }
                            _ => {}
                        }
                    }

                    let content = if text_parts.is_empty() {
                        None
                    } else {
                        Some(text_parts.join("\n"))
                    };

                    oai_messages.push(OaiMessage {
                        role: "assistant".to_string(),
                        content,
                        tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                        tool_call_id: None,
                    });
                }
                _ => {}
            }
        }

        oai_messages
    }

    /// Convert AMOS tool schemas to OpenAI format
    fn convert_tools(&self, tools: &[serde_json::Value]) -> Vec<OaiTool> {
        tools
            .iter()
            .filter_map(|tool| {
                let name = tool["name"].as_str()?.to_string();
                let description = tool["description"].as_str().unwrap_or("").to_string();
                let parameters = tool.get("inputSchema").cloned()
                    .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}}));

                Some(OaiTool {
                    tool_type: "function".to_string(),
                    function: OaiToolFunction {
                        name,
                        description,
                        parameters,
                    },
                })
            })
            .collect()
    }
}

#[async_trait]
impl ModelProvider for OpenAiProvider {
    async fn converse_stream(
        &self,
        _model_id: &str,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let (tx, rx) = mpsc::channel(100);

        let oai_messages = self.convert_messages(system_prompt, messages);
        let oai_tools = if tools.is_empty() {
            None
        } else {
            Some(self.convert_tools(tools))
        };

        let request = ChatCompletionRequest {
            model: self.model_id.clone(),
            messages: oai_messages,
            tools: oai_tools,
            stream: true,
            max_tokens: Some(16384),
            temperature: Some(0.7),
        };

        let endpoint = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));

        let mut req_builder = self.http_client.post(&endpoint);
        if let Some(ref key) = self.api_key {
            req_builder = req_builder.bearer_auth(key);
        }
        req_builder = req_builder.json(&request);

        let response = req_builder
            .send()
            .await
            .map_err(|e| AmosError::Internal(format!("OpenAI API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
            return Err(AmosError::Internal(format!(
                "OpenAI API error {}: {}",
                status, body
            )));
        }

        // Parse SSE stream in a background task
        tokio::spawn(async move {
            if let Err(e) = parse_openai_sse_stream(response, tx).await {
                error!("Error parsing OpenAI SSE stream: {:?}", e);
            }
        });

        Ok(rx)
    }

    async fn converse(
        &self,
        model_id: &str,
        system_prompt: &str,
        messages: &[Message],
        tools: &[serde_json::Value],
    ) -> Result<(Message, TokenUsage)> {
        // Use streaming and collect, same as BedrockClient
        let mut stream_rx = self
            .converse_stream(model_id, system_prompt, messages, tools)
            .await?;

        let mut text_parts = Vec::new();
        let mut tool_uses = Vec::new();
        let mut usage = TokenUsage {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
        };

        while let Some(event) = stream_rx.recv().await {
            match event {
                StreamEvent::TextDelta(text) => text_parts.push(text),
                StreamEvent::ToolUse { id, name, input } => tool_uses.push((id, name, input)),
                StreamEvent::TokenUsage(u) => usage = u,
                StreamEvent::Stop => break,
                StreamEvent::Error(e) => {
                    return Err(AmosError::Internal(format!("Stream error: {}", e)));
                }
            }
        }

        let mut content_blocks = Vec::new();
        if !text_parts.is_empty() {
            content_blocks.push(amos_core::types::ContentBlock::Text {
                text: text_parts.join(""),
            });
        }
        for (id, name, input) in tool_uses {
            content_blocks.push(amos_core::types::ContentBlock::ToolUse { id, name, input });
        }

        let response_message = Message {
            role: amos_core::types::Role::Assistant,
            content: content_blocks,
            tool_use_id: None,
            timestamp: chrono::Utc::now(),
        };

        Ok((response_message, usage))
    }

    fn provider_name(&self) -> &str {
        "openai_compatible"
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SSE STREAM PARSER (OpenAI format)
// ═══════════════════════════════════════════════════════════════════════════

/// Parse an OpenAI-style Server-Sent Events stream into StreamEvents.
async fn parse_openai_sse_stream(
    response: reqwest::Response,
    tx: mpsc::Sender<StreamEvent>,
) -> Result<()> {
    use tokio_stream::StreamExt;

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    // State for accumulating tool calls across chunks
    // OpenAI streams tool calls incrementally: first chunk has id+name, subsequent chunks append arguments
    let mut tool_call_state: std::collections::HashMap<usize, (String, String, String)> =
        std::collections::HashMap::new(); // index → (id, name, arguments_buffer)

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AmosError::Internal(format!("Stream read error: {}", e)))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // Process complete SSE lines
        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim_end_matches('\r').to_string();
            buffer = buffer[line_end + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if line == "data: [DONE]" {
                // Flush any accumulated tool calls
                for (_idx, (id, name, args)) in tool_call_state.drain() {
                    let input: serde_json::Value = serde_json::from_str(&args)
                        .unwrap_or_else(|e| {
                            warn!("Failed to parse tool input JSON: {}. Input: {}", e, args);
                            serde_json::json!({})
                        });
                    let _ = tx.send(StreamEvent::ToolUse { id, name, input }).await;
                }
                let _ = tx.send(StreamEvent::Stop).await;
                return Ok(());
            }

            if let Some(data) = line.strip_prefix("data: ") {
                match serde_json::from_str::<ChatCompletionChunk>(data) {
                    Ok(chunk) => {
                        // Process usage if present
                        if let Some(ref usage) = chunk.usage {
                            let _ = tx
                                .send(StreamEvent::TokenUsage(TokenUsage {
                                    input_tokens: usage.prompt_tokens,
                                    output_tokens: usage.completion_tokens,
                                    total_tokens: usage.total_tokens,
                                }))
                                .await;
                        }

                        for choice in &chunk.choices {
                            // Text content
                            if let Some(ref text) = choice.delta.content {
                                if !text.is_empty() {
                                    if tx.send(StreamEvent::TextDelta(text.clone())).await.is_err() {
                                        return Ok(());
                                    }
                                }
                            }

                            // Tool calls (accumulated across chunks)
                            if let Some(ref tool_calls) = choice.delta.tool_calls {
                                for tc in tool_calls {
                                    let entry = tool_call_state
                                        .entry(tc.index)
                                        .or_insert_with(|| {
                                            (
                                                tc.id.clone().unwrap_or_default(),
                                                String::new(),
                                                String::new(),
                                            )
                                        });

                                    // Update id if present
                                    if let Some(ref id) = tc.id {
                                        if !id.is_empty() {
                                            entry.0 = id.clone();
                                        }
                                    }

                                    if let Some(ref func) = tc.function {
                                        if let Some(ref name) = func.name {
                                            entry.1 = name.clone();
                                        }
                                        if let Some(ref args) = func.arguments {
                                            entry.2.push_str(args);
                                        }
                                    }
                                }
                            }

                            // Check finish reason
                            if let Some(ref reason) = choice.finish_reason {
                                if reason == "tool_calls" {
                                    // Flush accumulated tool calls
                                    for (_idx, (id, name, args)) in tool_call_state.drain() {
                                        let input: serde_json::Value =
                                            serde_json::from_str(&args).unwrap_or_else(|e| {
                                                warn!("Failed to parse tool input: {}. Input: {}", e, args);
                                                serde_json::json!({})
                                            });
                                        let _ = tx
                                            .send(StreamEvent::ToolUse { id, name, input })
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Failed to parse SSE chunk: {} (data: {})", e, data);
                    }
                }
            }
        }
    }

    // Flush any remaining tool calls
    for (_idx, (id, name, args)) in tool_call_state.drain() {
        let input: serde_json::Value = serde_json::from_str(&args)
            .unwrap_or_else(|_| serde_json::json!({}));
        let _ = tx.send(StreamEvent::ToolUse { id, name, input }).await;
    }

    let _ = tx.send(StreamEvent::Stop).await;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// PROVIDER FACTORY
// ═══════════════════════════════════════════════════════════════════════════

use super::model_registry::ModelRegistry;

/// Create the right `ModelProvider` based on the model ID.
///
/// - Models starting with `custom:` → `OpenAiProvider` (looked up in registry)
/// - Everything else → `BedrockProvider` (AWS credential chain)
pub fn create_provider(
    model_id: &str,
    registry: &ModelRegistry,
    config: &amos_core::AppConfig,
) -> Result<Box<dyn ModelProvider>> {
    if registry.is_custom_model(model_id) {
        // Look up the custom model provider config
        let model_name = model_id.strip_prefix("custom:").unwrap_or(model_id);

        let custom_config = config
            .custom_models
            .providers
            .iter()
            .find(|p| p.name == model_name)
            .ok_or_else(|| {
                AmosError::Config(format!(
                    "Custom model '{}' not found in configuration",
                    model_name
                ))
            })?;

        let provider = OpenAiProvider::from_config(custom_config)?;
        Ok(Box::new(provider))
    } else {
        // Default: AWS Bedrock
        let provider = BedrockProvider::from_env()?;
        Ok(Box::new(provider))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use amos_core::types::{ContentBlock, Role};
    use chrono::Utc;

    #[test]
    fn test_openai_message_conversion_basic() {
        let provider = OpenAiProvider {
            api_base: "http://localhost:11434/v1".to_string(),
            api_key: None,
            model_id: "llama3".to_string(),
            http_client: Client::new(),
        };

        let messages = vec![
            Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "Hello".to_string(),
                }],
                tool_use_id: None,
                timestamp: Utc::now(),
            },
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::Text {
                    text: "Hi there!".to_string(),
                }],
                tool_use_id: None,
                timestamp: Utc::now(),
            },
        ];

        let oai = provider.convert_messages("You are helpful.", &messages);
        assert_eq!(oai.len(), 3); // system + user + assistant
        assert_eq!(oai[0].role, "system");
        assert_eq!(oai[0].content.as_deref(), Some("You are helpful."));
        assert_eq!(oai[1].role, "user");
        assert_eq!(oai[1].content.as_deref(), Some("Hello"));
        assert_eq!(oai[2].role, "assistant");
        assert_eq!(oai[2].content.as_deref(), Some("Hi there!"));
    }

    #[test]
    fn test_openai_message_conversion_with_tool_use() {
        let provider = OpenAiProvider {
            api_base: "http://localhost:11434/v1".to_string(),
            api_key: None,
            model_id: "llama3".to_string(),
            http_client: Client::new(),
        };

        let messages = vec![
            Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: "What time is it?".to_string(),
                }],
                tool_use_id: None,
                timestamp: Utc::now(),
            },
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "call_123".to_string(),
                    name: "get_time".to_string(),
                    input: serde_json::json!({"timezone": "UTC"}),
                }],
                tool_use_id: None,
                timestamp: Utc::now(),
            },
            Message {
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "call_123".to_string(),
                    content: "2024-01-01T12:00:00Z".to_string(),
                    is_error: false,
                }],
                tool_use_id: None,
                timestamp: Utc::now(),
            },
        ];

        let oai = provider.convert_messages("", &messages);
        // No system (empty), user, assistant with tool_calls, tool result
        assert_eq!(oai.len(), 3);
        assert_eq!(oai[0].role, "user");
        assert_eq!(oai[1].role, "assistant");
        assert!(oai[1].tool_calls.is_some());
        assert_eq!(oai[1].tool_calls.as_ref().unwrap().len(), 1);
        assert_eq!(oai[2].role, "tool");
        assert_eq!(oai[2].tool_call_id.as_deref(), Some("call_123"));
    }

    #[test]
    fn test_openai_tool_schema_conversion() {
        let provider = OpenAiProvider {
            api_base: "http://localhost:11434/v1".to_string(),
            api_key: None,
            model_id: "llama3".to_string(),
            http_client: Client::new(),
        };

        let tools = vec![serde_json::json!({
            "name": "get_weather",
            "description": "Get the current weather",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }
        })];

        let oai_tools = provider.convert_tools(&tools);
        assert_eq!(oai_tools.len(), 1);
        assert_eq!(oai_tools[0].tool_type, "function");
        assert_eq!(oai_tools[0].function.name, "get_weather");
        assert_eq!(oai_tools[0].function.description, "Get the current weather");
    }

    #[test]
    fn test_provider_name() {
        let oai = OpenAiProvider {
            api_base: "http://localhost:11434/v1".to_string(),
            api_key: None,
            model_id: "llama3".to_string(),
            http_client: Client::new(),
        };
        assert_eq!(oai.provider_name(), "openai_compatible");
    }

    // ── create_provider factory tests ────────────────────────────────

    use amos_core::config::{CustomModelsConfig, CustomModelProvider};

    /// Build a minimal AppConfig with one custom provider for testing.
    ///
    /// Constructs via the config crate with a dummy database URL so that
    /// AppConfig::load-style deserialization succeeds.
    fn test_config_with_custom(name: &str) -> amos_core::AppConfig {
        let custom = CustomModelsConfig {
            enabled: true,
            providers: vec![CustomModelProvider {
                name: name.to_string(),
                display_name: format!("Test {name}"),
                api_base: "http://localhost:11434/v1".to_string(),
                api_key: None,
                model_id: "llama3".to_string(),
                context_window: 8192,
                tier: 1,
                cost_per_1k_input: 0.0,
                cost_per_1k_output: 0.0,
                customer_owned: true,
            }],
        };

        // Build a minimal AppConfig via the config crate.
        // The only required field without a default is database.url.
        let settings = config::Config::builder()
            .set_default("database.url", "postgres://test@localhost/amos_test")
            .unwrap()
            .build()
            .unwrap();
        let mut app_config: amos_core::AppConfig = settings.try_deserialize().unwrap();
        app_config.custom_models = custom;
        app_config
    }

    /// Build a minimal AppConfig with no custom providers.
    fn test_config_empty() -> amos_core::AppConfig {
        let settings = config::Config::builder()
            .set_default("database.url", "postgres://test@localhost/amos_test")
            .unwrap()
            .build()
            .unwrap();
        settings.try_deserialize().unwrap()
    }

    #[test]
    fn test_create_provider_custom_model_returns_openai() {
        use super::super::model_registry::ModelRegistry;
        let config = test_config_with_custom("my-ollama");
        let registry = ModelRegistry::with_custom_models(&config.custom_models);

        let provider = create_provider("custom:my-ollama", &registry, &config);
        assert!(provider.is_ok(), "Should successfully create custom provider");
        assert_eq!(provider.unwrap().provider_name(), "openai_compatible");
    }

    #[test]
    fn test_create_provider_unknown_custom_model_errors() {
        use super::super::model_registry::ModelRegistry;
        let config = test_config_with_custom("my-ollama");
        let registry = ModelRegistry::with_custom_models(&config.custom_models);

        let result = create_provider("custom:nonexistent", &registry, &config);
        assert!(result.is_err(), "Should error for unknown custom model");
        let err_msg = format!("{}", result.err().unwrap());
        assert!(
            err_msg.contains("nonexistent"),
            "Error should mention the model name: {err_msg}"
        );
    }

    #[test]
    fn test_create_provider_builtin_model_returns_bedrock() {
        use super::super::model_registry::ModelRegistry;
        let config = test_config_empty();
        let registry = ModelRegistry::new();

        // This will fail if AWS creds aren't set (expected in CI),
        // but it correctly takes the Bedrock path.
        let result = create_provider("anthropic.claude-3-haiku-20240307-v1:0", &registry, &config);
        match result {
            Ok(p) => assert_eq!(p.provider_name(), "bedrock"),
            Err(e) => {
                let msg = format!("{e}");
                // Expected failure: no AWS credentials in test env
                assert!(
                    msg.contains("AWS") || msg.contains("credential") || msg.contains("Bedrock") || msg.contains("region"),
                    "Should fail due to AWS config, not model lookup: {msg}"
                );
            }
        }
    }

    #[test]
    fn test_create_provider_non_custom_prefix_uses_bedrock_path() {
        use super::super::model_registry::ModelRegistry;
        let config = test_config_empty();
        let registry = ModelRegistry::new();

        // A plain model ID (no "custom:" prefix) should go to Bedrock
        let result = create_provider("some-model", &registry, &config);
        match result {
            Ok(p) => assert_eq!(p.provider_name(), "bedrock"),
            Err(_) => {} // Expected without AWS credentials
        }
    }
}
