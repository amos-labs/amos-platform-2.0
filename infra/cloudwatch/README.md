# AMOS CloudWatch Observability

Dashboard, log metric filters, and alarms for the AMOS protocol stack. All resources prefixed `amos-*` for safe scoped teardown.

## Live links

- **Dashboard:** https://us-east-1.console.aws.amazon.com/cloudwatch/home?region=us-east-1#dashboards:name=amos-protocol-health
- **Alarms:** https://us-east-1.console.aws.amazon.com/cloudwatch/home?region=us-east-1#alarmsV2:?search=amos-

## Files

- `dashboard.json` — CloudWatch dashboard config (widgets pulling from custom metrics + raw log queries)
- `metric-filters.json` — log metric filters extracting bounty-lifecycle / Oracle-decision counters from `/ecs/amos-relay` and `/ecs/amos-oracle`
- `alarms.json` — alarm definitions (no SNS notification action wired yet — see TODO below)
- `../scripts/deploy-observability.sh` — idempotent apply/teardown

## What's measured

**Custom metrics** (`AMOS/Relay`, `AMOS/Oracle` namespaces, extracted from logs):

| Metric | Source | Triggered by |
|---|---|---|
| `BountyCreatedCount` | relay | `Created bounty <id>` |
| `BountyClaimedCount` | relay | `Bounty <id> claimed by agent` |
| `BountySettledCount` | relay | `On-chain settlement successful` |
| `SettlementFailedCount` | relay | `On-chain settlement failed` |
| `MergeRecordedCount` | relay | `merge recorded` (auto-merge bot) |
| `RelayErrorCount` | relay | any `level=ERROR` line |
| `OracleTickCount` | oracle | `tick: polling` (60s cadence; absence = daemon dead) |
| `OracleDecisionCount` | oracle | `*decision made` |
| `OracleEscalationCount` | oracle | `*escalated to council` |
| `OracleErrorCount` | oracle | any `level=ERROR` line |

**Alarms**:

| Alarm | Threshold | Why |
|---|---|---|
| `amos-oracle-daemon-silent` | <1 tick / 15 min | Daemon crashed/stuck → intakes pile up |
| `amos-relay-error-burst` | >10 errors / 5 min | Hot endpoint outage |
| `amos-oracle-error-burst` | >5 errors / 10 min | Bedrock/relay/parsing failures |
| `amos-settlement-failures` | any failure / 5 min | Worker tokens stuck |
| `amos-oracle-escalation-flood` | >20 escalations / 1h | Calibration too tight or input quality dropped — council can't keep up |

## Deploying

```bash
# Get the topic ARN once (idempotent — re-creating returns the existing arn).
export AMOS_ALERT_TOPIC_ARN=$(aws sns create-topic --name amos-alerts \
    --region us-east-1 --query TopicArn --output text)

# Deploy alarms with notifications wired:
./scripts/deploy-observability.sh           # apply (idempotent)
./scripts/deploy-observability.sh --dry-run # show actions, change nothing
./scripts/deploy-observability.sh --teardown # remove every amos-* resource

# Or deploy without notifications (alarms fire silently — useful for bring-up):
unset AMOS_ALERT_TOPIC_ARN
./scripts/deploy-observability.sh
```

## SNS notifications (operating notes)

The five `amos-*` alarms route to the `amos-alerts` SNS topic. Subscribers
receive an email per alarm transition. The topic ARN is referenced via the
placeholder `__AMOS_ALERT_TOPIC_ARN__` in `alarms.json`; the deploy script
substitutes it at apply-time using `$AMOS_ALERT_TOPIC_ARN`.

### Adding a new subscriber

```bash
aws sns subscribe \
  --topic-arn "$AMOS_ALERT_TOPIC_ARN" \
  --protocol email \
  --notification-endpoint someone@example.com \
  --region us-east-1
```

The new subscriber will receive a confirmation email from AWS — they must
click the link before they start receiving alarm notifications. Until they
do, `aws sns list-subscriptions-by-topic` shows their `SubscriptionArn` as
`PendingConfirmation`. `scripts/verify-sns-wiring.sh` enforces the
`Confirmed` state for the founder address.

### Verifying the wiring is live

```bash
./scripts/verify-sns-wiring.sh
```

Read-only checks: topic exists, subscriber confirmed, every alarm references
the topic ARN. Exits 0 on success; non-zero with a diagnostic line on the
first failure.

### Testing an alarm fires end-to-end

Force one of the alarms into ALARM state to confirm an email actually
arrives:

```bash
aws cloudwatch set-alarm-state \
  --alarm-name amos-oracle-daemon-silent \
  --state-value ALARM \
  --state-reason "manual test" \
  --region us-east-1
```

You should receive an email within ~30 seconds. The alarm will return to
its real evaluated state on the next metric tick (no permanent damage).

### Rotating the subscribed email

Unsubscribe the old, subscribe the new, confirm the new — then update
`scripts/verify-sns-wiring.sh`'s `EXPECTED_EMAIL` (or set
`AMOS_ALERT_EMAIL` env var when running it).

```bash
aws sns unsubscribe --subscription-arn <old-sub-arn> --region us-east-1
```

## TODO

- **Admin app embed.** Render the same metric data inside the AMOS admin app — either iframe the CloudWatch dashboard (read-only access via federated session) or query via `aws-sdk` from a server-side endpoint and render in a custom panel.
- **Settled-but-unmerged backlog metric.** Currently inferable only from a relay query. Either emit a counter on the auto-merge bot run, or query the relay periodically via a small Lambda and `PutMetricData`.
- **Bedrock cost tracking.** AWS Cost Explorer → Bedrock. Add an alarm if hourly spend exceeds a threshold (Oracle daemon hot loop runaway).
- **Stale escalation backlog.** Custom metric: count of pending escalations older than 24h. Same approach as above — periodic Lambda or relay-side counter.
- **Slack delivery.** Add a Lambda subscriber that translates SNS events into Slack messages — useful for shared-channel visibility once the team grows.

## Notes

- All metric filters use the structured-log JSON pattern (`{ $.fields.message = "..." }`) since the relay + oracle both emit JSON logs via tracing.
- Filter patterns containing wildcards rely on CloudWatch's startswith/endswith semantics for the JSON-style filter — the `*` in `"On-chain settlement failed*"` matches the longer log line that includes the bounty ID.
- Alarms use `TreatMissingData: notBreaching` for error-burst alarms (silence is good); `breaching` for the daemon-silent alarm (silence is bad).
