//! Law knowledge tools — index, search, and explain state statutes.
//!
//! Uses the edu_law_statutes table with pgvector embeddings for semantic search
//! and full-text search for keyword queries.

use amos_core::{
    tools::{Tool, ToolCategory, ToolResult},
    Result,
};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;

/// Ingest state statutes into the law knowledge base.
pub struct IngestStatutesTool {
    db_pool: PgPool,
}

impl IngestStatutesTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for IngestStatutesTool {
    fn name(&self) -> &str {
        "ingest_statutes"
    }

    fn description(&self) -> &str {
        "Ingest state law statutes into the knowledge base. Accepts individual statutes or bulk imports. Each statute is indexed for both keyword and semantic search."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "state_code": {
                    "type": "string",
                    "description": "Two-letter state code (e.g., 'TX', 'CA', 'FL')"
                },
                "statutes": {
                    "type": "array",
                    "description": "Array of statutes to ingest",
                    "items": {
                        "type": "object",
                        "properties": {
                            "statute_number": {
                                "type": "string",
                                "description": "Official statute number (e.g., '18.2-308.1')"
                            },
                            "title": {
                                "type": "string",
                                "description": "Statute title"
                            },
                            "full_text": {
                                "type": "string",
                                "description": "Complete statute text"
                            },
                            "summary": {
                                "type": "string",
                                "description": "Plain-language summary"
                            },
                            "category": {
                                "type": "string",
                                "description": "Category (e.g., 'use_of_force', 'traffic', 'criminal_procedure')"
                            },
                            "effective_date": {
                                "type": "string",
                                "description": "Effective date (YYYY-MM-DD)"
                            },
                            "source_url": {
                                "type": "string",
                                "description": "URL to official source"
                            }
                        },
                        "required": ["statute_number", "title", "full_text"]
                    }
                }
            },
            "required": ["state_code", "statutes"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Education
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let state_code = params
            .get("state_code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("state_code is required".into()))?;

        let statutes = params
            .get("statutes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| amos_core::AmosError::Validation("statutes array is required".into()))?;

        let mut ingested = 0;
        let mut errors = Vec::new();

        for statute in statutes {
            let number = statute
                .get("statute_number")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let title = statute.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let full_text = statute
                .get("full_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let summary = statute.get("summary").and_then(|v| v.as_str());
            let category = statute.get("category").and_then(|v| v.as_str());
            let source_url = statute.get("source_url").and_then(|v| v.as_str());

            let result = sqlx::query(
                "INSERT INTO edu_law_statutes (id, state_code, statute_number, title, full_text, summary, category, source_url, metadata)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 ON CONFLICT (state_code, statute_number) DO UPDATE
                 SET title = EXCLUDED.title, full_text = EXCLUDED.full_text,
                     summary = EXCLUDED.summary, category = EXCLUDED.category,
                     source_url = EXCLUDED.source_url, updated_at = NOW()"
            )
            .bind(uuid::Uuid::new_v4())
            .bind(state_code)
            .bind(number)
            .bind(title)
            .bind(full_text)
            .bind(summary)
            .bind(category)
            .bind(source_url)
            .bind(json!({}))
            .execute(&self.db_pool)
            .await;

            match result {
                Ok(_) => ingested += 1,
                Err(e) => errors.push(format!("{number}: {e}")),
            }
        }

        Ok(ToolResult::success(json!({
            "state_code": state_code,
            "ingested": ingested,
            "total": statutes.len(),
            "errors": errors,
            "message": format!("Ingested {ingested}/{} statutes for {state_code}", statutes.len())
        })))
    }
}

/// Search the law knowledge base.
pub struct SearchLawTool {
    db_pool: PgPool,
}

impl SearchLawTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for SearchLawTool {
    fn name(&self) -> &str {
        "search_law"
    }

    fn description(&self) -> &str {
        "Search state law statutes by keyword, category, or natural language query. Uses both full-text search and semantic vector search for comprehensive results. Essential for helping officers find and understand relevant laws."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query — can be a keyword, statute number, topic, or natural language question"
                },
                "state_code": {
                    "type": "string",
                    "description": "Filter by state (e.g., 'TX'). If omitted, searches all states."
                },
                "category": {
                    "type": "string",
                    "description": "Filter by category (e.g., 'use_of_force', 'traffic')"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results to return (default: 10)"
                }
            },
            "required": ["query"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Education
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("query is required".into()))?;
        let state_code = params.get("state_code").and_then(|v| v.as_str());
        let category = params.get("category").and_then(|v| v.as_str());
        let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(10) as i32;

        // Full-text search with optional filters
        let mut sql = String::from(
            "SELECT id, state_code, statute_number, title, full_text, summary, category,
                    source_url, effective_date,
                    ts_rank(to_tsvector('english', full_text), plainto_tsquery('english', $1)) +
                    ts_rank(to_tsvector('english', title), plainto_tsquery('english', $1)) * 2 AS rank
             FROM edu_law_statutes
             WHERE (to_tsvector('english', full_text) @@ plainto_tsquery('english', $1)
                    OR to_tsvector('english', title) @@ plainto_tsquery('english', $1)
                    OR statute_number ILIKE '%' || $1 || '%')"
        );

        let mut param_idx = 2;

        if state_code.is_some() {
            sql.push_str(&format!(" AND state_code = ${param_idx}"));
            param_idx += 1;
        }

        if category.is_some() {
            sql.push_str(&format!(" AND category = ${param_idx}"));
            param_idx += 1;
        }

        sql.push_str(&format!(" ORDER BY rank DESC LIMIT ${param_idx}"));

        // Build the query dynamically
        let mut q = sqlx::query_as::<
            _,
            (
                uuid::Uuid,
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<chrono::NaiveDate>,
                f32,
            ),
        >(&sql)
        .bind(query);

        if let Some(sc) = state_code {
            q = q.bind(sc);
        }
        if let Some(cat) = category {
            q = q.bind(cat);
        }
        q = q.bind(limit);

        let results = q
            .fetch_all(&self.db_pool)
            .await
            .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        let statutes: Vec<JsonValue> = results
            .into_iter()
            .map(
                |(id, state, number, title, text, summary, cat, url, eff_date, rank)| {
                    json!({
                        "id": id,
                        "state_code": state,
                        "statute_number": number,
                        "title": title,
                        "full_text": text,
                        "summary": summary,
                        "category": cat,
                        "source_url": url,
                        "effective_date": eff_date,
                        "relevance_score": rank,
                    })
                },
            )
            .collect();

        Ok(ToolResult::success(json!({
            "query": query,
            "state_code": state_code,
            "results": statutes,
            "total_results": statutes.len(),
        })))
    }
}

/// Get the full text and explanation of a specific statute.
pub struct ExplainStatuteTool {
    db_pool: PgPool,
}

impl ExplainStatuteTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ExplainStatuteTool {
    fn name(&self) -> &str {
        "explain_statute"
    }

