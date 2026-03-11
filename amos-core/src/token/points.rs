//! Points and work-token economy implementation.
//!
//! This module implements the referral and contribution points system that
//! determines how daily AMOS token emissions are distributed to contributors.
//!
//! ## Points System
//!
//! Contributors earn points through various activities:
//! - Email invitation sent: 1 point
//! - User signup (from referral): 5 points
//! - Paid conversion: 10 points
//! - Active month (referred user stays active): 2 points per month
//! - Sales signup: 1 point per user
//! - Bounty completion: Points equal to bounty value
//!
//! ## Daily Rewards
//!
//! At the end of each day, the daily emission is distributed proportionally:
//! ```text
//! contributor_reward = (contributor_points / total_points) * daily_emission
//! ```
//!
//! See `docs/token_economy_equations.md` for the complete specification.

use crate::error::{AmosError, Result};

// ═══════════════════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// Types of point-earning activities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointActivity {
    /// Email invitation sent: 1 point
    EmailInvitation,
    /// User signup from referral: 5 points
    ReferralSignup,
    /// Paid conversion: 10 points
    PaidConversion,
    /// Active month (referred user stays active): 2 points
    ActiveMonth,
    /// Sales signup: 1 point per user
    SalesSignup,
    /// Bounty completion: Variable points equal to bounty value in AMOS
    BountyCompletion(u64),
}

/// A single points ledger entry.
#[derive(Debug, Clone)]
pub struct PointEntry {
    /// The activity that earned these points
    pub activity: PointActivity,
    /// Number of points earned
    pub points: u64,
    /// Unix timestamp when points were earned
    pub timestamp: i64,
    /// Optional reference ID (e.g., user ID, bounty ID)
    pub reference_id: Option<String>,
}

/// Accumulated points for a contributor.
#[derive(Debug, Clone)]
pub struct ContributorPoints {
    /// Unique contributor identifier
    pub contributor_id: u64,
    /// Total accumulated points
    pub total_points: u64,
    /// Individual point entries
    pub entries: Vec<PointEntry>,
    /// Period start (Unix timestamp)
    pub period_start: i64,
    /// Period end (Unix timestamp)
    pub period_end: i64,
}

/// Daily points snapshot for emission calculation.
#[derive(Debug, Clone)]
pub struct DailyPointsSnapshot {
    /// Day index (0 = genesis day)
    pub day_index: u64,
    /// Total points earned across all contributors
    pub total_points: u64,
    /// Number of contributors who earned points
    pub contributor_count: u64,
    /// Individual contributor entries: (contributor_id, points)
    pub entries: Vec<(u64, u64)>,
}

// ═══════════════════════════════════════════════════════════════════════════
// CORE FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════

/// Returns the number of points for a given activity.
///
/// # Examples
///
/// ```
/// use amos_core::token::points::{PointActivity, calculate_points};
///
/// assert_eq!(calculate_points(&PointActivity::EmailInvitation), 1);
/// assert_eq!(calculate_points(&PointActivity::ReferralSignup), 5);
/// assert_eq!(calculate_points(&PointActivity::PaidConversion), 10);
/// assert_eq!(calculate_points(&PointActivity::ActiveMonth), 2);
/// assert_eq!(calculate_points(&PointActivity::SalesSignup), 1);
/// assert_eq!(calculate_points(&PointActivity::BountyCompletion(500)), 500);
/// ```
pub fn calculate_points(activity: &PointActivity) -> u64 {
    match activity {
        PointActivity::EmailInvitation => 1,
        PointActivity::ReferralSignup => 5,
        PointActivity::PaidConversion => 10,
        PointActivity::ActiveMonth => 2,
        PointActivity::SalesSignup => 1,
        PointActivity::BountyCompletion(value) => *value,
    }
}

/// Calculates total referral points using the standard formula.
///
/// Formula:
/// ```text
/// total = (emails_sent * 1) + (signups * 5) + (conversions * 10) + (active_months * 2)
/// ```
///
/// # Examples
///
/// ```
/// use amos_core::token::points::calculate_referral_points;
///
/// // Sent 10 emails, got 3 signups, 1 conversion, 2 active months
/// let points = calculate_referral_points(10, 3, 1, 2);
/// assert_eq!(points, 10 + 15 + 10 + 4); // 39 points
/// ```
pub fn calculate_referral_points(
    emails_sent: u64,
    signups: u64,
    conversions: u64,
    active_months: u64,
) -> u64 {
    emails_sent
        .saturating_mul(1)
        .saturating_add(signups.saturating_mul(5))
        .saturating_add(conversions.saturating_mul(10))
        .saturating_add(active_months.saturating_mul(2))
}

