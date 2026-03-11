//! Universal API executor for integration operations
//!
//! Makes authenticated HTTP calls to external APIs based on operation definitions
//! and credential configurations stored in the database.

use crate::integrations::types::*;
use amos_core::CredentialVault;
use chrono::Utc;
use reqwest::{Client, Method, RequestBuilder};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Result of an API execution
#[derive(Debug, serde::Serialize)]
pub struct ExecutionResult {
    pub status_code: u16,
    pub body: JsonValue,
    pub headers: HashMap<String, String>,
    pub duration_ms: u64,
    pub operation_id: String,
}

/// Errors that can occur during API execution
#[derive(Debug)]
pub enum ExecutionError {
    NotFound(String),
    AuthError(String),
    ApiError { status: u16, body: String },
    NetworkError(String),
    ConfigError(String),
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionError::NotFound(msg) => write!(f, "Not found: {}", msg),
            ExecutionError::AuthError(msg) => write!(f, "Authentication error: {}", msg),
            ExecutionError::ApiError { status, body } => {
                write!(f, "API error (status {}): {}", status, body)
            }
            ExecutionError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ExecutionError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for ExecutionError {}

/// Universal API executor for making authenticated HTTP calls to external services
pub struct ApiExecutor {
    client: Client,
    db_pool: PgPool,
    /// Optional credential vault for resolving encrypted secrets at runtime.
    /// When a credential's `credentials_data` contains a `vault_credential_id`,
    /// the executor decrypts the secret from the vault instead of reading plaintext.
    vault: Option<Arc<CredentialVault>>,
}

impl ApiExecutor {
    /// Create a new API executor with default HTTP client (30s timeout)
    pub fn new(db_pool: PgPool) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            db_pool,
            vault: None,
        }
    }

    /// Create a new API executor with vault support for encrypted credential resolution
    pub fn with_vault(db_pool: PgPool, vault: Arc<CredentialVault>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            db_pool,
            vault: Some(vault),
        }
    }

    /// Execute an API operation with the given connection and parameters
    pub async fn execute(
        &self,
        connection_id: Uuid,
        operation_id: &str,
        params: JsonValue,
    ) -> Result<ExecutionResult, ExecutionError> {
        let start_time = Instant::now();
        let correlation_id = Uuid::new_v4().to_string();

        debug!(
            "Executing operation {} for connection {} (correlation_id: {})",
            operation_id, connection_id, correlation_id
        );

        // Load connection from database
        let connection = sqlx::query_as::<_, ConnectionRow>(
            "SELECT * FROM integration_connections WHERE id = $1",
        )
        .bind(connection_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| ExecutionError::ConfigError(format!("Database error: {}", e)))?
        .ok_or_else(|| {
            ExecutionError::NotFound(format!("Connection not found: {}", connection_id))
        })?;

        // Load integration from database
        let integration =
            sqlx::query_as::<_, IntegrationRow>("SELECT * FROM integrations WHERE id = $1")
                .bind(connection.integration_id)
                .fetch_optional(&self.db_pool)
                .await
                .map_err(|e| ExecutionError::ConfigError(format!("Database error: {}", e)))?
                .ok_or_else(|| {
                    ExecutionError::NotFound(format!(
                        "Integration not found: {}",
                        connection.integration_id
                    ))
                })?;

        // Load credential from database if credential_id is set
        let mut credential = if let Some(credential_id) = connection.credential_id {
            sqlx::query_as::<_, CredentialRow>(
                "SELECT * FROM integration_credentials WHERE id = $1",
            )
            .bind(credential_id)
            .fetch_optional(&self.db_pool)
            .await
            .map_err(|e| ExecutionError::ConfigError(format!("Database error: {}", e)))?
        } else {
            None
        };

        // Resolve vault credential references: if credentials_data contains
        // a `vault_credential_id`, decrypt the secret from the encrypted vault
        // and inject it into credentials_data so add_auth_headers works unchanged.
        if let Some(ref mut cred) = credential {
            if let Some(vault_id_str) = cred
                .credentials_data
                .get("vault_credential_id")
                .and_then(|v| v.as_str())
            {
                if let Some(ref vault) = self.vault {
                    let vault_id = Uuid::parse_str(vault_id_str).map_err(|_| {
                        ExecutionError::ConfigError("Invalid vault_credential_id UUID".to_string())
                    })?;

                    // Decrypt the secret from the vault
                    let decrypted = crate::routes::credentials::decrypt_credential(
                        &self.db_pool,
                        vault.as_ref(),
                        vault_id,
                    )
                    .await
                    .map_err(|status| {
                        ExecutionError::AuthError(format!(
                            "Failed to decrypt vault credential {}: HTTP {}",
                            vault_id,
                            status.as_u16()
                        ))
                    })?;

                    // Determine the key name based on auth_type (clone to String
                    // so we release the immutable borrow before as_object_mut).
                    let key_name: String = cred
                        .credentials_data
                        .get("vault_key_name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| match cred.auth_type.as_str() {
                            "api_key" => "api_key".to_string(),
                            "bearer_token" | "oauth2" => "access_token".to_string(),
                            "basic_auth" => "password".to_string(),
                            _ => "api_key".to_string(),
                        });

                    // Inject the decrypted value into credentials_data
                    if let Some(obj) = cred.credentials_data.as_object_mut() {
                        obj.insert(key_name, JsonValue::String(decrypted));
                        // Remove the vault reference so it doesn't leak into logs
                        obj.remove("vault_credential_id");
                        obj.remove("vault_key_name");
                    }

                    debug!(
                        "Resolved vault credential {} for auth_type={}",
                        vault_id, cred.auth_type
                    );
                } else {
                    warn!("Credential references vault_credential_id but vault is not configured");
                    return Err(ExecutionError::ConfigError(
                        "Credential vault not configured".to_string(),
                    ));
                }
            }
        }

        // Load operation from database
        let operation = sqlx::query_as::<_, OperationRow>(
            "SELECT * FROM integration_operations WHERE integration_id = $1 AND operation_id = $2",
        )
        .bind(connection.integration_id)
        .bind(operation_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| ExecutionError::ConfigError(format!("Database error: {}", e)))?
        .ok_or_else(|| {
            ExecutionError::NotFound(format!(
                "Operation not found: {} for integration {}",
                operation_id, connection.integration_id
            ))
        })?;

        // Build the request URL by combining base URL and path template
        let base_url = integration.endpoint_url.as_deref().ok_or_else(|| {
            ExecutionError::ConfigError(format!(
                "Integration {} has no endpoint_url configured",
                integration.name
            ))
        })?;

        let path = substitute_path_params(&operation.path_template, &params);
        let url = format!("{}{}", base_url, path);

        debug!("Request URL: {}", url);

        // Parse HTTP method
        let method = match operation.http_method.to_uppercase().as_str() {
            "GET" => Method::GET,
            "POST" => Method::POST,
            "PUT" => Method::PUT,
            "PATCH" => Method::PATCH,
            "DELETE" => Method::DELETE,
            _ => {
                return Err(ExecutionError::ConfigError(format!(
                    "Unsupported HTTP method: {}",
                    operation.http_method
                )))
            }
        };

        // Build the request
        let mut request = self.client.request(method.clone(), &url);

        // Build authentication headers
        request = self.add_auth_headers(request, credential.as_ref())?;

        // Add query params or body based on HTTP method
        let request_body = if method == Method::GET || method == Method::DELETE {
            // For GET/DELETE, add params as query string
            if let Some(params_obj) = params.as_object() {
                for (key, value) in params_obj {
                    let value_str = match value {
                        JsonValue::String(s) => s.clone(),
                        JsonValue::Number(n) => n.to_string(),
                        JsonValue::Bool(b) => b.to_string(),
                        _ => value.to_string(),
                    };
                    request = request.query(&[(key, value_str)]);
                }
            }
            None
        } else {
            // For POST/PUT/PATCH, add params as JSON body
            request = request.json(&params);
            Some(params.clone())
        };

        // Capture request headers for logging
        let request_headers = request
            .try_clone()
            .and_then(|r| r.build().ok())
            .map(|req| {
                req.headers()
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.to_string(),
                            serde_json::Value::String(v.to_str().unwrap_or("").to_string()),
                        )
                    })
                    .collect::<serde_json::Map<String, serde_json::Value>>()
            })
            .unwrap_or_default();

        // Execute the request
        debug!("Sending {} request to {}", method, url);
        let response_result = request.send().await;

        let (status_code, response_headers, response_text, duration_ms) = match response_result {
            Ok(response) => {
                let status_code = response.status().as_u16();
                let response_headers: HashMap<String, String> = response
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();

                // Read response body
                let response_text = response.text().await.map_err(|e| {
                    ExecutionError::NetworkError(format!("Failed to read response: {}", e))
                })?;

                let duration_ms = start_time.elapsed().as_millis() as u64;

                debug!(
                    "Response received: status={}, duration={}ms",
                    status_code, duration_ms
                );

                (status_code, response_headers, response_text, duration_ms)
            }
            Err(e) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                let error_message = format!("Request failed: {}", e);

                // Log the failed request
                self.log_api_call(
                    connection_id,
                    integration.id,
                    Some(operation_id),
                    method.as_str(),
                    &url,
                    JsonValue::Object(request_headers),
                    request_body,
                    None,
                    JsonValue::Object(Default::default()),
                    None,
                    duration_ms,
                    "failed",
                    Some(&error_message),
                    None,
                    None,
                    &correlation_id,
                )
                .await;

                return Err(ExecutionError::NetworkError(error_message));
            }
        };

        // Parse response body as JSON, fallback to string if not valid JSON
        let body: JsonValue = serde_json::from_str(&response_text)
            .unwrap_or_else(|_| JsonValue::String(response_text.clone()));

        // Determine log status
        let log_status = if (200..300).contains(&status_code) {
            "success"
        } else if status_code == 429 {
            "rate_limited"
        } else {
            "failed"
        };

        let error_message = if status_code >= 400 {
            Some(response_text.clone())
        } else {
            None
        };

        // Extract rate limit info from headers
        let rate_limit_remaining = response_headers
            .get("x-ratelimit-remaining")
            .or_else(|| response_headers.get("x-rate-limit-remaining"))
            .and_then(|v| v.parse::<i32>().ok());

        let rate_limit_reset_at = response_headers
            .get("x-ratelimit-reset")
            .or_else(|| response_headers.get("x-rate-limit-reset"))
            .and_then(|v| v.parse::<i64>().ok())
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0));

        // Convert response headers to JSON for logging
        let response_headers_json: serde_json::Map<String, serde_json::Value> = response_headers
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();

        // Log the API call
        self.log_api_call(
            connection_id,
            integration.id,
            Some(operation_id),
            method.as_str(),
            &url,
            JsonValue::Object(request_headers),
            request_body,
            Some(status_code as i32),
            JsonValue::Object(response_headers_json),
            Some(body.clone()),
            duration_ms,
            log_status,
            error_message.as_deref(),
            rate_limit_remaining,
            rate_limit_reset_at,
            &correlation_id,
        )
        .await;

        // Update connection last_used_at timestamp
        sqlx::query("UPDATE integration_connections SET last_used_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(connection_id)
            .execute(&self.db_pool)
            .await
            .map_err(|e| {
                warn!("Failed to update connection last_used_at: {}", e);
                e
            })
            .ok();

        // Check if the API call was successful
        if status_code >= 400 {
            return Err(ExecutionError::ApiError {
                status: status_code,
                body: response_text,
            });
        }

        Ok(ExecutionResult {
            status_code,
            body,
            headers: response_headers,
            duration_ms,
            operation_id: operation_id.to_string(),
        })
    }

    /// Test a connection by executing its test_connection operation
    pub async fn test_connection(
        &self,
        connection_id: Uuid,
    ) -> Result<ExecutionResult, ExecutionError> {
        info!("Testing connection {}", connection_id);

        // Execute the test_connection operation with empty params
        self.execute(
            connection_id,
            "test_connection",
            JsonValue::Object(Default::default()),
        )
        .await
    }

    /// Add authentication headers to the request based on credential configuration
    fn add_auth_headers(
        &self,
        mut request: RequestBuilder,
        credential: Option<&CredentialRow>,
    ) -> Result<RequestBuilder, ExecutionError> {
        let credential = match credential {
            Some(c) => c,
            None => {
                debug!("No credential configured, proceeding without authentication");
                return Ok(request);
            }
        };

        let auth_type = &credential.auth_type;
        let credentials_data = &credential.credentials_data;

        match auth_type.as_str() {
            "bearer_token" | "oauth2" => {
                // For OAuth2, prefer the access_token column, fall back to credentials_data
                let access_token = credential
                    .access_token
                    .as_ref()
                    .cloned()
                    .or_else(|| {
                        credentials_data
                            .get("access_token")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .ok_or_else(|| {
                        ExecutionError::AuthError("Missing access_token in credentials".to_string())
                    })?;

                request = request.bearer_auth(access_token);
                debug!("Added Bearer token authentication");
            }
            "api_key" => {
                // Get the header name from auth_key field or default to X-API-Key
                let header_name = credential.auth_key.as_deref().unwrap_or("X-API-Key");

                let api_key = credentials_data
                    .get("api_key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ExecutionError::AuthError("Missing api_key in credentials".to_string())
                    })?;

                request = request.header(header_name, api_key);
                debug!("Added API key authentication with header {}", header_name);
            }
            "basic_auth" => {
                // Extract username and password
                let username = credentials_data
                    .get("username")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ExecutionError::AuthError("Missing username in credentials".to_string())
                    })?;

                let password = credentials_data
                    .get("password")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ExecutionError::AuthError("Missing password in credentials".to_string())
                    })?;

                request = request.basic_auth(username, Some(password));
                debug!("Added Basic authentication");
            }
            "sso_key" => {
                // Use auth_value_template from credential to build the header value
                let template = credential.auth_value_template.as_ref().ok_or_else(|| {
                    ExecutionError::ConfigError(
                        "Missing auth_value_template for sso_key".to_string(),
                    )
                })?;

                let header_value = substitute_auth_template(template, credentials_data)?;
                let header_name = credential.auth_key.as_deref().unwrap_or("Authorization");

                request = request.header(header_name, header_value);
                debug!("Added SSO key authentication");
            }
            "no_auth" => {
                debug!("No authentication required");
            }
            "custom" | _ => {
                // If auth_value_template is set, use it
                if let Some(template) = &credential.auth_value_template {
                    let header_value = substitute_auth_template(template, credentials_data)?;
                    let header_name = credential.auth_key.as_deref().unwrap_or("Authorization");

                    request = request.header(header_name, header_value);
                    debug!("Added custom authentication from template");
                } else {
                    warn!("Unknown auth type: {}, no authentication added", auth_type);
                }
            }
        }

        Ok(request)
    }

    /// Log an API call to the integration_logs table
    #[allow(clippy::too_many_arguments)]
    async fn log_api_call(
        &self,
        connection_id: Uuid,
        integration_id: Uuid,
        operation_id: Option<&str>,
        http_method: &str,
        request_url: &str,
        request_headers: JsonValue,
        request_body: Option<JsonValue>,
        http_status: Option<i32>,
        response_headers: JsonValue,
        response_body: Option<JsonValue>,
        duration_ms: u64,
        status: &str,
        error_message: Option<&str>,
        rate_limit_remaining: Option<i32>,
        rate_limit_reset_at: Option<chrono::DateTime<Utc>>,
        correlation_id: &str,
    ) {
        let result = sqlx::query(
            r#"
            INSERT INTO integration_logs
                (connection_id, integration_id, operation_id, http_method, request_url,
                 request_headers, request_body, http_status, response_headers, response_body,
                 duration_ms, status, error_message, rate_limit_remaining, rate_limit_reset_at,
                 correlation_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            "#,
        )
        .bind(connection_id)
        .bind(integration_id)
        .bind(operation_id)
        .bind(http_method)
        .bind(request_url)
        .bind(&request_headers)
        .bind(&request_body)
        .bind(http_status)
        .bind(&response_headers)
        .bind(&response_body)
        .bind(duration_ms as i32)
        .bind(status)
        .bind(error_message)
        .bind(rate_limit_remaining)
        .bind(rate_limit_reset_at)
        .bind(correlation_id)
        .execute(&self.db_pool)
        .await;

        if let Err(e) = result {
            warn!("Failed to log API call: {}", e);
        }
    }
}

