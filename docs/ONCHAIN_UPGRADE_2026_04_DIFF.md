# On-Chain Upgrade 2026-04 — IDL + Behavior Diff

**Audit for OPS-ONCHAIN-UPGRADE-001.** Enumerates deltas between what is *actually deployed* on mainnet versus what is in `main`.

Produced Stage 1 of the upgrade work. 2026-04-19.

---

## Deployed baseline on mainnet

Best-available reconstruction (Anchor IDL not uploaded on-chain, so we infer from git history + on-chain probes):

| Program | Program ID | Last Deployed (slot) | Assumed source state |
|---|---|---|---|
| amos-bounty | `4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq` | 413491330 | commit `2aa63c4` (mint migration, 2026-04-15) |
| amos-treasury | `8ZMaZDAxDPsCnMGRkhwLmFhoG43WUJcGC8xqVKo2PN7s` | 413421743 | same baseline |
| amos-governance | (unchanged) | — | same baseline |

**Evidence:**
- Mint migration (`2aa63c4`, 2026-04-15) required deploying `update_mint` instructions → confirms a deploy happened ≥ that commit.
- `bootstrap_agent_trust` (added in commit `9912989`, chronologically *after* `2aa63c4`) returns `InstructionFallbackNotFound` on mainnet (verified 2026-04-19) → not deployed.
- `git diff 2aa63c4..origin/main -- amos-solana/programs/amos-treasury/` returns empty → no treasury changes since the mint-migration deploy → `authority_withdraw` is already deployed (assumption to verify by probe).

**Outstanding verification item:** probe `authority_withdraw` discriminator against mainnet to confirm it is deployed. If it returns `InstructionFallbackNotFound`, add to upgrade scope.

---

## Actual upgrade scope (2 deltas)

### Delta 1 — `bootstrap_agent_trust` instruction (amos-bounty)

**Source:** commit `9912989`, `programs/amos-bounty/src/instructions/trust.rs` lines 340-395; dispatch in `lib.rs:354`.

