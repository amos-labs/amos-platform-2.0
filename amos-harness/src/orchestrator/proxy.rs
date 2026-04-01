//! HTTP proxy for calling sibling harness APIs.
//!
//! Uses the EAP (External Agent Protocol) endpoints already present on every
//! harness to execute tools, list capabilities, and check health.

use super::discovery::{Discovery, SiblingHarness};
use amos_core::{tools::ToolResult, AppConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tracing::debug;

/// HTTP proxy for inter-harness communication.
pub struct HarnessProxy {
    client: reqwest::Client,
    discovery: Discovery,
}

/// Status of a sibling harness.
#[derive(Debug, Serialize)]
pub struct HarnessStatus {
    pub harness_id: String,
    pub name: Option<String>,
    pub role: String,
    pub packages: Vec<String>,
    pub status: String,
    pub healthy: Option<bool>,
    pub tools: Vec<String>,
}

/// EAP tool execution request.
#[derive(Serialize)]
struct EapToolExecuteRequest {
    tool_name: String,
    input: JsonValue,
}

/// EAP tool execution response.
#[derive(Deserialize)]
struct EapToolExecuteResponse {
    content: String,
    is_error: bool,
    #[allow(dead_code)]
    duration_ms: Option<u64>,
    metadata: Option<JsonValue>,
}

/// Harness info response from /api/v1/harness/info.
#[derive(Debug, Deserialize)]
pub struct HarnessInfo {
    pub harness_id: Option<String>,
    pub role: Option<String>,
    pub packages: Option<Vec<String>>,
    pub tools: Option<Vec<String>>,
    pub status: Option<String>,
    pub uptime_secs: Option<u64>,
}

/// Packages listing response from /api/v1/packages.
#[derive(Debug, Deserialize)]
struct PackagesResponse {
    packages: Vec<PackageEntry>,
}

#[derive(Debug, Deserialize)]
struct PackageEntry {
    #[allow(dead_code)]
    name: String,
    tool_names: Option<Vec<String>>,
}

impl HarnessProxy {
    pub fn new(_config: Arc<AppConfig>) -> Self {
        let platform_url = std::env::var("AMOS_PLATFORM_URL")
            .or_else(|_| std::env::var("AMOS__PLATFORM__URL"))
            .ok();
        let tenant_id = std::env::var("CUSTOMER_ID").ok();
        let harness_id = std::env::var("AMOS_HARNESS_ID").ok();

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let discovery = Discovery::new(platform_url, tenant_id, harness_id);

        Self { client, discovery }
    }

    /// Refresh sibling discovery.
    pub async fn refresh(&self) {
        self.discovery.refresh().await;
    }

    /// Get all known siblings.
    pub async fn get_siblings(&self) -> Vec<SiblingHarness> {
        self.discovery.get_siblings().await
    }

    /// Find a sibling by name or ID.
    pub async fn find_sibling(&self, name_or_id: &str) -> Option<SiblingHarness> {
        self.discovery.find_sibling(name_or_id).await
    }

    /// Find siblings that have a specific package.
    pub async fn find_by_package(&self, package: &str) -> Vec<SiblingHarness> {
        self.discovery.find_by_package(package).await
    }

    /// Execute a tool on a sibling harness by name or ID.
    pub async fn execute_tool(
        &self,
        harness_name_or_id: &str,
        tool_name: &str,
        params: JsonValue,
    ) -> Result<ToolResult, String> {
        let sibling = self
            .find_sibling(harness_name_or_id)
            .await
            .ok_or_else(|| format!("Harness '{}' not found", harness_name_or_id))?;

        let url = format!(
            "{}/api/v1/agents/internal/tools/execute",
            sibling.internal_url.trim_end_matches('/')
        );

        let req_body = EapToolExecuteRequest {
            tool_name: tool_name.to_string(),
            input: params,
        };

        debug!(harness = %sibling.internal_url, tool = tool_name, "Executing tool on sibling");

        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| format!("Failed to reach harness '{}': {}", harness_name_or_id, e))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Harness '{}' returned status {}",
                harness_name_or_id,
                resp.status()
            ));
        }

        let eap_resp: EapToolExecuteResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if eap_resp.is_error {
            Ok(ToolResult::error(eap_resp.content))
        } else {
            // Try to parse content as JSON, fall back to string
            let data = serde_json::from_str::<JsonValue>(&eap_resp.content)
                .unwrap_or(JsonValue::String(eap_resp.content));
            Ok(ToolResult {
                success: true,
                data: Some(data),
                error: None,
                metadata: eap_resp.metadata,
            })
        }
    }

    /// List tools available on a sibling harness.
    pub async fn list_tools(&self, harness_name_or_id: &str) -> Result<Vec<String>, String> {
        let sibling = self
            .find_sibling(harness_name_or_id)
            .await
            .ok_or_else(|| format!("Harness '{}' not found", harness_name_or_id))?;

        // Try /api/v1/harness/info first (has tool list)
        let info_url = format!(
            "{}/api/v1/harness/info",
            sibling.internal_url.trim_end_matches('/')
        );

        match self.client.get(&info_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(info) = resp.json::<HarnessInfo>().await {
                    if let Some(tools) = info.tools {
                        return Ok(tools);
                    }
                }
            }
            _ => {}
        }

        // Fallback to /api/v1/packages
        let pkg_url = format!(
            "{}/api/v1/packages",
            sibling.internal_url.trim_end_matches('/')
        );

        match self.client.get(&pkg_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(pkg_resp) = resp.json::<PackagesResponse>().await {
                    let tools: Vec<String> = pkg_resp
                        .packages
                        .into_iter()
                        .flat_map(|p| p.tool_names.unwrap_or_default())
                        .collect();
                    return Ok(tools);
                }
            }
            _ => {}
        }

        Ok(vec![])
    }

    /// Get status/health of a sibling harness.
    pub async fn get_status(&self, harness_name_or_id: &str) -> Result<HarnessStatus, String> {
        let sibling = self
            .find_sibling(harness_name_or_id)
            .await
            .ok_or_else(|| format!("Harness '{}' not found", harness_name_or_id))?;

        let health_url = format!("{}/health", sibling.internal_url.trim_end_matches('/'));

        let healthy = match self.client.get(&health_url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        };

        let tools = self
            .list_tools(harness_name_or_id)
            .await
            .unwrap_or_default();

        Ok(HarnessStatus {
            harness_id: sibling.harness_id,
            name: sibling.name,
            role: sibling.role,
            packages: sibling.packages,
            status: if healthy {
                "running".to_string()
            } else {
                "unreachable".to_string()
            },
            healthy: Some(healthy),
            tools,
        })
    }

    /// Submit an async task to a sibling harness's agent.
    /// Uses the EAP task submission endpoint.
    pub async fn submit_task(
        &self,
        harness_name_or_id: &str,
        task_description: &str,
    ) -> Result<String, String> {
        let sibling = self
            .find_sibling(harness_name_or_id)
            .await
            .ok_or_else(|| format!("Harness '{}' not found", harness_name_or_id))?;

        // Use the agent chat endpoint to submit work
        let url = format!(
            "{}/api/v1/agent/chat",
            sibling.internal_url.trim_end_matches('/')
        );

        let body = serde_json::json!({
            "message": task_description,
            "async": true,
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to submit task to '{}': {}", harness_name_or_id, e))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Harness '{}' returned status {} for task submission",
                harness_name_or_id,
                resp.status()
            ));
        }

        // Return a task reference
        Ok(format!(
            "task-{}-{}",
            sibling.harness_id,
            uuid::Uuid::new_v4()
        ))
    }
}
