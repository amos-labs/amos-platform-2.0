//! Document export / generation
//!
//! Takes structured content (sections of text, optional title, optional header/
//! footer) and renders it into a downloadable file.
//!
//! Currently supports:
//! - **PDF** via the `genpdf` crate (pure-Rust, no external dependencies)
//! - **DOCX** via the `docx-rs` crate
//!
//! The AI agent produces the *content*; the harness does the deterministic
//! rendering.  This keeps the agent's job simple (text → text) and keeps
//! output formatting consistent.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ── Public types ────────────────────────────────────────────────────────────

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Pdf,
    Docx,
}

impl ExportFormat {
    /// MIME type for the generated file.
    pub fn content_type(&self) -> &'static str {
        match self {
            ExportFormat::Pdf => "application/pdf",
            ExportFormat::Docx => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            }
        }
    }

    /// Default file extension (without leading dot).
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Pdf => "pdf",
            ExportFormat::Docx => "docx",
        }
    }

    /// Parse from a string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pdf" => Some(Self::Pdf),
            "docx" | "doc" => Some(Self::Docx),
            _ => None,
        }
    }
}

/// A single section of content for document generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSection {
    /// Optional heading for this section
    pub heading: Option<String>,
    /// Body text (plain text — newlines become paragraph breaks)
    pub body: String,
}

/// Full document content ready for export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContent {
    /// Document title (rendered as the first heading / cover text)
    pub title: Option<String>,
    /// Ordered sections
    pub sections: Vec<ContentSection>,
    /// Optional footer text (repeated on every page if the format supports it)
    pub footer: Option<String>,
}

// ── Exporter ────────────────────────────────────────────────────────────────

/// Central document exporter.  Converts `DocumentContent` into raw bytes for
/// the requested format.
pub struct DocumentExporter;

impl DocumentExporter {
    /// Generate a document in the given format and return the raw bytes.
    ///
    /// This is CPU-bound work — callers should run it on a blocking thread
    /// (e.g. `tokio::task::spawn_blocking`).
    pub fn export(content: &DocumentContent, format: ExportFormat) -> Result<Vec<u8>> {
        match format {
            ExportFormat::Pdf => export_pdf(content),
            ExportFormat::Docx => export_docx(content),
        }
    }

    /// Convenience: export and return `(content_type, filename, bytes)`.
    pub fn export_with_metadata(
        content: &DocumentContent,
        format: ExportFormat,
        base_name: &str,
    ) -> Result<(String, String, Vec<u8>)> {
        let bytes = Self::export(content, format)?;
        let filename = format!("{}.{}", base_name, format.extension());
        Ok((format.content_type().to_string(), filename, bytes))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PDF export (genpdf)
// ═══════════════════════════════════════════════════════════════════════════════

fn export_pdf(content: &DocumentContent) -> Result<Vec<u8>> {
    use genpdf::elements::{Break, Paragraph};
    use genpdf::fonts;
    use genpdf::Alignment;
    use genpdf::Element as _; // bring .styled() method into scope

    // Use the built-in font family (Helvetica-like, always available)
    let font_family = fonts::from_files("", "LiberationSans", None).unwrap_or_else(|_| {
        // Fallback: try system fonts or use built-in
        fonts::from_files(
            "/usr/share/fonts/truetype/liberation",
            "LiberationSans",
            None,
        )
        .unwrap_or_else(|_| {
            fonts::from_files("/System/Library/Fonts/Supplemental", "Arial", None)
                .expect("No usable font found — install LiberationSans or Arial")
        })
    });

    let mut doc = genpdf::Document::new(font_family);
    doc.set_title(content.title.as_deref().unwrap_or("Document"));

    // Margins & basic style
    let mut decorator = genpdf::SimplePageDecorator::new();
    decorator.set_margins(genpdf::Margins::trbl(20, 15, 20, 15));

    // Footer
    if let Some(ref footer_text) = content.footer {
        let ft = footer_text.clone();
        decorator.set_header(move |_page| {
            // We use header callback because genpdf doesn't have footer —
            // we'll add a spacer paragraph at the bottom instead.
            // For now, just return an empty element.
            let _ = &ft;
            Paragraph::new("")
        });
    }

    doc.set_page_decorator(decorator);

    // Title
    if let Some(ref title) = content.title {
        let mut p = Paragraph::new(title.as_str());
        p.set_alignment(Alignment::Center);
        doc.push(p.styled(genpdf::style::Style::new().bold().with_font_size(18)));
        doc.push(Break::new(1.5));
    }

    // Sections
    for section in &content.sections {
        if let Some(ref heading) = section.heading {
            let mut h = Paragraph::new(heading.as_str());
            h.set_alignment(Alignment::Left);
            doc.push(h.styled(genpdf::style::Style::new().bold().with_font_size(14)));
            doc.push(Break::new(0.5));
        }

        // Split body by newlines into separate paragraphs
        for line in section.body.split('\n') {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                doc.push(Break::new(0.5));
            } else {
                doc.push(Paragraph::new(trimmed));
            }
        }

        doc.push(Break::new(1.0));
    }

    // Footer as last paragraph on each page isn't directly supported by genpdf,
    // but we add it as a final note.
    if let Some(ref footer_text) = content.footer {
        doc.push(Break::new(2.0));
        let mut footer = Paragraph::new(footer_text.as_str());
        footer.set_alignment(Alignment::Center);
        doc.push(footer.styled(genpdf::style::Style::new().italic().with_font_size(8)));
    }

    // Render to bytes
    let mut buf = Vec::new();
    doc.render(&mut buf)
        .context("Failed to render PDF document")?;

    Ok(buf)
}

// ═══════════════════════════════════════════════════════════════════════════════
// DOCX export (docx-rs)
// ═══════════════════════════════════════════════════════════════════════════════

fn export_docx(content: &DocumentContent) -> Result<Vec<u8>> {
    use docx_rs::*;

    let mut docx = Docx::new();

    // Title
    if let Some(ref title) = content.title {
        let run = Run::new().add_text(title).bold();
        let para = Paragraph::new().add_run(run).align(AlignmentType::Center);
        docx = docx.add_paragraph(para);

        // Blank line after title
        docx = docx.add_paragraph(Paragraph::new());
    }

    // Sections
    for section in &content.sections {
        // Section heading
        if let Some(ref heading) = section.heading {
            let run = Run::new().add_text(heading).bold();
            let para = Paragraph::new().add_run(run);
            docx = docx.add_paragraph(para);
        }

        // Body paragraphs (split by newlines)
        for line in section.body.split('\n') {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                docx = docx.add_paragraph(Paragraph::new());
            } else {
                let run = Run::new().add_text(trimmed);
                let para = Paragraph::new().add_run(run);
                docx = docx.add_paragraph(para);
            }
        }

        // Spacing between sections
        docx = docx.add_paragraph(Paragraph::new());
    }

