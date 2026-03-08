/// AMOS Treasury State Accounts
///
/// Defines all on-chain account structures for the treasury system.
/// These accounts store configuration, stakes, distributions, and pool state.

use anchor_lang::prelude::*;

// ============================================================================
// Treasury Configuration Account
// ============================================================================

/// Main configuration account for the AMOS Treasury
///
/// This account stores the core configuration including authority,
/// multisig addresses, token mints, and running totals.
///
/// PDA: ["treasury_config"]
#[account]
pub struct TreasuryConfig {
    /// Program authority (can only be changed by current authority)
    pub authority: Pubkey,

    /// R&D multisig address (receives 40% of USDC revenue)
    pub rnd_multisig: Pubkey,

    /// Operations multisig address (receives 5% of USDC revenue)
    pub ops_multisig: Pubkey,

    /// USDC mint address
    pub usdc_mint: Pubkey,

    /// AMOS token mint address
    pub amos_mint: Pubkey,

    /// Treasury USDC vault address
    pub treasury_usdc_vault: Pubkey,

    /// Treasury AMOS vault address
    pub treasury_amos_vault: Pubkey,

    /// Reserve vault address (receives 5% of USDC revenue + rounding)
    pub reserve_vault: Pubkey,

    /// Total USDC revenue received (all-time)
    pub total_usdc_received: u64,

    /// Total AMOS payments received (all-time)
    pub total_amos_received: u64,

    /// Total AMOS tokens burned (from AMOS payments)
    pub total_amos_burned: u64,

    /// Total USDC distributed to holders
    pub total_usdc_to_holders: u64,

    /// Total USDC distributed to R&D
    pub total_usdc_to_rnd: u64,

    /// Total USDC distributed to operations
    pub total_usdc_to_ops: u64,

    /// Total USDC distributed to reserve
    pub total_usdc_to_reserve: u64,

    /// Total AMOS distributed to holders (from AMOS payments)
    pub total_amos_to_holders: u64,

    /// Number of distributions processed
    pub distribution_count: u64,

    /// Total number of registered stakes
    pub total_stakes: u64,

    /// Total AMOS staked across all users
    pub total_staked_amount: u64,

    /// Timestamp of treasury initialization
    pub initialized_at: i64,

    /// Timestamp of last distribution
    pub last_distribution_at: i64,

    /// PDA bump seed
    pub bump: u8,
}

impl TreasuryConfig {
    /// Calculate space needed for TreasuryConfig account
    pub const LEN: usize = 8 + // discriminator
        32 + // authority
        32 + // rnd_multisig
        32 + // ops_multisig
        32 + // usdc_mint
        32 + // amos_mint
        32 + // treasury_usdc_vault
        32 + // treasury_amos_vault
        32 + // reserve_vault
        8 + // total_usdc_received
        8 + // total_amos_received
        8 + // total_amos_burned
        8 + // total_usdc_to_holders
        8 + // total_usdc_to_rnd
        8 + // total_usdc_to_ops
        8 + // total_usdc_to_reserve
        8 + // total_amos_to_holders
        8 + // distribution_count
        8 + // total_stakes
        8 + // total_staked_amount
        8 + // initialized_at
        8 + // last_distribution_at
        1; // bump
}

// ============================================================================
// Stake Record Account
// ============================================================================

/// Individual stake record for a user
///
/// Tracks a user's AMOS stake amount, timestamps, and claim history.
/// Users must stake for minimum 30 days before claiming revenue.
///
/// PDA: ["stake_record", user_pubkey]
#[account]
pub struct StakeRecord {
    /// Owner of this stake
    pub owner: Pubkey,

    /// Amount of AMOS tokens staked
    pub amount: u64,

    /// Timestamp when stake was registered
    pub staked_at: i64,

    /// Timestamp of last stake update
    pub updated_at: i64,

    /// Timestamp of last claim
    pub last_claim_at: i64,

    /// Total USDC claimed (all-time)
    pub total_usdc_claimed: u64,

    /// Total AMOS claimed (all-time, from AMOS payments)
    pub total_amos_claimed: u64,

    /// Number of claims made
    pub claim_count: u64,

    /// PDA bump seed
    pub bump: u8,
}

impl StakeRecord {
    /// Calculate space needed for StakeRecord account
    pub const LEN: usize = 8 + // discriminator
        32 + // owner
        8 + // amount
        8 + // staked_at
        8 + // updated_at
        8 + // last_claim_at
        8 + // total_usdc_claimed
        8 + // total_amos_claimed
        8 + // claim_count
        1; // bump

    /// Check if minimum stake period has been met
    pub fn can_claim(&self, current_time: i64, min_stake_seconds: i64) -> bool {
        let stake_duration = current_time.saturating_sub(self.staked_at);
        stake_duration >= min_stake_seconds
    }

    /// Get stake duration in days
    pub fn stake_duration_days(&self, current_time: i64) -> u64 {
        let duration_seconds = current_time.saturating_sub(self.staked_at);
        (duration_seconds / 86400) as u64 // 86400 seconds in a day
    }
}

// ============================================================================
// Distribution Record Account
// ============================================================================

/// Distribution type enum
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum DistributionType {
    /// USDC revenue distribution
    UsdcRevenue,
    /// AMOS payment distribution
    AmosPayment,
}

