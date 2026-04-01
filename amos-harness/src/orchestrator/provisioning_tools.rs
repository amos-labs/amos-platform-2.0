//! Provisioning tools for user-friendly specialist management.
//!
//! These tools let the agent activate/deactivate specialist assistants on
//! behalf of users, mapping friendly names like "Research Assistant" to the
//! underlying package slugs and platform provisioning API.

use super::proxy::HarnessProxy;
use amos_core::{
    tools::{Tool, ToolCategory, ToolResult},
    Result,
};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::sync::Arc;

/// A specialist type that can be activated.
#[derive(Debug, Clone, Serialize)]
pub struct SpecialistEntry {
    pub slug: &'static str,
    pub friendly_name: &'static str,
    pub description: &'static str,
    pub icon_hint: &'static str,
}

/// Static catalog of available specialist types.
pub static SPECIALIST_CATALOG: &[SpecialistEntry] = &[
    SpecialistEntry {
        slug: "autoresearch",
        friendly_name: "Research Assistant",
        description:
            "Market analysis, Darwinian optimization, trading strategies, and competitive research",
        icon_hint: "search",
    },
    SpecialistEntry {
        slug: "education",
        friendly_name: "Education & Training",
        description:
            "Course creation, learning content generation, SCORM support, and training programs",
        icon_hint: "graduation-cap",
    },
];

/// Look up a catalog entry by slug or friendly name (case-insensitive).
pub fn find_catalog_entry(name_or_slug: &str) -> Option<&'static SpecialistEntry> {
    let lower = name_or_slug.to_lowercase();
    SPECIALIST_CATALOG.iter().find(|e| {
        e.slug == lower
            || e.friendly_name.to_lowercase() == lower
            || e.friendly_name.to_lowercase().contains(&lower)
    })
}

// ── list_available_specialists ──────────────────────────────────────────

pub struct ListAvailableSpecialistsTool {
    proxy: Arc<HarnessProxy>,
}

impl ListAvailableSpecialistsTool {
    pub fn new(proxy: Arc<HarnessProxy>) -> Self {
        Self { proxy }
    }
}

#[async_trait]
impl Tool for ListAvailableSpecialistsTool {
    fn name(&self) -> &str {
        "list_available_specialists"
    }

    fn description(&self) -> &str {
        "List all specialist assistant types that can be activated, along with which ones are already running. Use this to see what capabilities are available."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Orchestrator
    }

    async fn execute(&self, _params: JsonValue) -> Result<ToolResult> {
        // Refresh discovery to get current state
        self.proxy.refresh().await;
        let siblings = self.proxy.get_siblings().await;

        let mut available = Vec::new();
        let mut active = Vec::new();

        for entry in SPECIALIST_CATALOG {
            // Check if any running sibling has this package
            let running = siblings
                .iter()
                .find(|s| s.packages.contains(&entry.slug.to_string()));

            if let Some(sibling) = running {
                active.push(serde_json::json!({
                    "friendly_name": entry.friendly_name,
                    "slug": entry.slug,
                    "description": entry.description,
                    "icon_hint": entry.icon_hint,
                    "status": "active",
                    "harness_id": sibling.harness_id,
                    "healthy": sibling.healthy.unwrap_or(false),
                }));
            } else {
                available.push(serde_json::json!({
                    "friendly_name": entry.friendly_name,
                    "slug": entry.slug,
                    "description": entry.description,
                    "icon_hint": entry.icon_hint,
                    "status": "available",
                }));
            }
        }

        Ok(ToolResult::success(serde_json::json!({
            "active_specialists": active,
            "available_specialists": available,
        })))
    }
}

// ── activate_specialist ─────────────────────────────────────────────────

pub struct ActivateSpecialistTool {
    proxy: Arc<HarnessProxy>,
}

impl ActivateSpecialistTool {
    pub fn new(proxy: Arc<HarnessProxy>) -> Self {
        Self { proxy }
    }
}

