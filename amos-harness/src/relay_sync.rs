//! Relay sync client: connects the harness to the AMOS Network Relay (marketplace layer).
//!
//! Three background loops run concurrently:
//! - **Heartbeat**: Reports health/version to relay every N seconds
//! - **Bounty sync**: Pulls available bounties from marketplace
//! - **Reputation reporter**: Pushes agent performance and completion data

use amos_core::config::{DeploymentConfig, RelayConfig};
use reqwest::Client;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Relay sync client manages communication between harness and relay.
pub struct RelaySyncClient {
    http: Client,
    relay_url: String,
    api_key: Option<String>,
    harness_id: String,
    harness_version: String,
    config: RelayConfig,
    /// Cached bounties (updated by sync loop).
    bounties: Arc<RwLock<Vec<RelayBounty>>>,
}

/// Bounty pulled from the relay marketplace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayBounty {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub reward_tokens: u64,
    pub deadline: String,
    pub required_capabilities: Vec<String>,
}

/// Heartbeat payload sent to relay.
#[derive(Debug, Serialize)]
struct HeartbeatPayload {
    harness_id: String,
    harness_version: String,
    status: String,
    capabilities: Vec<String>,
    agent_count: u32,
    timestamp: String,
}

/// Reputation report sent to relay.
#[derive(Debug, Serialize)]
struct ReputationReport {
    harness_id: String,
    agents: Vec<AgentReputation>,
    timestamp: String,
}

/// Agent reputation data.
#[derive(Debug, Serialize)]
struct AgentReputation {
    agent_id: Uuid,
    bounties_completed: u32,
    avg_quality_score: f64,
    uptime_pct: f64,
}

