//! Package system trait — the contract for harness extensions.
//!
//! Lives in amos-core so package crates can implement `AmosPackage`
//! without depending on amos-harness (breaking the circular dependency).

use crate::{tools::Tool, AppConfig, Result};
use async_trait::async_trait;
use std::sync::Arc;

/// Context passed to packages during registration and activation.
///
/// Provides the dependencies packages need without requiring the full
/// harness AppState (which would create a circular dependency).
pub struct PackageContext {
    pub db_pool: sqlx::PgPool,
    pub config: Arc<AppConfig>,
}

/// Trait for registering package-scoped tools.
///
/// Implemented by amos-harness's ToolRegistry. Package crates call this
/// to register their tools without knowing the registry internals.
pub trait PackageToolRegistry {
    fn register_package_tool(&mut self, tool: Arc<dyn Tool>, package: &str);
}

/// Trait that all AMOS packages must implement.
///
/// Packages are self-contained domain extensions (education, CRM, healthcare, etc.)
/// that carry their own tools and can be enabled/disabled at runtime.
///
/// Lifecycle:
/// 1. `register_tools()` — called at startup, register tools with the harness
/// 2. `on_activate()` — bootstrap schemas, seed data (idempotent, every boot)
///
/// Routes are handled separately by the harness (which can access package-specific
/// route functions when the feature flag is enabled).
#[async_trait]
pub trait AmosPackage: Send + Sync {
    /// Unique package identifier (e.g., "education", "crm")
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// Semantic version
    fn version(&self) -> &str;

    /// Register package-specific tools.
    ///
    /// Use `registry.register_package_tool(tool, self.name())` so tools are
    /// scoped to this package and respect enable/disable toggling.
    fn register_tools(&self, registry: &mut dyn PackageToolRegistry, ctx: &PackageContext);

    /// Bootstrap schemas, seed data, canvas templates. Must be idempotent.
    async fn on_activate(&self, ctx: &PackageContext) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
}
