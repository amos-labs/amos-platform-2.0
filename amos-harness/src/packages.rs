//! Package system for extending the AMOS Harness
//!
//! Packages are self-contained domain extensions (education, CRM, healthcare, etc.)
//! that register tools, routes, and bootstrap data without modifying core harness code.

use crate::{state::AppState, tools::ToolRegistry};
use amos_core::Result;
use async_trait::async_trait;
use axum::Router;
use std::sync::Arc;

/// Trait that all AMOS packages must implement.
///
/// Packages extend the harness with domain-specific tools, routes, and data.
/// The harness loads packages at startup based on the `AMOS_PACKAGES` env var.
#[async_trait]
pub trait AmosPackage: Send + Sync {
    /// Unique package identifier (e.g., "education", "crm")
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// Semantic version
    fn version(&self) -> &str;

    /// Register package-specific tools with the harness tool registry.
    /// Called during harness startup before the server begins accepting requests.
    fn register_tools(&self, registry: &mut ToolRegistry, state: &AppState);

    /// Return package-specific Axum routes to be nested under `/api/v1/pkg/{name}/`.
    /// Return `None` if the package has no custom routes.
    fn routes(&self, state: Arc<AppState>) -> Option<Router> {
        let _ = state;
        None
    }

    /// Called once after registration to bootstrap schemas, seed data, and canvas templates.
    /// Runs during harness startup — should be idempotent (safe to call on every boot).
    async fn on_activate(&self, state: &AppState) -> Result<()> {
        let _ = state;
        Ok(())
    }
}

/// Load and initialize all configured packages.
///
/// Reads `AMOS_PACKAGES` env var (comma-separated list of package names)
/// and returns the matching package instances.
pub fn load_configured_packages() -> Vec<Box<dyn AmosPackage>> {
    let package_list = std::env::var("AMOS_PACKAGES").unwrap_or_default();
    let requested: Vec<&str> = package_list
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if requested.is_empty() {
        tracing::info!("No packages configured (set AMOS_PACKAGES to enable)");
        return Vec::new();
    }

    #[allow(unused_mut)]
    let mut packages: Vec<Box<dyn AmosPackage>> = Vec::new();

    for name in &requested {
        // Package matching — enable features as package crates are added.
        // Example: #[cfg(feature = "pkg-education")]
        // "education" => packages.push(Box::new(amos_education::EducationPackage::new())),
        tracing::warn!("Unknown package requested: {name} (skipping)");
    }

    tracing::info!(
        "Loaded {}/{} requested packages",
        packages.len(),
        requested.len()
    );
    packages
}

/// Register all package tools and collect package routes.
///
/// Called from `server.rs` during harness initialization.
pub async fn activate_packages<'a>(
    packages: &'a [Box<dyn AmosPackage>],
    state: &'a AppState,
) -> Result<Vec<(&'a str, Router)>> {
    // Phase 1: Run on_activate for each package (bootstrap schemas, seed data)
    for pkg in packages {
        tracing::info!(
            "Activating package: {} v{} — {}",
            pkg.name(),
            pkg.version(),
            pkg.description()
        );
        pkg.on_activate(state).await?;
    }

    // Note: package routes are collected separately in server.rs where
    // Arc<AppState> is available (routes() requires Arc<AppState>).
    Ok(Vec::new())
}
