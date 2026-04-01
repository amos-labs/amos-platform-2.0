//! Package loading and activation for the AMOS Harness.
//!
//! This module bridges amos-core's `AmosPackage` trait with the harness runtime.
//! Packages carry their own tools and can be enabled/disabled at runtime.
//! Disabled packages' tools are hidden from agents — zero tool bloat.
//!
//! Routes are handled via feature-gated code since Axum lives in the harness,
//! not in amos-core.

use crate::{state::AppState, tools::ToolRegistry};
use amos_core::{AmosPackage, AppConfig, PackageContext, Result};
use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

/// Load all configured packages, register their tools, and enable them.
///
/// Reads `AMOS_PACKAGES` env var (comma-separated list of package names).
pub fn load_and_register_packages(
    registry: &mut ToolRegistry,
    db_pool: PgPool,
    config: Arc<AppConfig>,
) -> Vec<Box<dyn AmosPackage>> {
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

    let ctx = PackageContext {
        db_pool: db_pool.clone(),
        config: config.clone(),
    };

    let mut packages: Vec<Box<dyn AmosPackage>> = Vec::new();

    for name in &requested {
        match resolve_package(name) {
            Some(pkg) => {
                tracing::info!(
                    "Loading package: {} v{} — {}",
                    pkg.name(),
                    pkg.version(),
                    pkg.description()
                );
                pkg.register_tools(registry, &ctx);
                registry.enable_package(pkg.name());
                packages.push(pkg);
            }
            None => {
                tracing::warn!("Unknown package requested: {name} (skipping)");
            }
        }
    }

    tracing::info!(
        "Loaded {}/{} requested packages",
        packages.len(),
        requested.len()
    );
    packages
}

/// Resolve a package name to its implementation.
///
/// Add new packages here as feature-gated branches.
fn resolve_package(name: &str) -> Option<Box<dyn AmosPackage>> {
    match name {
        #[cfg(feature = "pkg-education")]
        "education" => Some(Box::new(amos_education::EducationPackage::new())),
        _ => None,
    }
}

/// Activate packages: run on_activate and collect routes.
///
/// Called from `server.rs` after AppState is fully constructed.
pub async fn activate_packages(
    packages: &[Box<dyn AmosPackage>],
    state: Arc<AppState>,
) -> Result<Vec<(String, Router)>> {
    let ctx = PackageContext {
        db_pool: state.db_pool.clone(),
        config: state.config.clone(),
    };

    let mut package_routes: Vec<(String, Router)> = Vec::new();

    for pkg in packages {
        tracing::info!("Activating package: {}", pkg.name());

        // Bootstrap schemas, seed data, canvas templates
        pkg.on_activate(&ctx).await?;

        // Collect package routes (feature-gated per package)
        if let Some(router) = get_package_routes(pkg.name(), state.clone()) {
            package_routes.push((pkg.name().to_string(), router));
        }
    }

    Ok(package_routes)
}

/// Get Axum routes for a package (feature-gated).
///
/// Routes live in the package crate but are collected here because
/// Axum Router types can't cross the amos-core boundary.
fn get_package_routes(package_name: &str, #[allow(unused)] state: Arc<AppState>) -> Option<Router> {
    match package_name {
        #[cfg(feature = "pkg-education")]
        "education" => {
            let scorm_state = Arc::new(amos_education::tools::scorm::ScormState {
                db_pool: state.db_pool.clone(),
            });
            Some(amos_education::routes(scorm_state))
        }
        _ => None,
    }
}