#[async_trait]
impl Tool for ActivateSpecialistTool {
    fn name(&self) -> &str {
        "activate_specialist"
    }

    fn description(&self) -> &str {
        "Activate a specialist assistant to extend your capabilities. Use the slug from list_available_specialists (e.g., 'autoresearch' for Research Assistant)."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "specialist": {
                    "type": "string",
                    "description": "The slug or friendly name of the specialist to activate (e.g., 'autoresearch' or 'Research Assistant')"
                }
            },
            "required": ["specialist"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Orchestrator
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let specialist = params
            .get("specialist")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("'specialist' is required".into()))?;

        // Resolve friendly name to catalog entry
        let entry = match find_catalog_entry(specialist) {
            Some(e) => e,
            None => {
                let available: Vec<&str> =
                    SPECIALIST_CATALOG.iter().map(|e| e.friendly_name).collect();
                return Ok(ToolResult::error(format!(
                    "Unknown specialist '{}'. Available specialists: {}",
                    specialist,
                    available.join(", ")
                )));
            }
        };

        // Check if already active
        self.proxy.refresh().await;
        let siblings = self.proxy.get_siblings().await;
        if siblings
            .iter()
            .any(|s| s.packages.contains(&entry.slug.to_string()))
        {
            return Ok(ToolResult::success(serde_json::json!({
                "status": "already_active",
                "friendly_name": entry.friendly_name,
                "message": format!("{} is already active and ready to use.", entry.friendly_name),
            })));
        }

        // Call platform provisioning API
        let platform_url = match std::env::var("AMOS_PLATFORM_URL")
            .or_else(|_| std::env::var("AMOS__PLATFORM__URL"))
        {
            Ok(url) => url,
            Err(_) => {
                return Ok(ToolResult::error(
                    "Specialist assistants require the AMOS platform to be configured. \
                     This feature is available with the full AMOS platform deployment."
                        .to_string(),
                ));
            }
        };

        let tenant_id = match std::env::var("CUSTOMER_ID") {
            Ok(id) => id,
            Err(_) => {
                return Ok(ToolResult::error(
                    "Unable to activate specialist: tenant configuration is missing.".to_string(),
                ));
            }
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let url = format!(
            "{}/api/v1/tenants/{}/harnesses",
            platform_url.trim_end_matches('/'),
            tenant_id
        );

        let body = serde_json::json!({
            "packages": [entry.slug],
            "role": "specialist",
            "name": entry.friendly_name,
        });

        match client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                // Trigger a discovery refresh so new sibling shows up
                self.proxy.refresh().await;

                Ok(ToolResult::success(serde_json::json!({
                    "status": "activating",
                    "friendly_name": entry.friendly_name,
                    "slug": entry.slug,
                    "message": format!(
                        "{} is being activated. It will be ready in about 30 seconds.",
                        entry.friendly_name
                    ),
                })))
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    status = %status,
                    body = %body,
                    "Platform rejected specialist activation"
                );
                Ok(ToolResult::error(format!(
                    "Failed to activate {}: the platform returned an error. Please try again later.",
                    entry.friendly_name
                )))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to reach platform for specialist activation");
                Ok(ToolResult::error(
                    "Unable to reach the platform service. Specialist activation is temporarily unavailable."
                        .to_string(),
                ))
            }
        }
    }
}

// ── deactivate_specialist ───────────────────────────────────────────────

pub struct DeactivateSpecialistTool {
    proxy: Arc<HarnessProxy>,
}

impl DeactivateSpecialistTool {
    pub fn new(proxy: Arc<HarnessProxy>) -> Self {
        Self { proxy }
    }
}

#[async_trait]
impl Tool for DeactivateSpecialistTool {
    fn name(&self) -> &str {
        "deactivate_specialist"
    }

    fn description(&self) -> &str {
        "Deactivate a running specialist assistant that is no longer needed. Use the slug or friendly name."
    }

