//! ETL Pipeline for integration data synchronization
//!
//! Implements Extract → Transform → Load for pulling data from external APIs
//! into AMOS collections, with deduplication, change detection, and cursor tracking.

use crate::integrations::executor::{ApiExecutor, ExecutionError};
use crate::integrations::types::{ConnectionRow, SyncConfigRow, SyncCursorRow, SyncRecordRow};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::fmt;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// ETL Pipeline for syncing external API data into AMOS collections
pub struct EtlPipeline {
    executor: ApiExecutor,
    db_pool: PgPool,
}

impl EtlPipeline {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            executor: ApiExecutor::new(db_pool.clone()),
            db_pool,
        }
    }

    /// Main orchestrator for the ETL process
    pub async fn run(&self, sync_config_id: Uuid) -> Result<SyncResult, EtlError> {
        let start_time = std::time::Instant::now();
        let mut result = SyncResult {
            extracted: 0,
            transformed: 0,
            loaded: 0,
            errors: Vec::new(),
            status: "success".to_string(),
            duration_ms: 0,
        };

        // 1. Load sync_config from DB
        let sync_config = self.load_sync_config(sync_config_id).await?;
        info!(
            "Starting ETL for sync_config '{}' ({})",
            sync_config.resource_type, sync_config.id
        );

        // 2. Extract data from external API
        let extracted_records = match self.extract(&sync_config).await {
            Ok(records) => {
                result.extracted = records.len();
                info!("Extracted {} records", records.len());
                records
            }
            Err(e) => {
                error!("Extract failed: {}", e);
                result.errors.push(format!("Extract: {}", e));
                result.status = "failed".to_string();
                result.duration_ms = start_time.elapsed().as_millis() as u64;
                self.update_sync_config_status(&sync_config, &result)
                    .await?;
                return Err(e);
            }
        };

        // 3. Transform records according to field mappings
        let transformed_records = match self.transform(&extracted_records, &sync_config).await {
            Ok(records) => {
                result.transformed = records.len();
                info!("Transformed {} records", records.len());
                records
            }
            Err(e) => {
                error!("Transform failed: {}", e);
                result.errors.push(format!("Transform: {}", e));
                result.status = "failed".to_string();
                result.duration_ms = start_time.elapsed().as_millis() as u64;
                self.update_sync_config_status(&sync_config, &result)
                    .await?;
                return Err(e);
            }
        };

        // 4. Load transformed records into AMOS collections
        let _load_result = match self.load(&transformed_records, &sync_config).await {
            Ok(lr) => {
                result.loaded = lr.inserted + lr.updated;
                info!(
                    "Load complete: {} inserted, {} updated, {} skipped",
                    lr.inserted, lr.updated, lr.skipped
                );
                if !lr.errors.is_empty() {
                    result
                        .errors
                        .extend(lr.errors.iter().map(|e| format!("Load: {}", e)));
                    result.status = "partial".to_string();
                }
                lr
            }
            Err(e) => {
                error!("Load failed: {}", e);
                result.errors.push(format!("Load: {}", e));
                result.status = "failed".to_string();
                result.duration_ms = start_time.elapsed().as_millis() as u64;
                self.update_sync_config_status(&sync_config, &result)
                    .await?;
                return Err(e);
            }
        };

        result.duration_ms = start_time.elapsed().as_millis() as u64;

        // 5. Update sync_config with run status
        self.update_sync_config_status(&sync_config, &result)
            .await?;

        info!(
            "ETL completed in {}ms: status={}, extracted={}, transformed={}, loaded={}",
            result.duration_ms, result.status, result.extracted, result.transformed, result.loaded
        );

        Ok(result)
    }

    /// Extract data from external API
    async fn extract(&self, sync_config: &SyncConfigRow) -> Result<Vec<JsonValue>, EtlError> {
        debug!("Starting extract for sync_config {}", sync_config.id);

        // Load or create sync cursor
        let mut cursor = self.load_or_create_cursor(sync_config).await?;

        // Load connection to get connection details
        let _connection = self.load_connection(sync_config.connection_id).await?;

        let mut all_records = Vec::new();
        let mut has_more = true;
        let mut page_count = 0;
        const MAX_PAGES: usize = 100; // Safety limit

        while has_more && page_count < MAX_PAGES {
            page_count += 1;

            // Build fetch params with cursor if incremental
            let mut fetch_params = sync_config
                .fetch_params
                .as_object()
                .cloned()
                .unwrap_or_default();

            // Add cursor params for incremental sync
            if sync_config.sync_mode == "incremental" {
                if let Some(cursor_value) = &cursor.cursor_value {
                    fetch_params.insert(
                        "starting_after".to_string(),
                        JsonValue::String(cursor_value.clone()),
                    );
                }
            }

            // Execute API call
            let response = self
                .executor
                .execute(
                    sync_config.connection_id,
                    &sync_config.fetch_operation_id,
                    JsonValue::Object(fetch_params.clone()),
                )
                .await
                .map_err(|e| EtlError::ExtractError(format!("API execution failed: {}", e)))?;

            // Parse response and extract records
            let (records, next_cursor, more) = self.parse_response(&response.body)?;
            debug!(
                "Page {}: got {} records, has_more={}",
                page_count,
                records.len(),
                more
            );

            all_records.extend(records);

            // Update cursor and pagination state
            if let Some(nc) = next_cursor {
                cursor.cursor_value = Some(nc);
                has_more = more;
            } else {
                has_more = false;
            }

            // For full sync, only fetch once
            if sync_config.sync_mode == "full" {
                has_more = false;
            }
        }

        if page_count >= MAX_PAGES {
            warn!(
                "Reached max pages limit ({}), stopping pagination",
                MAX_PAGES
            );
        }

        // Update cursor in DB
        cursor.records_synced = all_records.len() as i64;
        cursor.is_complete = !has_more;
        self.update_cursor(&cursor).await?;

        Ok(all_records)
    }

    /// Parse API response and extract records with pagination info
    fn parse_response(
        &self,
        response: &JsonValue,
    ) -> Result<(Vec<JsonValue>, Option<String>, bool), EtlError> {
        let mut records = Vec::new();
        let mut next_cursor = None;
        let mut has_more = false;

        // Check if response has a 'data' array (like Stripe)
        if let Some(data) = response.get("data").and_then(|d| d.as_array()) {
            records.extend(data.iter().cloned());

            // Check for pagination metadata
            if let Some(hm) = response.get("has_more").and_then(|v| v.as_bool()) {
                has_more = hm;
            }

            // Extract next cursor (could be in various fields)
            if let Some(nc) = response.get("next_cursor") {
                next_cursor = nc.as_str().map(|s| s.to_string());
            } else if let Some(last) = data.last() {
                // Use last record's ID as cursor
                if let Some(id) = last.get("id") {
                    next_cursor = id.as_str().map(|s| s.to_string());
                }
            }
        } else if let Some(array) = response.as_array() {
            // Response is directly an array
            records.extend(array.iter().cloned());
        } else if response.is_object() {
            // Single object response
            records.push(response.clone());
        } else {
            return Err(EtlError::ExtractError(
                "Unexpected response format".to_string(),
            ));
        }

        Ok((records, next_cursor, has_more))
    }

    /// Transform records according to field mappings
    async fn transform(
        &self,
        records: &[JsonValue],
        sync_config: &SyncConfigRow,
    ) -> Result<Vec<JsonValue>, EtlError> {
        debug!("Starting transform for {} records", records.len());

        // Parse field mappings
        let mappings = sync_config
            .field_mappings
            .as_array()
            .ok_or_else(|| EtlError::TransformError("No field mappings defined".to_string()))?;

        let mut transformed_records = Vec::new();

        for (idx, record) in records.iter().enumerate() {
            let mut output = serde_json::Map::new();

            for mapping in mappings {
                let mapping_obj = mapping.as_object().ok_or_else(|| {
                    EtlError::TransformError(format!("Invalid mapping at index {}", idx))
                })?;

                let source = mapping_obj
                    .get("source")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        EtlError::TransformError("Missing 'source' in mapping".to_string())
                    })?;

                let target = mapping_obj
                    .get("target")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        EtlError::TransformError("Missing 'target' in mapping".to_string())
                    })?;

                let transform = mapping_obj.get("transform").and_then(|v| v.as_str());

                // Extract source field (supports dot notation)
                if let Some(mut value) = extract_nested_field(record, source) {
                    // Apply transformation if specified
                    if let Some(transform_type) = transform {
                        value = apply_transform(&value, transform_type);
                    }

                    output.insert(target.to_string(), value);
                } else {
                    debug!("Source field '{}' not found in record {}", source, idx);
                }
            }

            // Always include original ID if available
            if let Some(id) = record.get("id") {
                output.insert("external_id".to_string(), id.clone());
            }

            transformed_records.push(JsonValue::Object(output));
        }

        Ok(transformed_records)
    }

    /// Load transformed records into AMOS collections
    async fn load(
        &self,
        records: &[JsonValue],
        sync_config: &SyncConfigRow,
    ) -> Result<LoadResult, EtlError> {
        debug!("Starting load for {} records", records.len());

        let mut result = LoadResult {
            inserted: 0,
            updated: 0,
            skipped: 0,
            errors: Vec::new(),
        };

        // Look up target collection
        let collection = self.load_collection(&sync_config.target_collection).await?;

        for (idx, record) in records.iter().enumerate() {
            match self.upsert_record(record, sync_config, &collection).await {
                Ok(UpsertResult::Inserted) => result.inserted += 1,
                Ok(UpsertResult::Updated) => result.updated += 1,
                Ok(UpsertResult::Skipped) => result.skipped += 1,
                Err(e) => {
                    error!("Failed to upsert record {}: {}", idx, e);
                    result.errors.push(format!("Record {}: {}", idx, e));
                }
            }
        }

        Ok(result)
    }

    /// Upsert a single record into the collection
    async fn upsert_record(
        &self,
        record: &JsonValue,
        sync_config: &SyncConfigRow,
        collection: &CollectionRow,
    ) -> Result<UpsertResult, EtlError> {
        let record_hash = compute_hash(record);

        // Extract external ID from record
        let external_id = record
            .get("external_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EtlError::LoadError("Record missing 'external_id' field".to_string()))?;

        let external_type = sync_config.resource_type.clone();

        // Look up existing sync_record
        let existing_sync = sqlx::query_as::<_, SyncRecordRow>(
            r#"
            SELECT * FROM integration_sync_records
            WHERE connection_id = $1 AND external_type = $2 AND external_id = $3
            "#,
        )
        .bind(sync_config.connection_id)
        .bind(&external_type)
        .bind(external_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| EtlError::LoadError(format!("Failed to lookup sync_record: {}", e)))?;

        match existing_sync {
            None => {
                // Insert new record
                let record_id = Uuid::new_v4();
                let now = Utc::now();

                // Insert into dynamic_records
                sqlx::query(
                    r#"
                    INSERT INTO dynamic_records (id, collection_id, data, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5)
                    "#,
                )
                .bind(record_id)
                .bind(collection.id)
                .bind(record)
                .bind(now)
                .bind(now)
                .execute(&self.db_pool)
                .await
                .map_err(|e| EtlError::LoadError(format!("Failed to insert record: {}", e)))?;

                // Create sync_record
                sqlx::query(
                    r#"
                    INSERT INTO integration_sync_records
                    (id, sync_config_id, connection_id, external_type, external_id,
                     internal_collection, internal_record_id, data_hash, last_external_data,
                     sync_status, retry_count, last_synced_at, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                    "#,
                )
                .bind(Uuid::new_v4())
                .bind(sync_config.id)
                .bind(sync_config.connection_id)
                .bind(&external_type)
                .bind(external_id)
                .bind(&sync_config.target_collection)
                .bind(record_id)
                .bind(&record_hash)
                .bind(record)
                .bind("synced")
                .bind(0)
                .bind(now)
                .bind(now)
                .bind(now)
                .execute(&self.db_pool)
                .await
                .map_err(|e| EtlError::LoadError(format!("Failed to create sync_record: {}", e)))?;

                debug!(
                    "Inserted new record {} with external_id {}",
                    record_id, external_id
                );
                Ok(UpsertResult::Inserted)
            }
            Some(sync_record) => {
                if sync_record.data_hash.as_deref() == Some(&record_hash) {
                    // No changes, skip
                    debug!("Skipped unchanged record with external_id {}", external_id);
                    Ok(UpsertResult::Skipped)
                } else {
                    // Update existing record
                    let now = Utc::now();

                    if let Some(internal_record_id) = sync_record.internal_record_id {
                        sqlx::query(
                            r#"
                            UPDATE dynamic_records
                            SET data = $1, updated_at = $2
                            WHERE id = $3
                            "#,
                        )
                        .bind(record)
                        .bind(now)
                        .bind(internal_record_id)
                        .execute(&self.db_pool)
                        .await
                        .map_err(|e| {
                            EtlError::LoadError(format!("Failed to update record: {}", e))
                        })?;
                    }

                    // Update sync_record
                    sqlx::query(
                        r#"
                        UPDATE integration_sync_records
                        SET data_hash = $1, last_external_data = $2, last_synced_at = $3,
                            updated_at = $4, sync_status = $5
                        WHERE id = $6
                        "#,
                    )
                    .bind(&record_hash)
                    .bind(record)
                    .bind(now)
                    .bind(now)
                    .bind("synced")
                    .bind(sync_record.id)
                    .execute(&self.db_pool)
                    .await
                    .map_err(|e| {
                        EtlError::LoadError(format!("Failed to update sync_record: {}", e))
                    })?;

                    debug!(
                        "Updated record {:?} with external_id {}",
                        sync_record.internal_record_id, external_id
                    );
                    Ok(UpsertResult::Updated)
                }
            }
        }
    }

    /// Load sync_config from database
    async fn load_sync_config(&self, id: Uuid) -> Result<SyncConfigRow, EtlError> {
        sqlx::query_as::<_, SyncConfigRow>(
            r#"
            SELECT * FROM integration_sync_configs
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| EtlError::ConfigNotFound(format!("Database error: {}", e)))?
        .ok_or_else(|| EtlError::ConfigNotFound(format!("Sync config {} not found", id)))
    }

    /// Load connection from database
    async fn load_connection(&self, id: Uuid) -> Result<ConnectionRow, EtlError> {
        sqlx::query_as::<_, ConnectionRow>(
            r#"
            SELECT * FROM integration_connections
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| EtlError::ConnectionError(format!("Database error: {}", e)))?
        .ok_or_else(|| EtlError::ConnectionError(format!("Connection {} not found", id)))
    }

    /// Load collection from database by name
    async fn load_collection(&self, name: &str) -> Result<CollectionRow, EtlError> {
        sqlx::query_as::<_, CollectionRow>(
            r#"
            SELECT * FROM dynamic_collections
            WHERE slug = $1 OR name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| EtlError::LoadError(format!("Database error: {}", e)))?
        .ok_or_else(|| EtlError::LoadError(format!("Collection '{}' not found", name)))
    }

    /// Load or create sync cursor
    async fn load_or_create_cursor(
        &self,
        sync_config: &SyncConfigRow,
    ) -> Result<SyncCursorRow, EtlError> {
        if let Some(cursor) = sqlx::query_as::<_, SyncCursorRow>(
            r#"
            SELECT * FROM integration_sync_cursors
            WHERE sync_config_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(sync_config.id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| EtlError::ExtractError(format!("Failed to load cursor: {}", e)))?
        {
            Ok(cursor)
        } else {
            // Create new cursor
            let now = Utc::now();
            let cursor_id = Uuid::new_v4();

            sqlx::query(
                r#"
                INSERT INTO integration_sync_cursors
                (id, sync_config_id, cursor_type, cursor_value, cursor_field,
                 records_synced, is_complete, started_at, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(cursor_id)
            .bind(sync_config.id)
            .bind("timestamp")
            .bind(None::<String>)
            .bind(None::<String>)
            .bind(0i64)
            .bind(false)
            .bind(now)
            .bind(now)
            .bind(now)
            .execute(&self.db_pool)
            .await
            .map_err(|e| EtlError::ExtractError(format!("Failed to create cursor: {}", e)))?;

            Ok(SyncCursorRow {
                id: cursor_id,
                sync_config_id: sync_config.id,
                cursor_type: "timestamp".to_string(),
                cursor_value: None,
                cursor_field: None,
                records_synced: 0,
                is_complete: false,
                started_at: Some(now),
                completed_at: None,
                created_at: now,
                updated_at: now,
            })
        }
    }

    /// Update sync cursor
    async fn update_cursor(&self, cursor: &SyncCursorRow) -> Result<(), EtlError> {
        let completed_at = if cursor.is_complete {
            Some(Utc::now())
        } else {
            cursor.completed_at
        };

        sqlx::query(
            r#"
            UPDATE integration_sync_cursors
            SET cursor_value = $1, records_synced = $2, is_complete = $3,
                completed_at = $4, updated_at = $5
            WHERE id = $6
            "#,
        )
        .bind(&cursor.cursor_value)
        .bind(cursor.records_synced)
        .bind(cursor.is_complete)
        .bind(completed_at)
        .bind(Utc::now())
        .bind(cursor.id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| EtlError::ExtractError(format!("Failed to update cursor: {}", e)))?;

        Ok(())
    }

    /// Update sync_config with run status
    async fn update_sync_config_status(
        &self,
        sync_config: &SyncConfigRow,
        result: &SyncResult,
    ) -> Result<(), EtlError> {
        let stats = serde_json::json!({
            "extracted": result.extracted,
            "transformed": result.transformed,
            "loaded": result.loaded,
            "errors": result.errors,
            "duration_ms": result.duration_ms,
        });

        sqlx::query(
            r#"
            UPDATE integration_sync_configs
            SET last_run_at = $1, last_run_status = $2, last_run_stats = $3, updated_at = $4
            WHERE id = $5
            "#,
        )
        .bind(Utc::now())
        .bind(&result.status)
        .bind(stats)
        .bind(Utc::now())
        .bind(sync_config.id)
        .execute(&self.db_pool)
        .await
        .map_err(|e| EtlError::LoadError(format!("Failed to update sync_config status: {}", e)))?;

        Ok(())
    }
}

/// Result of a sync operation
#[derive(Debug, Clone, Serialize)]
pub struct SyncResult {
    pub extracted: usize,
    pub transformed: usize,
    pub loaded: usize,
    pub errors: Vec<String>,
    pub status: String,
    pub duration_ms: u64,
}

/// Result of the load phase
#[derive(Debug)]
pub struct LoadResult {
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

/// Result of upserting a single record
#[derive(Debug)]
enum UpsertResult {
    Inserted,
    Updated,
    Skipped,
}

/// ETL errors
#[derive(Debug)]
pub enum EtlError {
    ConfigNotFound(String),
    ConnectionError(String),
    ExtractError(String),
    TransformError(String),
    LoadError(String),
    ExecutionError(ExecutionError),
}

impl fmt::Display for EtlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EtlError::ConfigNotFound(msg) => write!(f, "Config not found: {}", msg),
            EtlError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            EtlError::ExtractError(msg) => write!(f, "Extract error: {}", msg),
            EtlError::TransformError(msg) => write!(f, "Transform error: {}", msg),
            EtlError::LoadError(msg) => write!(f, "Load error: {}", msg),
            EtlError::ExecutionError(e) => write!(f, "Execution error: {}", e),
        }
    }
}

impl std::error::Error for EtlError {}

impl From<ExecutionError> for EtlError {
    fn from(e: ExecutionError) -> Self {
        EtlError::ExecutionError(e)
    }
}

/// Compute SHA-256 hash of a JSON value
fn compute_hash(value: &JsonValue) -> String {
    let mut hasher = Sha256::new();
    let canonical = serde_json::to_string(value).unwrap_or_default();
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Extract nested field from JSON using dot notation
fn extract_nested_field(record: &JsonValue, path: &str) -> Option<JsonValue> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = record;

    for part in parts {
        current = current.get(part)?;
    }

    Some(current.clone())
}

/// Apply transformation to a JSON value
fn apply_transform(value: &JsonValue, transform: &str) -> JsonValue {
    match transform {
        "lowercase" => {
            if let Some(s) = value.as_str() {
                JsonValue::String(s.to_lowercase())
            } else {
                value.clone()
            }
        }
        "uppercase" => {
            if let Some(s) = value.as_str() {
                JsonValue::String(s.to_uppercase())
            } else {
                value.clone()
            }
        }
        "titlecase" => {
            if let Some(s) = value.as_str() {
                let mut result = String::new();
                let mut capitalize_next = true;
                for c in s.chars() {
                    if c.is_whitespace() {
                        result.push(c);
                        capitalize_next = true;
                    } else if capitalize_next {
                        result.push(c.to_uppercase().next().unwrap_or(c));
                        capitalize_next = false;
                    } else {
                        result.push(c.to_lowercase().next().unwrap_or(c));
                    }
                }
                JsonValue::String(result)
            } else {
                value.clone()
            }
        }
        "trim" => {
            if let Some(s) = value.as_str() {
                JsonValue::String(s.trim().to_string())
            } else {
                value.clone()
            }
        }
        "to_string" => JsonValue::String(value.to_string()),
        "to_number" => {
            if let Some(s) = value.as_str() {
                if let Ok(n) = s.parse::<f64>() {
                    JsonValue::Number(
                        serde_json::Number::from_f64(n).unwrap_or(serde_json::Number::from(0)),
                    )
                } else {
                    value.clone()
                }
            } else {
                value.clone()
            }
        }
        "to_boolean" => {
            if let Some(b) = value.as_bool() {
                JsonValue::Bool(b)
            } else if let Some(s) = value.as_str() {
                JsonValue::Bool(matches!(s.to_lowercase().as_str(), "true" | "1" | "yes"))
            } else if let Some(n) = value.as_i64() {
                JsonValue::Bool(n != 0)
            } else {
                value.clone()
            }
        }
        _ => value.clone(),
    }
}

/// Private collection row for dynamic_collections table
#[derive(Debug, sqlx::FromRow)]
struct CollectionRow {
    id: Uuid,
    name: String,
    slug: String,
    schema: Option<JsonValue>,
    metadata: Option<JsonValue>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
