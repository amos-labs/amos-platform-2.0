//! Think tool - internal reasoning with no side effects.
//!
//! This tool allows the agent to "think out loud" without producing any
//! external actions. It's useful for chain-of-thought reasoning, planning
//! next steps, and reflecting on observations.

use amos_core::types::ToolDefinition;
use serde_json::json;

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "think".to_string(),
        description: "Use this tool for internal reasoning and chain-of-thought. \
            Think through a problem step by step before acting. \
            This produces no side effects - it's purely for your reasoning process."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "thought": {
                    "type": "string",
                    "description": "Your internal reasoning or chain-of-thought"
                }
            },
            "required": ["thought"]
        }),
        requires_confirmation: false,
        permission_level: amos_core::permissions::PermissionLevel::ReadOnly,
    }
}

/// Execute the think tool. Returns the thought as acknowledgment.
pub fn execute(input: &serde_json::Value) -> String {
    let thought = input["thought"].as_str().unwrap_or("(empty thought)");
    format!("[Thought recorded: {} chars]", thought.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_think_definition() {
        let def = definition();
        assert_eq!(def.name, "think");
        assert!(!def.requires_confirmation);
    }

    #[test]
    fn test_think_execute() {
        let input = json!({"thought": "I need to analyze the user's request first."});
        let result = execute(&input);
        assert!(result.contains("Thought recorded"), "got: {result}");
        assert!(result.contains("chars"), "got: {result}");
    }
}
