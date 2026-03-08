use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, info};

use super::Tool;
use amos_core::error::{AmosError, Result};
use amos_core::types::ToolDefinition;

/// Tool for creating platform objects (contacts, campaigns, bounties, workflows, etc.)
/// Delegates to the Rails API endpoint during hybrid mode.
pub struct PlatformCreateTool {
    rails_base_url: String,
    http_client: reqwest::Client,
}

impl PlatformCreateTool {
    /// Create a new PlatformCreateTool instance
    pub fn new(http_client: reqwest::Client) -> Self {
        Self {
            rails_base_url: "http://localhost:5001".to_string(),
            http_client,
        }
    }

    /// Create a new PlatformCreateTool with a custom Rails base URL
    pub fn with_base_url(http_client: reqwest::Client, rails_base_url: String) -> Self {
        Self {
            rails_base_url,
            http_client,
        }
    }

    /// Format the Rails API URL for the create endpoint
    fn rails_url(&self) -> String {
        format!("{}/api/v1/agent/create", self.rails_base_url)
    }
}

#[async_trait]
impl Tool for PlatformCreateTool {
    fn name(&self) -> &str {
        "platform_create"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "platform_create".to_string(),
            description: "Create a new object on the AMOS platform. Supports: contact, campaign, bounty, workflow, landing_page, email_template, integration, task, note, tag, custom_field, app_module, and 40+ more types.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "object_type": {
                        "type": "string",
                        "description": "The type of object to create (e.g., 'contact', 'campaign', 'bounty', 'workflow', 'landing_page', 'email_template', 'integration', 'task', 'note', 'tag', 'custom_field', 'app_module')"
                    },
                    "attributes": {
                        "type": "object",
                        "description": "The attributes for the new object. Structure varies by object_type."
                    },
                    "batch": {
                        "type": "array",
                        "description": "Optional: Array of objects to create in batch. Each object should have its own attributes.",
                        "items": {
                            "type": "object"
                        }
                    }
                },
                "required": ["object_type", "attributes"]
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: &Value) -> Result<String> {
        info!("Executing platform_create tool");
        debug!("Input: {}", input);

        // Validate input structure
        let object_type = input
            .get("object_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AmosError::ToolExecutionFailed {
                    tool: "platform_create".into(),
                    reason: "Missing required field: object_type".into(),
                }
            })?;

        let attributes = input.get("attributes").ok_or_else(|| {
            AmosError::ToolExecutionFailed {
                tool: "platform_create".into(),
                reason: "Missing required field: attributes".into(),
            }
        })?;

        debug!(
            "Creating object of type '{}' with attributes: {}",
            object_type, attributes
        );

        // Build request body
        let request_body = if let Some(batch) = input.get("batch") {
            json!({
                "object_type": object_type,
                "attributes": attributes,
                "batch": batch
            })
        } else {
            json!({
                "object_type": object_type,
                "attributes": attributes
            })
        };

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
                    tool: "platform_create".into(),
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
                tool: "platform_create".into(),
                reason: format!("Rails API returned error ({}): {}", status, error_text),
            });
        }

        // Parse response
        let result = response.json::<Value>().await.map_err(|e| {
            error!("Failed to parse response JSON: {}", e);
            AmosError::ToolExecutionFailed {
                tool: "platform_create".into(),
                reason: format!("Failed to parse response: {}", e),
            }
        })?;

        info!("Successfully created object of type '{}'", object_type);
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
        let tool = PlatformCreateTool::new(client);
        let def = tool.definition();

        assert_eq!(def.name, "platform_create");
        assert!(def.description.contains("Create a new object"));
        assert_eq!(def.input_schema["type"], "object");
        assert!(def.input_schema["required"].as_array().unwrap().contains(&json!("object_type")));
        assert!(def.input_schema["required"].as_array().unwrap().contains(&json!("attributes")));
    }

    #[test]
    fn test_rails_url_formatting() {
        let client = reqwest::Client::new();
        let tool = PlatformCreateTool::new(client);
        assert_eq!(tool.rails_url(), "http://localhost:5001/api/v1/agent/create");

        let client = reqwest::Client::new();
        let tool = PlatformCreateTool::with_base_url(client, "https://api.example.com".to_string());
        assert_eq!(tool.rails_url(), "https://api.example.com/api/v1/agent/create");
    }
}
