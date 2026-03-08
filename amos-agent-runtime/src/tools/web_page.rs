use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, error, info};

use super::Tool;
use amos_core::error::{AmosError, Result};
use amos_core::types::ToolDefinition;

/// ViewWebPageTool fetches and displays web page content
pub struct ViewWebPageTool {
    http_client: reqwest::Client,
}

impl ViewWebPageTool {
    /// Create a new ViewWebPageTool instance
    pub fn new(http_client: reqwest::Client) -> Self {
        Self { http_client }
    }

    /// Fetch web page content
    async fn fetch_page(&self, url: &str) -> Result<String> {
        debug!("Fetching web page: {}", url);

        let response = self.http_client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; AMOS-Agent/1.0)")
            .send()
            .await
            .map_err(|e| AmosError::ToolExecutionFailed {
                tool: "view_web_page".into(),
                reason: format!("Failed to fetch web page: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("Failed to fetch page: HTTP {}", status);
            return Err(AmosError::ToolExecutionFailed {
                tool: "view_web_page".into(),
                reason: format!("HTTP error: {}", status),
            });
        }

        let content = response.text().await
            .map_err(|e| AmosError::ToolExecutionFailed {
                tool: "view_web_page".into(),
                reason: format!("Failed to read response body: {}", e),
            })?;

        Ok(content)
    }

    /// Extract text content from HTML
    fn extract_text(&self, html: &str) -> String {
        // Simple HTML tag stripping - in production, use a proper HTML parser like scraper
        let text = html
            // Remove script and style tags with their content
            .replace(
                |c| matches!(c, '<'),
                "\n<"
            );

        let mut result = String::new();
        let mut inside_tag = false;
        let mut inside_script_or_style = false;
        let mut tag_name = String::new();

        for line in text.lines() {
            for ch in line.chars() {
                match ch {
                    '<' => {
                        inside_tag = true;
                        tag_name.clear();
                    }
                    '>' => {
                        inside_tag = false;
                        let tag_lower = tag_name.to_lowercase();
                        if tag_lower.starts_with("script") || tag_lower.starts_with("style") {
                            inside_script_or_style = true;
                        } else if tag_lower.starts_with("/script") || tag_lower.starts_with("/style") {
                            inside_script_or_style = false;
                        }
                        tag_name.clear();
                    }
                    _ if inside_tag => {
                        tag_name.push(ch);
                    }
                    _ if !inside_script_or_style => {
                        result.push(ch);
                    }
                    _ => {}
                }
            }
        }

        // Clean up whitespace
        let cleaned = result
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        // Decode common HTML entities
        cleaned
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&apos;", "'")
    }

    /// Truncate content to maximum size
    fn truncate_content(&self, content: &str, max_bytes: usize) -> String {
        if content.len() <= max_bytes {
            return content.to_string();
        }

        let truncated = &content[..max_bytes];
        let last_newline = truncated.rfind('\n').unwrap_or(max_bytes);
        let result = &truncated[..last_newline];

        format!("{}\n\n[Content truncated - original size: {} bytes, showing first {} KB]",
            result, content.len(), max_bytes / 1024)
    }
}

#[async_trait]
impl Tool for ViewWebPageTool {
    fn name(&self) -> &str {
        "view_web_page"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "view_web_page".to_string(),
            description: "Fetch a web page and extract its text content. Useful for reading articles, documentation, and web resources.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL of the web page to fetch"
                    },
                    "extract_mode": {
                        "type": "string",
                        "enum": ["text", "full", "screenshot"],
                        "default": "text",
                        "description": "Extraction mode: 'text' for cleaned text only, 'full' for complete HTML, 'screenshot' for visual capture (not yet implemented)"
                    }
                },
                "required": ["url"]
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: &Value) -> Result<String> {
        info!("Executing view_web_page tool");

        let url = input.get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AmosError::ToolExecutionFailed {
                tool: "view_web_page".into(),
                reason: "Missing required parameter: url".into(),
            })?;

        let extract_mode = input.get("extract_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("text");

        // Validate URL format
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(AmosError::ToolExecutionFailed {
                tool: "view_web_page".into(),
                reason: "URL must start with http:// or https://".into(),
            });
        }

        debug!("Fetching URL: {} with mode: {}", url, extract_mode);

        let content = self.fetch_page(url).await?;

        let result = match extract_mode {
            "full" => {
                debug!("Returning full HTML content");
                content
            }
            "screenshot" => {
                return Err(AmosError::ToolExecutionFailed {
                    tool: "view_web_page".into(),
                    reason: "Screenshot mode is not yet implemented. Please use 'text' or 'full' mode.".into(),
                });
            }
            _ => {
                debug!("Extracting text content from HTML");
                self.extract_text(&content)
            }
        };

        // Truncate to 50KB
        let max_size = 50 * 1024;
        let final_result = self.truncate_content(&result, max_size);

        info!("Successfully fetched and processed web page (final size: {} bytes)", final_result.len());
        Ok(final_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let client = reqwest::Client::new();
        let tool = ViewWebPageTool::new(client);
        let def = tool.definition();

        assert_eq!(def.name, "view_web_page");
        assert!(def.description.contains("Fetch a web page"));
    }

    #[test]
    fn test_extract_text() {
        let client = reqwest::Client::new();
        let tool = ViewWebPageTool::new(client);

        let html = r#"
            <html>
                <head><title>Test</title></head>
                <body>
                    <h1>Hello World</h1>
                    <p>This is a test.</p>
                    <script>console.log("ignore this");</script>
                </body>
            </html>
        "#;

        let text = tool.extract_text(html);
        assert!(text.contains("Hello World"));
        assert!(text.contains("This is a test"));
        assert!(!text.contains("console.log"));
    }

    #[test]
    fn test_truncate_content() {
        let client = reqwest::Client::new();
        let tool = ViewWebPageTool::new(client);

        let content = "Line 1\nLine 2\nLine 3\n".repeat(1000);
        let truncated = tool.truncate_content(&content, 100);

        assert!(truncated.len() <= 200); // Some overhead for truncation message
        assert!(truncated.contains("[Content truncated"));
    }
}
