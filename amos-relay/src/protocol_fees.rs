//! Protocol fee calculations and distribution.

use serde::{Deserialize, Serialize};

/// Protocol fee percentage in basis points (3% = 300 bps).
pub const PROTOCOL_FEE_BPS: u64 = 300;

/// Holder pool share of net revenue in basis points (70% = 7000 bps).
pub const HOLDER_SHARE_BPS: u64 = 7000;

/// Treasury share of net revenue in basis points (20% = 2000 bps).
pub const TREASURY_SHARE_BPS: u64 = 2000;

/// Operations/burn share of net revenue in basis points (10% = 1000 bps).
pub const OPS_BURN_SHARE_BPS: u64 = 1000;

/// Total basis points (100% = 10000 bps).
const TOTAL_BPS: u64 = 10000;

/// Protocol fee breakdown.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ProtocolFee {
    /// Total protocol fee (3% of reward).
    pub total_fee: u64,
    /// Amount allocated to holder pool (70% of fee).
    pub holder_share: u64,
    /// Amount allocated to treasury (20% of fee).
    pub treasury_share: u64,
    /// Amount allocated to ops/burn (10% of fee).
    pub ops_burn_share: u64,
}

/// Calculate protocol fee and distribution breakdown.
///
/// # Arguments
/// * `reward_tokens` - Total reward amount in tokens
///
/// # Returns
/// Protocol fee breakdown with total fee and distribution shares
///
/// # Example
/// ```
/// use amos_relay::protocol_fees::calculate_fee;
///
/// let fee = calculate_fee(1000);
/// assert_eq!(fee.total_fee, 30); // 3% of 1000
/// assert_eq!(fee.holder_share, 21); // 70% of 30
/// assert_eq!(fee.treasury_share, 6); // 20% of 30
/// assert_eq!(fee.ops_burn_share, 3); // 10% of 30
/// ```
pub fn calculate_fee(reward_tokens: u64) -> ProtocolFee {
    // Calculate total protocol fee (3%)
    let total_fee = (reward_tokens * PROTOCOL_FEE_BPS) / TOTAL_BPS;

    // Distribute the fee according to shares
    let holder_share = (total_fee * HOLDER_SHARE_BPS) / TOTAL_BPS;
    let treasury_share = (total_fee * TREASURY_SHARE_BPS) / TOTAL_BPS;
    let ops_burn_share = (total_fee * OPS_BURN_SHARE_BPS) / TOTAL_BPS;

    ProtocolFee {
        total_fee,
        holder_share,
        treasury_share,
        ops_burn_share,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_fee_basic() {
        let fee = calculate_fee(1000);
        assert_eq!(fee.total_fee, 30); // 3% of 1000
        assert_eq!(fee.holder_share, 21); // 70% of 30
        assert_eq!(fee.treasury_share, 6); // 20% of 30
        assert_eq!(fee.ops_burn_share, 3); // 10% of 30
    }

    #[test]
    fn test_calculate_fee_large_amount() {
        let fee = calculate_fee(1_000_000);
        assert_eq!(fee.total_fee, 30_000); // 3% of 1M
        assert_eq!(fee.holder_share, 21_000); // 70% of 30k
        assert_eq!(fee.treasury_share, 6_000); // 20% of 30k
        assert_eq!(fee.ops_burn_share, 3_000); // 10% of 30k
    }

    #[test]
    fn test_calculate_fee_zero() {
        let fee = calculate_fee(0);
        assert_eq!(fee.total_fee, 0);
        assert_eq!(fee.holder_share, 0);
        assert_eq!(fee.treasury_share, 0);
        assert_eq!(fee.ops_burn_share, 0);
    }

    #[test]
    fn test_calculate_fee_rounding() {
        // Test that rounding doesn't cause overflow
        let fee = calculate_fee(100);
        assert_eq!(fee.total_fee, 3); // 3% of 100
        assert_eq!(fee.holder_share, 2); // 70% of 3 = 2.1, rounds down to 2
        assert_eq!(fee.treasury_share, 0); // 20% of 3 = 0.6, rounds down to 0
        assert_eq!(fee.ops_burn_share, 0); // 10% of 3 = 0.3, rounds down to 0
    }

    #[test]
    fn test_fee_distribution_percentages() {
        // Verify the distribution adds up to ~100% (accounting for rounding)
        let reward = 10_000;
        let fee = calculate_fee(reward);

        // Total fee should be 3%
        assert_eq!(fee.total_fee, 300);

        // Distribution should sum to total fee (or very close due to rounding)
        let sum = fee.holder_share + fee.treasury_share + fee.ops_burn_share;
        assert!(
            sum <= fee.total_fee && sum >= fee.total_fee - 2,
            "Distribution sum {} should be close to total fee {}",
            sum,
            fee.total_fee
        );
    }

    #[test]
    fn test_protocol_fee_percentage() {
        // Verify 3% fee is correct
        assert_eq!(PROTOCOL_FEE_BPS, 300);
        assert_eq!(PROTOCOL_FEE_BPS as f64 / TOTAL_BPS as f64, 0.03);
    }

    #[test]
    fn test_distribution_shares_sum_to_100() {
        // Verify holder + treasury + ops shares = 100%
        let total = HOLDER_SHARE_BPS + TREASURY_SHARE_BPS + OPS_BURN_SHARE_BPS;
        assert_eq!(total, TOTAL_BPS);
    }

    #[test]
    fn test_holder_share_percentage() {
        assert_eq!(HOLDER_SHARE_BPS as f64 / TOTAL_BPS as f64, 0.70);
    }

    #[test]
    fn test_treasury_share_percentage() {
        assert_eq!(TREASURY_SHARE_BPS as f64 / TOTAL_BPS as f64, 0.20);
    }

    #[test]
    fn test_ops_burn_share_percentage() {
        assert_eq!(OPS_BURN_SHARE_BPS as f64 / TOTAL_BPS as f64, 0.10);
    }
}
