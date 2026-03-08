use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, info};

use super::Tool;
use amos_core::error::{AmosError, Result};
use amos_core::types::ToolDefinition;

/// Tool for updating existing platform objects (contacts, campaigns, bounties, workflows, etc.)
/// Delegates to the Rails API endpoint during hybrid mode.
pub struct PlatformUpdateTool {
    rails_base_url: String,
    http_client: reqwest::Client,
}

impl PlatformUpdateTool {
    /// Create a new PlatformUpdateTool instance
    pub fn new(http_client: reqwest::Client) -> Self {
        Self {
            rails_base_url: "http://localhost:5001".to_string(),
            http_client,
        }
    }

    /// Create a new PlatformUpdateTool with a custom Rails base URL
    pub fn with_base_url(http_client: reqwest::Client, rails_base_url: String) -> Self {
        Self {
            rails_base_url,
            http_client,
        }
    }

    /// Format the Rails API URL for the update endpoint
    fn rails_url(&self) -> String {
        format!("{}/api/v1/agent/update", self.rails_base_url)
    }
}

#[async_trait]
impl Tool for PlatformUpdateTool {
    fn name(&self) -> &str {
        "platform_update"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "platform_update".to_string(),
            description: "Update an existing object on the AMOS platform. Modify contacts, campaigns, bounties, workflows, and any other platform object by ID.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "object_type": {
                        "type": "string",
                        "description": "The type of object to update (e.g., 'contact', 'campaign', 'bounty', 'workflow', 'landing_page', 'email_template', 'integration', 'task', 'note', 'tag', 'custom_field', 'app_module')"
                    },
                    "id": {
                        "description": "The ID of the object to update (can be string or number)",
                        "oneOf": [
                            {"type": "string"},
                            {"type": "number"}
                        ]
                    },
                    "attributes": {
                        "type": "object",
                        "description": "The attributes to update. Only specified fields will be modified."
                    }
                },
                "required": ["object_type", "id", "attributes"]
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: &Value) -> Result<String> {
        info!("Executing platform_update tool");
        debug!("Input: {}", input);

        // Validate input structure
        let object_type = input
            .get("object_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AmosError::ToolExecutionFailed {
                    tool: "platform_update".into(),
                    reason: "Missing required field: object_type".into(),
                }
            })?;

        let id = input.get("id").ok_or_else(|| {
            AmosError::ToolExecutionFailed {
                tool: "platform_update".into(),
                reason: "Missing required field: id".into(),
            }
        })?;

        let attributes = input.get("attributes").ok_or_else(|| {
            AmosError::ToolExecutionFailed {
                tool: "platform_update".into(),
                reason: "Missing required field: attributes".into(),
            }
        })?;

        debug!(
            "Updating object of type '{}' with id '{}' and attributes: {}",
            object_type, id, attributes
        );

        // Build request body
        let request_body = json!({
            "object_type": object_type,
            "id": id,
            "attributes": attributes
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
                    tool: "platform_update".into(),
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
                tool: "platform_update".into(),
                reason: format!("Rails API returned error ({}): {}", status, error_text),
            });
        }

        // Parse response
        let result = response.json::<Value>().await.map_err(|e| {
            error!("Failed to parse response JSON: {}", e);
            AmosError::ToolExecutionFailed {
                tool: "platform_update".into(),
                reason: format!("Failed to parse response: {}", e),
            }
        })?;

        info!("Successfully updated object of type '{}' with id '{}'", object_type, id);
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
        let tool = PlatformUpdateTool::new(client);
        let def = tool.definition();

        assert_eq!(def.name, "platform_update");
        assert!(def.description.contains("Update an existing object"));
        assert_eq!(def.input_schema["type"], "object");
        assert!(def.input_schema["required"].as_array().unwrap().contains(&json!("object_type")));
        assert!(def.input_schema["required"].as_array().unwrap().contains(&json!("id")));
        assert!(def.input_schema["required"].as_array().unwrap().contains(&json!("attributes")));
    }

    #[test]
    fn test_rails_url_formatting() {
        let client = reqwest::Client::new();
        let tool = PlatformUpdateTool::new(client);
        assert_eq!(tool.rails_url(), "http://localhost:5001/api/v1/agent/update");

        let client = reqwest::Client::new();
        let tool = PlatformUpdateTool::with_base_url(client, "https://api.example.com".to_string());
        assert_eq!(tool.rails_url(), "https://api.example.com/api/v1/agent/update");
    }
}
