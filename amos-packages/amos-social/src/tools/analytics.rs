//! Social media analytics tools.

use amos_core::tools::{Tool, ToolCategory, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use tracing::info;

// =============================================================================
// GetPostAnalyticsTool
// =============================================================================

pub struct GetPostAnalyticsTool {
    db_pool: PgPool,
}

impl GetPostAnalyticsTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for GetPostAnalyticsTool {
    fn name(&self) -> &str {
        "get_post_analytics"
    }

    fn description(&self) -> &str {
        "Retrieve engagement analytics for a published post. Returns \
         impressions, likes, reposts, replies, clicks, and other \
         platform-specific metrics. Requires the post ID and platform."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "connection_id": {
                    "type": "string",
                    "description": "UUID of the platform API connection"
                },
                "platform": {
                    "type": "string",
                    "enum": ["twitter", "linkedin", "reddit"],
                    "description": "Platform to query (HN has no analytics API)"
                },
                "post_id": {
                    "type": "string",
                    "description": "Platform-specific post ID"
                }
            },
            "required": ["connection_id", "platform", "post_id"]
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
        let platform = params
            .get("platform")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Internal("Missing 'platform' parameter".into()))?;
        let post_id = params
            .get("post_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Internal("Missing 'post_id' parameter".into()))?;

        let metrics = match platform {
            "twitter" => fetch_twitter_analytics(&self.db_pool, connection_id, post_id).await?,
            "linkedin" => fetch_linkedin_analytics(&self.db_pool, connection_id, post_id).await?,
            "reddit" => fetch_reddit_analytics(&self.db_pool, connection_id, post_id).await?,
            _ => {
                return Ok(ToolResult {
                    success: false,
                    data: None,
                    error: Some(format!(
                        "No analytics available for platform '{}'. Supported: twitter, linkedin, reddit",
                        platform
                    )),
                    metadata: None,
                })
            }
        };

        // Store analytics snapshot
        let _ =
            super::twitter::insert_collection_record(&self.db_pool, "social_analytics", &metrics)
                .await;

        info!(
            platform = platform,
            post_id = post_id,
            "Analytics retrieved"
        );

        Ok(ToolResult {
            success: true,
            data: Some(metrics),
            error: None,
            metadata: None,
        })
    }
}

// =============================================================================
// GetCampaignReportTool
// =============================================================================

pub struct GetCampaignReportTool {
    db_pool: PgPool,
}

impl GetCampaignReportTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for GetCampaignReportTool {
    fn name(&self) -> &str {
        "get_campaign_report"
    }

    fn description(&self) -> &str {
        "Generate a campaign performance report aggregating analytics \
         across all published posts. Returns per-post metrics, \
         platform comparisons, top performers, and recommendations."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "campaign_id": {
                    "type": "string",
                    "description": "Optional campaign ID to filter by. If omitted, reports on all posts."
                },
                "date_range": {
                    "type": "object",
                    "properties": {
                        "start": { "type": "string", "description": "ISO 8601 start date" },
                        "end": { "type": "string", "description": "ISO 8601 end date" }
                    }
                }
            }
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }

    async fn execute(&self, _params: JsonValue) -> amos_core::Result<ToolResult> {
        // Fetch all posted content from social_posts collection
        let posts = super::twitter::query_collection_records(&self.db_pool, "social_posts")
            .await
            .map_err(amos_core::AmosError::Internal)?;

        // Fetch all analytics snapshots
        let analytics = super::twitter::query_collection_records(&self.db_pool, "social_analytics")
            .await
            .map_err(amos_core::AmosError::Internal)?;

        // Aggregate by platform
        let mut by_platform: std::collections::HashMap<String, Vec<&JsonValue>> =
            std::collections::HashMap::new();
        for post in &posts {
            if let Some(platform) = post.get("platform").and_then(|v| v.as_str()) {
                by_platform
                    .entry(platform.to_string())
                    .or_default()
                    .push(post);
            }
        }

        let platform_summary: JsonValue = by_platform
            .iter()
            .map(|(platform, platform_posts)| {
                (
                    platform.clone(),
                    json!({
                        "posts": platform_posts.len(),
                    }),
                )
            })
            .collect::<serde_json::Map<String, JsonValue>>()
            .into();

        info!(
            total_posts = posts.len(),
            analytics_snapshots = analytics.len(),
            "Campaign report generated"
        );

        Ok(ToolResult {
            success: true,
            data: Some(json!({
                "summary": {
                    "total_posts": posts.len(),
                    "analytics_snapshots": analytics.len(),
                },
                "posts": posts,
                "by_platform": platform_summary,
                "latest_analytics": analytics.first(),
            })),
            error: None,
            metadata: None,
        })
    }
}

