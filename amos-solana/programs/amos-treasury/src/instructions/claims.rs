/// AMOS Treasury Claims Instructions
///
/// Handles stake registration and revenue claiming:
/// - register_stake: Register AMOS tokens for revenue sharing (min 100 AMOS)
/// - update_stake: Update stake amount (maintain min 100 AMOS)
/// - claim_revenue: Claim proportional share (min 30 days stake)
/// - get_claimable_amount: Query claimable amounts (read-only)
///
/// Trust guarantees:
/// - No approval needed for claims
/// - Proportional distribution based on stake
/// - 30-day minimum hold period prevents gaming
/// - All arithmetic uses checked operations

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::constants::{seeds, MIN_STAKE_AMOUNT, MIN_STAKE_DAYS};
use crate::errors::TreasuryError;
use crate::state::{ClaimableAmount, HolderPool, StakeRecord, TreasuryConfig};

// ============================================================================
// Register Stake
// ============================================================================

/// Register AMOS tokens for revenue sharing
///
/// Stakes must be at least 100 AMOS and require 30 days minimum hold
/// before claiming revenue. Transfers AMOS from user to treasury vault.
///
/// # Arguments
/// * `amount` - Amount of AMOS tokens to stake (minimum 100)
///
pub fn register_stake(ctx: Context<RegisterStake>, amount: u64) -> Result<()> {
    require!(amount >= MIN_STAKE_AMOUNT, TreasuryError::StakeAmountTooLow);
    require!(amount > 0, TreasuryError::ZeroStakeAmount);

    let stake_record = &mut ctx.accounts.stake_record;
    let treasury_config = &mut ctx.accounts.treasury_config;
    let clock = Clock::get()?;

    // Transfer AMOS tokens from user to treasury vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_amos_account.to_account_info(),
                to: ctx.accounts.stake_vault.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ),
        amount,
    )?;

    // Initialize stake record
    stake_record.owner = ctx.accounts.owner.key();
    stake_record.amount = amount;
    stake_record.staked_at = clock.unix_timestamp;
    stake_record.updated_at = clock.unix_timestamp;
    stake_record.last_claim_at = 0;
    stake_record.total_usdc_claimed = 0;
    stake_record.total_amos_claimed = 0;
    stake_record.claim_count = 0;
    stake_record.bump = ctx.bumps.stake_record;

    // Update treasury totals
    treasury_config.total_stakes = treasury_config
        .total_stakes
        .checked_add(1)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.total_staked_amount = treasury_config
        .total_staked_amount
        .checked_add(amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    msg!("Stake registered successfully");
    msg!("Owner: {}", stake_record.owner);
    msg!("Amount: {}", amount);
    msg!("Staked at: {}", stake_record.staked_at);

    Ok(())
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct RegisterStake<'info> {
    /// Stake owner
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Treasury configuration
    #[account(
        mut,
        seeds = [seeds::TREASURY_CONFIG],
        bump = treasury_config.bump,
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,

    /// Stake record PDA
    #[account(
        init,
        payer = owner,
        space = StakeRecord::LEN,
        seeds = [seeds::STAKE_RECORD, owner.key().as_ref()],
        bump
    )]
    pub stake_record: Account<'info, StakeRecord>,

    /// User's AMOS token account
    #[account(
        mut,
        token::mint = treasury_config.amos_mint,
        token::authority = owner,
    )]
    pub user_amos_account: Account<'info, TokenAccount>,

    /// Stake vault (holds staked AMOS)
    #[account(
        mut,
        token::mint = treasury_config.amos_mint,
    )]
    pub stake_vault: Account<'info, TokenAccount>,

    /// SPL Token program
    pub token_program: Program<'info, Token>,

    /// System program
    pub system_program: Program<'info, System>,
}

// ============================================================================
// Update Stake
// ============================================================================

