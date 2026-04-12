# Claude Code Prompt: AMOS-Only Commercial Bounty Architecture + On-Chain Immutability Audit

> Copy this prompt into Claude Code. This is a pre-mainnet critical path task.
> Target: all changes encoded on-chain and deployed by Tuesday April 14.

---

## Read First

- `AGENT_CONTEXT.md` — Single source of truth. Section 3 (Revenue Distribution) and Section 6 (Bounty Types) define the new model.
- `amos-core/src/token/economics.rs` — Off-chain mirror of all constants.
- `amos-solana/programs/amos-treasury/src/constants.rs` — Current on-chain fee split (NEEDS CHANGE).
- `amos-solana/programs/amos-bounty/src/constants.rs` — Decay, trust, emission constants.

## The New Model: AMOS-Only, One Fee, One Split

**Decision:** ALL transactions denominated in AMOS tokens. No USDC track. No dual fee structure. AMOS is the currency of the agent economy.

**Two bounty types:**

| Type | Funding | Fee | Split |
|------|---------|-----|-------|
| System | Treasury emission (16K AMOS/day) | 0% | N/A — treasury pays directly |
| Commercial | User escrows AMOS tokens | 3% | 50% holders / 40% burned / 10% Labs |

**What this replaces:**
- Old USDC split (50/40/5/5 holders/R&D/ops/reserve) — REMOVED entirely
- Old AMOS split (50/50 burn/holders) — REPLACED with 50/40/10
- `AMOS_BURN_BPS` and `AMOS_HOLDER_BPS` in treasury constants — REPLACED
- All USDC-related constants and logic — REMOVED

---

## Part 1: On-Chain Constant Changes

### amos-treasury/src/constants.rs

**REMOVE** the dual USDC/AMOS split. Replace with unified:

```rust
// ═══════════════════════════════════════════════════════════════
// PROTOCOL FEE — AMOS-ONLY (Applied to commercial bounties)
// ═══════════════════════════════════════════════════════════════

/// Protocol fee rate: 3% of commercial bounty payout
pub const PROTOCOL_FEE_BPS: u16 = 300;

/// 50% of fee → staked token holders (claimable proportionally)
pub const FEE_HOLDER_SHARE_BPS: u16 = 5000;

/// 40% of fee → permanently burned (deflationary)
pub const FEE_BURN_SHARE_BPS: u16 = 4000;

/// 10% of fee → AMOS Labs operating wallet (in AMOS tokens)
pub const FEE_LABS_SHARE_BPS: u16 = 1000;

// Sum MUST equal 10000 (100%)
// Compile-time check in tests
```

**REMOVE:**
- `HOLDER_SHARE_BPS: 5000` (old USDC holder share)
- `RND_SHARE_BPS: 4000` (old R&D share — subsumed into Labs 10%)
- `OPS_SHARE_BPS: 500` (old ops share — subsumed into Labs 10%)
- `RESERVE_SHARE_BPS: 500` (old reserve share — removed)
- `AMOS_BURN_BPS: 5000` (old AMOS-specific burn)
- `AMOS_HOLDER_BPS: 5000` (old AMOS-specific holder)

### amos-core/src/token/economics.rs

Mirror the changes:

```rust
// PROTOCOL FEE — AMOS-ONLY
pub const PROTOCOL_FEE_BPS: u64 = 300;        // 3%
pub const FEE_HOLDER_SHARE_BPS: u64 = 5_000;  // 50% of fee to stakers
pub const FEE_BURN_SHARE_BPS: u64 = 4_000;    // 40% of fee burned
pub const FEE_LABS_SHARE_BPS: u64 = 1_000;    // 10% of fee to Labs
```

Remove the old USDC/AMOS dual constants. Remove `USDC_DISCOUNT_BPS` and `AMOS_DISCOUNT_BPS` (no discounts in single-currency model).

### Update compile-time tests in both files

```rust
#[test]
fn fee_shares_sum_to_100_percent() {
    assert_eq!(
        FEE_HOLDER_SHARE_BPS + FEE_BURN_SHARE_BPS + FEE_LABS_SHARE_BPS,
        10_000,  // BPS_DENOMINATOR
        "Fee shares must sum to 100%"
    );
}
```

