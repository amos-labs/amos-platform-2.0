//! # Token Emission Engine
//!
//! Calculates the daily emission pool and per-bounty token awards
//! with halving schedule:
//!
//! ```text
//! Year 0-1: 16,000 AMOS/day
//! Year 1-2:  8,000 AMOS/day
//! Year 2-3:  4,000 AMOS/day
//! ...
//! Floor:       100 AMOS/day (never goes below)
//! ```
//!
//! Individual bounty award:
//! ```text
//! tokens = (adjusted_points / total_points_today) × daily_emission
//! adjusted_points = raw_points × contribution_multiplier
//! ```

use super::economics::*;
use crate::error::{AmosError, Result};

/// The daily emission pool for a given day.
#[derive(Debug, Clone)]
pub struct DailyEmission {
    /// Day index (0-based from program start).
    pub day_index: u64,
    /// Current halving epoch.
    pub halving_epoch: u64,
    /// Total AMOS available for distribution today.
    pub emission: u64,
}

/// Result of a bounty token award calculation.
#[derive(Debug, Clone)]
pub struct BountyAward {
    /// Raw points from the bounty.
    pub raw_points: u64,
    /// Points after applying contribution type multiplier.
    pub adjusted_points: u64,
    /// Total points accumulated today (denominator).
    pub total_points_today: u64,
    /// Daily emission pool.
    pub daily_emission: u64,
    /// Tokens awarded to the contributor.
    pub contributor_tokens: u64,
    /// Tokens awarded to the human reviewer (5%).
    pub reviewer_tokens: u64,
    /// Total tokens distributed for this bounty.
    pub total_tokens: u64,
}

/// Contribution type for multiplier lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ContributionType {
    BugFix = 0,
    Feature = 1,
    Documentation = 2,
    Content = 3,
    Marketing = 4,
    Support = 5,
    Translation = 6,
    Design = 7,
    Testing = 8,
    Infrastructure = 9,
}

impl ContributionType {
    /// Basis-point multiplier for this contribution type.
    pub fn multiplier_bps(self) -> u64 {
        match self {
            Self::BugFix => MULTIPLIER_BUG_FIX_BPS,
            Self::Feature => MULTIPLIER_FEATURE_BPS,
            Self::Documentation | Self::Translation => MULTIPLIER_DOCS_BPS,
            Self::Content | Self::Marketing => MULTIPLIER_CONTENT_BPS,
            Self::Support => MULTIPLIER_SUPPORT_BPS,
            Self::Testing => MULTIPLIER_TESTING_BPS,
            Self::Design => MULTIPLIER_DESIGN_BPS,
            Self::Infrastructure => MULTIPLIER_INFRA_BPS,
        }
    }

    /// Parse from the on-chain `u8` bounty type.
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::BugFix),
            1 => Some(Self::Feature),
            2 => Some(Self::Documentation),
            3 => Some(Self::Content),
            4 => Some(Self::Marketing),
            5 => Some(Self::Support),
            6 => Some(Self::Translation),
            7 => Some(Self::Design),
            8 => Some(Self::Testing),
            9 => Some(Self::Infrastructure),
            _ => None,
        }
    }
}

/// Calculate the daily emission for a given day since program start.
pub fn daily_emission_for_day(day_index: u64) -> DailyEmission {
    let epoch = (day_index / HALVING_INTERVAL_DAYS).min(MAX_HALVING_EPOCHS);

    let emission = INITIAL_DAILY_EMISSION
        .checked_div(1u64.checked_shl(epoch as u32).unwrap_or(1024))
        .unwrap_or(MINIMUM_DAILY_EMISSION)
        .max(MINIMUM_DAILY_EMISSION);

    DailyEmission {
        day_index,
        halving_epoch: epoch,
        emission,
    }
}

