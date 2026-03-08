//! Working memory with salience-based attention
//!
//! Implements the memory system from the AMOS whitepaper with:
//! - Salience scoring for importance
//! - Temporal decay
//! - Reinforcement through repeated access
//! - Efficient retrieval of top N entries

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Memory entry with salience tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub category: String,
    pub salience: f64,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u32,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl MemoryEntry {
    /// Create a new memory entry
    pub fn new(id: String, content: String, category: String, initial_salience: f64) -> Self {
        let now = Utc::now();
        Self {
            id,
            content,
            category,
            salience: initial_salience,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            metadata: HashMap::new(),
        }
    }

    /// Calculate current salience with temporal decay
    pub fn current_salience(&self) -> f64 {
        let now = Utc::now();
        let age_hours = (now - self.last_accessed).num_hours() as f64;

        // Temporal decay: salience reduces by half every 24 hours
        let decay_factor = 0.5_f64.powf(age_hours / 24.0);

        // Access frequency boost
        let frequency_boost = 1.0 + (self.access_count as f64).ln().max(0.0) * 0.1;

        self.salience * decay_factor * frequency_boost
    }

    /// Apply temporal decay to salience
    pub fn apply_decay(&mut self) {
        self.salience = self.current_salience();
    }

    /// Reinforce memory through access
    pub fn reinforce(&mut self) {
        self.last_accessed = Utc::now();
        self.access_count += 1;

        // Small boost to salience on access
        self.salience = (self.salience * 1.05).min(1.0);
    }
}

/// Working memory store
#[derive(Debug, Clone)]
pub struct WorkingMemory {
    entries: Vec<MemoryEntry>,
    max_entries: usize,
}

impl WorkingMemory {
    /// Create a new working memory with capacity
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Add a memory entry
    pub fn remember(&mut self, mut entry: MemoryEntry) {
        // Apply decay to existing entries
        self.apply_decay_all();

        // Add new entry
        self.entries.push(entry);

        // Evict lowest salience entries if over capacity
        if self.entries.len() > self.max_entries {
            self.entries.sort_by(|a, b| {
                b.current_salience()
                    .partial_cmp(&a.current_salience())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            self.entries.truncate(self.max_entries);
        }
    }

    /// Apply decay to all entries
    pub fn apply_decay_all(&mut self) {
        for entry in &mut self.entries {
            entry.apply_decay();
        }
    }

    /// Reinforce a memory entry by ID
    pub fn reinforce(&mut self, id: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.reinforce();
        }
    }

    /// Get top N entries by current salience
    pub fn top_entries(&mut self, n: usize) -> Vec<MemoryEntry> {
        self.apply_decay_all();

        self.entries.sort_by(|a, b| {
            b.current_salience()
                .partial_cmp(&a.current_salience())
                .unwrap()
        });

        self.entries.iter().take(n).cloned().collect()
    }

    /// Search entries by content
    pub fn search(&mut self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        self.apply_decay_all();

        let query_lower = query.to_lowercase();

        let mut matching: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.content.to_lowercase().contains(&query_lower))
            .cloned()
            .collect();

        // Sort by salience
        matching.sort_by(|a, b| {
            b.current_salience()
                .partial_cmp(&a.current_salience())
                .unwrap()
        });

        // Reinforce accessed entries
        for entry in &matching {
            self.reinforce(&entry.id);
        }

        matching.into_iter().take(limit).collect()
    }

    /// Get entries by category
    pub fn get_by_category(&mut self, category: &str, limit: usize) -> Vec<MemoryEntry> {
        self.apply_decay_all();

        let mut matching: Vec<_> = self
            .entries
            .iter()
            .filter(|e| e.category == category)
            .cloned()
            .collect();

        matching.sort_by(|a, b| {
            b.current_salience()
                .partial_cmp(&a.current_salience())
                .unwrap()
        });

        matching.into_iter().take(limit).collect()
    }

    /// Get total number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if memory is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for WorkingMemory {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_entry_decay() {
        let mut entry = MemoryEntry::new(
            "test".to_string(),
            "Test content".to_string(),
            "test".to_string(),
            1.0,
        );

        // Simulate 24 hours passing
        entry.last_accessed = Utc::now() - Duration::hours(24);

        let current = entry.current_salience();
        assert!(current < 1.0); // Should have decayed
        assert!(current > 0.4); // But not too much
    }

    #[test]
    fn test_memory_reinforce() {
        let mut entry = MemoryEntry::new(
            "test".to_string(),
            "Test content".to_string(),
            "test".to_string(),
            0.5,
        );

        let initial = entry.salience;
        entry.reinforce();

        assert!(entry.salience > initial);
        assert_eq!(entry.access_count, 1);
    }

    #[test]
    fn test_working_memory() {
        let mut memory = WorkingMemory::new(5);

        memory.remember(MemoryEntry::new(
            "1".to_string(),
            "High importance".to_string(),
            "test".to_string(),
            0.9,
        ));

        memory.remember(MemoryEntry::new(
            "2".to_string(),
            "Low importance".to_string(),
            "test".to_string(),
            0.3,
        ));

        let top = memory.top_entries(1);
        assert_eq!(top[0].id, "1");
    }

    #[test]
    fn test_memory_search() {
        let mut memory = WorkingMemory::new(10);

        memory.remember(MemoryEntry::new(
            "1".to_string(),
            "Alice likes apples".to_string(),
            "preference".to_string(),
            0.8,
        ));

        memory.remember(MemoryEntry::new(
            "2".to_string(),
            "Bob likes bananas".to_string(),
            "preference".to_string(),
            0.7,
        ));

        let results = memory.search("alice", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "1");
    }
}
