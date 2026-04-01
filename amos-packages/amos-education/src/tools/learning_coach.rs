//! Learning coach tools — analyze performance, identify gaps, build personalized paths.
//!
//! These tools enable an AI agent to act as a personalized learning coach,
//! analyzing each officer's performance data and generating targeted training plans.

use amos_core::{
    tools::{Tool, ToolCategory, ToolResult},
    Result,
};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;

/// Analyze a learner's performance and identify knowledge gaps.
pub struct AnalyzeLearnerTool {
    db_pool: PgPool,
}

impl AnalyzeLearnerTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for AnalyzeLearnerTool {
    fn name(&self) -> &str {
        "analyze_learner"
    }

    fn description(&self) -> &str {
        "Analyze a learner's complete performance history — completions, scores, time spent, knowledge gaps, and trends. Returns a comprehensive profile that the learning coach uses to personalize training."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "learner_id": {
                    "type": "string",
                    "description": "The learner to analyze"
                },
                "include_gaps": {
                    "type": "boolean",
                    "description": "Include knowledge gap analysis (default: true)"
                },
                "include_sessions": {
                    "type": "boolean",
                    "description": "Include recent session history (default: true)"
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
        let include_gaps = params
            .get("include_gaps")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let include_sessions = params
            .get("include_sessions")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Get completion stats
        let completions = sqlx::query_as::<_, (i64, Option<f64>, Option<f64>)>(
            "SELECT COUNT(*), AVG((data->>'score')::numeric), AVG((data->>'time_spent_seconds')::numeric)
             FROM records r JOIN collections c ON r.collection_id = c.id
             WHERE c.name = 'edu_completions' AND r.data->>'learner_id' = $1",
        )
        .bind(learner_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        let (total_completions, avg_score, avg_time) = completions;

        // Get enrollment stats
        let enrollments = sqlx::query_as::<_, (i64, i64)>(
            "SELECT
                COUNT(*),
                COUNT(*) FILTER (WHERE r.data->>'status' = 'enrolled' AND
                    (r.data->>'due_date')::timestamptz < NOW())
             FROM records r JOIN collections c ON r.collection_id = c.id
             WHERE c.name = 'edu_enrollments' AND r.data->>'learner_id' = $1",
        )
        .bind(learner_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        let (total_enrollments, overdue_count) = enrollments;

        // Get certificate count
        let cert_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM records r JOIN collections c ON r.collection_id = c.id
             WHERE c.name = 'edu_certificates' AND r.data->>'learner_id' = $1",
        )
        .bind(learner_id)
        .fetch_one(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        let mut result = json!({
            "learner_id": learner_id,
            "performance": {
                "total_completions": total_completions,
                "total_enrollments": total_enrollments,
                "overdue_courses": overdue_count,
                "certificates_earned": cert_count,
                "average_score": avg_score,
                "average_time_seconds": avg_time,
            }
        });

        // Knowledge gaps
        if include_gaps {
            let gaps = sqlx::query_as::<_, (String, f64, f64, String, Option<String>)>(
                "SELECT competency_area, confidence_score, proficiency_level, status, recommended_action
                 FROM edu_knowledge_gaps WHERE learner_id = $1::uuid
                 ORDER BY proficiency_level ASC",
            )
            .bind(learner_id)
            .fetch_all(&self.db_pool)
            .await
            .unwrap_or_default();

            let gaps_json: Vec<JsonValue> = gaps
                .into_iter()
                .map(|(area, confidence, proficiency, status, action)| {
                    json!({
                        "competency_area": area,
                        "confidence_score": confidence,
                        "proficiency_level": proficiency,
                        "status": status,
                        "recommended_action": action,
                    })
                })
                .collect();

            result["knowledge_gaps"] = json!(gaps_json);
            result["gap_count"] = json!(gaps_json.len());
        }

        // Recent sessions
        if include_sessions {
            let sessions = sqlx::query_as::<
                _,
                (
                    uuid::Uuid,
                    String,
                    String,
                    Option<i32>,
                    Option<f64>,
                    String,
                    chrono::DateTime<chrono::Utc>,
                ),
            >(
                "SELECT id, session_type, status, duration_seconds, score::float8,
                        course_id::text, started_at
                 FROM edu_learner_sessions WHERE learner_id = $1::uuid
                 ORDER BY started_at DESC LIMIT 20",
            )
            .bind(learner_id)
            .fetch_all(&self.db_pool)
            .await
            .unwrap_or_default();

            let sessions_json: Vec<JsonValue> = sessions
                .into_iter()
                .map(|(id, stype, status, duration, score, course, started)| {
                    json!({
                        "session_id": id,
                        "type": stype,
                        "status": status,
                        "duration_seconds": duration,
                        "score": score,
                        "course_id": course,
                        "started_at": started.to_rfc3339(),
                    })
                })
                .collect();

            result["recent_sessions"] = json!(sessions_json);
        }

        Ok(ToolResult::success(result))
    }
}

/// Generate a personalized learning path for a learner.
pub struct RecommendPathTool {
    db_pool: PgPool,
}

impl RecommendPathTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for RecommendPathTool {
    fn name(&self) -> &str {
        "recommend_path"
    }

    fn description(&self) -> &str {
        "Generate or update a personalized learning path for a learner based on their knowledge gaps, performance history, and required competencies. The path is an ordered sequence of courses and activities tailored to the individual officer."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "learner_id": {
                    "type": "string",
                    "description": "The learner to create a path for"
                },
                "title": {
                    "type": "string",
                    "description": "Title for the learning path"
                },
                "target_competencies": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Competency areas to address (e.g., ['use_of_force', 'de_escalation'])"
                },
                "steps": {
                    "type": "array",
                    "description": "Ordered learning steps",
                    "items": {
                        "type": "object",
                        "properties": {
                            "course_id": { "type": "string" },
                            "type": {
                                "type": "string",
                                "enum": ["scorm_course", "law_review", "assessment", "scenario", "reading"]
                            },
                            "title": { "type": "string" },
                            "reason": {
                                "type": "string",
                                "description": "Why this step is recommended"
                            },
                            "priority": {
                                "type": "string",
                                "enum": ["required", "recommended", "optional"]
                            },
                            "estimated_minutes": { "type": "integer" }
                        },
                        "required": ["type", "title", "reason"]
                    }
                },
                "due_date": {
                    "type": "string",
                    "description": "Target completion date (ISO 8601)"
                }
            },
            "required": ["learner_id", "title", "steps"]
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
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("title is required".into()))?;
        let steps = params
            .get("steps")
            .ok_or_else(|| amos_core::AmosError::Validation("steps are required".into()))?;
        let default_competencies = json!([]);
        let target_competencies = params
            .get("target_competencies")
            .unwrap_or(&default_competencies);
        let due_date = params.get("due_date").and_then(|v| v.as_str());

        let total_steps = steps.as_array().map(|a| a.len()).unwrap_or(0) as i32;

        let estimated_hours: f64 = steps
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| s.get("estimated_minutes").and_then(|v| v.as_f64()))
                    .sum::<f64>()
                    / 60.0
            })
            .unwrap_or(0.0);

        let path_id = uuid::Uuid::new_v4();

        sqlx::query(
            "INSERT INTO edu_learning_paths
             (id, learner_id, title, description, path_data, target_competencies,
              estimated_hours, total_steps, due_date)
             VALUES ($1, $2::uuid, $3, $4, $5, $6, $7, $8, $9::timestamptz)",
        )
        .bind(path_id)
        .bind(learner_id)
        .bind(title)
        .bind(params.get("description").and_then(|v| v.as_str()))
        .bind(steps)
        .bind(target_competencies)
        .bind(estimated_hours)
        .bind(total_steps)
        .bind(due_date)
        .execute(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        Ok(ToolResult::success(json!({
            "path_id": path_id,
            "learner_id": learner_id,
            "title": title,
            "total_steps": total_steps,
            "estimated_hours": estimated_hours,
            "target_competencies": target_competencies,
            "message": format!("Learning path created with {total_steps} steps ({estimated_hours:.1}h estimated)")
        })))
    }
}

