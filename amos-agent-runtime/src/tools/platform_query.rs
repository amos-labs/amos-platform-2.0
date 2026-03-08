use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, info};

use super::Tool;
use amos_core::error::{AmosError, Result};
use amos_core::types::ToolDefinition;

/// Tool for querying platform data (contacts, campaigns, bounties, analytics, etc.)
/// Delegates to the Rails API endpoint during hybrid mode.
pub struct PlatformQueryTool {
    rails_base_url: String,
    http_client: reqwest::Client,
}

impl PlatformQueryTool {
    /// Create a new PlatformQueryTool instance
    pub fn new(http_client: reqwest::Client) -> Self {
        Self {
            rails_base_url: "http://localhost:5001".to_string(),
            http_client,
        }
    }

    /// Create a new PlatformQueryTool with a custom Rails base URL
    pub fn with_base_url(http_client: reqwest::Client, rails_base_url: String) -> Self {
        Self {
            rails_base_url,
            http_client,
        }
    }

    /// Format the Rails API URL for the query endpoint
    fn rails_url(&self) -> String {
        format!("{}/api/v1/agent/query", self.rails_base_url)
    }
}

#[async_trait]
impl Tool for PlatformQueryTool {
    fn name(&self) -> &str {
        "platform_query"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "platform_query".to_string(),
            description: "Query data from the AMOS platform. Read contacts, campaigns, bounties, analytics, integrations, schema info, and any other platform data.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query_type": {
                        "type": "string",
                        "description": "The type of query to perform (e.g., 'contacts', 'campaigns', 'bounties', 'analytics', 'integrations', 'schema', 'workflows', 'landing_pages', 'email_templates', 'tasks', 'notes', 'tags')"
                    },
                    "filters": {
                        "type": "object",
                        "description": "Optional filters to apply to the query. Structure varies by query_type."
                    },
                    "limit": {
                        "type": "number",
                        "description": "Optional maximum number of results to return"
                    },
                    "offset": {
                        "type": "number",
                        "description": "Optional number of results to skip (for pagination)"
                    },
                    "include": {
                        "type": "array",
                        "description": "Optional array of relation names to include in the response (e.g., ['tags', 'custom_fields'])",
                        "items": {
                            "type": "string"
                        }
                    }
                },
                "required": ["query_type"]
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: &Value) -> Result<String> {
        info!("Executing platform_query tool");
        debug!("Input: {}", input);

        // Validate input structure
        let query_type = input
            .get("query_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AmosError::ToolExecutionFailed {
                    tool: "platform_query".into(),
                    reason: "Missing required field: query_type".into(),
                }
            })?;

        debug!("Querying data for type '{}'", query_type);

        // Build request body with optional fields
        let mut request_body = json!({
            "query_type": query_type
        });

        if let Some(filters) = input.get("filters") {
            request_body["filters"] = filters.clone();
            debug!("Applying filters: {}", filters);
        }

        if let Some(limit) = input.get("limit") {
            request_body["limit"] = limit.clone();
        }

        if let Some(offset) = input.get("offset") {
            request_body["offset"] = offset.clone();
        }

        if let Some(include) = input.get("include") {
            request_body["include"] = include.clone();
        }

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
                    tool: "platform_query".into(),
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
                tool: "platform_query".into(),
                reason: format!("Rails API returned error ({}): {}", status, error_text),
            });
        }

        // Parse response
        let result = response.json::<Value>().await.map_err(|e| {
            error!("Failed to parse response JSON: {}", e);
            AmosError::ToolExecutionFailed {
                tool: "platform_query".into(),
                reason: format!("Failed to parse response: {}", e),
            }
        })?;

        info!("Successfully queried data for type '{}'", query_type);
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
        let tool = PlatformQueryTool::new(client);
        let def = tool.definition();

        assert_eq!(def.name, "platform_query");
        assert!(def.description.contains("Query data from the AMOS platform"));
        assert_eq!(def.input_schema["type"], "object");
        assert!(def.input_schema["required"].as_array().unwrap().contains(&json!("query_type")));
    }

    #[test]
    fn test_rails_url_formatting() {
        let client = reqwest::Client::new();
        let tool = PlatformQueryTool::new(client);
        assert_eq!(tool.rails_url(), "http://localhost:5001/api/v1/agent/query");

        let client = reqwest::Client::new();
        let tool = PlatformQueryTool::with_base_url(client, "https://api.example.com".to_string());
        assert_eq!(tool.rails_url(), "https://api.example.com/api/v1/agent/query");
    }
}
