//! Twitter/X posting tools.

use amos_core::tools::{Tool, ToolCategory, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use tracing::{info, warn};

// =============================================================================
// PostTweetTool
// =============================================================================

pub struct PostTweetTool {
    db_pool: PgPool,
}

impl PostTweetTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for PostTweetTool {
    fn name(&self) -> &str {
        "post_tweet"
    }

    fn description(&self) -> &str {
        "Post a single tweet to Twitter/X. Requires a Twitter API connection \
         with OAuth 2.0 credentials stored in the vault. Supports text up to \
         280 characters. Returns the tweet ID and URL on success."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "connection_id": {
                    "type": "string",
                    "description": "UUID of the Twitter/X API connection"
                },
                "text": {
                    "type": "string",
                    "description": "Tweet text (max 280 characters)",
                    "maxLength": 280
                },
                "reply_to": {
                    "type": "string",
                    "description": "Optional: Tweet ID to reply to"
                },
                "quote_tweet_id": {
                    "type": "string",
                    "description": "Optional: Tweet ID to quote"
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

        // Validate text length
        if text.len() > 280 {
            return Ok(ToolResult {
                success: false,
                data: None,
                error: Some(format!(
                    "Tweet text exceeds 280 characters ({} chars)",
                    text.len()
                )),
                metadata: None,
            });
        }

        // Resolve credentials from vault
        let creds = resolve_twitter_credentials(&self.db_pool, connection_id).await?;

        // Build request
        let mut body = json!({ "text": text });
        if let Some(reply_to) = params.get("reply_to").and_then(|v| v.as_str()) {
            body["reply"] = json!({ "in_reply_to_tweet_id": reply_to });
        }
        if let Some(quote_id) = params.get("quote_tweet_id").and_then(|v| v.as_str()) {
            body["quote_tweet_id"] = json!(quote_id);
        }

        // POST to Twitter API v2
        let client = reqwest::Client::new();
        let resp = client
            .post("https://api.twitter.com/2/tweets")
            .bearer_auth(&creds.access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                amos_core::AmosError::Internal(format!("Twitter API request failed: {}", e))
            })?;

        let status = resp.status();
        let resp_body: JsonValue = resp.json().await.unwrap_or(json!({}));

        if !status.is_success() {
            let error_msg = resp_body
                .get("detail")
                .or_else(|| resp_body.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown Twitter API error")
                .to_string();

            return Ok(ToolResult {
                success: false,
                data: Some(resp_body),
                error: Some(format!("Twitter API error ({}): {}", status, error_msg)),
                metadata: Some(json!({ "error_code": classify_twitter_error(status.as_u16()) })),
            });
        }

        let tweet_id = resp_body
            .pointer("/data/id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Record the post in social_posts schema
        let _ = record_post(
            &self.db_pool,
            "twitter",
            tweet_id,
            text,
            &format!("https://x.com/i/status/{}", tweet_id),
        )
        .await;

        info!(tweet_id = tweet_id, "Tweet posted successfully");

        Ok(ToolResult {
            success: true,
            data: Some(json!({
                "tweet_id": tweet_id,
                "url": format!("https://x.com/i/status/{}", tweet_id),
                "text": text,
                "created_at": chrono::Utc::now().to_rfc3339(),
            })),
            error: None,
            metadata: None,
        })
    }
}

// =============================================================================
// PostThreadTool
// =============================================================================

pub struct PostThreadTool {
    db_pool: PgPool,
}

impl PostThreadTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for PostThreadTool {
    fn name(&self) -> &str {
        "post_thread"
    }

    fn description(&self) -> &str {
        "Post a multi-tweet thread to Twitter/X. Takes an array of tweet texts \
         and posts them sequentially as replies to each other. Each tweet must be \
         ≤280 characters. Returns all tweet IDs and URLs. If any tweet fails, \
         returns partial results with error details."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "connection_id": {
                    "type": "string",
                    "description": "UUID of the Twitter/X API connection"
                },
                "tweets": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": {
                                "type": "string",
                                "description": "Tweet text (max 280 characters)",
                                "maxLength": 280
                            }
                        },
                        "required": ["text"]
                    },
                    "description": "Array of tweets in thread order. First tweet is the root.",
                    "minItems": 2,
                    "maxItems": 25
                }
            },
            "required": ["connection_id", "tweets"]
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

        let tweets = params
            .get("tweets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                amos_core::AmosError::Internal("Missing 'tweets' array parameter".into())
            })?;

        if tweets.len() < 2 {
            return Ok(ToolResult {
                success: false,
                data: None,
                error: Some("Thread must contain at least 2 tweets".into()),
                metadata: None,
            });
        }

        // Validate all tweet lengths up front
        for (i, tweet) in tweets.iter().enumerate() {
            let text = tweet.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if text.len() > 280 {
                return Ok(ToolResult {
                    success: false,
                    data: None,
                    error: Some(format!(
                        "Tweet {} exceeds 280 characters ({} chars)",
                        i + 1,
                        text.len()
                    )),
                    metadata: None,
                });
            }
        }

        let creds = resolve_twitter_credentials(&self.db_pool, connection_id).await?;
        let client = reqwest::Client::new();

        let mut posted: Vec<JsonValue> = Vec::new();
        let mut prev_tweet_id: Option<String> = None;

        for (i, tweet) in tweets.iter().enumerate() {
            let text = tweet.get("text").and_then(|v| v.as_str()).unwrap_or("");

            let mut body = json!({ "text": text });
            if let Some(ref reply_id) = prev_tweet_id {
                body["reply"] = json!({ "in_reply_to_tweet_id": reply_id });
            }

            let resp = client
                .post("https://api.twitter.com/2/tweets")
                .bearer_auth(&creds.access_token)
                .json(&body)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    let resp_body: JsonValue = r.json().await.unwrap_or(json!({}));
                    let tweet_id = resp_body
                        .pointer("/data/id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let url = format!("https://x.com/i/status/{}", tweet_id);
                    posted.push(json!({
                        "tweet_id": tweet_id,
                        "url": url,
                        "text": text,
                        "position": i + 1,
                    }));
                    prev_tweet_id = Some(tweet_id);
                }
                Ok(r) => {
                    let status = r.status();
                    let err_body: JsonValue = r.json().await.unwrap_or(json!({}));
                    warn!(
                        position = i + 1,
                        status = %status,
                        "Thread interrupted at tweet {}",
                        i + 1
                    );

                    return Ok(ToolResult {
                        success: false,
                        data: Some(json!({
                            "thread_id": posted.first().and_then(|t| t.get("tweet_id")).and_then(|v| v.as_str()),
                            "tweets": posted,
                            "total_tweets": tweets.len(),
                            "completed_tweets": i,
                            "status": "partial",
                            "error": format!("Failed at tweet {} ({}): {:?}", i + 1, status, err_body),
                            "resume_from": i + 1,
                        })),
                        error: Some(format!("Thread interrupted at tweet {}", i + 1)),
                        metadata: None,
                    });
                }
                Err(e) => {
                    return Ok(ToolResult {
                        success: false,
                        data: Some(json!({
                            "tweets": posted,
                            "completed_tweets": i,
                            "status": "partial",
                            "error": format!("Network error at tweet {}: {}", i + 1, e),
                            "resume_from": i + 1,
                        })),
                        error: Some(format!("Network error at tweet {}: {}", i + 1, e)),
                        metadata: None,
                    });
                }
            }

            // Rate limit delay between tweets (skip after last)
            if i < tweets.len() - 1 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }

        let thread_id = posted
            .first()
            .and_then(|t| t.get("tweet_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Record the thread in social_posts schema
        let full_text: Vec<&str> = tweets
            .iter()
            .filter_map(|t| t.get("text").and_then(|v| v.as_str()))
            .collect();
        let _ = record_post(
            &self.db_pool,
            "twitter",
            thread_id,
            &full_text.join("\n---\n"),
            &format!("https://x.com/i/status/{}", thread_id),
        )
        .await;

        info!(
            thread_id = thread_id,
            tweet_count = posted.len(),
            "Thread posted successfully"
        );

        Ok(ToolResult {
            success: true,
            data: Some(json!({
                "thread_id": thread_id,
                "thread_url": format!("https://x.com/i/status/{}", thread_id),
                "tweets": posted,
                "total_tweets": tweets.len(),
                "status": "complete",
            })),
            error: None,
            metadata: None,
        })
    }
}

