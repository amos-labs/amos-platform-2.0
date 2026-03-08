//! # AMOS Token Economics — Constants & Core Types
//!
//! Every number here is sourced from:
//!   - `docs/whitepaper_technical.md`
//!   - `docs/token_economy_equations.md`
//!   - `solana/programs/amos_treasury/src/constants.rs`
//!   - `solana/programs/amos_bounty/src/constants.rs`
//!
//! CRITICAL: These constants must remain byte-identical to the on-chain
//! program constants. Changing them here without redeploying the Solana
//! programs will cause divergence.

// ═══════════════════════════════════════════════════════════════════════════
// BASIS POINTS MATH
// ═══════════════════════════════════════════════════════════════════════════

/// 100% expressed in basis points (1 bps = 0.01%).
pub const BPS_DENOMINATOR: u64 = 10_000;

// ═══════════════════════════════════════════════════════════════════════════
// TOKEN SUPPLY
// ═══════════════════════════════════════════════════════════════════════════

/// Fixed total supply — mint authority permanently disabled.
pub const TOTAL_SUPPLY: u64 = 100_000_000;

/// Allocation: 60% Treasury (contributor rewards via daily emissions).
pub const TREASURY_ALLOCATION: u64 = 60_000_000;

/// Allocation: 15% AMOS Labs entity (10-year lockup, no decay).
pub const ENTITY_ALLOCATION: u64 = 15_000_000;

/// Allocation: 10% Investors (future fundraising rounds).
pub const INVESTOR_ALLOCATION: u64 = 10_000_000;

/// Allocation: 10% Community (airdrops, grants, marketing).
pub const COMMUNITY_ALLOCATION: u64 = 10_000_000;

/// Allocation: 5% Reserve (DAO-controlled emergency fund).
pub const RESERVE_ALLOCATION: u64 = 5_000_000;

// ═══════════════════════════════════════════════════════════════════════════
// REVENUE SPLIT — USDC PAYMENTS
// ═══════════════════════════════════════════════════════════════════════════

/// 50% to token holders (claimable proportionally by stakers).
pub const HOLDER_SHARE_BPS: u64 = 5_000;

/// 40% to R&D multisig (software dev, infrastructure, research).
pub const RND_SHARE_BPS: u64 = 4_000;

/// 5% to operations multisig (accounting, legal, hosting).
pub const OPS_SHARE_BPS: u64 = 500;

/// 5% to treasury reserve (DAO-controlled emergency fund).
pub const RESERVE_SHARE_BPS: u64 = 500;

// ═══════════════════════════════════════════════════════════════════════════
// REVENUE SPLIT — AMOS TOKEN PAYMENTS
// ═══════════════════════════════════════════════════════════════════════════

/// 50% of AMOS payments are permanently burned (deflationary).
pub const AMOS_BURN_BPS: u64 = 5_000;

/// 50% of AMOS payments go to holder pool (stakers claim).
pub const AMOS_HOLDER_BPS: u64 = 5_000;

// ═══════════════════════════════════════════════════════════════════════════
// DECAY PARAMETERS
// ═══════════════════════════════════════════════════════════════════════════

/// Base annual decay rate (10%).
/// Formula: Decay = 10% - (Profit_Ratio * 5%), clamped to [MIN, MAX].
pub const BASE_DECAY_RATE_BPS: u64 = 1_000;

/// Minimum annual decay rate (2%) — during healthy profitability.
pub const MIN_DECAY_RATE_BPS: u64 = 200;

/// Maximum annual decay rate (25%) — during sustained losses.
pub const MAX_DECAY_RATE_BPS: u64 = 2_500;

/// Default annual decay rate before platform economics kick in (5%).
pub const DEFAULT_DECAY_RATE_BPS: u64 = 500;

/// Profit ratio multiplier for decay formula (5% = 500 bps).
pub const DECAY_PROFIT_MULTIPLIER_BPS: u64 = 500;

/// Grace period: 12 months (365 days) of no decay for new stakes.
pub const GRACE_PERIOD_DAYS: u64 = 365;

/// On-chain grace period before decay triggers (90 days inactivity).
pub const ONCHAIN_DECAY_GRACE_PERIOD_DAYS: u64 = 90;

/// Decay floor: minimum 10% of original stake always preserved.
pub const DECAY_FLOOR_BPS: u64 = 1_000;

/// Portion of decayed tokens burned (10%), rest recycled to treasury.
pub const DECAY_BURN_PORTION_BPS: u64 = 1_000;

// ── Tenure-based decay floor progression ────────────────────────────
// Over time, long-term holders get an increasing permanent floor.

