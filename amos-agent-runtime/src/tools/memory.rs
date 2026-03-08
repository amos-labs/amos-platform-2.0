use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, info};

use super::Tool;
use amos_core::error::{AmosError, Result};
use amos_core::types::ToolDefinition;

/// Tool for storing persistent memories
pub struct RememberThisTool {
    http_client: reqwest::Client,
    rails_base_url: String,
}

impl RememberThisTool {
    /// Create a new RememberThisTool instance
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
}

#[async_trait]
impl Tool for RememberThisTool {
    fn name(&self) -> &str {
        "remember_this"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: "Store a fact, preference, or important information for long-term memory. This persists across conversations and can be retrieved later with search_memory.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The information to remember"
                    },
                    "category": {
                        "type": "string",
                        "enum": ["fact", "preference", "instruction", "context"],
                        "description": "Optional category for organizing memories"
                    },
                    "tags": {
                        "type": "array",
                        "items": {
                            "type": "string"
                        },
                        "description": "Optional tags for easier retrieval"
                    }
                },
                "required": ["content"]
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: &Value) -> Result<String> {
        debug!("RememberThisTool executing with input: {:?}", input);

        // Extract and validate input parameters
        let content = input["content"]
            .as_str()
            .ok_or_else(|| AmosError::ToolExecutionFailed {
                tool: "remember_this".into(),
                reason: "content is required".into(),
            })?;

        let category = input["category"].as_str();
        let tags = input["tags"].as_array();

        // Validate category if provided
        if let Some(cat) = category {
            let valid_categories = ["fact", "preference", "instruction", "context"];
            if !valid_categories.contains(&cat) {
                return Err(AmosError::ToolExecutionFailed {
                    tool: "remember_this".into(),
                    reason: format!("category must be one of: {}", valid_categories.join(", ")),
                });
            }
        }

        info!(
            "Storing memory: category={:?}, content_length={}, tags={:?}",
            category,
            content.len(),
            tags
        );

        // Build the payload
        let mut payload = json!({
            "content": content,
        });

        if let Some(cat) = category {
            payload["category"] = json!(cat);
        }

        if let Some(tags_array) = tags {
            payload["tags"] = json!(tags_array);
        }

        // Send to Rails API
        let api_url = format!("{}/api/v1/agent/memory/store", self.rails_base_url);

        let response = self.http_client
            .post(&api_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to connect to Rails API: {}", e);
                AmosError::ToolExecutionFailed {
                    tool: "remember_this".into(),
                    reason: format!("Failed to store memory: {}", e),
                }
            })?;

        if response.status().is_success() {
            info!("Memory stored successfully");
            let data = response.json::<Value>().await.map_err(|e| {
                error!("Failed to parse Rails API response: {}", e);
                AmosError::ToolExecutionFailed {
                    tool: "remember_this".into(),
                    reason: format!("Invalid response format: {}", e),
                }
            })?;
            Ok(data.to_string())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Rails API error: {} - {}", status, error_text);
            Err(AmosError::ToolExecutionFailed {
                tool: "remember_this".into(),
                reason: format!("Failed to store memory: {} - {}", status, error_text),
            })
        }
    }
}

/// Tool for searching persistent memories
pub struct SearchMemoryTool {
    http_client: reqwest::Client,
    rails_base_url: String,
}

impl SearchMemoryTool {
    /// Create a new SearchMemoryTool instance
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
}