impl RelaySyncClient {
    /// Create a new relay sync client.
    pub fn new(relay_config: &RelayConfig, deployment_config: &DeploymentConfig) -> Self {
        let api_key = relay_config
            .api_key
            .as_ref()
            .map(|s| s.expose_secret().to_string());

        // Generate a stable harness ID from env var or use a UUID
        let harness_id = std::env::var("HARNESS_ID")
            .unwrap_or_else(|_| format!("harness-{}", Uuid::new_v4()));

        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build HTTP client"),
            relay_url: relay_config.url.clone(),
            api_key,
            harness_id,
            harness_version: deployment_config.harness_version.clone(),
            config: relay_config.clone(),
            bounties: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the cached available bounties.
    pub async fn available_bounties(&self) -> Vec<RelayBounty> {
        self.bounties.read().await.clone()
    }

    /// Start all background sync loops. Returns a JoinHandle for the spawned task.
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let client = self.clone();
        tokio::spawn(async move {
            if !client.config.enabled {
                info!("Relay integration disabled");
                // Just sleep forever so the task doesn't exit
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                }
            }

            info!(
                "Relay sync started: url={}, heartbeat={}s, bounty_sync={}s, reputation={}s",
                client.relay_url,
                client.config.heartbeat_interval_secs,
                client.config.bounty_sync_interval_secs,
                client.config.reputation_report_interval_secs,
            );

            // Run all three loops concurrently
            tokio::join!(
                client.heartbeat_loop(),
                client.bounty_sync_loop(),
                client.reputation_report_loop(),
            );
        })
    }

    /// Add authorization header if API key is configured.
    fn auth_header(&self) -> Option<(String, String)> {
        self.api_key
            .as_ref()
            .map(|key| ("Authorization".to_string(), format!("Bearer {}", key)))
    }

    // ── Heartbeat Loop ──────────────────────────────────────────────────

    async fn heartbeat_loop(&self) {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.config.heartbeat_interval_secs),
        );

        loop {
            interval.tick().await;
            let payload = HeartbeatPayload {
                harness_id: self.harness_id.clone(),
                harness_version: self.harness_version.clone(),
                status: "healthy".to_string(),
                capabilities: vec![
                    "document_processing".to_string(),
                    "image_generation".to_string(),
                    "web_search".to_string(),
                    "code_execution".to_string(),
                ],
                agent_count: 0, // TODO: track active agent count
                timestamp: chrono::Utc::now().to_rfc3339(),
            };

            let url = format!("{}/api/v1/harnesses/heartbeat", self.relay_url);
            let mut req = self.http.post(&url).json(&payload);
            if let Some((key, value)) = self.auth_header() {
                req = req.header(&key, &value);
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    debug!("Relay heartbeat sent successfully");
                }
                Ok(resp) => {
                    warn!("Relay heartbeat returned {}: {}", resp.status(), resp.status());
                }
                Err(e) => {
                    debug!("Relay heartbeat failed (relay may be unreachable): {}", e);
                }
            }
        }
    }

    // ── Bounty Sync Loop ────────────────────────────────────────────────

    async fn bounty_sync_loop(&self) {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.config.bounty_sync_interval_secs),
        );

        loop {
            interval.tick().await;
            let url = format!("{}/api/v1/bounties?status=open", self.relay_url);
            let mut req = self.http.get(&url);
            if let Some((key, value)) = self.auth_header() {
                req = req.header(&key, &value);
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<Vec<RelayBounty>>().await {
                        Ok(bounties) => {
                            let count = bounties.len();
                            let mut cached = self.bounties.write().await;
                            *cached = bounties;
                            debug!("Bounty sync completed: {} bounties available", count);
                        }
                        Err(e) => {
                            warn!("Failed to parse bounties: {}", e);
                        }
                    }
                }
                Ok(resp) => {
                    debug!("Bounty sync returned {}", resp.status());
                }
                Err(e) => {
                    debug!("Bounty sync failed (relay may be unreachable): {}", e);
                }
            }
        }
    }

    // ── Reputation Report Loop ──────────────────────────────────────────

    async fn reputation_report_loop(&self) {
        let mut interval = tokio::time::interval(
            std::time::Duration::from_secs(self.config.reputation_report_interval_secs),
        );

        loop {
            interval.tick().await;

            // TODO: gather actual agent reputation data from agent manager
            let report = ReputationReport {
                harness_id: self.harness_id.clone(),
                agents: vec![
                    // Placeholder - will be populated from real agent data
                ],
                timestamp: chrono::Utc::now().to_rfc3339(),
            };

            // Skip empty reports
            if report.agents.is_empty() {
                debug!("Skipping empty reputation report");
                continue;
            }

            let url = format!("{}/api/v1/reputation/report", self.relay_url);
            let mut req = self.http.post(&url).json(&report);
            if let Some((key, value)) = self.auth_header() {
                req = req.header(&key, &value);
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    debug!("Reputation report sent: {} agents", report.agents.len());
                }
                Ok(resp) => {
                    warn!("Reputation report returned {}", resp.status());
                }
                Err(e) => {
                    debug!("Reputation report failed: {}", e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amos_core::config::{DeploymentConfig, RelayConfig};

    #[test]
    fn test_relay_sync_client_creation() {
        let relay_config = RelayConfig::default();
        let deployment_config = DeploymentConfig::default();
        let client = RelaySyncClient::new(&relay_config, &deployment_config);

        assert_eq!(client.relay_url, "http://localhost:4100");
        assert!(client.api_key.is_none());
        assert!(!client.harness_id.is_empty());
    }

    #[tokio::test]
    async fn test_relay_bounty_cache_default() {
        let relay_config = RelayConfig::default();
        let deployment_config = DeploymentConfig::default();
        let client = RelaySyncClient::new(&relay_config, &deployment_config);

        let bounties = client.available_bounties().await;
        assert!(bounties.is_empty());
    }

    #[tokio::test]
    async fn test_relay_sync_disabled() {
        let mut relay_config = RelayConfig::default();
        relay_config.enabled = false;
        let deployment_config = DeploymentConfig::default();
        let client = Arc::new(RelaySyncClient::new(&relay_config, &deployment_config));

        // Start should return immediately when disabled
        let handle = client.start();

        // Give it a moment to initialize
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Should still be running (sleeping forever)
        assert!(!handle.is_finished());

        // Clean up
        handle.abort();
    }
}
