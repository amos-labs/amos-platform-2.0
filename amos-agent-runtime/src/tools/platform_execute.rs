use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, info};

use super::Tool;
use amos_core::error::{AmosError, Result};
use amos_core::types::ToolDefinition;

/// Tool for executing platform actions (send emails, trigger workflows, publish pages, etc.)
/// Delegates to the Rails API endpoint during hybrid mode.
pub struct PlatformExecuteTool {
    rails_base_url: String,
    http_client: reqwest::Client,
}

impl PlatformExecuteTool {
    /// Create a new PlatformExecuteTool instance
    pub fn new(http_client: reqwest::Client) -> Self {
        Self {
            rails_base_url: "http://localhost:5001".to_string(),
            http_client,
        }
    }

    /// Create a new PlatformExecuteTool with a custom Rails base URL
    pub fn with_base_url(http_client: reqwest::Client, rails_base_url: String) -> Self {
        Self {
            rails_base_url,
            http_client,
        }
    }

    /// Format the Rails API URL for the execute endpoint
    fn rails_url(&self) -> String {
        format!("{}/api/v1/agent/execute", self.rails_base_url)
    }
}

#[async_trait]
impl Tool for PlatformExecuteTool {
    fn name(&self) -> &str {
        "platform_execute"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "platform_execute".to_string(),
            description: "Execute an action on the AMOS platform. Send emails, trigger workflows, publish landing pages, run integration operations, generate files/images, and more.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "The action to execute (e.g., 'send_email', 'trigger_workflow', 'publish_landing_page', 'run_integration', 'generate_file', 'generate_image', 'export_data', 'import_data', 'schedule_task', 'send_notification', 'archive_object', 'duplicate_object')"
                    },
                    "params": {
                        "type": "object",
                        "description": "Parameters for the action. Structure varies by action type."
                    }
                },
                "required": ["action", "params"]
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: &Value) -> Result<String> {
        info!("Executing platform_execute tool");
        debug!("Input: {}", input);

        // Validate input structure
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AmosError::ToolExecutionFailed {
                    tool: "platform_execute".into(),
                    reason: "Missing required field: action".into(),
                }
            })?;

        let params = input.get("params").ok_or_else(|| {
            AmosError::ToolExecutionFailed {
                tool: "platform_execute".into(),
                reason: "Missing required field: params".into(),
            }
        })?;

        debug!(
            "Executing action '{}' with params: {}",
            action, params
        );

        // Build request body
        let request_body = json!({
            "action": action,
            "params": params
        });

        // Make HTTP request to Rails API
        let url = self.rails_url();
        debug!("Sending POST request to: {}", url);

        let response = self
            .http_client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                error!("HTTP request failed: {}", e);
                AmosError::ToolExecutionFailed {
                    tool: "platform_execute".into(),
                    reason: format!("Failed to connect to Rails API: {}", e),
                }
            })?;

        let status = response.status();
        debug!("Received response with status: {}", status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| {
                "Failed to read error response".to_string()
            });
            error!("Rails API returned error ({}): {}", status, error_text);
            return Err(AmosError::ToolExecutionFailed {
                tool: "platform_execute".into(),
                reason: format!("Rails API returned error ({}): {}", status, error_text),
            });
        }

        // Parse response
        let result = response.json::<Value>().await.map_err(|e| {
            error!("Failed to parse response JSON: {}", e);
            AmosError::ToolExecutionFailed {
                tool: "platform_execute".into(),
                reason: format!("Failed to parse response: {}", e),
            }
        })?;

        info!("Successfully executed action '{}'", action);
        debug!("Result: {}", result);

        Ok(result.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let client = reqwest::Client::new();
        let tool = PlatformExecuteTool::new(client);
        let def = tool.definition();

        assert_eq!(def.name, "platform_execute");
        assert!(def.description.contains("Execute an action"));
        assert_eq!(def.input_schema["type"], "object");
        assert!(def.input_schema["required"].as_array().unwrap().contains(&json!("action")));
        assert!(def.input_schema["required"].as_array().unwrap().contains(&json!("params")));
    }

    #[test]
    fn test_rails_url_formatting() {
        let client = reqwest::Client::new();
        let tool = PlatformExecuteTool::new(client);
        assert_eq!(tool.rails_url(), "http://localhost:5001/api/v1/agent/execute");

        let client = reqwest::Client::new();
        let tool = PlatformExecuteTool::with_base_url(client, "https://api.example.com".to_string());
        assert_eq!(tool.rails_url(), "https://api.example.com/api/v1/agent/execute");
    }
}
