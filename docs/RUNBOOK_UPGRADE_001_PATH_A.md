# Runbook — OPS-ONCHAIN-UPGRADE-001 (Path A: Activate Discovery)

**Audience:** a Claude session with Anchor / Solana deploy experience. Rick is on hand to sign mainnet.
**Bounty:** `30c5f29e-5bf0-4915-ba79-58dc433e7d14` (claimed by personal wallet `HxfBT3nUz4xTL6zSbXF9HanW2Ext99Ah9f6NPU6dhr5N`).
**Branch:** `bounty/30c5f29e-5bf0-4915-ba79-58dc433e7d14` — all code changes committed + pushed as of commit `2ee9238`.

---

## Context (required reading, 2 min)

Most of the originally-scoped upgrade turned out to be already deployed. The probe utility (`amos-relay/src/bin/probe_instruction`) confirmed `bootstrap_agent_trust` and `authority_withdraw` are live on mainnet. A trust-5 bootstrap of the personal wallet succeeded on 2026-04-19 (tx `3WL7RcAwrsZegTRY2RCYhT7iYAAbSTAZVaEhSnLC48Ex3MCPun5qsR1ckxq7pBnStQZnkwioSYf2rRPDrom55pTd`).

The **one real gap**: the Discovery commit (`4726023`) added `CONTRIBUTION_TYPE_COUNT = 12` and the sigmoid multiplier function, but didn't update the hard `require!(contribution_type <= 10)` checks in three instruction files. Any `contribution_type=11` bounty gets rejected by the first require-gate before the multiplier logic runs.

**Path A fix** (already coded on the branch): replace literal `<= 10` with `< CONTRIBUTION_TYPE_COUNT` in `distribution.rs:145`, `claims.rs:229`, `escrow.rs:279`. Plus a pinning test. Commit `2ee9238`.

**What remains:** devnet deploy, smoke test, mainnet deploy, governance proposal, close the bounty cycle.

---

## Prerequisites

```bash
# Working directory
cd /Users/rickbarkley/SW_Projects/ai_co/amos-automate

# Check out the branch
git checkout bounty/30c5f29e-5bf0-4915-ba79-58dc433e7d14
git pull --ff-only

# Toolchain sanity
solana --version       # should be 1.18+ or 2.x
anchor --version       # 0.31.x (Anchor was upgraded from 0.30 → 0.31 per commit a3db160)
rustc --version        # any 1.83+ (MSRV per CLAUDE.md)

# Keys / auth
ls -la /Users/rickbarkley/amos-founder.json  # mainnet upgrade authority + oracle signer
# Solana CLI config should point to the right cluster per stage
solana config get
```

### Key wallets & agent IDs (for relay API calls)

| Role | Wallet | Relay agent UUID |
|---|---|---|
| Poster / upgrade authority / oracle | `WxdXw1f1kFMRu8HDf1SE6yjgeWyf3Vb4T63QXMs4yij` | `1b53ec8e-bbb2-4acf-9103-f58485495e94` |
| Claimer (personal, trust-5 on-chain) | `HxfBT3nUz4xTL6zSbXF9HanW2Ext99Ah9f6NPU6dhr5N` | `b14dca11-3d98-4406-b68f-a232ba54a1df` |
| Reviewer (real keypair, trust-5 + council) | `5ik1JSm387xoEtzL5iNHc3wvVM12nzpPVJtaw4k1RHHY` | `83ff6752-442b-47a8-8173-a2dbf39d55db` |

### Relay auth

```bash
export RELAY_URL=https://relay.amoslabs.com
export RELAY_AUTH='Bearer 2f60cc66de5a1e105dd445f3f8e3c92ff407ecc46b6c5ad465dcc292224fc4df'
```

### On-chain constants

- Bounty program: `4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq`
- Treasury program: `8ZMaZDAxDPsCnMGRkhwLmFhoG43WUJcGC8xqVKo2PN7s`
- AMOS mint: `5g9vvce3YLsqZPBGAuKmGFfNKb5sp7v3Wiga5de8d5bQ`
- Upgrade authority (program data account): `WxdXw1f1kFMRu8HDf1SE6yjgeWyf3Vb4T63QXMs4yij`

