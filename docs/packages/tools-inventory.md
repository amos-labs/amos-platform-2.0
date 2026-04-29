# AMOS Harness Tools Inventory

54 tools across 14 categories, exposed to agents via the External Agent Protocol.

Agents call tools at `POST /api/v1/tools/{name}/execute`.

---

## Platform Tools (4 tools) -- `platform_tools.rs`

| Tool | Description |
|------|-------------|
| `platform_query` | Query records from any module |
| `platform_create` | Create a new record in any platform module |
| `platform_update` | Update an existing record in any platform module |
| `platform_execute` | Execute a custom action on a module or record |

## Canvas Tools (5 tools) -- `canvas_tools.rs`

| Tool | Description |
|------|-------------|
| `load_canvas` | Load and display an existing canvas by slug |
| `create_dynamic_canvas` | Create a data-driven canvas (list, table, dashboard, form) |
| `create_freeform_canvas` | Create a custom canvas with full HTML/CSS/JS control |
| `update_canvas` | Update an existing canvas's content or configuration |
| `publish_canvas` | Make a canvas publicly accessible via a unique URL |

## Schema Tools (7 tools) -- `schema_tools.rs`

| Tool | Description |
|------|-------------|
| `define_collection` | Define or update a data collection's schema |
| `list_collections` | List all defined data collections |
| `get_collection` | Get the full schema definition of a specific collection |
| `create_record` | Create a new record in a data collection |
| `query_records` | Query records with filters, sorting, and pagination |
| `update_record` | Update an existing record (merge semantics) |
| `delete_record` | Delete a record by its ID |

## Integration Tools (8 tools) -- `integration_tools.rs`

| Tool | Description |
|------|-------------|
| `list_integrations` | List all available third-party integrations |
| `list_connections` | List active integration connections |
| `create_connection` | Add a new integration connection with auth credentials |
| `test_connection` | Test if an integration connection is working |
| `execute_integration_action` | Execute an API operation on an integration |
| `list_integration_operations` | List available operations for a specific integration |
| `create_sync_config` | Configure an ETL sync to pull data into a collection |
| `trigger_sync` | Manually trigger an ETL sync job |

## Task Tools (5 tools) -- `task_tools.rs`

| Tool | Description |
|------|-------------|
| `create_task` | Create an internal background task |
| `create_bounty` | Post an external bounty for agents to claim |
| `list_tasks` | List tasks and bounties with optional filtering |
| `get_task_result` | Get status, result, and message history for a task |
| `cancel_task` | Cancel a pending or running task |

## Agent Management Tools (5 tools) -- `openclaw_tools.rs`

| Tool | Description |
|------|-------------|
| `register_agent` | Register a new autonomous agent with AMOS |
| `list_agents` | List registered agents, their roles, status, and trust levels |
| `assign_task` | Assign a task to a specific agent |
| `get_agent_status` | Get agent status including active and recent tasks |
| `stop_agent` | Stop an agent and cancel its pending tasks |

## Site Tools (5 tools) -- `site_tools.rs`

| Tool | Description |
|------|-------------|
| `create_site` | Create a new website or landing page |
| `create_page` | Create or update a page on a website |
| `update_page` | Update an existing page's content |
| `publish_site` | Publish a site to make it publicly accessible |
| `list_sites` | List all websites and landing pages |

## Revision Tools (5 tools) -- `revision_tools.rs`

| Tool | Description |
|------|-------------|
| `list_revisions` | List revision history for an entity |
| `get_revision` | Get a specific revision by version number |
| `revert_entity` | Revert an entity to a previous version |
| `list_templates` | List available templates from the registry |
| `check_template_updates` | Check if an entity's template has available updates |

## Credential Tools (2 tools) -- `credential_tools.rs`

| Tool | Description |
|------|-------------|
| `collect_credential` | Securely collect a credential via Secure Input Canvas |
| `list_vault_credentials` | List stored credentials (metadata only, no plaintext) |

## Memory Tools (2 tools) -- `memory_tools.rs`

| Tool | Description |
|------|-------------|
| `remember_this` | Save information to working memory for future reference |
| `search_memory` | Search working memory for previously saved information |

## Web Tools (2 tools) -- `web_tools.rs`

| Tool | Description |
|------|-------------|
| `web_search` | Search the web for information |
| `view_web_page` | Fetch and parse web page content |

## System Tools (2 tools) -- `system_tools.rs`

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents from the filesystem |
| `bash` | Execute a shell command |

## Document Tools (1 tool) -- `document_tools.rs`

| Tool | Description |
|------|-------------|
| `generate_document` | Generate a PDF or DOCX from structured text content |

## Image Generation Tools (1 tool) -- `image_gen_tools.rs`

| Tool | Description |
|------|-------------|
| `generate_image` | Generate an image from a text prompt using AI |

---

## Summary

| Category | File | Count |
|----------|------|-------|
| Platform | `platform_tools.rs` | 4 |
| Canvas | `canvas_tools.rs` | 5 |
| Schema | `schema_tools.rs` | 7 |
| Integration | `integration_tools.rs` | 8 |
| Task | `task_tools.rs` | 5 |
| Agent Management | `openclaw_tools.rs` | 5 |
| Site | `site_tools.rs` | 5 |
| Revision | `revision_tools.rs` | 5 |
| Credential | `credential_tools.rs` | 2 |
| Memory | `memory_tools.rs` | 2 |
| Web | `web_tools.rs` | 2 |
| System | `system_tools.rs` | 2 |
| Document | `document_tools.rs` | 1 |
| Image Gen | `image_gen_tools.rs` | 1 |
| **Total** | **14 files** | **54** |