/// Update existing stake amount
///
/// Can increase or decrease stake, but must maintain minimum 100 AMOS.
/// Resets the stake timer if increasing stake.
///
/// # Arguments
/// * `new_amount` - New total stake amount (minimum 100)
///
pub fn update_stake(ctx: Context<UpdateStake>, new_amount: u64) -> Result<()> {
    require!(new_amount >= MIN_STAKE_AMOUNT, TreasuryError::StakeBelowMinimum);

    let stake_record = &mut ctx.accounts.stake_record;
    let treasury_config = &mut ctx.accounts.treasury_config;
    let clock = Clock::get()?;

    let old_amount = stake_record.amount;
    require!(new_amount != old_amount, TreasuryError::InvalidInput);

    if new_amount > old_amount {
        // Increasing stake - transfer additional tokens
        let additional = new_amount
            .checked_sub(old_amount)
            .ok_or(TreasuryError::ArithmeticUnderflow)?;

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_amos_account.to_account_info(),
                    to: ctx.accounts.stake_vault.to_account_info(),
                    authority: ctx.accounts.owner.to_account_info(),
                },
            ),
            additional,
        )?;

        // Update treasury total
        treasury_config.total_staked_amount = treasury_config
            .total_staked_amount
            .checked_add(additional)
            .ok_or(TreasuryError::ArithmeticOverflow)?;

        // Reset stake timer when increasing
        stake_record.staked_at = clock.unix_timestamp;

        msg!("Stake increased by {}", additional);
    } else {
        // Decreasing stake - transfer tokens back
        let reduction = old_amount
            .checked_sub(new_amount)
            .ok_or(TreasuryError::ArithmeticUnderflow)?;

        // Use treasury config as authority for vault
        let treasury_seeds = &[seeds::TREASURY_CONFIG, &[treasury_config.bump]];
        let signer_seeds = &[&treasury_seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stake_vault.to_account_info(),
                    to: ctx.accounts.user_amos_account.to_account_info(),
                    authority: treasury_config.to_account_info(),
                },
                signer_seeds,
            ),
            reduction,
        )?;

        // Update treasury total
        treasury_config.total_staked_amount = treasury_config
            .total_staked_amount
            .checked_sub(reduction)
            .ok_or(TreasuryError::ArithmeticUnderflow)?;

        msg!("Stake decreased by {}", reduction);
    }

    // Update stake record
    stake_record.amount = new_amount;
    stake_record.updated_at = clock.unix_timestamp;

    msg!("Stake updated successfully");
    msg!("New amount: {}", new_amount);
    msg!("Old amount: {}", old_amount);

    Ok(())
}

#[derive(Accounts)]
#[instruction(new_amount: u64)]
pub struct UpdateStake<'info> {
    /// Stake owner
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Treasury configuration
    #[account(
        mut,
        seeds = [seeds::TREASURY_CONFIG],
        bump = treasury_config.bump,
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,

    /// Stake record
    #[account(
        mut,
        seeds = [seeds::STAKE_RECORD, owner.key().as_ref()],
        bump = stake_record.bump,
        has_one = owner @ TreasuryError::NotStakeOwner,
    )]
    pub stake_record: Account<'info, StakeRecord>,

    /// User's AMOS token account
    #[account(
        mut,
        token::mint = treasury_config.amos_mint,
        token::authority = owner,
    )]
    pub user_amos_account: Account<'info, TokenAccount>,

    /// Stake vault
    #[account(
        mut,
        token::mint = treasury_config.amos_mint,
    )]
    pub stake_vault: Account<'info, TokenAccount>,

    /// SPL Token program
    pub token_program: Program<'info, Token>,
}

// ============================================================================
// Claim Revenue
// ============================================================================

