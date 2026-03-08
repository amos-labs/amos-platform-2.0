/// AMOS Treasury Admin Instructions
///
/// Handles treasury initialization and configuration.
/// Only the designated authority can execute these operations.

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
/// Sets up the treasury configuration, creates necessary PDAs,
/// and establishes the revenue distribution rules.
///
/// This can only be called once. All distribution percentages are
/// hardcoded in constants.rs to ensure immutability and trust.
///
/// # Arguments
/// * `rnd_multisig` - R&D multisig address (receives 40% of USDC revenue)
/// * `ops_multisig` - Operations multisig address (receives 5% of USDC revenue)
///
pub fn initialize(
    ctx: Context<Initialize>,
    rnd_multisig: Pubkey,
    ops_multisig: Pubkey,
) -> Result<()> {
    let treasury_config = &mut ctx.accounts.treasury_config;
    let holder_pool = &mut ctx.accounts.holder_pool;
    let clock = Clock::get()?;

    // Validate multisig addresses are not default pubkeys
    require!(
        rnd_multisig != Pubkey::default(),
        TreasuryError::InvalidMultisig
    );
    require!(
        ops_multisig != Pubkey::default(),
        TreasuryError::InvalidMultisig
    );

    // Validate mint addresses
    require!(
        ctx.accounts.usdc_mint.key() != Pubkey::default(),
        TreasuryError::InvalidMint
    );
    require!(
        ctx.accounts.amos_mint.key() != Pubkey::default(),
        TreasuryError::InvalidMint
    );

    // Initialize treasury configuration
    treasury_config.authority = ctx.accounts.authority.key();
    treasury_config.rnd_multisig = rnd_multisig;
    treasury_config.ops_multisig = ops_multisig;
    treasury_config.usdc_mint = ctx.accounts.usdc_mint.key();
    treasury_config.amos_mint = ctx.accounts.amos_mint.key();
    treasury_config.treasury_usdc_vault = ctx.accounts.treasury_usdc_vault.key();
    treasury_config.treasury_amos_vault = ctx.accounts.treasury_amos_vault.key();
    treasury_config.reserve_vault = ctx.accounts.reserve_vault.key();

    // Initialize running totals to zero
    treasury_config.total_usdc_received = 0;
    treasury_config.total_amos_received = 0;
    treasury_config.total_amos_burned = 0;
    treasury_config.total_usdc_to_holders = 0;
    treasury_config.total_usdc_to_rnd = 0;
    treasury_config.total_usdc_to_ops = 0;
    treasury_config.total_usdc_to_reserve = 0;
    treasury_config.total_amos_to_holders = 0;
    treasury_config.distribution_count = 0;
    treasury_config.total_stakes = 0;
    treasury_config.total_staked_amount = 0;

    // Set timestamps
    treasury_config.initialized_at = clock.unix_timestamp;
    treasury_config.last_distribution_at = 0;

    // Store PDA bump
    treasury_config.bump = ctx.bumps.treasury_config;

    // Initialize holder pool
    holder_pool.usdc_balance = 0;
    holder_pool.amos_balance = 0;
    holder_pool.total_usdc_deposited = 0;
    holder_pool.total_amos_deposited = 0;
    holder_pool.total_usdc_claimed = 0;
    holder_pool.total_amos_claimed = 0;
    holder_pool.claim_count = 0;
    holder_pool.last_deposit_at = 0;
    holder_pool.last_claim_at = 0;
    holder_pool.bump = ctx.bumps.holder_pool;

    msg!("Treasury initialized successfully");
    msg!("Authority: {}", treasury_config.authority);
    msg!("R&D Multisig: {}", rnd_multisig);
    msg!("Ops Multisig: {}", ops_multisig);
    msg!("USDC Mint: {}", treasury_config.usdc_mint);
    msg!("AMOS Mint: {}", treasury_config.amos_mint);

    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Treasury authority (program deployer/admin)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// Treasury configuration PDA
    /// PDA: ["treasury_config"]
    #[account(
        init,
        payer = authority,
        space = TreasuryConfig::LEN,
        seeds = [seeds::TREASURY_CONFIG],
        bump
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,

    /// Holder pool PDA
    /// PDA: ["holder_pool"]
    #[account(
        init,
        payer = authority,
        space = HolderPool::LEN,
        seeds = [seeds::HOLDER_POOL],
        bump
    )]
    pub holder_pool: Account<'info, HolderPool>,

    /// USDC mint account
    pub usdc_mint: Account<'info, Mint>,

    /// AMOS token mint account
    pub amos_mint: Account<'info, Mint>,

    /// Treasury USDC vault (PDA)
    /// This vault receives USDC revenue before distribution
    #[account(
        init,
        payer = authority,
        seeds = [seeds::TREASURY_USDC],
        bump,
        token::mint = usdc_mint,
        token::authority = treasury_config,
    )]
    pub treasury_usdc_vault: Account<'info, TokenAccount>,

    /// Treasury AMOS vault (PDA)
    /// This vault receives AMOS payments before distribution
    #[account(
        init,
        payer = authority,
        seeds = [seeds::TREASURY_AMOS],
        bump,
        token::mint = amos_mint,
        token::authority = treasury_config,
    )]
    pub treasury_amos_vault: Account<'info, TokenAccount>,

    /// Reserve vault for 5% + rounding (PDA)
    #[account(
        init,
        payer = authority,
        seeds = [seeds::RESERVE_VAULT],
        bump,
        token::mint = usdc_mint,
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
