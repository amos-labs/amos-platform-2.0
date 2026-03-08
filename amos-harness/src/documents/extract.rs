//! Document text/content extraction
//!
//! Trait-based extractor pipeline that converts uploaded binary files into
//! clean text (or page images) that the AI agent can consume directly.
//!
//! Adding support for a new file format = implementing `DocumentExtractor`.

use anyhow::{Context, Result};
use std::io::Cursor;

// ── Core types ───────────────────────────────────────────────────────────────

/// Content extracted from a single page of a document.
#[derive(Debug, Clone)]
pub struct PageContent {
    /// 1-based page number
    pub page_number: usize,
    /// Extracted text for this page
    pub text: String,
}

/// Result of running extraction on a document.
#[derive(Debug, Clone)]
pub enum ExtractionResult {
    /// Simple text blob (single-page or already concatenated)
    Text(String),
    /// Multi-page document with per-page text
    Pages(Vec<PageContent>),
    /// The document type is recognised but extraction failed — fall back to
    /// sending the raw image bytes to Claude Vision (e.g. scanned PDF pages).
    /// Each entry is (media_type, image_bytes).
    RenderedPages(Vec<(String, Vec<u8>)>),
    /// Text extraction succeeded but the PDF is image-heavy (sparse text
    /// relative to file size). Send the raw document bytes to Claude's native
    /// document content block for vision analysis instead.
    /// Fields: (format, filename, raw_bytes)
    RawDocument(String, String, Vec<u8>),
    /// Format not supported by any registered extractor
    Unsupported,
}

// ── Extractor trait ──────────────────────────────────────────────────────────

/// A pluggable document extractor.  Implement this for each file format.
pub trait DocumentExtractor: Send + Sync {
    /// Human-readable name for logging / debug.
    fn name(&self) -> &str;

    /// Return `true` if this extractor handles the given MIME type.
    fn can_handle(&self, content_type: &str, filename: &str) -> bool;

    /// Extract text (and optionally images) from the raw bytes.
    /// This is called synchronously on a blocking thread via `spawn_blocking`
    /// because PDF/DOCX parsing is CPU-bound.
    fn extract(&self, data: &[u8], filename: &str) -> Result<ExtractionResult>;
}

// ── Processor (chains extractors) ────────────────────────────────────────────

/// Central document processor.  Holds registered extractors and dispatches
/// incoming files to the first one that matches.
pub struct DocumentProcessor {
    extractors: Vec<Box<dyn DocumentExtractor>>,
}

impl DocumentProcessor {
    /// Build a processor with all built-in extractors registered.
    pub fn new() -> Self {
        let mut p = Self {
            extractors: Vec::new(),
        };
        // Register built-in extractors (order matters — first match wins)
        p.register(Box::new(PdfExtractor));
        p.register(Box::new(DocxExtractor));
        p.register(Box::new(HtmlExtractor));
        p
    }

    /// Register an additional extractor (for plugin-style extensibility).
    pub fn register(&mut self, extractor: Box<dyn DocumentExtractor>) {
        self.extractors.push(extractor);
    }

