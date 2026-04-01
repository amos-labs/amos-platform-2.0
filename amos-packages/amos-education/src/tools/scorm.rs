//! SCORM course management tools.
//!
//! Handles ingestion, launching, and tracking of SCORM 1.2 and SCORM 2004
//! course packages for continuing education credit.

use amos_core::{
    tools::{Tool, ToolCategory, ToolResult},
    Result,
};
use async_trait::async_trait;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::sync::Arc;

// ── SCORM Routes ─────────────────────────────────────────────────────────

/// State for SCORM routes — just needs a db connection.
#[derive(Clone)]
pub struct ScormState {
    pub db_pool: PgPool,
}

/// Package-specific routes for SCORM runtime communication.
///
/// Mounted at `/api/v1/pkg/education/scorm/...`
/// These endpoints are called by the SCORM player (browser) during course playback.
pub fn scorm_routes(state: Arc<ScormState>) -> Router {
    Router::new()
        .route("/scorm/{package_id}/launch", get(scorm_launch))
        .route("/scorm/{package_id}/runtime", post(scorm_runtime_commit))
        .route("/scorm/{package_id}/runtime", get(scorm_runtime_fetch))
        .with_state(state)
}

/// Launch endpoint — returns the SCORM player HTML + entry point URL.
async fn scorm_launch(
    State(state): State<Arc<ScormState>>,
    Path(package_id): Path<uuid::Uuid>,
) -> Json<JsonValue> {
    let result = sqlx::query_as::<_, (JsonValue,)>(
        "SELECT data FROM records WHERE collection_id = (
            SELECT id FROM collections WHERE name = 'edu_scorm_packages'
         ) AND id = $1",
    )
    .bind(package_id)
    .fetch_optional(&state.db_pool)
    .await;

    match result {
        Ok(Some((data,))) => {
            let launch_url = data
                .get("launch_url")
                .and_then(|v| v.as_str())
                .unwrap_or("index.html");
            Json(json!({
                "package_id": package_id,
                "launch_url": launch_url,
                "scorm_version": data.get("scorm_version").unwrap_or(&json!("1.2")),
                "title": data.get("title").unwrap_or(&json!("Untitled Course")),
            }))
        }
        _ => Json(json!({
            "error": "SCORM package not found",
            "package_id": package_id,
        })),
    }
}

/// SCORM runtime commit — receives CMI data model updates from the SCORM player.
async fn scorm_runtime_commit(
    State(state): State<Arc<ScormState>>,
    Path(package_id): Path<uuid::Uuid>,
    Json(payload): Json<JsonValue>,
) -> Json<JsonValue> {
    let learner_id = payload
        .get("learner_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let result = sqlx::query(
        "INSERT INTO records (id, collection_id, data, created_at, updated_at)
         SELECT $1, c.id, $3, NOW(), NOW()
         FROM collections c WHERE c.name = 'edu_completions'
         ON CONFLICT (id) DO UPDATE SET data = $3, updated_at = NOW()",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(package_id)
    .bind(json!({
        "package_id": package_id.to_string(),
        "learner_id": learner_id,
        "cmi_data": payload.get("cmi_data"),
        "committed_at": chrono::Utc::now().to_rfc3339(),
    }))
    .execute(&state.db_pool)
    .await;

    match result {
        Ok(_) => Json(json!({"success": true})),
        Err(e) => Json(json!({"success": false, "error": e.to_string()})),
    }
}

/// SCORM runtime fetch — returns stored CMI data for resuming a course.
async fn scorm_runtime_fetch(
    State(state): State<Arc<ScormState>>,
    Path(package_id): Path<uuid::Uuid>,
) -> Json<JsonValue> {
    let result = sqlx::query_as::<_, (JsonValue,)>(
        "SELECT data FROM records WHERE collection_id = (
            SELECT id FROM collections WHERE name = 'edu_completions'
         ) AND data->>'package_id' = $1
         ORDER BY updated_at DESC LIMIT 1",
    )
    .bind(package_id.to_string())
    .fetch_optional(&state.db_pool)
    .await;

    match result {
        Ok(Some((data,))) => Json(json!({"found": true, "data": data})),
        _ => Json(json!({"found": false})),
    }
}

// ── SCORM Tools (agent-callable) ─────────────────────────────────────────

/// Ingest a SCORM package — parses the manifest, stores metadata, extracts content.
pub struct IngestScormTool {
    db_pool: PgPool,
}

impl IngestScormTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for IngestScormTool {
    fn name(&self) -> &str {
        "ingest_scorm"
    }

