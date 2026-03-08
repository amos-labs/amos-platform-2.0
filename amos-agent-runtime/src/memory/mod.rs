//! # Agent Memory System
//!
//! Working memory for the autonomous loop (AMOS brain) and persistent
//! memory for cross-conversation recall.
//!
//! Architecture:
//! - **WorkingMemory**: In-memory store for the current autonomous cycle.
//!   Entries have salience scores that decay over time (attention-driven).
//! - **PersistentMemory**: Backed by PostgreSQL + pgvector for semantic search.
//!   Used by the `remember_this` and `search_memory` tools.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// A single entry in working memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: Uuid,
    /// The content of this memory.
    pub content: String,
    /// Category: "signal", "thought", "action", "observation", "reflection".
    pub category: String,
    /// Salience score (0.0 = forgotten, 1.0 = top of mind).
    pub salience: f64,
    /// When this entry was created.
    pub created_at: DateTime<Utc>,
    /// When this entry was last accessed/reinforced.
    pub last_accessed_at: DateTime<Utc>,
    /// Optional tags for filtering.
    pub tags: Vec<String>,
}

/// Working memory for the autonomous loop.
///
/// Implements attention-driven salience decay from the Rails
/// `AmosAutonomousLoop` meta-cognition phase.
pub struct WorkingMemory {
    entries: VecDeque<MemoryEntry>,
    max_entries: usize,
    /// Daily salience decay factor (e.g., 0.95 = 5% decay per cycle).
    decay_factor: f64,
}

impl WorkingMemory {
    /// Create a new working memory with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
            decay_factor: 0.95,
        }
    }

    /// Add a new entry to working memory.
    pub fn remember(&mut self, content: String, category: &str, salience: f64) -> Uuid {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let entry = MemoryEntry {
            id,
            content,
            category: category.to_string(),
            salience: salience.clamp(0.0, 1.0),
            created_at: now,
            last_accessed_at: now,
            tags: Vec::new(),
        };

        // Evict lowest-salience entry if at capacity
        if self.entries.len() >= self.max_entries {
            if let Some(min_idx) = self
                .entries
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.salience.partial_cmp(&b.salience).unwrap())
                .map(|(i, _)| i)
            {
                self.entries.remove(min_idx);
            }
        }

        self.entries.push_back(entry);
        id
    }

    /// Apply salience decay to all entries.
    ///
    /// This is called during the meta-cognition phase of the autonomous loop.
    /// Entries below a threshold are forgotten (removed).
    pub fn apply_decay(&mut self, threshold: f64) {
        for entry in &mut self.entries {
            entry.salience *= self.decay_factor;
        }
        // Remove entries below threshold
        self.entries.retain(|e| e.salience >= threshold);
    }

    /// Reinforce an entry (boost its salience) when it's accessed.
    pub fn reinforce(&mut self, id: &Uuid, boost: f64) {
        if let Some(entry) = self.entries.iter_mut().find(|e| &e.id == id) {
            entry.salience = (entry.salience + boost).clamp(0.0, 1.0);
            entry.last_accessed_at = Utc::now();
        }
    }

    /// Get top-N entries by salience (for the attention phase).
    pub fn top_entries(&self, n: usize) -> Vec<&MemoryEntry> {
        let mut sorted: Vec<&MemoryEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.salience.partial_cmp(&a.salience).unwrap());
        sorted.truncate(n);
        sorted
    }

    /// Get entries matching a category.
    pub fn by_category(&self, category: &str) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Get all entries.
    pub fn all(&self) -> &VecDeque<MemoryEntry> {
        &self.entries
    }

    /// Current number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Format working memory as context for the LLM.
    pub fn to_context_string(&self, max_entries: usize) -> String {
        let top = self.top_entries(max_entries);
        if top.is_empty() {
            return "No active thoughts in working memory.".into();
        }

        top.iter()
            .map(|e| {
                format!(
                    "[{:.0}%] ({}) {}",
                    e.salience * 100.0,
                    e.category,
                    e.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_and_recall() {
        let mut mem = WorkingMemory::new(10);
        let id = mem.remember("Test fact".into(), "observation", 0.8);
        assert_eq!(mem.len(), 1);
        assert_eq!(mem.top_entries(1)[0].id, id);
    }

    #[test]
    fn decay_removes_low_salience() {
        let mut mem = WorkingMemory::new(10);
        mem.remember("Low salience".into(), "signal", 0.05);
        mem.remember("High salience".into(), "thought", 0.9);
        mem.apply_decay(0.1);
        assert_eq!(mem.len(), 1);
        assert_eq!(mem.top_entries(1)[0].content, "High salience");
    }

    #[test]
    fn eviction_at_capacity() {
        let mut mem = WorkingMemory::new(2);
        mem.remember("First".into(), "signal", 0.5);
        mem.remember("Second".into(), "signal", 0.8);
        mem.remember("Third (evicts first)".into(), "signal", 0.9);
        assert_eq!(mem.len(), 2);
        // Lowest salience (0.5) should be evicted
        let names: Vec<&str> = mem.all().iter().map(|e| e.content.as_str()).collect();
        assert!(!names.contains(&"First"));
    }

    #[test]
    fn reinforcement_boosts_salience() {
        let mut mem = WorkingMemory::new(10);
        let id = mem.remember("Boost me".into(), "thought", 0.5);
        mem.reinforce(&id, 0.3);
        assert_eq!(mem.top_entries(1)[0].salience, 0.8);
    }
}
