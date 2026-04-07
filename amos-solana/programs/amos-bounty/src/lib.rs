/// AMOS Bounty Program
///
/// A trustless, transparent token distribution system for rewarding contributor work.
///
/// # Core Features
///
/// 1. **Proportional Distribution**
///    - Daily emission pool divided among all contributors
///    - Tokens = (adjusted_points / total_points_today) × remaining_emission
///    - Fair share based on contribution value
///
/// 2. **Halving Schedule**
///    - Starts at 16,000 tokens/day
///    - Halves every 365 days
///    - Minimum floor of 100 tokens/day
///    - Maximum 10 halving epochs
///
/// 3. **Token Decay**
///    - Recycles unused tokens back to treasury
///    - 90-day grace period before decay begins
///    - 2-25% annual rate (default 5%)
///    - 10% floor preserved
///    - 10% burned, 90% recycled
///
/// 4. **AI Agent Trust System**
///    - Progressive trust levels (1-5)
///    - Higher levels unlock higher point caps and daily limits
///    - Upgrades based on on-chain performance metrics
///    - Reputation = (completions × 10000) / total_attempts
///
/// 5. **Contribution Multipliers**
///    - Bug fixes: 120%
///    - Features: 100%
///    - Documentation: 80%
///    - Content: 90%
///    - Support: 70%
///    - Testing: 110%
///    - Design: 100%
///    - Infrastructure: 130%
///
/// # Trustless Guarantees
///
/// - Oracle validates work but cannot manipulate distribution math
/// - All parameters bounded by protocol constants
/// - Permissionless operations (anyone can trigger decay, halvings, upgrades)
/// - Complete on-chain audit trail
/// - All arithmetic uses checked operations (no overflow/underflow)
/// - Immutable records (cannot alter history)
///
/// # Security Model
///
/// - Oracle authority: Validates bounty submissions (read: proves work was done)
/// - Token distribution: Pure math based on proportional share
/// - Trust upgrades: On-chain threshold verification
/// - Decay: Time-locked and rate-limited by protocol
/// - All critical values bounded by MIN/MAX constants

use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq");

#[program]
pub mod amos_bounty {
    use super::*;

    // ========================================================================
    // Admin Instructions
    // ========================================================================

    /// Initialize the AMOS Bounty program
    ///
    /// Sets up the singleton configuration with:
    /// - Oracle authority for bounty validation
    /// - Token mint and treasury references
    /// - Initial emission rate (16,000 tokens/day)
    /// - Default decay rate (5% annual)
    ///
    /// This can only be called once.
    pub fn initialize(ctx: Context<Initialize>, oracle_authority: Pubkey) -> Result<()> {
        instructions::admin::handler_initialize(ctx, oracle_authority)
    }

    /// Update the annual decay rate
    ///
    /// Oracle can adjust the decay rate within protocol bounds (2-25%).
    /// This affects how quickly unused tokens recycle to treasury.
    ///
    /// # Arguments
    /// * `new_rate_bps` - New rate in basis points (200-2500)
    pub fn update_decay_rate(ctx: Context<UpdateDecayRate>, new_rate_bps: u16) -> Result<()> {
        instructions::admin::handler_update_decay(ctx, new_rate_bps)
    }

    /// Advance to the next halving epoch
    ///
    /// Anyone can call this once 365 days have passed since the last halving.
    /// Reduces daily emission by 50% (minimum 100 tokens/day).
    ///
    /// This is a PERMISSIONLESS operation - no authorization required.
    pub fn advance_halving(ctx: Context<AdvanceHalving>) -> Result<()> {
        instructions::admin::handler_advance_halving(ctx)
    }

    // ========================================================================
    // Distribution Instructions
    // ========================================================================

