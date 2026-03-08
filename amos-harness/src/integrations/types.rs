use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AuthType {
    ApiKey,
    BearerToken,
    BasicAuth,
    Oauth2,
    SsoKey,
    NoAuth,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ConnectionStatus {
    Disconnected,
    Connected,
    Error,
    RateLimited,
    Suspended,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ConnectionHealth {
    Unknown,
    Healthy,
    Degraded,
    Failing,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum PaginationStrategy {
    Cursor,
    Page,
    Offset,
    Token,
    LinkHeader,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum SyncMode {
    Full,
    Incremental,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum SyncDirection {
    Inbound,
    Outbound,
    Bidirectional,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ConflictResolution {
    ExternalWins,
    InternalWins,
    Manual,
    Newest,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ScheduleType {
    Manual,
    Scheduled,
    Realtime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum SyncRecordStatus {
    Synced,
    Pending,
    Error,
    Deleted,
    Orphaned,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum LogStatus {
    Pending,
    Success,
    Failed,
    RateLimited,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct IntegrationRow {
    pub id: Uuid,
    pub name: String,
    pub connector_type: String,
    pub endpoint_url: Option<String>,
    pub status: String,
    pub credentials: JsonValue,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub sync_config: JsonValue,
    pub available_actions: JsonValue,
    pub metadata: JsonValue,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OperationRow {
    pub id: Uuid,
    pub integration_id: Uuid,
    pub operation_id: String,
    pub name: String,
    pub description: Option<String>,
    pub http_method: String,
    pub path_template: String,
    pub request_schema: JsonValue,
    pub response_schema: JsonValue,
    pub pagination_strategy: Option<String>,
    pub requires_confirmation: bool,
    pub is_destructive: bool,
    pub status: String,
    pub examples: JsonValue,
    pub metadata: JsonValue,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CredentialRow {
    pub id: Uuid,
    pub integration_id: Uuid,
    pub auth_type: String,
    pub credentials_data: JsonValue,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
    pub oauth_scopes: Option<String>,
    pub oauth_auth_url: Option<String>,
    pub oauth_token_url: Option<String>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
    pub auth_placement: Option<String>,
    pub auth_key: Option<String>,
    pub auth_value_template: Option<String>,
    pub status: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_rotated_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub label: Option<String>,
    pub metadata: JsonValue,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConnectionRow {
    pub id: Uuid,
    pub integration_id: Uuid,
    pub credential_id: Option<Uuid>,
    pub name: Option<String>,
    pub status: String,
    pub health: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub consecutive_errors: i32,
    pub rate_limit_tier: Option<String>,
    pub daily_write_budget: Option<i32>,
    pub daily_writes_used: Option<i32>,
    pub budget_reset_at: Option<DateTime<Utc>>,
    pub config: JsonValue,
    pub metadata: JsonValue,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LogRow {
    pub id: Uuid,
    pub connection_id: Option<Uuid>,
    pub integration_id: Uuid,
    pub operation_id: Option<String>,
    pub http_method: Option<String>,
    pub request_url: Option<String>,
    pub request_headers: JsonValue,
    pub request_body: Option<JsonValue>,
    pub http_status: Option<i32>,
    pub response_headers: JsonValue,
    pub response_body: Option<JsonValue>,
    pub duration_ms: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
    pub rate_limit_remaining: Option<i32>,
    pub rate_limit_reset_at: Option<DateTime<Utc>>,
    pub correlation_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SyncConfigRow {
    pub id: Uuid,
    pub connection_id: Uuid,
    pub resource_type: String,
    pub target_collection: String,
    pub sync_mode: String,
    pub sync_direction: String,
    pub field_mappings: JsonValue,
    pub conflict_resolution: Option<String>,
    pub schedule_type: String,
    pub schedule_cron: Option<String>,
    pub requires_approval: bool,
    pub approval_threshold: Option<i32>,
    pub fetch_operation_id: String,
    pub fetch_params: JsonValue,
    pub enabled: bool,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_run_status: Option<String>,
    pub last_run_stats: JsonValue,
    pub metadata: JsonValue,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SyncRecordRow {
    pub id: Uuid,
    pub sync_config_id: Uuid,
    pub connection_id: Uuid,
    pub external_type: String,
    pub external_id: String,
    pub internal_collection: Option<String>,
    pub internal_record_id: Option<Uuid>,
    pub data_hash: Option<String>,
    pub last_external_data: Option<JsonValue>,
    pub sync_status: String,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub last_synced_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SyncCursorRow {
    pub id: Uuid,
    pub sync_config_id: Uuid,
    pub cursor_type: String,
    pub cursor_value: Option<String>,
    pub cursor_field: Option<String>,
    pub records_synced: i64,
    pub is_complete: bool,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConnectionRequest {
    pub integration_id: Uuid,
    pub name: Option<String>,
    pub credentials: JsonValue,
    pub auth_type: String,
    pub config: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConnectionRequest {
    pub name: Option<String>,
    pub config: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteActionRequest {
    pub operation_id: String,
    pub params: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSyncConfigRequest {
    pub connection_id: Uuid,
    pub resource_type: String,
    pub target_collection: String,
    pub sync_mode: Option<String>,
    pub sync_direction: Option<String>,
    pub field_mappings: JsonValue,
    pub fetch_operation_id: String,
    pub fetch_params: Option<JsonValue>,
    pub schedule_type: Option<String>,
    pub schedule_cron: Option<String>,
    pub conflict_resolution: Option<String>,
    pub requires_approval: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerSyncRequest {
    pub force_full: Option<bool>,
}
