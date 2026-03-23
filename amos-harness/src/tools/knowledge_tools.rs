//! Knowledge base tools: document ingestion and semantic search.
//!
//! These tools let the agent ingest documents into the knowledge base
//! (chunked + embedded for RAG) and perform semantic search over them.

use super::{Tool, ToolCategory, ToolResult};
use crate::embeddings::{chunk_text, EmbeddingService};
use amos_core::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value as JsonValue};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

/// Ingest a document into the knowledge base with chunking and embedding.
pub struct IngestDocumentTool {
    db_pool: PgPool,
    embedding_service: Option<Arc<EmbeddingService>>,
}

impl IngestDocumentTool {
    pub fn new(db_pool: PgPool, embedding_service: Option<Arc<EmbeddingService>>) -> Self {
        Self {
            db_pool,
            embedding_service,
        }
    }
}

#[async_trait]
impl Tool for IngestDocumentTool {
    fn name(&self) -> &str {
        "ingest_document"
    }

    fn description(&self) -> &str {
        "Ingest a document into the knowledge base. The text is chunked, embedded, and stored for semantic search. Use this to make documents permanently searchable across sessions."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The full text content of the document to ingest"
                },
                "title": {
                    "type": "string",
                    "description": "Title of the document"
                },
                "category": {
                    "type": "string",
                    "description": "Category for organization (e.g., 'policy', 'reference', 'report')"
                },
                "source_file": {
                    "type": "string",
                    "description": "Original filename if uploaded as a file"
                }
            },
            "required": ["content", "title"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let content = params["content"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("content is required".to_string()))?;

        let title = params["title"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("title is required".to_string()))?;

        let category = params
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("document");

        let source_file = params.get("source_file").and_then(|v| v.as_str());

        let embedding_svc = match &self.embedding_service {
            Some(svc) => svc,
            None => {
                return Ok(ToolResult::error(
                    "Embedding service not configured. Set AMOS__EMBEDDING__API_KEY to enable document ingestion.".to_string(),
                ));
            }
        };

        // Create parent entry (the document record)
        let metadata = json!({
            "title": title,
            "source_file": source_file,
            "char_count": content.len(),
        });

        let parent_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO memory_entries (content, category, salience, metadata, source, created_at, last_accessed_at)
            VALUES ($1, $2, 0.8, $3, 'document', $4, $5)
            RETURNING id
            "#,
        )
        .bind(title) // Parent entry stores the title as content
        .bind(category)
        .bind(&metadata)
        .bind(Utc::now())
        .bind(Utc::now())
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(format!("Failed to create parent entry: {e}")))?;

        // Chunk the document text
        let chunks = chunk_text(content, 2000, 200);

        // Embed all chunks in batch
        let chunk_refs: Vec<&str> = chunks.iter().map(|s| s.as_str()).collect();
        let embeddings = embedding_svc.embed_batch(&chunk_refs).await?;

        // Insert chunk entries with embeddings
        let mut inserted = 0;
        for (i, (chunk, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
            let embedding_str = serde_json::to_string(embedding).unwrap_or_default();
            let chunk_metadata = json!({
                "title": title,
                "chunk_index": i,
                "total_chunks": chunks.len(),
            });

            let result = sqlx::query(
                r#"
                INSERT INTO memory_entries
                    (content, category, salience, metadata, source, chunk_index, parent_id, embedding, created_at, last_accessed_at)
                VALUES ($1, $2, 0.7, $3, 'document', $4, $5, $6::vector, $7, $8)
                "#,
            )
            .bind(chunk)
            .bind(category)
            .bind(&chunk_metadata)
            .bind(i as i32)
            .bind(parent_id)
            .bind(&embedding_str)
            .bind(Utc::now())
            .bind(Utc::now())
            .execute(&self.db_pool)
            .await;

            if let Err(e) = result {
                tracing::warn!("Failed to insert chunk {}: {}", i, e);
            } else {
                inserted += 1;
            }
        }

        Ok(ToolResult::success(json!({
            "document_id": parent_id.to_string(),
            "title": title,
            "category": category,
            "chunks_created": inserted,
            "total_chunks": chunks.len(),
            "message": format!("Document '{}' ingested: {} chunks created with embeddings", title, inserted)
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Knowledge
    }
}

/// Search the knowledge base using semantic similarity.
pub struct KnowledgeSearchTool {
    db_pool: PgPool,
    embedding_service: Option<Arc<EmbeddingService>>,
}

impl KnowledgeSearchTool {
    pub fn new(db_pool: PgPool, embedding_service: Option<Arc<EmbeddingService>>) -> Self {
        Self {
            db_pool,
            embedding_service,
        }
    }
}

#[async_trait]
impl Tool for KnowledgeSearchTool {
    fn name(&self) -> &str {
        "knowledge_search"
    }

    fn description(&self) -> &str {
        "Search the knowledge base for relevant information using semantic similarity. Returns the most relevant document chunks. Use this to find information from previously ingested documents."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural language search query"
                },
                "category": {
                    "type": "string",
                    "description": "Filter by category (optional)"
                },
                "source": {
                    "type": "string",
                    "description": "Filter by source type: 'document', 'agent', or all (optional)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 5)",
                    "default": 5
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
        let source = params.get("source").and_then(|v| v.as_str());
        let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(5);

        let embedding_svc = match &self.embedding_service {
            Some(svc) => svc,
            None => {
                return Ok(ToolResult::error(
                    "Embedding service not configured. Set AMOS__EMBEDDING__API_KEY to enable knowledge search.".to_string(),
                ));
            }
        };

        // Embed the query
        let query_embedding = embedding_svc.embed(query).await?;
        let embedding_str = serde_json::to_string(&query_embedding).unwrap_or_default();

        // Build query with optional filters
        let mut sql = String::from(
            r#"
            SELECT id, content, category, source, salience, metadata,
                   1 - (embedding <=> $1::vector) AS similarity
            FROM memory_entries
            WHERE embedding IS NOT NULL
            "#,
        );

        let mut param_idx = 2;
        let mut bind_values: Vec<String> = Vec::new();

        if let Some(cat) = category {
            sql.push_str(&format!(" AND category = ${param_idx}"));
            bind_values.push(cat.to_string());
            param_idx += 1;
        }

        if let Some(src) = source {
            sql.push_str(&format!(" AND source = ${param_idx}"));
            bind_values.push(src.to_string());
            param_idx += 1;
        }

        sql.push_str(&format!(
            " ORDER BY embedding <=> $1::vector LIMIT ${param_idx}"
        ));

        // Execute with dynamic bindings
        let mut query_builder = sqlx::query(&sql).bind(&embedding_str);
        for val in &bind_values {
            query_builder = query_builder.bind(val);
        }
        query_builder = query_builder.bind(limit);

        let rows = query_builder.fetch_all(&self.db_pool).await;

        match rows {
            Ok(rows) => {
                let mut results = Vec::new();
                for row in &rows {
                    let id: Uuid = row.get("id");
                    let content: String = row.get("content");
                    let cat: String = row.get("category");
                    let src: String = row.get("source");
                    let similarity: f64 = row.get("similarity");
                    let metadata: JsonValue = row.get("metadata");

                    results.push(json!({
                        "id": id.to_string(),
                        "content": content,
                        "category": cat,
                        "source": src,
                        "similarity": similarity,
                        "title": metadata.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                    }));

                    // Reinforce accessed entries
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
                    "query": query
                })))
            }
            Err(e) => Ok(ToolResult::error(format!("Knowledge search failed: {}", e))),
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Knowledge
    }
}

/// Ingest document content in the background (non-blocking).
///
/// Called from the attachment processing pipeline to automatically
/// make uploaded documents searchable via RAG.
pub async fn background_ingest(
    db_pool: PgPool,
    embedding_service: Arc<EmbeddingService>,
    title: String,
    content: String,
    category: String,
) {
    let chunks = chunk_text(&content, 2000, 200);
    if chunks.is_empty() {
        return;
    }

    // Create parent entry
    let metadata = json!({
        "title": &title,
        "char_count": content.len(),
        "auto_ingested": true,
    });

    let parent_result: std::result::Result<Uuid, _> = sqlx::query_scalar(
        r#"
        INSERT INTO memory_entries (content, category, salience, metadata, source, created_at, last_accessed_at)
        VALUES ($1, $2, 0.7, $3, 'document', $4, $5)
        RETURNING id
        "#,
    )
    .bind(&title)
    .bind(&category)
    .bind(&metadata)
    .bind(Utc::now())
    .bind(Utc::now())
    .fetch_one(&db_pool)
    .await;

    let parent_id = match parent_result {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!("Background ingest: failed to create parent entry: {}", e);
            return;
        }
    };

    // Embed chunks in batch
    let chunk_refs: Vec<&str> = chunks.iter().map(|s| s.as_str()).collect();
    let embeddings = match embedding_service.embed_batch(&chunk_refs).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("Background ingest: embedding failed: {}", e);
            return;
        }
    };

    // Insert chunks
    let mut inserted = 0;
    for (i, (chunk, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
        let embedding_str = serde_json::to_string(embedding).unwrap_or_default();
        let chunk_metadata = json!({
            "title": &title,
            "chunk_index": i,
            "total_chunks": chunks.len(),
            "auto_ingested": true,
        });

        let result = sqlx::query(
            r#"
            INSERT INTO memory_entries
                (content, category, salience, metadata, source, chunk_index, parent_id, embedding, created_at, last_accessed_at)
            VALUES ($1, $2, 0.7, $3, 'document', $4, $5, $6::vector, $7, $8)
            "#,
        )
        .bind(chunk)
        .bind(&category)
        .bind(&chunk_metadata)
        .bind(i as i32)
        .bind(parent_id)
        .bind(&embedding_str)
        .bind(Utc::now())
        .bind(Utc::now())
        .execute(&db_pool)
        .await;

        if result.is_ok() {
            inserted += 1;
        }
    }

    tracing::info!(
        title = %title,
        parent_id = %parent_id,
        chunks = inserted,
        "Background document ingestion completed"
    );
}