// =============================================================================
// Helpers
// =============================================================================

struct TwitterCredentials {
    access_token: String,
}

async fn resolve_twitter_credentials(
    db_pool: &PgPool,
    connection_id: &str,
) -> amos_core::Result<TwitterCredentials> {
    let conn_id: uuid::Uuid = connection_id.parse().map_err(|_| {
        amos_core::AmosError::Internal("Invalid connection_id UUID".into())
    })?;

    // Look up the connection's credentials
    let row = sqlx::query(
        "SELECT credentials_data FROM integration_connections WHERE id = $1",
    )
    .bind(conn_id)
    .fetch_optional(db_pool)
    .await
    .map_err(|e| amos_core::AmosError::Internal(format!("DB error: {}", e)))?
    .ok_or_else(|| {
        amos_core::AmosError::Internal(format!(
            "Twitter connection {} not found. Set up credentials first.",
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
                "No access_token in Twitter credentials. Run OAuth setup first.".into(),
            )
        })?
        .to_string();

    Ok(TwitterCredentials { access_token })
}

fn classify_twitter_error(status: u16) -> &'static str {
    match status {
        401 => "AUTH_EXPIRED",
        403 => "AUTH_INVALID",
        429 => "RATE_LIMITED",
        422 => "CONTENT_REJECTED",
        _ => "PLATFORM_ERROR",
    }
}