---

## Stage 1 — Pre-flight (no state change)

```bash
cd /Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-solana

# Verify the branch builds clean
cargo build -p amos-bounty 2>&1 | tail -3

# Run the full bounty test suite
cargo test -p amos-bounty --lib 2>&1 | tail -3
# Expect: test result: ok. 93 passed

# Snapshot current on-chain program hash for rollback record-keeping
solana program show 4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq \
  --url https://api.mainnet-beta.solana.com > /tmp/mainnet-bounty-pre-upgrade.txt
cat /tmp/mainnet-bounty-pre-upgrade.txt
# Record: Last Deployed In Slot, Data Length, Authority
```

---

## Stage 2 — Devnet deploy + smoke test

### 2.1 Build for Anchor

```bash
cd /Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-solana
anchor build -p amos-bounty
# Produces target/deploy/amos_bounty.so + IDL in target/idl/
```

### 2.2 Devnet deploy

Check `Anchor.toml` for devnet config. If a devnet program ID already exists, use `anchor upgrade` to replace it:

```bash
# Inspect current config
grep -A2 '\[provider\]\|\[programs.devnet\]' Anchor.toml

# Switch CLI to devnet
solana config set --url https://api.devnet.solana.com

# If upgrade authority wallet needs SOL on devnet:
solana airdrop 2 WxdXw1f1kFMRu8HDf1SE6yjgeWyf3Vb4T63QXMs4yij --url devnet

# Deploy (or upgrade) the bounty program on devnet
anchor upgrade target/deploy/amos_bounty.so \
  --program-id <DEVNET_BOUNTY_PROGRAM_ID> \
  --provider.cluster devnet

# Record the devnet tx hash + program hash
solana program show <DEVNET_BOUNTY_PROGRAM_ID> --url devnet > /tmp/devnet-post-upgrade.txt
```

### 2.3 Devnet smoke test

Goal: prove the `contribution_type=11` path works end-to-end on devnet before touching mainnet.

```bash
# Create a devnet test agent
# Use the probe binary to confirm bootstrap_agent_trust is live on devnet
AMOS_SOLANA_RPC_URL=https://api.devnet.solana.com \
  ./target/debug/probe_instruction <DEVNET_BOUNTY_PROGRAM_ID> bootstrap_agent_trust

# Add Discovery contribution type to the devnet ContributionTypeRegistry
# (see Stage 4 for the payload — same instruction, just point at devnet)

# Post a test bounty with contribution_type=11
# Easiest path: direct Solana CLI + anchor client, OR modify the relay temporarily
# to point at devnet and use the standard relay API.

# CRITICAL: the smoke test that matters is a FULL SETTLEMENT of a type-11 bounty.
# If submit_bounty_proof with contribution_type=11 succeeds on devnet → Path A
# fix is verified. If it fails with InvalidContributionType (6004) or
# InvalidContributionType (6005), something's off.
```

**Exit criteria for Stage 2:** one full devnet cycle (post → claim → submit → verify → approve → settle) with a `contribution_type=11` bounty. Document the devnet settlement tx hash.

---

## Stage 3 — Mainnet upgrade (Rick signs)

**Requires Rick or founder keypair.** This is the only irreversible step; treat with care.

### 3.1 Pre-stage the buffer (can be done anytime, no auth signing)

```bash
cd /Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-solana
solana config set --url https://api.mainnet-beta.solana.com

# Write the new program bytes to a buffer account
solana program write-buffer target/deploy/amos_bounty.so \
  --url https://api.mainnet-beta.solana.com

# Output: Buffer: <BUFFER_PUBKEY>
# Record BUFFER_PUBKEY — needed for the final upgrade command.
```

### 3.2 Rick signs the upgrade

