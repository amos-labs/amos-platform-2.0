/// AMOS Treasury Program
///
/// Immutable revenue distribution system for the AMOS ecosystem.
///
/// ## Core Features
///
/// ### Revenue Distribution (USDC)
/// - 50% to AMOS holders (proportional to stake)
/// - 40% to R&D multisig
/// - 5% to operations multisig
/// - 5% to reserve vault (+ rounding remainder)
///
/// ### AMOS Payment Distribution
/// - 50% burned (deflationary)
/// - 50% to AMOS holders (+ rounding remainder)
///
/// ### Trust Guarantees
/// - All percentages hardcoded in constants.rs
/// - No approval needed for claims (fully permissionless)
/// - Proportional distribution based on stake weight
/// - 30-day minimum stake period prevents gaming
/// - All arithmetic uses checked operations
/// - Complete transparency via immutable distribution records
///
/// ### Staking Requirements
/// - Minimum stake: 100 AMOS tokens
/// - Minimum hold period: 30 days before claiming
/// - Can increase/decrease stake (maintaining minimum)
///
/// ## Instructions
///
/// ### Admin
/// - `initialize`: Set up treasury (one-time only)
///
/// ### Revenue
/// - `receive_revenue`: Process USDC revenue distribution
/// - `receive_amos_payment`: Process AMOS payment distribution
///
/// ### Claims
/// - `register_stake`: Register AMOS stake for revenue sharing
/// - `update_stake`: Modify stake amount
/// - `claim_revenue`: Claim proportional USDC/AMOS revenue
/// - `get_claimable_amount`: Query claimable amounts (view)
///
/// ### Transparency
/// - `get_treasury_state`: Get current treasury statistics (view)
/// - `get_distribution_history`: Query distribution records (view)
/// - `get_distribution`: Get specific distribution by index (view)

use anchor_lang::prelude::*;

// Module declarations
pub mod constants;
pub mod errors;
pub mod instructions;
pub mod state;

