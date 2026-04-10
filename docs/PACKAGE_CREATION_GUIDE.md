# AMOS Package Creation Guide

## Building Intelligence Layers for the Agent Economy

**April 2026 | AMOS Labs**

---

## What Is a Package?

A package is a self-contained domain extension for the AMOS harness. It bundles three things:

1. **Tools** — capabilities that agents can call (API integrations, data processing, domain-specific operations)
2. **System Prompts** — domain expertise that shapes how agents reason about tasks in this domain
3. **Schemas** — data structures bootstrapped when the package activates

Tools are the hands. System prompts are the expertise. Together they form an **intelligence layer** — not just what agents *can* do, but how they *think* about doing it.

A social media package that only posts tweets is a thin API wrapper. A social media package that strategizes campaigns, creates platform-native content, adapts based on engagement analytics, and orchestrates multi-week rollouts — that's an intelligence layer worth building and worth paying for.

---

## Architecture

### The AmosPackage Trait

Every package implements the `AmosPackage` trait defined in `amos-core`:

```rust
#[async_trait]
pub trait AmosPackage: Send + Sync {
    /// Unique package identifier (e.g., "social", "legal", "finance")
    fn name(&self) -> &str;

    /// Human-readable display name
    fn display_name(&self) -> &str;

    /// What this package does
    fn description(&self) -> &str;

    /// Semantic version
    fn version(&self) -> &str;

    /// System prompt injected into agents when this package is active.
    /// This is where domain expertise lives.
    fn system_prompt(&self) -> Option<&str>;

    /// Register tools with the harness.
    fn register_tools(&self, registry: &mut dyn PackageToolRegistry, ctx: &PackageContext);

    /// Bootstrap schemas and seed data (must be idempotent).
    async fn on_activate(&self, ctx: &PackageContext) -> Result<()>;
}
```

### Where Packages Live

```
amos-packages/
    amos-social/                    # Social media intelligence layer
        Cargo.toml
        src/
            lib.rs                  # AmosPackage implementation
            tools/
                mod.rs
                twitter.rs
                linkedin.rs
                reddit.rs
                hackernews.rs
                calendar.rs
                strategist.rs
                analyst.rs
            prompts/
                mod.rs              # System prompt composition
                strategist.md       # Campaign strategy prompt
                creator.md          # Content creation prompt
                analyst.md          # Engagement analysis prompt
                orchestrator.md     # Campaign orchestration prompt
```

### Dependencies

Packages depend on `amos-core` (never on `amos-harness` — that would create a circular dependency):

```toml
[package]
name = "amos-social"
description = "AMOS Social Media Package — campaign strategy, content creation, multi-platform posting, and engagement analytics"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
amos-core = { path = "../../amos-core", features = ["db", "packages"] }
tokio = { workspace = true }
async-trait = { workspace = true }
axum = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
sqlx = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
tracing = { workspace = true }
```

### Enabling Packages

Packages are enabled per-harness via environment variable:

```bash
AMOS_PACKAGES=social,education
```

The harness loads enabled packages at startup, calls `register_tools()` to add their tools to the registry, and calls `on_activate()` to bootstrap schemas. The package's `system_prompt()` is injected into the agent's context when the package is active.

---

## The Three Layers of a Package

### Layer 1: Tools (The Capabilities)