    /// Run extraction.  Returns `ExtractionResult::Unsupported` if no
    /// extractor handles the content type.
    pub async fn extract(
        &self,
        data: &[u8],
        filename: &str,
        content_type: &str,
    ) -> ExtractionResult {
        // Sniff actual content type from magic bytes — files are sometimes
        // mislabeled (e.g. an HTML page saved as .pdf).
        let effective_ct = sniff_content_type(data, content_type);
        let content_overridden = effective_ct != content_type;
        if content_overridden {
            tracing::info!(
                "Content-type sniff: '{}' declared as '{}' but detected as '{}'",
                filename,
                content_type,
                effective_ct
            );
        }

        // When the content type was overridden by sniffing, ignore filename
        // extension for extractor matching (prevents ".pdf" triggering the PDF
        // extractor when the file is actually HTML).
        let effective_filename = if content_overridden { "" } else { filename };

        // Find the first extractor that can handle this content type
        for ext in &self.extractors {
            if ext.can_handle(&effective_ct, effective_filename) {
                let name = ext.name().to_string();
                // Clone data for the blocking task
                let data = data.to_vec();
                let fname = filename.to_string();

                // PDF / DOCX parsing is CPU-bound — run on blocking thread pool
                let ext_ptr = &**ext as *const dyn DocumentExtractor;
                // SAFETY: extractors are &self (immutable) and live for 'static
                // because DocumentProcessor is held in AppState (Arc).
                let ext_ref = unsafe { &*ext_ptr };

                match tokio::task::spawn_blocking(move || ext_ref.extract(&data, &fname)).await {
                    Ok(Ok(result)) => {
                        tracing::info!(
                            "Extractor '{}' succeeded for '{}'",
                            name,
                            filename
                        );
                        return result;
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(
                            "Extractor '{}' failed for '{}': {}",
                            name,
                            filename,
                            e
                        );
                        // Fall through to try next extractor or return Unsupported
                    }
                    Err(e) => {
                        tracing::error!(
                            "Extractor '{}' panicked for '{}': {}",
                            name,
                            filename,
                            e
                        );
                    }
                }
            }
        }

        ExtractionResult::Unsupported
    }
}

impl Default for DocumentProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Built-in extractors
// ═══════════════════════════════════════════════════════════════════════════════

// ── PDF ──────────────────────────────────────────────────────────────────────

/// Extracts text from PDF files using the `pdf-extract` crate.
struct PdfExtractor;

impl DocumentExtractor for PdfExtractor {
    fn name(&self) -> &str {
        "pdf-text"
    }

    fn can_handle(&self, content_type: &str, filename: &str) -> bool {
        content_type == "application/pdf"
            || filename.to_lowercase().ends_with(".pdf")
    }

    fn extract(&self, data: &[u8], filename: &str) -> Result<ExtractionResult> {
        // Try pdf-extract first (best quality), fall back to lopdf raw extraction
        let text = match pdf_extract::extract_text_from_mem(data) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    "pdf-extract failed for '{}' ({}), trying lopdf fallback",
                    filename,
                    e
                );
                extract_text_with_lopdf(data, filename)?
            }
        };

        let trimmed = text.trim().to_string();
        if trimmed.is_empty() {
            // Likely a scanned/image-only PDF — text extraction yielded nothing.
            // Send as raw document so Claude can use native PDF vision.
            tracing::info!("PDF '{}' yielded no text (likely scanned), using raw document passthrough", filename);
            return Ok(ExtractionResult::RawDocument(
                "pdf".to_string(),
                filename.to_string(),
                data.to_vec(),
            ));
        }

        // ── Sparse-text detection ───────────────────────────────────────────
        // If the PDF is large (>50KB) but extracted text is very short (<500
        // chars), the document is likely image-heavy (pitch decks, scanned
        // forms, brochures, etc.). In this case, send the raw PDF to Claude's
        // native document content block for vision analysis — it will see all
        // the graphics and layout that text extraction misses.
        let text_bytes = trimmed.len();
        let file_bytes = data.len();
        let text_ratio = text_bytes as f64 / file_bytes as f64;

        if file_bytes > 50_000 && (text_bytes < 500 || text_ratio < 0.005) {
            tracing::info!(
                "PDF '{}' appears image-heavy ({}B text from {}B file, ratio {:.4}), using raw document passthrough",
                filename,
                text_bytes,
                file_bytes,
                text_ratio
            );
            return Ok(ExtractionResult::RawDocument(
                "pdf".to_string(),
                filename.to_string(),
                data.to_vec(),
            ));
        }

        // Split into pages by form-feed character (U+000C) which pdf-extract
        // inserts between pages.
        let pages: Vec<PageContent> = trimmed
            .split('\u{000C}')
            .enumerate()
            .map(|(i, page_text)| PageContent {
                page_number: i + 1,
                text: page_text.trim().to_string(),
            })
            .filter(|p| !p.text.is_empty())
            .collect();

        if pages.len() <= 1 {
            Ok(ExtractionResult::Text(trimmed))
        } else {
            Ok(ExtractionResult::Pages(pages))
        }
    }
}