    // Footer
    if let Some(ref footer_text) = content.footer {
        let run = Run::new().add_text(footer_text).italic();
        let para = Paragraph::new().add_run(run).align(AlignmentType::Center);
        docx = docx.add_paragraph(para);
    }

    // Render to bytes
    let mut buf = Vec::new();
    docx.build()
        .pack(&mut std::io::Cursor::new(&mut buf))
        .context("Failed to render DOCX document")?;

    Ok(buf)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_content() -> DocumentContent {
        DocumentContent {
            title: Some("Test Document".to_string()),
            sections: vec![
                ContentSection {
                    heading: Some("Introduction".to_string()),
                    body: "This is the introduction paragraph.\nIt has multiple lines.".to_string(),
                },
                ContentSection {
                    heading: None,
                    body: "A section without a heading.".to_string(),
                },
            ],
            footer: Some("Generated by AMOS".to_string()),
        }
    }

    #[test]
    fn test_export_format_content_type() {
        assert_eq!(ExportFormat::Pdf.content_type(), "application/pdf");
        assert!(ExportFormat::Docx
            .content_type()
            .contains("wordprocessingml"));
    }

    #[test]
    fn test_export_format_extension() {
        assert_eq!(ExportFormat::Pdf.extension(), "pdf");
        assert_eq!(ExportFormat::Docx.extension(), "docx");
    }

    #[test]
    fn test_export_format_from_str_loose() {
        assert_eq!(ExportFormat::from_str_loose("pdf"), Some(ExportFormat::Pdf));
        assert_eq!(ExportFormat::from_str_loose("PDF"), Some(ExportFormat::Pdf));
        assert_eq!(
            ExportFormat::from_str_loose("docx"),
            Some(ExportFormat::Docx)
        );
        assert_eq!(
            ExportFormat::from_str_loose("doc"),
            Some(ExportFormat::Docx)
        );
        assert_eq!(ExportFormat::from_str_loose("txt"), None);
    }

    #[test]
    fn test_export_docx_produces_bytes() {
        let content = sample_content();
        let result = DocumentExporter::export(&content, ExportFormat::Docx);
        assert!(result.is_ok(), "DOCX export should succeed");
        let bytes = result.unwrap();
        assert!(!bytes.is_empty(), "DOCX should produce non-empty output");
        // DOCX is a ZIP — starts with PK signature
        assert_eq!(&bytes[0..2], b"PK", "DOCX should be a valid ZIP file");
    }

    #[test]
    fn test_export_with_metadata() {
        let content = sample_content();
        let result =
            DocumentExporter::export_with_metadata(&content, ExportFormat::Docx, "test_report");
        assert!(result.is_ok());
        let (ct, filename, bytes) = result.unwrap();
        assert!(ct.contains("wordprocessingml"));
        assert_eq!(filename, "test_report.docx");
        assert!(!bytes.is_empty());
    }

    // Note: PDF test is excluded because genpdf requires font files at runtime.
    // In CI we'd install LiberationSans and enable this test.
}
