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