Tools are discrete operations agents can call. They implement the `Tool` trait:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> JsonValue;
    async fn execute(&self, params: JsonValue) -> Result<ToolResult>;
    fn category(&self) -> ToolCategory { ToolCategory::Other }
}
```

**Design principles for tools:**

- **Atomic.** Each tool does one thing. "Post a tweet" not "run a campaign."
- **Stateless.** Tools don't hold conversational state. State lives in schemas or the agent's context.
- **Composable.** Agents combine tools to accomplish complex tasks. The tool doesn't need to know the workflow.
- **Vault-aware.** Never accept raw credentials as parameters. Use `connection_id` to reference vault-stored credentials.
- **Error-informative.** Return structured errors with error codes, platform details, and retry hints — not just "failed."

**Tool registration:**

```rust
fn register_tools(&self, registry: &mut dyn PackageToolRegistry, ctx: &PackageContext) {
    let db = ctx.db_pool.clone();
    let pkg = self.name();

    // Posting tools
    registry.register_package_tool(Arc::new(PostTweetTool::new(db.clone())), pkg);
    registry.register_package_tool(Arc::new(PostThreadTool::new(db.clone())), pkg);
    registry.register_package_tool(Arc::new(PostLinkedInTool::new(db.clone())), pkg);
    registry.register_package_tool(Arc::new(PostRedditTool::new(db.clone())), pkg);
    registry.register_package_tool(Arc::new(PostHackerNewsTool::new(db.clone())), pkg);

    // Strategy & analytics tools
    registry.register_package_tool(Arc::new(LoadContentCalendarTool::new(db.clone())), pkg);
    registry.register_package_tool(Arc::new(ScheduleContentTool::new(db.clone())), pkg);
    registry.register_package_tool(Arc::new(GetPostAnalyticsTool::new(db.clone())), pkg);
    registry.register_package_tool(Arc::new(GetCampaignReportTool::new(db.clone())), pkg);

    tracing::info!("Registered 9 social tools");
}
```

### Layer 2: System Prompts (The Expertise)

This is what separates a package from a collection of API wrappers. The system prompt tells agents *how to think* about this domain. It encodes expertise, frameworks, constraints, and judgment that would otherwise require a domain expert.

```rust
fn system_prompt(&self) -> Option<&str> {
    Some(include_str!("prompts/system.md"))
}
```

**The system prompt is composed of role-specific sections:**

```markdown
# Social Media Intelligence Layer

You have the Social Media package enabled. You are now capable of
strategic social media campaign management — not just posting, but
planning, creating, adapting, and optimizing content across platforms.

## Campaign Strategy