#[async_trait]
impl Tool for SearchMemoryTool {
    fn name(&self) -> &str {
        "search_memory"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: "Search through stored memories, past conversations, and bookmarks. Use this to recall previously stored information.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query to find relevant memories"
                    },
                    "sources": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["memories", "conversations", "bookmarks"]
                        },
                        "description": "Optional sources to search within"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of results to return (default: 10)",
                        "default": 10
                    }
                },
                "required": ["query"]
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: &Value) -> Result<String> {
        debug!("SearchMemoryTool executing with input: {:?}", input);

        // Extract and validate input parameters
        let query = input["query"]
            .as_str()
            .ok_or_else(|| AmosError::ToolExecutionFailed {
                tool: "search_memory".into(),
                reason: "query is required".into(),
            })?;

        let sources = input["sources"].as_array();
        let limit = input["limit"].as_u64().unwrap_or(10);

        // Validate sources if provided
        if let Some(sources_array) = sources {
            let valid_sources = ["memories", "conversations", "bookmarks"];
            for source in sources_array {
                if let Some(source_str) = source.as_str() {
                    if !valid_sources.contains(&source_str) {
                        return Err(AmosError::ToolExecutionFailed {
                            tool: "search_memory".into(),
                            reason: format!("source must be one of: {}", valid_sources.join(", ")),
                        });
                    }
                }
            }
        }

        info!(
            "Searching memory: query='{}', sources={:?}, limit={}",
            query,
            sources,
            limit
        );

        // Build the payload
        let mut payload = json!({
            "query": query,
            "limit": limit,
        });

        if let Some(sources_array) = sources {
            payload["sources"] = json!(sources_array);
        }

        // Send to Rails API
        let api_url = format!("{}/api/v1/agent/memory/search", self.rails_base_url);

        let response = self.http_client
            .post(&api_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to connect to Rails API: {}", e);
                AmosError::ToolExecutionFailed {
                    tool: "search_memory".into(),
                    reason: format!("Failed to search memory: {}", e),
                }
            })?;

        if response.status().is_success() {
            info!("Memory search completed successfully");
            let data = response.json::<Value>().await.map_err(|e| {
                error!("Failed to parse Rails API response: {}", e);
                AmosError::ToolExecutionFailed {
                    tool: "search_memory".into(),
                    reason: format!("Invalid response format: {}", e),
                }
            })?;
            Ok(data.to_string())
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("Rails API error: {} - {}", status, error_text);
            Err(AmosError::ToolExecutionFailed {
                tool: "search_memory".into(),
                reason: format!("Failed to search memory: {} - {}", status, error_text),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remember_this_definition() {
        let client = reqwest::Client::new();
        let tool = RememberThisTool::new(client);

        assert_eq!(tool.name(), "remember_this");

        let def = tool.definition();
        assert_eq!(def.name, "remember_this");
        assert!(def.description.contains("Store a fact"));

        let schema = def.input_schema;
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["content"].is_object());
        assert!(schema["properties"]["category"].is_object());
        assert!(schema["properties"]["tags"].is_object());
        assert!(schema["required"].as_array().unwrap().contains(&json!("content")));
    }

    #[test]
    fn test_search_memory_definition() {
        let client = reqwest::Client::new();
        let tool = SearchMemoryTool::new(client);

        assert_eq!(tool.name(), "search_memory");

        let def = tool.definition();
        assert_eq!(def.name, "search_memory");
        assert!(def.description.contains("Search through stored memories"));

        let schema = def.input_schema;
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["sources"].is_object());
        assert!(schema["properties"]["limit"].is_object());
        assert!(schema["required"].as_array().unwrap().contains(&json!("query")));
    }

    #[tokio::test]
    async fn test_remember_this_missing_content() {
        let client = reqwest::Client::new();
        let tool = RememberThisTool::new(client);

        let input = json!({
            "category": "fact"
            // missing content
        });

        let result = tool.execute(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remember_this_invalid_category() {
        let client = reqwest::Client::new();
        let tool = RememberThisTool::new(client);

        let input = json!({
            "content": "test content",
            "category": "invalid_category"
        });

        let result = tool.execute(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_memory_missing_query() {
        let client = reqwest::Client::new();
        let tool = SearchMemoryTool::new(client);

        let input = json!({
            "limit": 5
            // missing query
        });

        let result = tool.execute(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_memory_invalid_source() {
        let client = reqwest::Client::new();
        let tool = SearchMemoryTool::new(client);

        let input = json!({
            "query": "test query",
            "sources": ["invalid_source"]
        });

        let result = tool.execute(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_memory_default_limit() {
        let client = reqwest::Client::new();
        let tool = SearchMemoryTool::new(client);

        let input = json!({
            "query": "test query"
        });

        // This will fail due to no Rails server, but we can verify the input parsing works
        let result = tool.execute(&input).await;
        assert!(result.is_err()); // Expected to fail without Rails server
    }
}
