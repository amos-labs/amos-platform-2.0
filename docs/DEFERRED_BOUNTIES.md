# Deferred & Follow-Up Work Tracker

**Purpose:** single source of truth for bounties/work items that are scoped but deliberately not-yet-posted, plus posted bounties that are *paused* pending external conditions. Update as items move.

Last updated: 2026-04-19

---

## 🟡 Ready to post (drafts complete, awaiting decision)

### OPS-ONCHAIN-UPGRADE-001
- **Draft:** `docs/OPS_ONCHAIN_UPGRADE_001_DRAFT.md`
- **What:** deploy the 5 already-in-source Anchor program deltas to mainnet (`bootstrap_agent_trust`, `authority_withdraw`, Discovery contribution type, emission fixes, `update_mint`)
- **Blocks:** ORACLE-001a, ORACLE-001b, new QA bots, Grand Challenge as bytecode
- **Status:** Rick reviewing. Open items: upgrade-authority hygiene, Discovery sigmoid params verification, `authority_withdraw` usage policy, rollback tolerance.

### OPS-ORACLE-001a (Intake Path)
- **Draft:** `docs/OPS_ORACLE_001_DRAFT.md`
- **What:** Oracle agent decides if customer submissions should become system bounties. Plural + earned qualification + council teaching window.
- **Depends on:** OPS-QA-001 (QA bot deployed) + OPS-ONCHAIN-UPGRADE-001 (trust-5 bootstrap)
- **Status:** spec v2 with Rick's edits complete. Open items before posting:
  - Verify customer submission channel exists (or add to scope)
  - Curate 20-submission test set + 10 red-team prompt-injection set
  - Founder + council sign v1 mission-alignment prompt
- **Deferred edits to apply:** 4 items from review (add UPGRADE-001 dep, note 10% is app-layer-only, Discovery test set coverage, oracle_review timing)

### OPS-ORACLE-001b (Review Path)
- **Same draft as 001a.** Ships *after* 001a has ≥30 days OR ≥30-bounty precedent + ≥80% council-match.
- **Status:** spec complete; build blocks on 001a's gate.

---

## 🟠 Deferred (coded/scoped, waiting on external signal)

### OPS-BUDGET-CAP-001 — 15% Autonomous Daily Cap
- **Posted:** `321818cd-57bd-413e-8328-cd9561db446b` (currently `open` on relay, 2343 points)
- **Status:** **PAUSED** — Rick is running simulations that may change the 15% parameter value before it goes into immutable bytecode.
- **What changes when unpaused:** simulations finalize cap value(s); code adds `autonomous_daily_budget_bps` field on `BountyConfig` PDA, `autonomous_spent_today` tracking on `DailyPool`, `autonomous: bool` flag on `post_bounty_listing`, and an `AutonomousBudgetExceeded` error. Ships as its own on-chain upgrade (following the UPGRADE-001 pattern).
- **Related risk until this ships:** v2 Part VIII's claim that the 15% cap is a program constant stays aspirational. Oracle's 10% intake budget is application-layer-only with no on-chain backstop. Runaway Oracle commissioning is bounded only by confidence-threshold escalation + council teaching window.
- **When to unpause:** after sim results settle. Consider bundling with any other on-chain changes accumulated by then.

---

## 🔵 Follow-on bounties (scoped but not yet drafted)

### OPS-TRUST-BOOTSTRAP-ENDPOINT-001
- **What:** reusable admin REST endpoint (`POST /api/v1/admin/trust/bootstrap` with `X-Admin-Key`) that wraps the on-chain `bootstrap_agent_trust` call. Replaces the one-shot standalone binary added 2026-04-19.
- **Status:** Option B from yesterday's wallet-bootstrap discussion; we did Option A (binary) for immediate unblock. This is the reusable-infrastructure follow-up.
- **Depends on:** OPS-ONCHAIN-UPGRADE-001 (bootstrap instruction needs to be deployed first).
- **Why:** lets QA bot + future Oracle agents + customer-onboarding flows self-bootstrap via auth, without operator running a local binary each time.

