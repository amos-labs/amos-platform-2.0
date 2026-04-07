//! LinkedIn posting tool.

use amos_core::tools::{Tool, ToolCategory, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use tracing::info;

pub struct PostLinkedInTool {
    db_pool: PgPool,
}

impl PostLinkedInTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for PostLinkedInTool {
    fn name(&self) -> &str {
        "post_linkedin"
    }

    fn description(&self) -> &str {
        "Post content to LinkedIn (personal profile or company page). \
         Supports text posts up to 3000 characters. Requires a LinkedIn \
         API connection with OAuth 2.0 credentials in the vault."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "connection_id": {
                    "type": "string",
                    "description": "UUID of the LinkedIn API connection"
                },
                "text": {
                    "type": "string",
                    "description": "Post text (max 3000 characters)",
                    "maxLength": 3000
                },
                "visibility": {
                    "type": "string",
                    "enum": ["PUBLIC", "CONNECTIONS"],
                    "description": "Post visibility (default: PUBLIC)",
                    "default": "PUBLIC"
                },
                "post_as": {
                    "type": "string",
                    "enum": ["personal", "organization"],
                    "description": "Post as personal profile or organization page (default: personal)",
                    "default": "personal"
                },
                "organization_id": {
                    "type": "string",
                    "description": "Required if post_as is 'organization'. LinkedIn organization URN."
                }
            },
            "required": ["connection_id", "text"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }

    async fn execute(&self, params: JsonValue) -> amos_core::Result<ToolResult> {
        let text = params
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Internal("Missing 'text' parameter".into()))?;

        let connection_id = params
            .get("connection_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Internal("Missing 'connection_id' parameter".into())
            })?;

        if text.len() > 3000 {
            return Ok(ToolResult {
                success: false,
                data: None,
                error: Some(format!(
                    "LinkedIn post exceeds 3000 characters ({} chars)",
                    text.len()
                )),
                metadata: None,
            });
        }

        let visibility = params
            .get("visibility")
            .and_then(|v| v.as_str())
            .unwrap_or("PUBLIC");
        let post_as = params
            .get("post_as")
            .and_then(|v| v.as_str())
            .unwrap_or("personal");

        // Resolve credentials
        let creds = resolve_linkedin_credentials(&self.db_pool, connection_id).await?;

        // Determine the author URN
        let author = if post_as == "organization" {
            let org_id = params
                .get("organization_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    amos_core::AmosError::Internal(
                        "organization_id required when post_as is 'organization'".into(),
                    )
                })?;
            format!("urn:li:organization:{}", org_id)
        } else {
            // Fetch the user's person URN
            fetch_linkedin_person_urn(&creds.access_token).await?
        };

        // Build the post payload (LinkedIn v2 Posts API)
        let body = json!({
            "author": author,
            "lifecycleState": "PUBLISHED",
            "specificContent": {
                "com.linkedin.ugc.ShareContent": {
                    "shareCommentary": { "text": text },
                    "shareMediaCategory": "NONE"
                }
            },
            "visibility": {
                "com.linkedin.ugc.MemberNetworkVisibility": visibility
            }
        });

        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.linkedin.com/v2/ugcPosts")
            .bearer_auth(&creds.access_token)
            .header("X-Restli-Protocol-Version", "2.0.0")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                amos_core::AmosError::Internal(format!("LinkedIn API request failed: {}", e))
            })?;

        let status = resp.status();

        // LinkedIn returns the post URN in the X-RestLi-Id header or response body
        let headers = resp.headers().clone();
        let resp_body: JsonValue = resp.json().await.unwrap_or(json!({}));

        if !status.is_success() {
            let error_msg = resp_body
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown LinkedIn API error")
                .to_string();

            return Ok(ToolResult {
                success: false,
                data: Some(resp_body),
                error: Some(format!("LinkedIn API error ({}): {}", status, error_msg)),
                metadata: Some(json!({ "error_code": classify_linkedin_error(status.as_u16()) })),
            });
        }

        // Extract post URN from header or body
        let post_urn = headers
            .get("x-restli-id")
            .and_then(|v| v.to_str().ok())
            .or_else(|| resp_body.get("id").and_then(|v| v.as_str()))
            .unwrap_or("unknown");

        let post_url = format!(
            "https://www.linkedin.com/feed/update/{}",
            post_urn
        );

        // Record the post
        let _ = super::twitter::record_post(
            &self.db_pool,
            "linkedin",
            post_urn,
            text,
            &post_url,
        )
        .await;

        info!(post_urn = post_urn, "LinkedIn post published");

        Ok(ToolResult {
            success: true,
            data: Some(json!({
                "post_urn": post_urn,
                "url": post_url,
                "text": text,
                "visibility": visibility,
                "posted_as": post_as,
                "created_at": chrono::Utc::now().to_rfc3339(),
            })),
            error: None,
            metadata: None,
        })
    }
}

struct LinkedInCredentials {
    access_token: String,
}

async fn resolve_linkedin_credentials(
    db_pool: &PgPool,
    connection_id: &str,
) -> amos_core::Result<LinkedInCredentials> {
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
            "LinkedIn connection {} not found. Set up credentials first.",
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
                "No access_token in LinkedIn credentials. Run OAuth setup first.".into(),
            )
        })?
        .to_string();

    Ok(LinkedInCredentials { access_token })
}

async fn fetch_linkedin_person_urn(access_token: &str) -> amos_core::Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.linkedin.com/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| {
            amos_core::AmosError::Internal(format!("Failed to fetch LinkedIn profile: {}", e))
        })?;

    let body: JsonValue = resp.json().await.map_err(|e| {
        amos_core::AmosError::Internal(format!("Failed to parse LinkedIn profile: {}", e))
    })?;

    let sub = body
        .get("sub")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            amos_core::AmosError::Internal(
                "Could not determine LinkedIn user ID from /v2/userinfo".into(),
            )
        })?;

    Ok(format!("urn:li:person:{}", sub))
}

fn classify_linkedin_error(status: u16) -> &'static str {
    match status {
        401 => "AUTH_EXPIRED",
        403 => "AUTH_INVALID",
        429 => "RATE_LIMITED",
        422 => "CONTENT_REJECTED",
        _ => "PLATFORM_ERROR",
    }
}
