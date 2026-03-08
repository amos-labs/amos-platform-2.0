//! Document processing: extraction (import) and generation (export)
//!
//! Provides a trait-based pipeline for:
//! - **Extraction**: Convert uploaded files (PDF, DOCX, etc.) into text/images
//!   that the AI agent can consume directly.
//! - **Export**: Generate documents (PDF, DOCX) from structured content that the
//!   AI agent produces.
//!
//! Design principle: the harness does the deterministic parsing/rendering so the
//! AI only ever works with clean text or renderable images.

pub mod export;
pub mod extract;

pub use extract::{DocumentProcessor, ExtractionResult, PageContent};
pub use export::{DocumentExporter, ExportFormat};
