//! Slash commands — built-in and user-defined commands.
//!
//! Built-in commands are hardcoded. User-defined commands are loaded from
//! `.amos/commands/*.md` files with YAML frontmatter.
//!
//! ## User-defined command format
//!
//! ```markdown
//! ---
//! name: deploy
//! description: Deploy the current project
//! allowed-tools: [harness_execute_code, write_file]
//! ---
//!
//! Deploy the project to production. Steps:
//! 1. Run tests
//! 2. Build release
//! 3. Push to registry
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A slash command specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    /// Whether this command accepts arguments.
    #[serde(default)]
    pub accepts_args: bool,
    /// Source: "builtin" or file path.
    pub source: String,
}

/// Parsed slash command from user input.
#[derive(Debug, Clone)]
pub enum SlashCommand {
    /// Built-in commands
    Help,
    Status,
    Compact,
    Memory,
    Model {
        model: Option<String>,
    },
    Permissions {
        mode: Option<String>,
    },
    Clear,
    Version,
    /// User-defined command from markdown file.
    Custom {
        name: String,
        prompt_template: String,
        allowed_tools: Option<Vec<String>>,
        args: Option<String>,
    },
    /// Unknown command.
    Unknown(String),
}

/// Frontmatter for user-defined commands.
#[derive(Debug, Deserialize)]
struct CommandFrontmatter {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "allowed-tools")]
    allowed_tools: Option<Vec<String>>,
}

/// Registry of available commands.
#[derive(Debug, Default)]
pub struct CommandRegistry {
    /// Built-in command specs.
    pub builtins: Vec<CommandSpec>,
    /// User-defined commands (name → (spec, prompt_template, allowed_tools)).
    pub custom: HashMap<String, (CommandSpec, String, Option<Vec<String>>)>,
}

impl CommandRegistry {
    /// Create a new registry with built-in commands and discover custom commands.
    pub fn new(project_dir: Option<&Path>) -> Self {
        let builtins = vec![
            CommandSpec {
                name: "help".to_string(),
                description: "Show available commands".to_string(),
                accepts_args: false,
                source: "builtin".to_string(),
            },
            CommandSpec {
                name: "status".to_string(),
                description: "Show agent status and connection info".to_string(),
                accepts_args: false,
                source: "builtin".to_string(),
            },
            CommandSpec {
                name: "compact".to_string(),
                description: "Manually compact conversation history".to_string(),
                accepts_args: false,
                source: "builtin".to_string(),
            },
            CommandSpec {
                name: "memory".to_string(),
                description: "Show stored memories".to_string(),
                accepts_args: false,
                source: "builtin".to_string(),
            },
            CommandSpec {
                name: "model".to_string(),
                description: "Switch model (e.g. /model claude-opus-4-6)".to_string(),
                accepts_args: true,
                source: "builtin".to_string(),
            },
            CommandSpec {
                name: "permissions".to_string(),
                description: "Show or change permission mode".to_string(),
                accepts_args: true,
                source: "builtin".to_string(),
            },
            CommandSpec {
                name: "clear".to_string(),
                description: "Clear conversation history".to_string(),
                accepts_args: false,
                source: "builtin".to_string(),
            },
            CommandSpec {
                name: "version".to_string(),
                description: "Show agent version".to_string(),
                accepts_args: false,
                source: "builtin".to_string(),
            },
        ];

        let mut custom = HashMap::new();

        // Discover custom commands from .amos/commands/
        if let Some(dir) = project_dir {
            discover_commands(dir, &mut custom);
        }

        // Also check ~/.amos/commands/ for user-global commands
        if let Some(home) = std::env::var("HOME").ok().map(PathBuf::from) {
            discover_commands(&home, &mut custom);
        }

        Self { builtins, custom }
    }

    /// Parse user input into a SlashCommand.
    pub fn parse(&self, input: &str) -> Option<SlashCommand> {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return None;
        }

        let without_slash = &trimmed[1..];
        let mut parts = without_slash.splitn(2, char::is_whitespace);
        let cmd_name = parts.next().unwrap_or_default().to_lowercase();
        let args = parts.next().map(|s| s.trim().to_string());