/// Record of a revenue distribution event
///
/// Immutable record of each distribution for transparency.
/// Allows users to audit the entire distribution history.
///
/// PDA: ["distribution", distribution_index]
#[account]
pub struct Distribution {
    /// Sequential index of this distribution
    pub index: u64,

    /// Timestamp of distribution
    pub timestamp: i64,

    /// Type of distribution
    pub distribution_type: DistributionType,

    /// Total amount received (before split)
    pub total_amount: u64,

    /// Amount to holders pool
    pub amount_to_holders: u64,

    /// Amount to R&D multisig (USDC only)
    pub amount_to_rnd: u64,

    /// Amount to operations multisig (USDC only)
    pub amount_to_ops: u64,

    /// Amount to reserve vault (USDC only, includes rounding)
    pub amount_to_reserve: u64,

    /// Amount burned (AMOS only)
    pub amount_burned: u64,

    /// Payment reference (invoice ID, subscription ID, etc.)
    pub payment_reference: String,

    /// Transaction signature (optional, for auditability)
    pub tx_signature: String,

    /// PDA bump seed
    pub bump: u8,
}

impl Distribution {
    /// Calculate space needed for Distribution account
    /// Variable size due to strings, using max lengths
    pub const MAX_PAYMENT_REF_LEN: usize = 64;
    pub const MAX_TX_SIG_LEN: usize = 88; // Base58 encoded signature length

    pub const LEN: usize = 8 + // discriminator
        8 + // index
        8 + // timestamp
        1 + // distribution_type enum
        8 + // total_amount
        8 + // amount_to_holders
        8 + // amount_to_rnd
        8 + // amount_to_ops
        8 + // amount_to_reserve
        8 + // amount_burned
        (4 + Self::MAX_PAYMENT_REF_LEN) + // payment_reference string
        (4 + Self::MAX_TX_SIG_LEN) + // tx_signature string
        1; // bump
}

// ============================================================================
// Holder Pool Account
// ============================================================================

/// Holder pool state tracking
///
/// Tracks the USDC/AMOS pool available for holder claims.
/// All revenue shares for holders accumulate here.
///
/// PDA: ["holder_pool"]
#[account]
pub struct HolderPool {
    /// Current USDC balance available for claims
    pub usdc_balance: u64,

    /// Current AMOS balance available for claims
    pub amos_balance: u64,

    /// Total USDC deposited (all-time)
    pub total_usdc_deposited: u64,

    /// Total AMOS deposited (all-time)
    pub total_amos_deposited: u64,

    /// Total USDC claimed by all holders (all-time)
    pub total_usdc_claimed: u64,

    /// Total AMOS claimed by all holders (all-time)
    pub total_amos_claimed: u64,

    /// Number of claims processed
    pub claim_count: u64,

    /// Timestamp of last deposit
    pub last_deposit_at: i64,

    /// Timestamp of last claim
    pub last_claim_at: i64,

    /// PDA bump seed
    pub bump: u8,
}

impl HolderPool {
    /// Calculate space needed for HolderPool account
    pub const LEN: usize = 8 + // discriminator
        8 + // usdc_balance
        8 + // amos_balance
        8 + // total_usdc_deposited
        8 + // total_amos_deposited
        8 + // total_usdc_claimed
        8 + // total_amos_claimed
        8 + // claim_count
        8 + // last_deposit_at
        8 + // last_claim_at
        1; // bump
}

// ============================================================================
// Treasury Statistics (View/Query struct, not an account)
// ============================================================================

/// Treasury statistics returned by get_treasury_state
///
/// This is not an on-chain account, but a data structure
/// returned by the view function for querying treasury state.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TreasuryStats {
    /// Total USDC revenue received (all-time)
    pub total_usdc_received: u64,

    /// Total AMOS payments received (all-time)
    pub total_amos_received: u64,

    /// Total AMOS burned
    pub total_amos_burned: u64,

    /// Total distributed to holders (USDC)
    pub total_usdc_to_holders: u64,

    /// Total distributed to holders (AMOS)
    pub total_amos_to_holders: u64,

    /// Total distributed to R&D
    pub total_usdc_to_rnd: u64,

    /// Total distributed to operations
    pub total_usdc_to_ops: u64,

    /// Total distributed to reserve
    pub total_usdc_to_reserve: u64,

    /// Number of distributions
    pub distribution_count: u64,

    /// Total registered stakes
    pub total_stakes: u64,

    /// Total amount staked
    pub total_staked_amount: u64,

    /// Current holder pool USDC balance
    pub holder_pool_usdc: u64,

    /// Current holder pool AMOS balance
    pub holder_pool_amos: u64,

    /// Treasury initialization timestamp
    pub initialized_at: i64,

    /// Last distribution timestamp
    pub last_distribution_at: i64,
}

// ============================================================================
// Claimable Amount (View/Query struct, not an account)
// ============================================================================

/// Claimable revenue amounts for a specific stake
///
/// Returned by get_claimable_amount view function.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ClaimableAmount {
    /// Claimable USDC amount
    pub usdc_amount: u64,

    /// Claimable AMOS amount
    pub amos_amount: u64,

    /// User's stake amount
    pub stake_amount: u64,

    /// Total staked across all users
    pub total_staked: u64,

    /// User's share percentage (basis points)
    pub share_bps: u16,

    /// Can claim (minimum period met)
    pub can_claim: bool,

    /// Days staked
    pub days_staked: u64,

    /// Days remaining until eligible
    pub days_remaining: u64,
}
