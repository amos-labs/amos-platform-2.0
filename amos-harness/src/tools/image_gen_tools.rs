//! Image generation tools for the AI agent
//!
//! Allows the agent to generate images via the Google Imagen API.
//! Reusable across chat, landing pages, websites, documents, and more.

use crate::image_gen::{ImageGenClient, ImageGenRequest};
use crate::tools::{Tool, ToolCategory, ToolResult};
use amos_core::Result;
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};

/// Tool: generate_image
///
/// Generates an image from a text prompt using Google Imagen (Nano Banana).
/// Returns base64-encoded image data that can be served to the user or
/// embedded in documents, landing pages, etc.
pub struct GenerateImageTool;

impl GenerateImageTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GenerateImageTool {
    fn name(&self) -> &str {
        "generate_image"
    }

    fn description(&self) -> &str {
        "Generate an image from a text prompt using AI image generation. Provide a detailed description of the desired image. Supports various aspect ratios and styles. Returns the generated image."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "Detailed text description of the image to generate. Be specific about subject, composition, lighting, style, etc."
                },
                "count": {
                    "type": "integer",
                    "description": "Number of images to generate (1-4). Default: 1",
                    "minimum": 1,
                    "maximum": 4
                },
                "aspect_ratio": {
                    "type": "string",
                    "description": "Image aspect ratio. Options: '1:1' (square), '16:9' (landscape), '9:16' (portrait), '4:3', '3:4'. Default: '1:1'",
                    "enum": ["1:1", "16:9", "9:16", "4:3", "3:4"]
                },
                "negative_prompt": {
                    "type": "string",
                    "description": "What to avoid in the image (e.g. 'blurry, low quality, text')"
                },
                "style": {
                    "type": "string",
                    "description": "Style hint for the image (e.g. 'photorealistic', 'watercolor', 'flat illustration', 'oil painting', 'digital art', '3D render')"
                }
            },
            "required": ["prompt"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::ImageGen
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let prompt = match params.get("prompt").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                return Ok(ToolResult::error(
                    "Missing required 'prompt' parameter".to_string(),
                ));
            }
        };

        // Parse optional parameters
        let count = params
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u8;
        let aspect_ratio = params
            .get("aspect_ratio")
            .and_then(|v| v.as_str())
            .unwrap_or("1:1");
        let negative_prompt = params
            .get("negative_prompt")
            .and_then(|v| v.as_str())
            .map(String::from);
        let style = params
            .get("style")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Try to create a client from env (the client checks for credentials)
        let client = match ImageGenClient::from_env() {
            Some(c) => c,
            None => {
                return Ok(ToolResult::error(
                    "Image generation is not configured. Set GOOGLE_CLOUD_PROJECT and GOOGLE_API_KEY or GOOGLE_ACCESS_TOKEN environment variables.".to_string(),
                ));
            }
        };

        let request = ImageGenRequest {
            prompt: prompt.to_string(),
            count,
            aspect_ratio: aspect_ratio.to_string(),
            negative_prompt,
            style,
        };

        match client.generate(&request).await {
            Ok(response) => {
                let images: Vec<JsonValue> = response
                    .images
                    .iter()
                    .enumerate()
                    .map(|(i, img)| {
                        let b64 = base64::Engine::encode(
                            &base64::engine::general_purpose::STANDARD,
                            &img.bytes,
                        );
                        json!({
                            "index": i,
                            "mime_type": img.mime_type,
                            "size_bytes": img.bytes.len(),
                            "data_base64": b64,
                        })
                    })
                    .collect();

                Ok(ToolResult::success_with_metadata(
                    json!({
                        "images": images,
                        "count": response.images.len(),
                        "message": format!(
                            "Generated {} image(s) from prompt: {}",
                            response.images.len(),
                            &prompt.chars().take(80).collect::<String>()
                        )
                    }),
                    json!({
                        "prompt": prompt,
                        "aspect_ratio": aspect_ratio,
                        "generated": true
                    }),
                ))
            }
            Err(e) => Ok(ToolResult::error(format!(
                "Image generation failed: {}",
                e
            ))),
        }
    }
}
