//! Plan tool - create and update structured task plans.
//!
//! The agent uses this to break down complex tasks into steps,
//! track progress, and reason about next actions.

use amos_core::types::ToolDefinition;
use serde_json::json;

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "plan".to_string(),
        description: "Create or update a structured plan for the current task. \
            Use this to break down complex work into clear steps, track progress, \
            and reason about what to do next. Plans help you stay organized and \
            ensure nothing is missed."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title of the plan"
                },
                "steps": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "description": {"type": "string"},
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed", "skipped"]
                            }
                        },
                        "required": ["description", "status"]
                    },
                    "description": "Ordered list of steps with their status"
                },
                "notes": {
                    "type": "string",
                    "description": "Additional context or notes about the plan"
                }
            },
            "required": ["title", "steps"]
        }),
        requires_confirmation: false,
    }
}

/// Execute the plan tool - formats and returns the plan as confirmation.
pub fn execute(input: &serde_json::Value) -> String {
    let title = input["title"].as_str().unwrap_or("Untitled Plan");
    let steps = input["steps"].as_array();
    let notes = input["notes"].as_str();

    let mut output = format!("Plan: {title}\n");
    output.push_str(&"=".repeat(title.len() + 6));
    output.push('\n');

    if let Some(steps) = steps {
        for (i, step) in steps.iter().enumerate() {
            let desc = step["description"].as_str().unwrap_or("???");
            let status = step["status"].as_str().unwrap_or("pending");
            let icon = match status {
                "completed" => "[x]",
                "in_progress" => "[>]",
                "skipped" => "[-]",
                _ => "[ ]",
            };
            output.push_str(&format!("  {icon} {}. {desc}\n", i + 1));
        }

        let completed = steps
            .iter()
            .filter(|s| s["status"].as_str() == Some("completed"))
            .count();
        output.push_str(&format!(
            "\nProgress: {completed}/{} steps completed\n",
            steps.len()
        ));
    }

    if let Some(notes) = notes {
        output.push_str(&format!("\nNotes: {notes}\n"));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_execute() {
        let input = json!({
            "title": "Build agent crate",
            "steps": [
                {"description": "Create Cargo.toml", "status": "completed"},
                {"description": "Implement tools", "status": "in_progress"},
                {"description": "Write tests", "status": "pending"},
            ],
            "notes": "Using SQLite for memory"
        });

        let result = execute(&input);
        assert!(result.contains("Build agent crate"));
        assert!(result.contains("[x] 1. Create Cargo.toml"));
        assert!(result.contains("[>] 2. Implement tools"));
        assert!(result.contains("[ ] 3. Write tests"));
        assert!(result.contains("1/3 steps completed"));
        assert!(result.contains("Using SQLite"));
    }
}
