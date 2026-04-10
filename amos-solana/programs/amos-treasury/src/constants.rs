/// AMOS Treasury Constants
///
/// These constants define the immutable rules of the AMOS Treasury system.
/// Revenue distribution percentages are hardcoded to ensure trust and transparency.

use anchor_lang::prelude::*;

// ============================================================================
// Revenue Distribution - USDC Revenue Split (basis points, 100 = 1%)
// ============================================================================

/// Holder share of USDC revenue: 50%
/// This allocation goes to the holder pool for proportional distribution
pub const HOLDER_SHARE_BPS: u16 = 5000;

/// R&D multisig share of USDC revenue: 40%
/// Funds product development and innovation
pub const RND_SHARE_BPS: u16 = 4000;

/// Operations multisig share of USDC revenue: 5%
/// Covers operational expenses
pub const OPS_SHARE_BPS: u16 = 500;

/// Reserve share of USDC revenue: 5%
/// Emergency fund and strategic reserves
/// Also receives rounding remainders to ensure exact distribution
pub const RESERVE_SHARE_BPS: u16 = 500;

// ============================================================================
// AMOS Token Payment Distribution (basis points)
// ============================================================================

/// AMOS burn share when AMOS tokens are used for payment: 50%
/// Deflationary mechanism
pub const AMOS_BURN_BPS: u16 = 5000;

/// AMOS holder share when AMOS tokens are used for payment: 50%
/// Additional rewards for token holders
pub const AMOS_HOLDER_BPS: u16 = 5000;

// ============================================================================
// Staking Requirements
// ============================================================================

/// Minimum stake period in days before claiming revenue
/// Prevents gaming the system with short-term stakes
pub const MIN_STAKE_DAYS: u64 = 30;

/// Minimum AMOS tokens required to register a stake
pub const MIN_STAKE_AMOUNT: u64 = 100;

// ============================================================================
// Payment Discounts
// ============================================================================

/// Discount for paying with AMOS tokens instead of USDC: 20%
pub const AMOS_PAYMENT_DISCOUNT_BPS: u16 = 2000;

/// Discount for annual subscription: 15%
pub const ANNUAL_SUBSCRIPTION_DISCOUNT_BPS: u16 = 1500;

// ============================================================================
// Basis Points Denominator
// ============================================================================

/// Denominator for all basis point calculations
/// 10000 basis points = 100%
pub const BPS_DENOMINATOR: u16 = 10000;

// ============================================================================
// PDA Seeds Module
// ============================================================================

pub mod seeds {
    /// Seed for treasury config PDA
    pub const TREASURY_CONFIG: &[u8] = b"treasury_config";

    /// Seed for stake record PDA
    pub const STAKE_RECORD: &[u8] = b"stake_record";

    /// Seed for distribution record PDA
    pub const DISTRIBUTION: &[u8] = b"distribution";

    /// Seed for holder pool PDA
    pub const HOLDER_POOL: &[u8] = b"holder_pool";

    /// Seed for treasury USDC account
    pub const TREASURY_USDC: &[u8] = b"treasury_usdc";

    /// Seed for treasury AMOS account
    pub const TREASURY_AMOS: &[u8] = b"treasury_amos";

    /// Seed for reserve vault
    pub const RESERVE_VAULT: &[u8] = b"reserve_vault";
}

// ============================================================================
// Compile-Time Validation Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usdc_revenue_split_totals_100_percent() {
        // Verify USDC revenue split adds up to exactly 100%
        let total = HOLDER_SHARE_BPS + RND_SHARE_BPS + OPS_SHARE_BPS + RESERVE_SHARE_BPS;
        assert_eq!(
            total,
            BPS_DENOMINATOR,
            "USDC revenue split must total exactly 10000 basis points (100%)"
        );
    }

    #[test]
    fn test_amos_payment_split_totals_100_percent() {
        // Verify AMOS payment split adds up to exactly 100%
        let total = AMOS_BURN_BPS + AMOS_HOLDER_BPS;
        assert_eq!(
            total,
            BPS_DENOMINATOR,
            "AMOS payment split must total exactly 10000 basis points (100%)"
        );
    }

    #[test]
    fn test_usdc_revenue_percentages() {
        // Verify each percentage is correct
        assert_eq!(HOLDER_SHARE_BPS, 5000, "Holder share should be 50%");
        assert_eq!(RND_SHARE_BPS, 4000, "R&D share should be 40%");
        assert_eq!(OPS_SHARE_BPS, 500, "Ops share should be 5%");
        assert_eq!(RESERVE_SHARE_BPS, 500, "Reserve share should be 5%");
    }

    #[test]
    fn test_amos_payment_percentages() {
        // Verify AMOS payment split percentages
        assert_eq!(AMOS_BURN_BPS, 5000, "AMOS burn should be 50%");
        assert_eq!(AMOS_HOLDER_BPS, 5000, "AMOS holder share should be 50%");
    }

    #[test]
    fn test_minimum_stake_requirements() {
        // Verify stake requirements are reasonable
        assert!(MIN_STAKE_DAYS >= 30, "Minimum stake period should be at least 30 days");
        assert!(MIN_STAKE_AMOUNT >= 100, "Minimum stake amount should be at least 100 tokens");
    }

    #[test]
    fn test_discount_percentages_valid() {
        // Verify discounts don't exceed 100%
        assert!(
            AMOS_PAYMENT_DISCOUNT_BPS <= BPS_DENOMINATOR,
            "AMOS payment discount cannot exceed 100%"
        );
        assert!(
            ANNUAL_SUBSCRIPTION_DISCOUNT_BPS <= BPS_DENOMINATOR,
            "Annual subscription discount cannot exceed 100%"
        );
    }

}
