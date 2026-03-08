/// AMOS Treasury Transparency Instructions
///
/// Read-only view functions for querying treasury state and history.
/// These functions provide full transparency into the treasury operations.

use anchor_lang::prelude::*;

use crate::constants::seeds;
use crate::errors::TreasuryError;
use crate::state::{Distribution, HolderPool, TreasuryConfig, TreasuryStats};

// ============================================================================
// Get Treasury State
// ============================================================================

/// Get current treasury statistics
///
/// Returns comprehensive treasury state including all-time totals,
/// current balances, and distribution statistics.
///
/// This is a read-only view function for transparency.
///
pub fn get_treasury_state(ctx: Context<GetTreasuryState>) -> Result<TreasuryStats> {
    let treasury_config = &ctx.accounts.treasury_config;
    let holder_pool = &ctx.accounts.holder_pool;

    Ok(TreasuryStats {
        total_usdc_received: treasury_config.total_usdc_received,
        total_amos_received: treasury_config.total_amos_received,
        total_amos_burned: treasury_config.total_amos_burned,
        total_usdc_to_holders: treasury_config.total_usdc_to_holders,
        total_amos_to_holders: treasury_config.total_amos_to_holders,
        total_usdc_to_rnd: treasury_config.total_usdc_to_rnd,
        total_usdc_to_ops: treasury_config.total_usdc_to_ops,
        total_usdc_to_reserve: treasury_config.total_usdc_to_reserve,
        distribution_count: treasury_config.distribution_count,
        total_stakes: treasury_config.total_stakes,
        total_staked_amount: treasury_config.total_staked_amount,
        holder_pool_usdc: holder_pool.usdc_balance,
        holder_pool_amos: holder_pool.amos_balance,
        initialized_at: treasury_config.initialized_at,
        last_distribution_at: treasury_config.last_distribution_at,
    })
}

#[derive(Accounts)]
pub struct GetTreasuryState<'info> {
    /// Treasury configuration
    #[account(
        seeds = [seeds::TREASURY_CONFIG],
        bump = treasury_config.bump,
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,

    /// Holder pool state
    #[account(
        seeds = [seeds::HOLDER_POOL],
        bump = holder_pool.bump,
    )]
    pub holder_pool: Account<'info, HolderPool>,
}

// ============================================================================
// Get Distribution History
// ============================================================================

/// Get distribution history
///
/// Returns a list of recent distribution records for audit purposes.
/// Each distribution is an immutable record of a revenue event.
///
/// # Arguments
/// * `limit` - Maximum number of records to return (max 100)
///
/// Note: This is a simplified version. In production, you would typically
/// implement pagination using start_index and limit parameters.
///
pub fn get_distribution_history(
    ctx: Context<GetDistributionHistory>,
    limit: u64,
) -> Result<Vec<Distribution>> {
    require!(limit > 0 && limit <= 100, TreasuryError::InvalidQueryLimit);

    let treasury_config = &ctx.accounts.treasury_config;

    // Calculate start index (most recent distributions)
    let total_distributions = treasury_config.distribution_count;
    let start_index = if total_distributions > limit {
        total_distributions - limit + 1
    } else {
        1
    };

    // In a real implementation, you would load Distribution accounts
    // using anchor's account loader or remaining_accounts
    // This is a placeholder that demonstrates the structure

    msg!("Query distribution history");
    msg!("Total distributions: {}", total_distributions);
    msg!("Returning records from index {} to {}", start_index, total_distributions);
    msg!("Limit: {}", limit);

    // Return empty vec - actual implementation would load from remaining_accounts
    Ok(Vec::new())
}

#[derive(Accounts)]
#[instruction(limit: u64)]
pub struct GetDistributionHistory<'info> {
    /// Treasury configuration
    #[account(
        seeds = [seeds::TREASURY_CONFIG],
        bump = treasury_config.bump,
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,
}

// ============================================================================
// Get Distribution by Index
// ============================================================================

/// Get a specific distribution record by index
///
/// Returns the complete distribution record for a given index.
/// All distributions are immutable and permanently on-chain.
///
/// # Arguments
/// * `index` - Distribution index to query (1-based)
///
pub fn get_distribution(ctx: Context<GetDistribution>, index: u64) -> Result<Distribution> {
    let distribution = &ctx.accounts.distribution;
    let treasury_config = &ctx.accounts.treasury_config;

    // Validate index is within bounds
    require!(
        index > 0 && index <= treasury_config.distribution_count,
        TreasuryError::DistributionIndexOutOfBounds
    );

    // Return the distribution
    Ok(Distribution {
        index: distribution.index,
        timestamp: distribution.timestamp,
        distribution_type: distribution.distribution_type,
        total_amount: distribution.total_amount,
        amount_to_holders: distribution.amount_to_holders,
        amount_to_rnd: distribution.amount_to_rnd,
        amount_to_ops: distribution.amount_to_ops,
        amount_to_reserve: distribution.amount_to_reserve,
        amount_burned: distribution.amount_burned,
        payment_reference: distribution.payment_reference.clone(),
        tx_signature: distribution.tx_signature.clone(),
        bump: distribution.bump,
    })
}

#[derive(Accounts)]
#[instruction(index: u64)]
pub struct GetDistribution<'info> {
    /// Treasury configuration
    #[account(
        seeds = [seeds::TREASURY_CONFIG],
        bump = treasury_config.bump,
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,

    /// Distribution record
    #[account(
        seeds = [seeds::DISTRIBUTION, &index.to_le_bytes()],
        bump = distribution.bump,
    )]
    pub distribution: Account<'info, Distribution>,
}