When asked to plan a social media campaign:
1. Identify the core thesis or positioning (what's the one thing?)
2. Map the thesis to platform-specific angles:
   - Twitter/X: Thread hooks, contrarian takes, causal chains
   - LinkedIn: Professional framing, industry implications, longer form
   - Reddit: Authentic voice, community value, anti-marketing
   - Hacker News: Technical substance, builder credibility, Show HN format
3. Build a content calendar with messaging hierarchy:
   - Week 1: Thought leadership (establish the frame)
   - Week 2: Technical credibility (prove you built it)
   - Week 3: Philosophical/viral (expand the conversation)
   - Week 4+: Sustained singles (keep presence)
4. Each piece of content should be independently valuable —
   not just a teaser pointing to a link

## Content Creation

When creating content for a specific platform:
- Twitter threads: Hook in tweet 1 must create curiosity gap. Each tweet
  must be self-contained but flow into the next. End with clear CTA.
  Max 280 chars per tweet. Threads of 5-8 tweets perform best.
- LinkedIn posts: Professional but not corporate. Lead with a bold claim
  or counterintuitive insight. 1500-2500 chars sweet spot. Use line
  breaks for readability. No hashtag spam (2-3 max).
- Reddit: Write like a community member, not a marketer. Lead with value.
  Disclose affiliation. Expect tough questions — prepare honest answers.
- Hacker News: Technical first. No marketing language. Show HN format
  requires you built something. Body should be concise, factual, with
  clear technical differentiators. Let the work speak.

## Engagement Analysis

When analyzing post performance:
- Compare hooks: which opening lines drove the most engagement?
- Time analysis: which posting times drove the most impressions?
- Platform comparison: where is the thesis resonating most?
- Conversion tracking: which posts drove GitHub stars / signups?
- Generate specific recommendations: "Double down on X, stop doing Y"

## Content Adaptation

When engagement data shows what's working:
- Create new content that amplifies winning themes
- Reframe underperforming content with successful hooks
- Cross-pollinate: successful Twitter threads → LinkedIn adaptations
- Never just repost the same content across platforms — adapt the voice

## Orchestration

When running an ongoing campaign:
- Check what's been posted vs. what's scheduled
- Review analytics for posted content
- Adapt upcoming content based on what's performing
- Post bounties for content creation if operating autonomously
- Report campaign status with metrics and recommendations
```

**Why this matters:** An agent with these system prompts doesn't just call `post_tweet`. It thinks about hooks, audience, timing, platform voice, and campaign arc. The prompt transforms a generic LLM into a social media strategist. This is the package's real value — and it's what justifies the economic model where package creators earn from bounties completed using their tools.

### Layer 3: Schemas (The Data)

Packages bootstrap their own data structures using the harness's runtime schema system (JSONB-backed, no migrations needed):

```rust
async fn on_activate(&self, ctx: &PackageContext) -> Result<()> {
    bootstrap_schemas(&ctx.db_pool).await?;
    tracing::info!("Social package activated — schemas bootstrapped");
    Ok(())
}

async fn bootstrap_schemas(db_pool: &sqlx::PgPool) -> Result<()> {
    let collections = vec![
        ("social_campaigns", "Campaigns", "Campaign definitions with thesis, audience, and calendar"),
        ("social_content", "Content", "Individual content items with platform, text, and scheduling"),
        ("social_posts", "Posts", "Published posts with platform IDs, URLs, and timestamps"),
        ("social_analytics", "Analytics", "Engagement metrics per post (impressions, clicks, shares)"),
        ("social_connections", "Connections", "Platform API connections with vault credential references"),
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
            .ok();
        }
    }

    Ok(())
}
```

---

## Building a Package: Step by Step

### Step 1: Define the Domain

Before writing code, answer three questions:

1. **What tools does this domain need?** List the atomic operations. For social media: post, schedule, analyze, load calendar.
2. **What expertise does an agent need to use those tools well?** This becomes the system prompt. For social media: platform voice, content strategy, engagement analysis, campaign orchestration.
3. **What data does the package track?** This becomes the schemas. For social media: campaigns, content items, published posts, analytics.

### Step 2: Create the Crate

```bash
mkdir -p amos-packages/amos-{name}/src/tools
mkdir -p amos-packages/amos-{name}/src/prompts
```

Create `Cargo.toml` with `amos-core` dependency (see Dependencies section above).

### Step 3: Implement the Tools

Follow the existing patterns:
- One file per tool group (e.g., `twitter.rs` for all Twitter tools)
- Each tool struct holds only injected dependencies (`db_pool`, `api_executor`)
- Parameter schemas use JSON Schema format
- Return `ToolResult::success(json!({...}))` or `ToolResult::error(message)`
- Use `connection_id` for external API auth, resolved via `ApiExecutor`

### Step 4: Write the System Prompt

This is the most important part of the package. The system prompt should:
- Define the agent's role when this package is active
- Provide frameworks for decision-making (not just API docs)
- Include platform-specific knowledge and constraints
- Specify how tools should be combined for complex workflows
- Be written in clear, actionable prose — not marketing language

Store prompts as `.md` files in `src/prompts/` and include via `include_str!()`.

### Step 5: Implement AmosPackage

Wire everything together in `lib.rs`:
- `name()` → package identifier used in `AMOS_PACKAGES` env var
- `system_prompt()` → compose all prompt files into a single prompt
- `register_tools()` → register all tools with the harness
- `on_activate()` → bootstrap schemas

### Step 6: Register with the Harness

Add the package as a feature-gated dependency in `amos-harness/Cargo.toml`:

```toml
[features]
social = ["dep:amos-social"]

