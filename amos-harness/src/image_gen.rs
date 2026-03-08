//! Image generation via Google Cloud Imagen API (Nano Banana model)
//!
//! Provides a reusable image generation service for:
//! - Agent chat (generate images on request)
//! - Landing pages & websites (hero images, banners, illustrations)
//! - Document illustrations
//! - Any other image generation need across the platform
//!
//! The harness handles the API call; the AI agent just provides a text prompt
//! and receives back a URL or raw bytes it can embed.

use anyhow::{bail, Context, Result};
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Configuration ───────────────────────────────────────────────────────────

/// Configuration for the Google Imagen API client.
#[derive(Debug, Clone)]
pub struct ImageGenConfig {
    /// Google Cloud project ID
    pub project_id: String,
    /// Google Cloud region (e.g. "us-central1")
    pub region: String,
    /// API key or OAuth token for authentication
    pub api_key: Option<String>,
    /// OAuth2 access token (preferred over API key for production)
    pub access_token: Option<String>,
    /// Model name (default: "imagen-3.0-generate-002" — Nano Banana)
    pub model: String,
}

impl ImageGenConfig {
    /// Build configuration from environment variables.
    ///
    /// Expected env vars:
    /// - `GOOGLE_CLOUD_PROJECT` — project ID
    /// - `GOOGLE_CLOUD_REGION` — region (default: "us-central1")
    /// - `GOOGLE_API_KEY` — API key (optional, use one of key or token)
    /// - `GOOGLE_ACCESS_TOKEN` — OAuth2 access token (optional)
    /// - `IMAGEN_MODEL` — model name (default: "imagen-3.0-generate-002")
    pub fn from_env() -> Option<Self> {
        let project_id = std::env::var("GOOGLE_CLOUD_PROJECT").ok()?;
        let region = std::env::var("GOOGLE_CLOUD_REGION").unwrap_or_else(|_| "us-central1".into());
        let api_key = std::env::var("GOOGLE_API_KEY").ok();
        let access_token = std::env::var("GOOGLE_ACCESS_TOKEN").ok();
        let model = std::env::var("IMAGEN_MODEL")
            .unwrap_or_else(|_| "imagen-3.0-generate-002".into());

        // Need at least one auth method
        if api_key.is_none() && access_token.is_none() {
            tracing::warn!("Image generation disabled: no GOOGLE_API_KEY or GOOGLE_ACCESS_TOKEN");
            return None;
        }

        Some(Self {
            project_id,
            region,
            api_key,
            access_token,
            model,
        })
    }
}

// ── Request / Response types ────────────────────────────────────────────────

/// Parameters for an image generation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenRequest {
    /// Text prompt describing the desired image
    pub prompt: String,
    /// Number of images to generate (1–4, default 1)
    #[serde(default = "default_count")]
    pub count: u8,
    /// Image aspect ratio (default: "1:1")
    #[serde(default = "default_aspect_ratio")]
    pub aspect_ratio: String,
    /// Optional negative prompt (what to avoid)
    pub negative_prompt: Option<String>,
    /// Optional style hint (e.g. "photorealistic", "watercolor", "flat illustration")
    pub style: Option<String>,
}

fn default_count() -> u8 { 1 }
fn default_aspect_ratio() -> String { "1:1".into() }

/// A single generated image.
#[derive(Debug, Clone)]
pub struct GeneratedImage {
    /// Raw image bytes (PNG format)
    pub bytes: Vec<u8>,
    /// MIME type (always "image/png" for Imagen)
    pub mime_type: String,
}

/// Result of an image generation request.
#[derive(Debug)]
pub struct ImageGenResponse {
    /// Generated images
    pub images: Vec<GeneratedImage>,
}

