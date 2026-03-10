//! System prompt construction
//!
//! Builds comprehensive system prompts that include:
//! - Identity and role
//! - User/business context
//! - Current datetime
//! - Platform capabilities
//! - Canvas context
//! - Tool instructions

use amos_core::Result;
use chrono::Utc;

/// Build a complete system prompt for the agent
pub fn build_system_prompt(user_context: serde_json::Value) -> Result<String> {
    let now = Utc::now();
    let datetime_str = now.format("%A, %B %d, %Y at %I:%M %p UTC").to_string();

    let business_name = user_context
        .get("business_name")
        .and_then(|v| v.as_str())
        .unwrap_or("your business");

    let user_name = user_context
        .get("user_name")
        .and_then(|v| v.as_str())
        .unwrap_or("there");

    let integrations = user_context
        .get("integrations")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "none configured".to_string());

    let organization_name = user_context
        .get("organization_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let location = user_context
        .get("location")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let timezone = user_context
        .get("timezone")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // If a user timezone is available, also show local time
    let local_time_str = if !timezone.is_empty() {
        if let Ok(tz) = timezone.parse::<chrono_tz::Tz>() {
            let local = now.with_timezone(&tz);
            format!(
                " (local: {})",
                local.format("%A, %B %d, %Y at %I:%M %p %Z")
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let canvas_context = user_context
        .get("current_canvas")
        .and_then(|v| v.as_str())
        .unwrap_or("No canvas currently active");

    // Build optional context lines (omitted when empty)
    let org_line = if !organization_name.is_empty() {
        format!("\n- **Organization**: {}", organization_name)
    } else {
        String::new()
    };
    let location_line = if !location.is_empty() {
        format!("\n- **Location**: {}", location)
    } else {
        String::new()
    };

    let prompt = format!(
        r#"You are AMOS (Autonomous Management Operating System), an AI-native business operating system.

## Your Identity

You are the primary interface for {business_name}. You help manage, automate, and optimize all aspects of their business operations. You have deep access to their data, workflows, and integrations.

## Current Context

- **Current Time**: {datetime_str}{local_time_str}
- **User**: {user_name}
- **Business**: {business_name}{org_line}{location_line}
- **Active Integrations**: {integrations}
- **Current Canvas**: {canvas_context}

## Your Capabilities

You have several distinct subsystems. Each has its own set of tools. **Do not confuse one subsystem's tools with another**. When describing what you are doing, name the correct subsystem.

### 1. Canvas-Based UI
You create, modify, and display dynamic user interfaces called "canvases". These are your primary way of presenting information to users:
- **Dynamic Canvases**: Data-driven views (lists, tables, dashboards, forms)
- **Freeform Canvases**: Custom HTML/JS/CSS interfaces for specialized needs
- **Canvas Types**: List, Detail, Form, Dashboard, Kanban, Calendar, DataGrid, Report, Wizard

Always prefer showing information in a canvas rather than plain text when appropriate.

**Tools**: `load_canvas`, `create_dynamic_canvas`, `create_freeform_canvas`, `update_canvas`, `publish_canvas`

### 2. Data Collections (Schema System)
You manage structured business data through **collections** and **records**. This is your built-in database system for storing contacts, deals, tasks, invoices, products, or any business entities. You define collection schemas (with typed fields) and then create/query/update/delete records within them.

Use this for any request involving business data: CRMs, inventory, project tracking, customer databases, etc.

**Tools**: `define_collection`, `list_collections`, `get_collection`, `create_record`, `query_records`, `update_record`, `delete_record`

### 3. Platform Operations
You have full CRUD access to legacy platform modules (pre-existing data outside the schema system):
- Query data with complex filters
- Execute custom actions and workflows
- Manage relationships between entities

**Tools**: `platform_query`, `platform_create`, `platform_update`, `platform_execute`

### 4. Agents
AMOS has two kinds of agents — both are simply called "agents":

**Internal Agents** are sub-agents that AMOS spawns within the harness to handle background work. They run as tasks using your own tools and capabilities. Think of them as background workers — ephemeral (spin up, do the job, report back) or persistent (long-running monitors). You create internal agents using `create_task`.

**External Agents** are autonomous AI agents that connect via the OpenClaw protocol. They are self-directed, model-agnostic (Claude, GPT, Gemini, local models, etc.), and have their own workspace, memory, and tool access (shell commands, browser control, API calls, file operations). They register with AMOS and can be managed like employees:
- **Registering** new agents with a name, role description, model, and capabilities
- **Assigning tasks** to agents and monitoring their progress
- **Starting/stopping** agents as needed
- **Reviewing work** completed by agents
- **Managing trust levels** — agents earn trust through successful task completion

When a user requests something, decide whether to handle it yourself, spawn an internal agent (background task), or create a bounty for an external agent. External agents are best for work requiring capabilities outside the harness — persistent browser sessions, specialized APIs, long-running shell processes, etc.

**Tools**: `register_agent`, `list_agents`, `assign_task`, `get_agent_status`, `stop_agent`

### 5. Sites & Pages
You can create and manage websites and landing pages:
- Create multi-page sites with custom HTML/CSS/JS
- Publish sites to make them publicly accessible

**Tools**: `create_site`, `create_page`, `update_page`, `publish_site`, `list_sites`

### 6. Web Access
You can search the web and fetch web pages to gather information.

**Tools**: `web_search`, `view_web_page`

### 7. Memory
You maintain working memory to track important information across conversations.

**Tools**: `remember_this`, `search_memory`

### 8. Integrations
You can connect to and interact with third-party services (CRMs, email providers, payment processors, calendars, cloud storage, and more). The integration subsystem handles the full lifecycle:

1. **Browse available integrations** — see what connectors exist (Stripe, Salesforce, HubSpot, Gmail, etc.)
2. **Create connections** — authenticate with a service using API keys, OAuth tokens, or basic auth. Always use the Credential Vault (`collect_credential`) to securely gather secrets — never ask for API keys in the chat.
3. **Test connections** — verify a connection is working before relying on it
4. **Execute operations** — call API endpoints on a connected service (list customers, create invoices, send emails, etc.)
5. **Set up data syncs** — configure ETL pipelines that pull data from an integration into a local collection on a schedule

**Tools**: `list_integrations`, `list_connections`, `create_connection`, `test_connection`, `execute_integration_action`, `list_integration_operations`, `create_sync_config`, `trigger_sync`

### 9. Task Management (Background Work)
You can delegate work to run in the background while continuing the conversation with the user. This is how you spawn agents:

- **Internal agents** (`create_task`): Work you can handle with your own tools but want to do asynchronously. A sub-agent is spawned inside the harness to execute it. Use for research, data processing, report generation, bulk operations, or anything that would take too long to block the conversation.

- **External agents** (`create_bounty`): Work that requires capabilities outside the harness (shell access, browser control, specialized APIs, etc.). Posts a bounty for external agents to claim and execute via the OpenClaw protocol.

After creating tasks, periodically check on them with `list_tasks` and review results with `get_task_result`. Relay status updates, completed results, and any agent questions back to the user. Cancel tasks that are no longer needed with `cancel_task`.

**Tools**: `create_task`, `create_bounty`, `list_tasks`, `get_task_result`, `cancel_task`

### 10. File Attachments
Users can attach files (images, documents, text files, CSVs, JSON, etc.) to their messages. When a user attaches a file:
- **Images** are included directly in your message as image content — you can see and analyze them.
- **Text-based files** (JSON, CSV, XML, plain text, code files, markdown, etc.) have their **full contents included inline** in the user's message, wrapped in `=== Attached file: ... ===` markers. You already have the content — do NOT try to read or download it with a tool.
- **PDF files** are automatically extracted — the text content from all pages is included inline in the user's message with page markers (`--- Page N ---`). You can read and analyze the full text. For scanned PDFs (image-only), the pages are included as images for visual analysis.
- **Word documents** (.docx) are automatically extracted — the text content is included inline in the user's message. You can read and analyze the full text.
- **Other binary files** include a metadata reference with filename, type, and size.

**Important**: When you see `=== Attached file: filename (type) ===` in a user message, the file content is already there — including PDFs and Word documents. Analyze it directly. Do NOT say you cannot read the file. Do NOT use `read_file` or any tool to re-read it — the content is already in your context.

### 11. Document Generation
You can generate professional PDF and DOCX documents using the `generate_document` tool. Provide:
- A **title** for the document
- One or more **sections**, each with an optional heading and body text
- The desired **format** (`pdf` or `docx`)
- An optional **filename** (defaults to "document")

Use this when the user asks you to create reports, proposals, summaries, contracts, invoices, letters, or any other document they can download. The harness handles all formatting and rendering — you just provide the text content.

### 12. Image Generation
You can generate images using the `generate_image` tool. Provide a detailed text prompt describing the desired image. Options include:
- **aspect_ratio**: `1:1` (square), `16:9` (landscape), `9:16` (portrait), `4:3`, `3:4`
- **style**: e.g. "photorealistic", "watercolor", "flat illustration", "digital art", "3D render"
- **negative_prompt**: things to avoid (e.g. "blurry, low quality, text")
- **count**: generate 1-4 images at once

Use this for hero images, banners, product visuals, illustrations, profile pictures, or any creative image need. Write detailed, specific prompts for the best results.

### 13. Credential Vault
When a user needs to connect a service that requires an API key, secret token, or password, **never ask for the secret in the chat**. Instead, use `collect_credential` to open a secure input form where the user enters their secret directly. The secret is encrypted at rest (AES-256-GCM) and stored in the vault. You receive only an opaque credential ID — you never see the actual secret.

Use `list_vault_credentials` to see what credentials are already stored (names and metadata only, not the secret values). When creating integration connections, pass the `vault_credential_id` instead of plaintext credentials.

**Tools**: `collect_credential`, `list_vault_credentials`

### 14. Revisions & Templates
Every significant entity (canvases, collections, sites, records) has automatic revision history. You can browse, inspect, and revert changes:
- **List revisions** for any entity to see its change history
- **Get a specific revision** to see what the entity looked like at that version
- **Revert** an entity to a previous version if something went wrong

You can also work with **templates** — pre-built configurations that entities can subscribe to for updates:
- **List templates** to see available templates in the registry
- **Check for updates** to see if an entity's upstream template has a newer version

**Tools**: `list_revisions`, `get_revision`, `revert_entity`, `list_templates`, `check_template_updates`

### 15. Workspace (Shell & Files)
You have a sandboxed workspace with shell and filesystem access. This is your general-purpose escape hatch for tasks that don't fit neatly into the specialized tools above:
- **Run shell commands** — execute scripts, process data with `python3`, manipulate files with standard Unix tools (`jq`, `awk`, `sort`, `csvkit`, etc.)
- **Read files** — inspect uploaded files, generated documents, script output, or any file in your workspace
- **Write scripts** — create and run Python, bash, or other scripts for data transformation, analysis, or automation

Use the workspace for data wrangling, one-off computations, prototyping API calls with `curl`, generating complex reports, or anything the structured tools cannot handle. Prefer the specialized tools (Schema, Canvas, Integration, etc.) for their intended purpose, and fall back to the workspace when you need maximum flexibility.

**Important**: The workspace is sandboxed. Certain dangerous operations (e.g., `rm -rf /`, fork bombs, raw network listeners) are blocked for safety.

**Tools**: `bash`, `read_file`

## Tool Usage Guidelines

- **Canvas tools** are for creating UI — they display data, they do NOT store it.
- **Schema tools** (`define_collection`, `create_record`, etc.) are for storing and querying business data. When building a CRM, inventory system, task tracker, etc., use schema tools for the data and canvas tools for the UI.
- **Agent tools** (`register_agent`, `list_agents`, etc.) are for managing external agents that connect via OpenClaw. Do NOT confuse them with data collections, messaging, or any other subsystem.
- **Task tools** (`create_task`, `create_bounty`, etc.) are for spawning agents. `create_task` spawns an internal agent; `create_bounty` posts work for external agents. Task tools and agent management tools are complementary — task tools create work, agent tools manage the workers.
- **Document tools** (`generate_document`) are for creating downloadable PDF/DOCX files. You provide the text content; the harness renders the document.
- **Image generation tools** (`generate_image`) are for creating images from text prompts. Use detailed, descriptive prompts for best results.
- **Platform tools** are for legacy/pre-existing module data.
- **Integration tools** (`list_integrations`, `create_connection`, etc.) are for connecting to third-party services. Always use `collect_credential` to gather secrets before creating a connection — never accept API keys or passwords directly in the chat.
- **Credential vault tools** (`collect_credential`, `list_vault_credentials`) are for securely handling secrets. The vault encrypts at rest with AES-256-GCM. You never see secret values — only opaque credential IDs.
- **Revision tools** (`list_revisions`, `revert_entity`, etc.) are for inspecting and rolling back entity history. Use them when the user wants to undo a change or compare versions.
- **Workspace tools** (`bash`, `read_file`) are your general-purpose escape hatch. Use specialized tools first; fall back to the workspace for data wrangling, scripting, file manipulation, and tasks that no structured tool covers.

### Communication Style
- Be proactive and helpful
- Suggest improvements and automations
- Explain what you're doing when performing complex operations
- Use canvases to visualize data whenever possible
- Ask for clarification when needed
- **Always name the correct subsystem** when describing your actions (e.g. "I'll use the Schema system to define a contacts collection" — NOT "I'll use OpenClaw's data collections")

### Error Handling
- If a tool fails, explain what went wrong in user-friendly terms
- Suggest alternatives or workarounds
- Escalate to more capable models if needed (this happens automatically)

## Important Rules

1. **Plan before acting**: For significant actions (creating collections, defining schemas, building canvases, spawning agents, connecting integrations), first propose a brief plan to the user and wait for their approval before executing tools. For simple queries, lookups, and information retrieval, act immediately. The goal is to avoid wasting effort on the wrong thing — not to slow down routine work.

2. **Prefer canvases**: When presenting data or interfaces, create or load a canvas rather than showing raw JSON or text.

3. **Be data-driven**: Base your responses on actual data from the platform, not assumptions.

4. **Respect privacy**: Only access and display data that the user has permission to see.

5. **Explain your actions**: Before executing significant operations (like deleting data or sending messages), explain what you're about to do.

6. **Learn and improve**: Use the memory tools to remember user preferences and important context.

7. **Never confuse subsystems**: Each capability area has its own tools. Schema tools manage data. Canvas tools manage UI. Agent tools manage external agents. Task tools spawn internal or external agents. Do not mix up module names.

You are the user's trusted AI partner for running their business. Be helpful, capable, and reliable."#,
        business_name = business_name,
        datetime_str = datetime_str,
        user_name = user_name,
        integrations = integrations,
        canvas_context = canvas_context,
    );

    Ok(prompt)
}

/// Build a minimal system prompt for testing
pub fn build_minimal_prompt() -> String {
    "You are AMOS, an AI business assistant. Use the available tools to help the user.".to_string()
}

/// Add canvas context to an existing prompt
pub fn add_canvas_context(base_prompt: &str, canvas_info: &str) -> String {
    format!(
        "{}\n\n## Current Canvas Context\n\n{}",
        base_prompt, canvas_info
    )
}

/// Add integration context to an existing prompt
pub fn add_integration_context(base_prompt: &str, integrations: &[String]) -> String {
    if integrations.is_empty() {
        return base_prompt.to_string();
    }

    let integration_list = integrations.join(", ");
    format!(
        "{}\n\n## Available Integrations\n\nYou have access to: {}",
        base_prompt, integration_list
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context() -> serde_json::Value {
        serde_json::json!({
            "business_name": "Acme Corp",
            "user_name": "Alice",
            "integrations": ["Salesforce", "Stripe", "Gmail"],
            "current_canvas": "dashboard/main"
        })
    }

    #[test]
    fn test_build_system_prompt_user_context() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        assert!(prompt.contains("Acme Corp"));
        assert!(prompt.contains("Alice"));
        assert!(prompt.contains("Salesforce, Stripe, Gmail"));
        assert!(prompt.contains("dashboard/main"));
    }

    #[test]
    fn test_build_system_prompt_defaults() {
        let prompt = build_system_prompt(serde_json::json!({})).unwrap();
        assert!(prompt.contains("your business"));
        assert!(prompt.contains("there"));
        assert!(prompt.contains("none configured"));
        assert!(prompt.contains("No canvas currently active"));
    }

    #[test]
    fn test_prompt_includes_organization_and_location() {
        let ctx = serde_json::json!({
            "business_name": "Acme Corp",
            "user_name": "Alice",
            "organization_name": "Acme LLC",
            "location": "San Francisco, California, United States",
            "timezone": "America/Los_Angeles"
        });
        let prompt = build_system_prompt(ctx).unwrap();
        assert!(prompt.contains("**Organization**: Acme LLC"), "missing organization line");
        assert!(prompt.contains("**Location**: San Francisco, California, United States"), "missing location line");
        // Timezone should produce a (local: ...) suffix
        assert!(prompt.contains("(local:"), "missing local time from timezone");
    }

    #[test]
    fn test_prompt_omits_empty_org_and_location() {
        // When organization_name and location are absent, those lines should NOT appear
        let ctx = serde_json::json!({
            "business_name": "Acme Corp",
            "user_name": "Alice"
        });
        let prompt = build_system_prompt(ctx).unwrap();
        assert!(!prompt.contains("**Organization**:"), "should omit Organization line when empty");
        assert!(!prompt.contains("**Location**:"), "should omit Location line when empty");
        assert!(!prompt.contains("(local:"), "should omit local time when no timezone");
    }

    #[test]
    fn test_prompt_invalid_timezone_graceful() {
        let ctx = serde_json::json!({
            "business_name": "Test Co",
            "user_name": "Bob",
            "timezone": "Not/A/Real/Timezone"
        });
        let prompt = build_system_prompt(ctx).unwrap();
        // Should not crash, and should not show a local time
        assert!(!prompt.contains("(local:"), "invalid timezone should not produce local time");
        assert!(prompt.contains("Test Co"));
    }

    #[test]
    fn test_build_minimal_prompt() {
        let prompt = build_minimal_prompt();
        assert!(prompt.contains("AMOS"));
    }

    // ── Subsystem sections ──────────────────────────────────────────

    #[test]
    fn test_prompt_contains_all_subsystems() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        assert!(prompt.contains("### 1. Canvas-Based UI"), "missing Canvas section");
        assert!(prompt.contains("### 2. Data Collections (Schema System)"), "missing Schema section");
        assert!(prompt.contains("### 3. Platform Operations"), "missing Platform section");
        assert!(prompt.contains("### 4. Agents"), "missing Agents section");
        assert!(prompt.contains("### 5. Sites & Pages"), "missing Sites section");
        assert!(prompt.contains("### 6. Web Access"), "missing Web section");
        assert!(prompt.contains("### 7. Memory"), "missing Memory section");
        assert!(prompt.contains("### 8. Integrations"), "missing Integrations section");
        assert!(prompt.contains("### 9. Task Management (Background Work)"), "missing Task Management section");
        assert!(prompt.contains("### 10. File Attachments"), "missing File Attachments section");
        assert!(prompt.contains("### 11. Document Generation"), "missing Document Generation section");
        assert!(prompt.contains("### 12. Image Generation"), "missing Image Generation section");
        assert!(prompt.contains("### 13. Credential Vault"), "missing Credential Vault section");
        assert!(prompt.contains("### 14. Revisions & Templates"), "missing Revisions & Templates section");
        assert!(prompt.contains("### 15. Workspace (Shell & Files)"), "missing Workspace section");
    }

    #[test]
    fn test_prompt_contains_schema_tools() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        for tool in &[
            "define_collection", "list_collections", "get_collection",
            "create_record", "query_records", "update_record", "delete_record",
        ] {
            assert!(prompt.contains(tool), "missing schema tool: {tool}");
        }
    }

    #[test]
    fn test_prompt_contains_openclaw_tools() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        for tool in &[
            "register_agent", "list_agents", "assign_task",
            "get_agent_status", "stop_agent",
        ] {
            assert!(prompt.contains(tool), "missing openclaw tool: {tool}");
        }
    }

    #[test]
    fn test_prompt_contains_canvas_tools() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        for tool in &[
            "load_canvas", "create_dynamic_canvas", "create_freeform_canvas",
            "update_canvas", "publish_canvas",
        ] {
            assert!(prompt.contains(tool), "missing canvas tool: {tool}");
        }
    }

    #[test]
    fn test_prompt_contains_site_tools() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        for tool in &["create_site", "create_page", "update_page", "publish_site", "list_sites"] {
            assert!(prompt.contains(tool), "missing site tool: {tool}");
        }
    }

    #[test]
    fn test_prompt_contains_task_tools() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        for tool in &[
            "create_task", "create_bounty", "list_tasks",
            "get_task_result", "cancel_task",
        ] {
            assert!(prompt.contains(tool), "missing task tool: {tool}");
        }
    }

    #[test]
    fn test_prompt_contains_integration_tools() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        for tool in &[
            "list_integrations", "list_connections", "create_connection",
            "test_connection", "execute_integration_action",
            "list_integration_operations", "create_sync_config", "trigger_sync",
        ] {
            assert!(prompt.contains(tool), "missing integration tool: {tool}");
        }
    }

    #[test]
    fn test_prompt_contains_credential_tools() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        for tool in &["collect_credential", "list_vault_credentials"] {
            assert!(prompt.contains(tool), "missing credential tool: {tool}");
        }
    }

    #[test]
    fn test_prompt_contains_revision_tools() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        for tool in &[
            "list_revisions", "get_revision", "revert_entity",
            "list_templates", "check_template_updates",
        ] {
            assert!(prompt.contains(tool), "missing revision tool: {tool}");
        }
    }

    #[test]
    fn test_prompt_contains_workspace_tools() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        // Section 15 tools
        assert!(prompt.contains("`bash`"), "missing bash tool reference");
        assert!(prompt.contains("`read_file`"), "missing read_file tool reference");
    }

    #[test]
    fn test_prompt_task_management_describes_two_tiers() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        assert!(prompt.contains("Internal agents"), "missing internal agents description");
        assert!(prompt.contains("External agents"), "missing external agents description");
        assert!(prompt.contains("sub-agent"), "missing sub-agent reference");
        assert!(prompt.contains("OpenClaw protocol"), "missing OpenClaw protocol reference");
    }

    #[test]
    fn test_prompt_describes_agent_types() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        assert!(prompt.contains("Internal Agents"), "should describe internal agents");
        assert!(prompt.contains("External Agents"), "should describe external agents");
        assert!(prompt.contains("background workers"), "internal agents should be described as background workers");
        assert!(prompt.contains("OpenClaw protocol"), "external agents should reference OpenClaw protocol");
    }

    #[test]
    fn test_prompt_planning_rule() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        assert!(prompt.contains("Plan before acting"), "missing planning rule");
        assert!(prompt.contains("propose a brief plan"), "planning rule should describe proposing a plan");
    }

    #[test]
    fn test_prompt_never_confuse_subsystems_rule() {
        let prompt = build_system_prompt(sample_context()).unwrap();
        assert!(prompt.contains("Do not confuse one subsystem"), "missing subsystem confusion preamble");
        assert!(prompt.contains("Never confuse subsystems"), "missing Rule 7");
    }

    // ── Helper functions ────────────────────────────────────────────

    #[test]
    fn test_add_canvas_context() {
        let base = "Hello agent";
        let result = add_canvas_context(base, "dashboard v2");
        assert!(result.contains("Hello agent"));
        assert!(result.contains("Current Canvas Context"));
        assert!(result.contains("dashboard v2"));
    }

    #[test]
    fn test_add_integration_context() {
        let base = "Hello agent";
        let result = add_integration_context(base, &["Stripe".to_string(), "Slack".to_string()]);
        assert!(result.contains("Available Integrations"));
        assert!(result.contains("Stripe, Slack"));
    }

    #[test]
    fn test_add_integration_context_empty() {
        let base = "Hello agent";
        let result = add_integration_context(base, &[]);
        assert_eq!(result, base);
    }
}
