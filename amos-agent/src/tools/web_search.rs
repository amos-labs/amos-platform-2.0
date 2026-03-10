//! Web search tool via Brave Search API.
//!
//! Gives the agent the ability to search the web for real-time information.
//! Uses the Brave Search API which provides high-quality results without
//! tracking.

use amos_core::types::ToolDefinition;
use serde::Deserialize;
use serde_json::json;

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "web_search".to_string(),
        description: "Search the web for current information. Use this when you need \
            real-time data, recent events, documentation, or any information that may \
            not be in your training data. Returns search results with titles, URLs, \
            and descriptions."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of results to return (default: 5, max: 10)"
                }
            },
            "required": ["query"]
        }),
        requires_confirmation: false,
    }
}

#[derive(Debug, Deserialize)]
struct BraveSearchResponse {
    #[serde(default)]
    web: Option<BraveWebResults>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResults {
    results: Vec<BraveResult>,
}

#[derive(Debug, Deserialize)]
struct BraveResult {
    title: String,
    url: String,
    description: Option<String>,
}

/// Execute a web search.
pub async fn execute(input: &serde_json::Value, api_key: Option<&str>) -> Result<String, String> {
    let query = input["query"]
        .as_str()
        .ok_or("Missing required field: query")?;
    let count = input
        .get("count")
        .and_then(|c| c.as_u64())
        .unwrap_or(5)
        .min(10);

    let api_key = api_key.ok_or(
        "Web search unavailable: BRAVE_API_KEY not configured. \
         Set the BRAVE_API_KEY environment variable to enable web search."
    )?;

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("X-Subscription-Token", api_key)
        .header("Accept", "application/json")
        .query(&[("q", query), ("count", &count.to_string())])
        .send()
        .await
        .map_err(|e| format!("Search request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Brave Search API error {status}: {body}"));
    }

    let search: BraveSearchResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse search results: {e}"))?;

    let results = match search.web {
        Some(web) => web.results,
        None => return Ok(json!({"results": [], "message": "No results found"}).to_string()),
    };

    let formatted: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            json!({
                "title": r.title,
                "url": r.url,
                "description": r.description.as_deref().unwrap_or(""),
            })
        })
        .collect();

    Ok(json!({
        "query": query,
        "count": formatted.len(),
        "results": formatted,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_web_search_no_api_key() {
        let input = json!({"query": "rust programming"});
        let result = execute(&input, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("BRAVE_API_KEY"));
    }

    #[test]
    fn test_web_search_definition() {
        let def = definition();
        assert_eq!(def.name, "web_search");
        assert!(!def.requires_confirmation);
    }
}
