/// AMOS Treasury Admin Instructions
///
/// Handles treasury initialization and configuration.
/// AMOS-only model: no USDC infrastructure.
/// Fee distribution: 50% holders, 40% burned, 10% Labs.

use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::constants::seeds;
use crate::errors::TreasuryError;
use crate::state::{HolderPool, TreasuryConfig};

// ============================================================================
// Initialize Treasury
// ============================================================================

/// Initialize the AMOS Treasury
///
/// Sets up the treasury configuration with Labs wallet, AMOS mint,
/// and creates necessary PDAs. All fee splits are hardcoded in constants.rs.
///
/// # Arguments
/// * `labs_wallet` - AMOS Labs operating wallet (receives 10% of protocol fees)
///
pub fn initialize(
    ctx: Context<Initialize>,
    labs_wallet: Pubkey,
) -> Result<()> {
    let treasury_config = &mut ctx.accounts.treasury_config;
    let holder_pool = &mut ctx.accounts.holder_pool;
    let clock = Clock::get()?;

    require!(
        labs_wallet != Pubkey::default(),
        TreasuryError::InvalidLabsWallet
    );

    // Initialize treasury configuration
    treasury_config.authority = ctx.accounts.authority.key();
    treasury_config.labs_wallet = labs_wallet;
    treasury_config.amos_mint = ctx.accounts.amos_mint.key();
    treasury_config.treasury_amos_vault = ctx.accounts.treasury_amos_vault.key();
    treasury_config.reserve_vault = ctx.accounts.reserve_vault.key();

    // Initialize running totals
    treasury_config.total_fees_collected = 0;
    treasury_config.total_fees_to_holders = 0;
    treasury_config.total_fees_burned = 0;
    treasury_config.total_fees_to_labs = 0;
    treasury_config.total_amos_burned = 0;
    treasury_config.distribution_count = 0;
    treasury_config.total_stakes = 0;
    treasury_config.total_staked_amount = 0;

    // Set timestamps
    treasury_config.initialized_at = clock.unix_timestamp;
    treasury_config.last_distribution_at = 0;

    // Store PDA bump
    treasury_config.bump = ctx.bumps.treasury_config;
    treasury_config.reserved = [0; 8];

    // Initialize holder pool
    holder_pool.amos_balance = 0;
    holder_pool.total_amos_deposited = 0;
    holder_pool.total_amos_claimed = 0;
    holder_pool.claim_count = 0;
    holder_pool.last_deposit_at = 0;
    holder_pool.last_claim_at = 0;
    holder_pool.bump = ctx.bumps.holder_pool;

    msg!("Treasury initialized successfully");
    msg!("Authority: {}", treasury_config.authority);
    msg!("Labs Wallet: {}", labs_wallet);
    msg!("AMOS Mint: {}", treasury_config.amos_mint);

    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Treasury authority (program deployer/admin)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Treasury configuration PDA
    #[account(
        init,
        payer = authority,
        space = TreasuryConfig::LEN,
        seeds = [seeds::TREASURY_CONFIG],
        bump
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,

    /// Holder pool PDA
    #[account(
        init,
        payer = authority,
        space = HolderPool::LEN,
        seeds = [seeds::HOLDER_POOL],
        bump
    )]
    pub holder_pool: Account<'info, HolderPool>,

    /// AMOS token mint account
    pub amos_mint: Account<'info, Mint>,

    /// Treasury AMOS vault (PDA) — holds bounty emission pool
    #[account(
        init,
        payer = authority,
        seeds = [seeds::TREASURY_AMOS],
        bump,
        token::mint = amos_mint,
        token::authority = treasury_config,
    )]
    pub treasury_amos_vault: Account<'info, TokenAccount>,

    /// Reserve vault (DAO-locked emergency reserve)
    #[account(
        init,
        payer = authority,
        seeds = [seeds::RESERVE_VAULT],
        bump,
        token::mint = amos_mint,
        token::authority = treasury_config,
    )]
    pub reserve_vault: Account<'info, TokenAccount>,

    /// SPL Token program
    pub token_program: Program<'info, Token>,

    /// System program
    pub system_program: Program<'info, System>,

    /// Rent sysvar
    pub rent: Sysvar<'info, Rent>,
}

// ============================================================================
// Update Labs Wallet
// ============================================================================

/// Update the Labs wallet address. Authority-only.
pub fn update_labs_wallet(
    ctx: Context<UpdateLabsWallet>,
    new_labs_wallet: Pubkey,
) -> Result<()> {
    require!(
        new_labs_wallet != Pubkey::default(),
        TreasuryError::InvalidLabsWallet
    );

    ctx.accounts.treasury_config.labs_wallet = new_labs_wallet;

    msg!("Labs wallet updated to: {}", new_labs_wallet);

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateLabsWallet<'info> {
    /// Current authority
    pub authority: Signer<'info>,

    /// Treasury configuration
    #[account(
        mut,
        seeds = [seeds::TREASURY_CONFIG],
        bump = treasury_config.bump,
        has_one = authority @ TreasuryError::Unauthorized,
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,
}