    fn parameters_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "specialist": {
                    "type": "string",
                    "description": "The slug or friendly name of the specialist to deactivate (e.g., 'autoresearch' or 'Research Assistant')"
                }
            },
            "required": ["specialist"]
        })
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Orchestrator
    }

    async fn execute(&self, params: JsonValue) -> Result<ToolResult> {
        let specialist = params
            .get("specialist")
            .and_then(|v| v.as_str())
            .ok_or_else(|| amos_core::AmosError::Validation("'specialist' is required".into()))?;

        // Resolve friendly name to catalog entry
        let entry = match find_catalog_entry(specialist) {
            Some(e) => e,
            None => {
                return Ok(ToolResult::error(format!(
                    "Unknown specialist '{}'.",
                    specialist
                )));
            }
        };

        // Find the running sibling with this package
        self.proxy.refresh().await;
        let siblings = self.proxy.get_siblings().await;
        let sibling = match siblings
            .iter()
            .find(|s| s.packages.contains(&entry.slug.to_string()))
        {
            Some(s) => s.clone(),
            None => {
                return Ok(ToolResult::success(serde_json::json!({
                    "status": "not_active",
                    "friendly_name": entry.friendly_name,
                    "message": format!("{} is not currently active.", entry.friendly_name),
                })));
            }
        };

        // Call platform deprovisioning API
        let platform_url = match std::env::var("AMOS_PLATFORM_URL")
            .or_else(|_| std::env::var("AMOS__PLATFORM__URL"))
        {
            Ok(url) => url,
            Err(_) => {
                return Ok(ToolResult::error(
                    "Platform configuration is missing. Cannot deactivate specialist.".to_string(),
                ));
            }
        };

        let tenant_id = match std::env::var("CUSTOMER_ID") {
            Ok(id) => id,
            Err(_) => {
                return Ok(ToolResult::error(
                    "Tenant configuration is missing. Cannot deactivate specialist.".to_string(),
                ));
            }
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let url = format!(
            "{}/api/v1/tenants/{}/harnesses/{}",
            platform_url.trim_end_matches('/'),
            tenant_id,
            sibling.harness_id
        );

        match client.delete(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                // Refresh discovery to remove the deactivated sibling
                self.proxy.refresh().await;

                Ok(ToolResult::success(serde_json::json!({
                    "status": "deactivating",
                    "friendly_name": entry.friendly_name,
                    "message": format!("{} is being deactivated.", entry.friendly_name),
                })))
            }
            Ok(resp) => {
                let status = resp.status();
                tracing::warn!(
                    status = %status,
                    "Platform rejected specialist deactivation"
                );
                Ok(ToolResult::error(format!(
                    "Failed to deactivate {}. Please try again later.",
                    entry.friendly_name
                )))
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to reach platform for specialist deactivation");
                Ok(ToolResult::error(
                    "Unable to reach the platform service. Please try again later.".to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_has_entries() {
        assert!(SPECIALIST_CATALOG.len() >= 2);
        assert_eq!(SPECIALIST_CATALOG[0].slug, "autoresearch");
        assert_eq!(SPECIALIST_CATALOG[0].friendly_name, "Research Assistant");
    }

    #[test]
    fn test_find_catalog_entry_by_slug() {
        let entry = find_catalog_entry("autoresearch").unwrap();
        assert_eq!(entry.friendly_name, "Research Assistant");
    }

    #[test]
    fn test_find_catalog_entry_by_friendly_name() {
        let entry = find_catalog_entry("Research Assistant").unwrap();
        assert_eq!(entry.slug, "autoresearch");
    }

    #[test]
    fn test_find_catalog_entry_case_insensitive() {
        let entry = find_catalog_entry("research assistant").unwrap();
        assert_eq!(entry.slug, "autoresearch");
    }

    #[test]
    fn test_find_catalog_entry_partial() {
        let entry = find_catalog_entry("research").unwrap();
        assert_eq!(entry.slug, "autoresearch");
    }

    #[test]
    fn test_find_catalog_entry_not_found() {
        assert!(find_catalog_entry("nonexistent").is_none());
    }
}