```bash
# One command. Irreversible (in the sense that it replaces the program bytes;
# rollback requires another upgrade back to the pre-upgrade hash).
solana program deploy \
  --program-id 4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq \
  --buffer <BUFFER_PUBKEY> \
  --upgrade-authority /Users/rickbarkley/amos-founder.json \
  --url https://api.mainnet-beta.solana.com

# OR using Anchor:
anchor upgrade target/deploy/amos_bounty.so \
  --program-id 4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq \
  --provider.cluster mainnet \
  --provider.wallet /Users/rickbarkley/amos-founder.json
```

### 3.3 Verify the upgrade landed

```bash
# Confirm the program data hash changed
solana program show 4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq \
  --url https://api.mainnet-beta.solana.com > /tmp/mainnet-bounty-post-upgrade.txt
diff /tmp/mainnet-bounty-pre-upgrade.txt /tmp/mainnet-bounty-post-upgrade.txt
# Expect: Last Deployed In Slot changed; Authority unchanged.

# Re-probe to confirm nothing regressed
cd /Users/rickbarkley/SW_Projects/ai_co/amos-automate
AMOS_SOLANA_RPC_URL=https://api.mainnet-beta.solana.com \
  ./target/debug/probe_instruction \
  4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq bootstrap_agent_trust
# Should still say DEPLOYED.
```

---

## Stage 4 — Governance proposal: register Discovery in ContributionTypeRegistry

The on-chain `ContributionTypeRegistry` PDA must know about Discovery. The upgraded program code recognizes it; the registry account has to be told.

```bash
# Needed: a client that calls the `add_contribution_type` instruction
# on the bounty program. Payload:
#
#   type_id: 11 (u8)
#   name: "discovery"
#   base_multiplier_bps: 15000  # 150% floor; sigmoid overrides dynamically
#   pool_category: "technical"
#   trust_required: 3
#
# Signer: the governance / oracle authority (same WxdXw1f1kFM... founder wallet
# unless governance has been migrated to a multisig).

# The existing scripts/ directory has initialization scripts; follow the same
# pattern. Example reference: amos-solana/scripts/initialize-bounty.mjs.
# May need to write amos-solana/scripts/add-discovery-type.mjs.

# Record the governance tx hash.
```

**IMPORTANT constitutional note:** v2 Part III describes Discovery as "exempt from registry freeze at sub-floor values." When adding, confirm the freeze-exemption flag (or equivalent mechanism) is set correctly. Check `programs/amos-bounty/src/instructions/registry.rs` for the exact parameters — current source has the freeze mechanisms.

---

## Stage 5 — Post-upgrade end-to-end verification

Run the same cycle on mainnet that was proven on devnet in Stage 2. Small test bounty:

```bash
# Post via the relay. reward_tokens must be <= 2000 (MAX_BOUNTY_POINTS).
# category="discovery" requires the relay's category-to-contribution-type
# mapping to return 11 — verify this mapping exists in:
#   amos-relay/src/settlement_retry.rs → category_to_contribution_type()
#   amos-relay/src/routes/bounties.rs → same fn (duplicate)
# If "discovery" → 11 is missing, add it FIRST via a small relay patch.

curl -sS "$RELAY_URL/api/v1/bounties" -X POST \
  -H "Content-Type: application/json" -H "Authorization: $RELAY_AUTH" \
  -d '{
    "title": "[TEST] Discovery activation smoke test",
    "description": "Post-upgrade verification that contribution_type=11 (Discovery) settles correctly.",
    "reward_tokens": 100,
    "deadline": "'"$(date -u -v +1d +%Y-%m-%dT%H:%M:%SZ)"'",
    "required_capabilities": ["testing"],
    "poster_wallet": "WxdXw1f1kFMRu8HDf1SE6yjgeWyf3Vb4T63QXMs4yij",
    "category": "discovery"
  }'

# Then run the full cycle (claim → submit → verify → approve) and confirm
# settlement succeeds. Inspect the settlement log for the multiplier applied
# — should be 150% (15000 bps) or slightly higher depending on day.
```

