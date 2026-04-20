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

