//! Persistent local memory store backed by SQLite.
//!
//! The agent uses this for cross-session recall. Memories are key-value pairs
//! with optional tags and full-text search via SQLite FTS5.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub key: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// SQLite-backed memory store.
pub struct MemoryStore {
    conn: Connection,
}

impl MemoryStore {
    /// Open or create a memory database at the given path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Create an in-memory store (for testing).
    pub fn in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                key TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                tags TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key);

            CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                key, content, tags, content=memories, content_rowid=id
            );

            CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, key, content, tags)
                VALUES (new.id, new.key, new.content, new.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, key, content, tags)
                VALUES ('delete', old.id, old.key, old.content, old.tags);
                INSERT INTO memories_fts(rowid, key, content, tags)
                VALUES (new.id, new.key, new.content, new.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, key, content, tags)
                VALUES ('delete', old.id, old.key, old.content, old.tags);
            END;"
        )?;
        Ok(())
    }

    /// Store a memory (upsert by key).
    pub fn remember(&self, key: &str, content: &str, tags: &[String]) -> Result<Memory, rusqlite::Error> {
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());

        self.conn.execute(
            "INSERT INTO memories (key, content, tags, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET
                content = excluded.content,
                tags = excluded.tags,
                updated_at = datetime('now')",
            params![key, content, tags_json],
        )?;

        self.get(key)
    }

    /// Retrieve a memory by exact key.
    pub fn get(&self, key: &str) -> Result<Memory, rusqlite::Error> {
        self.conn.query_row(
            "SELECT id, key, content, tags, created_at, updated_at FROM memories WHERE key = ?1",
            params![key],
            |row| {
                let tags_str: String = row.get(3)?;
                let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                Ok(Memory {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    content: row.get(2)?,
                    tags,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )
    }

    /// Full-text search across all memories.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.key, m.content, m.tags, m.created_at, m.updated_at
             FROM memories_fts f
             JOIN memories m ON f.rowid = m.id
             WHERE memories_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        )?;

        let memories = stmt.query_map(params![query, limit as i64], |row| {
            let tags_str: String = row.get(3)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            Ok(Memory {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                tags,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        memories.collect()
    }

    /// List recent memories.
    pub fn list_recent(&self, limit: usize) -> Result<Vec<Memory>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, key, content, tags, created_at, updated_at
             FROM memories ORDER BY updated_at DESC LIMIT ?1"
        )?;

        let memories = stmt.query_map(params![limit as i64], |row| {
            let tags_str: String = row.get(3)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            Ok(Memory {
                id: row.get(0)?,
                key: row.get(1)?,
                content: row.get(2)?,
                tags,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        memories.collect()
    }

    /// Delete a memory by key.
    pub fn forget(&self, key: &str) -> Result<bool, rusqlite::Error> {
        let count = self.conn.execute(
            "DELETE FROM memories WHERE key = ?1",
            params![key],
        )?;
        Ok(count > 0)
    }

    /// Count total memories.
    pub fn count(&self) -> Result<usize, rusqlite::Error> {
        self.conn.query_row(
            "SELECT COUNT(*) FROM memories",
            [],
            |row| row.get(0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remember_and_recall() {
        let store = MemoryStore::in_memory().unwrap();
        let mem = store.remember("user_name", "Rick", &["profile".to_string()]).unwrap();
        assert_eq!(mem.key, "user_name");
        assert_eq!(mem.content, "Rick");
        assert_eq!(mem.tags, vec!["profile"]);
    }

    #[test]
    fn test_upsert() {
        let store = MemoryStore::in_memory().unwrap();
        store.remember("fact", "old value", &[]).unwrap();
        store.remember("fact", "new value", &[]).unwrap();
        let mem = store.get("fact").unwrap();
        assert_eq!(mem.content, "new value");
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn test_search() {
        let store = MemoryStore::in_memory().unwrap();
        store.remember("project_goal", "Build an autonomous agent platform", &["project".to_string()]).unwrap();
        store.remember("user_preference", "Prefers dark mode", &["ui".to_string()]).unwrap();
        store.remember("tech_stack", "Rust, TypeScript, PostgreSQL", &["tech".to_string()]).unwrap();

        let results = store.search("autonomous agent", 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].key, "project_goal");
    }

    #[test]
    fn test_forget() {
        let store = MemoryStore::in_memory().unwrap();
        store.remember("temp", "temporary data", &[]).unwrap();
        assert_eq!(store.count().unwrap(), 1);
        assert!(store.forget("temp").unwrap());
        assert_eq!(store.count().unwrap(), 0);
        assert!(!store.forget("nonexistent").unwrap());
    }

    #[test]
    fn test_list_recent() {
        let store = MemoryStore::in_memory().unwrap();
        store.remember("first", "1", &[]).unwrap();
        store.remember("second", "2", &[]).unwrap();
        store.remember("third", "3", &[]).unwrap();

        let recent = store.list_recent(2).unwrap();
        assert_eq!(recent.len(), 2);
    }
}
