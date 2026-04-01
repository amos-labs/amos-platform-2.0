//! Learner enrollment and progress tracking tools.

use amos_core::{
    tools::{Tool, ToolCategory, ToolResult},
    Result,
};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;

/// Enroll a learner in a course.
pub struct EnrollLearnerTool {
    db_pool: PgPool,
}

impl EnrollLearnerTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for EnrollLearnerTool {
    fn name(&self) -> &str {
        "enroll_learner"
    }

    fn description(&self) -> &str {
        "Enroll a learner (officer) in a course. Creates an enrollment record and makes the course available to the learner."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "learner_id": {
                    "type": "string",
                    "description": "The learner to enroll"
                },
                "course_id": {
                    "type": "string",
                    "description": "The course to enroll in"
                },
                "due_date": {
                    "type": "string",
                    "description": "Optional due date for completion (ISO 8601)"
                },
                "assigned_by": {
                    "type": "string",
                    "description": "ID of the person/system assigning the course"
                },
                "department_id": {
                    "type": "string",
                    "description": "Department the learner belongs to"
                }
            },
            "required": ["learner_id", "course_id"]
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

        // Check for existing enrollment
        let existing = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1 FROM records r
                JOIN collections c ON r.collection_id = c.id
                WHERE c.name = 'edu_enrollments'
                  AND r.data->>'learner_id' = $1
                  AND r.data->>'course_id' = $2
                  AND r.data->>'status' != 'withdrawn'
             )",
        )
        .bind(learner_id)
        .bind(course_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        if existing {
            return Ok(ToolResult::error(format!(
                "Learner {learner_id} is already enrolled in course {course_id}"
            )));
        }

        let enrollment_id = uuid::Uuid::new_v4();

        let collection_id = sqlx::query_scalar::<_, uuid::Uuid>(
            "SELECT id FROM collections WHERE name = 'edu_enrollments'",
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?
        .ok_or_else(|| {
            amos_core::AmosError::Internal("edu_enrollments collection not found".into())
        })?;

        let data = json!({
            "learner_id": learner_id,
            "course_id": course_id,
            "status": "enrolled",
            "enrolled_at": chrono::Utc::now().to_rfc3339(),
            "due_date": params.get("due_date"),
            "assigned_by": params.get("assigned_by"),
            "department_id": params.get("department_id"),
            "progress_percent": 0,
        });

        sqlx::query(
            "INSERT INTO records (id, collection_id, data, created_at, updated_at)
             VALUES ($1, $2, $3, NOW(), NOW())",
        )
        .bind(enrollment_id)
        .bind(collection_id)
        .bind(&data)
        .execute(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        Ok(ToolResult::success(json!({
            "enrollment_id": enrollment_id,
            "learner_id": learner_id,
            "course_id": course_id,
            "status": "enrolled",
            "message": "Learner enrolled successfully"
        })))
    }
}

/// Get a learner's progress across all enrolled courses.
pub struct GetLearnerProgressTool {
    db_pool: PgPool,
}

impl GetLearnerProgressTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for GetLearnerProgressTool {
    fn name(&self) -> &str {
        "get_learner_progress"
    }

    fn description(&self) -> &str {
        "Get a learner's progress across all enrolled courses. Shows enrollment status, completion percentage, scores, and overdue courses."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "learner_id": {
                    "type": "string",
                    "description": "The learner to check progress for"
                },
                "status_filter": {
                    "type": "string",
                    "enum": ["enrolled", "in_progress", "completed", "overdue", "all"],
                    "description": "Filter by enrollment status (default: all)"
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

        let enrollments =
            sqlx::query_as::<_, (uuid::Uuid, JsonValue, chrono::DateTime<chrono::Utc>)>(
                "SELECT r.id, r.data, r.updated_at FROM records r
             JOIN collections c ON r.collection_id = c.id
             WHERE c.name = 'edu_enrollments' AND r.data->>'learner_id' = $1
             ORDER BY r.created_at DESC",
            )
            .bind(learner_id)
            .fetch_all(&self.db_pool)
            .await
            .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        let status_filter = params
            .get("status_filter")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let now = chrono::Utc::now();
        let mut results: Vec<JsonValue> = Vec::new();
        let mut overdue_count = 0;
        let mut completed_count = 0;

        for (id, data, updated_at) in &enrollments {
            let status = data
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            // Check if overdue
            let is_overdue = data
                .get("due_date")
                .and_then(|v| v.as_str())
                .and_then(|d| d.parse::<chrono::DateTime<chrono::Utc>>().ok())
                .is_some_and(|due| due < now && status != "completed");

            if is_overdue {
                overdue_count += 1;
            }
            if status == "completed" {
                completed_count += 1;
            }

            // Apply filter
            let include = match status_filter {
                "all" => true,
                "overdue" => is_overdue,
                filter => status == filter,
            };

            if include {
                results.push(json!({
                    "enrollment_id": id,
                    "course_id": data.get("course_id"),
                    "status": status,
                    "progress_percent": data.get("progress_percent").unwrap_or(&json!(0)),
                    "enrolled_at": data.get("enrolled_at"),
                    "due_date": data.get("due_date"),
                    "is_overdue": is_overdue,
                    "last_activity": updated_at.to_rfc3339(),
                }));
            }
        }

        Ok(ToolResult::success(json!({
            "learner_id": learner_id,
            "enrollments": results,
            "summary": {
                "total_enrollments": enrollments.len(),
                "completed": completed_count,
                "overdue": overdue_count,
                "active": enrollments.len() - completed_count,
            }
        })))
    }
}
