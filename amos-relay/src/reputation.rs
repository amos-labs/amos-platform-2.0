//! Reputation engine for computing agent trust levels.

/// Reputation engine for computing trust levels based on task performance.
pub struct ReputationEngine;

impl ReputationEngine {
    /// Compute trust level (1-5) based on agent's historical performance.
    ///
    /// Trust levels:
    /// - Level 1: Newcomer (< 5 tasks)
    /// - Level 2: Bronze (5+ tasks, 70%+ completion, 50+ quality)
    /// - Level 3: Silver (20+ tasks, 85%+ completion, 70+ quality)
    /// - Level 4: Gold (100+ tasks, 95%+ completion, 85+ quality)
    /// - Level 5: Elite (500+ tasks, 98%+ completion, 95+ quality)
    ///
    /// # Arguments
    /// * `completed` - Number of successfully completed tasks
    /// * `failed` - Number of failed tasks
    /// * `avg_quality` - Average quality score (0-100)
    ///
    /// # Returns
    /// Trust level from 1 (Newcomer) to 5 (Elite)
    pub fn compute_trust_level(completed: u32, failed: u32, avg_quality: f64) -> u8 {
        let total_tasks = completed + failed;

        // Compute completion rate
        let completion_rate = if total_tasks > 0 {
            (completed as f64) / (total_tasks as f64)
        } else {
            0.0
        };

        // Determine trust level based on thresholds
        if total_tasks < 5 {
            // Newcomer: less than 5 tasks
            1
        } else if total_tasks >= 500 && completion_rate >= 0.98 && avg_quality >= 95.0 {
            // Elite: 500+ tasks, 98%+ completion, 95+ quality
            5
        } else if total_tasks >= 100 && completion_rate >= 0.95 && avg_quality >= 85.0 {
            // Gold: 100+ tasks, 95%+ completion, 85+ quality
            4
        } else if total_tasks >= 20 && completion_rate >= 0.85 && avg_quality >= 70.0 {
            // Silver: 20+ tasks, 85%+ completion, 70+ quality
            3
        } else if total_tasks >= 5 && completion_rate >= 0.70 && avg_quality >= 50.0 {
            // Bronze: 5+ tasks, 70%+ completion, 50+ quality
            2
        } else {
            // Doesn't meet Bronze criteria, still Newcomer
            1
        }
    }

    /// Get a human-readable name for a trust level.
    pub fn trust_level_name(level: u8) -> &'static str {
        match level {
            1 => "Newcomer",
            2 => "Bronze",
            3 => "Silver",
            4 => "Gold",
            5 => "Elite",
            _ => "Unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_newcomer_level() {
        // Less than 5 tasks
        let level = ReputationEngine::compute_trust_level(3, 1, 80.0);
        assert_eq!(level, 1);
        assert_eq!(ReputationEngine::trust_level_name(level), "Newcomer");
    }

    #[test]
    fn test_bronze_level() {
        // 5+ tasks, 70%+ completion, 50+ quality
        let level = ReputationEngine::compute_trust_level(7, 3, 60.0);
        assert_eq!(level, 2);
        assert_eq!(ReputationEngine::trust_level_name(level), "Bronze");
    }

    #[test]
    fn test_silver_level() {
        // 20+ tasks, 85%+ completion, 70+ quality
        let level = ReputationEngine::compute_trust_level(20, 3, 75.0);
        assert_eq!(level, 3);
        assert_eq!(ReputationEngine::trust_level_name(level), "Silver");
    }

    #[test]
    fn test_gold_level() {
        // 100+ tasks, 95%+ completion, 85+ quality
        let level = ReputationEngine::compute_trust_level(100, 5, 90.0);
        assert_eq!(level, 4);
        assert_eq!(ReputationEngine::trust_level_name(level), "Gold");
    }

    #[test]
    fn test_elite_level() {
        // 500+ tasks, 98%+ completion, 95+ quality
        let level = ReputationEngine::compute_trust_level(500, 10, 98.0);
        assert_eq!(level, 5);
        assert_eq!(ReputationEngine::trust_level_name(level), "Elite");
    }

    #[test]
    fn test_insufficient_completion_rate() {
        // Many tasks but low completion rate
        let level = ReputationEngine::compute_trust_level(50, 50, 80.0);
        assert_eq!(level, 1); // Only 50% completion, doesn't meet Bronze
    }

    #[test]
    fn test_insufficient_quality() {
        // Good completion rate but low quality
        let level = ReputationEngine::compute_trust_level(20, 2, 40.0);
        assert_eq!(level, 1); // Quality too low for Bronze
    }

    #[test]
    fn test_edge_case_perfect_newcomer() {
        // Perfect stats but not enough tasks
        let level = ReputationEngine::compute_trust_level(4, 0, 100.0);
        assert_eq!(level, 1); // Still newcomer with < 5 tasks
    }

    #[test]
    fn test_edge_case_exact_thresholds() {
        // Exactly at Bronze threshold
        let level = ReputationEngine::compute_trust_level(7, 3, 50.0);
        assert_eq!(level, 2); // 70% completion, 50 quality -> Bronze

        // Exactly at Silver threshold
        let level = ReputationEngine::compute_trust_level(20, 3, 70.0);
        assert_eq!(level, 3); // ~87% completion, 70 quality -> Silver
    }
}