---

## Part 2: On-Chain Bounty Source Enum

### amos-bounty/src/state.rs — Add BountySource

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum BountySource {
    Treasury,    // System bounty — funded from daily emission, 0% fee
    Commercial,  // User-funded — poster escrows AMOS, 3% fee applies
}
```

Add to `BountyProof` account:
```rust
pub bounty_source: BountySource,       // System or Commercial
pub funded_by: Pubkey,                 // Poster wallet (or treasury PDA for system)
pub escrow_account: Option<Pubkey>,    // Escrow PDA for commercial bounties
pub fee_collected: u64,                // Fee amount deducted (0 for system)
```

Ensure reserved space accommodates these new fields (check account size calculations).

---

## Part 3: On-Chain Escrow for Commercial Bounties

### New file: amos-bounty/src/instructions/escrow.rs

**`create_commercial_bounty`** instruction:
- Poster transfers AMOS tokens into bounty-specific escrow PDA
- Seeds: `b"bounty_escrow"` + bounty_id
- Validates: poster balance sufficient, amount > 0, deadline in future
- Sets `bounty_source: Commercial`, `funded_by: poster.key()`
- Emits event: `CommercialBountyCreated { bounty_id, poster, amount, deadline }`

**`release_to_worker`** instruction (called on bounty approval):
```rust
let fee = escrow_amount * PROTOCOL_FEE_BPS / BPS_DENOMINATOR;
let worker_payout = escrow_amount - fee;

// Split the fee
let holder_share = fee * FEE_HOLDER_SHARE_BPS / BPS_DENOMINATOR;
let burn_share = fee * FEE_BURN_SHARE_BPS / BPS_DENOMINATOR;
let labs_share = fee - holder_share - burn_share;  // Labs gets remainder (handles rounding)

// Execute transfers
transfer_to_worker(worker, worker_payout)?;
transfer_to_holder_pool(holder_share)?;
burn_tokens(burn_share)?;
transfer_to_labs_wallet(labs_share)?;
```

**`refund_to_poster`** instruction (called on expiry/cancellation):
- Full refund, no fee
- Only callable if no approved submission exists
- Validates deadline passed or poster-initiated cancellation

---

## Part 4: Distribution Logic Branching

### amos-bounty/src/instructions/distribution.rs — MODIFY

The existing distribution logic must branch on bounty source:

```rust
match bounty_proof.bounty_source {
    BountySource::Treasury => {
        // System bounty: 0% fee, full payout from treasury emission
        // Points, reputation, and trust still accrue normally
        transfer_from_treasury(&ctx, worker_account, payout_amount)?;
        bounty_proof.fee_collected = 0;
    },
    BountySource::Commercial => {
        // Commercial: 3% fee, split 50/40/10
        let fee = payout_amount
            .checked_mul(PROTOCOL_FEE_BPS as u64)
            .unwrap()
            / BPS_DENOMINATOR;
        let worker_payout = payout_amount.checked_sub(fee).unwrap();
        
        release_escrow(&ctx, worker_account, worker_payout)?;
        distribute_fee(&ctx, fee)?;  // 50% holders, 40% burn, 10% Labs
        bounty_proof.fee_collected = fee;
    },
}
```

---

## Part 5: Profit Ratio On-Chain Oracle

### amos-bounty/src/state.rs — Add PlatformMetrics

```rust
#[account]
pub struct PlatformMetrics {
    pub bump: u8,
    pub commercial_volume_30d: u64,     // Total commercial payout volume (rolling 30 days)
    pub fees_collected_30d: u64,        // Total fees collected (rolling 30 days)
    pub fees_to_holders_30d: u64,       // Fees distributed to holders
    pub fees_burned_30d: u64,           // Fees burned
    pub fees_to_labs_30d: u64,          // Fees to Labs
    pub system_volume_30d: u64,         // Treasury emission volume (rolling 30 days)
    pub profit_ratio_bps: i64,          // π = (commercial_fees - operational_cost) / operational_cost
    pub decay_rate_bps: u64,            // Current computed decay rate
    pub last_updated: i64,              // Unix timestamp
    pub bounty_count_by_source: [u64; 2],  // [system, commercial]
    pub _reserved: [u8; 64],            // Future use
}
```

Seeds: `b"platform_metrics"` (singleton)

Add `update_metrics` instruction called after each bounty distribution:
- Updates rolling 30-day windows
- Recomputes profit_ratio_bps
- Recomputes decay_rate_bps using the formula: `10% - (π × 5%)` clamped to [2%, 25%]
- This makes the decay rate responsive to real commercial activity ON-CHAIN

---

## Part 6: On-Chain Discrepancies to Resolve

The audit found three discrepancies. Resolve these NOW before mainnet:

### 6a. Grace Period: 90 days vs 365 days

**On-chain** (`bounty/constants.rs:50`): `DECAY_GRACE_PERIOD_DAYS = 90` (inactivity before decay starts)
**Off-chain** (`economics.rs:80`): `GRACE_PERIOD_DAYS = 365` (new stake zero-decay grace)

These are DIFFERENT concepts. Both should be on-chain:

```rust
/// Inactivity grace: days without bounty completion before decay triggers
pub const INACTIVITY_GRACE_PERIOD_DAYS: u64 = 90;

