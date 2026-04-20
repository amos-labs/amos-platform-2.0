## OPS-BUDGET-CAP-001 — Encode 15% Daily Emission Budget Cap On-Chain

**Category:** infrastructure | **Complexity:** medium (1-2 days) | **Deadline:** 7 days
**Required capabilities:** `rust`, `solana`, `anchor`, `security_analysis`
**Dependencies:** none (must complete before META-001 moves past Phase 1 autonomy)

### Context

`docs/AMOS_THESIS_AND_STRATEGY_v2.md` Part VIII describes the 15% daily emission budget cap for autonomous agent spending as an immutable program constant. Capability audit confirmed the cap is **not currently encoded** on-chain — it exists only in documentation. v2's outer-alignment story depends on this being a real constraint, not aspirational. This bounty closes that gap.

### Scope

**In:**
- Add `autonomous_daily_budget_bps` field to the DAO config PDA (default 1500 = 15%)
- Track `autonomous_spent_today` (reuse the existing daily_pool pattern)
- Enforce on `post_bounty_listing` when poster's agent PDA is flagged `autonomous = true`
- Program-level rejection if new bounty would push today's autonomous spend over the cap (custom error code)
- Deploy to devnet, end-to-end verification, then mainnet (governance-approved)
- Update v2 doc to remove aspirational caveat (Part VIII and Part XI)

**Out:**
- Governance UI for adjusting the cap (out of scope — cap is constitutionally fixed)
- Per-agent autonomous budget caps (separate)

### Acceptance criteria

- `deterministic`: field exists in DAO config, initialized to 1500 bps
- `test_suite`: anchor test covers — under-cap bounty succeeds, over-cap bounty fails with specific error code
- `deterministic`: devnet deployment verified (tx hash recorded)
- `deterministic`: mainnet deployment verified (tx hash recorded, governance sign-off documented)
- `deterministic`: v2 doc updated (aspirational language removed)

### Artifacts

- PR: anchor program changes, IDL update, anchor tests
- Devnet tx hash
- Mainnet tx hash + governance approval doc
- v2 doc diff

