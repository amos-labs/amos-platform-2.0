//! Hierarchical settings system.
//!
//! Loads and merges settings from multiple sources in precedence order:
//!
//! 1. **User** — `~/.amos/settings.json` (personal defaults)
//! 2. **Project** — `.amos/settings.json` (workspace-level, committed to repo)
//! 3. **Local** — `.amos/settings.local.json` (machine-specific, gitignored)
//! 4. **Session** — per-request overrides via API
//!
//! Later sources override earlier ones. Objects are deep-merged; arrays are
//! concatenated and deduplicated for hooks.

use crate::hooks::HookConfig;
use crate::permissions::PermissionLevel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Source of a settings entry (for debugging/audit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingsSource {
    User,
    Project,
    Local,
    Session,
}

impl SettingsSource {
    pub fn as_str(&self) -> &str {
        match self {
            SettingsSource::User => "user",
            SettingsSource::Project => "project",
            SettingsSource::Local => "local",
            SettingsSource::Session => "session",
        }
    }
}

/// MCP server configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    /// Transport type: "stdio", "sse", "http".
    #[serde(rename = "type", default = "default_mcp_type")]
    pub transport: String,
    /// Command to run (stdio transport).
    #[serde(default)]
    pub command: Option<String>,
    /// Arguments for the command (stdio transport).
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables for the process.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// URL endpoint (sse/http transport).
    #[serde(default)]
    pub url: Option<String>,
    /// HTTP headers for remote transports.
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

fn default_mcp_type() -> String {
    "stdio".to_string()
}

/// The merged settings object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AmosSettings {
    /// Hook configuration (pre/post tool).
    #[serde(default)]
    pub hooks: HookConfig,

    /// Permission settings.
    #[serde(default)]
    pub permissions: PermissionsSettings,

    /// Model override.
    #[serde(default)]
    pub model: Option<String>,

    /// System prompt override.
    #[serde(default)]
    pub system_prompt: Option<String>,

    /// MCP server configurations.
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerEntry>,

    /// Allowed tools list (if set, only these tools are available).
    #[serde(default, rename = "allowedTools")]
    pub allowed_tools: Option<Vec<String>>,

    /// Denied tools list (these tools are never available).
    #[serde(default, rename = "deniedTools")]
    pub denied_tools: Option<Vec<String>>,

    /// Custom slash commands directory (default: .amos/commands).
    #[serde(default, rename = "commandsDir")]
    pub commands_dir: Option<String>,
}

/// Permission sub-settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionsSettings {
    /// Default permission mode.
    #[serde(default, rename = "defaultMode")]
    pub default_mode: Option<String>,
}

/// Loaded settings entry (source + path for debugging).
#[derive(Debug, Clone)]
pub struct LoadedEntry {
    pub source: SettingsSource,
    pub path: PathBuf,
}

/// Load and merge settings from all sources.
///
/// `project_dir` is the workspace root directory.
pub fn load_settings(project_dir: Option<&Path>) -> AmosSettings {
    let mut merged = AmosSettings::default();
    let mut entries: Vec<LoadedEntry> = Vec::new();

    // 1. User settings: ~/.amos/settings.json
    if let Some(home) = dirs_path() {
        let user_path = home.join(".amos").join("settings.json");
        if let Some(settings) = load_file(&user_path) {
            merge_settings(&mut merged, &settings);
            entries.push(LoadedEntry {
                source: SettingsSource::User,
                path: user_path,
            });
        }
    }

    // 2. Project settings: .amos/settings.json
    if let Some(dir) = project_dir {
        let project_path = dir.join(".amos").join("settings.json");
        if let Some(settings) = load_file(&project_path) {
            merge_settings(&mut merged, &settings);
            entries.push(LoadedEntry {
                source: SettingsSource::Project,
                path: project_path,
            });
        }

        // 3. Local settings: .amos/settings.local.json
        let local_path = dir.join(".amos").join("settings.local.json");
        if let Some(settings) = load_file(&local_path) {
            merge_settings(&mut merged, &settings);
            entries.push(LoadedEntry {
                source: SettingsSource::Local,
                path: local_path,
            });
        }
    }

    if !entries.is_empty() {
        let sources: Vec<&str> = entries.iter().map(|e| e.source.as_str()).collect();
        tracing::info!(sources = ?sources, "Loaded settings from {} sources", entries.len());
    }

    merged
}

