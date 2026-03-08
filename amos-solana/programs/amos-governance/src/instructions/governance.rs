// AMOS Governance Program - Governance Instructions
// Handles governance initialization, parameter updates, and budget gate operations

use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use crate::constants::*;
use crate::errors::GovernanceError;
use crate::state::*;

// ============================================================================
// Initialize Governance
// ============================================================================

/// Initializes the governance system with configuration parameters
#[derive(Accounts)]
pub struct InitializeGovernance<'info> {
    #[account(
        init,
        payer = authority,
        space = GOVERNANCE_CONFIG_SIZE,
        seeds = [GOVERNANCE_SEED],
        bump
    )]
    pub governance_config: Account<'info, GovernanceConfig>,

    #[account(
        init,
        payer = authority,
        space = GOVERNANCE_PARAMS_SIZE
    )]
    pub governance_params: Account<'info, StoredGovernanceParams>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// Oracle account for reporting gate results
    /// CHECK: Validated by authority
    pub oracle: UncheckedAccount<'info>,

    /// AMOS token mint
    pub mint: Account<'info, Mint>,

    /// Treasury token account
    #[account(
        constraint = treasury.mint == mint.key() @ GovernanceError::InvalidMint,
        constraint = treasury.owner == governance_config.key() @ GovernanceError::InvalidAccount
    )]
    pub treasury: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_governance(
    ctx: Context<InitializeGovernance>,
    params: Option<StoredGovernanceParams>,
) -> Result<()> {
    let governance = &mut ctx.accounts.governance_config;
    let stored_params = &mut ctx.accounts.governance_params;

    // Set governance config
    governance.authority = ctx.accounts.authority.key();
    governance.oracle = ctx.accounts.oracle.key();
    governance.mint = ctx.accounts.mint.key();
    governance.treasury = ctx.accounts.treasury.key();
    governance.params = stored_params.key();
    governance.total_proposals = 0;
    governance.total_votes = 0;
    governance.total_bounties_paid = 0;
    governance.bump = ctx.bumps.governance_config;
    governance.reserved = [0; 128];

    // Set parameters (use defaults if not provided)
    *stored_params = params.unwrap_or_default();

    // Validate parameter sums
    require!(
        stored_params.bounty_completion_bps
            .checked_add(stored_params.bounty_ab_bps)
            .and_then(|sum| sum.checked_add(stored_params.bounty_merge_bps))
            == Some(BPS_DENOMINATOR),
        GovernanceError::InvalidParameterSum
    );

    msg!("Governance initialized with authority: {}", governance.authority);
    msg!("Oracle: {}", governance.oracle);
    msg!("Mint: {}", governance.mint);

    Ok(())
}

// ============================================================================
// Update Governance Parameters
// ============================================================================

/// Updates governance parameters (authority only)
#[derive(Accounts)]
pub struct UpdateGovernanceParams<'info> {
    #[account(
        seeds = [GOVERNANCE_SEED],
        bump = governance_config.bump
    )]
    pub governance_config: Account<'info, GovernanceConfig>,

    #[account(
        mut,
        constraint = governance_config.params == governance_params.key() @ GovernanceError::InvalidAccount
    )]
    pub governance_params: Account<'info, StoredGovernanceParams>,

    #[account(
        mut,
        constraint = authority.key() == governance_config.authority @ GovernanceError::Unauthorized
    )]
    pub authority: Signer<'info>,
}

pub fn update_governance_params(
    ctx: Context<UpdateGovernanceParams>,
    new_params: StoredGovernanceParams,
) -> Result<()> {
    let params = &mut ctx.accounts.governance_params;

    // Validate bounty split sums to 100%
    let bounty_sum = new_params.bounty_completion_bps
        .checked_add(new_params.bounty_ab_bps)
        .and_then(|sum| sum.checked_add(new_params.bounty_merge_bps))
        .ok_or(GovernanceError::ArithmeticOverflow)?;

    require!(
        bounty_sum == BPS_DENOMINATOR,
        GovernanceError::InvalidParameterSum
    );

    // Validate priority weights sum to 100%
    let priority_sum = new_params.mrr_weight_bps
        .checked_add(new_params.community_weight_bps)
        .ok_or(GovernanceError::ArithmeticOverflow)?;

    require!(
        priority_sum == BPS_DENOMINATOR,
        GovernanceError::InvalidParameterSum
    );

    // Update parameters
    *params = new_params;

    msg!("Governance parameters updated");

    Ok(())
}

