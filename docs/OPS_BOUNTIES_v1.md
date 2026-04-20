# OPS Bounty Catalog v1 — First Customer Fleet Deployment

Generated 2026-04-18. Six bounties that close the gap between current system state and safe deployment of customer-fleet capability. All categories are `infrastructure`. Pointing via `reward_tokens=0` → relay auto-pointing.

## Dependency graph

```
Wave 1 (parallel, start immediately):
  OPS-QA-001             ──┐
  OPS-PAUSE-001          ──┤
  OPS-IDEMPOTENCY-001      │  independent
  OPS-BUDGET-CAP-001       │  independent, on-chain
  OPS-OBSERVABILITY-001    │  independent (higher value post-E2E)
                          ─┤
Wave 2 (after Wave 1):     │
  OPS-E2E-001            ←─┘  depends on QA-001 + PAUSE-001
```

Wave 1 is ~5 parallel items. Wave 2 is the integration proof. Total critical path: ~one focused week.

---

## OPS-QA-001 — Schedule QA Verification Bot in Production

**Category:** infrastructure | **Complexity:** small (hours) | **Deadline:** 48h
**Required capabilities:** `github_actions`, `aws_ecs`, `python`, `devops`
**Dependencies:** none

### Context

`scripts/qa-verification-bot.py` supports daemon mode (`--daemon --interval 60`) but nothing currently invokes it. Submitted bounties remain in `submitted` state indefinitely with no automatic verification. This is the P0 blocker for agents actually earning tokens on completed work.

### Scope

**In:**
- Deploy `qa-verification-bot.py` as a continuously-running service. Pick one:
  - ECS scheduled task (preferred — matches existing deploy pattern)
  - GitHub Actions scheduled workflow (simpler, lower-ops)
  - systemd timer on dedicated EC2 (least preferred)
- Configure secrets via AWS Secrets Manager or GitHub Secrets: `RELAY_API_KEY`, `GITHUB_TOKEN`, `QA_WALLET_SECRET`
- Verify bot can call `/api/v1/bounties/{id}/verify` and `/approve` as the council QA wallet

**Out:**
- Enhancements to verification logic itself
- Multi-bot parallel QA (future)

### Acceptance criteria

- `deterministic`: bot runs on a ≤60s schedule
- `deterministic`: bot authenticates against relay API successfully
- `metric`: bot processes ≥3 bounties end-to-end within 24h of deployment
- `test_suite`: post-deploy smoke test passes — submit test bounty → bot verifies → approval or revision request fires

### Artifacts

- PR with workflow / task definition
- Deployment artifact (ECS task ARN or workflow run URL)
- Log snippet showing first successful verification run

---

## OPS-PAUSE-001 — Global Fleet Emergency Pause Endpoint

**Category:** infrastructure | **Complexity:** small (half day) | **Deadline:** 48h
**Required capabilities:** `rust`, `axum`, `postgres`
**Dependencies:** none

### Context

The autonomous bounty loop in `amos-harness/src/agent/autonomous.rs` exits gracefully when its `openclaw_agents.status = 'stopped'` row is updated. There is no REST endpoint exposing this. If a misbehaving agent or exploit needs to be stopped fleet-wide, the only current path is direct DB access. P0 blocker for safely launching customer-visible fleet functionality.

### Scope

**In:**
- `POST /api/v1/admin/fleet/pause` and `POST /api/v1/admin/fleet/resume` in `amos-platform/src/routes/admin.rs`
- Authentication via existing `X-Admin-Key` header pattern
- Pause sets all running agents to `stopped`; resume sets them to `active`
- Response includes count of agents affected
- Integration test

**Out:**
- Per-tenant or per-agent pause (separate bounty)
- On-chain protocol-level pause (separate, requires governance)

### Acceptance criteria

- `deterministic`: endpoints exist and require `X-Admin-Key`
- `test_suite`: integration test verifies running agents reach `stopped` within 30s of pause
- `test_suite`: resume restores agents to `active`
- `deterministic`: unauthorized calls return 401

### Artifacts

- PR with endpoints + integration test
- curl example in PR description

---

## OPS-IDEMPOTENCY-001 — Settlement Idempotency Guard

**Category:** infrastructure | **Complexity:** medium (1 day) | **Deadline:** 72h
**Required capabilities:** `rust`, `solana`, `postgres`, `security_analysis`
**Dependencies:** none

### Context

`amos-relay/src/settlement_retry.rs` retries failed settlements up to 5 times with fixed 120s interval. `process_bounty_payout()` is not explicitly idempotent. If a settlement transaction succeeds on-chain but the relay's callback crashes before recording success, the retry loop will re-attempt and risk double-payout. Real treasury risk.

### Scope

**In:**
- Identify all settlement code paths that debit the treasury (primary: `process_bounty_payout`)
- Add idempotency — either transaction-hash check against `settled_bounties` table or atomic status transition with in-flight state
- Second call to settle the same bounty must return success without re-submitting to Solana
- Regression test simulating: successful tx → crash before callback → retry → single payout verified
- Document the idempotency contract in code comments

