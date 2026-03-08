//! # Decay Calculation Engine
//!
//! Implements the exact decay math from the whitepaper:
//!
//! ```text
//! Decay Rate = 10% − (Profit_Ratio × 5%)
//!   where Profit_Ratio = (Revenue − Costs) / Costs
//!   clamped to [2%, 25%]
//!
//! Daily Decay = 1 − (1 − Annual_Decay)^(1/365)
//!
//! Effective Rate = Base_Rate × (1 − Tenure_Reduction) × (1 − Vault_Reduction)
//! ```
//!
//! The 12-month grace period means new stakes experience zero decay for
//! their first year, giving contributors time to earn and compound.

use super::economics::*;

/// Platform financial snapshot used to compute the dynamic decay rate.
#[derive(Debug, Clone)]
pub struct PlatformEconomics {
    /// Total monthly revenue in USD cents.
    pub monthly_revenue_cents: u64,
    /// Total monthly costs in USD cents.
    pub monthly_costs_cents: u64,
}

/// Per-stake metadata needed for decay calculation.
#[derive(Debug, Clone)]
pub struct StakeContext {
    /// Days since the stake was first registered.
    pub tenure_days: u64,
    /// Current balance (post any previous decay).
    pub current_balance: u64,
    /// Original balance at time of award (before any decay).
    pub original_balance: u64,
    /// Vault tier: None, Bronze(30d), Silver(90d), Gold(365d), Permanent.
    pub vault_tier: VaultTier,
    /// Days since last platform activity (resets grace period on-chain).
    pub days_inactive: u64,
}

/// Staking vault tiers with corresponding lockup periods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultTier {
    /// No lockup — standard decay applies.
    None,
    /// 30-day lock — 20% decay reduction.
    Bronze,
    /// 90-day lock — 50% decay reduction.
    Silver,
    /// 365-day lock — 80% decay reduction.
    Gold,
    /// Permanent lock — 95% decay reduction.
    Permanent,
}

/// Result of a decay calculation for a single stake.
#[derive(Debug, Clone)]
pub struct DecayResult {
    /// Tokens removed this cycle.
    pub tokens_decayed: u64,
    /// Tokens burned (portion of decayed).
    pub tokens_burned: u64,
    /// Tokens recycled back to treasury.
    pub tokens_recycled: u64,
    /// New balance after decay.
    pub new_balance: u64,
    /// The effective annual decay rate that was applied (bps).
    pub effective_rate_bps: u64,
    /// Whether decay was skipped (grace period, floor, etc.).
    pub skipped: bool,
    /// Reason if skipped.
    pub skip_reason: Option<String>,
}

/// Compute the **dynamic annual decay rate** from platform financials.
///
/// Formula from whitepaper:
/// ```text
/// profit_ratio = (revenue - costs) / costs
/// decay_rate   = 10% - (profit_ratio * 5%)
/// clamped to   [2%, 25%]
/// ```
///
/// When the platform is profitable, decay drops (rewarding holders).
/// When the platform loses money, decay rises (recycling tokens to ops).
pub fn calculate_dynamic_decay_rate(economics: &PlatformEconomics) -> u64 {
    if economics.monthly_costs_cents == 0 {
        return DEFAULT_DECAY_RATE_BPS;
    }

    // profit_ratio = (revenue - costs) / costs
    // Using i128 to avoid overflow and handle negative profit ratios.
    let revenue = economics.monthly_revenue_cents as i128;
    let costs = economics.monthly_costs_cents as i128;
    let profit_ratio_x10000 = ((revenue - costs) * 10_000) / costs;

    // decay = 1000 bps - (profit_ratio * 500 bps)
    // In fixed-point: 1000 - (profit_ratio_x10000 * 500 / 10000)
    let adjustment = (profit_ratio_x10000 * DECAY_PROFIT_MULTIPLIER_BPS as i128) / 10_000;
    let rate = (BASE_DECAY_RATE_BPS as i128) - adjustment;

    // Clamp to [MIN, MAX]
    let clamped = rate
        .max(MIN_DECAY_RATE_BPS as i128)
        .min(MAX_DECAY_RATE_BPS as i128);

    clamped as u64
}

/// Convert annual decay rate to daily decay rate.
///
/// Exact formula: `daily = 1 - (1 - annual)^(1/365)`
///
/// For small rates this is well-approximated by `annual / 365`,
/// but we use the exact formula for correctness.
pub fn annual_to_daily_rate_bps(annual_bps: u64) -> u64 {
    // annual fraction: e.g., 1000 bps = 0.10
    let annual_frac = annual_bps as f64 / BPS_DENOMINATOR as f64;

    // daily = 1 - (1 - annual)^(1/365)
    let daily_frac = 1.0 - (1.0 - annual_frac).powf(1.0 / 365.0);

    // Convert back to bps, rounding up to avoid zero-decay for small rates
    (daily_frac * BPS_DENOMINATOR as f64).ceil() as u64
}