/// Apply session-level overrides to settings.
pub fn apply_session_overrides(settings: &mut AmosSettings, overrides: &serde_json::Value) {
    if let Some(model) = overrides.get("model").and_then(|v| v.as_str()) {
        settings.model = Some(model.to_string());
    }
    if let Some(mode) = overrides
        .get("permissions")
        .and_then(|p| p.get("defaultMode"))
        .and_then(|v| v.as_str())
    {
        settings.permissions.default_mode = Some(mode.to_string());
    }
    if let Some(prompt) = overrides.get("system_prompt").and_then(|v| v.as_str()) {
        settings.system_prompt = Some(prompt.to_string());
    }
}

/// Get the resolved permission level from settings.
pub fn resolve_permission_level(settings: &AmosSettings) -> PermissionLevel {
    settings
        .permissions
        .default_mode
        .as_deref()
        .map(PermissionLevel::from_config_str)
        .unwrap_or_default()
}

/// Load a single settings file, returning None if it doesn't exist or is invalid.
fn load_file(path: &Path) -> Option<AmosSettings> {
    let content = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<AmosSettings>(&content) {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Failed to parse settings file");
            None
        }
    }
}

/// Deep-merge source settings into target.
fn merge_settings(target: &mut AmosSettings, source: &AmosSettings) {
    // Hooks: concatenate arrays
    target
        .hooks
        .pre_tool_use
        .extend(source.hooks.pre_tool_use.clone());
    target
        .hooks
        .post_tool_use
        .extend(source.hooks.post_tool_use.clone());

    // Scalars: later wins
    if source.model.is_some() {
        target.model.clone_from(&source.model);
    }
    if source.system_prompt.is_some() {
        target.system_prompt.clone_from(&source.system_prompt);
    }
    if source.permissions.default_mode.is_some() {
        target
            .permissions
            .default_mode
            .clone_from(&source.permissions.default_mode);
    }
    if source.commands_dir.is_some() {
        target.commands_dir.clone_from(&source.commands_dir);
    }

    // MCP servers: merge map (later overrides same name)
    for (name, config) in &source.mcp_servers {
        target.mcp_servers.insert(name.clone(), config.clone());
    }

    // Tool lists: later wins if set
    if source.allowed_tools.is_some() {
        target.allowed_tools.clone_from(&source.allowed_tools);
    }
    if source.denied_tools.is_some() {
        target.denied_tools.clone_from(&source.denied_tools);
    }
}

/// Get the user's home directory.
fn dirs_path() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let s = AmosSettings::default();
        assert!(s.hooks.pre_tool_use.is_empty());
        assert!(s.mcp_servers.is_empty());
        assert!(s.model.is_none());
    }

    #[test]
    fn test_merge_settings() {
        let mut target = AmosSettings::default();
        let source = AmosSettings {
            model: Some("claude-opus-4-6".to_string()),
            ..Default::default()
        };
        merge_settings(&mut target, &source);
        assert_eq!(target.model, Some("claude-opus-4-6".to_string()));
    }

    #[test]
    fn test_deserialize_settings() {
        let json = r#"{
            "model": "claude-sonnet-4-6",
            "hooks": {
                "PreToolUse": [
                    {"command": "echo test", "matcher": "execute_*"}
                ]
            },
            "permissions": {
                "defaultMode": "read-only"
            },
            "mcpServers": {
                "my-server": {
                    "type": "stdio",
                    "command": "uvx",
                    "args": ["my-mcp-server"]
                }
            }
        }"#;
        let settings: AmosSettings = serde_json::from_str(json).unwrap();
        assert_eq!(settings.model, Some("claude-sonnet-4-6".to_string()));
        assert_eq!(settings.hooks.pre_tool_use.len(), 1);
        assert!(settings.mcp_servers.contains_key("my-server"));
        assert_eq!(
            settings.permissions.default_mode,
            Some("read-only".to_string())
        );
    }

    #[test]
    fn test_resolve_permission_level() {
        let s = AmosSettings {
            permissions: PermissionsSettings {
                default_mode: Some("read-only".to_string()),
            },
            ..Default::default()
        };
        assert_eq!(resolve_permission_level(&s), PermissionLevel::ReadOnly);
    }
}
