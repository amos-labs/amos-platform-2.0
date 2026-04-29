//! Protocol intake tools — OPS-HARNESS-INTAKE-TOOL-001.
//!
//! These tools turn the user's AMOS harness into the intake interface for the
//! AMOS protocol. When the user reports a bug, requests a feature, or
//! describes work that should be done across the system, the harness drafts
//! a structured ticket and submits it to the relay's `/api/v1/intakes`
//! endpoint. The Oracle picks it up, reasons about it, and either commissions
//! a bounty (an external agent will claim and fix it), refines, escalates, or
//! rejects.
//!
//! Two tools:
//!
//! - `submit_protocol_intake` — creates a new intake submission. The harness
//!   drafts the title and body collaboratively with the user, then calls
//!   this tool with the structured result.
//!
//! - `check_protocol_intake_status` — given a submission_id, returns the
//!   Oracle's verdict and (if commissioned) the resulting bounty's status,
//!   settlement state, merge SHA, and PR URL. Use when the user asks "what
//!   happened to that bug I reported."
//!
//! Identity, cost, and anti-spam are handled by the user's harness account
//! (per-user billing on Bedrock + relay rate-limiting), so no protocol-side
//! deposit gate is required.

use super::{Tool, ToolCategory, ToolResult};
use amos_core::{AppConfig, Result};
use async_trait::async_trait;
use secrecy::ExposeSecret;
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use tracing::warn;

const INTAKE_TITLE_MAX: usize = 500;
const INTAKE_BODY_MAX: usize = 50_000;
const INTAKE_BODY_MIN: usize = 40;

/// Default submitter label used when the harness has no more specific
/// identity to attribute. `amos-harness` is enough for the Oracle to know
/// "this came in through a user-facing harness, not a system path."
const DEFAULT_SUBMITTER: &str = "amos-harness";

// ──────────────────────────────────────────────────────────────────────
// Pure validation — extracted so unit tests don't need a full AppConfig.
// ──────────────────────────────────────────────────────────────────────

/// Validated, cleaned shape of a submit_protocol_intake request. Returned by
/// `validate_submit_params` so unit tests don't need a full `AppConfig`.
#[derive(Debug)]
struct ValidatedIntake {
    title: String,
    body: String,
    suggested_category: Option<String>,
    suggested_capabilities: Vec<String>,
    submitter_wallet: Option<String>,
}

/// Lightweight Solana base58 sanity check — same length and alphabet rules
/// the relay's validate_wallet_address enforces, kept inline so the harness
/// crate doesn't take a relay dependency.
fn looks_like_solana_wallet(s: &str) -> bool {
    let len = s.len();
    if !(32..=44).contains(&len) {
        return false;
    }
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric() && b != b'0' && b != b'O' && b != b'I' && b != b'l')
}

fn validate_submit_params(
    params: &JsonValue,
) -> std::result::Result<ValidatedIntake, String> {
    let title = params
        .get("title")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "title is required".to_string())?;
    if title.len() > INTAKE_TITLE_MAX {
        return Err(format!("title must be ≤{INTAKE_TITLE_MAX} chars"));
    }

    let body = params
        .get("body")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "body is required".to_string())?;
    if body.len() < INTAKE_BODY_MIN {
        return Err(format!(
            "body too short ({} chars); minimum {INTAKE_BODY_MIN}",
            body.len()
        ));
    }
    if body.len() > INTAKE_BODY_MAX {
        return Err(format!("body must be ≤{INTAKE_BODY_MAX} chars"));
    }

    let suggested_category = params
        .get("suggested_category")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    let suggested_capabilities: Vec<String> = params
        .get("suggested_capabilities")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::trim).filter(|s| !s.is_empty()))
                .map(String::from)
                .take(20)
                .collect()
        })
        .unwrap_or_default();

    let submitter_wallet = params
        .get("submitter_wallet")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(w) = submitter_wallet {
        if !looks_like_solana_wallet(w) {
            return Err(format!(
                "submitter_wallet '{w}' does not look like a Solana base58 address"
            ));
        }
    }

    Ok(ValidatedIntake {
        title: title.to_string(),
        body: body.to_string(),
        suggested_category,
        suggested_capabilities,
        submitter_wallet: submitter_wallet.map(String::from),
    })
}

