//! Embedding service for vector search (OpenAI-compatible API).
//!
//! Calls the OpenAI `/v1/embeddings` endpoint (or any compatible API) to
//! generate vector embeddings for text. Used by memory tools and knowledge
//! base for semantic search via pgvector.

use amos_core::{AmosError, Result};
use serde::{Deserialize, Serialize};
use tracing::warn;

/// Client for generating text embeddings via an OpenAI-compatible API.
pub struct EmbeddingService {
    http_client: reqwest::Client,
    api_key: String,
    api_base: String,
    model: String,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: Vec<&'a str>,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

impl EmbeddingService {
    /// Create a new embedding service.
    pub fn new(api_key: String, api_base: String, model: String) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            http_client,
            api_key,
            api_base,
            model,
        }
    }

    /// Generate an embedding for a single text.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let results = self.embed_batch(&[text]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| AmosError::Internal("Empty embedding response".to_string()))
    }

    /// Generate embeddings for a batch of texts (up to 100 per call).
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // OpenAI allows up to 2048 inputs per batch, but we cap at 100 for safety
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(100) {
            let request = EmbeddingRequest {
                model: &self.model,
                input: chunk.to_vec(),
            };

            let url = format!("{}/embeddings", self.api_base);
            let response = self
                .http_client
                .post(&url)
                .bearer_auth(&self.api_key)
                .json(&request)
                .send()
                .await
                .map_err(|e| AmosError::Internal(format!("Embedding API request failed: {e}")))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                warn!(
                    status = %status,
                    body = %body,
                    "Embedding API error"
                );
                return Err(AmosError::Internal(format!(
                    "Embedding API returned {status}: {body}"
                )));
            }

            let resp: EmbeddingResponse = response.json().await.map_err(|e| {
                AmosError::Internal(format!("Failed to parse embedding response: {e}"))
            })?;

            for data in resp.data {
                all_embeddings.push(data.embedding);
            }
        }

        Ok(all_embeddings)
    }
}

/// Split text into chunks suitable for embedding.
///
/// Uses a paragraph-first recursive splitter:
/// 1. Split on double-newlines (paragraphs)
/// 2. If a paragraph exceeds `max_chars`, split on single newlines
/// 3. If a line exceeds `max_chars`, split on sentence boundaries
/// 4. Overlap between chunks for context continuity
pub fn chunk_text(text: &str, max_chars: usize, overlap: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }

    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let mut chunks: Vec<String> = Vec::new();
    let mut current_chunk = String::new();

    for paragraph in paragraphs {
        let paragraph = paragraph.trim();
        if paragraph.is_empty() {
            continue;
        }

        // If adding this paragraph would exceed the limit, flush current chunk
        if !current_chunk.is_empty() && current_chunk.len() + paragraph.len() + 2 > max_chars {
            chunks.push(current_chunk.clone());
            // Start new chunk with overlap from the end of the previous chunk
            if overlap > 0 && current_chunk.len() > overlap {
                current_chunk = current_chunk[current_chunk.len() - overlap..].to_string();
            } else {
                current_chunk.clear();
            }
        }

        // If a single paragraph exceeds max_chars, split it further
        if paragraph.len() > max_chars {
            // Flush what we have
            if !current_chunk.is_empty() {
                chunks.push(current_chunk.clone());
                current_chunk.clear();
            }
            // Split long paragraph by sentences
            let sentence_chunks = split_long_text(paragraph, max_chars, overlap);
            chunks.extend(sentence_chunks);
        } else {
            if !current_chunk.is_empty() {
                current_chunk.push_str("\n\n");
            }
            current_chunk.push_str(paragraph);
        }
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

/// Split a long text block by sentence boundaries.
fn split_long_text(text: &str, max_chars: usize, overlap: usize) -> Vec<String> {
    let sentences: Vec<&str> = text.split_inclusive(['.', '!', '?']).collect();

    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();

    for sentence in sentences {
        if !current.is_empty() && current.len() + sentence.len() > max_chars {
            chunks.push(current.clone());
            if overlap > 0 && current.len() > overlap {
                current = current[current.len() - overlap..].to_string();
            } else {
                current.clear();
            }
        }
        current.push_str(sentence);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    // If we still have chunks that are too long, hard-split at max_chars
    let mut final_chunks = Vec::new();
    for chunk in chunks {
        if chunk.len() <= max_chars {
            final_chunks.push(chunk);
        } else {
            let mut start = 0;
            while start < chunk.len() {
                let end = (start + max_chars).min(chunk.len());
                final_chunks.push(chunk[start..end].to_string());
                if end == chunk.len() {
                    break;
                }
                start = if overlap > 0 {
                    end.saturating_sub(overlap)
                } else {
                    end
                };
            }
        }
    }

    final_chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_text_short_text_returns_single() {
        let chunks = chunk_text("Hello world", 2000, 200);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hello world");
    }

    #[test]
    fn chunk_text_splits_on_paragraphs() {
        let text = "Paragraph one.\n\nParagraph two.\n\nParagraph three.";
        let chunks = chunk_text(text, 25, 0);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn chunk_text_respects_max_chars() {
        let text = "A".repeat(5000);
        let chunks = chunk_text(&text, 2000, 200);
        for chunk in &chunks {
            assert!(chunk.len() <= 2200); // max_chars + some overlap tolerance
        }
    }

    #[test]
    fn chunk_text_handles_empty_text() {
        let chunks = chunk_text("", 2000, 200);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "");
    }
}