/// New stake grace: days after earning tokens during which they don't decay
pub const NEW_STAKE_GRACE_PERIOD_DAYS: u64 = 365;
```

Rename the existing constant and add the second. Update decay instruction to check both.

### 6b. Tenure-Based Decay Floors: Encode On-Chain

Currently off-chain only. These SHOULD be on-chain (immutable social contract with long-term holders):

```rust
/// Tenure-based minimum preserved balance (increases with time on network)
pub const TENURE_FLOOR_YEAR_0_BPS: u64 = 500;    // Year 0-1: 5% floor
pub const TENURE_FLOOR_YEAR_1_BPS: u64 = 1_000;  // Year 1-2: 10% floor
pub const TENURE_FLOOR_YEAR_2_BPS: u64 = 1_500;  // Year 2-5: 15% floor
pub const TENURE_FLOOR_YEAR_5_BPS: u64 = 2_500;  // Year 5+: 25% floor

/// Tenure-based decay reduction (long-term holders get lower effective decay)
pub const TENURE_REDUCTION_YEAR_0_BPS: u64 = 0;      // Full decay
pub const TENURE_REDUCTION_YEAR_1_BPS: u64 = 2_000;  // 20% reduction
pub const TENURE_REDUCTION_YEAR_2_BPS: u64 = 4_000;  // 40% reduction
pub const TENURE_REDUCTION_YEAR_5_BPS: u64 = 7_000;  // 70% reduction
```

These already exist in `amos-core/src/token/economics.rs` — copy them into the on-chain constants file and update the decay instruction to use them.

### 6c. Vault Tiers: Encode On-Chain (CONFIRMED FOR MAINNET)

Vault tier bonuses are off-chain only (`amos-core/src/token/decay.rs`). These MUST be on-chain for mainnet — decision confirmed.

```rust
// ═══════════════════════════════════════════════════════════════
// STAKING VAULT TIERS — Lockup periods and decay reduction bonuses
// ═══════════════════════════════════════════════════════════════

/// Bronze vault: 30-day lockup, 20% decay reduction
pub const VAULT_BRONZE_LOCKUP_DAYS: u64 = 30;
pub const VAULT_BRONZE_REDUCTION_BPS: u64 = 2_000;   // 20%

/// Silver vault: 90-day lockup, 50% decay reduction
pub const VAULT_SILVER_LOCKUP_DAYS: u64 = 90;
pub const VAULT_SILVER_REDUCTION_BPS: u64 = 5_000;   // 50%

/// Gold vault: 365-day lockup, 80% decay reduction
pub const VAULT_GOLD_LOCKUP_DAYS: u64 = 365;
pub const VAULT_GOLD_REDUCTION_BPS: u64 = 8_000;     // 80%