// ──────────────────────────────────────────────────────────────────────
// SubmitProtocolIntakeTool
// ──────────────────────────────────────────────────────────────────────

pub struct SubmitProtocolIntakeTool {
    config: Arc<AppConfig>,
}

impl SubmitProtocolIntakeTool {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }

    fn submitter_label(&self) -> String {
        std::env::var("AMOS_HARNESS_ID")
            .map(|id| format!("amos-harness:{id}"))
            .unwrap_or_else(|_| DEFAULT_SUBMITTER.to_string())
    }
}

#[async_trait]
impl Tool for SubmitProtocolIntakeTool {
    fn name(&self) -> &str {
        "submit_protocol_intake"
    }

    fn description(&self) -> &str {
        "Submit a bug report, feature request, or system gap to the AMOS \
         protocol intake queue. The Oracle will read it, reason about \
         priority + mission alignment, and either commission a bounty (an \
         external agent will pick it up and fix it), refine, escalate to \
         council, or reject.\n\
         \n\
         BEFORE CALLING: collaborate with the user to gather:\n\
         - A clear, specific title (≤500 chars)\n\
         - A body (≥40 chars, ≤50,000) describing: what's broken or wanted, \
           reproduction steps if it's a bug, the user's intent, and what \
           'done' looks like (acceptance criteria)\n\
         - Optional: suggested_category (e.g. 'bug', 'feature', \
           'infrastructure'), suggested_capabilities (e.g. ['rust','axum'])\n\
         \n\
         You generally have far more session context than the user remembers \
         to mention — pull from recent activity to make the ticket richer \
         (file paths, error messages, what workflow they were in when it \
         broke). The richer the ticket, the higher the chance Oracle \
         self-authorizes the bounty without escalating to council.\n\
         \n\
         Returns the submission_id; use check_protocol_intake_status later \
         to learn whether the Oracle commissioned a bounty for it.\n\
         \n\
         OPTIONAL — submitter_wallet: a Solana wallet to receive a finder's \
         fee (default 5%) when this report leads to a commissioned + settled \
         bounty. Only ask the user for one if they bring it up; reporting bugs \
         is free and they shouldn't have to think about wallets to file."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short imperative title (e.g., 'X-Request-ID header missing on /agents/register error responses'). ≤500 chars.",
                    "maxLength": INTAKE_TITLE_MAX
                },
                "body": {
                    "type": "string",
                    "description": "Full description: symptom, repro, intent, acceptance criteria. ≥40 chars, ≤50000.",
                    "minLength": INTAKE_BODY_MIN,
                    "maxLength": INTAKE_BODY_MAX
                },
                "suggested_category": {
                    "type": "string",
                    "description": "Hint to Oracle: 'bug', 'feature', 'infrastructure', 'docs', etc. Optional."
                },
                "suggested_capabilities": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Capabilities a worker would need (e.g. ['rust','axum','solana']). Optional, ≤20 items."
                },
                "submitter_wallet": {
                    "type": "string",
                    "description": "Optional Solana base58 wallet for finder's-fee payout (default 5% of reward when the report leads to a settled bounty). Reporting is free; only set this if the user volunteers a wallet."
                }
            },
            "required": ["title", "body"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let ValidatedIntake {
            title,
            body,
            suggested_category,
            suggested_capabilities,
            submitter_wallet,
        } = match validate_submit_params(&params) {
            Ok(v) => v,
            Err(msg) => {
                if msg.starts_with("body too short") {
                    return Ok(ToolResult::error(format!(
                        "{msg}. Describe symptom, repro, intent, and acceptance criteria so Oracle has enough to reason against."
                    )));
                }
                return Err(amos_core::AmosError::Validation(msg));
            }
        };

        let payload = json!({
            "title": title,
            "body": body,
            "submitter": self.submitter_label(),
            "suggested_category": suggested_category,
            "suggested_capabilities": suggested_capabilities,
            "submitter_wallet": submitter_wallet,
        });

        let url = format!(
            "{}/api/v1/intakes",
            self.config.relay.url.trim_end_matches('/')
        );
        let mut req = reqwest::Client::new().post(&url).json(&payload);
        if let Some(key) = self.config.relay.api_key.as_ref() {
            req = req.bearer_auth(key.expose_secret());
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => {
                let body: JsonValue = resp.json().await.unwrap_or(json!({}));
                let id = body
                    .get("submission_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                Ok(ToolResult::success(json!({
                    "submission_id": id,
                    "status": "pending",
                    "title": title,
                    "message": format!(
                        "Submitted to AMOS protocol intake (id: {id}). Oracle reviews intake on a 60s tick — use check_protocol_intake_status with this id to see the verdict."
                    )
                })))
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                warn!(%status, %url, "intake submission rejected");
                Ok(ToolResult::error(format!(
                    "Relay rejected intake (HTTP {status}): {text}"
                )))
            }
            Err(e) => {
                warn!(error = %e, %url, "intake submission send failed");
                Ok(ToolResult::error(format!(
                    "Failed to reach relay at {url}: {e}"
                )))
            }
        }
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TaskQueue
    }
}

