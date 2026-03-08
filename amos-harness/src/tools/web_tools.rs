//! Web access tools

use super::{Tool, ToolCategory, ToolResult};
use amos_core::{AppConfig, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;

/// Search the web
pub struct WebSearchTool {
    config: Arc<AppConfig>,
}

impl WebSearchTool {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for information using a search engine"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return",
                    "default": 5
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let query = params["query"].as_str().ok_or_else(|| {
            amos_core::AmosError::Validation("query is required".to_string())
        })?;

        let num_results = params
            .get("num_results")
            .and_then(|v| v.as_i64())
            .unwrap_or(5);

        // TODO: Integrate with actual search API (Brave, Google, etc.)
        // For now, return stub results

        let results = vec![
            json!({
                "title": format!("Search result for: {}", query),
                "url": "https://example.com/result1",
                "snippet": "This is a stub search result. Real implementation pending."
            }),
        ];

        Ok(ToolResult::success(json!({
            "query": query,
            "results": results,
            "count": results.len()
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Web
    }
}

/// Fetch and parse a web page
pub struct ViewWebPageTool;

impl ViewWebPageTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ViewWebPageTool {
    fn name(&self) -> &str {
        "view_web_page"
    }

    fn description(&self) -> &str {
        "Fetch and parse the content of a web page"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch"
                },
                "extract_format": {
                    "type": "string",
                    "enum": ["text", "markdown", "html"],
                    "description": "Format to extract content in",
                    "default": "text"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let url = params["url"].as_str().ok_or_else(|| {
            amos_core::AmosError::Validation("url is required".to_string())
        })?;

        let extract_format = params
            .get("extract_format")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        // Fetch the web page
        let response: reqwest::Response = reqwest::get(url).await.map_err(|e| {
            amos_core::AmosError::Internal(format!("External: Failed to fetch URL: {}", e))
        })?;

        let html = response.text().await.map_err(|e| {
            amos_core::AmosError::Internal(format!(
                "External: Failed to read response body: {}",
                e
            ))
        })?;

        // Extract content based on format
        let content = match extract_format {
            "html" => html.clone(),
            "markdown" => {
                // TODO: Convert HTML to markdown
                // For now, just strip tags
                strip_html_tags(&html)
            }
            _ => {
                // Extract text
                strip_html_tags(&html)
            }
        };

        Ok(ToolResult::success(json!({
            "url": url,
            "content": content,
            "format": extract_format
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Web
    }
}

/// Simple HTML tag stripper
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        let html = "<p>Hello <strong>world</strong>!</p>";
        let text = strip_html_tags(html);
        assert_eq!(text, "Hello world!");
    }
}