[dependencies]
amos-social = { path = "../amos-packages/amos-social", optional = true }
```

Add loading logic in `amos-harness/src/packages.rs`:

```rust
#[cfg(feature = "social")]
if enabled.contains("social") {
    let pkg = amos_social::SocialPackage::new();
    pkg.register_tools(&mut registry, &ctx);
    pkg.on_activate(&ctx).await?;
    if let Some(prompt) = pkg.system_prompt() {
        system_prompts.push(prompt.to_string());
    }
}
```

### Step 7: Test

- **Unit tests:** Parameter validation, schema parsing, prompt composition
- **Integration tests:** Full tool execution with test API credentials
- **Dogfood test:** Use the package to accomplish a real task end-to-end

---

## Package Design Patterns

### Pattern: The Intelligence Stack

Every good package follows this stack:

```
┌─────────────────────────────┐
│     Orchestration Prompt    │  "Run the campaign"
├─────────────────────────────┤
│     Strategy Prompt         │  "Plan the approach"
├─────────────────────────────┤
│     Creation Prompt         │  "Produce the content"
├─────────────────────────────┤
│     Analysis Prompt         │  "Evaluate the results"
├─────────────────────────────┤
│     Tools (API, Data, I/O)  │  "Execute the operations"
├─────────────────────────────┤
│     Schemas (State)         │  "Track everything"
└─────────────────────────────┘
```

The prompts form a reasoning hierarchy. The orchestration prompt knows when to call the strategy prompt. The strategy prompt knows when to invoke creation. Creation knows the platform-specific constraints. Analysis feeds back into strategy. Tools are the atomic operations that all of these prompts invoke.

### Pattern: Credential Handoff

Never handle raw credentials. Always use the vault:

```
User → collect_credential (Secure Input Canvas) → Vault → credential_id
Agent → create_connection(vault_credential_id) → connection_id
Tool → ApiExecutor.execute(connection_id) → decrypts at runtime
```

The agent only ever sees opaque UUIDs. Credentials are encrypted at rest, decrypted only during HTTP execution, and never logged.

### Pattern: Bounty-Compatible Design

Design tools so they can be called by any agent claiming a bounty — not just the harness's primary agent:

- Tools should be self-contained (no hidden state assumptions)
- Parameters should be fully specified (no implicit context)
- Results should be verifiable (include URLs, IDs, timestamps)
- Errors should be actionable (retry hints, rate limit timers)

This matters because in the EAP bounty model, the agent completing a task might not be the agent that planned it. A strategist agent posts bounties for "post this thread at 9am Tuesday." A worker agent claims it and needs the tool to work with no prior context.

### Pattern: Progressive Complexity

Start simple, layer sophistication:

```
Level 1: "Post this tweet"                    → PostTweetTool
Level 2: "Post this thread on schedule"        → PostThreadTool + ScheduleContentTool
Level 3: "Run this week's campaign"            → Orchestration prompt + multiple tools
Level 4: "Manage our social presence ongoing"  → Full intelligence stack + analytics loop
```

Each level works independently. A user who just wants to post a tweet doesn't need the campaign orchestrator. But the orchestrator is there when they're ready.

---

## Existing Packages (Reference Implementations)

### amos-education (15 tools)

Domain: LMS, SCORM courses, CE credit tracking, law knowledge base, personalized learning.

Demonstrates: Schema bootstrapping, multi-tool-group registration, domain-specific system prompt, SCORM file parsing.

### amos-autoresearch (research swarms)

Domain: Autonomous research with Darwinian evolution of agent strategies.

Demonstrates: Complex multi-agent coordination, fitness evaluation, swarm routing, experiment tracking.

---

## Package Ideas

The following domains are natural fits for the package model:

| Package | Tools | Intelligence Layer |
|---------|-------|-------------------|
| **Legal** | DocuSign, contract parsing, clause extraction | Negotiation frameworks, compliance checklists, risk assessment |
| **Finance** | QuickBooks, Stripe, invoicing, reporting | Forecasting models, anomaly detection, cash flow optimization |
| **Sales** | CRM sync, lead scoring, outreach | Pipeline management, objection handling, deal strategy |
| **DevOps** | GitHub, CI/CD, monitoring, alerting | Incident response playbooks, deployment strategy, capacity planning |
| **Healthcare** | FHIR, scheduling, claims processing | Clinical decision support, coding compliance, care coordination |
| **Real Estate** | MLS, document generation, compliance | Market analysis, deal structuring, regulatory navigation |
| **E-commerce** | Shopify, inventory, fulfillment | Pricing strategy, demand forecasting, product positioning |

In each case, the tools are table stakes — everyone can call an API. The system prompts are the differentiation — the domain expertise that makes agents effective in that vertical.

---

## What's Next

This guide covers the technical mechanics of building a package. For how packages integrate with the token economy — how package creators earn from bounties completed using their tools — see the companion document: `PACKAGE_ECONOMY_INTEGRATION.md`.

---

*Packages are how AMOS becomes an ecosystem. Tools are the hands. Prompts are the brain. Together they're the intelligence layers that make the agent economy work.*
