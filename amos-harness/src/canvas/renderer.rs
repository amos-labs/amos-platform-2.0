//! Canvas rendering logic

use super::types::{Canvas, CanvasResponse, CanvasType};
use amos_core::{AmosError, Result};
use serde_json::Value as JsonValue;
use sqlx::{Column, PgPool, Row};
use std::collections::HashMap;
use std::net::IpAddr;
use tera::{Context, Tera};

/// Validate that a string is a safe SQL identifier (alphanumeric + underscores only)
fn validate_identifier(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 128 {
        return Err(AmosError::Validation(
            "Invalid identifier length".to_string(),
        ));
    }
    let is_valid = name.chars().enumerate().all(|(i, c)| {
        if i == 0 {
            c.is_ascii_alphabetic() || c == '_'
        } else {
            c.is_ascii_alphanumeric() || c == '_'
        }
    });
    if !is_valid {
        return Err(AmosError::Validation(format!(
            "Invalid identifier '{}': only alphanumeric characters and underscores allowed",
            name
        )));
    }
    Ok(())
}

/// Check if an IP address is in a private/internal range
fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()             // 127.0.0.0/8
                || v4.is_private()       // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                || v4.is_link_local()    // 169.254.0.0/16
                || v4.is_unspecified() // 0.0.0.0
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()             // ::1
                || v6.is_unspecified()   // ::
                // fc00::/7 (unique local) and fe80::/10 (link-local)
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Validate that a URL is safe to fetch (no SSRF)
fn validate_url(endpoint: &str) -> Result<()> {
    let parsed = url::Url::parse(endpoint)
        .map_err(|e| AmosError::Validation(format!("Invalid URL '{}': {}", endpoint, e)))?;

    // Only allow http and https
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(AmosError::Validation(format!(
                "URL scheme '{}' not allowed; only http and https are permitted",
                scheme
            )));
        }
    }

    // Block known internal hostnames
    let host = parsed.host_str().unwrap_or("");
    let blocked_hosts = ["localhost", "0.0.0.0", "[::]", "[::1]"];
    if blocked_hosts.contains(&host) || host.ends_with(".local") || host.ends_with(".internal") {
        return Err(AmosError::Validation(format!(
            "URL host '{}' is not allowed: internal/private host",
            host
        )));
    }

    // Resolve and check IPs
    use std::net::ToSocketAddrs;
    let port = parsed.port_or_known_default().unwrap_or(80);
    let addr_str = format!("{}:{}", host, port);
    if let Ok(addrs) = addr_str.to_socket_addrs() {
        for addr in addrs {
            if is_private_ip(addr.ip()) {
                return Err(AmosError::Validation(format!(
                    "URL resolves to private/internal IP {}: not allowed",
                    addr.ip()
                )));
            }
        }
    }

    Ok(())
}

/// Render a canvas with the given data context
pub async fn render_canvas(
    canvas: &Canvas,
    data_context: Option<JsonValue>,
    db_pool: &PgPool,
) -> Result<CanvasResponse> {
    match canvas.canvas_type {
        CanvasType::Dynamic
        | CanvasType::Dashboard
        | CanvasType::DataGrid
        | CanvasType::Form
        | CanvasType::Detail
        | CanvasType::Kanban
        | CanvasType::Calendar
        | CanvasType::Report
        | CanvasType::Wizard => render_dynamic(canvas, data_context, db_pool).await,
        CanvasType::Freeform | CanvasType::Custom => Ok(render_freeform(canvas)),
    }
}