### OPS-POINTING-CAP-ALIGNMENT-001
- **What:** fix `amos-relay/src/pointing.rs` so the auto-pointing engine can't produce values that exceed the on-chain `MAX_BOUNTY_POINTS` (2000). Currently clamps to `[100, 10_000]` which silently creates settle-incapable bounties.
- **Status:** identified as a real bug during 2026-04-19 settlement debugging (IDEMPOTENCY-001 got 2072 pts → on-chain rejection).
- **Size:** small. Change the upper clamp to 2000 + add a clippy test that catches the drift.

### OPS-ORACLE-002 — Multi-Oracle Routing
- **What:** work-distribution among competing Oracles based on reputation per category
- **Depends on:** ORACLE-001a + ORACLE-001b shipped, ≥2 Oracle operators registered
- **Scoped in:** OPS-ORACLE-001_DRAFT.md bottom ("Out of scope")

### OPS-ORACLE-003 — Worker-Chooses-Reviewer
- **What:** worker picks from qualified Oracles with reputation weighting; picking consistently lenient ones degrades worker reputation
- **Depends on:** ORACLE-002

### OPS-ORACLE-004 — Oracle-to-Oracle Dispute Resolution
- **What:** when two Oracles disagree on a review, escalation protocol
- **Depends on:** ORACLE-002

### OPS-ORACLE-005 — Second Oracle Operator (alignment diversity)
- **What:** register a second Oracle with a deliberately different objective framing, compare outcomes
- **Depends on:** ORACLE-001b shipped with stable v1

### OPS-ORACLE-006 — Fine-Tuned Oracle Model
- **What:** train / fine-tune a model on the 001a + 001b decision corpus once ≥200 decisions accumulate
- **Depends on:** ORACLE-001a running ≥200 decisions

### OPS-ORACLE-007 — Drift-Detection Tooling
- **What:** automated weekly aggregate-pattern review of Oracle decisions; flag category / sentiment / systematic bias
- **Depends on:** ORACLE-001a deployed; formalizes the "single-Oracle drift detection" promise from Principle 9 in the ORACLE spec
- **Urgency:** before OPS-ORACLE-002 (multi-Oracle) lands, so we have a drift baseline before plural complicates the picture

---

## 🟣 Cross-cutting / operational

### Hardware-wallet migration for upgrade authority
- **What:** migrate mainnet program upgrade-authority signing off `/Users/rickbarkley/amos-founder.json` to a hardware wallet (Ledger / Seed Signer)
- **Why:** reduces key exposure; no hot keypair required for upgrades
- **When:** post-UPGRADE-001 (do after proving upgrade cycle works once)
- **Size:** small operationally, but requires testing with the Anchor upgrade flow
- **Noted in:** OPS_ONCHAIN_UPGRADE_001_DRAFT.md "Notes for Rick"

### v2 thesis doc: acknowledge aspirational vs bytecode claims
- **What:** update `docs/AMOS_THESIS_AND_STRATEGY_v2.md` Part VIII to note which on-chain claims are currently bytecode vs aspirational, and the intended sequence to move each from aspirational to bytecode
- **Why:** matches the reviewer feedback that load-bearing claims should flag their current state
- **Current aspirational claims (pre-UPGRADE-001):** Discovery / Grand Challenge direction, 15% autonomous cap, on-chain trust bootstrap
- **After UPGRADE-001:** Discovery bytecode; cap + bootstrap still aspirational (cap via BUDGET-CAP, bootstrap already bytecode post-upgrade)

### Pointing engine precision + simulation
- **What:** AMOS-RESEARCH-001 (from Part XI of v2 thesis) — simulate the compounded token economics (sigmoid emission × time-drip × virtual points × pool separation × tenure decay × vault tiers × trust gating) before the ContributionTypeRegistry freeze window starts ticking at year 3
- **Why:** nobody has tested this at scale; parameters going immutable should be validated
- **Overlaps:** Rick's in-flight simulations relate to this (the 15% cap param tuning is a subset)
- **Size:** large — a real research bounty

---

## How to use this doc

- **When scoping a new deferred item:** add it under the appropriate heading. Include draft path if it exists, what it depends on, what signal unpauses it.
- **When unpausing:** move the item out of this doc and into a posted bounty + task. Link here for history.
- **Weekly:** glance through and check no items have silently rotted (external signal arrived but item didn't move).
