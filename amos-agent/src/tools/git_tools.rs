//! Git-aware workspace context.
//!
//! Detects git repositories and provides branch, status, recent commits,
//! and diff information as workspace context.

use amos_core::types::ToolDefinition;
use serde_json::json;
use std::path::Path;
use std::process::Command;

pub fn git_status_definition() -> ToolDefinition {
    ToolDefinition {
        name: "git_status".to_string(),
        description:
            "Get git repository status including branch, uncommitted changes, and recent commits. \
            Use this to understand the current state of the workspace's version control."
                .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "include_diff": {
                    "type": "boolean",
                    "description": "Include diff of uncommitted changes (default: false)"
                },
                "commit_count": {
                    "type": "integer",
                    "description": "Number of recent commits to show (default: 5)"
                }
            }
        }),
        requires_confirmation: false,
        permission_level: amos_core::permissions::PermissionLevel::ReadOnly,
    }
}

/// Execute git_status tool.
pub fn execute(input: &serde_json::Value, work_dir: &str) -> Result<String, String> {
    let include_diff = input
        .get("include_diff")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let commit_count = input
        .get("commit_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;

    let dir = Path::new(work_dir);
    if !dir.join(".git").exists() {
        return Ok(json!({"is_git_repo": false, "message": "Not a git repository"}).to_string());
    }

    let mut result = serde_json::Map::new();
    result.insert("is_git_repo".to_string(), json!(true));

    // Current branch
    if let Ok(branch) = run_git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        result.insert("branch".to_string(), json!(branch.trim()));
    }

    // Status (porcelain for parsing)
    if let Ok(status) = run_git(dir, &["status", "--porcelain"]) {
        let changes: Vec<&str> = status.lines().filter(|l| !l.is_empty()).collect();
        result.insert("uncommitted_changes".to_string(), json!(changes.len()));
        if !changes.is_empty() {
            // Group by status
            let mut modified = Vec::new();
            let mut added = Vec::new();
            let mut deleted = Vec::new();
            let mut untracked = Vec::new();

            for line in &changes {
                if line.len() < 3 {
                    continue;
                }
                let status_code = &line[..2];
                let file = line[3..].trim();
                match status_code.trim() {
                    "M" | "MM" => modified.push(file),
                    "A" | "AM" => added.push(file),
                    "D" => deleted.push(file),
                    "??" => untracked.push(file),
                    _ => modified.push(file), // default bucket
                }
            }

            if !modified.is_empty() {
                result.insert("modified_files".to_string(), json!(modified));
            }
            if !added.is_empty() {
                result.insert("added_files".to_string(), json!(added));
            }
            if !deleted.is_empty() {
                result.insert("deleted_files".to_string(), json!(deleted));
            }
            if !untracked.is_empty() {
                result.insert(
                    "untracked_files".to_string(),
                    json!(untracked.iter().take(20).collect::<Vec<_>>()),
                );
            }
        }
    }

    // Recent commits
    let log_format = "--pretty=format:%h|%s|%an|%ar";
    if let Ok(log) = run_git(dir, &["log", log_format, &format!("-{}", commit_count)]) {
        let commits: Vec<serde_json::Value> = log
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(4, '|').collect();
                if parts.len() == 4 {
                    Some(json!({
                        "hash": parts[0],
                        "message": parts[1],
                        "author": parts[2],
                        "time_ago": parts[3],
                    }))
                } else {
                    None
                }
            })
            .collect();
        if !commits.is_empty() {
            result.insert("recent_commits".to_string(), json!(commits));
        }
    }

    // Diff (optional)
    if include_diff {
        if let Ok(diff) = run_git(dir, &["diff", "--stat"]) {
            if !diff.trim().is_empty() {
                result.insert("diff_stat".to_string(), json!(diff.trim()));
            }
        }
        // Also include short diff content (limited)
        if let Ok(diff) = run_git(dir, &["diff", "--no-color"]) {
            let truncated = if diff.len() > 5000 {
                format!("{}...\n[diff truncated at 5000 chars]", &diff[..5000])
            } else {
                diff
            };
            if !truncated.trim().is_empty() {
                result.insert("diff".to_string(), json!(truncated.trim()));
            }
        }
    }

    Ok(serde_json::Value::Object(result).to_string())
}

/// Collect git context for workspace summary injection.
/// Returns a JSON value with branch, status, and recent commits.
pub fn collect_git_context(work_dir: &str) -> Option<serde_json::Value> {
    let dir = Path::new(work_dir);
    if !dir.join(".git").exists() {
        return None;
    }

    let mut ctx = serde_json::Map::new();

    if let Ok(branch) = run_git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        ctx.insert("branch".to_string(), json!(branch.trim()));
    }

    if let Ok(status) = run_git(dir, &["status", "--porcelain"]) {
        let count = status.lines().filter(|l| !l.is_empty()).count();
        ctx.insert("uncommitted_changes".to_string(), json!(count));
    }

    if let Ok(log) = run_git(dir, &["log", "--pretty=format:%h %s", "-3"]) {
        let commits: Vec<&str> = log.lines().collect();
        if !commits.is_empty() {
            ctx.insert("recent_commits".to_string(), json!(commits));
        }
    }

    if ctx.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(ctx))
    }
}

fn run_git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("git error: {stderr}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_context_non_repo() {
        // /tmp is not a git repo
        assert!(collect_git_context("/tmp").is_none());
    }

    #[test]
    fn test_git_status_non_repo() {
        let result = execute(&json!({}), "/tmp").unwrap();
        assert!(result.contains("Not a git repository"));
    }
}