// ── Google API request/response shapes ──────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleImagenRequest {
    instances: Vec<GoogleImagenInstance>,
    parameters: GoogleImagenParameters,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleImagenInstance {
    prompt: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleImagenParameters {
    sample_count: u8,
    aspect_ratio: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    negative_prompt: Option<String>,
}

#[derive(Deserialize)]
struct GoogleImagenResponse {
    predictions: Option<Vec<GoogleImagenPrediction>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleImagenPrediction {
    bytes_base64_encoded: String,
    mime_type: String,
}

// ── Client ──────────────────────────────────────────────────────────────────

/// Google Imagen API client for image generation.
///
/// Designed to be held in `AppState` as `Arc<ImageGenClient>` and shared
/// across all request handlers and tools.
#[derive(Clone)]
pub struct ImageGenClient {
    config: ImageGenConfig,
    http: Client,
}

impl ImageGenClient {
    /// Create a new client from configuration.
    pub fn new(config: ImageGenConfig) -> Self {
        Self {
            config,
            http: Client::new(),
        }
    }

    /// Try to create a client from environment variables.
    /// Returns `None` if required env vars are missing.
    pub fn from_env() -> Option<Self> {
        ImageGenConfig::from_env().map(Self::new)
    }

    /// Generate images from a text prompt.
    pub async fn generate(&self, request: &ImageGenRequest) -> Result<ImageGenResponse> {
        let count = request.count.clamp(1, 4);

        // Build the prompt (optionally prepend style hint)
        let full_prompt = if let Some(ref style) = request.style {
            format!("{style} style: {}", request.prompt)
        } else {
            request.prompt.clone()
        };

        // Build Google API request
        let api_request = GoogleImagenRequest {
            instances: vec![GoogleImagenInstance {
                prompt: full_prompt,
            }],
            parameters: GoogleImagenParameters {
                sample_count: count,
                aspect_ratio: request.aspect_ratio.clone(),
                negative_prompt: request.negative_prompt.clone(),
            },
        };

        // Build URL
        // https://us-central1-aiplatform.googleapis.com/v1/projects/{PROJECT}/locations/{REGION}/publishers/google/models/{MODEL}:predict
        let url = format!(
            "https://{region}-aiplatform.googleapis.com/v1/projects/{project}/locations/{region}/publishers/google/models/{model}:predict",
            region = self.config.region,
            project = self.config.project_id,
            model = self.config.model,
        );

        // Build request with auth
        let mut http_req = self.http.post(&url).json(&api_request);

        if let Some(ref token) = self.config.access_token {
            http_req = http_req.bearer_auth(token);
        } else if let Some(ref key) = self.config.api_key {
            http_req = http_req.query(&[("key", key.as_str())]);
        }

        tracing::info!(
            "Generating {} image(s) with Imagen model '{}': {:?}",
            count,
            self.config.model,
            &request.prompt.chars().take(100).collect::<String>(),
        );

        let response = http_req
            .send()
            .await
            .context("Failed to send request to Google Imagen API")?;

        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            bail!(
                "Google Imagen API returned {} — {}",
                status.as_u16(),
                body.chars().take(500).collect::<String>()
            );
        }

        let api_response: GoogleImagenResponse = response
            .json()
            .await
            .context("Failed to parse Google Imagen API response")?;

        let images = api_response
            .predictions
            .unwrap_or_default()
            .into_iter()
            .filter_map(|pred| {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(&pred.bytes_base64_encoded)
                    .ok()?;
                Some(GeneratedImage {
                    bytes,
                    mime_type: pred.mime_type,
                })
            })
            .collect::<Vec<_>>();

        if images.is_empty() {
            bail!("Google Imagen API returned no images");
        }

        tracing::info!("Successfully generated {} image(s)", images.len());

        Ok(ImageGenResponse { images })
    }

    /// Convenience: generate a single image and return raw bytes + mime type.
    pub async fn generate_one(
        &self,
        prompt: &str,
        aspect_ratio: Option<&str>,
        style: Option<&str>,
    ) -> Result<GeneratedImage> {
        let request = ImageGenRequest {
            prompt: prompt.to_string(),
            count: 1,
            aspect_ratio: aspect_ratio.unwrap_or("1:1").to_string(),
            negative_prompt: None,
            style: style.map(|s| s.to_string()),
        };

        let mut response = self.generate(&request).await?;
        response
            .images
            .pop()
            .context("No image returned from generation")
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        // When env vars are not set, from_env returns None
        // (We can't rely on env vars being set in unit tests)
        // Instead test the manual construction
        let config = ImageGenConfig {
            project_id: "test-project".into(),
            region: "us-central1".into(),
            api_key: Some("test-key".into()),
            access_token: None,
            model: "imagen-3.0-generate-002".into(),
        };
        assert_eq!(config.project_id, "test-project");
        assert_eq!(config.model, "imagen-3.0-generate-002");
    }

    #[test]
    fn test_request_defaults() {
        let req: ImageGenRequest = serde_json::from_str(r#"{"prompt": "a cat"}"#).unwrap();
        assert_eq!(req.count, 1);
        assert_eq!(req.aspect_ratio, "1:1");
        assert!(req.negative_prompt.is_none());
        assert!(req.style.is_none());
    }

    #[test]
    fn test_export_format_serde() {
        let req = ImageGenRequest {
            prompt: "test".into(),
            count: 2,
            aspect_ratio: "16:9".into(),
            negative_prompt: Some("blurry".into()),
            style: Some("watercolor".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("watercolor"));
        assert!(json.contains("16:9"));
    }

    #[test]
    fn test_client_construction() {
        let config = ImageGenConfig {
            project_id: "test".into(),
            region: "us-central1".into(),
            api_key: Some("key".into()),
            access_token: None,
            model: "imagen-3.0-generate-002".into(),
        };
        let _client = ImageGenClient::new(config);
        // Just verifying it constructs without panicking
    }
}