/// Generate an adaptive assessment for a learner.
pub struct GenerateAssessmentTool {
    db_pool: PgPool,
}

impl GenerateAssessmentTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for GenerateAssessmentTool {
    fn name(&self) -> &str {
        "generate_assessment"
    }

    fn description(&self) -> &str {
        "Generate an adaptive assessment to evaluate a learner's knowledge in specific competency areas. The assessment difficulty adjusts based on the learner's known proficiency level. Returns questions that the agent can present conversationally."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "learner_id": {
                    "type": "string",
                    "description": "The learner being assessed"
                },
                "competency_areas": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Areas to assess (e.g., ['use_of_force', 'miranda_rights'])"
                },
                "question_count": {
                    "type": "integer",
                    "description": "Number of questions to generate (default: 5)"
                },
                "difficulty": {
                    "type": "string",
                    "enum": ["adaptive", "basic", "intermediate", "advanced"],
                    "description": "Difficulty level (default: adaptive — adjusts to learner)"
                }
            },
            "required": ["learner_id", "competency_areas"]
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
        let competency_areas = params
            .get("competency_areas")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                amos_core::AmosError::Validation("competency_areas is required".into())
            })?;
        let question_count = params
            .get("question_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(5) as i32;
        let difficulty = params
            .get("difficulty")
            .and_then(|v| v.as_str())
            .unwrap_or("adaptive");

        // Get learner's current proficiency in requested areas
        let area_strings: Vec<String> = competency_areas
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        let gaps = sqlx::query_as::<_, (String, f64)>(
            "SELECT competency_area, proficiency_level
             FROM edu_knowledge_gaps
             WHERE learner_id = $1::uuid AND competency_area = ANY($2)",
        )
        .bind(learner_id)
        .bind(&area_strings)
        .fetch_all(&self.db_pool)
        .await
        .unwrap_or_default();

        let proficiency_map: std::collections::HashMap<String, f64> = gaps.into_iter().collect();

        // Get relevant statutes for the competency areas to base questions on
        let statutes = sqlx::query_as::<_, (String, String, String, Option<String>)>(
            "SELECT statute_number, title, full_text, summary
             FROM edu_law_statutes
             WHERE category = ANY($1)
             ORDER BY RANDOM() LIMIT $2",
        )
        .bind(&area_strings)
        .bind(question_count * 2) // fetch more than needed for variety
        .fetch_all(&self.db_pool)
        .await
        .unwrap_or_default();

        // Build assessment context for the AI agent to use
        let assessment_id = uuid::Uuid::new_v4();

        let context: Vec<JsonValue> = competency_areas
            .iter()
            .filter_map(|v| v.as_str())
            .map(|area| {
                let proficiency = proficiency_map.get(area).copied().unwrap_or(0.5);
                let target_difficulty = if difficulty == "adaptive" {
                    match proficiency {
                        p if p < 0.3 => "basic",
                        p if p < 0.7 => "intermediate",
                        _ => "advanced",
                    }
                } else {
                    difficulty
                };
                json!({
                    "competency_area": area,
                    "current_proficiency": proficiency,
                    "target_difficulty": target_difficulty,
                })
            })
            .collect();

        let source_material: Vec<JsonValue> = statutes
            .into_iter()
            .map(|(number, title, text, summary)| {
                json!({
                    "statute_number": number,
                    "title": title,
                    "text": text,
                    "summary": summary,
                })
            })
            .collect();

        Ok(ToolResult::success(json!({
            "assessment_id": assessment_id,
            "learner_id": learner_id,
            "question_count": question_count,
            "competency_context": context,
            "source_material": source_material,
            "instructions": "Use the competency context and source material to generate scenario-based questions. Adjust difficulty based on the target_difficulty for each area. Present questions conversationally and evaluate responses to update the learner's proficiency scores.",
        })))
    }
}