/// Fallback PDF text extraction using lopdf directly.
///
/// This is less sophisticated than pdf-extract but handles more PDF variants
/// (encrypted, unusual fonts, malformed structures) by extracting raw string
/// objects from page content streams.
fn extract_text_with_lopdf(data: &[u8], filename: &str) -> Result<String> {
    use lopdf::Document;

    let doc = Document::load_mem(data)
        .with_context(|| format!("lopdf failed to parse PDF: {filename}"))?;

    let mut all_text = String::new();
    let pages = doc.get_pages();
    let mut page_nums: Vec<u32> = pages.keys().copied().collect();
    page_nums.sort();

    for (i, &page_num) in page_nums.iter().enumerate() {
        if pages.get(&page_num).is_some() {
            // Try to extract text from the page's content streams
            let page_text = doc
                .extract_text(&[page_num])
                .unwrap_or_default();

            let trimmed = page_text.trim();
            if !trimmed.is_empty() {
                if i > 0 {
                    all_text.push('\u{000C}'); // form feed between pages
                }
                all_text.push_str(trimmed);
            }
        }
    }

    if all_text.is_empty() {
        tracing::info!("lopdf fallback also yielded no text for '{}'", filename);
    } else {
        tracing::info!(
            "lopdf fallback extracted {} chars from '{}' ({} pages)",
            all_text.len(),
            filename,
            page_nums.len()
        );
    }

    Ok(all_text)
}

// ── DOCX ─────────────────────────────────────────────────────────────────────

/// Extracts text from .docx files by reading the XML inside the ZIP container.
struct DocxExtractor;

impl DocumentExtractor for DocxExtractor {
    fn name(&self) -> &str {
        "docx-text"
    }

    fn can_handle(&self, content_type: &str, filename: &str) -> bool {
        content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            || content_type == "application/msword"
            || filename.to_lowercase().ends_with(".docx")
    }

    fn extract(&self, data: &[u8], filename: &str) -> Result<ExtractionResult> {
        // DOCX is a ZIP with word/document.xml inside
        let cursor = Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor)
            .with_context(|| format!("Failed to open DOCX as ZIP: {filename}"))?;

        let mut text_parts: Vec<String> = Vec::new();

        // Read word/document.xml
        if let Ok(mut file) = archive.by_name("word/document.xml") {
            let mut xml = String::new();
            std::io::Read::read_to_string(&mut file, &mut xml)?;
            let extracted = extract_text_from_docx_xml(&xml);
            if !extracted.is_empty() {
                text_parts.push(extracted);
            }
        }

        // Also try word/header*.xml and word/footer*.xml
        let names: Vec<String> = (0..archive.len())
            .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
            .collect();

        for name in &names {
            if (name.starts_with("word/header") || name.starts_with("word/footer"))
                && name.ends_with(".xml")
            {
                if let Ok(mut file) = archive.by_name(name) {
                    let mut xml = String::new();
                    std::io::Read::read_to_string(&mut file, &mut xml)?;
                    let extracted = extract_text_from_docx_xml(&xml);
                    if !extracted.is_empty() {
                        text_parts.push(extracted);
                    }
                }
            }
        }

        let full_text = text_parts.join("\n\n");
        if full_text.trim().is_empty() {
            return Ok(ExtractionResult::Unsupported);
        }

        Ok(ExtractionResult::Text(full_text))
    }
}