        match cmd_name.as_str() {
            "help" | "h" => Some(SlashCommand::Help),
            "status" | "s" => Some(SlashCommand::Status),
            "compact" => Some(SlashCommand::Compact),
            "memory" | "mem" => Some(SlashCommand::Memory),
            "model" | "m" => Some(SlashCommand::Model { model: args }),
            "permissions" | "perm" => Some(SlashCommand::Permissions { mode: args }),
            "clear" => Some(SlashCommand::Clear),
            "version" | "v" => Some(SlashCommand::Version),
            _ => {
                // Check custom commands
                if let Some((_, template, allowed_tools)) = self.custom.get(&cmd_name) {
                    Some(SlashCommand::Custom {
                        name: cmd_name,
                        prompt_template: template.clone(),
                        allowed_tools: allowed_tools.clone(),
                        args,
                    })
                } else {
                    Some(SlashCommand::Unknown(cmd_name))
                }
            }
        }
    }

    /// Get help text listing all available commands.
    pub fn help_text(&self) -> String {
        let mut lines = vec!["Available commands:".to_string(), String::new()];

        for cmd in &self.builtins {
            lines.push(format!("  /{:<14} {}", cmd.name, cmd.description));
        }

        if !self.custom.is_empty() {
            lines.push(String::new());
            lines.push("Custom commands:".to_string());
            for (name, (spec, _, _)) in &self.custom {
                lines.push(format!(
                    "  /{:<14} {} [{}]",
                    name, spec.description, spec.source
                ));
            }
        }

        lines.join("\n")
    }
}

/// Discover custom commands from a directory's .amos/commands/ folder.
fn discover_commands(
    base_dir: &Path,
    commands: &mut HashMap<String, (CommandSpec, String, Option<Vec<String>>)>,
) {
    let commands_dir = base_dir.join(".amos").join("commands");
    if !commands_dir.is_dir() {
        return;
    }

    let entries = match std::fs::read_dir(&commands_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Some((name, spec, template, allowed_tools)) = parse_command_file(&path) {
                commands.insert(name, (spec, template, allowed_tools));
            }
        }
    }
}

/// Parse a command markdown file with YAML frontmatter.
fn parse_command_file(path: &Path) -> Option<(String, CommandSpec, String, Option<Vec<String>>)> {
    let content = std::fs::read_to_string(path).ok()?;

    // Split frontmatter from body
    let (frontmatter_str, body) = parse_frontmatter(&content)?;

    let frontmatter: CommandFrontmatter = serde_yaml::from_str(frontmatter_str)
        .map_err(|e| {
            tracing::warn!(path = %path.display(), error = %e, "Failed to parse command frontmatter");
            e
        })
        .ok()?;

    let spec = CommandSpec {
        name: frontmatter.name.clone(),
        description: frontmatter
            .description
            .unwrap_or_else(|| format!("Custom command from {}", path.display())),
        accepts_args: body.contains("{{args}}") || body.contains("{args}"),
        source: path.display().to_string(),
    };

    Some((
        frontmatter.name,
        spec,
        body.to_string(),
        frontmatter.allowed_tools,
    ))
}

/// Parse YAML frontmatter delimited by `---`.
fn parse_frontmatter(content: &str) -> Option<(&str, &str)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_first = &trimmed[3..];
    let end_pos = after_first.find("---")?;
    let frontmatter = after_first[..end_pos].trim();
    let body = after_first[end_pos + 3..].trim();

    Some((frontmatter, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_builtin_commands() {
        let reg = CommandRegistry::new(None);
        assert!(matches!(reg.parse("/help"), Some(SlashCommand::Help)));
        assert!(matches!(reg.parse("/status"), Some(SlashCommand::Status)));
        assert!(matches!(reg.parse("/compact"), Some(SlashCommand::Compact)));
        assert!(matches!(
            reg.parse("/model claude-opus-4-6"),
            Some(SlashCommand::Model {
                model: Some(ref m)
            }) if m == "claude-opus-4-6"
        ));
        assert!(matches!(
            reg.parse("/unknown"),
            Some(SlashCommand::Unknown(ref s)) if s == "unknown"
        ));
    }

    #[test]
    fn test_not_a_command() {
        let reg = CommandRegistry::new(None);
        assert!(reg.parse("hello").is_none());
        assert!(reg.parse("").is_none());
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = "---\nname: test\ndescription: A test\n---\nDo the thing.";
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert!(fm.contains("name: test"));
        assert_eq!(body, "Do the thing.");
    }

    #[test]
    fn test_help_text() {
        let reg = CommandRegistry::new(None);
        let help = reg.help_text();
        assert!(help.contains("/help"));
        assert!(help.contains("/model"));
        assert!(help.contains("/compact"));
    }
}
