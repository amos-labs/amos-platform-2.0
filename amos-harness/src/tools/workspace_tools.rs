//! Workspace self-awareness tools.
//!
//! Gives the agent a summary of what's been built in this harness instance
//! — collections, canvases, sites, and knowledge base entries.

use super::{Tool, ToolCategory, ToolResult};
use amos_core::Result;
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::{PgPool, Row};

/// Get a summary of the current workspace: collections, canvases, sites, knowledge base.
pub struct GetWorkspaceSummaryTool {
    db_pool: PgPool,
}

impl GetWorkspaceSummaryTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for GetWorkspaceSummaryTool {
    fn name(&self) -> &str {
        "get_workspace_summary"
    }

    fn description(&self) -> &str {
        "Get a summary of everything built in this workspace: collections (with field and record counts), canvases, sites, and knowledge base stats. Call this at the start of every conversation to understand what already exists."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _params: JsonValue) -> Result<ToolResult> {
        // Query collections with field count and record count
        let collections = sqlx::query(
            r#"
            SELECT
                c.name,
                c.display_name,
                jsonb_array_length(COALESCE(c.fields, '[]'::jsonb)) AS field_count,
                (SELECT COUNT(*) FROM records r WHERE r.collection_id = c.id) AS record_count
            FROM collections c
            ORDER BY c.name
            "#,
        )
        .fetch_all(&self.db_pool)
        .await;

        let collections_json: Vec<JsonValue> = match collections {
            Ok(rows) => rows
                .iter()
                .map(|row| {
                    json!({
                        "name": row.get::<String, _>("name"),
                        "display_name": row.get::<Option<String>, _>("display_name"),
                        "field_count": row.get::<Option<i32>, _>("field_count").unwrap_or(0),
                        "record_count": row.get::<Option<i64>, _>("record_count").unwrap_or(0),
                    })
                })
                .collect(),
            Err(e) => {
                tracing::warn!("Failed to query collections: {}", e);
                Vec::new()
            }
        };

        // Query non-system canvases
        let canvases = sqlx::query(
            r#"
            SELECT slug, name, canvas_type
            FROM canvases
            WHERE is_system = false
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.db_pool)
        .await;

        let canvases_json: Vec<JsonValue> = match canvases {
            Ok(rows) => rows
                .iter()
                .map(|row| {
                    json!({
                        "slug": row.get::<String, _>("slug"),
                        "name": row.get::<String, _>("name"),
                        "canvas_type": row.get::<String, _>("canvas_type"),
                    })
                })
                .collect(),
            Err(e) => {
                tracing::warn!("Failed to query canvases: {}", e);
                Vec::new()
            }
        };

        // Query sites
        let sites = sqlx::query(
            r#"
            SELECT slug, name, is_published
            FROM sites
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.db_pool)
        .await;

        let sites_json: Vec<JsonValue> = match sites {
            Ok(rows) => rows
                .iter()
                .map(|row| {
                    json!({
                        "slug": row.get::<String, _>("slug"),
                        "name": row.get::<String, _>("name"),
                        "is_published": row.get::<bool, _>("is_published"),
                    })
                })
                .collect(),
            Err(e) => {
                tracing::warn!("Failed to query sites: {}", e);
                Vec::new()
            }
        };

        // Query knowledge base stats
        let kb_stats = sqlx::query(
            r#"
            SELECT
                COUNT(*) AS total_entries,
                COUNT(*) FILTER (WHERE embedding IS NOT NULL) AS entries_with_embeddings,
                COUNT(DISTINCT category) AS category_count,
                COALESCE(
                    jsonb_agg(DISTINCT category) FILTER (WHERE category IS NOT NULL),
                    '[]'::jsonb
                ) AS categories
            FROM memory_entries
            "#,
        )
        .fetch_optional(&self.db_pool)
        .await;

        let knowledge_json = match kb_stats {
            Ok(Some(row)) => {
                json!({
                    "total_entries": row.get::<Option<i64>, _>("total_entries").unwrap_or(0),
                    "entries_with_embeddings": row.get::<Option<i64>, _>("entries_with_embeddings").unwrap_or(0),
                    "category_count": row.get::<Option<i64>, _>("category_count").unwrap_or(0),
                    "categories": row.get::<Option<JsonValue>, _>("categories").unwrap_or(json!([])),
                })
            }
            _ => json!({
                "total_entries": 0,
                "entries_with_embeddings": 0,
                "category_count": 0,
                "categories": [],
            }),
        };

        Ok(ToolResult::success(json!({
            "collections": collections_json,
            "canvases": canvases_json,
            "sites": sites_json,
            "knowledge_base": knowledge_json,
            "summary": format!(
                "{} collections, {} canvases, {} sites, {} knowledge entries",
                collections_json.len(),
                canvases_json.len(),
                sites_json.len(),
                knowledge_json["total_entries"]
            )
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
}