/// Year 0-1: 5% permanent floor.
pub const TENURE_FLOOR_YEAR_0_BPS: u64 = 500;
/// Year 1-2: 10% permanent floor.
pub const TENURE_FLOOR_YEAR_1_BPS: u64 = 1_000;
/// Year 2-5: 15% permanent floor.
pub const TENURE_FLOOR_YEAR_2_BPS: u64 = 1_500;
/// Year 5+: 25% permanent floor.
pub const TENURE_FLOOR_YEAR_5_BPS: u64 = 2_500;

// ── Tenure-based decay reduction ────────────────────────────────────
// Long-term holders get a percentage reduction in their decay rate.

/// Year 0-1: 0% reduction (full decay).
pub const TENURE_REDUCTION_YEAR_0_BPS: u64 = 0;
/// Year 1-2: 20% reduction.
pub const TENURE_REDUCTION_YEAR_1_BPS: u64 = 2_000;
/// Year 2-5: 40% reduction.
pub const TENURE_REDUCTION_YEAR_2_BPS: u64 = 4_000;
/// Year 5+: 70% reduction.
pub const TENURE_REDUCTION_YEAR_5_BPS: u64 = 7_000;

// ── Staking vault tiers (lockup bonuses) ────────────────────────────

/// Bronze vault (30-day lock): 20% decay reduction.
pub const VAULT_BRONZE_REDUCTION_BPS: u64 = 2_000;
/// Silver vault (90-day lock): 50% decay reduction.
pub const VAULT_SILVER_REDUCTION_BPS: u64 = 5_000;
/// Gold vault (365-day lock): 80% decay reduction.
pub const VAULT_GOLD_REDUCTION_BPS: u64 = 8_000;
/// Permanent vault (no unlock): 95% decay reduction.
pub const VAULT_PERMANENT_REDUCTION_BPS: u64 = 9_500;

// ═══════════════════════════════════════════════════════════════════════════
// EMISSION / HALVING
// ═══════════════════════════════════════════════════════════════════════════

/// Initial daily emission from treasury: 16,000 AMOS/day.
pub const INITIAL_DAILY_EMISSION: u64 = 16_000;

/// Halving interval: every 365 days (annual).
pub const HALVING_INTERVAL_DAYS: u64 = 365;

/// Minimum daily emission floor: 100 AMOS/day.
pub const MINIMUM_DAILY_EMISSION: u64 = 100;

/// Maximum halving epochs (prevents underflow).
pub const MAX_HALVING_EPOCHS: u64 = 10;

// ═══════════════════════════════════════════════════════════════════════════
// STAKING REQUIREMENTS
// ═══════════════════════════════════════════════════════════════════════════

/// Minimum stake to be eligible for revenue share: 100 AMOS.
pub const MIN_STAKE_AMOUNT: u64 = 100;

/// Minimum days staked before revenue eligibility: 30 days.
pub const MIN_STAKE_DAYS: u64 = 30;

// ═══════════════════════════════════════════════════════════════════════════
// ENTITY LOCKUP (AMOS Labs 10-year commitment)
// ═══════════════════════════════════════════════════════════════════════════

/// Entity lockup: 10 years in seconds.
pub const ENTITY_LOCKUP_SECONDS: i64 = 10 * 365 * 24 * 60 * 60;

/// Entity unlock: 2 years linear after lockup, 8 quarterly tranches.
pub const ENTITY_UNLOCK_SECONDS: i64 = 2 * 365 * 24 * 60 * 60;

/// Number of quarterly unlock tranches.
pub const ENTITY_UNLOCK_TRANCHES: u64 = 8;

// ═══════════════════════════════════════════════════════════════════════════
// BOUNTY SYSTEM
// ═══════════════════════════════════════════════════════════════════════════

/// Minimum quality score (0-100) for bounty approval.
pub const MIN_QUALITY_SCORE: u8 = 30;

/// Maximum points per single bounty.
pub const MAX_BOUNTY_POINTS: u64 = 2_000;

/// Maximum bounties per operator per day (on-chain enforcement).
pub const MAX_DAILY_BOUNTIES_PER_OPERATOR: u64 = 50;

/// Reviewer reward: 5% of bounty tokens go to the human reviewer.
pub const REVIEWER_REWARD_BPS: u64 = 500;

// ── Contribution type multipliers ───────────────────────────────────