/// Simple XML text extractor for DOCX.
/// Pulls text content from `<w:t>` elements, using `<w:p>` as paragraph breaks.
fn extract_text_from_docx_xml(xml: &str) -> String {
    let mut result = String::new();
    let mut in_paragraph = false;
    let mut paragraph_text = String::new();

    // Simple state-machine XML parser (avoids heavy XML crate dependency)
    let mut i = 0;
    let bytes = xml.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'<' {
            // Find end of tag
            let tag_start = i;
            while i < bytes.len() && bytes[i] != b'>' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // skip '>'
            }
            let tag = &xml[tag_start..i.min(xml.len())];

            if tag.starts_with("<w:p ") || tag == "<w:p>" {
                in_paragraph = true;
                paragraph_text.clear();
            } else if tag == "</w:p>" {
                if in_paragraph && !paragraph_text.trim().is_empty() {
                    if !result.is_empty() {
                        result.push('\n');
                    }
                    result.push_str(paragraph_text.trim());
                }
                in_paragraph = false;
            } else if tag == "</w:t>" || tag.starts_with("<w:t ") || tag == "<w:t>" {
                // The text content follows after <w:t> and before </w:t>
                // If this is an opening tag, read until </w:t>
                if !tag.starts_with("</") {
                    let text_start = i;
                    // Find </w:t>
                    if let Some(end_pos) = xml[i..].find("</w:t>") {
                        let text = &xml[text_start..i + end_pos];
                        paragraph_text.push_str(text);
                        i += end_pos + 6; // skip past </w:t>
                    }
                }
            } else if tag == "<w:tab/>" || tag == "<w:tab />" {
                paragraph_text.push('\t');
            } else if tag == "<w:br/>" || tag == "<w:br />" {
                paragraph_text.push('\n');
            }
        } else {
            i += 1;
        }
    }

    // Flush last paragraph
    if in_paragraph && !paragraph_text.trim().is_empty() {
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(paragraph_text.trim());
    }

    result
}

// ── HTML ─────────────────────────────────────────────────────────────────────

/// Extracts text from HTML content by stripping tags.
///
/// This handles two scenarios:
/// 1. Files genuinely served as text/html
/// 2. Mislabeled files (e.g. a .pdf that is actually an HTML page — common when
///    downloading from behind a login wall or paywall)
struct HtmlExtractor;

impl DocumentExtractor for HtmlExtractor {
    fn name(&self) -> &str {
        "html-text"
    }

    fn can_handle(&self, content_type: &str, filename: &str) -> bool {
        content_type == "text/html"
            || content_type == "application/xhtml+xml"
            || filename.to_lowercase().ends_with(".html")
            || filename.to_lowercase().ends_with(".htm")
    }

    fn extract(&self, data: &[u8], _filename: &str) -> Result<ExtractionResult> {
        let html = String::from_utf8_lossy(data);
        let text = strip_html_to_text(&html);
        let trimmed = text.trim().to_string();

        if trimmed.is_empty() {
            return Ok(ExtractionResult::Unsupported);
        }

        Ok(ExtractionResult::Text(trimmed))
    }
}

