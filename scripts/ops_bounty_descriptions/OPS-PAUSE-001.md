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

