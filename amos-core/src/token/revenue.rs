//! # Revenue Distribution Engine
//!
//! Implements the immutable revenue split from the treasury program:
//!
//! **USDC payments:**
//!   50% → token holders, 40% → R&D, 5% → ops, 5% → reserve
//!
//! **AMOS payments:**
//!   50% → burned, 50% → token holders
//!
//! This module provides the off-chain calculation. The on-chain program
//! enforces the same math with identical constants.

use super::economics::*;
use crate::error::{AmosError, Result};

/// Breakdown of a USDC revenue distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsdcRevenueDistribution {
    /// Total amount received.
    pub total_amount: u64,
    /// 50% to holder pool (claimable by stakers).
    pub holder_amount: u64,
    /// 40% to R&D multisig.
    pub rnd_amount: u64,
    /// 5% to operations multisig.
    pub ops_amount: u64,
    /// 5% to reserve (remainder absorbs rounding).
    pub reserve_amount: u64,
}

/// Breakdown of an AMOS token payment distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmosPaymentDistribution {
    /// Total AMOS received.
    pub total_amount: u64,
    /// 50% permanently burned.
    pub burn_amount: u64,
    /// 50% to holder pool (remainder absorbs rounding).
    pub holder_amount: u64,
}

/// Individual holder's revenue share claim.
#[derive(Debug, Clone)]
pub struct HolderClaim {
    /// Holder's staked AMOS amount.
    pub stake_amount: u64,
    /// Total eligible stake across all holders.
    pub total_eligible_stake: u64,
    /// Available pool balance (USDC).
    pub pool_balance: u64,
    /// Calculated payout.
    pub payout: u64,
    /// Share in basis points.
    pub share_bps: u64,
}

/// Calculate the USDC revenue split.
///
/// Uses checked arithmetic to match on-chain behavior exactly.
/// Reserve gets the remainder to absorb any rounding dust.
pub fn split_usdc_revenue(amount: u64) -> Result<UsdcRevenueDistribution> {
    if amount == 0 {
        return Err(AmosError::Validation("Amount must be > 0".into()));
    }

    let holder_amount = checked_bps_mul(amount, HOLDER_SHARE_BPS)?;
    let rnd_amount = checked_bps_mul(amount, RND_SHARE_BPS)?;
    let ops_amount = checked_bps_mul(amount, OPS_SHARE_BPS)?;

    // Reserve gets remainder (handles rounding dust).
    let reserve_amount = amount
        .checked_sub(holder_amount)
        .and_then(|v| v.checked_sub(rnd_amount))
        .and_then(|v| v.checked_sub(ops_amount))
        .ok_or(AmosError::ArithmeticOverflow {
            context: "USDC revenue split remainder".into(),
        })?;

    Ok(UsdcRevenueDistribution {
        total_amount: amount,
        holder_amount,
        rnd_amount,
        ops_amount,
        reserve_amount,
    })
}

/// Calculate the AMOS token payment split.
///
/// Holder gets remainder to absorb rounding dust.
pub fn split_amos_payment(amount: u64) -> Result<AmosPaymentDistribution> {
    if amount == 0 {
        return Err(AmosError::Validation("Amount must be > 0".into()));
    }

    let burn_amount = checked_bps_mul(amount, AMOS_BURN_BPS)?;
    let holder_amount = amount
        .checked_sub(burn_amount)
        .ok_or(AmosError::ArithmeticOverflow {
            context: "AMOS payment split remainder".into(),
        })?;

    Ok(AmosPaymentDistribution {
        total_amount: amount,
        burn_amount,
        holder_amount,
    })
}

/// Calculate a holder's proportional revenue claim.
///
/// ```text
/// share = (stake / total_stake)
/// payout = pool_balance × share
/// ```
pub fn calculate_holder_claim(
    stake_amount: u64,
    total_eligible_stake: u64,
    pool_balance: u64,
) -> Result<HolderClaim> {
    if total_eligible_stake == 0 {
        return Err(AmosError::NoRevenueToClaim);
    }
    if stake_amount < MIN_STAKE_AMOUNT {
        return Err(AmosError::InsufficientStake {
            have: stake_amount,
            need: MIN_STAKE_AMOUNT,
        });
    }

    let share_bps = stake_amount
        .checked_mul(BPS_DENOMINATOR)
        .ok_or(AmosError::ArithmeticOverflow {
            context: "holder share calculation".into(),
        })?
        .checked_div(total_eligible_stake)
        .ok_or(AmosError::ArithmeticOverflow {
            context: "holder share division".into(),
        })?;

    let payout = pool_balance
        .checked_mul(share_bps)
        .ok_or(AmosError::ArithmeticOverflow {
            context: "holder payout calculation".into(),
        })?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(AmosError::ArithmeticOverflow {
            context: "holder payout division".into(),
        })?;

    Ok(HolderClaim {
        stake_amount,
        total_eligible_stake,
        pool_balance,
        payout,
        share_bps,
    })
}

/// Checked basis-points multiplication: `amount * bps / 10000`.
fn checked_bps_mul(amount: u64, bps: u64) -> Result<u64> {
    amount
        .checked_mul(bps)
        .and_then(|v| v.checked_div(BPS_DENOMINATOR))
        .ok_or(AmosError::ArithmeticOverflow {
            context: format!("bps mul: {amount} * {bps} / {BPS_DENOMINATOR}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usdc_split_adds_up() {
        let dist = split_usdc_revenue(1_000_000).unwrap();
        assert_eq!(
            dist.holder_amount + dist.rnd_amount + dist.ops_amount + dist.reserve_amount,
            1_000_000
        );
        assert_eq!(dist.holder_amount, 500_000); // 50%
        assert_eq!(dist.rnd_amount, 400_000); // 40%
        assert_eq!(dist.ops_amount, 50_000); // 5%
        assert_eq!(dist.reserve_amount, 50_000); // 5%
    }

    #[test]
    fn amos_split_adds_up() {
        let dist = split_amos_payment(1_000_000).unwrap();
        assert_eq!(dist.burn_amount + dist.holder_amount, 1_000_000);
        assert_eq!(dist.burn_amount, 500_000); // 50%
        assert_eq!(dist.holder_amount, 500_000); // 50%
    }

    #[test]
    fn odd_amount_rounding_goes_to_remainder() {
        // 999 USDC: 499 holder, 399 rnd, 49 ops, 52 reserve (absorbs dust)
        let dist = split_usdc_revenue(999).unwrap();
        assert_eq!(
            dist.holder_amount + dist.rnd_amount + dist.ops_amount + dist.reserve_amount,
            999
        );
    }

    #[test]
    fn holder_claim_proportional() {
        let claim = calculate_holder_claim(1_000, 10_000, 50_000).unwrap();
        assert_eq!(claim.share_bps, 1_000); // 10%
        assert_eq!(claim.payout, 5_000); // 10% of 50k
    }

    #[test]
    fn zero_amount_errors() {
        assert!(split_usdc_revenue(0).is_err());
        assert!(split_amos_payment(0).is_err());
    }

    #[test]
    fn insufficient_stake_errors() {
        let result = calculate_holder_claim(50, 10_000, 50_000);
        assert!(matches!(result, Err(AmosError::InsufficientStake { .. })));
    }
}