// Re-exports
use instructions::*;
use state::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod amos_treasury {
    use super::*;

    // ========================================================================
    // Admin Instructions
    // ========================================================================

    /// Initialize the AMOS Treasury
    ///
    /// Sets up the treasury configuration with R&D and operations multisigs.
    /// All distribution percentages are hardcoded for immutability.
    ///
    /// # Arguments
    /// * `rnd_multisig` - R&D multisig address (receives 40% of USDC)
    /// * `ops_multisig` - Operations multisig address (receives 5% of USDC)
    ///
    /// # Trust Guarantee
    /// This can only be called once. After initialization, distribution
    /// percentages cannot be changed as they are hardcoded constants.
    ///
    pub fn initialize(
        ctx: Context<Initialize>,
        rnd_multisig: Pubkey,
        ops_multisig: Pubkey,
    ) -> Result<()> {
        instructions::initialize(ctx, rnd_multisig, ops_multisig)
    }

    // ========================================================================
    // Revenue Instructions
    // ========================================================================

    /// Receive and distribute USDC revenue
    ///
    /// Splits USDC revenue according to immutable percentages:
    /// - 50% to holder pool
    /// - 40% to R&D multisig
    /// - 5% to operations multisig
    /// - 5% to reserve vault (+ rounding remainder)
    ///
    /// # Arguments
    /// * `amount` - Amount of USDC received (in smallest unit)
    /// * `payment_reference` - Reference ID (invoice, subscription, etc.)
    ///
    /// # Trust Guarantee
    /// Distribution percentages are hardcoded and cannot be changed.
    /// All transfers are atomic - either all succeed or all fail.
    /// Creates immutable distribution record for full transparency.
    ///
    pub fn receive_revenue(
        ctx: Context<ReceiveRevenue>,
        amount: u64,
        payment_reference: String,
    ) -> Result<()> {
        instructions::receive_revenue(ctx, amount, payment_reference)
    }

    /// Receive and distribute AMOS payment
    ///
    /// When users pay with AMOS tokens (20% discount):
    /// - 50% burned (deflationary mechanism)
    /// - 50% to holder pool (+ rounding remainder)
    ///
    /// # Arguments
    /// * `amount` - Amount of AMOS tokens received
    /// * `payment_reference` - Reference ID for tracking
    ///
    /// # Trust Guarantee
    /// 50/50 split is hardcoded and cannot be changed.
    /// Burn operation is atomic and irreversible.
    /// Creates immutable distribution record for transparency.
    ///
    pub fn receive_amos_payment(
        ctx: Context<ReceiveAmosPayment>,
        amount: u64,
        payment_reference: String,
    ) -> Result<()> {
        instructions::receive_amos_payment(ctx, amount, payment_reference)
    }

    // ========================================================================
    // Claim Instructions
    // ========================================================================

    /// Register AMOS stake for revenue sharing
    ///
    /// Minimum 100 AMOS required. Must stake for 30 days before claiming.
    ///
    /// # Arguments
    /// * `amount` - Amount of AMOS tokens to stake (minimum 100)
    ///
    /// # Trust Guarantee
    /// Transfers AMOS to treasury vault controlled by program PDA.
    /// Stake is recorded immutably on-chain.
    ///
    pub fn register_stake(ctx: Context<RegisterStake>, amount: u64) -> Result<()> {
        instructions::register_stake(ctx, amount)
    }

    /// Update existing stake amount
    ///
    /// Can increase or decrease, but must maintain minimum 100 AMOS.
    /// Increasing stake resets the 30-day timer.
    ///
    /// # Arguments
    /// * `new_amount` - New total stake amount (minimum 100)
    ///
    pub fn update_stake(ctx: Context<UpdateStake>, new_amount: u64) -> Result<()> {
        instructions::update_stake(ctx, new_amount)
    }

    /// Claim proportional share of revenue
    ///
    /// No approval required - fully permissionless after 30-day minimum.
    /// Share calculated as: (user_stake / total_stake) * pool_balance
    ///
    /// # Trust Guarantees
    /// - Permissionless claiming (no multisig approval needed)
    /// - Proportional distribution based on stake weight
    /// - 30-day minimum prevents gaming the system
    /// - Checked arithmetic prevents manipulation
    /// - Atomic transfers ensure consistency
    ///
    pub fn claim_revenue(ctx: Context<ClaimRevenue>) -> Result<()> {
        instructions::claim_revenue(ctx)
    }

    /// Query claimable revenue amounts (view function)
    ///
    /// Returns how much USDC and AMOS the user can currently claim,
    /// plus eligibility information (days staked, days remaining, etc.)
    ///
    /// # Returns
    /// ClaimableAmount struct with full details
    ///
    pub fn get_claimable_amount(ctx: Context<GetClaimableAmount>) -> Result<ClaimableAmount> {
        instructions::get_claimable_amount(ctx)
    }

    // ========================================================================
    // Transparency Instructions (View Functions)
    // ========================================================================

    /// Get current treasury statistics (view function)
    ///
    /// Returns comprehensive statistics including:
    /// - All-time totals (received, distributed, burned)
    /// - Current pool balances
    /// - Distribution count
    /// - Total stakes and staked amount
    ///
    /// # Returns
    /// TreasuryStats struct with complete state
    ///
    pub fn get_treasury_state(ctx: Context<GetTreasuryState>) -> Result<TreasuryStats> {
        instructions::get_treasury_state(ctx)
    }

    /// Query distribution history (view function)
    ///
    /// Returns recent distribution records for transparency and auditing.
    /// All distributions are immutable records stored on-chain.
    ///
    /// # Arguments
    /// * `limit` - Maximum number of records to return (max 100)
    ///
    /// # Returns
    /// Vector of Distribution records
    ///
    pub fn get_distribution_history(
        ctx: Context<GetDistributionHistory>,
        limit: u64,
    ) -> Result<Vec<Distribution>> {
        instructions::get_distribution_history(ctx, limit)
    }

    /// Get specific distribution by index (view function)
    ///
    /// Returns complete details of a specific distribution event.
    /// Useful for detailed auditing and verification.
    ///
    /// # Arguments
    /// * `index` - Distribution index (1-based)
    ///
    /// # Returns
    /// Distribution record
    ///
    pub fn get_distribution(ctx: Context<GetDistribution>, index: u64) -> Result<Distribution> {
        instructions::get_distribution(ctx, index)
    }
}