/// Record a knowledge gap or update proficiency for a learner.
pub struct UpdateKnowledgeGapTool {
    db_pool: PgPool,
}

impl UpdateKnowledgeGapTool {
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait]
impl Tool for UpdateKnowledgeGapTool {
    fn name(&self) -> &str {
        "update_knowledge_gap"
    }

    fn description(&self) -> &str {
        "Record or update a knowledge gap for a learner. Used by the learning coach after assessments, course completions, or interactions to track what each officer knows and where they need improvement."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "learner_id": {
                    "type": "string",
                    "description": "The learner"
                },
                "competency_area": {
                    "type": "string",
                    "description": "The competency area (e.g., 'use_of_force', 'miranda_rights')"
                },
                "proficiency_level": {
                    "type": "number",
                    "description": "Estimated proficiency 0.0 to 1.0"
                },
                "confidence_score": {
                    "type": "number",
                    "description": "How confident the system is in this estimate (0.0 to 1.0)"
                },
                "evidence_type": {
                    "type": "string",
                    "description": "What generated this update (e.g., 'assessment', 'course_completion', 'interaction')"
                },
                "evidence_score": {
                    "type": "number",
                    "description": "The score from the evidence source"
                },
                "recommended_action": {
                    "type": "string",
                    "description": "What the learning coach recommends"
                },
                "status": {
                    "type": "string",
                    "enum": ["identified", "addressed", "resolved"],
                    "description": "Gap status"
                }
            },
            "required": ["learner_id", "competency_area", "proficiency_level"]
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
        let competency_area = params
            .get("competency_area")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                amos_core::AmosError::Validation("competency_area is required".into())
            })?;
        let proficiency = params
            .get("proficiency_level")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| {
                amos_core::AmosError::Validation("proficiency_level is required".into())
            })?;
        let confidence = params
            .get("confidence_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);
        let status = params
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("identified");
        let recommended_action = params.get("recommended_action").and_then(|v| v.as_str());

        // Build evidence entry
        let evidence_entry = json!({
            "type": params.get("evidence_type").and_then(|v| v.as_str()).unwrap_or("manual"),
            "score": params.get("evidence_score"),
            "date": chrono::Utc::now().to_rfc3339(),
        });

        let gap_id = uuid::Uuid::new_v4();

        sqlx::query(
            "INSERT INTO edu_knowledge_gaps
             (id, learner_id, competency_area, proficiency_level, confidence_score,
              evidence, status, recommended_action, last_assessed)
             VALUES ($1, $2::uuid, $3, $4, $5, jsonb_build_array($6), $7, $8, NOW())
             ON CONFLICT (learner_id, competency_area) DO UPDATE SET
                proficiency_level = $4,
                confidence_score = $5,
                evidence = edu_knowledge_gaps.evidence || $6,
                status = $7,
                recommended_action = COALESCE($8, edu_knowledge_gaps.recommended_action),
                last_assessed = NOW(),
                updated_at = NOW()",
        )
        .bind(gap_id)
        .bind(learner_id)
        .bind(competency_area)
        .bind(proficiency)
        .bind(confidence)
        .bind(&evidence_entry)
        .bind(status)
        .bind(recommended_action)
        .execute(&self.db_pool)
        .await
        .map_err(|e| amos_core::AmosError::Internal(e.to_string()))?;

        Ok(ToolResult::success(json!({
            "learner_id": learner_id,
            "competency_area": competency_area,
            "proficiency_level": proficiency,
            "confidence_score": confidence,
            "status": status,
            "message": format!("Knowledge gap updated: {competency_area} at {proficiency:.0}% proficiency")
        })))
    }
}
