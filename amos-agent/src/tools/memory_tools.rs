//! Memory tools - remember and recall from persistent local memory.

use crate::memory::MemoryStore;
use amos_core::types::ToolDefinition;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

pub fn remember_definition() -> ToolDefinition {
    ToolDefinition {
        name: "remember".to_string(),
        description: "Store a fact, insight, or piece of information to persistent memory. \
            Use this to remember things across conversations: user preferences, project context, \
            key decisions, important facts. Each memory has a unique key - storing with an \
            existing key updates the memory."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "Unique key for this memory (e.g. 'user_name', 'project_tech_stack')"
                },
                "content": {
                    "type": "string",
                    "description": "The information to remember"
                },
                "tags": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Optional tags for categorization (e.g. ['project', 'preference'])"
                }
            },
            "required": ["key", "content"]
        }),
        requires_confirmation: false,
    }
}

pub fn recall_definition() -> ToolDefinition {
    ToolDefinition {
        name: "recall".to_string(),
        description: "Search persistent memory for relevant information. \
            Use this to look up previously stored facts, preferences, or context. \
            Supports full-text search across all stored memories."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query to find relevant memories"
                },
                "key": {
                    "type": "string",
                    "description": "Optional: exact key to retrieve a specific memory"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results (default: 5)"
                }
            },
            "required": ["query"]
        }),
        requires_confirmation: false,
    }
}

/// Store a memory.
pub async fn remember(input: &serde_json::Value, store: &Arc<Mutex<MemoryStore>>) -> Result<String, String> {
    let key = input["key"]
        .as_str()
        .ok_or("Missing required field: key")?;
    let content = input["content"]
        .as_str()
        .ok_or("Missing required field: content")?;
    let tags: Vec<String> = input
        .get("tags")
        .and_then(|t| serde_json::from_value(t.clone()).ok())
        .unwrap_or_default();

    let store = store.lock().await;
    let mem = store.remember(key, content, &tags).map_err(|e| e.to_string())?;
    Ok(format!(
        "Stored memory '{}': {} ({} chars)",
        mem.key,
        &mem.content[..mem.content.len().min(100)],
        mem.content.len()
    ))
}

/// Search/retrieve memories.
pub async fn recall(input: &serde_json::Value, store: &Arc<Mutex<MemoryStore>>) -> Result<String, String> {
    let store = store.lock().await;

    // If an exact key is provided, try direct lookup first
    if let Some(key) = input.get("key").and_then(|k| k.as_str()) {
        match store.get(key) {
            Ok(mem) => {
                return Ok(json!({
                    "found": true,
                    "memory": {
                        "key": mem.key,
                        "content": mem.content,
                        "tags": mem.tags,
                        "updated_at": mem.updated_at,
                    }
                })
                .to_string());
            }
            Err(_) => {} // Fall through to search
        }
    }

    let query = input["query"]
        .as_str()
        .ok_or("Missing required field: query")?;
    let limit = input
        .get("limit")
        .and_then(|l| l.as_u64())
        .unwrap_or(5) as usize;

    let results = store.search(query, limit).map_err(|e| e.to_string())?;

    if results.is_empty() {
        // Try listing recent as fallback
        let recent = store.list_recent(limit).map_err(|e| e.to_string())?;
        if recent.is_empty() {
            return Ok(json!({"found": false, "message": "No memories stored yet."}).to_string());
        }
        return Ok(json!({
            "found": false,
            "message": format!("No matches for '{}'. Here are recent memories:", query),
            "recent": recent.iter().map(|m| json!({
                "key": m.key,
                "content": &m.content[..m.content.len().min(200)],
                "tags": m.tags,
            })).collect::<Vec<_>>(),
        })
        .to_string());
    }

    Ok(json!({
        "found": true,
        "count": results.len(),
        "memories": results.iter().map(|m| json!({
            "key": m.key,
            "content": m.content,
            "tags": m.tags,
            "updated_at": m.updated_at,
        })).collect::<Vec<_>>(),
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> Arc<Mutex<MemoryStore>> {
        Arc::new(Mutex::new(MemoryStore::in_memory().unwrap()))
    }

    #[tokio::test]
    async fn test_remember_tool() {
        let store = test_store();
        let input = json!({
            "key": "test_fact",
            "content": "The sky is blue",
            "tags": ["fact", "nature"]
        });
        let result = remember(&input, &store).await.unwrap();
        assert!(result.contains("test_fact"));
        assert!(result.contains("The sky is blue"));
    }

    #[tokio::test]
    async fn test_recall_by_key() {
        let store = test_store();
        {
            let s = store.lock().await;
            s.remember("user_name", "Alice", &[]).unwrap();
        }

        let input = json!({"query": "name", "key": "user_name"});
        let result = recall(&input, &store).await.unwrap();
        assert!(result.contains("Alice"));
    }

    #[tokio::test]
    async fn test_recall_by_search() {
        let store = test_store();
        {
            let s = store.lock().await;
            s.remember("project_lang", "Rust is the primary language", &["tech".to_string()])
                .unwrap();
        }

        let input = json!({"query": "language"});
        let result = recall(&input, &store).await.unwrap();
        assert!(result.contains("Rust"));
    }

    #[tokio::test]
    async fn test_recall_empty() {
        let store = test_store();
        let input = json!({"query": "anything"});
        let result = recall(&input, &store).await.unwrap();
        assert!(result.contains("No memories"));
    }
}
