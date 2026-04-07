/// AMOS Bounty Program - Distribution Instructions
///
/// This module handles the core bounty submission and token distribution logic.
/// It implements trustless, transparent token allocation based on contribution value.

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::constants::*;
use crate::errors::BountyError;
use crate::state::*;

// ============================================================================
// Submit Bounty Proof
// ============================================================================

/// Submit a validated bounty proof and distribute tokens proportionally.
///
/// This is the CORE distribution mechanism. Token allocation is calculated as:
/// `tokens = (adjusted_points / total_points_today) × remaining_daily_emission`
///
/// # Arguments
/// * `bounty_id` - Unique identifier for this bounty (32 bytes)
/// * `base_points` - Base point value before multipliers (1-2000)
/// * `quality_score` - Quality assessment (30-100)
/// * `contribution_type` - Type of work (0-7)
/// * `is_agent` - Whether this is an AI agent submission
/// * `agent_id` - Agent identifier if applicable
/// * `reviewer` - Address of the reviewer who validated this work
/// * `evidence_hash` - Hash of the work product/evidence
/// * `external_reference` - External ID (issue number, PR number, etc.)
///
/// # Trustless Guarantees
/// - Oracle-only submission (only validated work is accepted)
/// - Proportional distribution (fair share based on contribution value)
/// - Trust level enforcement (agents capped by reputation)
/// - Daily limits (prevents gaming through volume)
/// - Contribution multipliers (transparent value weighting)
/// - Reviewer rewards (5% incentivizes quality validation)
/// - All calculations use checked arithmetic (no overflow/underflow)
/// - Immutable records (complete audit trail)
#[derive(Accounts)]
#[instruction(bounty_id: [u8; 32], base_points: u16, quality_score: u8, contribution_type: u8, is_agent: bool, agent_id: [u8; 32])]
pub struct SubmitBountyProof<'info> {
    #[account(
        mut,
        seeds = [BOUNTY_CONFIG_SEED],
        bump = config.bump,
        has_one = oracle_authority @ BountyError::Unauthorized,
        has_one = mint @ BountyError::InvalidMint,
        has_one = treasury @ BountyError::InvalidTreasury
    )]
    pub config: Account<'info, BountyConfig>,

    #[account(
        init_if_needed,
        payer = oracle_authority,
        space = DailyPool::SIZE,
        seeds = [DAILY_POOL_SEED, &calculate_day_index(config.start_time)?.to_le_bytes()],
        bump
    )]
    pub daily_pool: Account<'info, DailyPool>,

    #[account(
        init,
        payer = oracle_authority,
        space = BountyProof::SIZE,
        seeds = [BOUNTY_PROOF_SEED, &bounty_id],
        bump
    )]
    pub bounty_proof: Account<'info, BountyProof>,

    #[account(
        init_if_needed,
        payer = oracle_authority,
        space = OperatorStats::SIZE,
        seeds = [OPERATOR_STATS_SEED, operator.key().as_ref()],
        bump
    )]
    pub operator_stats: Account<'info, OperatorStats>,

    /// The operator earning this bounty
    /// CHECK: This is validated through the operator_stats PDA derivation
    pub operator: AccountInfo<'info>,

    /// Optional agent trust record (required if is_agent = true)
    #[account(
        mut,
        seeds = [AGENT_TRUST_SEED, &agent_id],
        bump = agent_trust.bump
    )]
    pub agent_trust: Option<Account<'info, AgentTrustRecord>>,

    pub mint: Account<'info, Mint>,

    #[account(mut)]
    pub treasury: Account<'info, TokenAccount>,

    /// Operator's token account (receives bounty tokens)
    #[account(
        mut,
        constraint = operator_token_account.mint == mint.key() @ BountyError::InvalidMint,
        constraint = operator_token_account.owner == operator.key() @ BountyError::InvalidOperator
    )]
    pub operator_token_account: Account<'info, TokenAccount>,

    /// Reviewer's token account (receives 5% reward)
    #[account(
        mut,
        constraint = reviewer_token_account.mint == mint.key() @ BountyError::InvalidMint
    )]
    pub reviewer_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub oracle_authority: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler_submit_proof(
    ctx: Context<SubmitBountyProof>,
    bounty_id: [u8; 32],
    base_points: u16,
    quality_score: u8,
    contribution_type: u8,
    is_agent: bool,
    agent_id: [u8; 32],
    reviewer: Pubkey,
    evidence_hash: [u8; 32],
    external_reference: [u8; 64],
) -> Result<()> {
    let clock = Clock::get()?;
    let config = &mut ctx.accounts.config;
    let daily_pool = &mut ctx.accounts.daily_pool;
    let bounty_proof = &mut ctx.accounts.bounty_proof;
    let operator_stats = &mut ctx.accounts.operator_stats;

    // ========================================================================
    // Validation Phase
    // ========================================================================

    // Validate quality score
    require!(
        quality_score >= MIN_QUALITY_SCORE,
        BountyError::QualityScoreTooLow
    );

    // Validate contribution type
    require!(
        contribution_type <= 7,
        BountyError::InvalidContributionType
    );

    // Validate base points
    require!(
        base_points > 0 && base_points <= MAX_BOUNTY_POINTS,
        BountyError::InvalidBountyPoints
    );

    // Validate reviewer is different from operator
    require!(
        reviewer != ctx.accounts.operator.key(),
        BountyError::ReviewerSameAsOperator
    );

    // Validate evidence hash is not empty
    require!(
        evidence_hash != [0u8; 32],
        BountyError::InvalidEvidenceHash
    );

    // Initialize operator stats if needed
    if operator_stats.operator == Pubkey::default() {
        operator_stats.operator = ctx.accounts.operator.key();
        operator_stats.bump = ctx.bumps.operator_stats;
        operator_stats.last_activity_time = clock.unix_timestamp;
        operator_stats.last_decay_time = clock.unix_timestamp;
        operator_stats.original_allocation = 0;
    }

    // Calculate current day index
    let current_day = calculate_day_index(config.start_time)?;

    // Reset daily counter if new day
    if operator_stats.last_submission_day != current_day {
        operator_stats.daily_bounty_count = 0;
        operator_stats.last_submission_day = current_day;
    }

    // Initialize daily pool if needed
    if daily_pool.day_index == 0 {
        daily_pool.day_index = current_day;
        daily_pool.daily_emission = config.daily_emission;
        daily_pool.tokens_distributed = 0;
        daily_pool.total_points = 0;
        daily_pool.proof_count = 0;
        daily_pool.finalized = false;
        daily_pool.bump = ctx.bumps.daily_pool;
    }

    // Verify pool is not finalized
    require!(
        !daily_pool.finalized,
        BountyError::DailyPoolAlreadyFinalized
    );

    // ========================================================================
    // Trust Level Enforcement (for AI agents)
    // ========================================================================

    let mut trust_level: u8 = 1; // Default for human operators

    if is_agent {
        // Agent must have trust record
        let agent_trust = ctx
            .accounts
            .agent_trust
            .as_ref()
            .ok_or(BountyError::AgentNotRegistered)?;

        trust_level = agent_trust.trust_level;

        // Check trust level point cap
        let max_points = get_max_points_for_trust_level(trust_level)?;
        require!(
            base_points <= max_points,
            BountyError::InvalidBountyPoints
        );

        // Check daily limit for this trust level
        let daily_limit = get_daily_limit_for_trust_level(trust_level)?;
        require!(
            operator_stats.daily_bounty_count < daily_limit,
            BountyError::DailyLimitExceeded
        );
    } else {
        // Human operators have the max daily limit
        require!(
            operator_stats.daily_bounty_count < MAX_DAILY_BOUNTIES_PER_OPERATOR,
            BountyError::DailyLimitExceeded
        );
    }

    // ========================================================================
    // Apply Contribution Type Multiplier
    // ========================================================================

    let multiplier_bps = get_contribution_multiplier(contribution_type)?;

    // Calculate adjusted points: base_points × (multiplier / 10000)
    let adjusted_points = (base_points as u64)
        .checked_mul(multiplier_bps as u64)
        .ok_or(BountyError::ArithmeticOverflow)?
        .checked_div(BPS_DENOMINATOR as u64)
        .ok_or(BountyError::ArithmeticOverflow)? as u16;

    // Ensure adjusted points don't exceed max after multiplier
    let adjusted_points = adjusted_points.min(MAX_BOUNTY_POINTS);

    // ========================================================================
    // Token Distribution Calculation
    // ========================================================================

    // Calculate remaining emission for today
    let remaining_emission = daily_pool
        .daily_emission
        .checked_sub(daily_pool.tokens_distributed)
        .ok_or(BountyError::InsufficientEmission)?;

    require!(remaining_emission > 0, BountyError::InsufficientEmission);

    // Proportional calculation:
    // tokens = (adjusted_points / total_points_including_this_one) × remaining_emission
    //
    // To avoid division issues, we calculate this way:
    // tokens = (adjusted_points × remaining_emission) / (total_points + adjusted_points)

    let new_total_points = daily_pool
        .total_points
        .checked_add(adjusted_points as u64)
        .ok_or(BountyError::ArithmeticOverflow)?;

    let tokens_before_split = (adjusted_points as u64)
        .checked_mul(remaining_emission)
        .ok_or(BountyError::ArithmeticOverflow)?
        .checked_div(new_total_points)
        .ok_or(BountyError::ArithmeticOverflow)?;

    // Ensure at least 1 token if points awarded
    let tokens_before_split = tokens_before_split.max(1);

    // Split tokens: 95% to operator, 5% to reviewer
    let reviewer_tokens = tokens_before_split
        .checked_mul(REVIEWER_REWARD_BPS as u64)
        .ok_or(BountyError::ArithmeticOverflow)?
        .checked_div(BPS_DENOMINATOR as u64)
        .ok_or(BountyError::ArithmeticOverflow)?;

    let operator_tokens = tokens_before_split
        .checked_sub(reviewer_tokens)
        .ok_or(BountyError::ArithmeticUnderflow)?;

    require!(operator_tokens > 0, BountyError::ZeroTokensCalculated);

    // ========================================================================
    // Transfer Tokens
    // ========================================================================

    // Transfer to operator
    let _treasury_key = ctx.accounts.treasury.key();
    let config_seeds = &[BOUNTY_CONFIG_SEED, &[config.bump]];
    let signer_seeds = &[&config_seeds[..]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.treasury.to_account_info(),
                to: ctx.accounts.operator_token_account.to_account_info(),
                authority: config.to_account_info(),
            },
            signer_seeds,
        ),
        operator_tokens,
    )?;

    // Transfer to reviewer
    if reviewer_tokens > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.treasury.to_account_info(),
                    to: ctx.accounts.reviewer_token_account.to_account_info(),
                    authority: config.to_account_info(),
                },
                signer_seeds,
            ),
            reviewer_tokens,
        )?;
    }

    // ========================================================================
    // Update State
    // ========================================================================

    // Update daily pool
    daily_pool.tokens_distributed = daily_pool
        .tokens_distributed
        .checked_add(tokens_before_split)
        .ok_or(BountyError::ArithmeticOverflow)?;

    daily_pool.total_points = new_total_points;

    daily_pool.proof_count = daily_pool
        .proof_count
        .checked_add(1)
        .ok_or(BountyError::ArithmeticOverflow)?;

    // Update operator stats
    operator_stats.total_bounties = operator_stats
        .total_bounties
        .checked_add(1)
        .ok_or(BountyError::ArithmeticOverflow)?;

    operator_stats.total_points = operator_stats
        .total_points
        .checked_add(adjusted_points as u64)
        .ok_or(BountyError::ArithmeticOverflow)?;

    operator_stats.total_tokens_earned = operator_stats
        .total_tokens_earned
        .checked_add(operator_tokens)
        .ok_or(BountyError::ArithmeticOverflow)?;

    operator_stats.decayable_balance = operator_stats
        .decayable_balance
        .checked_add(operator_tokens)
        .ok_or(BountyError::ArithmeticOverflow)?;

    operator_stats.original_allocation = operator_stats
        .original_allocation
        .checked_add(operator_tokens)
        .ok_or(BountyError::ArithmeticOverflow)?;

    operator_stats.daily_bounty_count = operator_stats
        .daily_bounty_count
        .checked_add(1)
        .ok_or(BountyError::ArithmeticOverflow)?;

    operator_stats.last_activity_time = clock.unix_timestamp;

    // Update global config
    config.total_tokens_distributed = config
        .total_tokens_distributed
        .checked_add(tokens_before_split)
        .ok_or(BountyError::ArithmeticOverflow)?;

    config.total_bounties = config
        .total_bounties
        .checked_add(1)
        .ok_or(BountyError::ArithmeticOverflow)?;

    config.total_points = config
        .total_points
        .checked_add(adjusted_points as u64)
        .ok_or(BountyError::ArithmeticOverflow)?;

    // Record bounty proof (immutable record)
    bounty_proof.bounty_id = bounty_id;
    bounty_proof.operator = ctx.accounts.operator.key();
    bounty_proof.base_points = base_points;
    bounty_proof.adjusted_points = adjusted_points;
    bounty_proof.quality_score = quality_score;
    bounty_proof.contribution_type = contribution_type;
    bounty_proof.is_agent = is_agent;
    bounty_proof.agent_id = agent_id;
    bounty_proof.trust_level = trust_level;
    bounty_proof.tokens_earned = operator_tokens;
    bounty_proof.reviewer = reviewer;
    bounty_proof.reviewer_tokens = reviewer_tokens;
    bounty_proof.evidence_hash = evidence_hash;
    bounty_proof.timestamp = clock.unix_timestamp;
    bounty_proof.day_index = current_day;
    bounty_proof.external_reference = external_reference;
    bounty_proof.bump = ctx.bumps.bounty_proof;
    bounty_proof.reserved = [0; 8];

    // Update agent trust record if applicable
    if is_agent {
        if let Some(agent_trust) = ctx.accounts.agent_trust.as_mut() {
            agent_trust.total_tokens_earned = agent_trust
                .total_tokens_earned
                .checked_add(operator_tokens)
                .ok_or(BountyError::ArithmeticOverflow)?;

            agent_trust.total_points_earned = agent_trust
                .total_points_earned
                .checked_add(adjusted_points as u64)
                .ok_or(BountyError::ArithmeticOverflow)?;

            agent_trust.last_activity = clock.unix_timestamp;
        }
    }

    // ========================================================================
    // Emit Event
    // ========================================================================

    emit!(BountySubmitted {
        bounty_id,
        operator: ctx.accounts.operator.key(),
        base_points,
        adjusted_points,
        operator_tokens,
        reviewer_tokens,
        day_index: current_day,
        timestamp: clock.unix_timestamp,
    });

    msg!("Bounty submitted successfully");
    msg!("Base points: {}, Adjusted points: {}", base_points, adjusted_points);
    msg!("Operator tokens: {}, Reviewer tokens: {}", operator_tokens, reviewer_tokens);

    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Calculate the current day index since program start
