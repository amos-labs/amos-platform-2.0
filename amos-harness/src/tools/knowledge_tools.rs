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

        // If embedding service is available, embed and store with vectors.
        // Otherwise, store chunks as text-only (still searchable via ILIKE).
        let has_embeddings = self.embedding_service.is_some();
        let embeddings = if let Some(ref svc) = self.embedding_service {
            let chunk_refs: Vec<&str> = chunks.iter().map(|s| s.as_str()).collect();
            match svc.embed_batch(&chunk_refs).await {
                Ok(e) => Some(e),
                Err(err) => {
                    tracing::warn!("Embedding failed, storing without vectors: {err}");
                    None
                }
            }
        } else {
            None
        };

        // Insert chunk entries (with or without embeddings)
        let mut inserted = 0;
        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_metadata = json!({
                "title": title,
                "chunk_index": i,
                "total_chunks": chunks.len(),
            });

            let result = if let Some(ref embs) = embeddings {
                let embedding_str = serde_json::to_string(&embs[i]).unwrap_or_default();
                sqlx::query(
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
                .await
            } else {
                sqlx::query(
                    r#"
                    INSERT INTO memory_entries
                        (content, category, salience, metadata, source, chunk_index, parent_id, created_at, last_accessed_at)
                    VALUES ($1, $2, 0.7, $3, 'document', $4, $5, $6, $7)
                    "#,
                )
                .bind(chunk)
                .bind(category)
                .bind(&chunk_metadata)
                .bind(i as i32)
                .bind(parent_id)
                .bind(Utc::now())
                .bind(Utc::now())
                .execute(&self.db_pool)
                .await
            };

            if let Err(e) = result {
                tracing::warn!("Failed to insert chunk {}: {}", i, e);
            } else {
                inserted += 1;
            }
        }

        let note = if has_embeddings && embeddings.is_some() {
            format!("Document '{}' ingested: {} chunks created with embeddings", title, inserted)
        } else {
            format!("Document '{}' ingested: {} chunks stored without embeddings. Set AMOS__EMBEDDING__API_KEY for semantic search.", title, inserted)
        };

        Ok(ToolResult::success(json!({
            "document_id": parent_id.to_string(),
            "title": title,
            "category": category,
            "chunks_created": inserted,
            "total_chunks": chunks.len(),
            "has_embeddings": has_embeddings && embeddings.is_some(),
            "message": note
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

        // Try semantic search first if embedding service is available
        if let Some(ref embedding_svc) = self.embedding_service {
            if let Ok(query_embedding) = embedding_svc.embed(query).await {
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

                let mut query_builder = sqlx::query(&sql).bind(&embedding_str);
                for val in &bind_values {
                    query_builder = query_builder.bind(val);
                }
                query_builder = query_builder.bind(limit);

                if let Ok(rows) = query_builder.fetch_all(&self.db_pool).await {
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

                        let _ = sqlx::query(
                            "UPDATE memory_entries SET last_accessed_at = $1, access_count = access_count + 1 WHERE id = $2",
                        )
                        .bind(Utc::now())
                        .bind(id)
                        .execute(&self.db_pool)
                        .await;
                    }

                    return Ok(ToolResult::success(json!({
                        "results": results,
                        "count": results.len(),
                        "query": query,
                        "search_type": "semantic"
                    })));
                }
            }
        }

        // Fallback: text search using ILIKE (same pattern as SearchMemoryTool)
        let search_pattern = format!("%{}%", query);
        let rows = if let Some(cat) = category {
            sqlx::query(
                r#"
                SELECT id, content, category, source, salience, metadata
                FROM memory_entries
                WHERE source = 'document' AND category = $1 AND content ILIKE $2
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
                SELECT id, content, category, source, salience, metadata
                FROM memory_entries
                WHERE source = 'document' AND content ILIKE $1
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
                    let src: String = row.get("source");
                    let salience: f64 = row.get("salience");
                    let metadata: JsonValue = row.get("metadata");

                    results.push(json!({
                        "id": id.to_string(),
                        "content": content,
                        "category": cat,
                        "source": src,
                        "salience": salience,
                        "title": metadata.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                    }));

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
                    "query": query,
                    "search_type": "text"
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