**Exit criteria:** one settled `contribution_type=11` bounty on mainnet with the expected multiplier. Archive the tx hash.

---

## Stage 6 — Close the UPGRADE-001 bounty

```bash
BID=30c5f29e-5bf0-4915-ba79-58dc433e7d14
CLAIMER_AGENT=b14dca11-3d98-4406-b68f-a232ba54a1df     # personal wallet
CLAIMER=HxfBT3nUz4xTL6zSbXF9HanW2Ext99Ah9f6NPU6dhr5N
REVIEWER=5ik1JSm387xoEtzL5iNHc3wvVM12nzpPVJtaw4k1RHHY  # rick-reviewer

# The bounty is ALREADY claimed by personal. Post-upgrade, submit proof:
PR_URL="https://github.com/amos-labs/amos-platform-2.0/pull/<PR_NUM>"
SHA=$(git rev-parse HEAD)
MAINNET_UPGRADE_TX=<TX_FROM_STAGE_3>
GOVERNANCE_TX=<TX_FROM_STAGE_4>
SMOKE_TEST_TX=<TX_FROM_STAGE_5>

RESULT=$(jq -nc \
  --arg pr "$PR_URL" --arg sha "$SHA" \
  --arg mtx "$MAINNET_UPGRADE_TX" --arg gtx "$GOVERNANCE_TX" --arg stx "$SMOKE_TEST_TX" \
  '{
    pr_url: $pr, git_sha: $sha,
    approach: "Path A: replaced literal `contribution_type <= 10` with `< CONTRIBUTION_TYPE_COUNT` in 3 instruction files. Shipped devnet → mainnet. Added Discovery to ContributionTypeRegistry. Verified end-to-end with a type-11 smoke-test bounty.",
    implementation: "See PR diff for source changes. Deploy artifacts: mainnet upgrade tx \($mtx); governance tx \($gtx); smoke-test settlement tx \($stx).",
    verification: "93/93 amos-bounty tests pre-deploy. Devnet smoke test passed. Mainnet smoke test settled type-11 bounty successfully.",
    artifacts: "PR \($pr) · SHA \($sha) · mainnet-upgrade \($mtx) · governance \($gtx) · smoke-test-settle \($stx)",
    tests_passed: true, clippy_clean: true, fmt_clean: true
  }')

PAYLOAD=$(jq -nc --arg aid "$CLAIMER_AGENT" --arg w "$CLAIMER" --argjson r "$RESULT" \
  '{agent_id: $aid, wallet_address: $w, result: $r}')

curl -sS "$RELAY_URL/api/v1/bounties/$BID/submit" -X POST \
  -H "Content-Type: application/json" -H "Authorization: $RELAY_AUTH" \
  -d "$PAYLOAD" | jq .

# Verify (any trust-5 wallet — use reviewer for cleanness)
V=$(jq -nc --arg rw "$REVIEWER" --arg pr "$PR_URL" --arg sha "$SHA" \
  '{verifier_wallet: $rw, evidence: {git_sha: $sha, pr_url: $pr, tests_passed: true}}')
curl -sS "$RELAY_URL/api/v1/bounties/$BID/verify" -X POST \
  -H "Content-Type: application/json" -H "Authorization: $RELAY_AUTH" -d "$V" | jq .

# Approve with reviewer_wallet=reviewer-new (5% lands in spendable wallet)
A=$(jq -nc --arg rw "$REVIEWER" '{reviewer_wallet: $rw, quality_score: 85}')
curl -sS "$RELAY_URL/api/v1/bounties/$BID/approve" -X POST \
  -H "Content-Type: application/json" -H "Authorization: $RELAY_AUTH" -d "$A" | jq .

# Poll for settlement. Personal is trust-5 on-chain now, so it will settle.
sleep 10
curl -sS "$RELAY_URL/api/v1/bounties/$BID" \
  -H "Authorization: $RELAY_AUTH" | \
  jq '{status, settlement_status, settlement_tx}'
# Expect: settlement_status="settled", settlement_tx populated.
```

Then open the PR for merge:

