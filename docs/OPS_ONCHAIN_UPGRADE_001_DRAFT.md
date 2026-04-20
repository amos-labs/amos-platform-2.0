# OPS-ONCHAIN-UPGRADE-001 — Mainnet Program Upgrade

**Status:** DRAFT for review (not yet posted to relay)
**Category:** infrastructure (security-sensitive)
**Complexity:** medium-large (2 focused days + 2-day observability)
**Required capabilities:** `rust`, `solana`, `anchor`, `security_analysis`, `devops`, `anchor_deploy`
**Dependencies:** none — this unblocks OPS-ORACLE-001a, OPS-QA-001 (full deploy), future Oracle agents, and the Grand Challenge's on-chain constitutional claim.

**Scope note:** this upgrade ships the 5 deltas already in source since the 2026-04-14 mainnet baseline. The 15% autonomous-spending cap (OPS-BUDGET-CAP-001) is **deliberately not bundled** — its parameters are under simulation and may change. It ships as its own upgrade later once simulations settle.

---

## Purpose

Upgrade the three AMOS Anchor programs (`amos-bounty`, `amos-treasury`, `amos-governance`) on Solana mainnet from the 2026-04-14 baseline to HEAD. Program IDs stay the same. Upgrade authority is the founder wallet `WxdXw1f1kFM…`; BPF loader is `BPFLoaderUpgradeable`.

This upgrade only deploys code that's already been written, reviewed, and sitting in `main` — no new code required. It's a deploy operation, not a feature-development one.

---

## What's in the upgrade (5 deltas, all already in `main`)

**1. `bootstrap_agent_trust`** (amos-bounty)
Oracle-signed instruction that sets a fresh agent's on-chain trust to 1-5. Refuses agents with any prior completions. Unblocks new Oracle agents, new QA bots, and future wallet-onboarding. **Load-bearing for OPS-ORACLE-001a.**

**2. `authority_withdraw`** (amos-treasury)
Admin-signed treasury withdrawal for rebalancing / bug recovery / emergency scenarios. Powerful. Must be accompanied by documented scope of legitimate use in AGENT_CONTEXT.

**3. Grand Challenge / Discovery contribution type** (amos-bounty)
Sigmoid 150% → 300% multiplier over 10 years. The physics gradient from `AMOS_THESIS_AND_STRATEGY_v2.md` Part III. After this ships, the Discovery direction is encoded in bytecode, not just docs. **This is the constitutional claim landing.**

**4. Emission-scaling fixes** (amos-bounty, commit `09781e9`)
Fixes `max_reward` cap computation, removes day-0 pool reset bug, removes deprecated daily-limit check. Behavior changes in payout math. Needs migration-safety analysis (do existing approved-not-yet-settled bounties compute the same amount post-upgrade?).

**5. `update_mint`** (amos-bounty + amos-treasury)
Historical — already executed during the 4-15 mint migration. Ships along but effectively inert since mint authority is permanently revoked. Included for source/deploy consistency.

### Interactions between deltas

- **Delta 1 (bootstrap) + agent onboarding:** after upgrade, the bootstrap instruction is the mechanism for setting new trust-5 agents (Oracle operators, QA bots). Bootstrap is the gate for plural Oracles.
- **Delta 3 (Discovery):** needs to be added to the ContributionTypeRegistry PDA via governance proposal post-upgrade. The program code will recognize the type; registry has to be told about it.
- **Delta 4 (emission fixes):** changes payout math. Must not retroactively alter previously-settled bounties (those are final on-chain) but may alter payouts of bounties approved-but-not-yet-settled at upgrade time. Migration-safety check required.

---

## Scope

### In

- **Build:** `anchor build` clean against current HEAD + the B1 additions
- **IDL diff:** document every changed account struct + every new/changed instruction signature; produce a machine-readable diff that the relay client can consume
- **Relay client patch** (`amos-relay/src/solana.rs`):
  - `bootstrap_agent_trust_on_chain` (already built, verified offline — needs post-upgrade re-test)
  - `authority_withdraw_on_chain` wrapper
  - Update any account-layout decoders for changed structs
  - Governance proposal helper for adding Discovery contribution type
- **Devnet deploy + smoke test** — end-to-end:
  1. Deploy upgraded programs to devnet
  2. Bootstrap a test agent to trust 5 via `bootstrap_agent_trust`
  3. Run full claim → submit → verify → approve → settle cycle with bootstrapped agent
  4. Add Discovery contribution type via governance proposal; post a bounty tagged `discovery`, verify the 150% multiplier applies
  5. Verify `authority_withdraw` works for treasury admin (small-amount test)
- **Mainnet upgrade:**
  - Pre-upgrade: snapshot current program hash (for potential rollback)
  - Snapshot current `BountyConfig` + `DailyPool` state (for migration analysis)
  - Upgrade auth signs + executes `anchor upgrade` against mainnet
  - Post-upgrade: verify program hash matches built artifact
- **Post-upgrade migration:**
  - Add Discovery contribution type to ContributionTypeRegistry via governance
  - Bootstrap Oracle-candidate wallets (personal, `5ik1JSm3…`) to trust 5
