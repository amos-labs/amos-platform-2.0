//! System tools for file and process operations

use super::{Tool, ToolCategory, ToolResult};
use amos_core::Result;
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use std::process::Command;
use tokio::fs;

/// Read a file from the filesystem
pub struct ReadFileTool;

impl ReadFileTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file from the filesystem"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let path = params["path"].as_str().ok_or_else(|| {
            amos_core::AmosError::Validation("path is required".to_string())
        })?;

        // Security check: prevent reading sensitive files
        let blocked_paths = ["/etc/passwd", "/etc/shadow", ".env", "credentials"];
        if blocked_paths.iter().any(|p| path.contains(p)) {
            return Ok(ToolResult::error(
                "Access denied: Cannot read sensitive files".to_string(),
            ));
        }

        let content = fs::read_to_string(path).await.map_err(|e| {
            amos_core::AmosError::Internal(format!("Failed to read file: {}", e))
        })?;

        Ok(ToolResult::success(json!({
            "path": path,
            "content": content,
            "size": content.len()
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
}

/// Execute a bash command
pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command (with security restrictions)"
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for command execution"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let command = params["command"].as_str().ok_or_else(|| {
            amos_core::AmosError::Validation("command is required".to_string())
        })?.to_string();

        // Security: Block dangerous commands
        let blocked_patterns = [
            "rm -rf",
            "mkfs",
            "dd if=",
            ":(){ :|:& };:",
            "> /dev/sda",
            "wget",
            "curl",
            "nc ",
            "netcat",
        ];

        for pattern in &blocked_patterns {
            if command.contains(pattern) {
                return Ok(ToolResult::error(format!(
                    "Blocked: Command contains dangerous pattern: {}",
                    pattern
                )));
            }
        }

        // Execute command
        let output = tokio::task::spawn_blocking(move || {
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
        })
        .await
        .map_err(|e| amos_core::AmosError::Internal(format!("Task join error: {}", e)))?
        .map_err(|e| {
            amos_core::AmosError::Internal(format!("Command execution failed: {}", e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(ToolResult::success(json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": output.status.code(),
            "success": output.status.success()
        })))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
}
