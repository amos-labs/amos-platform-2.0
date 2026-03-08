/// AMOS Treasury Revenue Instructions
///
/// Handles receipt and distribution of revenue:
/// - receive_revenue: USDC revenue (50% holders, 40% R&D, 5% ops, 5% reserve)
/// - receive_amos_payment: AMOS payments (50% burn, 50% holders)
///
/// All arithmetic uses checked operations to prevent overflow/underflow.
/// Reserve receives remainder to handle rounding and ensure exact distribution.

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, Transfer};

use crate::constants::{
    seeds, AMOS_BURN_BPS, AMOS_HOLDER_BPS, BPS_DENOMINATOR, HOLDER_SHARE_BPS, OPS_SHARE_BPS,
    RESERVE_SHARE_BPS, RND_SHARE_BPS,
};
use crate::errors::TreasuryError;
use crate::state::{Distribution, DistributionType, HolderPool, TreasuryConfig};

// ============================================================================
// Receive USDC Revenue
// ============================================================================

/// Receive and distribute USDC revenue
///
/// Splits incoming USDC revenue according to immutable percentages:
/// - 50% to holder pool (for proportional distribution to stakers)
/// - 40% to R&D multisig
/// - 5% to operations multisig
/// - 5% to reserve vault (also receives rounding remainder)
///
/// Creates an immutable distribution record for transparency.
///
/// # Arguments
/// * `amount` - Amount of USDC to receive (in smallest unit)
/// * `payment_reference` - Reference ID for tracking (e.g., invoice ID)
///
pub fn receive_revenue(
    ctx: Context<ReceiveRevenue>,
    amount: u64,
    payment_reference: String,
) -> Result<()> {
    require!(amount > 0, TreasuryError::ZeroRevenueAmount);
    require!(
        payment_reference.len() <= Distribution::MAX_PAYMENT_REF_LEN,
        TreasuryError::PaymentReferenceTooLong
    );
    require!(
        !payment_reference.is_empty(),
        TreasuryError::MissingPaymentReference
    );

    let treasury_config = &mut ctx.accounts.treasury_config;
    let holder_pool = &mut ctx.accounts.holder_pool;
    let distribution = &mut ctx.accounts.distribution;
    let clock = Clock::get()?;

    // Calculate distribution amounts using checked arithmetic
    // All percentages are basis points (100 = 1%)

    // Holder share: 50%
    let holder_amount = amount
        .checked_mul(HOLDER_SHARE_BPS as u64)
        .ok_or(TreasuryError::ArithmeticOverflow)?
        .checked_div(BPS_DENOMINATOR as u64)
        .ok_or(TreasuryError::DivisionByZero)?;

    // R&D share: 40%
    let rnd_amount = amount
        .checked_mul(RND_SHARE_BPS as u64)
        .ok_or(TreasuryError::ArithmeticOverflow)?
        .checked_div(BPS_DENOMINATOR as u64)
        .ok_or(TreasuryError::DivisionByZero)?;

    // Operations share: 5%
    let ops_amount = amount
        .checked_mul(OPS_SHARE_BPS as u64)
        .ok_or(TreasuryError::ArithmeticOverflow)?
        .checked_div(BPS_DENOMINATOR as u64)
        .ok_or(TreasuryError::DivisionByZero)?;

    // Reserve base: 5%
    let reserve_base = amount
        .checked_mul(RESERVE_SHARE_BPS as u64)
        .ok_or(TreasuryError::ArithmeticOverflow)?
        .checked_div(BPS_DENOMINATOR as u64)
        .ok_or(TreasuryError::DivisionByZero)?;

    // Calculate distributed amount
    let distributed = holder_amount
        .checked_add(rnd_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?
        .checked_add(ops_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?
        .checked_add(reserve_base)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    // Reserve gets base amount + any remainder from rounding
    let reserve_amount = reserve_base
        .checked_add(amount.checked_sub(distributed).ok_or(TreasuryError::ArithmeticUnderflow)?)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    // Verify total equals input amount (critical invariant)
    let total_check = holder_amount
        .checked_add(rnd_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?
        .checked_add(ops_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?
        .checked_add(reserve_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    require!(
        total_check == amount,
        TreasuryError::RevenueSplitError
    );

    // Get PDA signer seeds for transfers
    let treasury_seeds = &[seeds::TREASURY_CONFIG, &[treasury_config.bump]];
    let signer_seeds = &[&treasury_seeds[..]];

    // Transfer to holder pool (stays in treasury-controlled account)
    let holder_pool_account = &ctx.accounts.holder_pool_usdc;
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.treasury_usdc_vault.to_account_info(),
                to: holder_pool_account.to_account_info(),
                authority: treasury_config.to_account_info(),
            },
            signer_seeds,
        ),
        holder_amount,
    )?;

    // Transfer to R&D multisig
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.treasury_usdc_vault.to_account_info(),
                to: ctx.accounts.rnd_usdc_account.to_account_info(),
                authority: treasury_config.to_account_info(),
            },
            signer_seeds,
        ),
        rnd_amount,
    )?;

    // Transfer to operations multisig
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.treasury_usdc_vault.to_account_info(),
                to: ctx.accounts.ops_usdc_account.to_account_info(),
                authority: treasury_config.to_account_info(),
            },
            signer_seeds,
        ),
        ops_amount,
    )?;

    // Transfer to reserve vault (includes rounding)
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.treasury_usdc_vault.to_account_info(),
                to: ctx.accounts.reserve_vault.to_account_info(),
                authority: treasury_config.to_account_info(),
            },
            signer_seeds,
        ),
        reserve_amount,
    )?;

    // Update treasury state
    treasury_config.total_usdc_received = treasury_config
        .total_usdc_received
        .checked_add(amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.total_usdc_to_holders = treasury_config
        .total_usdc_to_holders
        .checked_add(holder_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.total_usdc_to_rnd = treasury_config
        .total_usdc_to_rnd
        .checked_add(rnd_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.total_usdc_to_ops = treasury_config
        .total_usdc_to_ops
        .checked_add(ops_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.total_usdc_to_reserve = treasury_config
        .total_usdc_to_reserve
        .checked_add(reserve_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.distribution_count = treasury_config
        .distribution_count
        .checked_add(1)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.last_distribution_at = clock.unix_timestamp;

    // Update holder pool state
    holder_pool.usdc_balance = holder_pool
        .usdc_balance
        .checked_add(holder_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    holder_pool.total_usdc_deposited = holder_pool
        .total_usdc_deposited
        .checked_add(holder_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    holder_pool.last_deposit_at = clock.unix_timestamp;

    // Create distribution record
    distribution.index = treasury_config.distribution_count;
    distribution.timestamp = clock.unix_timestamp;
    distribution.distribution_type = DistributionType::UsdcRevenue;
    distribution.total_amount = amount;
    distribution.amount_to_holders = holder_amount;
    distribution.amount_to_rnd = rnd_amount;
    distribution.amount_to_ops = ops_amount;
    distribution.amount_to_reserve = reserve_amount;
    distribution.amount_burned = 0; // Not applicable for USDC
    distribution.payment_reference = payment_reference.clone();
    distribution.tx_signature = String::new(); // Can be set by client
    distribution.bump = ctx.bumps.distribution;

    msg!("USDC revenue received and distributed");
    msg!("Total amount: {}", amount);
    msg!("To holders: {} (50%)", holder_amount);
    msg!("To R&D: {} (40%)", rnd_amount);
    msg!("To ops: {} (5%)", ops_amount);
    msg!("To reserve: {} (5% + {} rounding)", reserve_amount, reserve_amount - reserve_base);
    msg!("Payment reference: {}", payment_reference);

    Ok(())
}

#[derive(Accounts)]
#[instruction(amount: u64, payment_reference: String)]
pub struct ReceiveRevenue<'info> {
    /// Payer of transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Treasury configuration
    #[account(
        mut,
        seeds = [seeds::TREASURY_CONFIG],
        bump = treasury_config.bump,
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,

    /// Holder pool state
    #[account(
        mut,
        seeds = [seeds::HOLDER_POOL],
        bump = holder_pool.bump,
    )]
    pub holder_pool: Account<'info, HolderPool>,

    /// Distribution record (created for this transaction)
    #[account(
        init,
        payer = payer,
        space = Distribution::LEN,
        seeds = [
            seeds::DISTRIBUTION,
            &treasury_config.distribution_count.checked_add(1).unwrap().to_le_bytes()
        ],
        bump
    )]
    pub distribution: Account<'info, Distribution>,

    /// Treasury USDC vault (receives revenue initially)
    #[account(
        mut,
        seeds = [seeds::TREASURY_USDC],
        bump,
        token::mint = treasury_config.usdc_mint,
        token::authority = treasury_config,
    )]
    pub treasury_usdc_vault: Account<'info, TokenAccount>,

    /// Holder pool USDC account
    #[account(
        mut,
        token::mint = treasury_config.usdc_mint,
    )]
    pub holder_pool_usdc: Account<'info, TokenAccount>,

    /// R&D multisig USDC account
    #[account(
        mut,
        token::mint = treasury_config.usdc_mint,
        token::authority = treasury_config.rnd_multisig,
    )]
    pub rnd_usdc_account: Account<'info, TokenAccount>,

    /// Operations multisig USDC account
    #[account(
        mut,
        token::mint = treasury_config.usdc_mint,
        token::authority = treasury_config.ops_multisig,
    )]
    pub ops_usdc_account: Account<'info, TokenAccount>,

    /// Reserve vault
    #[account(
        mut,
        seeds = [seeds::RESERVE_VAULT],
        bump,
        token::mint = treasury_config.usdc_mint,
        token::authority = treasury_config,
    )]
    pub reserve_vault: Account<'info, TokenAccount>,

    /// SPL Token program
    pub token_program: Program<'info, Token>,

    /// System program
    pub system_program: Program<'info, System>,
}

