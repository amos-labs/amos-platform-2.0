//! # AMOS Social Media Package
//!
//! A complete social media intelligence layer for the AMOS Harness.
//!
//! - **Posting tools** — Twitter/X (tweet + thread), LinkedIn, Reddit, Hacker News
//! - **Campaign management** — Content calendar loading, scheduled posting, bounty integration
//! - **Analytics** — Per-post engagement metrics, aggregated campaign reports
//! - **System prompts** — Campaign strategy, platform-native content creation, engagement analysis
//!
//! ## Usage
//!
//! ```bash
//! AMOS_PACKAGES=social
//! ```
//!
//! ## Tools (9 total)
//!
//! **Posting**: post_tweet, post_thread, post_linkedin, post_reddit, post_hackernews
//! **Calendar**: load_content_calendar, schedule_content
//! **Analytics**: get_post_analytics, get_campaign_report
//!
//! ## The Meta-Narrative
//!
//! This package's first job is to announce AMOS to the world — through its own
//! bounty system. Content calendar items become bounties, agents claim and execute
//! them using these tools, and the first on-chain settlements are social media
//! posts telling the world the network exists.

pub mod tools;

use amos_core::{
    packages::{AmosPackage, PackageContext, PackageToolRegistry},
    Result,
};
use async_trait::async_trait;
use std::sync::Arc;

/// The social media package — implements `AmosPackage` for harness loading.
pub struct SocialPackage;

impl SocialPackage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SocialPackage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AmosPackage for SocialPackage {
    fn name(&self) -> &str {
        "social"
    }

    fn display_name(&self) -> &str {
        "Social Media Intelligence"
    }

    fn description(&self) -> &str {
        "Campaign strategy, content creation, multi-platform posting, \
         engagement analytics, and autonomous campaign orchestration"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn system_prompt(&self) -> Option<&str> {
        Some(include_str!("prompts/system.md"))
    }

    fn register_tools(&self, registry: &mut dyn PackageToolRegistry, ctx: &PackageContext) {
        let db = ctx.db_pool.clone();
        let pkg = self.name();

        // Posting tools (5)
        registry.register_package_tool(Arc::new(tools::twitter::PostTweetTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(tools::twitter::PostThreadTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(tools::linkedin::PostLinkedInTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(tools::reddit::PostRedditTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(tools::hackernews::PostHackerNewsTool::new(db.clone())), pkg);

        // Calendar & scheduling tools (2)
        registry.register_package_tool(Arc::new(tools::calendar::LoadContentCalendarTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(tools::calendar::ScheduleContentTool::new(db.clone())), pkg);

        // Analytics tools (2)
        registry.register_package_tool(Arc::new(tools::analytics::GetPostAnalyticsTool::new(db.clone())), pkg);
        registry.register_package_tool(Arc::new(tools::analytics::GetCampaignReportTool::new(db.clone())), pkg);

        tracing::info!("Registered 9 social media tools");
    }

    async fn on_activate(&self, ctx: &PackageContext) -> Result<()> {
        bootstrap_schemas(&ctx.db_pool).await?;
        tracing::info!("Social package activated — schemas bootstrapped");
        Ok(())
    }
}

/// Bootstrap social media schemas (idempotent).
async fn bootstrap_schemas(db_pool: &sqlx::PgPool) -> Result<()> {
    let schemas = [
        ("social_campaigns", "Campaign definitions and settings"),
        ("social_content", "Content items and drafts"),
        ("social_posts", "Published posts with platform IDs and URLs"),
        ("social_analytics", "Engagement metrics snapshots"),
        ("content_calendar", "Content calendar entries"),
        ("content_schedule", "Scheduled content items for posting"),
    ];

    for (collection, description) in schemas {
        let _ = sqlx::query(
            r#"INSERT INTO schema_collections (name, description, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW())
               ON CONFLICT (name) DO NOTHING"#,
        )
        .bind(collection)
        .bind(description)
        .execute(db_pool)
        .await;
    }

    Ok(())
}
