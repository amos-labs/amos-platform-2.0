//! File I/O tools - read and write local files.
//!
//! These tools give the agent the ability to work with the local filesystem,
//! constrained to the configured working directory for security.

use amos_core::types::ToolDefinition;
use serde_json::json;
use std::path::{Path, PathBuf};

pub fn read_file_definition() -> ToolDefinition {
    ToolDefinition {
        name: "read_file".to_string(),
        description: "Read the contents of a local file. The path is relative to the \
            agent's working directory."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file (relative to working directory)"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (0-indexed, default: 0)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read (default: 500)"
                }
            },
            "required": ["path"]
        }),
        requires_confirmation: false,
    }
}

pub fn write_file_definition() -> ToolDefinition {
    ToolDefinition {
        name: "write_file".to_string(),
        description: "Write content to a local file. Creates the file if it doesn't exist, \
            or overwrites it if it does. The path is relative to the agent's working directory."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file (relative to working directory)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        }),
        requires_confirmation: true,
    }
}

/// Resolve and validate a path against the working directory.
/// Prevents path traversal attacks by normalizing `..` and `.` components
/// without requiring the target to exist on the filesystem.
fn resolve_path(rel_path: &str, work_dir: &str) -> Result<PathBuf, String> {
    let work = Path::new(work_dir)
        .canonicalize()
        .map_err(|e| format!("Invalid working directory '{}': {}", work_dir, e))?;

    let target = work.join(rel_path);

    // Normalize the path by resolving `.` and `..` components lexically
    let normalized = normalize_path(&target);

    // Security: ensure the normalized path is within the working directory
    if !normalized.starts_with(&work) {
        return Err(format!(
            "Path traversal denied: '{}' is outside the working directory",
            rel_path
        ));
    }

    Ok(normalized)
}

/// Normalize a path by resolving `.` and `..` components without touching the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            c => components.push(c),
        }
    }
    components.iter().collect()
}

/// Read a local file.
pub fn read_file(input: &serde_json::Value, work_dir: &str) -> Result<String, String> {
    let rel_path = input["path"]
        .as_str()
        .ok_or("Missing required field: path")?;
    let offset = input.get("offset").and_then(|o| o.as_u64()).unwrap_or(0) as usize;
    let limit = input.get("limit").and_then(|l| l.as_u64()).unwrap_or(500) as usize;

    let full_path = resolve_path(rel_path, work_dir)?;

    let content =
        std::fs::read_to_string(&full_path).map_err(|e| format!("Failed to read file: {e}"))?;

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    let selected: Vec<String> = lines
        .into_iter()
        .skip(offset)
        .take(limit)
        .enumerate()
        .map(|(i, line)| format!("{:>5} | {}", offset + i + 1, line))
        .collect();

    Ok(json!({
        "path": rel_path,
        "total_lines": total,
        "offset": offset,
        "lines_returned": selected.len(),
        "content": selected.join("\n"),
    })
    .to_string())
}

/// Write content to a local file.
pub fn write_file(input: &serde_json::Value, work_dir: &str) -> Result<String, String> {
    let rel_path = input["path"]
        .as_str()
        .ok_or("Missing required field: path")?;
    let content = input["content"]
        .as_str()
        .ok_or("Missing required field: content")?;

    let full_path = resolve_path(rel_path, work_dir)?;

    // Create parent directories if needed
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {e}"))?;
    }

    std::fs::write(&full_path, content).map_err(|e| format!("Failed to write file: {e}"))?;

    Ok(format!("Written {} bytes to {}", content.len(), rel_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_read_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line 1\nline 2\nline 3\n").unwrap();

        let input = json!({"path": "test.txt"});
        let result = read_file(&input, dir.path().to_str().unwrap()).unwrap();
        assert!(result.contains("line 1"));
        assert!(result.contains("line 2"));
        assert!(result.contains("total_lines"));
    }

    #[test]
    fn test_read_file_with_offset() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "a\nb\nc\nd\ne\n").unwrap();

        let input = json!({"path": "test.txt", "offset": 2, "limit": 2});
        let result = read_file(&input, dir.path().to_str().unwrap()).unwrap();
        assert!(result.contains("\"lines_returned\":2"));
    }

    #[test]
    fn test_write_file() {
        let dir = TempDir::new().unwrap();
        let input = json!({"path": "output.txt", "content": "hello world"});
        let result = write_file(&input, dir.path().to_str().unwrap()).unwrap();
        assert!(result.contains("11 bytes"));

        let content = std::fs::read_to_string(dir.path().join("output.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_path_traversal_blocked() {
        let dir = TempDir::new().unwrap();
        let input = json!({"path": "../../../etc/passwd"});
        let result = read_file(&input, dir.path().to_str().unwrap());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("traversal") || err.contains("outside"));
    }

    #[test]
    fn test_write_creates_subdirectories() {
        let dir = TempDir::new().unwrap();
        let input = json!({"path": "sub/dir/file.txt", "content": "nested!"});
        let result = write_file(&input, dir.path().to_str().unwrap()).unwrap();
        assert!(result.contains("7 bytes"));
        assert!(dir.path().join("sub/dir/file.txt").exists());
    }
}