/// Substitute path parameters in a URL template
///
/// Replaces `{key}` placeholders with values from the params JSON object.
/// Only replaces parameters that exist in the JSON.
fn substitute_path_params(template: &str, params: &JsonValue) -> String {
    let mut result = template.to_string();

    if let Some(params_obj) = params.as_object() {
        for (key, value) in params_obj {
            let placeholder = format!("{{{}}}", key);
            if result.contains(&placeholder) {
                let value_str = match value {
                    JsonValue::String(s) => s.clone(),
                    JsonValue::Number(n) => n.to_string(),
                    JsonValue::Bool(b) => b.to_string(),
                    _ => value.to_string(),
                };
                result = result.replace(&placeholder, &value_str);
            }
        }
    }

    result
}

/// Substitute placeholders in an auth template with credential values
///
/// Replaces `{key}` placeholders with values from the credentials_data JSON object.
fn substitute_auth_template(
    template: &str,
    credentials_data: &JsonValue,
) -> Result<String, ExecutionError> {
    let mut result = template.to_string();

    if let Some(creds_obj) = credentials_data.as_object() {
        for (key, value) in creds_obj {
            let placeholder = format!("{{{}}}", key);
            if result.contains(&placeholder) {
                let value_str = match value {
                    JsonValue::String(s) => s.clone(),
                    JsonValue::Number(n) => n.to_string(),
                    JsonValue::Bool(b) => b.to_string(),
                    _ => {
                        return Err(ExecutionError::AuthError(format!(
                            "Invalid credential value type for key: {}",
                            key
                        )))
                    }
                };
                result = result.replace(&placeholder, &value_str);
            }
        }
    }

    // Check if there are any unreplaced placeholders
    if result.contains('{') && result.contains('}') {
        warn!(
            "Auth template still contains placeholders after substitution: {}",
            result
        );
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_substitute_path_params() {
        let template = "/customers/{customer_id}/invoices/{invoice_id}";
        let params = json!({
            "customer_id": "cus_123",
            "invoice_id": "inv_456"
        });

        let result = substitute_path_params(template, &params);
        assert_eq!(result, "/customers/cus_123/invoices/inv_456");
    }

    #[test]
    fn test_substitute_path_params_partial() {
        let template = "/customers/{customer_id}/invoices";
        let params = json!({
            "customer_id": "cus_123",
            "unused_param": "value"
        });

        let result = substitute_path_params(template, &params);
        assert_eq!(result, "/customers/cus_123/invoices");
    }

    #[test]
    fn test_substitute_path_params_number() {
        let template = "/items/{item_id}";
        let params = json!({
            "item_id": 12345
        });

        let result = substitute_path_params(template, &params);
        assert_eq!(result, "/items/12345");
    }

    #[test]
    fn test_substitute_auth_template() {
        let template = "Bearer {access_token}";
        let credentials = json!({
            "access_token": "tok_abc123"
        });

        let result = substitute_auth_template(template, &credentials).unwrap();
        assert_eq!(result, "Bearer tok_abc123");
    }

    #[test]
    fn test_substitute_auth_template_api_key() {
        let template = "{api_key}";
        let credentials = json!({
            "api_key": "sk_live_123456"
        });

        let result = substitute_auth_template(template, &credentials).unwrap();
        assert_eq!(result, "sk_live_123456");
    }
}