/// Render a dynamic canvas with template interpolation
pub async fn render_dynamic(
    canvas: &Canvas,
    data_context: Option<JsonValue>,
    db_pool: &PgPool,
) -> Result<CanvasResponse> {
    // Fetch data from data sources if configured
    let mut context_data = data_context.unwrap_or(JsonValue::Object(serde_json::Map::new()));

    if let Some(data_sources) = &canvas.data_sources {
        let fetched_data = fetch_data_from_sources(data_sources, db_pool).await?;

        // Merge fetched data into context
        if let JsonValue::Object(ref mut map) = context_data {
            if let JsonValue::Object(fetched_map) = fetched_data {
                for (key, value) in fetched_map {
                    map.insert(key, value);
                }
            }
        }
    }

    // Inject canvas metadata into context (title, columns, etc.)
    if let JsonValue::Object(ref mut map) = context_data {
        // Always provide title from canvas name
        if !map.contains_key("title") {
            map.insert("title".to_string(), JsonValue::String(canvas.name.clone()));
        }

        // Alias first data source items as "items" if not already present
        // Templates use {% for item in items %} — map the first data source to "items"
        if !map.contains_key("items") {
            if let Some(data_sources) = &canvas.data_sources {
                if let Some((_collection_name, data_key)) = extract_collection_info(data_sources) {
                    if let Some(data) = map.get(&data_key).cloned() {
                        map.insert("items".to_string(), data);
                    }
                }
            }
        }

        // Provide columns based on canvas type
        if !map.contains_key("columns") {
            match canvas.canvas_type {
                CanvasType::Kanban => {
                    // Kanban columns: derive from the enum field that defines stages
                    if let Some(columns) = build_kanban_columns(canvas, map, db_pool).await? {
                        map.insert("columns".to_string(), columns);
                    }
                }
                _ => {
                    // Standard columns: field display names from collection
                    if let Some(columns) = fetch_columns_for_canvas(canvas, db_pool).await? {
                        map.insert("columns".to_string(), columns);
                    }
                }
            }
        }
    }

    // Render with Tera template engine
    let html_source = canvas.html_content.as_deref().unwrap_or("");
    let rendered_html = render_with_tera(html_source, &context_data)?;

    Ok(CanvasResponse::new(
        canvas,
        rendered_html,
        None,
        canvas.canvas_type.to_string(),
    ))
}

/// Extract the collection name and data key from data_sources (handles both flat and nested formats)
fn extract_collection_info(data_sources: &JsonValue) -> Option<(String, String)> {
    if let JsonValue::Object(sources) = data_sources {
        // Flat format: {"collection": "support_tickets", "group_by": "status", ...}
        if let Some(collection_name) = sources.get("collection").and_then(|v| v.as_str()) {
            return Some((collection_name.to_string(), collection_name.to_string()));
        }
        // Nested format: {"deals": {"collection": "deals"}}
        for (key, source_config) in sources {
            if let Some(collection_name) = source_config.get("collection").and_then(|v| v.as_str())
            {
                return Some((collection_name.to_string(), key.clone()));
            }
        }
    }
    None
}