// ============================================================================
// Receive AMOS Payment
// ============================================================================

/// Receive and distribute AMOS token payment
///
/// When users pay with AMOS tokens (with 20% discount), the tokens are:
/// - 50% burned (deflationary mechanism)
/// - 50% distributed to holder pool
///
/// Creates an immutable distribution record for transparency.
///
/// # Arguments
/// * `amount` - Amount of AMOS tokens received
/// * `payment_reference` - Reference ID for tracking
///
pub fn receive_amos_payment(
    ctx: Context<ReceiveAmosPayment>,
    amount: u64,
    payment_reference: String,
) -> Result<()> {
    require!(amount > 0, TreasuryError::ZeroRevenueAmount);
    require!(
        payment_reference.len() <= Distribution::MAX_PAYMENT_REF_LEN,
        TreasuryError::PaymentReferenceTooLong
    );
    require!(
        !payment_reference.is_empty(),
        TreasuryError::MissingPaymentReference
    );

    let treasury_config = &mut ctx.accounts.treasury_config;
    let holder_pool = &mut ctx.accounts.holder_pool;
    let distribution = &mut ctx.accounts.distribution;
    let clock = Clock::get()?;

    // Calculate distribution using checked arithmetic
    // Burn: 50%
    let burn_amount = amount
        .checked_mul(AMOS_BURN_BPS as u64)
        .ok_or(TreasuryError::ArithmeticOverflow)?
        .checked_div(BPS_DENOMINATOR as u64)
        .ok_or(TreasuryError::DivisionByZero)?;

    // Holder share: 50% + any rounding remainder
    let holder_base = amount
        .checked_mul(AMOS_HOLDER_BPS as u64)
        .ok_or(TreasuryError::ArithmeticOverflow)?
        .checked_div(BPS_DENOMINATOR as u64)
        .ok_or(TreasuryError::DivisionByZero)?;

    // Calculate distributed amount
    let distributed = burn_amount
        .checked_add(holder_base)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    // Holders get base + remainder
    let holder_amount = holder_base
        .checked_add(amount.checked_sub(distributed).ok_or(TreasuryError::ArithmeticUnderflow)?)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    // Verify total equals input (critical invariant)
    let total_check = burn_amount
        .checked_add(holder_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    require!(
        total_check == amount,
        TreasuryError::RevenueSplitError
    );

    // Get PDA signer seeds
    let treasury_seeds = &[seeds::TREASURY_CONFIG, &[treasury_config.bump]];
    let signer_seeds = &[&treasury_seeds[..]];

    // Burn tokens
    token::burn(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.amos_mint.to_account_info(),
                from: ctx.accounts.treasury_amos_vault.to_account_info(),
                authority: treasury_config.to_account_info(),
            },
            signer_seeds,
        ),
        burn_amount,
    )?;

    // Transfer to holder pool
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.treasury_amos_vault.to_account_info(),
                to: ctx.accounts.holder_pool_amos.to_account_info(),
                authority: treasury_config.to_account_info(),
            },
            signer_seeds,
        ),
        holder_amount,
    )?;

    // Update treasury state
    treasury_config.total_amos_received = treasury_config
        .total_amos_received
        .checked_add(amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.total_amos_burned = treasury_config
        .total_amos_burned
        .checked_add(burn_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.total_amos_to_holders = treasury_config
        .total_amos_to_holders
        .checked_add(holder_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.distribution_count = treasury_config
        .distribution_count
        .checked_add(1)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    treasury_config.last_distribution_at = clock.unix_timestamp;

    // Update holder pool state
    holder_pool.amos_balance = holder_pool
        .amos_balance
        .checked_add(holder_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    holder_pool.total_amos_deposited = holder_pool
        .total_amos_deposited
        .checked_add(holder_amount)
        .ok_or(TreasuryError::ArithmeticOverflow)?;

    holder_pool.last_deposit_at = clock.unix_timestamp;

    // Create distribution record
    distribution.index = treasury_config.distribution_count;
    distribution.timestamp = clock.unix_timestamp;
    distribution.distribution_type = DistributionType::AmosPayment;
    distribution.total_amount = amount;
    distribution.amount_to_holders = holder_amount;
    distribution.amount_to_rnd = 0; // Not applicable for AMOS
    distribution.amount_to_ops = 0; // Not applicable for AMOS
    distribution.amount_to_reserve = 0; // Not applicable for AMOS
    distribution.amount_burned = burn_amount;
    distribution.payment_reference = payment_reference.clone();
    distribution.tx_signature = String::new();
    distribution.bump = ctx.bumps.distribution;

    msg!("AMOS payment received and distributed");
    msg!("Total amount: {}", amount);
    msg!("Burned: {} (50%)", burn_amount);
    msg!("To holders: {} (50% + {} rounding)", holder_amount, holder_amount - holder_base);
    msg!("Payment reference: {}", payment_reference);

    Ok(())
}

#[derive(Accounts)]
#[instruction(amount: u64, payment_reference: String)]
pub struct ReceiveAmosPayment<'info> {
    /// Payer of transaction fees
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Treasury configuration
    #[account(
        mut,
        seeds = [seeds::TREASURY_CONFIG],
        bump = treasury_config.bump,
    )]
    pub treasury_config: Account<'info, TreasuryConfig>,

    /// Holder pool state
    #[account(
        mut,
        seeds = [seeds::HOLDER_POOL],
        bump = holder_pool.bump,
    )]
    pub holder_pool: Account<'info, HolderPool>,

    /// Distribution record
    #[account(
        init,
        payer = payer,
        space = Distribution::LEN,
        seeds = [
            seeds::DISTRIBUTION,
            &treasury_config.distribution_count.checked_add(1).unwrap().to_le_bytes()
        ],
        bump
    )]
    pub distribution: Account<'info, Distribution>,

    /// AMOS token mint (for burning)
    #[account(
        mut,
        address = treasury_config.amos_mint,
    )]
    pub amos_mint: Account<'info, Mint>,

    /// Treasury AMOS vault
    #[account(
        mut,
        seeds = [seeds::TREASURY_AMOS],
        bump,
        token::mint = treasury_config.amos_mint,
        token::authority = treasury_config,
    )]
    pub treasury_amos_vault: Account<'info, TokenAccount>,

    /// Holder pool AMOS account
    #[account(
        mut,
        token::mint = treasury_config.amos_mint,
    )]
    pub holder_pool_amos: Account<'info, TokenAccount>,

    /// SPL Token program
    pub token_program: Program<'info, Token>,

    /// System program
    pub system_program: Program<'info, System>,
}
