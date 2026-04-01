//! Auto-compaction for conversation context management.
//!
//! When the conversation approaches the token limit, older messages are
//! summarized into a compact form, preserving recent context while staying
//! within budget.
//!
//! ## Strategy
//!
//! 1. Estimate token count for all messages.
//! 2. If over threshold (default: 80% of max context), compact.
//! 3. Keep the most recent N messages intact.
//! 4. Summarize older messages (extract tools used, topics, key files).
//! 5. Replace older messages with a single summary message.

use amos_core::types::{ContentBlock, Message, Role};
use chrono::Utc;

/// Configuration for auto-compaction.
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Number of recent messages to always preserve.
    pub preserve_recent: usize,
    /// Maximum estimated tokens before triggering compaction.
    pub max_tokens: usize,
    /// Threshold ratio (0.0-1.0) at which to trigger compaction.
    pub threshold: f64,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            preserve_recent: 6,
            max_tokens: 200_000,
            threshold: 0.80,
        }
    }
}

/// Result of a compaction operation.
#[derive(Debug)]
pub struct CompactionResult {
    /// The compacted message list.
    pub messages: Vec<Message>,
    /// Number of messages that were removed/summarized.
    pub removed_count: usize,
    /// Estimated tokens after compaction.
    pub estimated_tokens: usize,
    /// The summary text that was generated.
    pub summary: String,
}

/// Estimate token count for a message list.
///
/// Uses a simple heuristic: ~4 chars per token, plus overhead for message
/// structure. This is intentionally conservative.
pub fn estimate_tokens(messages: &[Message]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

fn estimate_message_tokens(msg: &Message) -> usize {
    let content_chars: usize = msg
        .content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => text.len(),
            ContentBlock::ToolUse { name, input, .. } => {
                name.len() + serde_json::to_string(input).map(|s| s.len()).unwrap_or(100)
            }
            ContentBlock::ToolResult { content, .. } => content.len(),
            ContentBlock::Image { .. } => 1000, // rough estimate for image tokens
            ContentBlock::Document { .. } => 2000,
        })
        .sum();

    // ~4 chars per token + message overhead (~4 tokens)
    content_chars / 4 + 4
}

/// Check if compaction is needed and perform it if so.
///
/// Returns `Some(CompactionResult)` if compaction was performed, `None` if not needed.
pub fn maybe_compact(messages: &[Message], config: &CompactionConfig) -> Option<CompactionResult> {
    let current_tokens = estimate_tokens(messages);
    let threshold = (config.max_tokens as f64 * config.threshold) as usize;

    if current_tokens < threshold {
        return None;
    }

    tracing::info!(
        current_tokens,
        threshold,
        message_count = messages.len(),
        "Compacting conversation"
    );

    Some(compact(messages, config))
}

/// Perform compaction on the message list.
fn compact(messages: &[Message], config: &CompactionConfig) -> CompactionResult {
    if messages.len() <= config.preserve_recent + 1 {
        // Not enough messages to compact
        return CompactionResult {
            messages: messages.to_vec(),
            removed_count: 0,
            estimated_tokens: estimate_tokens(messages),
            summary: String::new(),
        };
    }

    // Check for existing compaction summary (continuation)
    let has_existing_summary = messages
        .first()
        .map(|m| {
            m.content.iter().any(|b| match b {
                ContentBlock::Text { text } => text.contains("[Conversation Summary"),
                _ => false,
            })
        })
        .unwrap_or(false);

    let split_point = messages.len().saturating_sub(config.preserve_recent);
    let older = &messages[..split_point];
    let recent = &messages[split_point..];

    let summary = build_summary(older, has_existing_summary);

    let mut compacted = Vec::with_capacity(1 + recent.len());

    // Insert summary as first message
    compacted.push(Message {
        role: Role::User,
        content: vec![ContentBlock::Text {
            text: format!("[Conversation Summary (auto-compacted)]\n\n{summary}"),
        }],
        tool_use_id: None,
        timestamp: Utc::now(),
    });

    // Keep recent messages as-is
    compacted.extend_from_slice(recent);

    let estimated_tokens = estimate_tokens(&compacted);

    CompactionResult {
        messages: compacted,
        removed_count: older.len(),
        estimated_tokens,
        summary,
    }
}