pub(crate) async fn record_post(
    db_pool: &PgPool,
    platform: &str,
    post_id: &str,
    text: &str,
    url: &str,
) -> Result<(), ()> {
    // Look up the social_posts collection ID
    let collection_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM collections WHERE name = 'social_posts'",
    )
    .fetch_optional(db_pool)
    .await
    .ok()
    .flatten();

    if let Some(cid) = collection_id {
        let _ = sqlx::query(
            "INSERT INTO records (id, collection_id, data, created_at, updated_at)
             VALUES ($1, $2, $3, NOW(), NOW())",
        )
        .bind(uuid::Uuid::new_v4())
        .bind(cid)
        .bind(json!({
            "platform": platform,
            "post_id": post_id,
            "text": text,
            "url": url,
            "posted_at": chrono::Utc::now().to_rfc3339(),
            "status": "posted",
        }))
        .execute(db_pool)
        .await;
    }
    Ok(())
}

/// Helper to insert a record into any social collection by name.
pub(crate) async fn insert_collection_record(
    db_pool: &PgPool,
    collection_name: &str,
    data: &serde_json::Value,
) -> Result<uuid::Uuid, String> {
    let collection_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM collections WHERE name = $1",
    )
    .bind(collection_name)
    .fetch_optional(db_pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("Collection '{}' not found. Is the social package activated?", collection_name))?;

    let record_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO records (id, collection_id, data, created_at, updated_at)
         VALUES ($1, $2, $3, NOW(), NOW())",
    )
    .bind(record_id)
    .bind(collection_id)
    .bind(data)
    .execute(db_pool)
    .await
    .map_err(|e| format!("Insert error: {}", e))?;

    Ok(record_id)
}

/// Helper to query records from a social collection by name.
pub(crate) async fn query_collection_records(
    db_pool: &PgPool,
    collection_name: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let rows = sqlx::query(
        "SELECT r.data FROM records r
         JOIN collections c ON r.collection_id = c.id
         WHERE c.name = $1
         ORDER BY r.created_at",
    )
    .bind(collection_name)
    .fetch_all(db_pool)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(rows
        .iter()
        .filter_map(|row| sqlx::Row::try_get::<serde_json::Value, _>(row, "data").ok())
        .collect())
}