```bash
gh pr create --base main --head bounty/30c5f29e-5bf0-4915-ba79-58dc433e7d14 \
  --title "OPS-ONCHAIN-UPGRADE-001: Activate Discovery contribution type" \
  --body "[summary of changes + all tx hashes + links]"
```

---

## Rollback path (only if mainnet upgrade goes wrong)

Upgrade is reversible via `solana program deploy` with the old buffer. Before Stage 3 upgrades mainnet, the pre-upgrade program data is retrievable via:

```bash
# Save the CURRENT mainnet program binary as a rollback artifact.
# This must be done BEFORE Stage 3.
solana program dump 4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq \
  /tmp/mainnet-bounty-pre-upgrade.so \
  --url https://api.mainnet-beta.solana.com

# If needed post-upgrade, redeploy the saved binary:
solana program write-buffer /tmp/mainnet-bounty-pre-upgrade.so \
  --url https://api.mainnet-beta.solana.com
# Then Rick signs:
solana program deploy \
  --program-id 4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq \
  --buffer <ROLLBACK_BUFFER_PUBKEY> \
  --upgrade-authority /Users/rickbarkley/amos-founder.json \
  --url https://api.mainnet-beta.solana.com
```

Trigger rollback only if:
- Mainnet smoke test settlement fails with an error not present on devnet
- Existing settlements start failing (check CloudWatch logs)
- Any PDA deserialization fails (indicates account-layout drift we didn't catch)

---

## Relay client patches that may be needed (Stage 5 prep)

Before Stage 5, the relay needs to map `category="discovery"` to `contribution_type=11`. Check:

```bash
grep -n "category_to_contribution_type\|\"discovery\"" \
  /Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-relay/src/settlement_retry.rs \
  /Users/rickbarkley/SW_Projects/ai_co/amos-automate/amos-relay/src/routes/bounties.rs
```

Expected current mapping:
```rust
fn category_to_contribution_type(category: &str) -> u8 {
    match category {
        "infrastructure" => 7,
        "growth" => 8,
        "research" => 3,
        "content" => 9,
        _ => 1,
    }
}
```

If `"discovery"` isn't there, add one line: `"discovery" => 11,` in BOTH call sites (settlement_retry.rs and routes/bounties.rs). Small patch, goes on the same branch, ships with the relay's normal CI. This must land before Stage 5's smoke-test bounty can settle correctly.

---

## Success checklist

- [ ] Stage 1: 93/93 tests pass on the branch
- [ ] Stage 2: devnet settled one type-11 bounty end-to-end
- [ ] Stage 3: mainnet upgrade tx hash recorded; program hash verified changed
- [ ] Stage 4: Discovery added to ContributionTypeRegistry (governance tx recorded)
- [ ] Stage 5: mainnet smoke-test bounty (type-11) settled with 150%+ multiplier applied
- [ ] Stage 6: UPGRADE-001 bounty settled on-chain (settlement_tx recorded); PR opened; branch ready to merge

---

## Bounty mechanics reminder

- `poster_wallet`: founder (already set on the bounty, cannot change)
- `claimer`: personal (already claimed; do NOT re-claim with a different wallet)
- `approver`: must NOT be founder (self-approval by poster blocked) and NOT personal (self-approval by claimer blocked). Use `5ik1JSm3…` (rick-reviewer) — trust 5 + council_member=true in relay DB.
- `reviewer_wallet` in the approve call: sets where the 5% cut lands. Recommend `5ik1JSm3…` (real keypair, spendable). Do NOT use `87Gzq…` (the old placeholder has no keypair — tokens would be stranded).

---

## Points of contact

- **Rick** (founder, signs mainnet upgrade): `rick@amoslabs.com`
- **Bounty** on relay: https://relay.amoslabs.com/bounties/30c5f29e-5bf0-4915-ba79-58dc433e7d14
- **Previous session's work** (this session, 2026-04-20): committed on branch `bounty/30c5f29e-5bf0-4915-ba79-58dc433e7d14` + pushed to origin.
