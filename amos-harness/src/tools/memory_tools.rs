//! Working memory tools

use super::{Tool, ToolCategory, ToolResult};
use amos_core::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value as JsonValue};
use sqlx::{PgPool, Row};

/// Save something to working memory
pub struct RememberThisTool {
    db_pool: PgPool,
}

impl RememberThisTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for RememberThisTool {
    fn name(&self) -> &str {
        "remember_this"
    }

    fn description(&self) -> &str {
        "Save important information to working memory for future reference"
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
            INSERT INTO working_memory (content, category, salience, metadata, created_at, last_accessed)
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
                let id: i32 = row.get(0);
                Ok(ToolResult::success(json!({
                    "memory_id": id,
                    "saved": true,
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
}

impl SearchMemoryTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for SearchMemoryTool {
    fn name(&self) -> &str {
        "search_memory"
    }

    fn description(&self) -> &str {
        "Search working memory for previously saved information"
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

        // Search memory using text search
        let sql = if let Some(_cat) = category {
            r#"
                SELECT id, content, category, salience, created_at
                FROM working_memory
                WHERE category = $1 AND content ILIKE $2
                ORDER BY salience DESC, last_accessed DESC
                LIMIT $3
                "#
            .to_string()
        } else {
            r#"
                SELECT id, content, category, salience, created_at
                FROM working_memory
                WHERE content ILIKE $1
                ORDER BY salience DESC, last_accessed DESC
                LIMIT $2
                "#
            .to_string()
        };

        let search_pattern = format!("%{}%", query);

        let rows = if let Some(cat) = category {
            sqlx::query(&sql)
                .bind(cat)
                .bind(&search_pattern)
                .bind(limit)
                .fetch_all(&self.db_pool)
                .await
        } else {
            sqlx::query(&sql)
                .bind(&search_pattern)
                .bind(limit)
                .fetch_all(&self.db_pool)
                .await
        };

        match rows {
            Ok(rows) => {
                let mut results = Vec::new();
                for row in rows {
                    let id: i32 = row.get(0);
                    let content: String = row.get(1);
                    let category: String = row.get(2);
                    let salience: f64 = row.get(3);

                    results.push(json!({
                        "id": id,
                        "content": content,
                        "category": category,
                        "salience": salience
                    }));

                    // Update last_accessed to reinforce the memory
                    let _ = sqlx::query(
                        "UPDATE working_memory SET last_accessed = $1, access_count = access_count + 1 WHERE id = $2"
                    )
                    .bind(Utc::now())
                    .bind(id)
                    .execute(&self.db_pool)
                    .await;
                }

                Ok(ToolResult::success(json!({
                    "results": results,
                    "count": results.len()
                })))
            }
            Err(e) => Ok(ToolResult::error(format!("Memory search failed: {}", e))),
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Memory
    }
}