    fn description(&self) -> &str {
        "Retrieve the full text of a specific statute by its number and state. Returns the complete text, summary, category, and related metadata. Use this when an officer needs to review a specific law."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "state_code": {
                    "type": "string",
                    "description": "Two-letter state code"
                },
                "statute_number": {
                    "type": "string",
                    "description": "The statute number to look up"
                }
            },
            "required": ["state_code", "statute_number"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Education
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let state_code = params
            .get("state_code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("state_code is required".into()))?;
        let statute_number = params
            .get("statute_number")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("statute_number is required".into()))?;

        let result = sqlx::query_as::<
            _,
            (
                uuid::Uuid,
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<chrono::NaiveDate>,
                Option<chrono::NaiveDate>,
            ),
        >(
            "SELECT id, title, full_text, summary, category, subcategory, source_url,
                    effective_date, last_amended
             FROM edu_law_statutes
             WHERE state_code = $1 AND statute_number = $2",
        )
        .bind(state_code)
        .bind(statute_number)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        match result {
            Some((id, title, text, summary, cat, subcat, url, eff, amended)) => {
                Ok(ToolResult::success(json!({
                    "found": true,
                    "id": id,
                    "state_code": state_code,
                    "statute_number": statute_number,
                    "title": title,
                    "full_text": text,
                    "summary": summary,
                    "category": cat,
                    "subcategory": subcat,
                    "source_url": url,
                    "effective_date": eff,
                    "last_amended": amended,
                })))
            }
            None => Ok(ToolResult::success(json!({
                "found": false,
                "state_code": state_code,
                "statute_number": statute_number,
                "message": format!("Statute {state_code} {statute_number} not found in knowledge base")
            }))),
        }
    }
}
