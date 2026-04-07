//! Content calendar management tools.

use amos_core::tools::{Tool, ToolCategory, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use tracing::info;

// =============================================================================
// LoadContentCalendarTool
// =============================================================================

pub struct LoadContentCalendarTool {
    db_pool: PgPool,
}

impl LoadContentCalendarTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for LoadContentCalendarTool {
    fn name(&self) -> &str {
        "load_content_calendar"
    }

    fn description(&self) -> &str {
        "Load a content calendar from a markdown file or from schema records. \
         Parses the calendar into a structured list of scheduled posts with \
         platform, content reference, scheduled date, and status."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "enum": ["file", "schema"],
                    "description": "Load from a markdown file or from schema records"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to the content calendar markdown file (when source=file)"
                },
                "collection": {
                    "type": "string",
                    "description": "Schema collection name (when source=schema, default: 'content_calendar')"
                }
            },
            "required": ["source"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }

    async fn execute(&self, params: JsonValue) -> amos_core::Result<ToolResult> {
        let source = params
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Internal("Missing 'source' parameter".into())
            })?;

        match source {
            "file" => {
                let file_path = params
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        amos_core::AmosError::Internal(
                            "Missing 'file_path' when source=file".into(),
                        )
                    })?;

                let content = tokio::fs::read_to_string(file_path).await.map_err(|e| {
                    amos_core::AmosError::Internal(format!(
                        "Failed to read calendar file '{}': {}",
                        file_path, e
                    ))
                })?;

                let calendar = parse_markdown_calendar(&content);
                let platforms: Vec<String> = calendar
                    .iter()
                    .map(|item| {
                        item.get("platform")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string()
                    })
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();

                info!(
                    items = calendar.len(),
                    file = file_path,
                    "Content calendar loaded from file"
                );

                Ok(ToolResult {
                    success: true,
                    data: Some(json!({
                        "calendar": calendar,
                        "total_items": calendar.len(),
                        "platforms": platforms,
                        "source": "file",
                        "file_path": file_path,
                    })),
                    error: None,
                    metadata: None,
                })
            }
            "schema" => {
                let collection = params
                    .get("collection")
                    .and_then(|v| v.as_str())
                    .unwrap_or("content_calendar");

                let calendar = super::twitter::query_collection_records(&self.db_pool, collection)
                    .await
                    .map_err(|e| amos_core::AmosError::Internal(e))?;

                let platforms: Vec<String> = calendar
                    .iter()
                    .filter_map(|item| item.get("platform").and_then(|v| v.as_str()))
                    .map(|s| s.to_string())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();

                info!(
                    items = calendar.len(),
                    collection = collection,
                    "Content calendar loaded from schema"
                );

                Ok(ToolResult {
                    success: true,
                    data: Some(json!({
                        "calendar": calendar,
                        "total_items": calendar.len(),
                        "platforms": platforms,
                        "source": "schema",
                        "collection": collection,
                    })),
                    error: None,
                    metadata: None,
                })
            }
            _ => Ok(ToolResult {
                success: false,
                data: None,
                error: Some(format!("Unknown source '{}'. Use 'file' or 'schema'.", source)),
                metadata: None,
            }),
        }
    }
}

// =============================================================================
// ScheduleContentTool
// =============================================================================

pub struct ScheduleContentTool {
    db_pool: PgPool,
}

impl ScheduleContentTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for ScheduleContentTool {
    fn name(&self) -> &str {
        "schedule_content"
    }

    fn description(&self) -> &str {
        "Schedule a content item for posting at a specific date/time. \
         Creates a record in the content_schedule schema that can be \
         picked up by the automation engine or converted to a bounty. \
         Integrates with the existing automation and bounty systems."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "platform": {
                    "type": "string",
                    "enum": ["twitter", "twitter_thread", "linkedin", "reddit", "hackernews"],
                    "description": "Target platform"
                },
                "connection_id": {
                    "type": "string",
                    "description": "UUID of the platform API connection"
                },
                "content": {
                    "type": "object",
                    "description": "Content payload matching the target tool's schema"
                },
                "scheduled_at": {
                    "type": "string",
                    "description": "ISO 8601 datetime for when to post"
                },
                "label": {
                    "type": "string",
                    "description": "Human-readable label (e.g., 'Week 1 - Thread 1 - Macro Thesis')"
                },
                "create_bounty": {
                    "type": "boolean",
                    "description": "If true, create a bounty for this post instead of scheduling direct execution (default: false)"
                },
                "bounty_reward": {
                    "type": "integer",
                    "description": "Reward tokens for the bounty (required if create_bounty=true)"
                }
            },
            "required": ["platform", "connection_id", "content", "scheduled_at"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }

