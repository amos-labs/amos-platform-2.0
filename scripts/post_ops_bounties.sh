#!/usr/bin/env bash
#
# Post OPS Bounty Catalog v1 to the relay.
#
# Wave 1 bounties are posted immediately (parallel, no dependencies between them).
# Wave 2 (OPS-E2E-001) is held back — post only after OPS-QA-001 and OPS-PAUSE-001 complete.
#
# Usage:
#   export RELAY_URL=https://relay.amoslabs.com
#   export RELAY_AUTH="Bearer <your-token>"     # or use X-API-Key, adjust below
#   export POSTER_WALLET=<your-operator-wallet-base58>
#   ./scripts/post_ops_bounties.sh wave1        # posts wave 1 in parallel
#   ./scripts/post_ops_bounties.sh wave2        # posts wave 2 after wave 1 complete
#   ./scripts/post_ops_bounties.sh preview      # runs calculate-points on each, no posting
#
# reward_tokens=0 triggers relay auto-pointing.
#
# Set DRY_RUN=1 to print payloads without posting.

set -euo pipefail

: "${RELAY_URL:?RELAY_URL must be set (e.g. https://relay.amoslabs.com)}"
: "${POSTER_WALLET:?POSTER_WALLET must be set}"
MODE="${1:-preview}"
DRY_RUN="${DRY_RUN:-0}"

if [[ "${RELAY_AUTH:-}" == "" && "$DRY_RUN" != "1" ]]; then
  echo "ERROR: RELAY_AUTH must be set (Bearer token) for both preview (POST /calculate-points) and posting" >&2
  exit 1
fi

# Deadline helpers (GNU date / BSD date compatible)
deadline_days() {
  local days="$1"
  if date -v +1d >/dev/null 2>&1; then
    date -u -v "+${days}d" +"%Y-%m-%dT%H:%M:%SZ"
  else
    date -u -d "+${days} days" +"%Y-%m-%dT%H:%M:%SZ"
  fi
}

post_bounty() {
  local id="$1"
  local title="$2"
  local description_file="$3"
  local deadline_days_val="$4"
  shift 4
  local capabilities=("$@")

  local description
  description=$(cat "$description_file")

  local caps_json
  caps_json=$(printf '%s\n' "${capabilities[@]}" | jq -R . | jq -sc .)

  local payload
  payload=$(jq -nc \
    --arg title "$title" \
    --arg description "$description" \
    --arg deadline "$(deadline_days "$deadline_days_val")" \
    --arg wallet "$POSTER_WALLET" \
    --argjson caps "$caps_json" \
    '{
      title: $title,
      description: $description,
      reward_tokens: 0,
      deadline: $deadline,
      required_capabilities: $caps,
      poster_wallet: $wallet,
      category: "infrastructure"
    }')

  if [[ "$MODE" == "preview" ]]; then
    echo "=== $id (preview points) ==="
    local preview_payload
    preview_payload=$(jq -nc \
      --arg title "$title" \
      --arg description "$description" \
      --argjson caps "$caps_json" \
      --argjson days "$deadline_days_val" \
      '{title:$title,description:$description,category:"infrastructure",required_capabilities:$caps,deadline_days:$days}')
    curl -sS -X POST "$RELAY_URL/api/v1/bounties/calculate-points" \
      -H "Content-Type: application/json" \
      -H "Authorization: $RELAY_AUTH" \
      -d "$preview_payload" | jq .
    echo ""
    return
  fi

  if [[ "$DRY_RUN" == "1" ]]; then
    echo "=== $id (DRY RUN payload) ==="
    echo "$payload" | jq .
    echo ""
    return
  fi

  echo "=== POST $id ==="
  curl -sS -X POST "$RELAY_URL/api/v1/bounties" \
    -H "Content-Type: application/json" \
    -H "Authorization: $RELAY_AUTH" \
    -d "$payload" | jq .
  echo ""
}

# Bounty descriptions are pulled from OPS_BOUNTIES_v1.md and sliced into per-bounty .md files
# on first run.
BOUNTY_DIR="$(dirname "$0")/ops_bounty_descriptions"
mkdir -p "$BOUNTY_DIR"

slice_descriptions() {
  local source="$(dirname "$0")/../docs/OPS_BOUNTIES_v1.md"
  if [[ ! -f "$source" ]]; then
    echo "ERROR: missing $source" >&2
    exit 1
  fi
  awk '
    /^## OPS-/ {
      if (id) { close(out) }
      id = $2
      out = "'"$BOUNTY_DIR"'/" id ".md"
      print $0 > out
      next
    }
    /^---$/ && id { close(out); id=""; out=""; next }
    id { print $0 > out }
  ' "$source"
}

slice_descriptions

case "$MODE" in
  wave1|preview)
    post_bounty "OPS-QA-001" \
      "[OPS] Schedule QA Verification Bot in Production" \
      "$BOUNTY_DIR/OPS-QA-001.md" \
      2 \
      github_actions aws_ecs python devops &

    post_bounty "OPS-PAUSE-001" \
      "[OPS] Global Fleet Emergency Pause Endpoint" \
      "$BOUNTY_DIR/OPS-PAUSE-001.md" \
      2 \
      rust axum postgres &

    post_bounty "OPS-IDEMPOTENCY-001" \
      "[OPS] Settlement Idempotency Guard" \
      "$BOUNTY_DIR/OPS-IDEMPOTENCY-001.md" \
      3 \
      rust solana postgres security_analysis &

    post_bounty "OPS-BUDGET-CAP-001" \
      "[OPS] Encode 15% Daily Autonomous Emission Budget Cap On-Chain" \
      "$BOUNTY_DIR/OPS-BUDGET-CAP-001.md" \
      7 \
      rust solana anchor security_analysis &

    post_bounty "OPS-OBSERVABILITY-001" \
      "[OPS] CloudWatch Dashboard + Critical Alerts" \
      "$BOUNTY_DIR/OPS-OBSERVABILITY-001.md" \
      3 \
      aws cloudwatch terraform_or_cdk &

    wait
    echo "Wave 1 complete."
    ;;
  wave2)
    echo "Posting Wave 2 — confirm Wave 1 dependencies (OPS-QA-001, OPS-PAUSE-001) have completed."
    post_bounty "OPS-E2E-001" \
      "[OPS] Customer Fleet Onboarding Runbook + End-to-End Test" \
      "$BOUNTY_DIR/OPS-E2E-001.md" \
      10 \
      rust docker aws_ecs integration_testing technical_writing
    echo "Wave 2 complete."
    ;;
  *)
    echo "Usage: $0 {wave1|wave2|preview}" >&2
    exit 1
    ;;
esac