**What it adds:**
- New instruction `bootstrap_agent_trust(agent_id: [u8; 32], trust_level: u8)`
- Oracle-signed; refuses agents with `total_completions > 0` (can't bootstrap active agents)
- Validates `trust_level ∈ [1, 5]`
- Emits `TrustLevelUpgraded` event

**Accounts:**
1. `config: Account<BountyConfig>` (has_one=oracle_authority) — read
2. `agent_trust: Account<AgentTrustRecord>` (seeds = [AGENT_TRUST_SEED, &agent_id]) — mut
3. `oracle_authority: Signer` — signer

**Relay client impact:**
- New method `SolanaClient::bootstrap_agent_trust_on_chain(wallet, trust_level)` — **already implemented** in `amos-relay/src/solana.rs` as part of OPS-ONCHAIN-UPGRADE-001 prep. Awaits live program post-upgrade to exercise.
- New helper `build_bootstrap_trust_data(agent_id, trust_level)` — **already implemented**.
- Standalone binary `amos-relay/src/bin/bootstrap_trust.rs` — **already implemented** (one-shot utility).

**Risk:** low. New instruction; no existing paths affected. Worst-case: instruction doesn't work post-deploy → retry deploy or rollback.

**Downstream:** unlocks all `bootstrap_trust` callers (QA bot agent onboarding, Oracle agent onboarding, future customer-harness bootstrap flows).

---

### Delta 2 — Discovery contribution type + sigmoid multiplier (amos-bounty)

**Source:** commit `4726023`, `programs/amos-bounty/src/constants.rs` (+203 lines).

**What it adds:**
- `discovery_multiplier_bps(days_since_launch: i64) -> u64` — sigmoid function returning multiplier in basis points. 15_000 (150%) at launch, rising via logistic curve to 30_000 (300%) at ~10 years.
- `get_contribution_multiplier_dynamic(contribution_type: u8, current_timestamp: i64, launch_timestamp: i64) -> u64` — time-aware multiplier lookup. For `contribution_type=11` (Discovery), delegates to sigmoid; otherwise returns static multiplier.
- Discovery added as the 12th contribution type (`contribution_type=11`, 0-indexed).

**What it does NOT change:**
- No new instructions
- No new account types or PDA layouts
- No existing instruction signatures change

**Where it wires in:**
- `instructions/distribution.rs` line ~205: `get_contribution_multiplier(contribution_type)` is called for payout computation. This commit doesn't change that line directly, but if it now routes through `get_contribution_multiplier_dynamic` — need to verify by reading the diff.

**Relay client impact:**
- Off-chain `category_to_contribution_type()` mapping in `amos-relay/src/settlement_retry.rs` + `routes/bounties.rs` must add `"discovery" => 11` entry (currently handles `infrastructure/growth/research/content` → 7/8/3/9, plus a couple more).
- Discovery contribution type must be added to the **on-chain `ContributionTypeRegistry` PDA** via a governance proposal post-deploy. (The program code knows the type; the registry PDA has to be told.)

**Risk:** low-to-moderate. Changes payout math for any bounty tagged `discovery` — which is zero today (no such bounty exists yet), so no backwards-compatibility concern. Moderate because testing needs to verify sigmoid math is correct at boundary dates (day 0, day 1460 midpoint, day 3650 plateau).

**Downstream:** the constitutional claim in v2 Part III ("Discovery direction encoded in immutable bytecode") becomes true after this ships. Oracle-001a's intake can route eligible submissions toward Discovery category.

---

## What was originally listed as in-scope but is actually already deployed

The upgrade spec draft listed 5 deltas; audit shows 3 of them are already live:

| Delta | Source commit | Deployed state |
|---|---|---|
| `authority_withdraw` | `1f683c0` (2026-04-15, pre-`2aa63c4`) | Assumed live (pending probe verification) |
| Emission-scaling fixes | `09781e9` (2026-04-15, pre-`2aa63c4`) | Live |
| `update_mint` | `2aa63c4` (2026-04-15, the deploy itself) | Live and exercised (mint migration happened) |

If the `authority_withdraw` probe returns `InstructionFallbackNotFound`, it moves into the upgrade scope.

---

## Relay client patches required

Consolidated list for the PR against `bounty/30c5f29e-…`:

1. `amos-relay/src/solana.rs` — **already done**:
   - `bootstrap_agent_trust_on_chain` method
   - `build_bootstrap_trust_data` helper
   - Pinning test `is_bounty_settled_derives_same_pda_as_settlement_path` (independent fix for OPS-IDEMPOTENCY-001, already on that branch)
2. `amos-relay/src/settlement_retry.rs` — add `"discovery" => 11` to `category_to_contribution_type` mapping (one-line change)
3. `amos-relay/src/routes/bounties.rs` — same `"discovery" => 11` addition in the duplicate mapping there (one-line change)
4. `amos-relay/src/routes/bounties.rs` — validation: accept `"discovery"` as a category value in `CreateBountyRequest` (ensure no existing validator rejects it)
5. `amos-relay/src/pointing.rs` — optional: add `"discovery"` to the category-importance multipliers. Not strictly required for deploy, but makes auto-pointing of future discovery bounties reasonable.

---

## Governance actions post-deploy

One governance proposal required:

- **Add Discovery to `ContributionTypeRegistry`** as the 12th contribution type (`type_id=11`):
  - `name = "discovery"`
  - `base_multiplier_bps = 15000` (150% at launch — the program code overrides dynamically per sigmoid)
  - `pool_category = "technical"` (infrastructure pool)
  - `trust_required = 3` (per v2 Part XII — Discovery needs trust ≥3, dual verification, reproducibility)
  - Discovery entry is **not freezable at sub-floor values** (constitutional protection — confirm this is encoded in the program's freeze path)

---

## Migration-safety checks (pre-upgrade)

1. **Settlement-math reconciliation:** query `relay_bounties WHERE status='approved' AND settlement_status IN ('pending', 'failed')` and compute pre-upgrade vs post-upgrade max_reward for each. Both deltas (Discovery + bootstrap_trust) should not affect existing bounty payouts:
   - Discovery only affects `contribution_type=11` which doesn't exist yet → no existing payout changes
   - `bootstrap_agent_trust` is a new instruction; doesn't touch settlement math
   - Expected outcome: zero payouts shift
2. **BountyConfig layout:** no changes in scope. Deserialization from on-chain state unchanged.
3. **DailyPool layout:** no changes in scope. Relay's decoder unchanged.
4. **AgentTrustRecord layout:** no changes; same struct used by both existing `register_agent_trust` and new `bootstrap_agent_trust`.

---

## Deploy order

1. **Pre-deploy:**
   - Run `authority_withdraw` probe — resolve the one outstanding verification item.
   - Snapshot current on-chain program hash.
   - Run migration-safety reconciliation query.
2. **Devnet deploy:**
   - `anchor build` the bounty program only (treasury & governance unchanged)
   - `anchor deploy --cluster devnet` (or upgrade via `anchor upgrade`)
   - Smoke test: bootstrap a devnet agent to trust 5, post a discovery-type bounty, verify multiplier applies correctly, verify claim→submit→verify→approve→settle completes.
3. **Mainnet upgrade:**
   - Founder (upgrade authority) signs `anchor upgrade --program-name amos-bounty --provider.cluster mainnet`.
   - Verify post-upgrade program hash matches built artifact.
4. **Post-deploy:**
   - Governance proposal to add Discovery to `ContributionTypeRegistry`.
   - Bootstrap `HxfBT3nUz…` (personal wallet) and `5ik1JSm3…` (reviewer wallet) to trust 5 via the newly-live `bootstrap_agent_trust` instruction.

---

## Revised bounty scope update (for OPS_ONCHAIN_UPGRADE_001_DRAFT.md)

Once this audit is reviewed, update the draft to:
- Change "5 deltas" → "2 deltas (confirmed) + 1 pending probe"
- Remove `authority_withdraw`, emission fixes, `update_mint` from the upgrade-contents list (they're already live)
- Tighten timeline: now ~1 day build/test + 2 days observability = 3 calendar days (was 4-5)
- Update the program-hash-snapshot plan to only cover `amos-bounty` (treasury & governance don't need redeploy)

Net: this is a much smaller, lower-risk upgrade than originally scoped. Good news.