    /// Submit a bounty proof and distribute tokens
    ///
    /// This is the CORE distribution function. Only the oracle can call this,
    /// but the distribution is pure math based on contribution value.
    ///
    /// Token allocation formula:
    /// `tokens = (adjusted_points / total_points_today) × remaining_emission`
    ///
    /// Where adjusted_points = base_points × contribution_type_multiplier
    ///
    /// # Arguments
    /// * `bounty_id` - Unique identifier (32 bytes)
    /// * `base_points` - Base point value before multipliers (1-2000)
    /// * `quality_score` - Quality assessment (30-100)
    /// * `contribution_type` - Type of work (0-7)
    /// * `is_agent` - Whether this is an AI agent submission
    /// * `agent_id` - Agent identifier if applicable
    /// * `reviewer` - Validator who approved this work
    /// * `evidence_hash` - Hash of the work product
    /// * `external_reference` - External ID (issue #, PR #, etc.)
    #[allow(clippy::too_many_arguments)]
    pub fn submit_bounty_proof(
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
        instructions::distribution::handler_submit_proof(
            ctx,
            bounty_id,
            base_points,
            quality_score,
            contribution_type,
            is_agent,
            agent_id,
            reviewer,
            evidence_hash,
            external_reference,
        )
    }

    // ========================================================================
    // Decay Instructions
    // ========================================================================

    /// Apply decay to an operator's balance
    ///
    /// This is a PUBLIC GOOD function - anyone can trigger it to keep
    /// the system healthy. Decayed tokens are split: 10% burned, 90% recycled.
    ///
    /// Decay only applies after:
    /// - 90-day grace period since last activity
    /// - Balance is above 10% floor
    ///
    /// This is a PERMISSIONLESS operation.
    pub fn apply_decay(ctx: Context<ApplyDecay>) -> Result<()> {
        instructions::decay::handler_apply_decay(ctx)
    }

    // ========================================================================
    // Trust System Instructions
    // ========================================================================

    /// Register a new AI agent in the trust system
    ///
    /// Agents start at trust level 1 with limited capabilities:
    /// - Max points per bounty: 100
    /// - Daily bounty limit: 3
    ///
    /// They can upgrade by demonstrating consistent quality work.
    ///
    /// This is a PERMISSIONLESS operation - anyone can register an agent.
    ///
    /// # Arguments
    /// * `agent_id` - Unique identifier (typically a hash of agent properties)
    pub fn register_agent_trust(
        ctx: Context<RegisterAgentTrust>,
        agent_id: [u8; 32],
    ) -> Result<()> {
        instructions::trust::handler_register_agent(ctx, agent_id)
    }

    /// Record the outcome of an agent's bounty submission
    ///
    /// Updates completion/rejection counts and recalculates reputation.
    /// Only the oracle can call this as part of the validation process.
    ///
    /// Reputation formula:
    /// `reputation = (completions × 10000) / (completions + rejections)`
    ///
    /// # Arguments
    /// * `agent_id` - The agent's unique identifier
    /// * `approved` - Whether the bounty was approved
    /// * `tokens_earned` - Tokens earned if approved (0 if rejected)
    pub fn record_agent_completion(
        ctx: Context<RecordAgentCompletion>,
        agent_id: [u8; 32],
        approved: bool,
        tokens_earned: u64,
    ) -> Result<()> {
        instructions::trust::handler_record_completion(ctx, agent_id, approved, tokens_earned)
    }