/// Claim proportional share of revenue
///
/// Users can claim their proportional share of USDC and AMOS revenue
/// after meeting the 30-day minimum stake period. No approval required.
///
/// Calculation: (user_stake / total_stake) * pool_balance
///
/// Trust guarantees:
/// - Fully permissionless - no multisig approval needed
/// - Proportional distribution based on stake weight
/// - 30-day minimum prevents gaming
/// - Checked arithmetic prevents manipulation
///
pub fn claim_revenue(ctx: Context<ClaimRevenue>) -> Result<()> {
    let stake_record = &mut ctx.accounts.stake_record;
    let treasury_config = &ctx.accounts.treasury_config;
    let holder_pool = &mut ctx.accounts.holder_pool;
    let clock = Clock::get()?;

    // Verify minimum stake period (30 days = 2,592,000 seconds)
    let min_stake_seconds = (MIN_STAKE_DAYS as i64)
        .checked_mul(86400)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    require!(
        stake_record.can_claim(clock.unix_timestamp, min_stake_seconds),
        TreasuryError::MinimumStakePeriodNotMet
    );

    // Prevent division by zero
    require!(
        treasury_config.total_staked_amount > 0,
        TreasuryError::DivisionByZero
    );

    // Calculate proportional shares using checked arithmetic
    // Share = (user_stake * pool_balance) / total_stake

    // Calculate USDC claim
    let usdc_claim = if holder_pool.usdc_balance > 0 {
        stake_record
            .amount
            .checked_mul(holder_pool.usdc_balance)
            .ok_or(TreasuryError::ArithmeticOverflow)?
            .checked_div(treasury_config.total_staked_amount)
            .ok_or(TreasuryError::DivisionByZero)?
    } else {
        0
    };

    // Calculate AMOS claim
    let amos_claim = if holder_pool.amos_balance > 0 {
        stake_record
            .amount
            .checked_mul(holder_pool.amos_balance)
            .ok_or(TreasuryError::ArithmeticOverflow)?
            .checked_div(treasury_config.total_staked_amount)
            .ok_or(TreasuryError::DivisionByZero)?
    } else {
        0
    };

    // Require at least some claimable amount
    require!(
        usdc_claim > 0 || amos_claim > 0,
        TreasuryError::NoClaimableRevenue
    );

    // Validate sufficient pool balances
    require!(
        usdc_claim <= holder_pool.usdc_balance,
        TreasuryError::InsufficientHolderPoolFunds
    );
    require!(
        amos_claim <= holder_pool.amos_balance,
        TreasuryError::InsufficientHolderPoolFunds
    );

    // Get PDA signer seeds
    let treasury_seeds = &[seeds::TREASURY_CONFIG, &[treasury_config.bump]];
    let signer_seeds = &[&treasury_seeds[..]];

    // Transfer USDC if claimable
    if usdc_claim > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.holder_pool_usdc.to_account_info(),
                    to: ctx.accounts.user_usdc_account.to_account_info(),
                    authority: treasury_config.to_account_info(),
                },
                signer_seeds,
            ),
            usdc_claim,
        )?;

        // Update holder pool USDC state
        holder_pool.usdc_balance = holder_pool
            .usdc_balance
            .checked_sub(usdc_claim)
            .ok_or(TreasuryError::ArithmeticUnderflow)?;

        holder_pool.total_usdc_claimed = holder_pool
            .total_usdc_claimed
            .checked_add(usdc_claim)
            .ok_or(TreasuryError::ArithmeticOverflow)?;
    }

    // Transfer AMOS if claimable
    if amos_claim > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.holder_pool_amos.to_account_info(),
                    to: ctx.accounts.user_amos_account.to_account_info(),
                    authority: treasury_config.to_account_info(),
                },
                signer_seeds,
            ),
            amos_claim,
        )?;

        // Update holder pool AMOS state
        holder_pool.amos_balance = holder_pool
            .amos_balance
            .checked_sub(amos_claim)
            .ok_or(TreasuryError::ArithmeticUnderflow)?;

        holder_pool.total_amos_claimed = holder_pool
            .total_amos_claimed
            .checked_add(amos_claim)
            .ok_or(TreasuryError::ArithmeticOverflow)?;
    }

    // Update stake record
    stake_record.last_claim_at = clock.unix_timestamp;
    stake_record.total_usdc_claimed = stake_record
        .total_usdc_claimed
        .checked_add(usdc_claim)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    stake_record.total_amos_claimed = stake_record
        .total_amos_claimed
        .checked_add(amos_claim)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    stake_record.claim_count = stake_record
        .claim_count
        .checked_add(1)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    // Update holder pool claim stats
    holder_pool.claim_count = holder_pool
        .claim_count
        .checked_add(1)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    holder_pool.last_claim_at = clock.unix_timestamp;

    msg!("Revenue claimed successfully");
    msg!("USDC claimed: {}", usdc_claim);
    msg!("AMOS claimed: {}", amos_claim);
    msg!("Stake amount: {}", stake_record.amount);
    msg!("Total staked: {}", treasury_config.total_staked_amount);

    Ok(())
}

