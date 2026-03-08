use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;
use tracing::{debug, error, info, warn};

use super::Tool;
use amos_core::error::{AmosError, Result};
use amos_core::types::ToolDefinition;

/// WebSearchTool provides web search capabilities via Serper API
pub struct WebSearchTool {
    http_client: reqwest::Client,
    api_key: String,
}

impl WebSearchTool {
    /// Create a new WebSearchTool instance
    pub fn new(http_client: reqwest::Client) -> Self {
        let api_key = std::env::var("SERPER_API_KEY")
            .unwrap_or_else(|_| String::new());

        Self {
            http_client,
            api_key,
        }
    }

    /// Perform a single search query
    async fn search_once(&self, query: &str, num_results: usize) -> Result<Vec<SearchResult>> {
        debug!("Performing search for query: {}", query);

        if self.api_key.is_empty() {
            return Err(AmosError::ToolExecutionFailed {
                tool: "web_search".into(),
                reason: "SERPER_API_KEY environment variable not set".into(),
            });
        }

        let response = self.http_client
            .post("https://google.serper.dev/search")
            .header("X-API-KEY", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&json!({
                "q": query,
                "num": num_results
            }))
            .send()
            .await
            .map_err(|e| AmosError::ToolExecutionFailed {
                tool: "web_search".into(),
                reason: format!("Failed to perform web search: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Search API returned error: {} - {}", status, error_text);
            return Err(AmosError::ToolExecutionFailed {
                tool: "web_search".into(),
                reason: format!("Search API error: {} - {}", status, error_text),
            });
        }

        let data: Value = response.json().await
            .map_err(|e| AmosError::ToolExecutionFailed {
                tool: "web_search".into(),
                reason: format!("Failed to parse search response: {}", e),
            })?;

        let mut results = Vec::new();
        if let Some(organic) = data.get("organic").and_then(|v| v.as_array()) {
            for item in organic {
                if let (Some(title), Some(link)) = (
                    item.get("title").and_then(|v| v.as_str()),
                    item.get("link").and_then(|v| v.as_str())
                ) {
                    let snippet = item.get("snippet")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    results.push(SearchResult {
                        title: title.to_string(),
                        url: link.to_string(),
                        snippet,
                    });
                }
            }
        }

        debug!("Found {} search results", results.len());
        Ok(results)
    }

    /// Perform deep search with query variations
    async fn search_deep(&self, query: &str, num_results: usize) -> Result<Vec<SearchResult>> {
        info!("Performing deep search for: {}", query);

        // Generate query variations
        let variations = vec![
            query.to_string(),
            format!("{} tutorial", query),
            format!("{} guide", query),
            format!("what is {}", query),
        ];

        let mut all_results = Vec::new();
        let mut seen_urls = HashSet::new();

        for variation in variations {
            match self.search_once(&variation, num_results).await {
                Ok(results) => {
                    for result in results {
                        // Deduplicate by URL
                        if !seen_urls.contains(&result.url) {
                            seen_urls.insert(result.url.clone());
                            all_results.push(result);
                        }
                    }
                }
                Err(e) => {
                    warn!("Search variation '{}' failed: {}", variation, e);
                }
            }
        }

        // Limit to requested number of results
        all_results.truncate(num_results * 2); // Allow more for deep mode

        info!("Deep search returned {} unique results", all_results.len());
        Ok(all_results)
    }

    /// Format search results as a numbered list
    fn format_results(&self, results: &[SearchResult]) -> String {
        if results.is_empty() {
            return "No results found.".to_string();
        }

        let mut output = String::new();
        output.push_str(&format!("Found {} results:\n\n", results.len()));

        for (i, result) in results.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, result.title));
            output.push_str(&format!("   URL: {}\n", result.url));
            if !result.snippet.is_empty() {
                output.push_str(&format!("   {}\n", result.snippet));
            }
            output.push('\n');
        }

        output
    }
}

#[derive(Debug, Clone)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "web_search".to_string(),
            description: "Search the web for information. Supports quick and deep search modes. Returns titles, URLs, and snippets.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["quick", "deep"],
                        "default": "quick",
                        "description": "Search mode: 'quick' for single query, 'deep' for multiple query variations with deduplication"
                    },
                    "num_results": {
                        "type": "number",
                        "default": 5,
                        "description": "Number of results to return (1-20)"
                    }
                },
                "required": ["query"]
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: &Value) -> Result<String> {
        info!("Executing web_search tool");

        let query = input.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AmosError::ToolExecutionFailed {
                tool: "web_search".into(),
                reason: "Missing required parameter: query".into(),
            })?;

        let mode = input.get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("quick");

        let num_results = input.get("num_results")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        // Validate num_results
        let num_results = num_results.clamp(1, 20);

        debug!("Search parameters - query: '{}', mode: '{}', num_results: {}", query, mode, num_results);

        let results = match mode {
            "deep" => self.search_deep(query, num_results).await?,
            _ => self.search_once(query, num_results).await?,
        };

        Ok(self.format_results(&results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let client = reqwest::Client::new();
        let tool = WebSearchTool::new(client);
        let def = tool.definition();

        assert_eq!(def.name, "web_search");
        assert!(def.description.contains("Search the web"));
    }
}