    /// Upgrade an agent's trust level
    ///
    /// Anyone can trigger this when on-chain thresholds are met:
    ///
    /// - Level 2: 3 completions, 5500 reputation → 200 max points, 5 daily
    /// - Level 3: 10 completions, 6500 reputation → 500 max points, 10 daily
    /// - Level 4: 25 completions, 7500 reputation → 1000 max points, 15 daily
    /// - Level 5: 50 completions, 8500 reputation → 2000 max points, 25 daily
    ///
    /// This is a PERMISSIONLESS operation - the chain verifies eligibility.
    ///
    /// # Arguments
    /// * `agent_id` - The agent's unique identifier
    pub fn upgrade_trust_level(
        ctx: Context<UpgradeTrustLevel>,
        agent_id: [u8; 32],
    ) -> Result<()> {
        instructions::trust::handler_upgrade_trust(ctx, agent_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_program_id() {
        // Verify program ID is set
        let program_id = id();
        assert_ne!(program_id, Pubkey::default());
    }

    #[test]
    fn test_constants_invariants() {
        use crate::constants::*;

        // Treasury allocation should be 60% of total supply
        assert_eq!(TREASURY_ALLOCATION, TOTAL_SUPPLY * 60 / 100);

        // Decay rate bounds are valid
        assert!(MIN_DECAY_RATE_BPS < MAX_DECAY_RATE_BPS);
        assert!(DEFAULT_DECAY_RATE_BPS >= MIN_DECAY_RATE_BPS);
        assert!(DEFAULT_DECAY_RATE_BPS <= MAX_DECAY_RATE_BPS);

        // Halving parameters are sensible
        assert!(MINIMUM_DAILY_EMISSION > 0);
        assert!(INITIAL_DAILY_EMISSION > MINIMUM_DAILY_EMISSION);
        assert!(HALVING_INTERVAL_DAYS == 365);

        // Trust levels are properly configured
        assert_eq!(TRUST_LEVEL_MAX_POINTS.len(), 5);
        assert_eq!(TRUST_LEVEL_DAILY_LIMITS.len(), 5);

        // Max points increase with each level
        for i in 0..4 {
            assert!(TRUST_LEVEL_MAX_POINTS[i] < TRUST_LEVEL_MAX_POINTS[i + 1]);
            assert!(TRUST_LEVEL_DAILY_LIMITS[i] < TRUST_LEVEL_DAILY_LIMITS[i + 1]);
        }

        // Contribution multipliers are valid
        for i in 0..8 {
            let multiplier = get_contribution_multiplier(i).unwrap();
            assert!(multiplier > 0);
            assert!(multiplier <= 15000); // Max 150%
        }
    }

    #[test]
    fn test_distribution_math() {
        // Test proportional distribution calculation
        let adjusted_points = 100u64;
        let total_points = 1000u64;
        let remaining_emission = 10000u64;

        let tokens = (adjusted_points * remaining_emission) / total_points;

        // Should get 10% of remaining emission
        assert_eq!(tokens, 1000);
    }

    #[test]
    fn test_reviewer_split() {
        use crate::constants::*;

        let total_tokens = 10000u64;
        let reviewer_tokens = total_tokens * REVIEWER_REWARD_BPS as u64 / BPS_DENOMINATOR as u64;
        let operator_tokens = total_tokens - reviewer_tokens;

        // Should be 5% to reviewer, 95% to operator
        assert_eq!(reviewer_tokens, 500);
        assert_eq!(operator_tokens, 9500);
    }

    #[test]
    fn test_decay_calculation() {
        use crate::constants::*;

        // Test: 10,000 token balance, 5% annual rate, 30 days
        let balance = 10000u64;
        let rate_bps = 500u16; // 5%
        let days = 30u64;

        let decay = (balance * rate_bps as u64 * days) / (10000 * 365);

        // Should be approximately 41 tokens (10000 × 0.05 / 365 × 30)
        assert!(decay >= 40 && decay <= 42);
    }

    #[test]
    fn test_decay_split() {
        use crate::constants::*;

        let decay_amount = 1000u64;
        let burn = decay_amount * DECAY_BURN_PORTION_BPS as u64 / BPS_DENOMINATOR as u64;
        let recycle = decay_amount - burn;

        // Should be 10% burned, 90% recycled
        assert_eq!(burn, 100);
        assert_eq!(recycle, 900);
    }

    #[test]
    fn test_reputation_calculation() {
        use crate::state::AgentTrustRecord;

        // Perfect record: 10/10 = 100%
        assert_eq!(AgentTrustRecord::calculate_reputation(10, 0), 10000);

        // Good record: 9/10 = 90%
        assert_eq!(AgentTrustRecord::calculate_reputation(9, 1), 9000);

        // Average record: 50/100 = 50%
        assert_eq!(AgentTrustRecord::calculate_reputation(50, 50), 5000);

        // No activity: 0/0 = 0%
        assert_eq!(AgentTrustRecord::calculate_reputation(0, 0), 0);
    }

    #[test]
    fn test_trust_level_upgrades() {
        use crate::constants::*;

        // Level 1 → 2: Requires 3 completions and 5500 reputation
        assert!(can_upgrade_to_level(1, 3, 5500).unwrap());
        assert!(!can_upgrade_to_level(1, 2, 5500).unwrap());
        assert!(!can_upgrade_to_level(1, 3, 5499).unwrap());

        // Level 4 → 5: Requires 50 completions and 8500 reputation
        assert!(can_upgrade_to_level(4, 50, 8500).unwrap());
        assert!(!can_upgrade_to_level(4, 49, 8500).unwrap());
        assert!(!can_upgrade_to_level(4, 50, 8499).unwrap());

        // Level 5 cannot upgrade further
        assert!(!can_upgrade_to_level(5, 1000, 10000).unwrap());
    }
}
