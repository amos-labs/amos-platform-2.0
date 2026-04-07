/// AMOS Bounty Program - Admin Instructions
///
/// This module handles program initialization and administrative functions.
/// Only the oracle authority can perform these operations.

use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::*;
use crate::errors::BountyError;
use crate::state::*;

// ============================================================================
// Initialize Program
// ============================================================================

/// Initialize the AMOS Bounty program with core configuration.
/// This can only be called once and sets up the singleton BountyConfig account.
///
/// # Arguments
/// * `oracle_authority` - The authority that will validate bounty submissions
///
/// # Trustless Guarantees
/// - Immutable oracle authority (cannot be changed after init)
/// - Fixed initial emission rate (16,000 tokens/day)
/// - Default decay rate within protocol bounds (5% annual)
/// - All parameters are transparent and on-chain
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = BountyConfig::SIZE,
        seeds = [BOUNTY_CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, BountyConfig>,

    /// The AMOS token mint
    pub mint: Account<'info, Mint>,

    /// The treasury token account holding the distribution pool
    #[account(
        constraint = treasury.mint == mint.key() @ BountyError::InvalidMint,
        constraint = treasury.amount >= TREASURY_ALLOCATION @ BountyError::TreasuryInsufficientFunds
    )]
    pub treasury: Account<'info, TokenAccount>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler_initialize(ctx: Context<Initialize>, oracle_authority: Pubkey) -> Result<()> {
    let config = &mut ctx.accounts.config;
    let clock = Clock::get()?;

    // Initialize configuration with default values
    config.oracle_authority = oracle_authority;
    config.mint = ctx.accounts.mint.key();
    config.treasury = ctx.accounts.treasury.key();
    config.start_time = clock.unix_timestamp;
    config.halving_epoch = 0;
    config.daily_emission = INITIAL_DAILY_EMISSION;
    config.total_tokens_distributed = 0;
    config.total_bounties = 0;
    config.total_points = 0;
    config.decay_rate_bps = DEFAULT_DECAY_RATE_BPS;
    config.bump = ctx.bumps.config;
    config.reserved = [0; 16];

    msg!("AMOS Bounty Program initialized");
    msg!("Oracle Authority: {}", oracle_authority);
    msg!("Initial Daily Emission: {} tokens", INITIAL_DAILY_EMISSION);
    msg!("Default Decay Rate: {}%", DEFAULT_DECAY_RATE_BPS / 100);

    Ok(())
}

// ============================================================================
// Update Decay Rate
// ============================================================================

/// Update the annual decay rate within protocol-defined bounds.
/// Only the oracle can call this, and the rate must be between 2% and 25%.
///
/// # Arguments
/// * `new_rate_bps` - New decay rate in basis points (200-2500)
///
/// # Trustless Guarantees
/// - Rate bounded by protocol constants (MIN_DECAY_RATE_BPS to MAX_DECAY_RATE_BPS)
/// - Cannot be set below 2% or above 25%
/// - All existing balances subject to same rules
/// - Change is transparent and auditable on-chain
#[derive(Accounts)]
pub struct UpdateDecayRate<'info> {
    #[account(
        mut,
        seeds = [BOUNTY_CONFIG_SEED],
        bump = config.bump,
        has_one = oracle_authority @ BountyError::Unauthorized
    )]
    pub config: Account<'info, BountyConfig>,

    pub oracle_authority: Signer<'info>,
}

pub fn handler_update_decay(ctx: Context<UpdateDecayRate>, new_rate_bps: u16) -> Result<()> {
    // Validate new rate is within bounds
    require!(
        new_rate_bps >= MIN_DECAY_RATE_BPS && new_rate_bps <= MAX_DECAY_RATE_BPS,
        BountyError::InvalidDecayRate
    );

    let config = &mut ctx.accounts.config;
    let old_rate = config.decay_rate_bps;
    config.decay_rate_bps = new_rate_bps;

    msg!("Decay rate updated from {} bps to {} bps", old_rate, new_rate_bps);
    msg!("New annual decay rate: {}%", new_rate_bps / 100);

    Ok(())
}

// ============================================================================
// Advance Halving
// ============================================================================

/// Advance to the next halving epoch, reducing daily emission by 50%.
/// Anyone can call this once 365 days have passed since last halving.
///
/// # Trustless Guarantees
/// - Time-locked: Can only advance after HALVING_INTERVAL_DAYS (365 days)
/// - Automatic halving: Emission rate cut in half each epoch
/// - Minimum floor: Never goes below MINIMUM_DAILY_EMISSION (100 tokens)
/// - Max epochs: Stops at MAX_HALVING_EPOCHS (10 halvings)
/// - Permissionless: Anyone can trigger when time requirements met
#[derive(Accounts)]
pub struct AdvanceHalving<'info> {
    #[account(
        mut,
        seeds = [BOUNTY_CONFIG_SEED],
        bump = config.bump
    )]
    pub config: Account<'info, BountyConfig>,
}

pub fn handler_advance_halving(ctx: Context<AdvanceHalving>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    let clock = Clock::get()?;

    // Check if max halvings reached
    require!(
        config.halving_epoch < MAX_HALVING_EPOCHS,
        BountyError::MaxHalvingsReached
    );

    // Calculate time since start
    let time_elapsed = clock
        .unix_timestamp
        .checked_sub(config.start_time)
        .ok_or(BountyError::InvalidTimestamp)?;

    // Calculate expected epoch based on time
    let days_elapsed = (time_elapsed as u64)
        .checked_div(86400) // seconds per day
        .ok_or(BountyError::ArithmeticOverflow)?;

    let expected_epoch = days_elapsed
        .checked_div(HALVING_INTERVAL_DAYS)
        .ok_or(BountyError::ArithmeticOverflow)?;

    // Check if we can advance to next epoch
    require!(
        expected_epoch > config.halving_epoch as u64,
        BountyError::HalvingNotAvailable
    );

    // Advance epoch and halve emission
    config.halving_epoch = config.halving_epoch.saturating_add(1);

    let new_emission = config
        .daily_emission
        .checked_div(2)
        .ok_or(BountyError::ArithmeticOverflow)?;

    // Apply minimum emission floor
    config.daily_emission = new_emission.max(MINIMUM_DAILY_EMISSION);

    msg!("Halving epoch advanced to {}", config.halving_epoch);
    msg!("New daily emission: {} tokens", config.daily_emission);

    // Emit event for off-chain tracking
    emit!(HalvingAdvanced {
        epoch: config.halving_epoch,
        new_emission: config.daily_emission,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

// ============================================================================
// Events
// ============================================================================

#[event]
pub struct HalvingAdvanced {
    pub epoch: u8,
    pub new_emission: u64,
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_halving_schedule() {
        let mut emission = INITIAL_DAILY_EMISSION;

        for epoch in 1..=MAX_HALVING_EPOCHS {
            emission = emission / 2;
            emission = emission.max(MINIMUM_DAILY_EMISSION);

            println!("Epoch {}: {} tokens/day", epoch, emission);

            // Verify emission never goes below minimum
            assert!(emission >= MINIMUM_DAILY_EMISSION);
        }
    }

    #[test]
    fn test_decay_rate_bounds() {
        // Valid rates
        assert!(MIN_DECAY_RATE_BPS <= DEFAULT_DECAY_RATE_BPS);
        assert!(DEFAULT_DECAY_RATE_BPS <= MAX_DECAY_RATE_BPS);

        // 2% minimum
        assert_eq!(MIN_DECAY_RATE_BPS, 200);

        // 25% maximum
        assert_eq!(MAX_DECAY_RATE_BPS, 2500);
    }
}
