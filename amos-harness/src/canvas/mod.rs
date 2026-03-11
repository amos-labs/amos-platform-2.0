//! Canvas engine for dynamic UI generation and rendering
//!
//! Canvases are the primary UI mechanism in AMOS. They can be:
//! - Dynamic: Data-driven views with templates
//! - Freeform: Custom HTML/JS/CSS created by AI or users

pub mod generator;
pub mod renderer;
pub mod templates;
pub mod types;

pub use types::{Canvas, CanvasResponse, CanvasTemplate, CanvasType, DataSource};

use amos_core::{AmosError, AppConfig, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Columns we SELECT from the `canvases` table to match the Canvas struct.
/// The table has additional columns (ui_mode, template_key, locking, etc.)
/// that we don't need in every query.
const CANVAS_COLUMNS: &str = r#"
    id, slug, name, description, html_content, js_content, css_content,
    canvas_type, data_sources, actions, layout_config, version,
    is_public, public_slug, is_system, nav_icon, nav_order,
    metadata, created_at, updated_at
"#;

/// The canvas engine handles all canvas operations
#[allow(dead_code)]
pub struct CanvasEngine {
    db_pool: PgPool,
    config: Arc<AppConfig>,
}

impl CanvasEngine {
    /// Create a new canvas engine
    pub fn new(db_pool: PgPool, config: Arc<AppConfig>) -> Self {
        Self { db_pool, config }
    }

    /// Render a canvas with the given data context
    pub async fn render_canvas(
        &self,
        canvas: &Canvas,
        data_context: Option<JsonValue>,
    ) -> Result<CanvasResponse> {
        renderer::render_canvas(canvas, data_context, &self.db_pool).await
    }

    /// Create a new canvas
    pub async fn create_canvas(
        &self,
        name: String,
        description: Option<String>,
        canvas_type: CanvasType,
        html_content: String,
        js_content: Option<String>,
        css_content: Option<String>,
        data_sources: Option<JsonValue>,
        actions: Option<JsonValue>,
        layout_config: Option<JsonValue>,
    ) -> Result<Canvas> {
        let slug = generate_slug(&name);

        let query = format!(
            r#"
            INSERT INTO canvases (
                slug, name, description, html_content, js_content, css_content,
                canvas_type, data_sources, actions, layout_config, version
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1)
            RETURNING {}
            "#,
            CANVAS_COLUMNS
        );

        let canvas = sqlx::query_as::<_, Canvas>(&query)
            .bind(&slug)
            .bind(&name)
            .bind(description)
            .bind(&html_content)
            .bind(js_content)
            .bind(css_content)
            .bind(&canvas_type)
            .bind(data_sources)
            .bind(actions)
            .bind(layout_config)
            .fetch_one(&self.db_pool)
            .await
            .map_err(|e| {
                AmosError::Internal(format!("Database: Failed to create canvas: {}", e))
            })?;

        Ok(canvas)
    }

    /// Update an existing canvas
    pub async fn update_canvas(&self, canvas_id: Uuid, updates: CanvasUpdate) -> Result<Canvas> {
        // Build dynamic update query based on which fields are provided
        let mut set_parts: Vec<String> = vec!["version = version + 1".to_string()];
        let mut bind_idx = 1u32;

        if updates.name.is_some() {
            set_parts.push(format!("name = ${}", bind_idx));
            bind_idx += 1;
        }
        if updates.description.is_some() {
            set_parts.push(format!("description = ${}", bind_idx));
            bind_idx += 1;
        }
        if updates.html_content.is_some() {
            set_parts.push(format!("html_content = ${}", bind_idx));
            bind_idx += 1;
        }
        if updates.js_content.is_some() {
            set_parts.push(format!("js_content = ${}", bind_idx));
            bind_idx += 1;
        }
        if updates.css_content.is_some() {
            set_parts.push(format!("css_content = ${}", bind_idx));
            bind_idx += 1;
        }
        if updates.data_sources.is_some() {
            set_parts.push(format!("data_sources = ${}", bind_idx));
            bind_idx += 1;
        }
        if updates.actions.is_some() {
            set_parts.push(format!("actions = ${}", bind_idx));
            bind_idx += 1;
        }

        let query = format!(
            "UPDATE canvases SET {}, updated_at = NOW() WHERE id = ${} RETURNING {}",
            set_parts.join(", "),
            bind_idx,
            CANVAS_COLUMNS
        );

        let mut query_builder = sqlx::query_as::<_, Canvas>(&query);

        if let Some(name) = updates.name {
            query_builder = query_builder.bind(name);
        }
        if let Some(description) = updates.description {
            query_builder = query_builder.bind(description);
        }
        if let Some(html_content) = updates.html_content {
            query_builder = query_builder.bind(html_content);
        }
        if let Some(js_content) = updates.js_content {
            query_builder = query_builder.bind(js_content);
        }
        if let Some(css_content) = updates.css_content {
            query_builder = query_builder.bind(css_content);
        }
        if let Some(data_sources) = updates.data_sources {
            query_builder = query_builder.bind(data_sources);
        }
        if let Some(actions) = updates.actions {
            query_builder = query_builder.bind(actions);
        }

        query_builder = query_builder.bind(canvas_id);

        let canvas = query_builder.fetch_one(&self.db_pool).await.map_err(|e| {
            AmosError::Internal(format!("Database: Failed to update canvas: {}", e))
        })?;

        Ok(canvas)
    }

    /// List all canvases (excludes system canvases by default)
    pub async fn list_canvases(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<Canvas>> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);

        let query = format!(
            "SELECT {} FROM canvases WHERE is_system = false ORDER BY updated_at DESC LIMIT $1 OFFSET $2",
            CANVAS_COLUMNS
        );

        let canvases = sqlx::query_as::<_, Canvas>(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.db_pool)
            .await
            .map_err(|e| {
                AmosError::Internal(format!("Database: Failed to list canvases: {}", e))
            })?;

        Ok(canvases)
    }

    /// List system canvases (for navigation sidebar)
    pub async fn list_system_canvases(&self) -> Result<Vec<Canvas>> {
        let query = format!(
            "SELECT {} FROM canvases WHERE is_system = true ORDER BY nav_order ASC",
            CANVAS_COLUMNS
        );

        let canvases = sqlx::query_as::<_, Canvas>(&query)
            .fetch_all(&self.db_pool)
            .await
            .map_err(|e| {
                AmosError::Internal(format!("Database: Failed to list system canvases: {}", e))
            })?;

        Ok(canvases)
    }

    /// Get a canvas by ID
    pub async fn get_canvas(&self, canvas_id: Uuid) -> Result<Canvas> {
        let query = format!("SELECT {} FROM canvases WHERE id = $1", CANVAS_COLUMNS);

        let canvas = sqlx::query_as::<_, Canvas>(&query)
            .bind(canvas_id)
            .fetch_one(&self.db_pool)
            .await
            .map_err(|_e| AmosError::NotFound {
                entity: "Canvas".to_string(),
                id: canvas_id.to_string(),
            })?;

        Ok(canvas)
    }

    /// Get a canvas by slug
    pub async fn get_canvas_by_slug(&self, slug: &str) -> Result<Canvas> {
        let query = format!("SELECT {} FROM canvases WHERE slug = $1", CANVAS_COLUMNS);

        let canvas = sqlx::query_as::<_, Canvas>(&query)
            .bind(slug)
            .fetch_one(&self.db_pool)
            .await
            .map_err(|_e| AmosError::NotFound {
                entity: "Canvas".to_string(),
                id: slug.to_string(),
            })?;

        Ok(canvas)
    }

    /// Delete a canvas
    pub async fn delete_canvas(&self, canvas_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM canvases WHERE id = $1")
            .bind(canvas_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| {
                AmosError::Internal(format!("Database: Failed to delete canvas: {}", e))
            })?;

        Ok(())
    }

    /// Publish a canvas (make it publicly accessible)
    pub async fn publish_canvas(&self, canvas_id: Uuid) -> Result<String> {
        let canvas = self.get_canvas(canvas_id).await?;
        let public_slug = generate_public_slug(&canvas.slug);

        sqlx::query(
            r#"
            UPDATE canvases
            SET is_public = true, public_slug = $1, published_at = NOW()
            WHERE id = $2
            "#,
        )
        .bind(&public_slug)
        .bind(canvas_id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Database: Failed to publish canvas: {}", e)))?;

        Ok(public_slug)
    }

    /// Unpublish a canvas
    pub async fn unpublish_canvas(&self, canvas_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE canvases
            SET is_public = false, public_slug = NULL
            WHERE id = $1
            "#,
        )
        .bind(canvas_id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Database: Failed to unpublish canvas: {}", e)))?;

        Ok(())
    }

    /// Get a public canvas by its public slug
    pub async fn get_public_canvas(&self, public_slug: &str) -> Result<Canvas> {
        let query = format!(
            "SELECT {} FROM canvases WHERE public_slug = $1 AND is_public = true",
            CANVAS_COLUMNS
        );

        let canvas = sqlx::query_as::<_, Canvas>(&query)
            .bind(public_slug)
            .fetch_one(&self.db_pool)
            .await
            .map_err(|_e| AmosError::NotFound {
                entity: "PublicCanvas".to_string(),
                id: public_slug.to_string(),
            })?;

        Ok(canvas)
    }
}

/// Updates that can be applied to a canvas
#[derive(Debug, Default)]
pub struct CanvasUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub html_content: Option<String>,
    pub js_content: Option<String>,
    pub css_content: Option<String>,
    pub data_sources: Option<JsonValue>,
    pub actions: Option<JsonValue>,
}

/// Generate a URL-safe slug from a name
fn generate_slug(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c.is_whitespace() || c == '-' {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Generate a unique public slug
fn generate_public_slug(base_slug: &str) -> String {
    use rand::Rng;
    let suffix: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();

    format!("{}-{}", base_slug, suffix.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_slug() {
        assert_eq!(generate_slug("My Canvas"), "my-canvas");
        assert_eq!(
            generate_slug("Sales Dashboard 2024"),
            "sales-dashboard-2024"
        );
        assert_eq!(generate_slug("User@Profile!"), "user_profile_");
    }
}
