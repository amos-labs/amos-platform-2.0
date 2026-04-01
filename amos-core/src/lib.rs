//! # AMOS Core
//!
//! Shared types, configuration, errors, and token economics for the
//! Autonomous Management Operating System (AMOS).
//!
//! This crate is the single source of truth for all domain types used across
//! the Rust workspace. Every other crate depends on `amos-core`.

pub mod config;
pub mod error;
pub mod hooks;
#[cfg(feature = "packages")]
pub mod packages;
pub mod permissions;
pub mod settings;
pub mod token;
pub mod tools;
pub mod types;
pub mod vault;

pub use config::AppConfig;
pub use error::{AmosError, Result};
pub use hooks::HookConfig;
pub use permissions::PermissionLevel;
pub use settings::AmosSettings;
pub use token::economics;
pub use tools::{Tool, ToolCategory, ToolResult};
pub use vault::CredentialVault;

#[cfg(feature = "packages")]
pub use packages::{AmosPackage, PackageContext, PackageToolRegistry};
