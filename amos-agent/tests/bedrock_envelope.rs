//! Bedrock envelope contract — Phase 2 of the test harness plan.
//!
//! Snapshot-tests the exact JSON body our Bedrock request builder produces.
//! Targets regression classes that have actually hit prod:
//! - toolSpec outer wrapper (forgot → "ToolSpec field missing")
//! - inputSchema.json inner envelope (forgot → "inputSchema is empty")
//! - inferenceConfig keys (drift fails 400 before the stream starts)
//! - maxTokens tiering (haiku vs. sonnet/opus — latency budget)
//!
//! Fixtures live in `tests/fixtures/`. Regenerate them consciously — set
//! `REGEN_FIXTURES=1` to overwrite, eyeball the diff, commit. This is the
//! intentional human gate: a change to the envelope must be an explicit act.
//!
//! Forward-looking: when Opus 4.7 extended thinking lands (needs
//! `thinking.type: "adaptive"` per the plan), add a fixture here.

use amos_agent::bedrock::BedrockProvider;
use amos_core::types::{ContentBlock, Message, Role};
use chrono::{TimeZone, Utc};
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────
// Fixture helpers
// ─────────────────────────────────────────────────────────────────────────

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// Compare `actual` to the JSON fixture at `name`. With `REGEN_FIXTURES=1`
/// set, overwrite the fixture with pretty-printed `actual` and pass —
/// intended for conscious regeneration after an intentional envelope change.
fn assert_matches_fixture(name: &str, actual: &Value) {
    let path = fixture_path(name);
    let pretty = serde_json::to_string_pretty(actual).expect("serialize actual");

    if std::env::var("REGEN_FIXTURES").is_ok() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir -p fixtures");
        }
        std::fs::write(&path, format!("{pretty}\n")).expect("write fixture");
        eprintln!("regenerated {}", path.display());
        return;
    }

    let expected_raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "fixture {} unreadable ({e}). \
             Generate with: REGEN_FIXTURES=1 cargo test --test bedrock_envelope -p amos-agent",
            path.display()
        )
    });
    let expected: Value = serde_json::from_str(&expected_raw).expect("fixture must be valid JSON");
    assert_eq!(*actual, expected, "envelope drift for {name}");
}

fn provider() -> BedrockProvider {
    BedrockProvider::new(
        "us-east-1".into(),
        "test_key".into(),
        "test_secret".into(),
        None,
    )
}

fn fixed_ts() -> chrono::DateTime<Utc> {
    // Deterministic timestamp — build_request_body doesn't serialize it,
    // but keep the input stable so any future serialization would be stable too.
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
}

fn user_text(text: &str) -> Message {
    Message {
        role: Role::User,
        content: vec![ContentBlock::Text { text: text.into() }],
        tool_use_id: None,
        timestamp: fixed_ts(),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Case 1 — haiku, no tools, minimal body.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn haiku_no_tools() {
    let body = provider()
        .build_request_body(
            "You are helpful.",
            &[user_text("What's the weather?")],
            &[],
            "anthropic.claude-haiku-4-5",
        )
        .expect("build_request_body");

    // Haiku tier: 4096 maxTokens. Flipping this to 16384 silently doubles
    // the per-request latency budget on the cheap/fast model path.
    assert_eq!(body["inferenceConfig"]["maxTokens"], 4096);
    assert!(
        body.get("toolConfig").is_none(),
        "no tools requested → toolConfig key must be absent, not empty"
    );
    assert_matches_fixture("bedrock_envelope_haiku_no_tools.json", &body);
}

// ─────────────────────────────────────────────────────────────────────────
// Case 2 — agent-local tool shape gets normalized for Bedrock.
// Regression class: toolSpec outer wrapper + inputSchema.json envelope.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn haiku_agent_local_tools_get_wrapped() {
    let tools = vec![json!({
        "name": "think",
        "description": "internal reasoning",
        "inputSchema": {
            "type": "object",
            "properties": { "thought": { "type": "string" } }
        }
    })];

    let body = provider()
        .build_request_body(
            "You are helpful.",
            &[user_text("Think about this.")],
            &tools,
            "us.anthropic.claude-haiku-4-5-20251001-v1:0",
        )
        .expect("build_request_body");

    let spec = &body["toolConfig"]["tools"][0]["toolSpec"];
    assert_eq!(spec["name"], "think");
    // The raw schema must be re-wrapped as {json: <schema>} — omitting the
    // envelope yields a 400 "inputSchema is empty" at Bedrock.
    assert_eq!(spec["inputSchema"]["json"]["type"], "object");

    assert_matches_fixture("bedrock_envelope_haiku_agent_local_tools.json", &body);
}

// ─────────────────────────────────────────────────────────────────────────
// Case 3 — harness-enveloped tool shape passes through; no double-wrap;
// empty system prompt drops the `system` field entirely.
// ─────────────────────────────────────────────────────────────────────────

#[test]
fn sonnet_harness_preenveloped_tools_pass_through() {
    let tools = vec![json!({
        "toolSpec": {
            "name": "create_landing_page",
            "description": "Creates a landing page in one call.",
            "inputSchema": {
                "json": {
                    "type": "object",
                    "properties": {
                        "slug": {"type": "string"},
                        "html_content": {"type": "string"}
                    },
                    "required": ["slug", "html_content"]
                }
            }
        }
    })];

    let body = provider()
        .build_request_body(
            "",
            &[user_text("Build a landing page.")],
            &tools,
            "us.anthropic.claude-sonnet-4-6-20250929-v1:0",
        )
        .expect("build_request_body");

    assert_eq!(
        body["inferenceConfig"]["maxTokens"], 16384,
        "sonnet/opus tier must get the 16384 maxTokens bump"
    );
    let spec = &body["toolConfig"]["tools"][0]["toolSpec"];
    assert!(
        spec["inputSchema"]["json"].get("json").is_none(),
        "normalizer must not double-wrap → inputSchema.json.json is invalid"
    );
    assert!(
        body.get("system").is_none(),
        "empty system prompt must drop the `system` key entirely"
    );

    assert_matches_fixture("bedrock_envelope_sonnet_harness_tools.json", &body);
}
