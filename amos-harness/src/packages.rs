//! Package loading and activation for the AMOS Harness.
//!
//! This module bridges amos-core's `AmosPackage` trait with the harness runtime.
//! Packages carry their own tools and can be enabled/disabled at runtime.
//! Disabled packages' tools are hidden from agents — zero tool bloat.
//!
//! ## Declarative registry
//!
//! All compiled-in packages are listed in `build_package_registry()`. Adding a new
//! package only requires one line there (plus the feature gate in Cargo.toml).
//! At startup, `load_and_register_packages()` upserts every compiled-in package
//! into the `packages` DB table, then enables those requested via `AMOS_PACKAGES`
//! env var or already enabled in the DB.
//!
//! Routes are handled via feature-gated code since Axum lives in the harness,
//! not in amos-core.

use crate::{state::AppState, tools::ToolRegistry};
use amos_core::{AmosPackage, AppConfig, PackageContext, Result};
use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

/// Build the list of all compiled-in packages.
///
/// Add new packages here as feature-gated entries. This is the only place
/// you need to touch when adding a new package (besides Cargo.toml features).
#[allow(clippy::vec_init_then_push)]
fn build_package_registry() -> Vec<Box<dyn AmosPackage>> {
    #[allow(unused_mut)]
    let mut packages: Vec<Box<dyn AmosPackage>> = Vec::new();

    #[cfg(feature = "pkg-education")]
    packages.push(Box::new(amos_education::EducationPackage::new()));

    #[cfg(feature = "pkg-autoresearch")]
    packages.push(Box::new(amos_autoresearch::AutoresearchPackage::new()));

    #[cfg(feature = "pkg-social")]
    packages.push(Box::new(amos_social::SocialPackage::new()));

    packages
}

/// Load all configured packages, register their tools, and enable them.
///
/// 1. Builds the compiled-in package registry
/// 2. Upserts each package into the `packages` DB table
/// 3. Reads `AMOS_PACKAGES` env var — auto-enables those packages in DB
/// 4. Checks `packages.enabled` flag to decide activation
/// 5. Records `tool_names` and `tool_count` after registration
pub async fn load_and_register_packages(
    registry: &mut ToolRegistry,
    db_pool: PgPool,
    config: Arc<AppConfig>,
) -> Vec<Box<dyn AmosPackage>> {
    let all_packages = build_package_registry();

    if all_packages.is_empty() {
        tracing::info!("No packages compiled in (enable via Cargo features)");
        return Vec::new();
    }

    let ctx = PackageContext {
        db_pool: db_pool.clone(),
        config: config.clone(),
    };

    // Parse AMOS_PACKAGES env var for auto-enable
    let env_requested: Vec<String> = std::env::var("AMOS_PACKAGES")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut activated: Vec<Box<dyn AmosPackage>> = Vec::new();

    for pkg in all_packages {
        let name = pkg.name().to_string();
        let display_name = pkg.display_name().to_string();
        let description = pkg.description().to_string();
        let version = pkg.version().to_string();
        let system_prompt = pkg.system_prompt().map(|s| s.to_string());

        // Should this package be enabled? Check env var first, then DB.
        let env_enabled = env_requested.contains(&name);

        // Upsert into packages table + handle enable flag
        let db_enabled = upsert_package(
            &db_pool,
            &name,
            &display_name,
            &description,
            &version,
            system_prompt.as_deref(),
            env_enabled,
        )
        .await;

        let enabled = env_enabled || db_enabled;

        if enabled {
            tracing::info!("Loading package: {} v{} — {}", name, version, description);
            pkg.register_tools(registry, &ctx);
            registry.enable_package(&name);

            // Record tool names in DB
            let tool_names = registry.tools_for_package(&name);
            let tool_count = tool_names.len() as i32;
            let tool_names_json =
                serde_json::to_value(&tool_names).unwrap_or(serde_json::json!([]));

            let _ = sqlx::query(
                "UPDATE packages SET tool_count = $1, tool_names = $2, enabled = true, updated_at = NOW() WHERE name = $3",
            )
            .bind(tool_count)
            .bind(&tool_names_json)
            .bind(&name)
            .execute(&db_pool)
            .await;

            activated.push(pkg);
        } else {
            tracing::debug!("Package {} v{} available but not enabled", name, version);
        }
    }

    tracing::info!(
        "Loaded {}/{} available packages",
        activated.len(),
        activated.len() + env_requested.len().saturating_sub(activated.len())
    );

    activated
}

/// Upsert a package into the `packages` table. Returns whether the package
/// is currently enabled in the DB (before any env-var override).
async fn upsert_package(
    db_pool: &PgPool,
    name: &str,
    display_name: &str,
    description: &str,
    version: &str,
    system_prompt: Option<&str>,
    env_enabled: bool,
) -> bool {
    // Try to get existing enabled state
    let existing: Option<(bool,)> = sqlx::query_as("SELECT enabled FROM packages WHERE name = $1")
        .bind(name)
        .fetch_optional(db_pool)
        .await
        .ok()
        .flatten();

    match existing {
        Some((db_enabled,)) => {
            // Update metadata but preserve enabled state (unless env says enable)
            let new_enabled = db_enabled || env_enabled;
            let _ = sqlx::query(
                r#"UPDATE packages
                   SET display_name = $1, description = $2, version = $3,
                       system_prompt = $4, enabled = $5, updated_at = NOW()
                   WHERE name = $6"#,
            )
            .bind(display_name)
            .bind(description)
            .bind(version)
            .bind(system_prompt)
            .bind(new_enabled)
            .bind(name)
            .execute(db_pool)
            .await;
            db_enabled
        }
        None => {
            // Insert new
            let _ = sqlx::query(
                r#"INSERT INTO packages (name, display_name, description, version, enabled, system_prompt)
                   VALUES ($1, $2, $3, $4, $5, $6)"#,
            )
            .bind(name)
            .bind(display_name)
            .bind(description)
            .bind(version)
            .bind(env_enabled)
            .bind(system_prompt)
            .execute(db_pool)
            .await;
            false
        }
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
        #[cfg(feature = "pkg-autoresearch")]
        "autoresearch" => {
            let autoresearch_state = amos_autoresearch::AutoresearchState {
                db_pool: state.db_pool.clone(),
            };
            Some(amos_autoresearch::package_routes(autoresearch_state))
        }
        _ => None,
    }
}