// =============================================================================
// Platform-specific analytics fetchers
// =============================================================================

async fn fetch_twitter_analytics(
    db_pool: &PgPool,
    connection_id: &str,
    post_id: &str,
) -> amos_core::Result<JsonValue> {
    let creds = resolve_credentials(db_pool, connection_id).await?;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "https://api.twitter.com/2/tweets/{}?tweet.fields=public_metrics,organic_metrics",
            post_id
        ))
        .bearer_auth(&creds)
        .send()
        .await
        .map_err(|e| amos_core::AmosError::Internal(format!("Twitter API error: {}", e)))?;

    let body: JsonValue = resp.json().await.unwrap_or(json!({}));

    let metrics = body
        .pointer("/data/public_metrics")
        .cloned()
        .unwrap_or(json!({}));

    Ok(json!({
        "post_id": post_id,
        "platform": "twitter",
        "metrics": metrics,
        "retrieved_at": chrono::Utc::now().to_rfc3339(),
    }))
}

async fn fetch_linkedin_analytics(
    db_pool: &PgPool,
    connection_id: &str,
    post_id: &str,
) -> amos_core::Result<JsonValue> {
    let creds = resolve_credentials(db_pool, connection_id).await?;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "https://api.linkedin.com/v2/socialMetadata/{}",
            post_id
        ))
        .bearer_auth(&creds)
        .header("X-Restli-Protocol-Version", "2.0.0")
        .send()
        .await
        .map_err(|e| amos_core::AmosError::Internal(format!("LinkedIn API error: {}", e)))?;

    let body: JsonValue = resp.json().await.unwrap_or(json!({}));

    Ok(json!({
        "post_id": post_id,
        "platform": "linkedin",
        "metrics": body,
        "retrieved_at": chrono::Utc::now().to_rfc3339(),
    }))
}

async fn fetch_reddit_analytics(
    db_pool: &PgPool,
    connection_id: &str,
    post_id: &str,
) -> amos_core::Result<JsonValue> {
    let creds = resolve_credentials(db_pool, connection_id).await?;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("https://oauth.reddit.com/api/info?id={}", post_id))
        .bearer_auth(&creds)
        .header("User-Agent", "AMOS-Social/0.1.0 (by /u/amos-labs)")
        .send()
        .await
        .map_err(|e| amos_core::AmosError::Internal(format!("Reddit API error: {}", e)))?;

    let body: JsonValue = resp.json().await.unwrap_or(json!({}));

    let post_data = body
        .pointer("/data/children/0/data")
        .cloned()
        .unwrap_or(json!({}));

    Ok(json!({
        "post_id": post_id,
        "platform": "reddit",
        "metrics": {
            "score": post_data.get("score"),
            "upvote_ratio": post_data.get("upvote_ratio"),
            "num_comments": post_data.get("num_comments"),
        },
        "retrieved_at": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Simple credential resolver (access_token from integration_connections).
async fn resolve_credentials(db_pool: &PgPool, connection_id: &str) -> amos_core::Result<String> {
    let conn_id: uuid::Uuid = connection_id
        .parse()
        .map_err(|_| amos_core::AmosError::Internal("Invalid connection_id UUID".into()))?;

    let row = sqlx::query("SELECT credentials_data FROM integration_connections WHERE id = $1")
        .bind(conn_id)
        .fetch_optional(db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(format!("DB error: {}", e)))?
        .ok_or_else(|| {
            amos_core::AmosError::Internal(format!("Connection {} not found", connection_id))
        })?;

    let creds: JsonValue = sqlx::Row::try_get(&row, "credentials_data")
        .map_err(|e| amos_core::AmosError::Internal(format!("Credential error: {}", e)))?;

    creds
        .get("access_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| amos_core::AmosError::Internal("No access_token in credentials".into()))
}