/// Strip HTML tags and decode common entities to produce clean text.
///
/// Uses a simple state-machine approach (no external HTML parser dependency).
/// Handles `<script>`, `<style>`, and `<noscript>` by discarding their content.
/// Inserts newlines at block-level boundaries (`<p>`, `<div>`, `<br>`, `<li>`, headings).
fn strip_html_to_text(html: &str) -> String {
    let mut result = String::with_capacity(html.len() / 3);
    let mut in_tag = false;
    let mut in_skip_element = false;
    let mut current_tag = String::new();
    let mut skip_depth = 0;

    let bytes = html.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i] as char;

        if in_tag {
            if ch == '>' {
                in_tag = false;
                let tag_lower = current_tag.to_lowercase();
                let tag_name = tag_lower
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('/');

                // Track skip elements (script, style, noscript)
                if tag_lower.starts_with("script") || tag_lower.starts_with("style") || tag_lower.starts_with("noscript") {
                    in_skip_element = true;
                    skip_depth += 1;
                } else if (tag_lower.starts_with("/script") || tag_lower.starts_with("/style") || tag_lower.starts_with("/noscript")) && in_skip_element {
                    skip_depth -= 1;
                    if skip_depth <= 0 {
                        in_skip_element = false;
                        skip_depth = 0;
                    }
                }

                // Insert whitespace at block boundaries
                if !in_skip_element {
                    match tag_name {
                        "p" | "div" | "br" | "hr" | "li" | "tr" | "h1" | "h2" | "h3"
                        | "h4" | "h5" | "h6" | "blockquote" | "section" | "article"
                        | "header" | "footer" | "main" | "nav" | "aside" | "dt" | "dd"
                        | "figcaption" | "details" | "summary" => {
                            if !result.ends_with('\n') {
                                result.push('\n');
                            }
                        }
                        "td" | "th" => {
                            if !result.ends_with('\t') && !result.ends_with('\n') {
                                result.push('\t');
                            }
                        }
                        _ => {}
                    }
                }

                current_tag.clear();
            } else {
                current_tag.push(ch);
            }
        } else if ch == '<' {
            in_tag = true;
            current_tag.clear();
        } else if ch == '&' && !in_skip_element {
            // Decode HTML entities
            let remaining = &html[i..];
            if remaining.starts_with("&amp;") {
                result.push('&');
                i += 5;
                continue;
            } else if remaining.starts_with("&lt;") {
                result.push('<');
                i += 4;
                continue;
            } else if remaining.starts_with("&gt;") {
                result.push('>');
                i += 4;
                continue;
            } else if remaining.starts_with("&quot;") {
                result.push('"');
                i += 6;
                continue;
            } else if remaining.starts_with("&#39;") || remaining.starts_with("&apos;") {
                result.push('\'');
                i += if remaining.starts_with("&#39;") { 5 } else { 6 };
                continue;
            } else if remaining.starts_with("&nbsp;") {
                result.push(' ');
                i += 6;
                continue;
            } else if remaining.starts_with("&#") {
                // Numeric entity
                if let Some(semi) = remaining.find(';') {
                    let num_str = &remaining[2..semi];
                    let code = if num_str.starts_with('x') || num_str.starts_with('X') {
                        u32::from_str_radix(&num_str[1..], 16).ok()
                    } else {
                        num_str.parse::<u32>().ok()
                    };
                    if let Some(c) = code.and_then(char::from_u32) {
                        result.push(c);
                        i += semi + 1;
                        continue;
                    }
                }
                result.push('&');
            } else {
                result.push('&');
            }
        } else if !in_skip_element {
            result.push(ch);
        }

        i += 1;
    }

    // Collapse multiple blank lines into at most two newlines
    let mut cleaned = String::with_capacity(result.len());
    let mut newline_count = 0;
    for ch in result.chars() {
        if ch == '\n' {
            newline_count += 1;
            if newline_count <= 2 {
                cleaned.push(ch);
            }
        } else {
            newline_count = 0;
            cleaned.push(ch);
        }
    }

    cleaned
}

// ── Content-type sniffing ────────────────────────────────────────────────────

