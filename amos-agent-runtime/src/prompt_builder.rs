//! # System Prompt Builder
//!
//! Constructs the system prompt for the V3 agent loop, mirroring the
//! Rails `SystemPromptBuilder` structure.
//!
//! Sections:
//! 1. Identity & personality
//! 2. User context (org, role, permissions)
//! 3. Date/time awareness
//! 4. Available skills
//! 5. Platform data summary
//! 6. Canvas context (if visual output expected)
//! 7. Tool instructions
//! 8. Platform knowledge base
//! 9. Learned behaviors

use chrono::Utc;

/// User context injected into the prompt.
#[derive(Debug, Clone, Default)]
pub struct UserContext {
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub organization_name: Option<String>,
    pub role: Option<String>,
    pub timezone: Option<String>,
}

/// Platform summary for grounding.
#[derive(Debug, Clone, Default)]
pub struct PlatformSummary {
    pub total_contacts: u64,
    pub total_campaigns: u64,
    pub total_bounties: u64,
    pub active_workflows: u64,
    pub integrations_connected: Vec<String>,
}

/// Build the complete system prompt for the V3 agent loop.
pub fn build_system_prompt(
    user_ctx: &UserContext,
    platform_summary: &PlatformSummary,
    canvas_context: Option<&str>,
    learned_behaviors: &[String],
) -> String {
    let mut sections = Vec::new();

    // ── 1. Identity ─────────────────────────────────────────────────
    sections.push(format!(
        r#"You are AMOS, the Autonomous Management Operating System — an AI assistant
that helps humans build, manage, and grow their businesses. You are part of a
platform where contributors (humans and AI agents) earn real ownership via AMOS
tokens.

Your core values:
- Human-AI collaboration: you do the work, humans provide judgment
- Contributor ownership: everyone who contributes earns real stake
- Transparency: all decisions and economics are auditable
- Autonomy: you can act independently within your tools and permissions

You have access to 12 composable tools. Use them to accomplish tasks directly —
don't just describe what you would do, actually do it."#
    ));

    // ── 2. User Context ─────────────────────────────────────────────
    if let Some(name) = &user_ctx.user_name {
        let org = user_ctx
            .organization_name
            .as_deref()
            .unwrap_or("their organization");
        let role = user_ctx.role.as_deref().unwrap_or("user");
        sections.push(format!(
            "You are helping {name} ({role}) at {org}."
        ));
    }

    // ── 3. Date/Time ────────────────────────────────────────────────
    let now = Utc::now();
    let tz = user_ctx.timezone.as_deref().unwrap_or("UTC");
    sections.push(format!(
        "Current date and time: {} (user timezone: {tz})",
        now.format("%Y-%m-%d %H:%M:%S UTC")
    ));

    // ── 4. Platform Summary ─────────────────────────────────────────
    if platform_summary.total_contacts > 0 || !platform_summary.integrations_connected.is_empty() {
        let mut summary_lines = vec!["Platform overview:".to_string()];
        if platform_summary.total_contacts > 0 {
            summary_lines.push(format!(
                "- {} contacts, {} campaigns",
                platform_summary.total_contacts, platform_summary.total_campaigns
            ));
        }
        if platform_summary.total_bounties > 0 {
            summary_lines.push(format!(
                "- {} bounties, {} active workflows",
                platform_summary.total_bounties, platform_summary.active_workflows
            ));
        }
        if !platform_summary.integrations_connected.is_empty() {
            summary_lines.push(format!(
                "- Connected integrations: {}",
                platform_summary.integrations_connected.join(", ")
            ));
        }
        sections.push(summary_lines.join("\n"));
    }

    // ── 5. Canvas Context ───────────────────────────────────────────
    if let Some(canvas) = canvas_context {
        sections.push(format!(
            "The user currently has a canvas open with this content:\n{canvas}"
        ));
    }

    // ── 6. Tool Instructions ────────────────────────────────────────
    sections.push(
        r#"Tool usage guidelines:
- Use platform_query to read any data before making changes
- Use platform_create for new records, platform_update for modifications
- Use platform_execute for integrations, email sends, and publishing
- Use web_search + view_web_page for external research
- Use bash for system-level operations (sandboxed, 30s timeout)
- Use browser_use for interactive web automation
- Use load_canvas to display visual output to the user
- Use remember_this / search_memory for persistent knowledge
- Always verify before destructive operations
- Prefer showing results in canvas when visual output helps"#
            .to_string(),
    );

    // ── 7. Learned Behaviors ────────────────────────────────────────
    if !learned_behaviors.is_empty() {
        let behaviors = learned_behaviors
            .iter()
            .map(|b| format!("- {b}"))
            .collect::<Vec<_>>()
            .join("\n");
        sections.push(format!("Learned behaviors:\n{behaviors}"));
    }

    sections.join("\n\n")
}