fn calculate_day_index(start_time: i64) -> Result<u32> {
    let clock = Clock::get()?;
    let elapsed = clock
        .unix_timestamp
        .checked_sub(start_time)
        .ok_or(BountyError::InvalidTimestamp)?;

    let days = (elapsed as u64)
        .checked_div(86400) // seconds per day
        .ok_or(BountyError::ArithmeticOverflow)?;

    Ok(days as u32)
}

// ============================================================================
// Events
// ============================================================================

#[event]
pub struct BountySubmitted {
    pub bounty_id: [u8; 32],
    pub operator: Pubkey,
    pub base_points: u16,
    pub adjusted_points: u16,
    pub operator_tokens: u64,
    pub reviewer_tokens: u64,
    pub day_index: u32,
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contribution_multipliers() {
        // Bug fix: 120%
        assert_eq!(get_contribution_multiplier(0).unwrap(), 12000);

        // Feature: 100%
        assert_eq!(get_contribution_multiplier(1).unwrap(), 10000);

        // Documentation: 80%
        assert_eq!(get_contribution_multiplier(2).unwrap(), 8000);

        // Infrastructure: 130%
        assert_eq!(get_contribution_multiplier(7).unwrap(), 13000);
    }

    #[test]
    fn test_reviewer_split() {
        let total_tokens = 10000u64;
        let reviewer_portion = total_tokens * REVIEWER_REWARD_BPS as u64 / BPS_DENOMINATOR as u64;
        let operator_portion = total_tokens - reviewer_portion;

        // Should be 5% to reviewer, 95% to operator
        assert_eq!(reviewer_portion, 500);
        assert_eq!(operator_portion, 9500);
    }

    #[test]
    fn test_proportional_distribution() {
        // Simulate: 1000 remaining emission, contributing 100 points when 900 already exist
        let remaining_emission = 1000u64;
        let adjusted_points = 100u64;
        let existing_points = 900u64;
        let new_total = existing_points + adjusted_points; // 1000

        let tokens = (adjusted_points * remaining_emission) / new_total;

        // Should get 100/1000 = 10% of remaining = 100 tokens
        assert_eq!(tokens, 100);
    }
}
