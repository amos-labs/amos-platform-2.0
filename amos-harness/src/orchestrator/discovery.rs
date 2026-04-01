//! Sibling harness discovery.
//!
//! Two modes:
//! - **Platform API**: GET {PLATFORM_URL}/api/v1/sync/siblings?tenant_id=...
//! - **Environment**: AMOS_SIBLING_HARNESSES=name:url,name:url

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Discovered sibling harness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiblingHarness {
    pub harness_id: String,
    pub name: Option<String>,
    pub role: String,
    pub packages: Vec<String>,
    pub internal_url: String,
    pub status: String,
    pub healthy: Option<bool>,
}

/// Response from the platform siblings endpoint.
#[derive(Debug, Deserialize)]
struct SiblingsApiResponse {
    siblings: Vec<SiblingApiEntry>,
}

#[derive(Debug, Deserialize)]
struct SiblingApiEntry {
    harness_id: String,
    name: Option<String>,
    harness_role: String,
    packages: serde_json::Value,
    internal_url: Option<String>,
    status: String,
    healthy: Option<bool>,
}

/// Manages discovery of sibling harnesses, cached and periodically refreshed.
pub struct Discovery {
    siblings: Arc<RwLock<Vec<SiblingHarness>>>,
    platform_url: Option<String>,
    tenant_id: Option<String>,
    harness_id: Option<String>,
}

impl Discovery {
    pub fn new(
        platform_url: Option<String>,
        tenant_id: Option<String>,
        harness_id: Option<String>,
    ) -> Self {
        let siblings = Arc::new(RwLock::new(Vec::new()));

        // Parse env-based siblings at construction time
        let env_siblings = parse_env_siblings();

        let discovery = Self {
            siblings: siblings.clone(),
            platform_url,
            tenant_id,
            harness_id,
        };

        // Seed with env-based siblings if any
        if !env_siblings.is_empty() {
            let siblings_clone = siblings.clone();
            tokio::spawn(async move {
                *siblings_clone.write().await = env_siblings;
            });
        }

        discovery
    }

    /// Get current list of known siblings.
    pub async fn get_siblings(&self) -> Vec<SiblingHarness> {
        self.siblings.read().await.clone()
    }

    /// Find a sibling by name (case-insensitive) or harness_id.
    pub async fn find_sibling(&self, name_or_id: &str) -> Option<SiblingHarness> {
        let siblings = self.siblings.read().await;
        let lower = name_or_id.to_lowercase();
        siblings
            .iter()
            .find(|s| {
                s.harness_id == name_or_id
                    || s.name.as_ref().map(|n| n.to_lowercase()) == Some(lower.clone())
            })
            .cloned()
    }

    /// Find siblings that have a specific package.
    pub async fn find_by_package(&self, package: &str) -> Vec<SiblingHarness> {
        let siblings = self.siblings.read().await;
        siblings
            .iter()
            .filter(|s| s.packages.contains(&package.to_string()))
            .cloned()
            .collect()
    }

    /// Refresh sibling list from platform API or env.
    pub async fn refresh(&self) {
        // Try platform API first
        if let (Some(url), Some(tenant_id)) = (&self.platform_url, &self.tenant_id) {
            match self.fetch_from_platform(url, tenant_id).await {
                Ok(siblings) => {
                    debug!(
                        count = siblings.len(),
                        "Refreshed siblings from platform API"
                    );
                    *self.siblings.write().await = siblings;
                    return;
                }
                Err(e) => {
                    warn!("Failed to fetch siblings from platform: {}", e);
                }
            }
        }

        // Fallback to env
        let env_siblings = parse_env_siblings();
        if !env_siblings.is_empty() {
            *self.siblings.write().await = env_siblings;
        }
    }

    async fn fetch_from_platform(
        &self,
        platform_url: &str,
        tenant_id: &str,
    ) -> Result<Vec<SiblingHarness>, String> {
        let client = reqwest::Client::new();

        let mut url = format!(
            "{}/api/v1/sync/siblings?tenant_id={}",
            platform_url.trim_end_matches('/'),
            tenant_id
        );

        if let Some(harness_id) = &self.harness_id {
            url.push_str(&format!("&exclude_harness_id={}", harness_id));
        }

        let resp = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Platform returned status {}", resp.status()));
        }

        let api_resp: SiblingsApiResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(api_resp
            .siblings
            .into_iter()
            .filter_map(|entry| {
                let internal_url = entry.internal_url?;
                let packages = match entry.packages {
                    serde_json::Value::Array(arr) => arr
                        .into_iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                    _ => vec![],
                };
                Some(SiblingHarness {
                    harness_id: entry.harness_id,
                    name: entry.name,
                    role: entry.harness_role,
                    packages,
                    internal_url,
                    status: entry.status,
                    healthy: entry.healthy,
                })
            })
            .collect())
    }
}

/// Parse AMOS_SIBLING_HARNESSES env var.
/// Format: `name:url,name:url` (e.g., `autoresearch:http://localhost:3001`)
fn parse_env_siblings() -> Vec<SiblingHarness> {
    let val = match std::env::var("AMOS_SIBLING_HARNESSES") {
        Ok(v) if !v.is_empty() => v,
        _ => return vec![],
    };

    val.split(',')
        .filter_map(|entry| {
            let parts: Vec<&str> = entry.trim().splitn(2, ':').collect();
            if parts.len() == 2 {
                // Handle name:http://... by finding the first colon-slash-slash
                let entry = entry.trim();
                let url_start = entry.find("http://").or_else(|| entry.find("https://"))?;
                let name = entry[..url_start].trim_end_matches(':').to_string();
                let url = entry[url_start..].to_string();
                Some(SiblingHarness {
                    harness_id: name.clone(),
                    name: Some(name.clone()),
                    role: "specialist".to_string(),
                    packages: vec![name],
                    internal_url: url,
                    status: "running".to_string(),
                    healthy: Some(true),
                })
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_siblings() {
        std::env::set_var(
            "AMOS_SIBLING_HARNESSES",
            "autoresearch:http://localhost:3001,education:http://localhost:3002",
        );
        let siblings = parse_env_siblings();
        assert_eq!(siblings.len(), 2);
        assert_eq!(siblings[0].name.as_deref(), Some("autoresearch"));
        assert_eq!(siblings[0].internal_url, "http://localhost:3001");
        assert_eq!(siblings[1].name.as_deref(), Some("education"));
        assert_eq!(siblings[1].internal_url, "http://localhost:3002");
        std::env::remove_var("AMOS_SIBLING_HARNESSES");
    }

    #[test]
    fn test_parse_env_siblings_empty() {
        std::env::remove_var("AMOS_SIBLING_HARNESSES");
        let siblings = parse_env_siblings();
        assert!(siblings.is_empty());
    }
}