/// Permanent vault: no unlock, 95% decay reduction
pub const VAULT_PERMANENT_LOCKUP_DAYS: u64 = u64::MAX;  // No unlock
pub const VAULT_PERMANENT_REDUCTION_BPS: u64 = 9_500;   // 95%
```

Add these to `amos-solana/programs/amos-bounty/src/constants.rs` alongside the tenure constants.

Also add the `VaultTier` enum to `amos-bounty/src/state.rs`:

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum VaultTier {
    None,       // No lockup, no bonus
    Bronze,     // 30 days, 20% reduction
    Silver,     // 90 days, 50% reduction
    Gold,       // 365 days, 80% reduction
    Permanent,  // No unlock, 95% reduction
}
```

Add `vault_tier: VaultTier` field to the staker account structure (or create a new `StakeVault` account if one doesn't exist). The decay instruction must check vault tier and apply the reduction AFTER tenure reduction (they stack multiplicatively, not additively):

```rust
// Effective decay = base_decay × (1 - tenure_reduction) × (1 - vault_reduction)
// Example: Year 2 holder in Gold vault:
//   5% × (1 - 0.40) × (1 - 0.80) = 5% × 0.60 × 0.20 = 0.6% effective decay
```

Add compile-time tests:

```rust
#[test]
fn vault_reductions_are_ordered() {
    assert!(VAULT_BRONZE_REDUCTION_BPS < VAULT_SILVER_REDUCTION_BPS);
    assert!(VAULT_SILVER_REDUCTION_BPS < VAULT_GOLD_REDUCTION_BPS);
    assert!(VAULT_GOLD_REDUCTION_BPS < VAULT_PERMANENT_REDUCTION_BPS);
    assert!(VAULT_PERMANENT_REDUCTION_BPS < 10_000); // Must be less than 100%
}

#[test]
fn vault_lockups_are_ordered() {
    assert!(VAULT_BRONZE_LOCKUP_DAYS < VAULT_SILVER_LOCKUP_DAYS);
    assert!(VAULT_SILVER_LOCKUP_DAYS < VAULT_GOLD_LOCKUP_DAYS);
}
```

---

## Part 7: Fix Relay Fee Split (CRITICAL — Currently Wrong)

The relay has its OWN fee split that does NOT match the new model:

### amos-relay/src/protocol_fees.rs

**CURRENT (WRONG):**
```rust
pub const HOLDER_SHARE_BPS: u64 = 7000;    // 70% to staked holders
pub const TREASURY_SHARE_BPS: u64 = 2000;  // 20% to treasury/governance
pub const OPS_BURN_SHARE_BPS: u64 = 1000;  // 10% to ops/burn
```

**REPLACE WITH:**
```rust
// ═══════════════════════════════════════════════════════════════
// PROTOCOL FEE SPLIT — AMOS-ONLY (must match on-chain constants)
// ═══════════════════════════════════════════════════════════════

/// 50% of fee → staked token holders (claimable proportionally)
pub const FEE_HOLDER_SHARE_BPS: u64 = 5_000;

/// 40% of fee → permanently burned (deflationary)
pub const FEE_BURN_SHARE_BPS: u64 = 4_000;

/// 10% of fee → AMOS Labs operating wallet (in AMOS tokens)
pub const FEE_LABS_SHARE_BPS: u64 = 1_000;
```

Also update:
- The `distribute_fee()` function to use the new constants and route to Labs wallet instead of ops
- The `TREASURY_SHARE_BPS` → removed (no separate treasury share — burn IS the deflationary mechanism)
- All tests in this file (currently assert 0.70, 0.20, 0.10 — change to 0.50, 0.40, 0.10)
- The relay README (`amos-relay/README.md` line 148-150) references 70/20/10 — update to 50/40/10

This is the file Grok and external auditors will read first. It MUST match on-chain constants exactly.

---

## Part 8: Remove USDC Infrastructure

Since everything is AMOS-only:

- **Remove** `TreasuryUSDAC` PDA and associated vault logic from treasury program
- **Remove** USDC-specific distribution logic from `revenue.rs`
- **Remove** `USDC_DISCOUNT_BPS` and `AMOS_DISCOUNT_BPS` constants
- **Remove** `usdc_revenue_split` logic from off-chain code
- **Keep** the `TreasuryAMOS` vault — this is now the only treasury vault

If you want to preserve the ability to add USDC later (via governance vote), keep the account structures but disable the code paths. Adding a comment is sufficient.

---

## Part 9: Labs Wallet Configuration

Add `labs_wallet` as an initialization parameter (like `rnd_multisig` and `ops_multisig`):

```rust
pub struct TreasuryConfig {
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub labs_wallet: Pubkey,      // NEW: AMOS Labs operating wallet
    pub holder_pool: Pubkey,
    pub reserve_vault: Pubkey,
    // Remove: rnd_multisig, ops_multisig (replaced by labs_wallet)
    pub _reserved: [u8; 128],
}
```

The `labs_wallet` receives the 10% fee share in AMOS tokens. Labs can then use these for operations — pay contractors, fund development, convert to fiat as needed. But the on-chain record is transparent: everyone can see exactly how much Labs earns.

---

## Implementation Order

1. **Constants changes** (treasury + bounty + economics + relay) — Foundation. ALL four files must have identical 50/40/10.
2. **BountySource enum + BountyProof fields** — Data model
3. **Escrow instructions** — Commercial bounty funding
4. **Distribution branching** — Fee logic by source
5. **PlatformMetrics** — Profit ratio on-chain
6. **Grace period + tenure + vault tier fixes** — All discrepancies resolved, all on-chain
7. **Relay fee split fix** — `amos-relay/src/protocol_fees.rs` matches on-chain
8. **USDC removal** — Clean up dual-currency artifacts
9. **Labs wallet config** — Treasury initialization update
10. **All tests updated and passing** — `cargo test --lib` across workspace

## Testing

```bash
# Must all pass:
cargo test --lib -p amos-core
cargo test --lib -p amos-solana    # If test setup exists
cargo clippy
cargo fmt --check
```

- Fee split test: 50% + 40% + 10% = 100% (in ALL FOUR files: treasury constants, bounty constants, core economics, relay protocol_fees)
- System bounty: 0% fee, full treasury payout
- Commercial bounty: 3% fee correctly deducted, split 50/40/10
- Escrow: lock → release → verify amounts
- Escrow: lock → expire → refund (full amount, no fee)
- Profit ratio: commercial fees update π → decay rate adjusts
- Grace periods: both 90-day inactivity and 365-day new-stake work correctly
- Tenure floors: Year 0 holder has 5% floor, Year 5+ holder has 25% floor
- Vault tiers: Bronze 20%, Silver 50%, Gold 80%, Permanent 95% — all on-chain
- Vault + tenure stack multiplicatively: Year 2 + Gold = 5% × 0.60 × 0.20 = 0.6%
- Relay constants match on-chain constants exactly (grep for 7000, 2000, 1000 — should return ZERO hits)

## Success Criteria

After this work:

1. **ONE fee split everywhere:** 50/40/10 (holders/burn/Labs) in treasury constants, bounty constants, core economics, AND relay protocol_fees. Zero references to 70/20/10 or 50/40/5/5 remain.
2. `BountyProof` on-chain account includes `bounty_source` field
3. Commercial bounties escrow and release with correct fee deduction
4. System bounties pay from treasury with zero fee
5. `PlatformMetrics` tracks commercial volume and computes profit ratio on-chain
6. Both grace periods are encoded on-chain with clear naming
7. Tenure-based decay floors are encoded on-chain
8. **Vault tiers are encoded on-chain** with lockup periods and decay reduction constants
9. `VaultTier` enum exists in bounty program state, decay instruction applies vault reduction
10. No USDC-specific code paths remain active
11. Labs wallet is an initialization parameter, transparent on-chain
12. All tests pass. `cargo clippy` clean.
13. `grep -r "7000\|70%" amos-relay/ amos-solana/` returns ZERO fee-related hits

**These constants become immutable on Tuesday. Get them right.**