/// Detect the actual content type from file magic bytes.
///
/// Returns the declared `content_type` if it matches the data, or an
/// overridden type if the magic bytes indicate something different.
fn sniff_content_type<'a>(data: &[u8], declared: &'a str) -> &'a str {
    // Skip leading whitespace/BOM for text-based detection
    let trimmed = {
        let s = std::str::from_utf8(&data[..data.len().min(512)]).unwrap_or("");
        s.trim_start()
    };

    // Check for HTML signatures
    let lower = trimmed.to_lowercase();
    if lower.starts_with("<!doctype html")
        || lower.starts_with("<html")
        || lower.starts_with("<?xml") && lower.contains("<html")
    {
        return "text/html";
    }

    // Check for PDF magic bytes
    if data.len() >= 5 && &data[..5] == b"%PDF-" {
        return "application/pdf";
    }

    // Check for ZIP/DOCX magic bytes
    if data.len() >= 4 && &data[..4] == b"PK\x03\x04" {
        // Could be DOCX, XLSX, ZIP, etc. — leave as declared
        return declared;
    }

    // No override detected
    declared
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docx_xml_extraction() {
        let xml = r#"<?xml version="1.0"?>
<w:document>
  <w:body>
    <w:p><w:r><w:t>Hello World</w:t></w:r></w:p>
    <w:p><w:r><w:t>Second paragraph</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let result = extract_text_from_docx_xml(xml);
        assert!(result.contains("Hello World"));
        assert!(result.contains("Second paragraph"));
    }

    #[test]
    fn test_docx_xml_with_spaces() {
        let xml = r#"<w:p><w:r><w:t xml:space="preserve">word one </w:t></w:r><w:r><w:t>word two</w:t></w:r></w:p>"#;
        let result = extract_text_from_docx_xml(xml);
        assert!(result.contains("word one"));
        assert!(result.contains("word two"));
    }

    #[test]
    fn test_pdf_extractor_can_handle() {
        let ext = PdfExtractor;
        assert!(ext.can_handle("application/pdf", "test.pdf"));
        assert!(ext.can_handle("application/octet-stream", "report.pdf"));
        assert!(!ext.can_handle("text/plain", "notes.txt"));
    }

    #[test]
    fn test_docx_extractor_can_handle() {
        let ext = DocxExtractor;
        assert!(ext.can_handle(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "doc.docx"
        ));
        assert!(ext.can_handle("application/octet-stream", "resume.docx"));
        assert!(!ext.can_handle("text/plain", "notes.txt"));
    }

    #[test]
    fn test_processor_has_extractors() {
        let p = DocumentProcessor::new();
        assert_eq!(p.extractors.len(), 3, "should have PDF, DOCX, and HTML extractors");
    }

    #[test]
    fn test_html_extractor_can_handle() {
        let ext = HtmlExtractor;
        assert!(ext.can_handle("text/html", "page.html"));
        assert!(ext.can_handle("application/xhtml+xml", "page.xhtml"));
        assert!(ext.can_handle("application/octet-stream", "page.htm"));
        assert!(!ext.can_handle("application/pdf", "doc.pdf"));
    }

    #[test]
    fn test_strip_html_basic() {
        let html = "<html><body><p>Hello World</p><p>Second paragraph</p></body></html>";
        let text = strip_html_to_text(html);
        assert!(text.contains("Hello World"));
        assert!(text.contains("Second paragraph"));
    }

    #[test]
    fn test_strip_html_removes_script_and_style() {
        let html = r#"<html><head><style>body { color: red; }</style></head>
            <body><script>alert('hi');</script><p>Visible text</p></body></html>"#;
        let text = strip_html_to_text(html);
        assert!(text.contains("Visible text"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("color: red"));
    }

    #[test]
    fn test_strip_html_decodes_entities() {
        let html = "<p>A &amp; B &lt; C &gt; D &quot;E&quot;</p>";
        let text = strip_html_to_text(html);
        assert!(text.contains("A & B < C > D \"E\""));
    }

    #[test]
    fn test_sniff_html_in_pdf() {
        let data = b"<!DOCTYPE html><html><body>Not a PDF</body></html>";
        assert_eq!(sniff_content_type(data, "application/pdf"), "text/html");
    }

    #[test]
    fn test_sniff_real_pdf() {
        let data = b"%PDF-1.4 fake pdf content";
        assert_eq!(sniff_content_type(data, "application/pdf"), "application/pdf");
    }

    #[test]
    fn test_sniff_unknown_preserves_declared() {
        let data = b"\x00\x01\x02\x03 random binary";
        assert_eq!(sniff_content_type(data, "application/octet-stream"), "application/octet-stream");
    }
}