// ============================================================================
// Submit Budget Gate Proposal
// ============================================================================

/// Submits a proposal to change budget allocation thresholds
#[derive(Accounts)]
#[instruction(proposal_id: u64)]
pub struct SubmitBudgetGateProposal<'info> {
    #[account(
        seeds = [GOVERNANCE_SEED],
        bump = governance_config.bump
    )]
    pub governance_config: Account<'info, GovernanceConfig>,

    #[account(
        init,
        payer = proposer,
        space = BUDGET_GATE_SIZE,
        seeds = [BUDGET_GATE_SEED, proposal_id.to_le_bytes().as_ref()],
        bump
    )]
    pub budget_proposal: Account<'info, BudgetGateProposal>,

    #[account(mut)]
    pub proposer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn submit_budget_gate_proposal(
    ctx: Context<SubmitBudgetGateProposal>,
    proposal_id: u64,
    title: String,
    new_budget_threshold: u64,
) -> Result<()> {
    require!(title.len() <= MAX_TITLE_LEN, GovernanceError::TitleTooLong);
    require!(
        new_budget_threshold > 0,
        GovernanceError::InvalidBudgetThreshold
    );

    let proposal = &mut ctx.accounts.budget_proposal;
    let clock = Clock::get()?;

    proposal.id = proposal_id;
    proposal.proposer = ctx.accounts.proposer.key();
    proposal.title = title;
    proposal.new_budget_threshold = new_budget_threshold;
    proposal.submitted_at = clock.unix_timestamp;
    proposal.executed_at = None;
    proposal.yes_votes = 0;
    proposal.no_votes = 0;
    proposal.status = BudgetGateStatus::Voting;
    proposal.bump = ctx.bumps.budget_proposal;
    proposal.reserved = [0; 128];

    msg!("Budget gate proposal {} submitted by {}", proposal_id, proposal.proposer);

    Ok(())
}

// ============================================================================
// Cast Steward Vote
// ============================================================================

/// Allows stewards to vote on budget gate proposals
#[derive(Accounts)]
#[instruction(proposal_id: u64, vote_type: StewardVoteType)]
pub struct CastStewardVote<'info> {
    #[account(
        seeds = [GOVERNANCE_SEED],
        bump = governance_config.bump
    )]
    pub governance_config: Account<'info, GovernanceConfig>,

    #[account(
        mut,
        seeds = [BUDGET_GATE_SEED, proposal_id.to_le_bytes().as_ref()],
        bump = budget_proposal.bump
    )]
    pub budget_proposal: Account<'info, BudgetGateProposal>,

    #[account(
        init,
        payer = steward,
        space = STEWARD_VOTE_SIZE,
        seeds = [
            STEWARD_VOTE_SEED,
            proposal_id.to_le_bytes().as_ref(),
            steward.key().as_ref(),
            &[vote_type as u8]
        ],
        bump
    )]
    pub vote_record: Account<'info, StewardVoteRecord>,

    #[account(mut)]
    pub steward: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn cast_steward_vote(
    ctx: Context<CastStewardVote>,
    proposal_id: u64,
    vote_type: StewardVoteType,
    approve: bool,
) -> Result<()> {
    let proposal = &mut ctx.accounts.budget_proposal;
    let vote_record = &mut ctx.accounts.vote_record;
    let params = &ctx.accounts.governance_config;

    // Ensure proposal is in voting state
    require!(
        proposal.status == BudgetGateStatus::Voting,
        GovernanceError::BudgetGateVotingEnded
    );

    // Record vote
    vote_record.steward = ctx.accounts.steward.key();
    vote_record.proposal_id = proposal_id;
    vote_record.vote_type = vote_type;
    vote_record.approve = approve;
    vote_record.voted_at = Clock::get()?.unix_timestamp;
    vote_record.bump = ctx.bumps.vote_record;
    vote_record.reserved = [0; 64];

    // Update vote counts
    if approve {
        proposal.yes_votes = proposal.yes_votes
            .checked_add(1)
            .ok_or(GovernanceError::ArithmeticOverflow)?;
    } else {
        proposal.no_votes = proposal.no_votes
            .checked_add(1)
            .ok_or(GovernanceError::ArithmeticOverflow)?;
    }

    msg!("Steward {} voted {} on proposal {}",
        vote_record.steward,
        if approve { "yes" } else { "no" },
        proposal_id
    );

    // Check if quorum reached - need to load params properly
    // For now, using default quorum
    let quorum = DEFAULT_STEWARD_QUORUM;
    if proposal.yes_votes >= quorum as u8 {
        proposal.status = BudgetGateStatus::Approved;
        msg!("Budget gate proposal {} approved", proposal_id);
    } else if proposal.no_votes >= quorum as u8 {
        proposal.status = BudgetGateStatus::Rejected;
        msg!("Budget gate proposal {} rejected", proposal_id);
    }

    Ok(())
}

