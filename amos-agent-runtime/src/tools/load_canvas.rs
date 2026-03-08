use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, info};

use super::Tool;
use amos_core::error::{AmosError, Result};
use amos_core::types::ToolDefinition;

/// Tool for displaying visual content to the user in a canvas panel
pub struct LoadCanvasTool {
    http_client: reqwest::Client,
    rails_base_url: String,
}

impl LoadCanvasTool {
    /// Create a new LoadCanvasTool instance
    pub fn new(http_client: reqwest::Client) -> Self {
        Self {
            http_client,
            rails_base_url: "http://localhost:5001".to_string(),
        }
    }

    /// Set the Rails base URL
    pub fn with_rails_url(mut self, url: String) -> Self {
        self.rails_base_url = url;
        self
    }

    /// Format content for standalone mode display
    fn format_standalone_output(&self, canvas_type: &str, content: &str, title: Option<&str>, language: Option<&str>) -> String {
        let mut output = String::new();

        if let Some(title_text) = title {
            output.push_str(&format!("=== {} ===\n\n", title_text));
        }

        output.push_str(&format!("Canvas Type: {}\n", canvas_type));

        if let Some(lang) = language {
            output.push_str(&format!("Language: {}\n", lang));
        }

        output.push_str("\n--- Content ---\n");
        output.push_str(content);
        output.push_str("\n--- End Content ---\n");

        output
    }
}

#[async_trait]
impl Tool for LoadCanvasTool {
    fn name(&self) -> &str {
        "load_canvas"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: "Display visual content to the user in a canvas panel. Use this for reports, charts, tables, HTML previews, code, markdown, and any content that benefits from visual display.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "canvas_type": {
                        "type": "string",
                        "enum": ["html", "markdown", "code", "table", "chart", "image"],
                        "description": "The type of content being displayed"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to display in the canvas"
                    },
                    "title": {
                        "type": "string",
                        "description": "Optional title for the canvas panel"
                    },
                    "language": {
                        "type": "string",
                        "description": "Optional language for code syntax highlighting (only used when canvas_type is 'code')"
                    }
                },
                "required": ["canvas_type", "content"]
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: &Value) -> Result<String> {
        debug!("LoadCanvasTool executing with input: {:?}", input);

        // Extract and validate input parameters
        let canvas_type = input["canvas_type"]
            .as_str()
            .ok_or_else(|| AmosError::ToolExecutionFailed {
                tool: "load_canvas".into(),
                reason: "canvas_type is required".into(),
            })?;

        let content = input["content"]
            .as_str()
            .ok_or_else(|| AmosError::ToolExecutionFailed {
                tool: "load_canvas".into(),
                reason: "content is required".into(),
            })?;

        let title = input["title"].as_str();
        let language = input["language"].as_str();

        // Validate canvas_type
        let valid_types = ["html", "markdown", "code", "table", "chart", "image"];
        if !valid_types.contains(&canvas_type) {
            return Err(AmosError::ToolExecutionFailed {
                tool: "load_canvas".into(),
                reason: format!("canvas_type must be one of: {}", valid_types.join(", ")),
            });
        }

        info!(
            "Loading canvas: type={}, title={:?}, content_length={}",
            canvas_type,
            title,
            content.len()
        );

        // Build the payload
        let mut payload = json!({
            "canvas_type": canvas_type,
            "content": content,
        });

        if let Some(title_text) = title {
            payload["title"] = json!(title_text);
        }

        if let Some(lang) = language {
            payload["language"] = json!(lang);
        }

        // Try to send to Rails API (hybrid mode)
        let api_url = format!("{}/api/v1/agent/canvas", self.rails_base_url);

        match self.http_client
            .post(&api_url)
            .json(&payload)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    info!("Canvas loaded successfully via Rails API");
                    match response.json::<Value>().await {
                        Ok(data) => Ok(data.to_string()),
                        Err(e) => {
                            error!("Failed to parse Rails API response: {}", e);
                            Ok("Canvas loaded successfully".to_string())
                        }
                    }
                } else {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                    error!("Rails API error: {} - {}", status, error_text);

                    // Fall back to standalone mode
                    info!("Falling back to standalone mode");
                    let output = self.format_standalone_output(canvas_type, content, title, language);
                    Ok(output)
                }
            }
            Err(e) => {
                error!("Failed to connect to Rails API: {}", e);
                info!("Running in standalone mode");

                // Standalone mode - return formatted text representation
                let output = self.format_standalone_output(canvas_type, content, title, language);
                Ok(output)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let client = reqwest::Client::new();
        let tool = LoadCanvasTool::new(client);

        assert_eq!(tool.name(), "load_canvas");

        let def = tool.definition();
        assert_eq!(def.name, "load_canvas");
        assert!(def.description.contains("Display visual content"));

        let schema = def.input_schema;
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["canvas_type"].is_object());
        assert!(schema["properties"]["content"].is_object());
        assert!(schema["required"].as_array().unwrap().contains(&json!("canvas_type")));
        assert!(schema["required"].as_array().unwrap().contains(&json!("content")));
    }

    #[tokio::test]
    async fn test_invalid_canvas_type() {
        let client = reqwest::Client::new();
        let tool = LoadCanvasTool::new(client);

        let input = json!({
            "canvas_type": "invalid_type",
            "content": "test content"
        });

        let result = tool.execute(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_missing_required_field() {
        let client = reqwest::Client::new();
        let tool = LoadCanvasTool::new(client);

        let input = json!({
            "canvas_type": "markdown"
            // missing content
        });

        let result = tool.execute(&input).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_format_standalone_output() {
        let client = reqwest::Client::new();
        let tool = LoadCanvasTool::new(client);

        let output = tool.format_standalone_output(
            "code",
            "fn main() {}",
            Some("Test Code"),
            Some("rust")
        );

        assert!(output.contains("=== Test Code ==="));
        assert!(output.contains("Canvas Type: code"));
        assert!(output.contains("Language: rust"));
        assert!(output.contains("fn main() {}"));
    }
}
