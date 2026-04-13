//! Document generation tools for the AI agent
//!
//! Allows the agent to generate PDF and DOCX documents from structured content.
//! The agent provides the text content; the harness handles deterministic rendering.

use crate::documents::export::{ContentSection, DocumentContent, DocumentExporter, ExportFormat};
use crate::storage::StorageClient;
use crate::tools::{Tool, ToolCategory, ToolResult};
use amos_core::{AppConfig, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Tool: generate_document
///
/// Generates a PDF or DOCX document from structured text content.
/// The agent provides title, sections (with optional headings), and format.
/// Persists the document to storage and returns a download URL.
pub struct GenerateDocumentTool {
    config: Arc<AppConfig>,
    db_pool: PgPool,
    storage: Arc<StorageClient>,
}

impl GenerateDocumentTool {
    pub fn new(config: Arc<AppConfig>, db_pool: PgPool, storage: Arc<StorageClient>) -> Self {
        Self {
            config,
            db_pool,
            storage,
        }
    }
}

#[async_trait]
impl Tool for GenerateDocumentTool {
    fn name(&self) -> &str {
        "generate_document"
    }

    fn description(&self) -> &str {
        "Generate a PDF or DOCX document from structured text content. Provide a title, sections with optional headings, and the desired format. Returns the document as a downloadable file."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Document title (displayed as the main heading)"
                },
                "sections": {
                    "type": "array",
                    "description": "Ordered list of content sections",
                    "items": {
                        "type": "object",
                        "properties": {
                            "heading": {
                                "type": "string",
                                "description": "Optional section heading"
                            },
                            "body": {
                                "type": "string",
                                "description": "Section body text. Use newlines for paragraph breaks."
                            }
                        },
                        "required": ["body"]
                    }
                },
                "format": {
                    "type": "string",
                    "enum": ["pdf", "docx"],
                    "description": "Output format: 'pdf' or 'docx'"
                },
                "filename": {
                    "type": "string",
                    "description": "Base filename (without extension). Defaults to 'document'."
                },
                "footer": {
                    "type": "string",
                    "description": "Optional footer text"
                }
            },
            "required": ["sections", "format"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Document
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        // Parse parameters
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .map(String::from);
        let footer = params
            .get("footer")
            .and_then(|v| v.as_str())
            .map(String::from);
        let filename = params
            .get("filename")
            .and_then(|v| v.as_str())
            .unwrap_or("document");

        let format_str = params
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("pdf");

        let format = match ExportFormat::from_str_loose(format_str) {
            Some(f) => f,
            None => {
                return Ok(ToolResult::error(format!(
                    "Unsupported format '{}'. Use 'pdf' or 'docx'.",
                    format_str
                )));
            }
        };

        // Parse sections
        let sections_arr = match params.get("sections").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => {
                return Ok(ToolResult::error(
                    "Missing 'sections' array parameter".to_string(),
                ));
            }
        };

        let sections: Vec<ContentSection> = sections_arr
            .iter()
            .map(|s| ContentSection {
                heading: s.get("heading").and_then(|v| v.as_str()).map(String::from),
                body: s
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
            .collect();

        if sections.is_empty() {
            return Ok(ToolResult::error(
                "At least one section is required".to_string(),
            ));
        }

        let content = DocumentContent {
            title,
            sections,
            footer,
        };

        let base_name = filename.to_string();
        let fmt = format;

        // Run document generation on blocking thread (CPU-bound)
        let result = tokio::task::spawn_blocking(move || {
            DocumentExporter::export_with_metadata(&content, fmt, &base_name)
        })
        .await;

        match result {
            Ok(Ok((content_type, generated_filename, bytes))) => {
                let size_bytes = bytes.len() as i64;
                let upload_id = Uuid::new_v4();

                // Derive extension from generated filename
                let ext = generated_filename
                    .rsplit('.')
                    .next()
                    .filter(|e| e.len() <= 10)
                    .unwrap_or("bin");
                let storage_key = format!("{}.{}", upload_id, ext);

                // Persist to storage backend
                if let Err(e) = self
                    .storage
                    .upload(&storage_key, &bytes, &content_type)
                    .await
                {
                    return Ok(ToolResult::error(format!(
                        "Failed to store generated document: {e}"
                    )));
                }

                // Persist metadata to uploads table
                if let Err(e) = sqlx::query(
                    r#"
                    INSERT INTO uploads (id, filename, original_filename, content_type, size_bytes, storage_key, upload_context)
                    VALUES ($1, $2, $3, $4, $5, $6, 'document')
                    "#,
                )
                .bind(upload_id)
                .bind(&storage_key)
                .bind(&generated_filename)
                .bind(&content_type)
                .bind(size_bytes)
                .bind(&storage_key)
                .execute(&self.db_pool)
                .await
                {
                    tracing::error!("Failed to save document upload record: {e}");
                    // File is in storage but metadata failed — still return success
                    // since the document was generated. Log and continue.
                }

                let download_url = format!("/api/v1/uploads/{}/file", upload_id);

                Ok(ToolResult::success_with_metadata(
                    json!({
                        "filename": generated_filename,
                        "content_type": content_type,
                        "size_bytes": size_bytes,
                        "download_url": download_url,
                        "upload_id": upload_id.to_string(),
                        "message": format!(
                            "Generated {} ({} bytes). Download: {}",
                            generated_filename, size_bytes, download_url
                        )
                    }),
                    json!({
                        "format": format_str,
                        "generated": true,
                        "upload_id": upload_id.to_string(),
                    }),
                ))
            }
            Ok(Err(e)) => Ok(ToolResult::error(format!(
                "Document generation failed: {}",
                e
            ))),
            Err(e) => Ok(ToolResult::error(format!(
                "Document generation task panicked: {}",
                e
            ))),
        }
    }
}
