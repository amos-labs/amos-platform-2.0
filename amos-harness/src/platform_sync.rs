//! Platform sync client: connects the harness back to the central AMOS platform.
//!
//! Three background loops run concurrently:
//! - **Heartbeat**: Reports health/version to platform every N seconds
//! - **Config sync**: Pulls configuration updates (agent defs, templates, schemas)
//! - **Activity reporter**: Pushes usage metrics and activity data

use amos_core::config::{DeploymentConfig, DeploymentMode, PlatformConfig};
use reqwest::Client;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Platform sync client manages communication between harness and platform.
pub struct PlatformSyncClient {
    http: Client,
    platform_url: String,
    api_key: Option<String>,
    deployment_mode: DeploymentMode,
    harness_version: String,
    config: PlatformConfig,
    /// Cached remote config (updated by sync loop).
    remote_config: Arc<RwLock<Option<RemoteConfig>>>,
}

/// Configuration pulled from the platform.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteConfig {
    /// Latest available harness version (for update checks).
    pub latest_version: Option<String>,
    /// Whether this harness instance is enabled on the platform.
    pub enabled: bool,
    /// Custom model overrides pushed from platform.
    pub model_overrides: Vec<ModelOverride>,
    /// Feature flags from platform.
    pub feature_flags: std::collections::HashMap<String, bool>,
    /// Last sync timestamp (ISO 8601).
    pub synced_at: String,
}

/// Model override pushed from platform (e.g., admin changes default model).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOverride {
    pub name: String,
    pub model_id: String,
    pub tier: u8,
}

/// Heartbeat payload sent to platform.
#[derive(Debug, Serialize)]
struct HeartbeatPayload {
    harness_version: String,
    deployment_mode: String,
    uptime_secs: u64,
    healthy: bool,
    timestamp: String,
}

/// Per-model token usage entry for metered billing.
#[derive(Debug, Clone, Serialize)]
pub struct ModelUsageEntry {
    pub model_id: String,
    pub tokens_input: u64,
    pub tokens_output: u64,
}

/// Activity report sent to platform.
#[derive(Debug, Serialize)]
struct ActivityReport {
    period_start: String,
    period_end: String,
    conversations: u64,
    messages: u64,
    tokens_input: u64,
    tokens_output: u64,
    tools_executed: u64,
    models_used: Vec<String>,
    /// Per-model token breakdown for metered billing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    model_usage: Vec<ModelUsageEntry>,
    timestamp: String,
    /// Harness ID for per-harness billing (from AMOS_HARNESS_ID env var).
    #[serde(skip_serializing_if = "Option::is_none")]
    harness_id: Option<String>,
}

/// Accumulated activity counters (reset after each report).
#[derive(Debug, Default)]
pub struct ActivityCounters {
    pub conversations: std::sync::atomic::AtomicU64,
    pub messages: std::sync::atomic::AtomicU64,
    pub tokens_input: std::sync::atomic::AtomicU64,
    pub tokens_output: std::sync::atomic::AtomicU64,
    pub tools_executed: std::sync::atomic::AtomicU64,
    /// Per-model token usage (model_id → (input, output)).
    pub model_usage: tokio::sync::RwLock<std::collections::HashMap<String, (u64, u64)>>,
}

impl ActivityCounters {
    /// Record token usage for a specific model.
    pub async fn record_model_usage(&self, model_id: &str, input: u64, output: u64) {
        let mut map = self.model_usage.write().await;
        let entry = map.entry(model_id.to_string()).or_insert((0, 0));
        entry.0 += input;
        entry.1 += output;
    }

    /// Drain and return per-model usage, resetting to empty.
    pub async fn drain_model_usage(&self) -> Vec<ModelUsageEntry> {
        let mut map = self.model_usage.write().await;
        let entries: Vec<ModelUsageEntry> = map
            .drain()
            .map(
                |(model_id, (tokens_input, tokens_output))| ModelUsageEntry {
                    model_id,
                    tokens_input,
                    tokens_output,
                },
            )
            .collect();
        entries
    }
}

impl PlatformSyncClient {
    /// Create a new platform sync client.
    pub fn new(platform_config: &PlatformConfig, deployment_config: &DeploymentConfig) -> Self {
        let api_key = platform_config
            .api_key
            .as_ref()
            .map(|s| s.expose_secret().to_string());

        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build HTTP client"),
            platform_url: platform_config.url.clone(),
            api_key,
            deployment_mode: deployment_config.mode,
            harness_version: deployment_config.harness_version.clone(),
            config: platform_config.clone(),
            remote_config: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the cached remote config (may be None if never synced).
    pub async fn remote_config(&self) -> Option<RemoteConfig> {
        self.remote_config.read().await.clone()
    }

    /// Check if a newer version is available.
    pub async fn update_available(&self) -> Option<String> {
        let config = self.remote_config.read().await;
        if let Some(ref cfg) = *config {
            if let Some(ref latest) = cfg.latest_version {
                if latest != &self.harness_version {
                    return Some(latest.clone());
                }
            }
        }
        None
    }

    /// Start all background sync loops. Returns a JoinHandle for the spawned task.
    pub fn start(self: Arc<Self>, counters: Arc<ActivityCounters>) -> tokio::task::JoinHandle<()> {
        let client = self.clone();
        tokio::spawn(async move {
            let start_time = std::time::Instant::now();

            info!(
                "Platform sync started: url={}, mode={:?}, heartbeat={}s, sync={}s, activity={}s",
                client.platform_url,
                client.deployment_mode,
                client.config.heartbeat_interval_secs,
                client.config.sync_interval_secs,
                client.config.activity_report_interval_secs,
            );

            // Run all three loops concurrently
            tokio::join!(
                client.heartbeat_loop(start_time),
                client.config_sync_loop(),
                client.activity_report_loop(counters),
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

    async fn heartbeat_loop(&self, start_time: std::time::Instant) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            self.config.heartbeat_interval_secs,
        ));

        loop {
            interval.tick().await;
            let payload = HeartbeatPayload {
                harness_version: self.harness_version.clone(),
                deployment_mode: format!("{:?}", self.deployment_mode).to_lowercase(),
                uptime_secs: start_time.elapsed().as_secs(),
                healthy: true,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };

            let url = format!("{}/api/v1/sync/heartbeat", self.platform_url);
            let mut req = self.http.post(&url).json(&payload);
            if let Some((key, value)) = self.auth_header() {
                req = req.header(&key, &value);
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    debug!("Heartbeat sent successfully");
                }
                Ok(resp) => {
                    warn!("Heartbeat returned {}: {}", resp.status(), resp.status());
                }
                Err(e) => {
                    debug!("Heartbeat failed (platform may be unreachable): {}", e);
                }
            }
        }
    }

    // ── Config Sync Loop ────────────────────────────────────────────────

    async fn config_sync_loop(&self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            self.config.sync_interval_secs,
        ));