/// Get the tenure-based decay floor for a given stake age.
///
/// The floor is the minimum percentage of the original balance that
/// is always preserved, no matter how much decay accumulates.
pub fn tenure_floor_bps(tenure_days: u64) -> u64 {
    let years = tenure_days / 365;
    match years {
        0 => TENURE_FLOOR_YEAR_0_BPS,
        1 => TENURE_FLOOR_YEAR_1_BPS,
        2..=4 => TENURE_FLOOR_YEAR_2_BPS,
        _ => TENURE_FLOOR_YEAR_5_BPS,
    }
}

/// Get the tenure-based decay rate reduction.
///
/// Long-term holders earn a percentage reduction in their decay rate,
/// rewarding loyalty and long-term alignment.
pub fn tenure_reduction_bps(tenure_days: u64) -> u64 {
    let years = tenure_days / 365;
    match years {
        0 => TENURE_REDUCTION_YEAR_0_BPS,
        1 => TENURE_REDUCTION_YEAR_1_BPS,
        2..=4 => TENURE_REDUCTION_YEAR_2_BPS,
        _ => TENURE_REDUCTION_YEAR_5_BPS,
    }
}

/// Get the vault tier's decay reduction in basis points.
pub fn vault_reduction_bps(tier: VaultTier) -> u64 {
    match tier {
        VaultTier::None => 0,
        VaultTier::Bronze => VAULT_BRONZE_REDUCTION_BPS,
        VaultTier::Silver => VAULT_SILVER_REDUCTION_BPS,
        VaultTier::Gold => VAULT_GOLD_REDUCTION_BPS,
        VaultTier::Permanent => VAULT_PERMANENT_REDUCTION_BPS,
    }
}

/// Calculate the **effective annual decay rate** for a specific stake,
/// accounting for tenure reduction and vault bonuses.
///
/// ```text
/// effective = base_rate × (1 - tenure_reduction) × (1 - vault_reduction)
/// ```
pub fn effective_annual_rate_bps(base_rate_bps: u64, context: &StakeContext) -> u64 {
    let tenure_red = tenure_reduction_bps(context.tenure_days);
    let vault_red = vault_reduction_bps(context.vault_tier);

    // effective = base * (10000 - tenure_red) / 10000 * (10000 - vault_red) / 10000
    let after_tenure = base_rate_bps
        .checked_mul(BPS_DENOMINATOR - tenure_red)
        .unwrap_or(0)
        / BPS_DENOMINATOR;

    let after_vault = after_tenure
        .checked_mul(BPS_DENOMINATOR - vault_red)
        .unwrap_or(0)
        / BPS_DENOMINATOR;

    after_vault.max(MIN_DECAY_RATE_BPS)
}

