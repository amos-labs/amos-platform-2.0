//! Permission cascade system.
//!
//! Defines permission levels that gate which tools are available in a session.
//! Tools declare their minimum required permission level. The session's active
//! permission level determines which tools the agent can use.
//!
//! ## Permission Levels (ordered least → most privileged)
//!
//! - **ReadOnly** — Can only read data, no modifications.
//! - **WorkspaceWrite** — Can modify workspace data (collections, records, files).
//! - **FullAccess** — Unrestricted: can execute code, manage agents, delete data.
//!
//! ## Configuration
//!
//! Set in `.amos/settings.json`:
//! ```json
//! { "permissions": { "defaultMode": "workspace-write" } }
//! ```

use serde::{Deserialize, Serialize};

/// Permission levels, ordered from least to most privileged.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    /// Read-only: can query data but not modify anything.
    ReadOnly = 0,
    /// Workspace write: can create/update/delete workspace data.
    #[default]
    WorkspaceWrite = 1,
    /// Full access: unrestricted, including code execution and agent management.
    FullAccess = 2,
}

impl PermissionLevel {
    pub fn as_str(&self) -> &str {
        match self {
            PermissionLevel::ReadOnly => "read_only",
            PermissionLevel::WorkspaceWrite => "workspace_write",
            PermissionLevel::FullAccess => "full_access",
        }
    }

    /// Parse from string (settings.json format).
    pub fn from_config_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "read-only" | "readonly" | "read_only" | "plan" => PermissionLevel::ReadOnly,
            "workspace-write" | "workspace_write" | "auto" | "default" => {
                PermissionLevel::WorkspaceWrite
            }
            "full-access" | "full_access" | "danger" | "danger-full-access" => {
                PermissionLevel::FullAccess
            }
            _ => PermissionLevel::WorkspaceWrite,
        }
    }

    /// Check if this level grants access to the required level.
    pub fn allows(&self, required: PermissionLevel) -> bool {
        *self >= required
    }
}

/// Permission policy for a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    /// The active permission mode for this session.
    pub active_mode: PermissionLevel,
    /// Per-tool permission overrides (tool_name → required level).
    #[serde(default)]
    pub tool_overrides: std::collections::HashMap<String, PermissionLevel>,
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        Self {
            active_mode: PermissionLevel::WorkspaceWrite,
            tool_overrides: std::collections::HashMap::new(),
        }
    }
}

impl PermissionPolicy {
    /// Check if a tool is allowed under this policy.
    pub fn allows_tool(&self, tool_name: &str, tool_required: PermissionLevel) -> bool {
        // Check per-tool override first
        if let Some(&override_level) = self.tool_overrides.get(tool_name) {
            return self.active_mode.allows(override_level);
        }
        self.active_mode.allows(tool_required)
    }
}

/// Assign default permission levels to tool categories.
pub fn default_tool_permission(tool_name: &str) -> PermissionLevel {
    // Strip harness_ prefix for matching
    let base = tool_name.strip_prefix("harness_").unwrap_or(tool_name);

    match base {
        // Read-only tools
        "think"
        | "recall"
        | "plan"
        | "web_search"
        | "read_file"
        | "get_workspace_summary"
        | "query_records"
        | "list_records"
        | "list_collections"
        | "search_memory"
        | "knowledge_search"
        | "view_web_page"
        | "get_canvas"
        | "list_canvases"
        | "list_sites"
        | "get_site"
        | "get_page"
        | "list_apps"
        | "get_app" => PermissionLevel::ReadOnly,

        // Workspace write tools
        "remember" | "write_file" | "define_collection" | "create_record" | "update_record"
        | "delete_record" | "create_canvas" | "update_canvas" | "create_site" | "create_page"
        | "update_page" | "publish_site" | "create_app" | "update_app_view" | "ingest_document"
        | "remember_this" | "create_automation" | "update_automation" => {
            PermissionLevel::WorkspaceWrite
        }

        // Full access tools (destructive or admin operations)
        "delete_collection" | "execute_code" | "register_agent" | "assign_task" | "delete_site"
        | "delete_canvas" | "delete_app" => PermissionLevel::FullAccess,

        // Default: workspace write for unknown tools
        _ => PermissionLevel::WorkspaceWrite,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_ordering() {
        assert!(PermissionLevel::ReadOnly < PermissionLevel::WorkspaceWrite);
        assert!(PermissionLevel::WorkspaceWrite < PermissionLevel::FullAccess);
    }

    #[test]
    fn test_allows() {
        assert!(PermissionLevel::FullAccess.allows(PermissionLevel::ReadOnly));
        assert!(PermissionLevel::WorkspaceWrite.allows(PermissionLevel::ReadOnly));
        assert!(!PermissionLevel::ReadOnly.allows(PermissionLevel::WorkspaceWrite));
    }

    #[test]
    fn test_policy_allows_tool() {
        let policy = PermissionPolicy::default(); // WorkspaceWrite
        assert!(policy.allows_tool("think", PermissionLevel::ReadOnly));
        assert!(policy.allows_tool("create_record", PermissionLevel::WorkspaceWrite));
        assert!(!policy.allows_tool("execute_code", PermissionLevel::FullAccess));
    }

    #[test]
    fn test_from_config_str() {
        assert_eq!(
            PermissionLevel::from_config_str("read-only"),
            PermissionLevel::ReadOnly
        );
        assert_eq!(
            PermissionLevel::from_config_str("workspace-write"),
            PermissionLevel::WorkspaceWrite
        );
        assert_eq!(
            PermissionLevel::from_config_str("danger-full-access"),
            PermissionLevel::FullAccess
        );
        assert_eq!(
            PermissionLevel::from_config_str("auto"),
            PermissionLevel::WorkspaceWrite
        );
    }

    #[test]
    fn test_default_tool_permissions() {
        assert_eq!(default_tool_permission("think"), PermissionLevel::ReadOnly);
        assert_eq!(
            default_tool_permission("harness_create_record"),
            PermissionLevel::WorkspaceWrite
        );
        assert_eq!(
            default_tool_permission("harness_execute_code"),
            PermissionLevel::FullAccess
        );
    }
}