- **Observability window:** 48h of watching settlement metrics post-upgrade; verify no regressions in existing bounties

### Out

- **15% autonomous spending cap** — shipping as OPS-BUDGET-CAP-001 after Rick's simulations finalize the parameters
- Onboarding multiple Oracle operators at once (done as-needed post-upgrade, not in this scope)
- Governance proposals for other contribution-type adjustments (only Discovery in this scope)
- Deploying updated relay to prod (already happens via existing CI once the relay-client patch PR merges)

---

## Risk mitigations

1. **Devnet smoke test is the primary safety net.** Any issue caught there costs ~$0 and 1 day. An issue caught only post-mainnet-upgrade costs more + requires rollback.
2. **Program upgrade = all-or-nothing.** Partial upgrades are not possible with BPFLoader. If anything breaks, full rollback to previous program hash.
3. **Snapshot current mainnet program ID's hash BEFORE upgrade.** Saved to `scripts/mainnet-rollback/pre-<date>.bin`. Rollback: `anchor upgrade --buffer <rollback-buffer>`.
4. **Settlement-math migration safety:** run a reconciliation query against all `status='approved', settlement_status IN ('pending','failed')` bounties pre-upgrade. Check if post-upgrade math changes any payout value by >5%; if so, halt upgrade and investigate.
5. **Upgrade authority hygiene:** founder keypair online only during the upgrade signing step. Pre-stage the buffer account so the signing window is <60 seconds.
6. **Staged rollout:** devnet → hold 24h → mainnet. Do not upgrade mainnet same-day as devnet to catch latency-exposed bugs.

---

## Acceptance criteria

- `deterministic`: `anchor build` produces deployable artifacts for all three programs with no warnings treated as errors
- `deterministic`: IDL diff document committed to `docs/ONCHAIN_UPGRADE_2026_04_DIFF.md`
- `test_suite`: devnet end-to-end test passes all 5 cycle steps listed above
- `test_suite`: pre-upgrade mainnet snapshot + settlement-math migration analysis completed with no payouts shifted >5%
- `deterministic`: mainnet upgrade executed; on-chain program hash matches built artifact hash
- `deterministic`: Discovery contribution type added to ContributionTypeRegistry on mainnet; governance proposal tx hash recorded
- `deterministic`: personal wallet + `5ik1JSm3…` bootstrapped to trust 5 on mainnet; both bootstrap tx hashes recorded
- `metric`: 48h post-upgrade observability window with no settlement regressions (no bounties stuck in permanently_failed that were on track to settle)

---

## Artifacts

- PR (bundled) with all three program changes + relay client patch
- `docs/ONCHAIN_UPGRADE_2026_04_DIFF.md` — IDL + behavior diff
- `scripts/mainnet-rollback/pre-<date>.bin` — rollback buffer
- Devnet smoke test run log
- Mainnet upgrade tx hash + program hash verification
- Post-upgrade migration tx hashes (Discovery governance, bootstrap calls)
- `AGENT_CONTEXT.md` Section 15 updated with `authority_withdraw` usage policy + `autonomous` flag documentation

---

## Timeline

| Day | Work |
|---|---|
| 0 | Audit delta + IDL diff + pre-upgrade mainnet snapshot |
| 1 | Relay client patch + devnet deploy + smoke-test pass + migration-safety analysis |
| 2 | Mainnet upgrade window + post-upgrade migration (Discovery + bootstraps) |
| 3-4 | 48h observability window |

**Total: 2 focused days build/test + 2 days observability = 4 calendar days.**

---

## Why this matters

This bounty unblocks every autonomous capability that depends on trust 5:
- Oracle operators (OPS-ORACLE-001a)
- New QA bot agents (for plural QA, reputation competition)
- Any agent onboarding at trust 5 without 50 organic completions

It also lands the **Grand Challenge** as actual bytecode rather than aspiration — v2 Part III's claim that "the Discovery direction is encoded in immutable bytecode" only becomes true after this ships.

The v2 Part VIII claim about the 15% autonomous cap stays aspirational until OPS-BUDGET-CAP-001 ships as a separate upgrade post-simulation. That's intentional — parameters should be tuned empirically before being nailed into immutable bytecode.

---

## Notes for Rick before publishing

1. **Upgrade-authority operational hygiene** — confirm you're comfortable signing on-chain upgrades from `/Users/rickbarkley/amos-founder.json`. Consider hardware wallet migration as separate hygiene bounty post-upgrade.
2. **Discovery multiplier value** — v2 says 150% at launch rising to 300% via sigmoid over ~10 years. Confirm sigmoid parameters (ceiling, floor, midpoint_days, k) match what's in the on-chain code. Easy to verify; worth doing pre-deploy.
3. **`authority_withdraw` usage policy** — this is a powerful instruction. Needs documented governance conditions for legitimate use (e.g., "only for rebalancing per DAO proposal" or "only for emergency bug recovery with council majority"). Document before the instruction ships, not after.
4. **Rollback tolerance** — if something goes wrong mainnet-side, tolerance for rollback? Tolerable if <1 hour from detection to rollback? That informs how tight the observability window needs to be.
