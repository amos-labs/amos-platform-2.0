//! GitHub webhook handler for PR merge/reject signals.
//!
//! Listens for `pull_request` events:
//! - closed + merged: true  → record successful merge (optional reputation bonus)
//! - closed + merged: false → trigger pushback (reputation hit on the agent)
//!
//! Authentication: HMAC-SHA256 via `X-Hub-Signature-256` header.
//! The shared secret is in `GITHUB_WEBHOOK_SECRET` env var.

use crate::state::RelayState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use tracing::{info, warn};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub fn routes() -> Router<RelayState> {
    Router::new().route("/github", post(github_webhook))
}

// ── Request types ────────────────────────────────────────────────────────

/// Minimal GitHub pull_request webhook payload.
#[derive(Debug, Deserialize)]
struct GitHubWebhookPayload {
    action: String,
    pull_request: Option<PullRequestPayload>,
}

#[derive(Debug, Deserialize)]
struct PullRequestPayload {
    number: u64,
    merged: Option<bool>,
    head: HeadRef,
    title: String,
}

#[derive(Debug, Deserialize)]
struct HeadRef {
    #[serde(rename = "ref")]
    ref_name: String,
}

// ── Handler ──────────────────────────────────────────────────────────────

async fn github_webhook(
    State(state): State<RelayState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Step 1: Verify HMAC signature
    let secret = std::env::var("GITHUB_WEBHOOK_SECRET").unwrap_or_default();
    if secret.is_empty() {
        warn!("GITHUB_WEBHOOK_SECRET not configured — rejecting webhook");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !verify_signature(&secret, &body, signature) {
        warn!("GitHub webhook signature verification failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Step 2: Parse event type
    let event_type = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if event_type != "pull_request" {
        // We only care about pull_request events
        return Ok(Json(serde_json::json!({
            "status": "ignored",
            "event": event_type,
        })));
    }

    // Step 3: Parse payload
    let payload: GitHubWebhookPayload =
        serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

    if payload.action != "closed" {
        return Ok(Json(serde_json::json!({
            "status": "ignored",
            "action": payload.action,
        })));
    }

    let pr = payload.pull_request.ok_or(StatusCode::BAD_REQUEST)?;

    let merged = pr.merged.unwrap_or(false);

    // Step 4: Extract bounty ID from branch name (pattern: bounty/<uuid>)
    let bounty_id = extract_bounty_id_from_branch(&pr.head.ref_name);

    let Some(bounty_id) = bounty_id else {
        info!(
            pr_number = pr.number,
            branch = %pr.head.ref_name,
            "PR closed but branch doesn't match bounty pattern — ignoring"
        );
        return Ok(Json(serde_json::json!({
            "status": "ignored",
            "reason": "branch not a bounty branch",
        })));
    };

    if merged {
        // PR merged — record success
        info!(
            bounty_id = %bounty_id,
            pr_number = pr.number,
            "PR merged for bounty — recording success"
        );

        // Update bounty with merge info (optional — success is the default path)
        sqlx::query(
            "UPDATE relay_bounties SET result = jsonb_set(COALESCE(result, '{}'::jsonb), '{pr_merged}', 'true') WHERE id = $1",
        )
        .bind(bounty_id)
        .execute(&state.db)
        .await
        .ok();

        Ok(Json(serde_json::json!({
            "status": "merged",
            "bounty_id": bounty_id.to_string(),
            "pr_number": pr.number,
        })))
    } else {
        // PR closed without merge — pushback
        info!(
            bounty_id = %bounty_id,
            pr_number = pr.number,
            "PR closed without merge — recording pushback"
        );

        // Look up the agent who claimed this bounty
        let agent_row = sqlx::query_as::<_, (Uuid, Option<String>)>(
            "SELECT claimed_by_agent_id, claimed_by_wallet FROM relay_bounties WHERE id = $1",
        )
        .bind(bounty_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((_agent_id, Some(wallet))) = agent_row {
            // Apply pushback quality score penalty (-30)
            let quality_result = sqlx::query_as::<_, (Option<i16>,)>(
                r#"UPDATE relay_bounties
                   SET quality_score = GREATEST(0, COALESCE(quality_score, 85) - 30)
                   WHERE id = $1
                   RETURNING quality_score"#,
            )
            .bind(bounty_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            let new_score = quality_result.and_then(|r| r.0).unwrap_or(0);

            // Report negative outcome to reputation system
            sqlx::query(
                r#"INSERT INTO relay_reputation_events
                   (agent_id, event_type, bounty_id, quality_delta, reason, created_at)
                   SELECT id, 'pushback', $1, -30, $2, NOW()
                   FROM relay_agents WHERE wallet_address = $3"#,
            )
            .bind(bounty_id)
            .bind(format!(
                "PR #{} closed without merge: {}",
                pr.number, pr.title
            ))
            .bind(&wallet)
            .execute(&state.db)
            .await
            .ok();

            info!(
                bounty_id = %bounty_id,
                wallet = %wallet,
                new_score,
                "Pushback recorded: -30 quality score"
            );

            Ok(Json(serde_json::json!({
                "status": "pushback_recorded",
                "bounty_id": bounty_id.to_string(),
                "pr_number": pr.number,
                "quality_score": new_score,
            })))
        } else {
            warn!(
                bounty_id = %bounty_id,
                "Could not find agent for bounty — pushback not recorded"
            );
            Ok(Json(serde_json::json!({
                "status": "pushback_skipped",
                "bounty_id": bounty_id.to_string(),
                "reason": "no agent found",
            })))
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Verify GitHub webhook HMAC-SHA256 signature.
fn verify_signature(secret: &str, body: &str, signature: &str) -> bool {
    let expected_prefix = "sha256=";
    let Some(hex_sig) = signature.strip_prefix(expected_prefix) else {
        return false;
    };

    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body.as_bytes());

    let Ok(expected_bytes) = hex::decode(hex_sig) else {
        return false;
    };

    mac.verify_slice(&expected_bytes).is_ok()
}

/// Extract bounty UUID from a branch name like "bounty/<uuid>".
fn extract_bounty_id_from_branch(branch: &str) -> Option<Uuid> {
    branch
        .strip_prefix("bounty/")
        .and_then(|id_str| Uuid::parse_str(id_str).ok())
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_signature_valid() {
        let secret = "test_secret";
        let body = r#"{"action":"closed"}"#;
        // Compute expected signature
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body.as_bytes());
        let result = mac.finalize();
        let hex_sig = hex::encode(result.into_bytes());
        let signature = format!("sha256={hex_sig}");

        assert!(verify_signature(secret, body, &signature));
    }

    #[test]
    fn test_verify_signature_invalid() {
        assert!(!verify_signature("secret", "body", "sha256=0000000000"));
    }

    #[test]
    fn test_verify_signature_missing_prefix() {
        assert!(!verify_signature("secret", "body", "bad_signature"));
    }

    #[test]
    fn test_extract_bounty_id_valid() {
        let uuid = Uuid::new_v4();
        let branch = format!("bounty/{uuid}");
        assert_eq!(extract_bounty_id_from_branch(&branch), Some(uuid));
    }

    #[test]
    fn test_extract_bounty_id_no_prefix() {
        assert_eq!(extract_bounty_id_from_branch("feature/my-feature"), None);
    }

    #[test]
    fn test_extract_bounty_id_invalid_uuid() {
        assert_eq!(extract_bounty_id_from_branch("bounty/not-a-uuid"), None);
    }

    #[test]
    fn test_extract_bounty_id_main_branch() {
        assert_eq!(extract_bounty_id_from_branch("main"), None);
    }

    #[test]
    fn test_payload_deserialize() {
        let json = r#"{
            "action": "closed",
            "pull_request": {
                "number": 42,
                "merged": true,
                "head": {"ref": "bounty/123e4567-e89b-12d3-a456-426614174000"},
                "title": "Bounty: fix the thing"
            }
        }"#;
        let payload: GitHubWebhookPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.action, "closed");
        let pr = payload.pull_request.unwrap();
        assert_eq!(pr.number, 42);
        assert_eq!(pr.merged, Some(true));
        assert!(pr.head.ref_name.starts_with("bounty/"));
    }
}
