//! # Revenue Distribution Engine
//!
//! Implements the immutable AMOS-only revenue split from the treasury program:
//!
//! **Protocol Fee Distribution (from commercial bounties):**
//!   50% → staked token holders (proportional to stake)
//!   40% → permanently burned (deflationary)
//!   10% → AMOS Labs operating wallet
//!
//! This module provides the off-chain calculation. The on-chain program
//! enforces the same math with identical constants.

use super::economics::*;
use crate::error::{AmosError, Result};

/// Breakdown of an AMOS protocol fee distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolFeeDistribution {
    /// Total fee amount in AMOS tokens.
    pub total_amount: u64,
    /// 50% to holder pool (claimable by stakers).
    pub holder_amount: u64,
    /// 40% permanently burned.
    pub burn_amount: u64,
    /// 10% to Labs wallet (remainder absorbs rounding).
    pub labs_amount: u64,
}

/// Individual holder's revenue share claim.
#[derive(Debug, Clone)]
pub struct HolderClaim {
    /// Holder's staked AMOS amount.
    pub stake_amount: u64,
    /// Total eligible stake across all holders.
    pub total_eligible_stake: u64,
    /// Available pool balance (AMOS).
    pub pool_balance: u64,
    /// Calculated payout.
    pub payout: u64,
    /// Share in basis points.
    pub share_bps: u64,
}

/// Calculate the AMOS protocol fee split (50/40/10).
///
/// Uses checked arithmetic to match on-chain behavior exactly.
/// Labs gets the remainder to absorb any rounding dust.
pub fn split_protocol_fee(amount: u64) -> Result<ProtocolFeeDistribution> {
    if amount == 0 {
        return Err(AmosError::Validation("Amount must be > 0".into()));
    }

    let holder_amount = checked_bps_mul(amount, FEE_HOLDER_SHARE_BPS)?;
    let burn_amount = checked_bps_mul(amount, FEE_BURN_SHARE_BPS)?;

    // Labs gets remainder (handles rounding dust).
    let labs_amount = amount
        .checked_sub(holder_amount)
        .and_then(|v| v.checked_sub(burn_amount))
        .ok_or(AmosError::ArithmeticOverflow {
            context: "Protocol fee split remainder".into(),
        })?;

    Ok(ProtocolFeeDistribution {
        total_amount: amount,
        holder_amount,
        burn_amount,
        labs_amount,
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
    fn protocol_fee_split_adds_up() {
        let dist = split_protocol_fee(1_000_000).unwrap();
        assert_eq!(
            dist.holder_amount + dist.burn_amount + dist.labs_amount,
            1_000_000
        );
        assert_eq!(dist.holder_amount, 500_000); // 50%
        assert_eq!(dist.burn_amount, 400_000); // 40%
        assert_eq!(dist.labs_amount, 100_000); // 10%
    }

    #[test]
    fn odd_amount_rounding_goes_to_labs() {
        // 999 AMOS: 499 holder, 399 burn, 101 labs (absorbs dust)
        let dist = split_protocol_fee(999).unwrap();
        assert_eq!(
            dist.holder_amount + dist.burn_amount + dist.labs_amount,
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
        assert!(split_protocol_fee(0).is_err());
    }

    #[test]
    fn insufficient_stake_errors() {
        let result = calculate_holder_claim(50, 10_000, 50_000);
        assert!(matches!(result, Err(AmosError::InsufficientStake { .. })));
    }
}
