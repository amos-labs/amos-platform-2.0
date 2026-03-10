//! Credential collection tool for AI agents.
//!
//! When the agent needs to collect a sensitive secret from the user (e.g., a
//! Stripe API key), it calls `collect_credential`. This does NOT ask for the
//! secret in chat. Instead, it opens a Secure Input Canvas in the UI where the
//! user can safely enter the value. The canvas POSTs the secret directly to
//! `/api/v1/credentials`, and the agent receives only the opaque credential_id.

use crate::tools::{Tool, ToolCategory, ToolResult};
use amos_core::Result;
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;

// ═══════════════════════════════════════════════════════════════════════════
// Collect Credential Tool
// ═══════════════════════════════════════════════════════════════════════════

/// Tool that triggers the Secure Input Canvas for collecting credentials.
/// The tool result contains instructions for the frontend to open the canvas,
/// along with metadata about what credential is being collected.
pub struct CollectCredentialTool {
    db_pool: PgPool,
}

impl CollectCredentialTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for CollectCredentialTool {
    fn name(&self) -> &str {
        "collect_credential"
    }

    fn description(&self) -> &str {
        "Securely collect a sensitive credential (API key, secret, token, password) from the user. \
         This opens a Secure Input Canvas where the user enters the value privately. \
         The secret is encrypted and stored in the credential vault. You will receive \
         a credential_id that can be used to reference the stored credential in \
         integration connections and API calls. NEVER ask the user to type secrets \
         directly in the chat - always use this tool instead."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "The service this credential is for (e.g., 'stripe', 'github', 'sendgrid', 'openai')"
                },
                "label": {
                    "type": "string",
                    "description": "Human-readable label for this credential (e.g., 'Stripe Secret Key', 'GitHub Personal Access Token')"
                },
                "credential_type": {
                    "type": "string",
                    "description": "Type of credential being collected",
                    "enum": ["api_key", "secret_key", "access_token", "password", "oauth_token", "webhook_secret"],
                    "default": "api_key"
                },
                "instructions": {
                    "type": "string",
                    "description": "Instructions to show the user about where to find this credential (e.g., 'Go to Stripe Dashboard > Developers > API Keys > Secret key')"
                },
                "placeholder": {
                    "type": "string",
                    "description": "Placeholder text for the input field (e.g., 'sk_live_...')"
                }
            },
            "required": ["service", "label"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let service = params["service"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("service is required".to_string()))?
            .to_string();

        let label = params["label"]
            .as_str()
            .ok_or_else(|| amos_core::AmosError::Validation("label is required".to_string()))?
            .to_string();

        let credential_type = params
            .get("credential_type")
            .and_then(|v| v.as_str())
            .unwrap_or("api_key")
            .to_string();

        let instructions = params
            .get("instructions")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let placeholder = params
            .get("placeholder")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Check if we already have an active credential for this service
        let existing: Option<(uuid::Uuid,)> = sqlx::query_as(
            "SELECT id FROM credential_vault WHERE service = $1 AND status = 'active' LIMIT 1",
        )
        .bind(&service)
        .fetch_optional(&self.db_pool)
        .await
        .ok()
        .flatten();

        if let Some((existing_id,)) = existing {
            return Ok(ToolResult::success(json!({
                "status": "already_exists",
                "credential_id": existing_id,
                "service": service,
                "message": format!("An active credential already exists for {}. Credential ID: {}", service, existing_id)
            })));
        }

        // Return a special tool result that the frontend knows to handle.
        // The `__canvas_action` field signals the frontend to open the
        // Secure Input Canvas instead of just displaying text.
        Ok(ToolResult::success_with_metadata(
            json!({
                "status": "awaiting_input",
                "message": format!(
                    "I need your {} for {}. A secure input form will appear where you can safely enter it. \
                     Your credential will be encrypted and stored securely - it will never appear in this chat.",
                    credential_type.replace('_', " "), service
                ),
                "service": service,
                "label": label,
                "credential_type": credential_type,
            }),
            json!({
                "__canvas_action": "secure_input",
                "service": service,
                "label": label,
                "credential_type": credential_type,
                "instructions": instructions,
                "placeholder": placeholder,
            }),
        ))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// List Vault Credentials Tool
// ═══════════════════════════════════════════════════════════════════════════

/// Tool to list stored credentials (metadata only - never exposes secrets).
pub struct ListVaultCredentialsTool {
    db_pool: PgPool,
}

impl ListVaultCredentialsTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct VaultCredentialSummary {
    id: uuid::Uuid,
    label: String,
    service: String,
    credential_type: String,
    status: String,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[async_trait]
impl Tool for ListVaultCredentialsTool {
    fn name(&self) -> &str {
        "list_vault_credentials"
    }

    fn description(&self) -> &str {
        "List all credentials stored in the secure vault (metadata only, secrets are never shown). \
         Use this to check which services already have stored credentials before asking the user \
         to provide a new one."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "description": "Optional: filter by service name (e.g., 'stripe')"
                },
                "status": {
                    "type": "string",
                    "description": "Optional: filter by status (default: 'active')",
                    "enum": ["active", "revoked", "expired"],
                    "default": "active"
                }
            },
            "required": []
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let status = params
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("active");

        let service_filter = params.get("service").and_then(|v| v.as_str());

        let credentials: Vec<VaultCredentialSummary> = if let Some(svc) = service_filter {
            sqlx::query_as(
                r#"SELECT id, label, service, credential_type, status, created_at, last_used_at
                   FROM credential_vault
                   WHERE status = $1 AND service = $2
                   ORDER BY created_at DESC"#,
            )
            .bind(status)
            .bind(svc)
            .fetch_all(&self.db_pool)
            .await?
        } else {
            sqlx::query_as(
                r#"SELECT id, label, service, credential_type, status, created_at, last_used_at
                   FROM credential_vault
                   WHERE status = $1
                   ORDER BY created_at DESC"#,
            )
            .bind(status)
            .fetch_all(&self.db_pool)
            .await?
        };

        let result: Vec<JsonValue> = credentials
            .iter()
            .map(|c| {
                json!({
                    "credential_id": c.id,
                    "label": c.label,
                    "service": c.service,
                    "credential_type": c.credential_type,
                    "status": c.status,
                    "created_at": c.created_at,
                    "last_used_at": c.last_used_at,
                })
            })
            .collect();

        let count = result.len();
        Ok(ToolResult::success(json!({
            "credentials": result,
            "count": count
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Integration
    }
}