/// Calculates an individual contributor's token reward from daily emission.
///
/// Formula:
/// ```text
/// reward = (contributor_points / total_points) * daily_emission
/// ```
///
/// # Arguments
///
/// * `contributor_points` - Points earned by this contributor
/// * `total_points` - Total points earned by all contributors
/// * `daily_emission` - Total tokens emitted this day
///
/// # Errors
///
/// Returns `AmosError::ArithmeticOverflow` if the calculation would overflow.
/// Returns `AmosError::Validation` if total_points is zero.
///
/// # Examples
///
/// ```
/// use amos_core::token::points::calculate_token_reward;
///
/// // Contributor earned 50 points out of 1000 total, emission is 16000 tokens
/// let reward = calculate_token_reward(50, 1000, 16000).unwrap();
/// assert_eq!(reward, 800); // (50/1000) * 16000 = 800
/// ```
pub fn calculate_token_reward(
    contributor_points: u64,
    total_points: u64,
    daily_emission: u64,
) -> Result<u64> {
    if total_points == 0 {
        return Err(AmosError::Validation(
            "total_points must be greater than zero".to_string(),
        ));
    }

    // Use checked arithmetic to prevent overflow
    let numerator = contributor_points
        .checked_mul(daily_emission)
        .ok_or_else(|| AmosError::ArithmeticOverflow {
            context: format!(
                "overflow calculating reward: {} * {}",
                contributor_points, daily_emission
            ),
        })?;

    Ok(numerator / total_points)
}