#[derive(Accounts)]
pub struct ClaimRevenue<'info> {
    /// Stake owner (claimer)
    #[account(mut)]
    pub owner: Signer<'info>,

    /// Treasury configuration
    #[account(
        seeds = [seeds::TREASURY_CONFIG],
        bump = treasury_config.bump,
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,

    /// Stake record
    #[account(
        mut,
        seeds = [seeds::STAKE_RECORD, owner.key().as_ref()],
        bump = stake_record.bump,
        has_one = owner @ TreasuryError::NotStakeOwner,
    )]
    pub stake_record: Account<'info, StakeRecord>,

    /// Holder pool state
    #[account(
        mut,
        seeds = [seeds::HOLDER_POOL],
        bump = holder_pool.bump,
    )]
    pub holder_pool: Account<'info, HolderPool>,

    /// Holder pool USDC account
    #[account(
        mut,
        token::mint = treasury_config.usdc_mint,
    )]
    pub holder_pool_usdc: Account<'info, TokenAccount>,

    /// Holder pool AMOS account
    #[account(
        mut,
        token::mint = treasury_config.amos_mint,
    )]
    pub holder_pool_amos: Account<'info, TokenAccount>,

    /// User's USDC account (to receive claim)
    #[account(
        mut,
        token::mint = treasury_config.usdc_mint,
        token::authority = owner,
    )]
    pub user_usdc_account: Account<'info, TokenAccount>,

    /// User's AMOS account (to receive claim)
    #[account(
        mut,
        token::mint = treasury_config.amos_mint,
        token::authority = owner,
    )]
    pub user_amos_account: Account<'info, TokenAccount>,

    /// SPL Token program
    pub token_program: Program<'info, Token>,
}

// ============================================================================
// Get Claimable Amount (View Function)
// ============================================================================

/// Query claimable revenue amounts
///
/// Read-only function to calculate how much USDC and AMOS
/// a user can currently claim, plus eligibility information.
///
/// Returns ClaimableAmount struct with full details.
///
pub fn get_claimable_amount(ctx: Context<GetClaimableAmount>) -> Result<ClaimableAmount> {
    let stake_record = &ctx.accounts.stake_record;
    let treasury_config = &ctx.accounts.treasury_config;
    let holder_pool = &ctx.accounts.holder_pool;
    let clock = Clock::get()?;

    // Calculate minimum stake duration
    let min_stake_seconds = (MIN_STAKE_DAYS as i64)
        .checked_mul(86400)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    let can_claim = stake_record.can_claim(clock.unix_timestamp, min_stake_seconds);
    let days_staked = stake_record.stake_duration_days(clock.unix_timestamp);
    let days_remaining = if days_staked >= MIN_STAKE_DAYS {
        0
    } else {
        MIN_STAKE_DAYS - days_staked
    };

    // Calculate proportional shares
    let (usdc_amount, amos_amount, share_bps) = if treasury_config.total_staked_amount > 0 {
        // Calculate share in basis points (10000 = 100%)
        let share = stake_record
            .amount
            .checked_mul(10000)
            .ok_or(TreasuryError::ArithmeticOverflow)?
            .checked_div(treasury_config.total_staked_amount)
            .ok_or(TreasuryError::DivisionByZero)? as u16;

        // Calculate USDC claimable
        let usdc = stake_record
            .amount
            .checked_mul(holder_pool.usdc_balance)
            .ok_or(TreasuryError::ArithmeticOverflow)?
            .checked_div(treasury_config.total_staked_amount)
            .ok_or(TreasuryError::DivisionByZero)?;

        // Calculate AMOS claimable
        let amos = stake_record
            .amount
            .checked_mul(holder_pool.amos_balance)
            .ok_or(TreasuryError::ArithmeticOverflow)?
            .checked_div(treasury_config.total_staked_amount)
            .ok_or(TreasuryError::DivisionByZero)?;

        (usdc, amos, share)
    } else {
        (0, 0, 0)
    };

    Ok(ClaimableAmount {
        usdc_amount,
        amos_amount,
        stake_amount: stake_record.amount,
        total_staked: treasury_config.total_staked_amount,
        share_bps,
        can_claim,
        days_staked,
        days_remaining,
    })
}

#[derive(Accounts)]
pub struct GetClaimableAmount<'info> {
    /// Stake owner
    pub owner: Signer<'info>,

    /// Treasury configuration
    #[account(
        seeds = [seeds::TREASURY_CONFIG],
        bump = treasury_config.bump,
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,

    /// Stake record
    #[account(
        seeds = [seeds::STAKE_RECORD, owner.key().as_ref()],
        bump = stake_record.bump,
        has_one = owner @ TreasuryError::NotStakeOwner,
    )]
    pub stake_record: Account<'info, StakeRecord>,

    /// Holder pool state
    #[account(
        seeds = [seeds::HOLDER_POOL],
        bump = holder_pool.bump,
    )]
    pub holder_pool: Account<'info, HolderPool>,
}