/// Build kanban columns from the collection's enum/stage field, grouping items by stage
async fn build_kanban_columns(
    canvas: &Canvas,
    context_map: &serde_json::Map<String, JsonValue>,
    db_pool: &PgPool,
) -> Result<Option<JsonValue>> {
    let data_sources = match &canvas.data_sources {
        Some(ds) => ds,
        None => return Ok(None),
    };

    let (collection_name, data_key) = match extract_collection_info(data_sources) {
        Some(info) => info,
        None => return Ok(None),
    };

    // Check if data_sources specifies a preferred group_by field (flat format)
    let preferred_group_by = data_sources
        .get("group_by")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Fetch collection fields to find the enum field with stage choices
    let fields_row =
        sqlx::query_as::<_, (JsonValue,)>("SELECT fields FROM collections WHERE name = $1 LIMIT 1")
            .bind(&collection_name)
            .fetch_optional(db_pool)
            .await
            .map_err(|e| {
                AmosError::Internal(format!("Failed to fetch collection fields: {}", e))
            })?;

    let (fields,) = match fields_row {
        Some(row) => row,
        None => return Ok(None),
    };

    let field_list = match fields {
        JsonValue::Array(list) => list,
        _ => return Ok(None),
    };

    // Find the enum field to group by
    for field in &field_list {
        let field_type = field
            .get("field_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if field_type != "enum" {
            continue;
        }

        let field_name = field.get("name").and_then(|v| v.as_str()).unwrap_or("");

        // If a preferred group_by was specified, use that field
        if let Some(ref preferred) = preferred_group_by {
            if field_name != preferred {
                continue;
            }
        } else {
            // Default heuristic: prefer "stage" or "status" fields for kanban
            let is_stage = field_name == "stage"
                || field_name.contains("stage")
                || field_name.contains("status");
            if !is_stage
                && field_list.iter().any(|f| {
                    f.get("name").and_then(|v| v.as_str()).is_some_and(|n| {
                        n == "stage" || n.contains("stage") || n.contains("status")
                    })
                })
            {
                continue; // Skip non-stage enums if a stage/status field exists
            }
        }

        let choices = match field
            .get("options")
            .and_then(|o| o.get("choices"))
            .and_then(|c| c.as_array())
        {
            Some(c) => c,
            None => continue,
        };

        // Get the items from context
        let items = context_map
            .get(&data_key)
            .or_else(|| context_map.get("items"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Build columns with items grouped by stage value
        let columns: Vec<JsonValue> = choices
            .iter()
            .map(|choice| {
                let choice_str = choice.as_str().unwrap_or("");
                let column_items: Vec<JsonValue> = items
                    .iter()
                    .filter(|item| {
                        item.get(field_name).and_then(|v| v.as_str()) == Some(choice_str)
                    })
                    .cloned()
                    .collect();

                serde_json::json!({
                    "key": choice_str.to_lowercase().replace(' ', "_"),
                    "name": choice_str,
                    "items": column_items,
                })
            })
            .collect();

        return Ok(Some(JsonValue::Array(columns)));
    }

    Ok(None)
}

/// Fetch column (field) names for a canvas from its first data source collection
async fn fetch_columns_for_canvas(canvas: &Canvas, db_pool: &PgPool) -> Result<Option<JsonValue>> {
    let data_sources = match &canvas.data_sources {
        Some(ds) => ds,
        None => return Ok(None),
    };

    let (collection_name, _data_key) = match extract_collection_info(data_sources) {
        Some(info) => info,
        None => return Ok(None),
    };

    // Fetch collection field display names
    let fields_row =
        sqlx::query_as::<_, (JsonValue,)>("SELECT fields FROM collections WHERE name = $1 LIMIT 1")
            .bind(&collection_name)
            .fetch_optional(db_pool)
            .await
            .map_err(|e| {
                AmosError::Internal(format!("Failed to fetch collection fields: {}", e))
            })?;

    if let Some((fields,)) = fields_row {
        if let JsonValue::Array(field_list) = fields {
            let column_names: Vec<JsonValue> = field_list
                .iter()
                .filter_map(|f| {
                    f.get("display_name")
                        .and_then(|v| v.as_str())
                        .map(|s| JsonValue::String(s.to_string()))
                })
                .collect();
            return Ok(Some(JsonValue::Array(column_names)));
        }
    }

    Ok(None)
}

/// Render a freeform canvas (just wrap in iframe structure)
pub fn render_freeform(canvas: &Canvas) -> CanvasResponse {
    CanvasResponse::freeform(canvas)
}

/// Render HTML content with Tera template engine
fn render_with_tera(template_str: &str, context: &JsonValue) -> Result<String> {
    let mut tera = Tera::default();
    tera.autoescape_on(vec![".html", ".htm", ""]);

    tera.add_raw_template("canvas", template_str)
        .map_err(|e| AmosError::Validation(format!("Invalid template: {}", e)))?;

    let mut tera_context = Context::new();

    // Convert JsonValue to Context
    if let JsonValue::Object(map) = context {
        for (key, value) in map {
            tera_context.insert(key, value);
        }
    }

    let rendered = tera
        .render("canvas", &tera_context)
        .map_err(|e| AmosError::Internal(format!("Template rendering failed: {}", e)))?;

    Ok(rendered)
}

/// Fetch data from configured data sources
///
/// Supports two formats:
/// 1. Nested: `{"contacts": {"collection": "contacts"}}` — each key is a named source
/// 2. Flat: `{"collection": "support_tickets", "group_by": "status", ...}` — single source
async fn fetch_data_from_sources(data_sources: &JsonValue, db_pool: &PgPool) -> Result<JsonValue> {
    let mut result = serde_json::Map::new();

    if let JsonValue::Object(sources) = data_sources {
        // Detect flat format: data_sources itself has a top-level "collection" key with a string value
        // e.g. {"collection": "support_tickets", "group_by": "status", "view_mode": "kanban"}
        if let Some(collection_name) = sources.get("collection").and_then(|v| v.as_str()) {
            let data = fetch_collection_data(collection_name, data_sources, db_pool).await?;
            result.insert(collection_name.to_string(), data);
            return Ok(JsonValue::Object(result));
        }

        for (key, source_config) in sources {
            // Support shorthand collection format: {"contacts": {"collection": "contacts"}}
            if let Some(collection_name) = source_config.get("collection").and_then(|v| v.as_str())
            {
                let data = fetch_collection_data(collection_name, source_config, db_pool).await?;
                result.insert(key.clone(), data);
                continue;
            }

            if let Some(source_type) = source_config.get("source_type").and_then(|v| v.as_str()) {
                match source_type {
                    "model" => {
                        // Fetch data from a database model
                        if let Some(model_name) =
                            source_config.get("model_name").and_then(|v| v.as_str())
                        {
                            let data = fetch_model_data(model_name, source_config, db_pool).await?;
                            result.insert(key.clone(), data);
                        }
                    }
                    "static" => {
                        // Use static data from config
                        if let Some(data) = source_config.get("data") {
                            result.insert(key.clone(), data.clone());
                        }
                    }
                    "api" => {
                        // Fetch from external API
                        if let Some(endpoint) =
                            source_config.get("endpoint").and_then(|v| v.as_str())
                        {
                            let data = fetch_api_data(endpoint).await?;
                            result.insert(key.clone(), data);
                        }
                    }
                    _ => {
                        // Unknown source type, skip
                        continue;
                    }
                }
            }
        }
    }

    Ok(JsonValue::Object(result))
}

/// Fetch data from a collection (records table + collections table)
async fn fetch_collection_data(
    collection_name: &str,
    config: &JsonValue,
    db_pool: &PgPool,
) -> Result<JsonValue> {
    let limit = config.get("limit").and_then(|v| v.as_i64()).unwrap_or(100);

    // Query records joined with collections to get the data JSONB field
    let rows = sqlx::query_as::<_, (sqlx::types::Uuid, JsonValue)>(
        r#"
        SELECT r.id, r.data
        FROM records r
        JOIN collections c ON c.id = r.collection_id
        WHERE c.name = $1
        ORDER BY r.created_at DESC
        LIMIT $2
        "#,
    )
    .bind(collection_name)
    .bind(limit)
    .fetch_all(db_pool)
    .await
    .map_err(|e| AmosError::Internal(format!("Failed to fetch collection data: {}", e)))?;

    // Convert rows to JSON array, flattening the data field
    let records: Vec<JsonValue> = rows
        .into_iter()
        .map(|(id, data)| {
            let mut record = if let JsonValue::Object(map) = data {
                map
            } else {
                serde_json::Map::new()
            };
            record.insert("id".to_string(), JsonValue::String(id.to_string()));
            JsonValue::Object(record)
        })
        .collect();

    Ok(JsonValue::Array(records))
}

/// Fetch data from a database model
async fn fetch_model_data(
    model_name: &str,
    config: &JsonValue,
    db_pool: &PgPool,
) -> Result<JsonValue> {
    // This is a simplified implementation
    // In production, this would use the actual module system to query data

    let limit = config.get("limit").and_then(|v| v.as_i64()).unwrap_or(50);

    // Validate model_name is a safe identifier (prevent SQL injection)
    validate_identifier(model_name)?;

    // Build a basic query with quoted identifier
    let query = format!(
        "SELECT * FROM \"{}\" ORDER BY created_at DESC LIMIT $1",
        model_name
    );

    // Execute query
    let rows = sqlx::query(&query)
        .bind(limit)
        .fetch_all(db_pool)
        .await
        .map_err(|e| AmosError::Internal(format!("Database: Failed to fetch model data: {}", e)))?;

    // Convert rows to JSON
    let mut records = Vec::new();
    for row in rows {
        let mut record = serde_json::Map::new();

        for (i, column) in row.columns().iter().enumerate() {
            let name = column.name();

            // Get value (simplified - in production would handle all types)
            if let Ok(value) = row.try_get::<String, _>(i) {
                record.insert(name.to_string(), JsonValue::String(value));
            } else if let Ok(value) = row.try_get::<i64, _>(i) {
                record.insert(name.to_string(), JsonValue::Number(value.into()));
            }
        }

        records.push(JsonValue::Object(record));
    }

    Ok(JsonValue::Array(records))
}

/// Fetch data from an external API
async fn fetch_api_data(endpoint: &str) -> Result<JsonValue> {
    // Validate URL to prevent SSRF attacks
    validate_url(endpoint)?;

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AmosError::Internal(format!("Failed to build HTTP client: {}", e)))?;

    let response: reqwest::Response = client
        .get(endpoint)
        .send()
        .await
        .map_err(|e| AmosError::Internal(format!("External: API request failed: {}", e)))?;

    let data = response.json::<JsonValue>().await.map_err(|e| {
        AmosError::Internal(format!("External: Failed to parse API response: {}", e))
    })?;

    Ok(data)
}

/// Simple template variable interpolation (fallback if Tera is not available)
pub fn simple_interpolate(template: &str, context: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    for (key, value) in context {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_interpolate() {
        let template = "Hello {{name}}, your balance is {{balance}}!";
        let mut context = HashMap::new();
        context.insert("name".to_string(), "Alice".to_string());
        context.insert("balance".to_string(), "$100".to_string());

        let result = simple_interpolate(template, &context);
        assert_eq!(result, "Hello Alice, your balance is $100!");
    }

    #[test]
    fn test_render_with_tera() {
        let template = "<h1>Hello {{ name }}</h1>";
        let context = serde_json::json!({
            "name": "World"
        });

        let result = render_with_tera(template, &context).unwrap();
        assert_eq!(result, "<h1>Hello World</h1>");
    }
}