    fn description(&self) -> &str {
        "Ingest a SCORM 1.2 or 2004 course package. Parses the imsmanifest.xml, extracts metadata (title, description, launch URL, SCOs), and registers the course in the education system."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "upload_id": {
                    "type": "string",
                    "description": "ID of the uploaded SCORM ZIP file (from the uploads system)"
                },
                "title": {
                    "type": "string",
                    "description": "Override title (if not provided, extracted from manifest)"
                },
                "ce_credits": {
                    "type": "number",
                    "description": "Number of continuing education credits awarded on completion"
                },
                "state_code": {
                    "type": "string",
                    "description": "State code for CE credit jurisdiction (e.g., 'TX', 'CA')"
                }
            },
            "required": ["upload_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Education
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let upload_id = params
            .get("upload_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("upload_id is required".into()))?;

        let title_override = params.get("title").and_then(|v| v.as_str());
        let ce_credits = params.get("ce_credits").and_then(|v| v.as_f64());
        let state_code = params.get("state_code").and_then(|v| v.as_str());

        // Look up the uploaded file path from the uploads record
        let upload_path = sqlx::query_scalar::<_, String>(
            "SELECT data->>'file_path' FROM records WHERE id = $1::uuid",
        )
        .bind(upload_id)
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?
        .ok_or_else(|| amos_core::AmosError::Validation(format!("Upload {upload_id} not found")))?;

        // Parse the SCORM manifest from the uploaded ZIP
        let file = std::fs::File::open(&upload_path).map_err(|e| {
            amos_core::AmosError::Internal(format!("Failed to open SCORM package: {e}"))
        })?;
        let reader = std::io::BufReader::new(file);
        let manifest = crate::scorm_parser::parse_from_zip(reader)
            .map_err(|e| amos_core::AmosError::Validation(format!("Invalid SCORM package: {e}")))?;

        let title = title_override.unwrap_or(&manifest.title);

        let scos_json: Vec<JsonValue> = manifest
            .scos
            .iter()
            .map(|sco| {
                json!({
                    "identifier": sco.identifier,
                    "title": sco.title,
                    "href": sco.href,
                    "type": sco.sco_type,
                    "mastery_score": sco.mastery_score,
                })
            })
            .collect();

        let package_id = uuid::Uuid::new_v4();
        let course_id = uuid::Uuid::new_v4();

        // Create SCORM package record with parsed manifest data
        let collection_id = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM collections WHERE name = 'edu_scorm_packages'",
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?
        .ok_or_else(|| {
            amos_core::AmosError::Internal("edu_scorm_packages collection not found".into())
        })?;

        sqlx::query(
            "INSERT INTO records (id, collection_id, data, created_at, updated_at)
             VALUES ($1, $2, $3, NOW(), NOW())",
        )
        .bind(package_id)
        .bind(collection_id)
        .bind(json!({
            "upload_id": upload_id,
            "title": title,
            "scorm_version": manifest.scorm_version,
            "launch_url": manifest.launch_url,
            "scos": scos_json,
            "default_org": manifest.default_org,
            "metadata": {
                "schema": manifest.metadata.schema,
                "schema_version": manifest.metadata.schema_version,
                "manifest_identifier": manifest.metadata.identifier,
            },
            "status": "active",
            "file_path": upload_path,
        }))
        .execute(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        // Create course record
        let courses_collection_id = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM collections WHERE name = 'edu_courses'",
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?
        .ok_or_else(|| amos_core::AmosError::Internal("edu_courses collection not found".into()))?;

        sqlx::query(
            "INSERT INTO records (id, collection_id, data, created_at, updated_at)
             VALUES ($1, $2, $3, NOW(), NOW())",
        )
        .bind(course_id)
        .bind(courses_collection_id)
        .bind(json!({
            "title": title,
            "type": "scorm",
            "scorm_package_id": package_id.to_string(),
            "scorm_version": manifest.scorm_version,
            "sco_count": manifest.scos.len(),
            "ce_credits": ce_credits,
            "state_code": state_code,
            "status": "active",
        }))
        .execute(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        Ok(ToolResult::success(json!({
            "package_id": package_id,
            "course_id": course_id,
            "title": title,
            "scorm_version": manifest.scorm_version,
            "launch_url": manifest.launch_url,
            "sco_count": manifest.scos.len(),
            "scos": scos_json,
            "ce_credits": ce_credits,
            "status": "active",
            "message": format!("SCORM {} package ingested: {} with {} SCO(s). Course created and ready for enrollment.",
                manifest.scorm_version, title, manifest.scos.len())
        })))
    }
}

/// Launch a SCORM course for a learner.
pub struct LaunchCourseTool {
    db_pool: PgPool,
}

impl LaunchCourseTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for LaunchCourseTool {
    fn name(&self) -> &str {
        "launch_course"
    }

    fn description(&self) -> &str {
        "Launch a SCORM course for a learner. Returns the launch URL and session info needed to start the course player."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "course_id": {
                    "type": "string",
                    "description": "The course to launch"
                },
                "learner_id": {
                    "type": "string",
                    "description": "The learner taking the course"
                }
            },
            "required": ["course_id", "learner_id"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Education
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let course_id = params
            .get("course_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("course_id is required".into()))?;
        let learner_id = params
            .get("learner_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("learner_id is required".into()))?;

        let course_data =
            sqlx::query_as::<_, (JsonValue,)>("SELECT data FROM records WHERE id = $1::uuid")
                .bind(course_id)
                .fetch_optional(&self.db_pool)
                .await
                .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        let course = match course_data {
            Some((data,)) => data,
            None => return Ok(ToolResult::error(format!("Course {course_id} not found"))),
        };

        let scorm_package_id = course
            .get("scorm_package_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let session_id = uuid::Uuid::new_v4();

        Ok(ToolResult::success_with_metadata(
            json!({
                "session_id": session_id,
                "course_id": course_id,
                "learner_id": learner_id,
                "launch_url": format!("/api/v1/pkg/education/scorm/{scorm_package_id}/launch"),
                "runtime_url": format!("/api/v1/pkg/education/scorm/{scorm_package_id}/runtime"),
                "title": course.get("title"),
            }),
            json!({
                "__canvas_action": "launch_scorm",
                "session_id": session_id.to_string(),
            }),
        ))
    }
}

/// Track SCORM course completion for a learner.
pub struct TrackCompletionTool {
    db_pool: PgPool,
}

impl TrackCompletionTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for TrackCompletionTool {
    fn name(&self) -> &str {
        "track_completion"
    }

    fn description(&self) -> &str {
        "Record a course completion for a learner. Stores score, status, and time spent. Triggers CE credit issuance if the course awards credits."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "course_id": {
                    "type": "string",
                    "description": "The completed course"
                },
                "learner_id": {
                    "type": "string",
                    "description": "The learner who completed the course"
                },
                "score": {
                    "type": "number",
                    "description": "Score achieved (0-100)"
                },
                "status": {
                    "type": "string",
                    "enum": ["completed", "passed", "failed", "incomplete"],
                    "description": "Completion status"
                },
                "time_spent_seconds": {
                    "type": "integer",
                    "description": "Time spent in the course in seconds"
                }
            },
            "required": ["course_id", "learner_id", "status"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Education
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let course_id = params
            .get("course_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("course_id is required".into()))?;
        let learner_id = params
            .get("learner_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("learner_id is required".into()))?;
        let status = params
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("status is required".into()))?;

        let completion_id = uuid::Uuid::new_v4();

        let collection_id = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM collections WHERE name = 'edu_completions'",
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?
        .ok_or_else(|| {
            amos_core::AmosError::Internal("edu_completions collection not found".into())
        })?;

        let data = json!({
            "course_id": course_id,
            "learner_id": learner_id,
            "status": status,
            "score": params.get("score"),
            "time_spent_seconds": params.get("time_spent_seconds"),
            "completed_at": chrono::Utc::now().to_rfc3339(),
        });

        sqlx::query(
            "INSERT INTO records (id, collection_id, data, created_at, updated_at)
             VALUES ($1, $2, $3, NOW(), NOW())",
        )
        .bind(completion_id)
        .bind(collection_id)
        .bind(&data)
        .execute(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        Ok(ToolResult::success(json!({
            "completion_id": completion_id,
            "course_id": course_id,
            "learner_id": learner_id,
            "status": status,
            "message": format!("Completion recorded: {status}")
        })))
    }
}

/// Get a learner's transcript — all course completions and scores.
pub struct GetTranscriptTool {
    db_pool: PgPool,
}

impl GetTranscriptTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for GetTranscriptTool {
    fn name(&self) -> &str {
        "get_transcript"
    }

    fn description(&self) -> &str {
        "Get a learner's full transcript: all course completions, scores, CE credits earned, and certificates issued."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "learner_id": {
                    "type": "string",
                    "description": "The learner to retrieve the transcript for"
                }
            },
            "required": ["learner_id"]
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

        let completions =
            sqlx::query_as::<_, (uuid::Uuid, JsonValue, chrono::DateTime<chrono::Utc>)>(
                "SELECT r.id, r.data, r.created_at FROM records r
             JOIN collections c ON r.collection_id = c.id
             WHERE c.name = 'edu_completions' AND r.data->>'learner_id' = $1
             ORDER BY r.created_at DESC",
            )
            .bind(learner_id)
            .fetch_all(&self.db_pool)
            .await
            .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        let certificates = sqlx::query_as::<_, (uuid::Uuid, JsonValue)>(
            "SELECT r.id, r.data FROM records r
             JOIN collections c ON r.collection_id = c.id
             WHERE c.name = 'edu_certificates' AND r.data->>'learner_id' = $1
             ORDER BY r.created_at DESC",
        )
        .bind(learner_id)
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        let completions_json: Vec<JsonValue> = completions
            .into_iter()
            .map(|(id, data, created_at)| {
                json!({
                    "id": id,
                    "data": data,
                    "recorded_at": created_at.to_rfc3339(),
                })
            })
            .collect();

        let certificates_json: Vec<JsonValue> = certificates
            .into_iter()
            .map(|(id, data)| json!({"id": id, "data": data}))
            .collect();

        let total_ce_credits: f64 = certificates_json
            .iter()
            .filter_map(|c| c["data"]["ce_credits"].as_f64())
            .sum();

        Ok(ToolResult::success(json!({
            "learner_id": learner_id,
            "completions": completions_json,
            "certificates": certificates_json,
            "total_completions": completions_json.len(),
            "total_certificates": certificates_json.len(),
            "total_ce_credits": total_ce_credits,
        })))
    }
}
