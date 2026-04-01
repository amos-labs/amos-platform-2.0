//! CE credit certificate issuance and verification tools.

use amos_core::{
    tools::{Tool, ToolCategory, ToolResult},
    Result,
};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;

/// Issue a CE credit certificate to a learner upon course completion.
pub struct IssueCertificateTool {
    db_pool: PgPool,
}

impl IssueCertificateTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for IssueCertificateTool {
    fn name(&self) -> &str {
        "issue_certificate"
    }

    fn description(&self) -> &str {
        "Issue a continuing education certificate to a learner. Records CE credits, jurisdiction, and generates a verifiable certificate number."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "learner_id": {
                    "type": "string",
                    "description": "The learner receiving the certificate"
                },
                "course_id": {
                    "type": "string",
                    "description": "The course that was completed"
                },
                "completion_id": {
                    "type": "string",
                    "description": "The completion record ID"
                },
                "ce_credits": {
                    "type": "number",
                    "description": "Number of CE credits to award"
                },
                "state_code": {
                    "type": "string",
                    "description": "State jurisdiction for the CE credits (e.g., 'TX')"
                },
                "credit_type": {
                    "type": "string",
                    "description": "Type of CE credit (e.g., 'general', 'use_of_force', 'legal_update')"
                }
            },
            "required": ["learner_id", "course_id", "ce_credits"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Education
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let learner_id = params
            .get("learner_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("learner_id is required".into()))?;
        let course_id = params
            .get("course_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("course_id is required".into()))?;
        let ce_credits = params
            .get("ce_credits")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| amos_core::AmosError::Validation("ce_credits is required".into()))?;

        let certificate_id = uuid::Uuid::new_v4();
        // Generate a human-readable certificate number: CE-{YEAR}-{SHORT_UUID}
        let cert_number = format!(
            "CE-{}-{}",
            chrono::Utc::now().format("%Y"),
            &certificate_id.to_string()[..8].to_uppercase()
        );

        let collection_id = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM collections WHERE name = 'edu_certificates'",
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?
        .ok_or_else(|| {
            amos_core::AmosError::Internal("edu_certificates collection not found".into())
        })?;

        let data = json!({
            "certificate_number": cert_number,
            "learner_id": learner_id,
            "course_id": course_id,
            "completion_id": params.get("completion_id"),
            "ce_credits": ce_credits,
            "state_code": params.get("state_code"),
            "credit_type": params.get("credit_type"),
            "issued_at": chrono::Utc::now().to_rfc3339(),
            "status": "active",
        });

        sqlx::query(
            "INSERT INTO records (id, collection_id, data, created_at, updated_at)
             VALUES ($1, $2, $3, NOW(), NOW())",
        )
        .bind(certificate_id)
        .bind(collection_id)
        .bind(&data)
        .execute(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        Ok(ToolResult::success(json!({
            "certificate_id": certificate_id,
            "certificate_number": cert_number,
            "learner_id": learner_id,
            "ce_credits": ce_credits,
            "message": format!("Certificate {cert_number} issued for {ce_credits} CE credits")
        })))
    }
}

/// Verify a CE certificate by its number.
pub struct VerifyCertificateTool {
    db_pool: PgPool,
}

impl VerifyCertificateTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for VerifyCertificateTool {
    fn name(&self) -> &str {
        "verify_certificate"
    }

    fn description(&self) -> &str {
        "Verify a continuing education certificate by its certificate number. Returns the certificate details if valid."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "certificate_number": {
                    "type": "string",
                    "description": "The certificate number to verify (e.g., 'CE-2026-A1B2C3D4')"
                }
            },
            "required": ["certificate_number"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Education
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let cert_number = params
            .get("certificate_number")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Validation("certificate_number is required".into())
            })?;

        let result = sqlx::query_as::<_, (uuid::Uuid, JsonValue)>(
            "SELECT r.id, r.data FROM records r
             JOIN collections c ON r.collection_id = c.id
             WHERE c.name = 'edu_certificates'
               AND r.data->>'certificate_number' = $1",
        )
        .bind(cert_number)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        match result {
            Some((id, data)) => Ok(ToolResult::success(json!({
                "valid": true,
                "certificate_id": id,
                "certificate_number": cert_number,
                "learner_id": data.get("learner_id"),
                "course_id": data.get("course_id"),
                "ce_credits": data.get("ce_credits"),
                "state_code": data.get("state_code"),
                "issued_at": data.get("issued_at"),
                "status": data.get("status"),
            }))),
            None => Ok(ToolResult::success(json!({
                "valid": false,
                "certificate_number": cert_number,
                "message": "Certificate not found or invalid"
            }))),
        }
    }
}
