//! Quartile-based weight adjustment for Darwinian optimization.
//!
//! Agents in the top fitness quartile receive a small weight boost (capped at
//! 2.5), while agents in the bottom quartile are penalized (floored at 0.3).
//! Agents in the middle two quartiles are left unchanged. An optional
//! normalization step can rescale all weights so they average to 1.0.

/// Adjust Darwinian weights based on fitness quartiles.
///
/// Top quartile: weight = min(weight * 1.05, 2.5)
/// Bottom quartile: weight = max(weight * 0.95, 0.3)
/// Middle two quartiles: unchanged
///
/// # Arguments
///
/// * `members` — mutable slice of `(agent_id, current_weight, fitness_score)`.
///   The slice is sorted in place by fitness (descending).
///
/// # Returns
///
/// A vec of `(agent_id, new_weight)` for every agent whose weight changed.
pub fn adjust_weights(members: &mut [(i32, f64, f64)]) -> Vec<(i32, f64)> {
    if members.is_empty() {
        return Vec::new();
    }

    // Sort by fitness descending
    members.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let len = members.len();
    let quartile_size = len / 4;

    // Need at least 4 members to define meaningful quartiles.
    if quartile_size == 0 {
        return Vec::new();
    }

    let top_boundary = quartile_size;
    let bottom_start = len - quartile_size;

    let mut changed = Vec::new();

    for (i, (agent_id, weight, _fitness)) in members.iter_mut().enumerate() {
        let original_weight = *weight;

        if i < top_boundary {
            // Top quartile — boost
            *weight = (*weight * 1.05).min(2.5);
        } else if i >= bottom_start {
            // Bottom quartile — penalize
            *weight = (*weight * 0.95).max(0.3);
        }
        // Middle two quartiles — unchanged

        if (*weight - original_weight).abs() > f64::EPSILON {
            changed.push((*agent_id, *weight));
        }
    }

    changed
}

/// Normalize weights so they sum to the number of agents (i.e. the average
/// weight becomes 1.0).
///
/// # Arguments
///
/// * `members` — slice of `(agent_id, current_weight)`.
///
/// # Returns
///
/// A vec of `(agent_id, normalized_weight)` for every agent.
pub fn normalize_weights(members: &[(i32, f64)]) -> Vec<(i32, f64)> {
    if members.is_empty() {
        return Vec::new();
    }

    let n = members.len() as f64;
    let total: f64 = members.iter().map(|(_, w)| w).sum();

    if total == 0.0 {
        // Avoid division by zero — assign uniform weights.
        return members.iter().map(|(id, _)| (*id, 1.0)).collect();
    }

    let scale = n / total;

    members.iter().map(|(id, w)| (*id, w * scale)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjust_weights_basic() {
        // 8 agents — top 2, bottom 2 should change
        let mut members = vec![
            (1, 1.0, 0.9),
            (2, 1.0, 0.8),
            (3, 1.0, 0.7),
            (4, 1.0, 0.6),
            (5, 1.0, 0.5),
            (6, 1.0, 0.4),
            (7, 1.0, 0.3),
            (8, 1.0, 0.2),
        ];

        let changed = adjust_weights(&mut members);

        // Top 2 agents boosted, bottom 2 penalized = 4 changes
        assert_eq!(changed.len(), 4);

        // Top quartile should be boosted to 1.05
        let top: Vec<_> = changed.iter().filter(|(_, w)| *w > 1.0).collect();
        assert_eq!(top.len(), 2);
        for (_, w) in &top {
            assert!((*w - 1.05).abs() < f64::EPSILON);
        }

        // Bottom quartile should be penalized to 0.95
        let bottom: Vec<_> = changed.iter().filter(|(_, w)| *w < 1.0).collect();
        assert_eq!(bottom.len(), 2);
        for (_, w) in &bottom {
            assert!((*w - 0.95).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_adjust_weights_caps() {
        let mut members = vec![
            (1, 2.5, 0.9), // already at cap
            (2, 1.0, 0.8),
            (3, 1.0, 0.5),
            (4, 0.3, 0.1), // already at floor
        ];

        let changed = adjust_weights(&mut members);

        // Agent 1: 2.5 * 1.05 = 2.625 -> capped at 2.5 -> no change
        // Agent 4: 0.3 * 0.95 = 0.285 -> floored at 0.3 -> no change
        // So no weights actually change
        assert!(changed.is_empty());
    }

    #[test]
    fn test_adjust_weights_empty() {
        let mut members: Vec<(i32, f64, f64)> = vec![];
        let changed = adjust_weights(&mut members);
        assert!(changed.is_empty());
    }

    #[test]
    fn test_adjust_weights_too_few() {
        let mut members = vec![(1, 1.0, 0.9), (2, 1.0, 0.5), (3, 1.0, 0.1)];
        // 3 / 4 = 0 quartile size — no changes
        let changed = adjust_weights(&mut members);
        assert!(changed.is_empty());
    }

    #[test]
    fn test_normalize_weights() {
        let members = vec![(1, 1.5), (2, 0.5), (3, 1.0)];

        let normalized = normalize_weights(&members);

        // Total = 3.0, n = 3, scale = 1.0 — already normalized
        assert_eq!(normalized.len(), 3);
        let total: f64 = normalized.iter().map(|(_, w)| w).sum();
        assert!((total - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_weights_scales() {
        let members = vec![(1, 2.0), (2, 4.0)];

        let normalized = normalize_weights(&members);

        // Total = 6.0, n = 2, scale = 2/6 = 1/3
        // Agent 1: 2.0 * 1/3 = 0.6667
        // Agent 2: 4.0 * 1/3 = 1.3333
        assert_eq!(normalized.len(), 2);
        let total: f64 = normalized.iter().map(|(_, w)| w).sum();
        assert!((total - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_normalize_weights_empty() {
        let members: Vec<(i32, f64)> = vec![];
        let normalized = normalize_weights(&members);
        assert!(normalized.is_empty());
    }

    #[test]
    fn test_normalize_weights_all_zero() {
        let members = vec![(1, 0.0), (2, 0.0)];
        let normalized = normalize_weights(&members);

        // All zero weights → uniform
        for (_, w) in &normalized {
            assert!((*w - 1.0).abs() < f64::EPSILON);
        }
    }
}