/// Build a summary of older messages.
fn build_summary(messages: &[Message], has_existing_summary: bool) -> String {
    let mut user_requests: Vec<String> = Vec::new();
    let mut tools_used: Vec<String> = Vec::new();
    let mut key_files: Vec<String> = Vec::new();
    let mut existing_summary = String::new();

    // Track message counts by role
    let mut user_count = 0;
    let mut assistant_count = 0;
    let mut tool_count = 0;

    for msg in messages {
        match msg.role {
            Role::User => user_count += 1,
            Role::Assistant => assistant_count += 1,
            _ => {}
        }

        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    // Extract existing summary if present
                    if has_existing_summary && text.contains("[Conversation Summary") {
                        existing_summary = text.clone();
                        continue;
                    }

                    if msg.role == Role::User && !text.starts_with('[') {
                        // Capture user request (truncated)
                        let truncated = if text.len() > 120 {
                            format!("{}...", &text[..120])
                        } else {
                            text.clone()
                        };
                        user_requests.push(truncated);
                    }

                    // Extract file references
                    extract_file_refs(text, &mut key_files);
                }
                ContentBlock::ToolUse { name, .. } => {
                    tool_count += 1;
                    if !tools_used.contains(name) {
                        tools_used.push(name.clone());
                    }
                }
                ContentBlock::ToolResult { .. } => {
                    tool_count += 1;
                }
                _ => {}
            }
        }
    }

    let mut parts: Vec<String> = Vec::new();

    // Include existing summary if present
    if !existing_summary.is_empty() {
        parts.push(format!("Previous summary:\n{existing_summary}"));
    }

    // Scope
    parts.push(format!(
        "Scope: {user_count} user messages, {assistant_count} assistant messages, {tool_count} tool interactions"
    ));

    // User requests (last 5)
    if !user_requests.is_empty() {
        let recent_requests: Vec<_> = user_requests.iter().rev().take(5).rev().collect();
        parts.push(format!(
            "User requests:\n{}",
            recent_requests
                .iter()
                .map(|r| format!("  - {r}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    // Tools used
    if !tools_used.is_empty() {
        parts.push(format!("Tools used: {}", tools_used.join(", ")));
    }

    // Key files
    if !key_files.is_empty() {
        key_files.sort();
        key_files.dedup();
        let display_files: Vec<_> = key_files.iter().take(20).collect();
        parts.push(format!(
            "Key files referenced: {}",
            display_files
                .iter()
                .map(|f| f.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    parts.join("\n\n")
}

/// Extract file path references from text.
fn extract_file_refs(text: &str, files: &mut Vec<String>) {
    // Simple heuristic: look for path-like strings
    for word in text.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| c == '\'' || c == '"' || c == '`' || c == ',');
        if looks_like_file_path(cleaned) && !files.contains(&cleaned.to_string()) {
            files.push(cleaned.to_string());
        }
    }
}

fn looks_like_file_path(s: &str) -> bool {
    if s.len() < 3 || s.len() > 200 {
        return false;
    }
    let extensions = [
        ".rs", ".ts", ".tsx", ".js", ".jsx", ".json", ".toml", ".yaml", ".yml", ".md", ".sql",
        ".html", ".css", ".py", ".go", ".java",
    ];
    extensions.iter().any(|ext| s.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(role: Role, text: &str) -> Message {
        Message {
            role,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            tool_use_id: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_estimate_tokens() {
        let msgs = vec![
            make_msg(Role::User, "Hello, how are you?"),
            make_msg(Role::Assistant, "I'm doing well, thanks!"),
        ];
        let tokens = estimate_tokens(&msgs);
        assert!(tokens > 0);
        assert!(tokens < 100); // should be a small number
    }

    #[test]
    fn test_no_compaction_needed() {
        let msgs = vec![
            make_msg(Role::User, "Hello"),
            make_msg(Role::Assistant, "Hi"),
        ];
        let config = CompactionConfig::default();
        assert!(maybe_compact(&msgs, &config).is_none());
    }

    #[test]
    fn test_compaction_preserves_recent() {
        // Create enough messages to trigger compaction
        let config = CompactionConfig {
            preserve_recent: 2,
            max_tokens: 100, // very low to trigger
            threshold: 0.5,
        };

        let mut msgs = Vec::new();
        for i in 0..20 {
            msgs.push(make_msg(
                if i % 2 == 0 { Role::User } else { Role::Assistant },
                &format!("Message number {i} with enough content to push token count up. This is a long message to ensure we exceed the threshold for testing purposes."),
            ));
        }

        let result = maybe_compact(&msgs, &config);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.removed_count > 0);
        // Summary + preserved recent messages
        assert!(result.messages.len() <= config.preserve_recent + 1);
    }

    #[test]
    fn test_file_ref_extraction() {
        let mut files = Vec::new();
        extract_file_refs("Check src/main.rs and config.toml please", &mut files);
        assert!(files.contains(&"src/main.rs".to_string()));
        assert!(files.contains(&"config.toml".to_string()));
    }
}
