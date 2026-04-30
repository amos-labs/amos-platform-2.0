#!/usr/bin/env bash
#
# verify-sns-wiring.sh — read-only verifier for the SNS notification setup
# behind the amos-* CloudWatch alarms.
#
# This script IS the bounty's `test_command` for the QA verification bot. It
# performs only describe/list/get operations against AWS — never create/
# update/delete. Exits 0 when the deployed configuration matches expectations,
# non-zero with a diagnostic line on the first failure.
#
# Verifies (the bounty's acceptance criteria):
#   1. SNS topic `amos-alerts` exists in us-east-1
#   2. rick@amoslabs.com is subscribed to it AND status is `Confirmed`
#   3. All 5 amos-* alarms have AlarmActions = [topic ARN]
#   4. (No CRUD performed — script is safe to run any number of times)

set -euo pipefail

REGION="${AWS_REGION:-us-east-1}"
TOPIC_NAME="${AMOS_ALERT_TOPIC_NAME:-amos-alerts}"
EXPECTED_EMAIL="${AMOS_ALERT_EMAIL:-rick@amoslabs.com}"
EXPECTED_ALARMS=(
    "amos-oracle-daemon-silent"
    "amos-relay-error-burst"
    "amos-oracle-error-burst"
    "amos-settlement-failures"
    "amos-oracle-escalation-flood"
)

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

log() { echo "[verify-sns $(date -u +%H:%M:%S)] $*"; }

# ── Check 1: topic exists ─────────────────────────────────────────────
log "Looking for SNS topic '$TOPIC_NAME' in $REGION..."
TOPIC_ARN=$(aws sns list-topics --region "$REGION" \
    --query "Topics[?ends_with(TopicArn, ':${TOPIC_NAME}')].TopicArn" \
    --output text 2>/dev/null || true)

if [ -z "$TOPIC_ARN" ] || [ "$TOPIC_ARN" = "None" ]; then
    fail "SNS topic '$TOPIC_NAME' not found in $REGION"
fi
log "topic ARN: $TOPIC_ARN"

# ── Check 2: subscription is confirmed ────────────────────────────────
log "Checking subscriber '$EXPECTED_EMAIL' status..."
SUB_STATUS=$(aws sns list-subscriptions-by-topic --region "$REGION" \
    --topic-arn "$TOPIC_ARN" \
    --query "Subscriptions[?Endpoint=='${EXPECTED_EMAIL}'].SubscriptionArn" \
    --output text 2>/dev/null || true)

if [ -z "$SUB_STATUS" ] || [ "$SUB_STATUS" = "None" ]; then
    fail "no subscription found for $EXPECTED_EMAIL on $TOPIC_ARN"
fi

if [ "$SUB_STATUS" = "PendingConfirmation" ]; then
    fail "subscription for $EXPECTED_EMAIL is still PendingConfirmation — \
the subscriber must click the confirmation link in the AWS-sent email"
fi

if [[ "$SUB_STATUS" != arn:aws:sns:* ]]; then
    fail "unexpected subscription state: $SUB_STATUS"
fi

log "subscriber '$EXPECTED_EMAIL' confirmed (sub: ${SUB_STATUS##*:})"

# ── Check 3: every amos-* alarm has AlarmActions = [topic ARN] ────────
log "Verifying alarm actions..."
for ALARM in "${EXPECTED_ALARMS[@]}"; do
    ACTIONS=$(aws cloudwatch describe-alarms --region "$REGION" \
        --alarm-names "$ALARM" \
        --query 'MetricAlarms[0].AlarmActions' \
        --output json 2>/dev/null || echo "[]")

    if [ "$ACTIONS" = "null" ] || [ "$ACTIONS" = "[]" ]; then
        fail "alarm '$ALARM' has no AlarmActions (expected $TOPIC_ARN)"
    fi

    # Expect the actions array to contain the topic ARN.
    if ! echo "$ACTIONS" | grep -qF "$TOPIC_ARN"; then
        fail "alarm '$ALARM' AlarmActions does not include $TOPIC_ARN (got: $ACTIONS)"
    fi

    log "  ✓ $ALARM → $TOPIC_ARN"
done

echo
log "All checks passed: SNS wiring is live and notifications will fire."
exit 0