/// Calculates token rewards for all contributors in a daily snapshot.
///
/// Returns a vector of (contributor_id, token_reward) tuples.
///
/// # Arguments
///
/// * `snapshot` - Daily points snapshot containing all contributor points
/// * `daily_emission` - Total tokens to distribute this day
///
/// # Errors
///
/// Returns `AmosError::ArithmeticOverflow` if any calculation would overflow.
/// Returns `AmosError::Validation` if snapshot.total_points is zero.
///
/// # Examples
///
/// ```
/// use amos_core::token::points::{DailyPointsSnapshot, calculate_daily_rewards};
///
/// let snapshot = DailyPointsSnapshot {
///     day_index: 0,
///     total_points: 1000,
///     contributor_count: 3,
///     entries: vec![
///         (1, 500),  // Contributor 1: 50% of points
///         (2, 300),  // Contributor 2: 30% of points
///         (3, 200),  // Contributor 3: 20% of points
///     ],
/// };
///
/// let rewards = calculate_daily_rewards(&snapshot, 10000).unwrap();
/// assert_eq!(rewards.len(), 3);
/// assert_eq!(rewards[0], (1, 5000));  // 50% of 10000
/// assert_eq!(rewards[1], (2, 3000));  // 30% of 10000
/// assert_eq!(rewards[2], (3, 2000));  // 20% of 10000
/// ```
pub fn calculate_daily_rewards(
    snapshot: &DailyPointsSnapshot,
    daily_emission: u64,
) -> Result<Vec<(u64, u64)>> {
    if snapshot.total_points == 0 {
        return Err(AmosError::Validation(
            "snapshot.total_points must be greater than zero".to_string(),
        ));
    }

    let mut rewards = Vec::with_capacity(snapshot.entries.len());

    for (contributor_id, points) in &snapshot.entries {
        let reward = calculate_token_reward(*points, snapshot.total_points, daily_emission)?;
        rewards.push((*contributor_id, reward));
    }

    Ok(rewards)
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════

impl PointEntry {
    /// Creates a new point entry.
    pub fn new(activity: PointActivity, timestamp: i64, reference_id: Option<String>) -> Self {
        let points = calculate_points(&activity);
        Self {
            activity,
            points,
            timestamp,
            reference_id,
        }
    }
}

impl ContributorPoints {
    /// Creates a new contributor points record.
    pub fn new(contributor_id: u64, period_start: i64, period_end: i64) -> Self {
        Self {
            contributor_id,
            total_points: 0,
            entries: Vec::new(),
            period_start,
            period_end,
        }
    }

    /// Adds a point entry and updates the total.
    pub fn add_entry(&mut self, entry: PointEntry) {
        self.total_points = self.total_points.saturating_add(entry.points);
        self.entries.push(entry);
    }

    /// Calculates this contributor's token reward for a given emission.
    pub fn calculate_reward(&self, total_points: u64, daily_emission: u64) -> Result<u64> {
        calculate_token_reward(self.total_points, total_points, daily_emission)
    }
}

impl DailyPointsSnapshot {
    /// Creates a new daily snapshot from contributor points.
    pub fn new(day_index: u64, contributors: &[ContributorPoints]) -> Self {
        let mut entries = Vec::with_capacity(contributors.len());
        let mut total_points = 0u64;

        for contributor in contributors {
            if contributor.total_points > 0 {
                entries.push((contributor.contributor_id, contributor.total_points));
                total_points = total_points.saturating_add(contributor.total_points);
            }
        }

        Self {
            day_index,
            total_points,
            contributor_count: entries.len() as u64,
            entries,
        }
    }

    /// Returns true if this snapshot has no points.
    pub fn is_empty(&self) -> bool {
        self.total_points == 0
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::super::economics::MAX_BOUNTY_POINTS;
    use super::*;

    #[test]
    fn test_calculate_points() {
        assert_eq!(calculate_points(&PointActivity::EmailInvitation), 1);
        assert_eq!(calculate_points(&PointActivity::ReferralSignup), 5);
        assert_eq!(calculate_points(&PointActivity::PaidConversion), 10);
        assert_eq!(calculate_points(&PointActivity::ActiveMonth), 2);
        assert_eq!(calculate_points(&PointActivity::SalesSignup), 1);
        assert_eq!(calculate_points(&PointActivity::BountyCompletion(100)), 100);
        assert_eq!(
            calculate_points(&PointActivity::BountyCompletion(2000)),
            2000
        );
    }

    #[test]
    fn test_calculate_referral_points() {
        // No activity
        assert_eq!(calculate_referral_points(0, 0, 0, 0), 0);

        // Only emails
        assert_eq!(calculate_referral_points(10, 0, 0, 0), 10);

        // Emails + signups
        assert_eq!(calculate_referral_points(10, 3, 0, 0), 25); // 10 + 15

        // Full pipeline
        assert_eq!(calculate_referral_points(10, 3, 1, 2), 39); // 10 + 15 + 10 + 4

        // Large numbers
        assert_eq!(calculate_referral_points(1000, 500, 100, 50), 4600);
    }

    #[test]
    fn test_calculate_referral_points_saturation() {
        // Test saturating arithmetic doesn't panic
        let max = u64::MAX;
        let result = calculate_referral_points(max, max, max, max);
        assert_eq!(result, u64::MAX); // Should saturate, not panic
    }

    #[test]
    fn test_calculate_token_reward_basic() {
        // 50% of points, 50% of reward
        let reward = calculate_token_reward(500, 1000, 10000).unwrap();
        assert_eq!(reward, 5000);

        // 25% of points, 25% of reward
        let reward = calculate_token_reward(250, 1000, 10000).unwrap();
        assert_eq!(reward, 2500);

        // 100% of points, 100% of reward
        let reward = calculate_token_reward(1000, 1000, 10000).unwrap();
        assert_eq!(reward, 10000);
    }

    #[test]
    fn test_calculate_token_reward_realistic() {
        // Realistic scenario: contributor earned 50 points out of 1000 total
        // Daily emission is 16000 AMOS
        let reward = calculate_token_reward(50, 1000, 16000).unwrap();
        assert_eq!(reward, 800); // (50/1000) * 16000 = 800

        // Contributor earned 200 points out of 5000 total
        let reward = calculate_token_reward(200, 5000, 16000).unwrap();
        assert_eq!(reward, 640); // (200/5000) * 16000 = 640
    }

    #[test]
    fn test_calculate_token_reward_rounding() {
        // Test integer division rounding
        let reward = calculate_token_reward(1, 3, 10000).unwrap();
        assert_eq!(reward, 3333); // 10000/3 = 3333 (rounded down)

        let reward = calculate_token_reward(2, 3, 10000).unwrap();
        assert_eq!(reward, 6666); // 20000/3 = 6666 (rounded down)
    }

    #[test]
    fn test_calculate_token_reward_zero_total_points() {
        let result = calculate_token_reward(100, 0, 10000);
        assert!(result.is_err());
        match result {
            Err(AmosError::Validation(msg)) => {
                assert!(msg.contains("total_points must be greater than zero"));
            }
            _ => panic!("Expected Validation error"),
        }
    }

    #[test]
    fn test_calculate_token_reward_overflow() {
        // Force overflow by multiplying large numbers
        let result = calculate_token_reward(u64::MAX, 1, u64::MAX);
        assert!(result.is_err());
        match result {
            Err(AmosError::ArithmeticOverflow { .. }) => {}
            _ => panic!("Expected ArithmeticOverflow error"),
        }
    }

    #[test]
    fn test_calculate_daily_rewards_basic() {
        let snapshot = DailyPointsSnapshot {
            day_index: 0,
            total_points: 1000,
            contributor_count: 3,
            entries: vec![
                (1, 500), // 50%
                (2, 300), // 30%
                (3, 200), // 20%
            ],
        };

        let rewards = calculate_daily_rewards(&snapshot, 10000).unwrap();
        assert_eq!(rewards.len(), 3);
        assert_eq!(rewards[0], (1, 5000));
        assert_eq!(rewards[1], (2, 3000));
        assert_eq!(rewards[2], (3, 2000));
    }

    #[test]
    fn test_calculate_daily_rewards_realistic() {
        // Realistic scenario with 16000 AMOS daily emission
        let snapshot = DailyPointsSnapshot {
            day_index: 0,
            total_points: 1000,
            contributor_count: 5,
            entries: vec![
                (1, 400), // 40% = 6400 AMOS
                (2, 250), // 25% = 4000 AMOS
                (3, 200), // 20% = 3200 AMOS
                (4, 100), // 10% = 1600 AMOS
                (5, 50),  // 5% = 800 AMOS
            ],
        };

        let rewards = calculate_daily_rewards(&snapshot, 16000).unwrap();
        assert_eq!(rewards.len(), 5);
        assert_eq!(rewards[0], (1, 6400));
        assert_eq!(rewards[1], (2, 4000));
        assert_eq!(rewards[2], (3, 3200));
        assert_eq!(rewards[3], (4, 1600));
        assert_eq!(rewards[4], (5, 800));
    }

    #[test]
    fn test_calculate_daily_rewards_empty() {
        let snapshot = DailyPointsSnapshot {
            day_index: 0,
            total_points: 0,
            contributor_count: 0,
            entries: vec![],
        };

        let result = calculate_daily_rewards(&snapshot, 10000);
        assert!(result.is_err());
    }

    #[test]
    fn test_point_entry_creation() {
        let entry = PointEntry::new(
            PointActivity::EmailInvitation,
            1234567890,
            Some("user123".to_string()),
        );

        assert_eq!(entry.points, 1);
        assert_eq!(entry.timestamp, 1234567890);
        assert_eq!(entry.reference_id, Some("user123".to_string()));
    }

    #[test]
    fn test_contributor_points_accumulation() {
        let mut contributor = ContributorPoints::new(1, 0, 86400);

        // Add some activities
        contributor.add_entry(PointEntry::new(PointActivity::EmailInvitation, 1000, None));
        contributor.add_entry(PointEntry::new(
            PointActivity::ReferralSignup,
            2000,
            Some("user1".to_string()),
        ));
        contributor.add_entry(PointEntry::new(
            PointActivity::PaidConversion,
            3000,
            Some("user1".to_string()),
        ));

        assert_eq!(contributor.total_points, 16); // 1 + 5 + 10
        assert_eq!(contributor.entries.len(), 3);
    }

    #[test]
    fn test_contributor_points_calculate_reward() {
        let mut contributor = ContributorPoints::new(1, 0, 86400);
        contributor.add_entry(PointEntry::new(
            PointActivity::BountyCompletion(500),
            1000,
            Some("bounty1".to_string()),
        ));

        // Contributor has 500 points out of 1000 total, emission is 16000
        let reward = contributor.calculate_reward(1000, 16000).unwrap();
        assert_eq!(reward, 8000); // (500/1000) * 16000 = 8000
    }

    #[test]
    fn test_daily_snapshot_creation() {
        let contributors = vec![
            {
                let mut c = ContributorPoints::new(1, 0, 86400);
                c.add_entry(PointEntry::new(PointActivity::EmailInvitation, 1000, None));
                c.add_entry(PointEntry::new(PointActivity::ReferralSignup, 2000, None));
                c
            },
            {
                let mut c = ContributorPoints::new(2, 0, 86400);
                c.add_entry(PointEntry::new(PointActivity::PaidConversion, 3000, None));
                c
            },
            {
                // Empty contributor (should be filtered out)
                ContributorPoints::new(3, 0, 86400)
            },
        ];

        let snapshot = DailyPointsSnapshot::new(0, &contributors);

        assert_eq!(snapshot.day_index, 0);
        assert_eq!(snapshot.total_points, 16); // 6 + 10
        assert_eq!(snapshot.contributor_count, 2); // Only 2 have points
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.entries[0], (1, 6));
        assert_eq!(snapshot.entries[1], (2, 10));
    }

    #[test]
    fn test_daily_snapshot_is_empty() {
        let snapshot = DailyPointsSnapshot {
            day_index: 0,
            total_points: 0,
            contributor_count: 0,
            entries: vec![],
        };
        assert!(snapshot.is_empty());

        let snapshot = DailyPointsSnapshot {
            day_index: 0,
            total_points: 100,
            contributor_count: 1,
            entries: vec![(1, 100)],
        };
        assert!(!snapshot.is_empty());
    }

    #[test]
    fn test_bounty_completion_max_points() {
        // Test bounty with MAX_BOUNTY_POINTS
        let activity = PointActivity::BountyCompletion(MAX_BOUNTY_POINTS);
        assert_eq!(calculate_points(&activity), MAX_BOUNTY_POINTS);

        // Should work in reward calculation
        let reward =
            calculate_token_reward(MAX_BOUNTY_POINTS, MAX_BOUNTY_POINTS * 2, 16000).unwrap();
        assert_eq!(reward, 8000); // 50% of emission
    }

    #[test]
    fn test_complete_referral_workflow() {
        // Simulate a complete referral workflow
        let mut contributor = ContributorPoints::new(1, 0, 86400);

        // Send 10 email invitations
        for i in 0..10 {
            contributor.add_entry(PointEntry::new(
                PointActivity::EmailInvitation,
                1000 + i,
                Some(format!("email{}", i)),
            ));
        }

        // 3 users sign up
        for i in 0..3 {
            contributor.add_entry(PointEntry::new(
                PointActivity::ReferralSignup,
                2000 + i,
                Some(format!("user{}", i)),
            ));
        }

        // 1 converts to paid
        contributor.add_entry(PointEntry::new(
            PointActivity::PaidConversion,
            3000,
            Some("user0".to_string()),
        ));

        // 2 active months tracked
        for i in 0..2 {
            contributor.add_entry(PointEntry::new(
                PointActivity::ActiveMonth,
                4000 + i,
                Some("user0".to_string()),
            ));
        }

        // Should match calculate_referral_points formula
        assert_eq!(contributor.total_points, 39); // 10 + 15 + 10 + 4
        assert_eq!(
            contributor.total_points,
            calculate_referral_points(10, 3, 1, 2)
        );
    }

    #[test]
    fn test_multiple_contributors_distribution() {
        // Test realistic multi-contributor scenario
        let mut contributors = Vec::new();

        // Contributor 1: Active referrer (400 points)
        let mut c1 = ContributorPoints::new(1, 0, 86400);
        c1.total_points = 400;
        contributors.push(c1);

        // Contributor 2: Bounty hunter (300 points)
        let mut c2 = ContributorPoints::new(2, 0, 86400);
        c2.total_points = 300;
        contributors.push(c2);

        // Contributor 3: Sales person (200 points)
        let mut c3 = ContributorPoints::new(3, 0, 86400);
        c3.total_points = 200;
        contributors.push(c3);

        // Contributor 4: Small contributor (100 points)
        let mut c4 = ContributorPoints::new(4, 0, 86400);
        c4.total_points = 100;
        contributors.push(c4);

        let snapshot = DailyPointsSnapshot::new(0, &contributors);
        assert_eq!(snapshot.total_points, 1000);
        assert_eq!(snapshot.contributor_count, 4);

        // Calculate rewards with 16000 AMOS emission
        let rewards = calculate_daily_rewards(&snapshot, 16000).unwrap();

        assert_eq!(rewards[0], (1, 6400)); // 40% of 16000
        assert_eq!(rewards[1], (2, 4800)); // 30% of 16000
        assert_eq!(rewards[2], (3, 3200)); // 20% of 16000
        assert_eq!(rewards[3], (4, 1600)); // 10% of 16000

        // Verify total distribution equals emission
        let total_distributed: u64 = rewards.iter().map(|(_, r)| r).sum();
        assert_eq!(total_distributed, 16000);
    }
}