/// Bug fix: 120% (bonus for fixing).
pub const MULTIPLIER_BUG_FIX_BPS: u64 = 12_000;
/// Feature development: 100% (baseline).
pub const MULTIPLIER_FEATURE_BPS: u64 = 10_000;
/// Documentation: 80%.
pub const MULTIPLIER_DOCS_BPS: u64 = 8_000;
/// Content/Marketing: 90%.
pub const MULTIPLIER_CONTENT_BPS: u64 = 9_000;
/// Support: 70%.
pub const MULTIPLIER_SUPPORT_BPS: u64 = 7_000;
/// Testing/QA: 110% (bonus for quality).
pub const MULTIPLIER_TESTING_BPS: u64 = 11_000;
/// Design: 100%.
pub const MULTIPLIER_DESIGN_BPS: u64 = 10_000;
/// Infrastructure: 130% (highest — core platform).
pub const MULTIPLIER_INFRA_BPS: u64 = 13_000;

// ═══════════════════════════════════════════════════════════════════════════
// LP INCENTIVES
// ═══════════════════════════════════════════════════════════════════════════

/// Total LP incentive allocation: 3% of supply.
pub const LP_INCENTIVE_ALLOCATION: u64 = 3_000_000;
/// Year 1: 1,500,000 AMOS.
pub const LP_INCENTIVE_YEAR_1: u64 = 1_500_000;
/// Year 2: 1,000,000 AMOS.
pub const LP_INCENTIVE_YEAR_2: u64 = 1_000_000;
/// Year 3: 500,000 AMOS.
pub const LP_INCENTIVE_YEAR_3: u64 = 500_000;

/// Founder LP permanent fee: 0.05% of all trades forever.
pub const FOUNDER_LP_FEE_BPS: u64 = 5;

// ═══════════════════════════════════════════════════════════════════════════
// PAYMENT DISCOUNTS
// ═══════════════════════════════════════════════════════════════════════════

/// USDC direct payment discount: 5%.
pub const USDC_DISCOUNT_BPS: u64 = 500;
/// AMOS token payment discount: 20% (matches on-chain AMOS_PAYMENT_DISCOUNT_BPS).
pub const AMOS_DISCOUNT_BPS: u64 = 2_000;

// ═══════════════════════════════════════════════════════════════════════════
// COMPILE-TIME VERIFICATION
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supply_allocations_sum_to_total() {
        assert_eq!(
            TREASURY_ALLOCATION
                + ENTITY_ALLOCATION
                + INVESTOR_ALLOCATION
                + COMMUNITY_ALLOCATION
                + RESERVE_ALLOCATION,
            TOTAL_SUPPLY,
            "Allocations must sum to 100M"
        );
    }

    #[test]
    fn usdc_revenue_splits_sum_to_100_percent() {
        assert_eq!(
            HOLDER_SHARE_BPS + RND_SHARE_BPS + OPS_SHARE_BPS + RESERVE_SHARE_BPS,
            BPS_DENOMINATOR,
            "USDC revenue splits must sum to 10000 bps"
        );
    }

    #[test]
    fn amos_payment_splits_sum_to_100_percent() {
        assert_eq!(
            AMOS_BURN_BPS + AMOS_HOLDER_BPS,
            BPS_DENOMINATOR,
            "AMOS payment splits must sum to 10000 bps"
        );
    }

    #[test]
    fn decay_range_is_valid() {
        assert!(MIN_DECAY_RATE_BPS < MAX_DECAY_RATE_BPS);
        assert!(DEFAULT_DECAY_RATE_BPS >= MIN_DECAY_RATE_BPS);
        assert!(DEFAULT_DECAY_RATE_BPS <= MAX_DECAY_RATE_BPS);
    }

    #[test]
    fn lp_incentives_sum_to_allocation() {
        assert_eq!(
            LP_INCENTIVE_YEAR_1 + LP_INCENTIVE_YEAR_2 + LP_INCENTIVE_YEAR_3,
            LP_INCENTIVE_ALLOCATION,
        );
    }

    #[test]
    fn tenure_floors_are_progressive() {
        assert!(TENURE_FLOOR_YEAR_0_BPS < TENURE_FLOOR_YEAR_1_BPS);
        assert!(TENURE_FLOOR_YEAR_1_BPS < TENURE_FLOOR_YEAR_2_BPS);
        assert!(TENURE_FLOOR_YEAR_2_BPS < TENURE_FLOOR_YEAR_5_BPS);
    }

    #[test]
    fn tenure_reductions_are_progressive() {
        assert!(TENURE_REDUCTION_YEAR_0_BPS < TENURE_REDUCTION_YEAR_1_BPS);
        assert!(TENURE_REDUCTION_YEAR_1_BPS < TENURE_REDUCTION_YEAR_2_BPS);
        assert!(TENURE_REDUCTION_YEAR_2_BPS < TENURE_REDUCTION_YEAR_5_BPS);
    }
}
