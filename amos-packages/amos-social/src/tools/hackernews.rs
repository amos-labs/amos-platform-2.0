//! Hacker News posting tool.
//!
//! HN has no official posting API — uses authenticated form submission
//! with CSRF token extraction.

use amos_core::tools::{Tool, ToolCategory, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use tracing::{info, warn};

pub struct PostHackerNewsTool {
    db_pool: PgPool,
}

impl PostHackerNewsTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for PostHackerNewsTool {
    fn name(&self) -> &str {
        "post_hackernews"
    }

    fn description(&self) -> &str {
        "Post a submission to Hacker News (news.ycombinator.com). Supports \
         link posts and Show HN / Ask HN text posts. Requires HN account \
         credentials in the vault. Note: HN has no official API for posting \
         — this tool uses authenticated form submission."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "connection_id": {
                    "type": "string",
                    "description": "UUID of the HN account connection"
                },
                "title": {
                    "type": "string",
                    "description": "Post title (max ~80 characters recommended). Prefix with 'Show HN: ' for Show HN posts.",
                    "maxLength": 80
                },
                "url": {
                    "type": "string",
                    "description": "URL for link posts. Omit for text-only (Ask HN) posts."
                },
                "text": {
                    "type": "string",
                    "description": "Body text for Ask HN or Show HN posts. Only used when url is omitted."
                }
            },
            "required": ["connection_id", "title"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }

    async fn execute(&self, params: JsonValue) -> amos_core::Result<ToolResult> {
        let connection_id = params
            .get("connection_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Internal("Missing 'connection_id' parameter".into())
            })?;
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Internal("Missing 'title' parameter".into())
            })?;
        let url = params.get("url").and_then(|v| v.as_str());
        let text = params.get("text").and_then(|v| v.as_str());

        let creds = resolve_hn_credentials(&self.db_pool, connection_id).await?;

        // Use a client with cookie support
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .build()
            .map_err(|e| {
                amos_core::AmosError::Internal(format!("Failed to create HTTP client: {}", e))
            })?;

        // Step 1: Login to HN
        let login_resp = client
            .post("https://news.ycombinator.com/login")
            .form(&[
                ("acct", creds.username.as_str()),
                ("pw", creds.password.as_str()),
                ("goto", "submit"),
            ])
            .send()
            .await
            .map_err(|e| {
                amos_core::AmosError::Internal(format!("HN login request failed: {}", e))
            })?;

        if !login_resp.status().is_success() && !login_resp.status().is_redirection() {
            return Ok(ToolResult {
                success: false,
                data: None,
                error: Some("HN login failed — check username and password".into()),
                metadata: Some(json!({ "error_code": "AUTH_INVALID" })),
            });
        }

        // Step 2: GET /submit to extract CSRF token (fnid)
        let submit_page = client
            .get("https://news.ycombinator.com/submit")
            .send()
            .await
            .map_err(|e| {
                amos_core::AmosError::Internal(format!("Failed to load submit page: {}", e))
            })?
            .text()
            .await
            .map_err(|e| {
                amos_core::AmosError::Internal(format!("Failed to read submit page: {}", e))
            })?;

        // Extract fnid from hidden form field
        let fnid = extract_fnid(&submit_page).ok_or_else(|| {
            amos_core::AmosError::Internal(
                "Could not extract CSRF token from HN submit page. Login may have failed.".into(),
            )
        })?;

        // Step 3: POST the submission
        let mut form = vec![
            ("fnid", fnid.as_str()),
            ("fnop", "submit-page"),
            ("title", title),
        ];
        if let Some(u) = url {
            form.push(("url", u));
        }
        if let Some(t) = text {
            form.push(("text", t));
        }

        let post_resp = client
            .post("https://news.ycombinator.com/r")
            .form(&form)
            .send()
            .await
            .map_err(|e| {
                amos_core::AmosError::Internal(format!("HN submit request failed: {}", e))
            })?;

        // HN redirects to the new post on success, or back to submit on failure
        let final_url = post_resp.url().to_string();
        let resp_text = post_resp.text().await.unwrap_or_default();

        // Check if we got redirected to the post
        let post_id = if final_url.contains("item?id=") {
            final_url
                .split("item?id=")
                .last()
                .unwrap_or("unknown")
                .to_string()
        } else {
            // Try to find the post ID in newest submissions
            warn!("HN did not redirect to post. Checking newest...");

            // Check for common error indicators
            if resp_text.contains("duplicate") {
                return Ok(ToolResult {
                    success: false,
                    data: None,
                    error: Some("HN rejected the submission — duplicate URL detected".into()),
                    metadata: Some(json!({ "error_code": "CONTENT_REJECTED" })),
                });
            }

            "pending".to_string()
        };

        let post_url = if post_id != "pending" {
            format!("https://news.ycombinator.com/item?id={}", post_id)
        } else {
            "https://news.ycombinator.com/newest".to_string()
        };

        let post_type = if title.starts_with("Show HN:") || title.starts_with("Show HN ") {
            "show_hn"
        } else if title.starts_with("Ask HN:") || title.starts_with("Ask HN ") {
            "ask_hn"
        } else {
            "link"
        };

        // Record the post
        let _ = super::twitter::record_post(
            &self.db_pool,
            "hackernews",
            &post_id,
            title,
            &post_url,
        )
        .await;

        info!(
            post_id = %post_id,
            post_type = post_type,
            "Hacker News submission posted"
        );

        Ok(ToolResult {
            success: true,
            data: Some(json!({
                "post_id": post_id,
                "url": post_url,
                "title": title,
                "type": post_type,
                "created_at": chrono::Utc::now().to_rfc3339(),
            })),
            error: None,
            metadata: None,
        })
    }
}

struct HnCredentials {
    username: String,
    password: String,
}

async fn resolve_hn_credentials(
    db_pool: &PgPool,
    connection_id: &str,
) -> amos_core::Result<HnCredentials> {
    let conn_id: uuid::Uuid = connection_id.parse().map_err(|_| {
        amos_core::AmosError::Internal("Invalid connection_id UUID".into())
    })?;

    let row = sqlx::query(
        "SELECT credentials_data FROM integration_connections WHERE id = $1",
    )
    .bind(conn_id)
    .fetch_optional(db_pool)
    .await
    .map_err(|e| amos_core::AmosError::Internal(format!("DB error: {}", e)))?
    .ok_or_else(|| {
        amos_core::AmosError::Internal(format!(
            "HN connection {} not found. Set up credentials first.",
            connection_id
        ))
    })?;

    let creds: JsonValue = sqlx::Row::try_get(&row, "credentials_data")
        .map_err(|e| amos_core::AmosError::Internal(format!("Credential read error: {}", e)))?;

    let username = creds
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            amos_core::AmosError::Internal("No username in HN credentials".into())
        })?
        .to_string();

    let password = creds
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            amos_core::AmosError::Internal("No password in HN credentials".into())
        })?
        .to_string();

    Ok(HnCredentials { username, password })
}

/// Extract the CSRF token (fnid) from the HN submit page HTML.
fn extract_fnid(html: &str) -> Option<String> {
    // Look for: <input type="hidden" name="fnid" value="...">
    let marker = "name=\"fnid\" value=\"";
    let start = html.find(marker)? + marker.len();
    let end = html[start..].find('"')? + start;
    Some(html[start..end].to_string())
}