// ──────────────────────────────────────────────────────────────────────
// CheckProtocolIntakeStatusTool
// ──────────────────────────────────────────────────────────────────────

pub struct CheckProtocolIntakeStatusTool {
    config: Arc<AppConfig>,
}

impl CheckProtocolIntakeStatusTool {
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for CheckProtocolIntakeStatusTool {
    fn name(&self) -> &str {
        "check_protocol_intake_status"
    }

    fn description(&self) -> &str {
        "Look up an AMOS protocol intake by submission_id. Returns the current \
         status (pending / evaluated), Oracle verdict (commission / refine / \
         escalate / reject / null), and — if Oracle commissioned a bounty — \
         the resulting bounty's status (open / claimed / submitted / approved \
         / etc.), settlement state, merge commit SHA, and PR URL. Use this \
         when the user asks what happened to a bug or request they previously \
         reported."
    }

    fn parameters_schema(&self) -> JsonValue {
        json!({
            "type": "object",
            "properties": {
                "submission_id": {
                    "type": "string",
                    "description": "The intake submission_id returned by submit_protocol_intake (UUID)."
                }
            },
            "required": ["submission_id"]
        })
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let id = params
            .get("submission_id")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| amos_core::AmosError::Validation("submission_id is required".into()))?;

        let base = self.config.relay.url.trim_end_matches('/');
        let intake_url = format!("{base}/api/v1/intakes/{id}");
        let client = reqwest::Client::new();
        let mut req = client.get(&intake_url);
        if let Some(key) = self.config.relay.api_key.as_ref() {
            req = req.bearer_auth(key.expose_secret());
        }

        let intake: JsonValue = match req.send().await {
            Ok(r) if r.status().is_success() => r.json().await.unwrap_or(json!({})),
            Ok(r) if r.status() == reqwest::StatusCode::NOT_FOUND => {
                return Ok(ToolResult::error(format!(
                    "Intake {id} not found. Check the submission_id is correct."
                )));
            }
            Ok(r) => {
                let s = r.status();
                let t = r.text().await.unwrap_or_default();
                return Ok(ToolResult::error(format!(
                    "Relay error fetching intake {id} (HTTP {s}): {t}"
                )));
            }
            Err(e) => {
                return Ok(ToolResult::error(format!("Failed to reach relay: {e}")));
            }
        };

        let mut out = json!({
            "submission_id": intake.get("submission_id").cloned().unwrap_or(json!(null)),
            "title": intake.get("title").cloned().unwrap_or(json!(null)),
            "status": intake.get("status").cloned().unwrap_or(json!("unknown")),
            "verdict": intake.get("verdict").cloned().unwrap_or(json!(null)),
            "evaluated_at": intake.get("evaluated_at").cloned().unwrap_or(json!(null)),
            "commissioned_bounty_id": intake.get("commissioned_bounty_id").cloned().unwrap_or(json!(null)),
        });

        if let Some(bid) = intake
            .get("commissioned_bounty_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            let bounty_url = format!("{base}/api/v1/bounties/{bid}");
            let mut breq = client.get(&bounty_url);
            if let Some(key) = self.config.relay.api_key.as_ref() {
                breq = breq.bearer_auth(key.expose_secret());
            }
            if let Ok(r) = breq.send().await {
                if r.status().is_success() {
                    let bounty: JsonValue = r.json().await.unwrap_or(json!({}));
                    let bounty_summary = json!({
                        "bounty_id": bounty.get("id").cloned().unwrap_or(json!(null)),
                        "status": bounty.get("status").cloned().unwrap_or(json!(null)),
                        "settlement_status": bounty.get("settlement_status").cloned().unwrap_or(json!(null)),
                        "settlement_tx": bounty.get("settlement_tx").cloned().unwrap_or(json!(null)),
                        "merge_commit_sha": bounty.get("merge_commit_sha").cloned().unwrap_or(json!(null)),
                        "pr_url": bounty.get("pr_url").cloned().unwrap_or(json!(null)),
                        "reward_tokens": bounty.get("reward_tokens").cloned().unwrap_or(json!(null)),
                    });
                    out["bounty"] = bounty_summary;
                }
            }
        }

        Ok(ToolResult::success(out))
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::TaskQueue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_missing_title() {
        let err = validate_submit_params(&json!({"body": "x".repeat(50)})).unwrap_err();
        assert!(err.contains("title"));
    }

    #[test]
    fn validate_rejects_whitespace_title() {
        let err = validate_submit_params(&json!({
            "title": "   ",
            "body": "x".repeat(50),
        }))
        .unwrap_err();
        assert!(err.contains("title"));
    }

    #[test]
    fn validate_rejects_short_body() {
        let err = validate_submit_params(&json!({"title": "Bug", "body": "short"})).unwrap_err();
        assert!(err.contains("body too short"), "got: {err}");
    }

    #[test]
    fn validate_caps_capability_list_at_20() {
        let caps: Vec<String> = (0..30).map(|i| format!("cap{i}")).collect();
        let v = validate_submit_params(&json!({
            "title": "Thing",
            "body": "x".repeat(50),
            "suggested_capabilities": caps,
        }))
        .unwrap();
        assert_eq!(v.suggested_capabilities.len(), 20);
    }

    #[test]
    fn validate_strips_empty_capabilities() {
        let v = validate_submit_params(&json!({
            "title": "Thing",
            "body": "x".repeat(50),
            "suggested_capabilities": ["rust", "  ", "axum", ""],
        }))
        .unwrap();
        assert_eq!(
            v.suggested_capabilities,
            vec!["rust".to_string(), "axum".to_string()]
        );
    }

    #[test]
    fn validate_accepts_valid_submitter_wallet() {
        let v = validate_submit_params(&json!({
            "title": "T",
            "body": "x".repeat(50),
            "submitter_wallet": "WxdXw1f1kFMRu8HDf1SE6yjgeWyf3Vb4T63QXMs4yij"
        }))
        .unwrap();
        assert!(v.submitter_wallet.is_some());
    }

    #[test]
    fn validate_rejects_short_submitter_wallet() {
        let err = validate_submit_params(&json!({
            "title": "T",
            "body": "x".repeat(50),
            "submitter_wallet": "tooshort"
        }))
        .unwrap_err();
        assert!(err.contains("submitter_wallet"));
    }

    #[test]
    fn validate_rejects_submitter_wallet_with_invalid_chars() {
        // 0/O/I/l are excluded from base58
        let err = validate_submit_params(&json!({
            "title": "T",
            "body": "x".repeat(50),
            "submitter_wallet": "00000000000000000000000000000000"
        }))
        .unwrap_err();
        assert!(err.contains("submitter_wallet"));
    }

    #[test]
    fn validate_omits_submitter_wallet_when_absent() {
        let v = validate_submit_params(&json!({
            "title": "T",
            "body": "x".repeat(50)
        }))
        .unwrap();
        assert!(v.submitter_wallet.is_none());
    }

    #[test]
    fn validate_accepts_minimal_valid_input() {
        let v = validate_submit_params(&json!({
            "title": "Bug in /agents/register",
            "body": "When the wallet param is malformed the response is a 500 instead of a 400 with a useful error message. Repro: send POST /agents/register with wallet='not-a-pubkey'.",
        }))
        .unwrap();
        assert_eq!(v.title, "Bug in /agents/register");
        assert!(v.body.starts_with("When the wallet"));
        assert!(v.suggested_category.is_none());
        assert!(v.suggested_capabilities.is_empty());
    }
}
