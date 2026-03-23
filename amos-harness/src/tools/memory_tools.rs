//! Working memory tools with optional semantic embedding support

use super::{Tool, ToolCategory, ToolResult};
use amos_core::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value as JsonValue};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

use crate::embeddings::EmbeddingService;

/// Save something to working memory
pub struct RememberThisTool {
    db_pool: PgPool,
    embedding_service: Option<Arc<EmbeddingService>>,
}

impl RememberThisTool {
    pub fn new(db_pool: PgPool, embedding_service: Option<Arc<EmbeddingService>>) -> Self {
        Self {
            db_pool,
            embedding_service,
        }
    }
}

#[async_trait]
impl Tool for RememberThisTool {
    fn name(&self) -> &str {
        "remember_this"
    }

    fn description(&self) -> &str {
        "Save important information to working memory for future reference. Supports semantic search if embeddings are enabled."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "What to remember"
                },
                "category": {
                    "type": "string",
                    "description": "Category for organization (e.g., 'user_preference', 'business_rule', 'context')"
                },
                "importance": {
                    "type": "integer",
                    "description": "Importance level (1-10)",
                    "minimum": 1,
                    "maximum": 10,
                    "default": 5
                }
            },
            "required": ["content"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let content = params["content"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("content is required".to_string()))?;

        let category = params
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let importance = params
            .get("importance")
            .and_then(|v| v.as_i64())
            .unwrap_or(5) as f64;

        // Calculate initial salience based on importance
        let salience = importance / 10.0;

        // Store in memory table
        let result = sqlx::query(
            r#"
            INSERT INTO memory_entries (content, category, salience, metadata, created_at, last_accessed_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            "#,
        )
        .bind(content)
        .bind(category)
        .bind(salience)
        .bind(json!({ "importance": importance }))
        .bind(Utc::now())
        .bind(Utc::now())
        .fetch_one(&self.db_pool)
        .await;

        match result {
            Ok(row) => {
                let id: Uuid = row.get(0);

                // Generate and store embedding in the background if service is available
                if let Some(ref embedding_svc) = self.embedding_service {
                    let svc = embedding_svc.clone();
                    let pool = self.db_pool.clone();
                    let text = content.to_string();
                    tokio::spawn(async move {
                        match svc.embed(&text).await {
                            Ok(embedding) => {
                                let embedding_json = serde_json::to_value(&embedding).ok();
                                if let Some(emb_val) = embedding_json {
                                    let _ = sqlx::query(
                                        "UPDATE memory_entries SET embedding = $1::vector WHERE id = $2",
                                    )
                                    .bind(emb_val.to_string())
                                    .bind(id)
                                    .execute(&pool)
                                    .await;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to generate embedding for memory {}: {}",
                                    id,
                                    e
                                );
                            }
                        }
                    });
                }

                Ok(ToolResult::success(json!({
                    "memory_id": id.to_string(),
                    "saved": true,
                    "has_embedding": self.embedding_service.is_some(),
                    "message": "Memory saved successfully"
                })))
            }
            Err(e) => Ok(ToolResult::error(format!("Failed to save memory: {}", e))),
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }
}

/// Search working memory
pub struct SearchMemoryTool {
    db_pool: PgPool,
    embedding_service: Option<Arc<EmbeddingService>>,
}

impl SearchMemoryTool {
    pub fn new(db_pool: PgPool, embedding_service: Option<Arc<EmbeddingService>>) -> Self {
        Self {
            db_pool,
            embedding_service,
        }
    }
}

#[async_trait]
impl Tool for SearchMemoryTool {
    fn name(&self) -> &str {
        "search_memory"
    }

    fn description(&self) -> &str {
        "Search working memory for previously saved information. Uses semantic search when embeddings are available, falls back to text matching."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "category": {
                    "type": "string",
                    "description": "Filter by category"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results",
                    "default": 10
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let query = params["query"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("query is required".to_string()))?;

        let category = params.get("category").and_then(|v| v.as_str());
        let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(10);

        // Try semantic search first if embedding service is available
        if let Some(ref embedding_svc) = self.embedding_service {
            if let Ok(query_embedding) = embedding_svc.embed(query).await {
                let embedding_str = serde_json::to_string(&query_embedding).unwrap_or_default();
                let rows = if let Some(cat) = category {
                    sqlx::query(
                        r#"
                        SELECT id, content, category, salience,
                               1 - (embedding <=> $1::vector) AS similarity
                        FROM memory_entries
                        WHERE embedding IS NOT NULL AND category = $2
                        ORDER BY embedding <=> $1::vector
                        LIMIT $3
                        "#,
                    )
                    .bind(&embedding_str)
                    .bind(cat)
                    .bind(limit)
                    .fetch_all(&self.db_pool)
                    .await
                } else {
                    sqlx::query(
                        r#"
                        SELECT id, content, category, salience,
                               1 - (embedding <=> $1::vector) AS similarity
                        FROM memory_entries
                        WHERE embedding IS NOT NULL
                        ORDER BY embedding <=> $1::vector
                        LIMIT $2
                        "#,
                    )
                    .bind(&embedding_str)
                    .bind(limit)
                    .fetch_all(&self.db_pool)
                    .await
                };

                if let Ok(rows) = rows {
                    let mut results = Vec::new();
                    for row in &rows {
                        let id: Uuid = row.get("id");
                        let content: String = row.get("content");
                        let cat: String = row.get("category");
                        let salience: f64 = row.get("salience");
                        let similarity: f64 = row.get("similarity");

                        results.push(json!({
                            "id": id.to_string(),
                            "content": content,
                            "category": cat,
                            "salience": salience,
                            "similarity": similarity
                        }));

                        // Reinforce accessed memory
                        let _ = sqlx::query(
                            "UPDATE memory_entries SET last_accessed_at = $1, access_count = access_count + 1 WHERE id = $2"
                        )
                        .bind(Utc::now())
                        .bind(id)
                        .execute(&self.db_pool)
                        .await;
                    }

                    return Ok(ToolResult::success(json!({
                        "results": results,
                        "count": results.len(),
                        "search_type": "semantic"
                    })));
                }
            }
        }

        // Fallback: text search using ILIKE
        let search_pattern = format!("%{}%", query);
        let rows = if let Some(cat) = category {
            sqlx::query(
                r#"
                SELECT id, content, category, salience, created_at
                FROM memory_entries
                WHERE category = $1 AND content ILIKE $2
                ORDER BY salience DESC, last_accessed_at DESC
                LIMIT $3
                "#,
            )
            .bind(cat)
            .bind(&search_pattern)
            .bind(limit)
            .fetch_all(&self.db_pool)
            .await
        } else {
            sqlx::query(
                r#"
                SELECT id, content, category, salience, created_at
                FROM memory_entries
                WHERE content ILIKE $1
                ORDER BY salience DESC, last_accessed_at DESC
                LIMIT $2
                "#,
            )
            .bind(&search_pattern)
            .bind(limit)
            .fetch_all(&self.db_pool)
            .await
        };

        match rows {
            Ok(rows) => {
                let mut results = Vec::new();
                for row in &rows {
                    let id: Uuid = row.get("id");
                    let content: String = row.get("content");
                    let cat: String = row.get("category");
                    let salience: f64 = row.get("salience");

                    results.push(json!({
                        "id": id.to_string(),
                        "content": content,
                        "category": cat,
                        "salience": salience
                    }));

                    // Update last_accessed to reinforce the memory
                    let _ = sqlx::query(
                        "UPDATE memory_entries SET last_accessed_at = $1, access_count = access_count + 1 WHERE id = $2",
                    )
                    .bind(Utc::now())
                    .bind(id)
                    .execute(&self.db_pool)
                    .await;
                }

                Ok(ToolResult::success(json!({
                    "results": results,
                    "count": results.len(),
                    "search_type": "text"
                })))
            }
            Err(e) => Ok(ToolResult::error(format!("Memory search failed: {}", e))),
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }
}
