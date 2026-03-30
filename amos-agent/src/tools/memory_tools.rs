//! Memory tools - remember and recall from persistent local memory.
//!
//! The `remember` tool writes through to both local SQLite (fast, in-process)
//! and the harness PostgreSQL (persistent across container restarts) when a
//! harness client is available. The `recall` tool searches locally first and
//! falls back to harness search when local results are sparse.

use crate::harness_client::HarnessClient;
use crate::memory::MemoryStore;
use amos_core::types::ToolDefinition;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

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

/// Store a memory (write-through: local SQLite + harness PostgreSQL).
pub async fn remember(
    input: &serde_json::Value,
    store: &Arc<Mutex<MemoryStore>>,
    harness: Option<&Arc<RwLock<HarnessClient>>>,
) -> Result<String, String> {
    let key = input["key"].as_str().ok_or("Missing required field: key")?;
    let content = input["content"]
        .as_str()
        .ok_or("Missing required field: content")?;
    let tags: Vec<String> = input
        .get("tags")
        .and_then(|t| serde_json::from_value(t.clone()).ok())
        .unwrap_or_default();

    // Write to local SQLite (fast, in-process)
    let mem = {
        let store = store.lock().await;
        store
            .remember(key, content, &tags)
            .map_err(|e| e.to_string())?
    };

    // Fire-and-forget write-through to harness PostgreSQL for persistence
    if let Some(harness) = harness {
        let h = harness.clone();
        let harness_input = json!({
            "content": content,
            "category": "agent_memory",
            "key": key,
            "tags": tags,
        });
        tokio::spawn(async move {
            let h = h.read().await;
            if let Err(e) = h.execute_tool("remember_this", harness_input, None).await {
                tracing::debug!("Harness memory write-through failed (non-fatal): {e}");
            }
        });
    }

    Ok(format!(
        "Stored memory '{}': {} ({} chars)",
        mem.key,
        &mem.content[..mem.content.len().min(100)],
        mem.content.len()
    ))
}

/// Search/retrieve memories (local-first with harness fallback).
pub async fn recall(
    input: &serde_json::Value,
    store: &Arc<Mutex<MemoryStore>>,
    harness: Option<&Arc<RwLock<HarnessClient>>>,
) -> Result<String, String> {
    let store_guard = store.lock().await;

    // If an exact key is provided, try direct lookup first
    if let Some(key) = input.get("key").and_then(|k| k.as_str()) {
        if let Ok(mem) = store_guard.get(key) {
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
    }

    let query = input["query"]
        .as_str()
        .ok_or("Missing required field: query")?;
    let limit = input.get("limit").and_then(|l| l.as_u64()).unwrap_or(5) as usize;

    let local_results = store_guard
        .search(query, limit)
        .map_err(|e| e.to_string())?;
    drop(store_guard); // release lock before potential harness call

    // If local results are sparse, also search harness for persistent memories
    let mut all_results: Vec<serde_json::Value> = local_results
        .iter()
        .map(|m| {
            json!({
                "key": m.key,
                "content": m.content,
                "tags": m.tags,
                "updated_at": m.updated_at,
                "source": "local",
            })
        })
        .collect();

    if local_results.len() < 2 {
        if let Some(harness) = harness {
            let h = harness.read().await;
            let harness_input = json!({
                "query": query,
                "limit": limit,
                "category": "agent_memory",
            });
            if let Ok(resp) = h.execute_tool("search_memory", harness_input, None).await {
                if !resp.is_error {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&resp.content) {
                        if let Some(results) = data.get("results").and_then(|r| r.as_array()) {
                            // Merge harness results, deduplicating by content
                            let local_contents: std::collections::HashSet<String> =
                                local_results.iter().map(|m| m.content.clone()).collect();
                            for r in results {
                                let content = r
                                    .get("content")
                                    .and_then(|c| c.as_str())
                                    .unwrap_or_default();
                                if !local_contents.contains(content) {
                                    all_results.push(json!({
                                        "content": content,
                                        "category": r.get("category").and_then(|c| c.as_str()).unwrap_or(""),
                                        "source": "harness",
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if all_results.is_empty() {
        // Try listing recent as fallback
        let store_guard = store.lock().await;
        let recent = store_guard.list_recent(limit).map_err(|e| e.to_string())?;
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
        "count": all_results.len(),
        "memories": all_results,
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
        let result = remember(&input, &store, None).await.unwrap();
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
        let result = recall(&input, &store, None).await.unwrap();
        assert!(result.contains("Alice"));
    }

    #[tokio::test]
    async fn test_recall_by_search() {
        let store = test_store();
        {
            let s = store.lock().await;
            s.remember(
                "project_lang",
                "Rust is the primary language",
                &["tech".to_string()],
            )
            .unwrap();
        }

        let input = json!({"query": "language"});
        let result = recall(&input, &store, None).await.unwrap();
        assert!(result.contains("Rust"));
    }

    #[tokio::test]
    async fn test_recall_empty() {
        let store = test_store();
        let input = json!({"query": "anything"});
        let result = recall(&input, &store, None).await.unwrap();
        assert!(result.contains("No memories"));
    }
}
