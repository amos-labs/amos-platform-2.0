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