    async fn execute(&self, params: JsonValue) -> amos_core::Result<ToolResult> {
        let platform = params
            .get("platform")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Internal("Missing 'platform' parameter".into())
            })?;
        let connection_id = params
            .get("connection_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Internal("Missing 'connection_id' parameter".into())
            })?;
        let content = params.get("content").ok_or_else(|| {
            amos_core::AmosError::Internal("Missing 'content' parameter".into())
        })?;
        let scheduled_at = params
            .get("scheduled_at")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Internal("Missing 'scheduled_at' parameter".into())
            })?;
        let label = params
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let create_bounty = params
            .get("create_bounty")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Map platform to tool name
        let tool_name = match platform {
            "twitter" => "post_tweet",
            "twitter_thread" => "post_thread",
            "linkedin" => "post_linkedin",
            "reddit" => "post_reddit",
            "hackernews" => "post_hackernews",
            _ => {
                return Ok(ToolResult {
                    success: false,
                    data: None,
                    error: Some(format!("Unknown platform: {}", platform)),
                    metadata: None,
                })
            }
        };

        // Store in content_schedule collection
        let schedule_record = json!({
            "platform": platform,
            "tool": tool_name,
            "connection_id": connection_id,
            "content_payload": content,
            "scheduled_at": scheduled_at,
            "label": label,
            "status": if create_bounty { "bounty_pending" } else { "scheduled" },
            "create_bounty": create_bounty,
        });

        let schedule_id = super::twitter::insert_collection_record(
            &self.db_pool,
            "content_schedule",
            &schedule_record,
        )
        .await
        .map_err(|e| amos_core::AmosError::Internal(e))?;

        info!(
            schedule_id = %schedule_id,
            platform = platform,
            scheduled_at = scheduled_at,
            label = label,
            "Content scheduled"
        );

        Ok(ToolResult {
            success: true,
            data: Some(json!({
                "schedule_id": schedule_id.to_string(),
                "platform": platform,
                "tool": tool_name,
                "scheduled_at": scheduled_at,
                "label": label,
                "status": if create_bounty { "bounty_pending" } else { "scheduled" },
                "create_bounty": create_bounty,
            })),
            error: None,
            metadata: None,
        })
    }
}

// =============================================================================
// Markdown calendar parser
// =============================================================================

/// Parse a markdown content calendar table into structured items.
///
/// Expects tables with headers like: Week | Platform | Type | Description | Goal
fn parse_markdown_calendar(content: &str) -> Vec<JsonValue> {
    let mut items = Vec::new();
    let mut headers: Vec<String> = Vec::new();
    let mut in_table = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect table rows (lines starting with |)
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let cells: Vec<&str> = trimmed
                .split('|')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();

            if cells.is_empty() {
                continue;
            }

            // Skip separator rows (---|---|---)
            if cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':')) {
                in_table = true;
                continue;
            }

            if !in_table && headers.is_empty() {
                // This is the header row
                headers = cells.iter().map(|c| c.to_lowercase().replace(' ', "_")).collect();
                continue;
            }

            if in_table && !headers.is_empty() {
                let mut item = json!({});
                for (i, cell) in cells.iter().enumerate() {
                    if let Some(header) = headers.get(i) {
                        item[header] = json!(cell);
                    }
                }
                // Normalize platform names
                if let Some(platform) = item.get("platform").and_then(|v| v.as_str()) {
                    let lowered = platform.to_lowercase();
                    let normalized = match lowered.as_str() {
                        "twitter" | "twitter/x" | "x" => "twitter",
                        "linkedin" => "linkedin",
                        "reddit" => "reddit",
                        "hacker news" | "hackernews" | "hn" => "hackernews",
                        _ => &lowered,
                    };
                    item["platform"] = json!(normalized);
                }
                item["status"] = json!("pending");
                items.push(item);
            }
        } else {
            // Reset table state on non-table lines
            if in_table {
                in_table = false;
                headers.clear();
            }
        }
    }

    items
}