        loop {
            interval.tick().await;
            let url = format!(
                "{}/api/v1/sync/config?version={}",
                self.platform_url, self.harness_version
            );
            let mut req = self.http.get(&url);
            if let Some((key, value)) = self.auth_header() {
                req = req.header(&key, &value);
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<RemoteConfig>().await {
                        Ok(config) => {
                            // Check for version update
                            if let Some(ref latest) = config.latest_version {
                                if latest != &self.harness_version {
                                    info!(
                                        "New harness version available: {} (current: {})",
                                        latest, self.harness_version
                                    );
                                }
                            }

                            let mut cached = self.remote_config.write().await;
                            *cached = Some(config);
                            debug!("Config sync completed");
                        }
                        Err(e) => {
                            warn!("Failed to parse remote config: {}", e);
                        }
                    }
                }
                Ok(resp) => {
                    debug!("Config sync returned {}", resp.status());
                }
                Err(e) => {
                    debug!("Config sync failed (platform may be unreachable): {}", e);
                }
            }
        }
    }

    // ── Activity Report Loop ────────────────────────────────────────────

    async fn activity_report_loop(&self, counters: Arc<ActivityCounters>) {
        if !self.config.telemetry_enabled {
            info!("Telemetry disabled, activity reporting will not run");
            // Just sleep forever so tokio::join! doesn't exit
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
            }
        }

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            self.config.activity_report_interval_secs,
        ));
        let mut last_report = chrono::Utc::now();

        loop {
            interval.tick().await;
            let now = chrono::Utc::now();

            // Swap counters to zero and capture values
            use std::sync::atomic::Ordering::Relaxed;
            let model_usage = counters.drain_model_usage().await;
            let models_used: Vec<String> = model_usage.iter().map(|e| e.model_id.clone()).collect();
            let report = ActivityReport {
                period_start: last_report.to_rfc3339(),
                period_end: now.to_rfc3339(),
                conversations: counters.conversations.swap(0, Relaxed),
                messages: counters.messages.swap(0, Relaxed),
                tokens_input: counters.tokens_input.swap(0, Relaxed),
                tokens_output: counters.tokens_output.swap(0, Relaxed),
                tools_executed: counters.tools_executed.swap(0, Relaxed),
                models_used,
                model_usage,
                timestamp: now.to_rfc3339(),
                harness_id: std::env::var("AMOS_HARNESS_ID").ok(),
            };

            // Skip empty reports
            if report.conversations == 0 && report.messages == 0 && report.tokens_input == 0 {
                debug!("Skipping empty activity report");
                last_report = now;
                continue;
            }

            let url = format!("{}/api/v1/sync/activity", self.platform_url);
            let mut req = self.http.post(&url).json(&report);
            if let Some((key, value)) = self.auth_header() {
                req = req.header(&key, &value);
            }

            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    debug!(
                        "Activity report sent: {} convs, {} msgs, {} tokens",
                        report.conversations,
                        report.messages,
                        report.tokens_input + report.tokens_output,
                    );
                }
                Ok(resp) => {
                    warn!("Activity report returned {}", resp.status());
                }
                Err(e) => {
                    debug!("Activity report failed: {}", e);
                }
            }

            last_report = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amos_core::config::{DeploymentConfig, PlatformConfig};

    #[test]
    fn test_sync_client_creation() {
        let platform_config = PlatformConfig::default();
        let deployment_config = DeploymentConfig::default();
        let client = PlatformSyncClient::new(&platform_config, &deployment_config);

        assert_eq!(client.platform_url, "http://localhost:4000");
        assert!(client.api_key.is_none());
        assert_eq!(client.deployment_mode, DeploymentMode::Managed);
    }

    #[test]
    fn test_remote_config_default() {
        let config = RemoteConfig::default();
        assert!(config.latest_version.is_none());
        assert!(!config.enabled);
        assert!(config.model_overrides.is_empty());
        assert!(config.feature_flags.is_empty());
    }

    #[test]
    fn test_activity_counters_default() {
        let counters = ActivityCounters::default();
        assert_eq!(
            counters
                .conversations
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        assert_eq!(
            counters.messages.load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }
}
