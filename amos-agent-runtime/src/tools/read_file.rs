use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, info};

use super::Tool;
use amos_core::error::{AmosError, Result};
use amos_core::types::ToolDefinition;

/// ReadFileTool provides access to platform documents and knowledge base
pub struct ReadFileTool {
    http_client: reqwest::Client,
    rails_base_url: String,
}

impl ReadFileTool {
    /// Create a new ReadFileTool instance with default Rails base URL
    pub fn new(http_client: reqwest::Client) -> Self {
        Self::with_base_url(
            http_client,
            std::env::var("RAILS_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string())
        )
    }

    /// Create a new ReadFileTool instance with custom Rails base URL
    pub fn with_base_url(http_client: reqwest::Client, rails_base_url: String) -> Self {
        Self {
            http_client,
            rails_base_url,
        }
    }

    /// URL encode a string
    fn url_encode(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                _ => format!("%{:02X}", c as u8),
            })
            .collect()
    }

    /// List available documents
    async fn list_documents(&self, path: Option<&str>) -> Result<String> {
        debug!("Listing documents at path: {:?}", path);

        let mut url = format!("{}/api/v1/agent/documents/list", self.rails_base_url);
        if let Some(p) = path {
            url.push_str(&format!("?path={}", Self::url_encode(p)));
        }

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: format!("Failed to list documents: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Document list API returned error: {} - {}", status, error_text);
            return Err(AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: format!("Failed to list documents: {} - {}", status, error_text),
            });
        }

        let data: Value = response.json().await
            .map_err(|e| AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: format!("Failed to parse list response: {}", e),
            })?;

        self.format_list_response(&data)
    }

    /// Read a specific document
    async fn read_document(&self, path: &str) -> Result<String> {
        debug!("Reading document: {}", path);

        let url = format!(
            "{}/api/v1/agent/documents/read?path={}",
            self.rails_base_url,
            Self::url_encode(path)
        );

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: format!("Failed to read document: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Document read API returned error: {} - {}", status, error_text);
            return Err(AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: format!("Failed to read document '{}': {} - {}", path, status, error_text),
            });
        }

        let data: Value = response.json().await
            .map_err(|e| AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: format!("Failed to parse read response: {}", e),
            })?;

        self.format_read_response(&data, path)
    }

    /// Search documents
    async fn search_documents(&self, query: &str) -> Result<String> {
        debug!("Searching documents with query: {}", query);

        let url = format!(
            "{}/api/v1/agent/documents/search?query={}",
            self.rails_base_url,
            Self::url_encode(query)
        );

        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: format!("Failed to search documents: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Document search API returned error: {} - {}", status, error_text);
            return Err(AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: format!("Failed to search documents: {} - {}", status, error_text),
            });
        }

        let data: Value = response.json().await
            .map_err(|e| AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: format!("Failed to parse search response: {}", e),
            })?;

        self.format_search_response(&data, query)
    }

    /// Format list response
    fn format_list_response(&self, data: &Value) -> Result<String> {
        let documents = data.get("documents")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: "Invalid list response format".into(),
            })?;

        if documents.is_empty() {
            return Ok("No documents found.".to_string());
        }

        let mut output = String::new();
        output.push_str(&format!("Found {} document(s):\n\n", documents.len()));

        for doc in documents {
            if let Some(path) = doc.get("path").and_then(|v| v.as_str()) {
                let doc_type = doc.get("type").and_then(|v| v.as_str()).unwrap_or("file");
                let size = doc.get("size").and_then(|v| v.as_u64()).unwrap_or(0);

                output.push_str(&format!("- {} ({})", path, doc_type));
                if size > 0 {
                    output.push_str(&format!(" - {} bytes", size));
                }
                output.push('\n');
            }
        }

        Ok(output)
    }

    /// Format read response
    fn format_read_response(&self, data: &Value, path: &str) -> Result<String> {
        let content = data.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: "Invalid read response format".into(),
            })?;

        let mut output = String::new();
        output.push_str(&format!("=== Document: {} ===\n\n", path));
        output.push_str(content);
        output.push_str("\n\n=== End of document ===");

        Ok(output)
    }

    /// Format search response
    fn format_search_response(&self, data: &Value, query: &str) -> Result<String> {
        let results = data.get("results")
            .and_then(|v| v.as_array())
            .ok_or_else(|| AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: "Invalid search response format".into(),
            })?;

        if results.is_empty() {
            return Ok(format!("No documents found matching '{}'.", query));
        }

        let mut output = String::new();
        output.push_str(&format!("Found {} result(s) for '{}':\n\n", results.len(), query));

        for (i, result) in results.iter().enumerate() {
            if let Some(path) = result.get("path").and_then(|v| v.as_str()) {
                output.push_str(&format!("{}. {}\n", i + 1, path));

                if let Some(snippet) = result.get("snippet").and_then(|v| v.as_str()) {
                    output.push_str(&format!("   {}\n", snippet));
                }

                if let Some(score) = result.get("score").and_then(|v| v.as_f64()) {
                    output.push_str(&format!("   Relevance: {:.2}\n", score));
                }

                output.push('\n');
            }
        }

        Ok(output)
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read files from the platform's document storage and knowledge base. Supports listing, reading, and searching documents.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list", "read", "search"],
                        "description": "The action to perform: 'list' to see available documents, 'read' to get document content, 'search' to find documents"
                    },
                    "path": {
                        "type": "string",
                        "description": "Document path (required for 'read' action, optional for 'list' to filter by directory)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query (required for 'search' action)"
                    }
                },
                "required": ["action"]
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: &Value) -> Result<String> {
        info!("Executing read_file tool");

        let action = input.get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: "Missing required parameter: action".into(),
            })?;

        debug!("Read file action: {}", action);

        match action {
            "list" => {
                let path = input.get("path").and_then(|v| v.as_str());
                self.list_documents(path).await
            }
            "read" => {
                let path = input.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AmosError::ToolExecutionFailed {
                        tool: "read_file".into(),
                        reason: "Missing required parameter 'path' for read action".into(),
                    })?;
                self.read_document(path).await
            }
            "search" => {
                let query = input.get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AmosError::ToolExecutionFailed {
                        tool: "read_file".into(),
                        reason: "Missing required parameter 'query' for search action".into(),
                    })?;
                self.search_documents(query).await
            }
            _ => Err(AmosError::ToolExecutionFailed {
                tool: "read_file".into(),
                reason: format!("Invalid action: '{}'. Must be 'list', 'read', or 'search'", action),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let client = reqwest::Client::new();
        let tool = ReadFileTool::new(client);
        let def = tool.definition();

        assert_eq!(def.name, "read_file");
        assert!(def.description.contains("document storage"));
    }
}
