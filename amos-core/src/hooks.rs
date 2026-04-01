//! Hook system — pre/post tool execution hooks.
//!
//! Hooks are shell commands that fire before and/or after tool calls.
//! They can block execution (deny), log, or modify behavior.
//!
//! Configured in `.amos/settings.json` under `hooks`:
//! ```json
//! {
//!   "hooks": {
//!     "PreToolUse": [
//!       { "matcher": "execute_code", "command": "echo 'blocked' && exit 2" }
//!     ],
//!     "PostToolUse": [
//!       { "command": "/usr/local/bin/audit-log.sh" }
//!     ]
//!   }
//! }
//! ```
//!
//! ## Exit Codes
//! - `0` — Allow (success)
//! - `2` — Deny (block tool execution)
//! - Other — Warn (allow but log warning)

use serde::{Deserialize, Serialize};

/// Hook event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
}

impl HookEvent {
    pub fn as_str(&self) -> &str {
        match self {
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
        }
    }
}

/// A single hook definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDef {
    /// Shell command to execute.
    pub command: String,
    /// Optional tool name matcher (glob pattern). If None, matches all tools.
    #[serde(default)]
    pub matcher: Option<String>,
    /// Timeout in milliseconds (default: 10_000).
    #[serde(default = "default_hook_timeout")]
    pub timeout_ms: u64,
}

fn default_hook_timeout() -> u64 {
    10_000
}

/// Hook configuration loaded from settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookConfig {
    /// Hooks to run before tool execution.
    #[serde(default, rename = "PreToolUse")]
    pub pre_tool_use: Vec<HookDef>,
    /// Hooks to run after tool execution.
    #[serde(default, rename = "PostToolUse")]
    pub post_tool_use: Vec<HookDef>,
}

/// Result of running a hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRunResult {
    /// Whether the hook denied (blocked) the action.
    pub denied: bool,
    /// Messages from hook stdout/stderr.
    pub messages: Vec<String>,
}

impl HookRunResult {
    pub fn allowed() -> Self {
        Self {
            denied: false,
            messages: Vec::new(),
        }
    }

    pub fn denied(message: String) -> Self {
        Self {
            denied: true,
            messages: vec![message],
        }
    }
}

/// Payload sent to hooks via stdin as JSON.
#[derive(Debug, Serialize)]
pub struct HookPayload {
    pub hook_event_name: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    /// Only present for PostToolUse hooks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output: Option<String>,
    /// Only present for PostToolUse hooks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result_is_error: Option<bool>,
}

/// Run hooks for an event, returning the aggregate result.
///
/// Hooks are run sequentially. If any hook denies, execution stops.
pub async fn run_hooks(
    config: &HookConfig,
    event: HookEvent,
    tool_name: &str,
    tool_input: &serde_json::Value,
    tool_output: Option<&str>,
    tool_is_error: Option<bool>,
) -> HookRunResult {
    let hooks = match event {
        HookEvent::PreToolUse => &config.pre_tool_use,
        HookEvent::PostToolUse => &config.post_tool_use,
    };

    let mut result = HookRunResult::allowed();

    for hook in hooks {
        // Check matcher
        if let Some(ref matcher) = hook.matcher {
            if !matches_tool(matcher, tool_name) {
                continue;
            }
        }

        let payload = HookPayload {
            hook_event_name: event.as_str().to_string(),
            tool_name: tool_name.to_string(),
            tool_input: tool_input.clone(),
            tool_output: tool_output.map(|s| s.to_string()),
            tool_result_is_error: tool_is_error,
        };

        match execute_hook_command(&hook.command, &payload, hook.timeout_ms).await {
            Ok(output) => {
                if output.denied {
                    return output;
                }
                result.messages.extend(output.messages);
            }
            Err(e) => {
                tracing::warn!(hook = %hook.command, error = %e, "Hook execution error");
                result.messages.push(format!("Hook error: {e}"));
            }
        }
    }

    result
}

/// Simple glob matching for tool names.
/// Supports `*` as wildcard and exact match.
fn matches_tool(pattern: &str, tool_name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if pattern.contains('*') {
        // Simple prefix/suffix glob
        if let Some(prefix) = pattern.strip_suffix('*') {
            return tool_name.starts_with(prefix);
        }
        if let Some(suffix) = pattern.strip_prefix('*') {
            return tool_name.ends_with(suffix);
        }
    }
    pattern == tool_name
}

/// Execute a shell command as a hook.
async fn execute_hook_command(
    command: &str,
    payload: &HookPayload,
    timeout_ms: u64,
) -> std::result::Result<HookRunResult, String> {
    use tokio::process::Command;

    let payload_json = serde_json::to_string(payload).map_err(|e| e.to_string())?;

    let child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env("HOOK_EVENT", payload.hook_event_name.as_str())
        .env("HOOK_TOOL_NAME", &payload.tool_name)
        .env(
            "HOOK_TOOL_INPUT",
            serde_json::to_string(&payload.tool_input).unwrap_or_default(),
        )
        .spawn()
        .map_err(|e| format!("Failed to spawn hook: {e}"))?;

    // Write payload to stdin
    let mut child = child;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(payload_json.as_bytes()).await;
        drop(stdin);
    }

    // Wait with timeout
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| format!("Hook timed out after {timeout_ms}ms"))?
        .map_err(|e| format!("Hook execution failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let mut messages = Vec::new();
    if !stdout.trim().is_empty() {
        messages.push(stdout.trim().to_string());
    }
    if !stderr.trim().is_empty() {
        messages.push(stderr.trim().to_string());
    }

    let exit_code = output.status.code().unwrap_or(1);

    match exit_code {
        0 => Ok(HookRunResult {
            denied: false,
            messages,
        }),
        2 => Ok(HookRunResult {
            denied: true,
            messages,
        }),
        code => {
            tracing::warn!(
                hook = %command,
                exit_code = code,
                "Hook returned non-standard exit code (allowing)"
            );
            Ok(HookRunResult {
                denied: false,
                messages,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_tool() {
        assert!(matches_tool("*", "anything"));
        assert!(matches_tool("execute_code", "execute_code"));
        assert!(!matches_tool("execute_code", "read_file"));
        assert!(matches_tool("harness_*", "harness_create_record"));
        assert!(!matches_tool("harness_*", "read_file"));
        assert!(matches_tool("*_file", "read_file"));
        assert!(matches_tool("*_file", "write_file"));
        assert!(!matches_tool("*_file", "think"));
    }

    #[test]
    fn test_hook_config_deserialize() {
        let json = r#"{
            "PreToolUse": [
                {"command": "echo hi", "matcher": "execute_*"}
            ],
            "PostToolUse": []
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.pre_tool_use.len(), 1);
        assert_eq!(
            config.pre_tool_use[0].matcher,
            Some("execute_*".to_string())
        );
    }
}