/// Calculate the token award for a completed bounty.
///
/// This mirrors the on-chain `submit_bounty_proof` calculation exactly.
pub fn calculate_bounty_award(
    raw_points: u64,
    contribution_type: ContributionType,
    total_points_today: u64,
    daily_emission: u64,
    tokens_already_distributed_today: u64,
) -> Result<BountyAward> {
    if raw_points == 0 || raw_points > MAX_BOUNTY_POINTS {
        return Err(AmosError::Validation(format!(
            "Points must be in [1, {}], got {}",
            MAX_BOUNTY_POINTS, raw_points
        )));
    }

    // Apply contribution multiplier
    let adjusted_points = raw_points
        .checked_mul(contribution_type.multiplier_bps())
        .ok_or(AmosError::ArithmeticOverflow {
            context: "contribution multiplier".into(),
        })?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(AmosError::ArithmeticOverflow {
            context: "contribution multiplier div".into(),
        })?;

    // The denominator includes this bounty's points
    let total_points = total_points_today
        .checked_add(adjusted_points)
        .ok_or(AmosError::ArithmeticOverflow {
            context: "total points accumulation".into(),
        })?;

    // Calculate token award from remaining emission pool
    let remaining_emission = daily_emission.saturating_sub(tokens_already_distributed_today);

    let total_tokens = if total_points > 0 {
        adjusted_points
            .checked_mul(remaining_emission)
            .ok_or(AmosError::ArithmeticOverflow {
                context: "bounty award numerator".into(),
            })?
            .checked_div(total_points)
            .ok_or(AmosError::ArithmeticOverflow {
                context: "bounty award division".into(),
            })?
    } else {
        0
    };

    // Reviewer gets 5%
    let reviewer_tokens = total_tokens
        .checked_mul(REVIEWER_REWARD_BPS)
        .ok_or(AmosError::ArithmeticOverflow {
            context: "reviewer reward".into(),
        })?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(AmosError::ArithmeticOverflow {
            context: "reviewer reward div".into(),
        })?;

    let contributor_tokens = total_tokens.saturating_sub(reviewer_tokens);

    Ok(BountyAward {
        raw_points,
        adjusted_points,
        total_points_today: total_points,
        daily_emission,
        contributor_tokens,
        reviewer_tokens,
        total_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_0_emission_is_16k() {
        let em = daily_emission_for_day(0);
        assert_eq!(em.emission, 16_000);
        assert_eq!(em.halving_epoch, 0);
    }

    #[test]
    fn year_1_emission_is_8k() {
        let em = daily_emission_for_day(365);
        assert_eq!(em.emission, 8_000);
        assert_eq!(em.halving_epoch, 1);
    }

    #[test]
    fn year_2_emission_is_4k() {
        let em = daily_emission_for_day(730);
        assert_eq!(em.emission, 4_000);
    }

    #[test]
    fn emission_never_goes_below_floor() {
        let em = daily_emission_for_day(365 * 20);
        assert!(em.emission >= MINIMUM_DAILY_EMISSION);
    }

    #[test]
    fn bounty_award_includes_reviewer() {
        let award = calculate_bounty_award(
            100,
            ContributionType::Feature,
            0,   // first bounty today
            16_000,
            0,
        )
        .unwrap();

        assert_eq!(award.adjusted_points, 100); // 100% multiplier
        assert_eq!(award.total_tokens, 16_000); // only bounty today gets full pool
        assert_eq!(award.reviewer_tokens, 800); // 5% of 16000
        assert_eq!(award.contributor_tokens, 15_200);
    }

    #[test]
    fn infra_gets_130_percent_multiplier() {
        let award = calculate_bounty_award(
            100,
            ContributionType::Infrastructure,
            0,
            16_000,
            0,
        )
        .unwrap();
        assert_eq!(award.adjusted_points, 130); // 130% of 100
    }

    #[test]
    fn zero_points_rejected() {
        assert!(calculate_bounty_award(0, ContributionType::Feature, 0, 16_000, 0).is_err());
    }

    #[test]
    fn over_max_points_rejected() {
        assert!(calculate_bounty_award(
            MAX_BOUNTY_POINTS + 1,
            ContributionType::Feature,
            0,
            16_000,
            0
        )
        .is_err());
    }
}
