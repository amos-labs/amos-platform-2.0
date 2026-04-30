//! Harness ↔ relay agent registration bridge.
//!
//! The harness has a local `openclaw_agents` table keyed by serial `id`. The
//! relay has a `relay_agents` table keyed by `UUID`. Without a mapping, the
//! harness can't tell the relay "this agent is claiming/submitting" — the
//! relay rejects with 401 because no `relay_agents.id = <local-i32>` row
//! exists.
//!
//! [`ensure_registered_with_relay`] closes that gap. On first call for a
//! given local agent, it POSTs to `/api/v1/agents/register` with the
//! agent's wallet + capabilities, gets back a relay UUID, caches it in
//! `openclaw_agents.relay_agent_id`. Subsequent calls return the cached
//! UUID without a network round-trip.
//!
//! Surfaced by Finding #2 of bounty ea3466b2 (harness↔relay integration
//! bug report).

use amos_core::{AmosError, AppConfig, Result};
use secrecy::ExposeSecret;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct AgentResponse {
    id: Uuid,
}

/// Resolve the relay-side UUID for a local openclaw_agents row, registering
/// with the relay on the first call.
///
/// Returns `Ok(uuid)` once the agent is known to the relay. Returns
/// `Err(AmosError::Validation)` if the local agent has no `wallet_address`
/// (the relay requires one to register).
pub async fn ensure_registered_with_relay(
    db: &PgPool,
    config: &Arc<AppConfig>,
    local_agent_id: i32,
) -> Result<Uuid> {
    // 1. Check the local cache first.
    let row = sqlx::query_as::<_, (Option<Uuid>, Option<String>, String, String, JsonValue)>(
        r#"SELECT relay_agent_id, wallet_address, name, display_name, capabilities
           FROM openclaw_agents WHERE id = $1"#,
    )
    .bind(local_agent_id)
    .fetch_optional(db)
    .await
    .map_err(|e| AmosError::Internal(format!("openclaw_agents lookup: {e}")))?
    .ok_or_else(|| {
        AmosError::Validation(format!("openclaw_agents row {local_agent_id} not found"))
    })?;

    let (cached, wallet_address, name, display_name, caps_json) = row;

    if let Some(uuid) = cached {
        debug!(local_agent_id, %uuid, "relay agent uuid: cache hit");
        return Ok(uuid);
    }

    let wallet = wallet_address.ok_or_else(|| {
        AmosError::Validation(format!(
            "openclaw_agents row {local_agent_id} has no wallet_address — \
             cannot register with the relay (wallet is required for trust + \
             on-chain settlement)"
        ))
    })?;

    let capabilities: Vec<String> = match caps_json {
        JsonValue::Array(arr) => arr
            .into_iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    };

    // 2. Cache miss — POST to relay /api/v1/agents/register.
    let url = format!(
        "{}/api/v1/agents/register",
        config.relay.url.trim_end_matches('/')
    );
    let api_key = config
        .relay
        .api_key
        .as_ref()
        .ok_or_else(|| AmosError::Internal("relay.api_key not set".into()))?;

    let payload = json!({
        "name": name,
        "display_name": display_name,
        // Harness-resident agents don't have an externally-reachable
        // endpoint; placeholder string keeps the relay's required-field
        // check happy without lying about reachability.
        "endpoint_url": format!("harness://local/{local_agent_id}"),
        "capabilities": capabilities,
        "wallet_address": wallet,
    });

    info!(
        local_agent_id,
        %wallet,
        "relay agent uuid: cache miss → registering with relay"
    );

    let resp = reqwest::Client::new()
        .post(&url)
        .bearer_auth(api_key.expose_secret())
        .json(&payload)
        .send()
        .await
        .map_err(|e| AmosError::Internal(format!("relay register POST failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        // 409 Conflict typically means "already registered with this wallet".
        // The relay's response in that case still carries the existing
        // agent's UUID, but we don't know the body shape — surface clearly.
        return Err(AmosError::Internal(format!(
            "relay register {url} returned {status}: {}",
            body.chars().take(300).collect::<String>()
        )));
    }

    let agent: AgentResponse = resp
        .json()
        .await
        .map_err(|e| AmosError::Internal(format!("relay register response decode: {e}")))?;

    // 3. Persist the UUID so subsequent calls hit the cache.
    sqlx::query("UPDATE openclaw_agents SET relay_agent_id = $1, updated_at = NOW() WHERE id = $2")
        .bind(agent.id)
        .bind(local_agent_id)
        .execute(db)
        .await
        .map_err(|e| {
            // Non-fatal — registration succeeded even if cache write
            // didn't. Next call will register again, which 409s, which we
            // currently treat as fatal — so log loudly.
            warn!(
                local_agent_id,
                relay_uuid = %agent.id,
                error = %e,
                "registered with relay but failed to cache UUID locally"
            );
            AmosError::Internal(format!("openclaw_agents UUID cache write failed: {e}"))
        })?;

    info!(local_agent_id, relay_uuid = %agent.id, "relay agent uuid: registered + cached");
    Ok(agent.id)
}
