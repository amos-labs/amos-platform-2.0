//! Reddit posting tool.

use amos_core::tools::{Tool, ToolCategory, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use tracing::info;

pub struct PostRedditTool {
    db_pool: PgPool,
}

impl PostRedditTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for PostRedditTool {
    fn name(&self) -> &str {
        "post_reddit"
    }

    fn description(&self) -> &str {
        "Post a submission to a Reddit subreddit. Supports text (self) posts \
         and link posts. Requires a Reddit API connection with OAuth 2.0 \
         credentials in the vault."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "connection_id": {
                    "type": "string",
                    "description": "UUID of the Reddit API connection"
                },
                "subreddit": {
                    "type": "string",
                    "description": "Subreddit name without r/ prefix (e.g., 'artificial')"
                },
                "title": {
                    "type": "string",
                    "description": "Post title (max 300 characters)",
                    "maxLength": 300
                },
                "text": {
                    "type": "string",
                    "description": "Post body text (markdown supported). Required for self posts."
                },
                "url": {
                    "type": "string",
                    "description": "URL for link posts. Mutually exclusive with text."
                },
                "flair_id": {
                    "type": "string",
                    "description": "Optional: Flair ID for the post"
                }
            },
            "required": ["connection_id", "subreddit", "title"]
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
        let subreddit = params
            .get("subreddit")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Internal("Missing 'subreddit' parameter".into())
            })?;
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Internal("Missing 'title' parameter".into())
            })?;

        if title.len() > 300 {
            return Ok(ToolResult {
                success: false,
                data: None,
                error: Some(format!("Title exceeds 300 characters ({} chars)", title.len())),
                metadata: None,
            });
        }

        let text = params.get("text").and_then(|v| v.as_str());
        let url = params.get("url").and_then(|v| v.as_str());

        let kind = if url.is_some() { "link" } else { "self" };

        let creds = resolve_reddit_credentials(&self.db_pool, connection_id).await?;
        let client = reqwest::Client::new();

        let mut form = vec![
            ("sr", subreddit.to_string()),
            ("title", title.to_string()),
            ("kind", kind.to_string()),
            ("api_type", "json".to_string()),
        ];

        if let Some(t) = text {
            form.push(("text", t.to_string()));
        }
        if let Some(u) = url {
            form.push(("url", u.to_string()));
        }
        if let Some(flair) = params.get("flair_id").and_then(|v| v.as_str()) {
            form.push(("flair_id", flair.to_string()));
        }

        let resp = client
            .post("https://oauth.reddit.com/api/submit")
            .bearer_auth(&creds.access_token)
            .header(
                "User-Agent",
                "AMOS-Social/0.1.0 (by /u/amos-labs)",
            )
            .form(&form)
            .send()
            .await
            .map_err(|e| {
                amos_core::AmosError::Internal(format!("Reddit API request failed: {}", e))
            })?;

        let status = resp.status();
        let resp_body: JsonValue = resp.json().await.unwrap_or(json!({}));

        if !status.is_success() {
            return Ok(ToolResult {
                success: false,
                data: Some(resp_body.clone()),
                error: Some(format!("Reddit API error ({})", status)),
                metadata: Some(json!({ "error_code": "PLATFORM_ERROR" })),
            });
        }

        // Check for Reddit-specific errors in the json response
        if let Some(errors) = resp_body.pointer("/json/errors").and_then(|v| v.as_array()) {
            if !errors.is_empty() {
                let error_msg = errors
                    .first()
                    .and_then(|e| e.as_array())
                    .and_then(|a| a.get(1))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown Reddit error")
                    .to_string();

                let error_code = errors
                    .first()
                    .and_then(|e| e.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("PLATFORM_ERROR")
                    .to_string();

                let code = if error_code == "RATELIMIT" { "RATE_LIMITED" } else { "PLATFORM_ERROR" };
                return Ok(ToolResult {
                    success: false,
                    data: Some(resp_body),
                    error: Some(error_msg),
                    metadata: Some(json!({ "error_code": code })),
                });
            }
        }

        let post_url = resp_body
            .pointer("/json/data/url")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let post_id = resp_body
            .pointer("/json/data/id")
            .or_else(|| resp_body.pointer("/json/data/name"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Record the post
        let _ = super::twitter::record_post(
            &self.db_pool,
            "reddit",
            post_id,
            &format!("{}\n\n{}", title, text.unwrap_or("")),
            post_url,
        )
        .await;

        info!(
            post_id = post_id,
            subreddit = subreddit,
            "Reddit post submitted"
        );

        Ok(ToolResult {
            success: true,
            data: Some(json!({
                "post_id": post_id,
                "url": post_url,
                "subreddit": subreddit,
                "title": title,
                "kind": kind,
                "created_at": chrono::Utc::now().to_rfc3339(),
            })),
            error: None,
            metadata: None,
        })
    }
}

struct RedditCredentials {
    access_token: String,
}

async fn resolve_reddit_credentials(
    db_pool: &PgPool,
    connection_id: &str,
) -> amos_core::Result<RedditCredentials> {
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
            "Reddit connection {} not found. Set up credentials first.",
            connection_id
        ))
    })?;

    let creds: JsonValue = sqlx::Row::try_get(&row, "credentials_data")
        .map_err(|e| amos_core::AmosError::Internal(format!("Credential read error: {}", e)))?;

    let access_token = creds
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            amos_core::AmosError::Internal(
                "No access_token in Reddit credentials. Run OAuth setup first.".into(),
            )
        })?
        .to_string();

    Ok(RedditCredentials { access_token })
}