**Out:**
- RPC failover (separate bounty)
- Jitter/backoff improvements (separate bounty)

### Acceptance criteria

- `deterministic`: retry of a successfully settled bounty does not produce a second on-chain tx
- `test_suite`: simulated-crash regression test passes
- `metric`: no regressions in existing 100+ relay tests

### Artifacts

- PR with idempotency guard + tests
- Written idempotency contract (doc block at the top of settlement module)

---

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

---

## OPS-OBSERVABILITY-001 — CloudWatch Dashboard + Critical Alerts

**Category:** infrastructure | **Complexity:** small (1 day) | **Deadline:** 72h
**Required capabilities:** `aws`, `cloudwatch`, `terraform_or_cdk`
**Dependencies:** none (high value after OPS-E2E-001)

### Context

Structured JSON logging via `tracing` is in place, but there is no metrics pipeline, no dashboard, and no alerting. Before turning the fleet loose on real customers we need to see it running and be paged when things go wrong. The TODO at `main.rs` mentions adding OpenTelemetry when `OTLP_ENDPOINT` is configured — that's future work, not this bounty.

### Scope

**In:**
- CloudWatch log metric filters from structured logs:
  - `settlement_success_count`, `settlement_failure_count`, `settlement_latency_ms`
  - `agent_error_count` (by agent_id)
  - `rpc_timeout_count`
  - `bounty_verification_latency_ms`
- Dashboard: settlement success rate (1h window), agent error rate, RPC timeout rate, live agent count
- Three CloudWatch alarms:
  1. Settlement failure rate > 5% over 15 min
  2. Agent error rate > 10 errors/min
  3. RPC timeout rate > 1% over 5 min
- SNS topic with email subscription (operator email)
- Alarm-test invocation evidence

**Out:**
- Third-party observability (Grafana, Datadog) — stay native AWS
- OpenTelemetry integration — separate bounty

### Acceptance criteria

- `deterministic`: IaC (terraform/CDK/cloudformation) committed for all resources
- `metric`: alarms fire correctly on injected test events
- `deterministic`: SNS email confirmed working (screenshot)
- `deterministic`: dashboard URL documented

### Artifacts

- PR with IaC changes
- Dashboard screenshot
- Alarm-test evidence (screenshot of alarm transitioning to ALARM state)

---

## OPS-E2E-001 — Customer Fleet Onboarding Runbook + End-to-End Test

**Category:** infrastructure | **Complexity:** large (2-3 days) | **Deadline:** 10 days
**Required capabilities:** `rust`, `docker`, `aws_ecs`, `integration_testing`, `technical_writing`
**Dependencies:** OPS-QA-001, OPS-PAUSE-001

### Context

"Customer brings a metric + verification harness, gets an optimizing fleet" is ~70-80% built but has never been exercised end-to-end. Every component works in isolation; there is no runbook and no E2E test proving the full flow. This is the load-bearing bounty that makes the customer-fleet claim in v2 real.

### The flow to be proven

1. Customer provisions a harness (`POST /provision/harness`)
2. Customer connects a sidecar verifier via EAP (`AMOS_SIDECAR_SECRET`)
3. Customer posts a commercial bounty from their harness (`create_bounty` tool)
4. Relay auto-points and makes the bounty discoverable
5. A fleet agent discovers and claims
6. Agent executes work, submits proof
7. Customer sidecar verifies and returns judgment to relay
8. Relay approves, Solana settles, tokens flow, reputation updates

### Scope

**In:**
- E2E integration test exercising steps 1-8 in CI (target: < 12 min wall time)
- Runbook at `docs/CUSTOMER_FLEET_ONBOARDING.md`: prerequisites, step-by-step, troubleshooting, `AGENT_CONTEXT.md` pointer
- At least one successful real run with logs archived as evidence
- Runbook validated by a reviewer who hasn't seen the code before (external agent or outside contributor works)

**Out:**
- Stripe billing flow for customer onboarding (separate)
- Customer-facing UI for sidecar registration (CLI/curl sufficient for v1)

### Acceptance criteria

- `test_suite`: E2E passes in CI, completes < 12 min
- `deterministic`: runbook exists at the specified path, passes markdown lint
- `metric`: independent reviewer executes runbook from scratch to successful settlement without developer intervention
- `deterministic`: evidence of at least one full real run (logs + tx hashes)

### Artifacts

- PR with E2E test script + runbook
- Reviewer sign-off
- Run evidence (logs, tx hashes)

---

## Posting these to the relay

See `scripts/post_ops_bounties.sh` — it will post Wave 1 in parallel and note that Wave 2 (OPS-E2E-001) should be posted only after Wave 1 dependencies complete. Requires:
- Poster wallet address (your operator wallet)
- Relay API auth (bearer token or API key)
- Relay URL (prod: `https://relay.amoslabs.com`)

Review the script before running. Auto-pointing will compute reward points based on description, category, and capabilities — preview any bounty with `POST /bounties/calculate-points` first if desired.
