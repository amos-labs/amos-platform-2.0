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