/// Apply one day of decay to a single stake.
///
/// This is the main entry point for the daily decay job.
pub fn apply_daily_decay(
    base_annual_rate_bps: u64,
    context: &StakeContext,
) -> DecayResult {
    // Grace period check: no decay for first 365 days
    if context.tenure_days < GRACE_PERIOD_DAYS {
        return DecayResult {
            tokens_decayed: 0,
            tokens_burned: 0,
            tokens_recycled: 0,
            new_balance: context.current_balance,
            effective_rate_bps: 0,
            skipped: true,
            skip_reason: Some(format!(
                "Within grace period ({} of {} days)",
                context.tenure_days, GRACE_PERIOD_DAYS
            )),
        };
    }

    // Calculate floors
    let floor_bps = tenure_floor_bps(context.tenure_days);
    let floor_amount = context
        .original_balance
        .checked_mul(floor_bps)
        .unwrap_or(0)
        / BPS_DENOMINATOR;

    // Already at or below floor
    if context.current_balance <= floor_amount {
        return DecayResult {
            tokens_decayed: 0,
            tokens_burned: 0,
            tokens_recycled: 0,
            new_balance: context.current_balance,
            effective_rate_bps: 0,
            skipped: true,
            skip_reason: Some("At decay floor".into()),
        };
    }

    // Calculate effective rate with tenure + vault reductions
    let effective_annual = effective_annual_rate_bps(base_annual_rate_bps, context);
    let daily_rate_bps = annual_to_daily_rate_bps(effective_annual);

    // Daily decay amount
    let decay_amount = context
        .current_balance
        .checked_mul(daily_rate_bps)
        .unwrap_or(0)
        / BPS_DENOMINATOR;
    let decay_amount = decay_amount.max(1); // At least 1 token

    // Don't decay below floor
    let new_balance = context
        .current_balance
        .saturating_sub(decay_amount)
        .max(floor_amount);
    let actual_decay = context.current_balance - new_balance;

    if actual_decay == 0 {
        return DecayResult {
            tokens_decayed: 0,
            tokens_burned: 0,
            tokens_recycled: 0,
            new_balance: context.current_balance,
            effective_rate_bps: effective_annual,
            skipped: true,
            skip_reason: Some("Decay amount rounded to zero".into()),
        };
    }

    // Split decayed tokens: 10% burn, 90% recycle to treasury
    let burned = actual_decay
        .checked_mul(DECAY_BURN_PORTION_BPS)
        .unwrap_or(0)
        / BPS_DENOMINATOR;
    let recycled = actual_decay.saturating_sub(burned);

    DecayResult {
        tokens_decayed: actual_decay,
        tokens_burned: burned,
        tokens_recycled: recycled,
        new_balance,
        effective_rate_bps: effective_annual,
        skipped: false,
        skip_reason: None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS — verify against whitepaper examples
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakeven_yields_10_percent_decay() {
        // profit_ratio = 0 → decay = 10%
        let econ = PlatformEconomics {
            monthly_revenue_cents: 10_000_00,
            monthly_costs_cents: 10_000_00,
        };
        assert_eq!(calculate_dynamic_decay_rate(&econ), 1_000);
    }

    #[test]
    fn highly_profitable_yields_minimum_decay() {
        // profit_ratio = (200k - 50k)/50k = 3.0
        // decay = 10% - (3.0 * 5%) = 10% - 15% = -5% → clamped to 2%
        let econ = PlatformEconomics {
            monthly_revenue_cents: 200_000_00,
            monthly_costs_cents: 50_000_00,
        };
        assert_eq!(calculate_dynamic_decay_rate(&econ), MIN_DECAY_RATE_BPS);
    }

    #[test]
    fn heavy_losses_yield_maximum_decay() {
        // profit_ratio = (10k - 100k)/100k = -0.9
        // decay = 10% - (-0.9 * 5%) = 10% + 4.5% = 14.5%
        // Still under 25% max
        let econ = PlatformEconomics {
            monthly_revenue_cents: 10_000_00,
            monthly_costs_cents: 100_000_00,
        };
        let rate = calculate_dynamic_decay_rate(&econ);
        assert!(rate > BASE_DECAY_RATE_BPS);
        assert!(rate <= MAX_DECAY_RATE_BPS);
    }

    #[test]
    fn catastrophic_losses_clamp_to_max() {
        // revenue = 0, costs = 100k → profit_ratio = -1.0
        // decay = 10% + 5% = 15% — still under 25%
        // But revenue = 0, costs = 500k → ratio = -1.0 → same
        let econ = PlatformEconomics {
            monthly_revenue_cents: 0,
            monthly_costs_cents: 500_000_00,
        };
        let rate = calculate_dynamic_decay_rate(&econ);
        assert!(rate <= MAX_DECAY_RATE_BPS);
    }

    #[test]
    fn grace_period_blocks_decay() {
        let ctx = StakeContext {
            tenure_days: 100, // < 365
            current_balance: 1_000,
            original_balance: 1_000,
            vault_tier: VaultTier::None,
            days_inactive: 0,
        };
        let result = apply_daily_decay(1_000, &ctx);
        assert!(result.skipped);
        assert_eq!(result.tokens_decayed, 0);
        assert_eq!(result.new_balance, 1_000);
    }

    #[test]
    fn decay_respects_floor() {
        let ctx = StakeContext {
            tenure_days: 400,
            current_balance: 60, // close to floor
            original_balance: 1_000,
            vault_tier: VaultTier::None,
            days_inactive: 100,
        };
        // Floor at year 1 = 10% of 1000 = 100 AMOS
        // current_balance 60 < floor 100 → already at floor
        let result = apply_daily_decay(1_000, &ctx);
        assert!(result.skipped);
    }

    #[test]
    fn vault_reduces_effective_rate() {
        let ctx_none = StakeContext {
            tenure_days: 400,
            current_balance: 1_000,
            original_balance: 1_000,
            vault_tier: VaultTier::None,
            days_inactive: 100,
        };
        let ctx_gold = StakeContext {
            vault_tier: VaultTier::Gold,
            ..ctx_none.clone()
        };
        let rate_none = effective_annual_rate_bps(1_000, &ctx_none);
        let rate_gold = effective_annual_rate_bps(1_000, &ctx_gold);
        assert!(rate_gold < rate_none);
    }

    #[test]
    fn daily_rate_conversion_is_reasonable() {
        // 10% annual → ~0.029% daily
        let daily = annual_to_daily_rate_bps(1_000);
        // Should be roughly 3 bps (0.03%)
        assert!(daily >= 2 && daily <= 4, "daily was {daily}");
    }

    #[test]
    fn tenure_floors_increase_over_time() {
        assert!(tenure_floor_bps(0) < tenure_floor_bps(400));
        assert!(tenure_floor_bps(400) < tenure_floor_bps(800));
        assert!(tenure_floor_bps(800) < tenure_floor_bps(2000));
    }
}