// ============================================================================
// Execute Budget Profile Activation
// ============================================================================

/// Executes an approved budget gate proposal
#[derive(Accounts)]
#[instruction(proposal_id: u64, profile_id: u64)]
pub struct ExecuteBudgetProfileActivation<'info> {
    #[account(
        seeds = [GOVERNANCE_SEED],
        bump = governance_config.bump
    )]
    pub governance_config: Account<'info, GovernanceConfig>,

    #[account(
        mut,
        seeds = [BUDGET_GATE_SEED, proposal_id.to_le_bytes().as_ref()],
        bump = budget_proposal.bump,
        constraint = budget_proposal.status == BudgetGateStatus::Approved @ GovernanceError::BudgetGateNotApproved
    )]
    pub budget_proposal: Account<'info, BudgetGateProposal>,

    #[account(
        init_if_needed,
        payer = authority,
        space = ALLOCATION_PROFILE_SIZE,
        seeds = [ALLOCATION_PROFILE_SEED, profile_id.to_le_bytes().as_ref()],
        bump
    )]
    pub allocation_profile: Account<'info, AllocationProfile>,

    #[account(
        mut,
        constraint = authority.key() == governance_config.authority @ GovernanceError::Unauthorized
    )]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn execute_budget_profile_activation(
    ctx: Context<ExecuteBudgetProfileActivation>,
    proposal_id: u64,
    profile_id: u64,
    name: String,
    feature_dev_bps: u16,
    research_bps: u16,
    maintenance_bps: u16,
    security_bps: u16,
) -> Result<()> {
    let proposal = &mut ctx.accounts.budget_proposal;
    let profile = &mut ctx.accounts.allocation_profile;

    // Validate allocation percentages sum to 100%
    let total_bps = feature_dev_bps
        .checked_add(research_bps)
        .and_then(|sum| sum.checked_add(maintenance_bps))
        .and_then(|sum| sum.checked_add(security_bps))
        .ok_or(GovernanceError::ArithmeticOverflow)?;

    require!(
        total_bps == BPS_DENOMINATOR,
        GovernanceError::InvalidAllocationSum
    );

    // Update allocation profile
    profile.id = profile_id;
    profile.name = name;
    profile.feature_dev_bps = feature_dev_bps;
    profile.research_bps = research_bps;
    profile.maintenance_bps = maintenance_bps;
    profile.security_bps = security_bps;
    profile.active = true;
    profile.activated_at = Some(Clock::get()?.unix_timestamp);
    profile.bump = ctx.bumps.allocation_profile;
    profile.reserved = [0; 64];

    // Mark proposal as executed
    proposal.status = BudgetGateStatus::Executed;
    proposal.executed_at = Some(Clock::get()?.unix_timestamp);

    msg!("Budget profile {} activated for proposal {}", profile_id, proposal_id);

    Ok(())
}
