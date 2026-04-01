//! # AMOS Education Package
//!
//! Extends the AMOS Harness with education-specific capabilities:
//!
//! - **SCORM runtime** — Ingest, launch, and track SCORM 1.2/2004 course packages
//! - **CE credit tracking** — Issue and verify continuing education certificates
//! - **Law knowledge base** — Index and search state statutes for officer reference
//! - **Personalized learning** — Adaptive learning paths, assessments, and knowledge gap tracking
//!
//! ## Usage
//!
//! Enable via environment variable:
//! ```bash
//! AMOS_PACKAGES=education
//! ```
//!
//! ## Tools (15 total)
//!
//! **SCORM**: ingest_scorm, launch_course, track_completion, get_transcript
//! **Certification**: issue_certificate, verify_certificate
//! **Enrollment**: enroll_learner, get_learner_progress
//! **Law Knowledge**: ingest_statutes, search_law, explain_statute
//! **Learning Coach**: analyze_learner, recommend_path, generate_assessment, update_knowledge_gap

pub mod scorm_parser;
pub mod tools;

use amos_core::{
    packages::{AmosPackage, PackageContext, PackageToolRegistry},
    Result,
};
use async_trait::async_trait;
use std::sync::Arc;

/// The education package — implements `AmosPackage` for harness loading.
pub struct EducationPackage;

impl EducationPackage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EducationPackage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AmosPackage for EducationPackage {
    fn name(&self) -> &str {
        "education"
    }

    fn description(&self) -> &str {
        "LMS, SCORM runtime, CE credit tracking, law knowledge base, and personalized learning for law enforcement"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn register_tools(&self, registry: &mut dyn PackageToolRegistry, ctx: &PackageContext) {
        let db = ctx.db_pool.clone();
        let pkg = self.name();

        // SCORM tools (4)
        registry.register_package_tool(
            Arc::new(tools::scorm::IngestScormTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::scorm::LaunchCourseTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::scorm::TrackCompletionTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::scorm::GetTranscriptTool::new(db.clone())),
            pkg,
        );

        // CE credit tools (2)
        registry.register_package_tool(
            Arc::new(tools::certification::IssueCertificateTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::certification::VerifyCertificateTool::new(db.clone())),
            pkg,
        );

        // Enrollment tools (2)
        registry.register_package_tool(
            Arc::new(tools::enrollment::EnrollLearnerTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::enrollment::GetLearnerProgressTool::new(db.clone())),
            pkg,
        );

        // Law knowledge tools (3)
        registry.register_package_tool(
            Arc::new(tools::law_knowledge::IngestStatutesTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::law_knowledge::SearchLawTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::law_knowledge::ExplainStatuteTool::new(db.clone())),
            pkg,
        );

        // Learning coach tools (4)
        registry.register_package_tool(
            Arc::new(tools::learning_coach::AnalyzeLearnerTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::learning_coach::RecommendPathTool::new(db.clone())),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::learning_coach::GenerateAssessmentTool::new(
                db.clone(),
            )),
            pkg,
        );
        registry.register_package_tool(
            Arc::new(tools::learning_coach::UpdateKnowledgeGapTool::new(db)),
            pkg,
        );

        tracing::info!("Registered 15 education tools");
    }

    async fn on_activate(&self, ctx: &PackageContext) -> Result<()> {
        bootstrap_schemas(&ctx.db_pool).await?;
        tracing::info!("Education package activated — schemas bootstrapped");
        Ok(())
    }
}

/// Public route function — called by harness packages.rs (feature-gated).
pub fn routes(state: std::sync::Arc<tools::scorm::ScormState>) -> axum::Router {
    tools::scorm::scorm_routes(state)
}

/// Bootstrap education schema collections (idempotent).
async fn bootstrap_schemas(db_pool: &sqlx::PgPool) -> Result<()> {
    let collections = vec![
        (
            "edu_courses",
            "Courses",
            "SCORM courses and learning modules",
        ),
        ("edu_learners", "Learners", "Officers and other learners"),
        (
            "edu_enrollments",
            "Enrollments",
            "Learner-course enrollment records",
        ),
        (
            "edu_completions",
            "Completions",
            "Course completion and score records",
        ),
        (
            "edu_certificates",
            "Certificates",
            "CE credit certificates issued",
        ),
        (
            "edu_scorm_packages",
            "SCORM Packages",
            "Uploaded SCORM package metadata and manifest data",
        ),
    ];

    for (name, display_name, description) in collections {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM collections WHERE name = $1)",
        )
        .bind(name)
        .fetch_one(db_pool)
        .await
        .unwrap_or(false);

        if !exists {
            tracing::info!("Creating education collection: {display_name}");
            sqlx::query(
                "INSERT INTO collections (id, name, display_name, description, fields, settings, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, '{}'::jsonb, NOW(), NOW())"
            )
            .bind(uuid::Uuid::new_v4())
            .bind(name)
            .bind(display_name)
            .bind(description)
            .bind(serde_json::json!([]))
            .execute(db_pool)
            .await
            .map_err(|e| {
                tracing::warn!("Failed to create collection {name}: {e}");
                e
            })
            .ok();
        }
    }

    Ok(())
}
