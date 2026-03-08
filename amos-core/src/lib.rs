//! # AMOS Core
//!
//! Shared types, configuration, errors, and token economics for the
//! Autonomous Management Operating System (AMOS).
//!
//! This crate is the single source of truth for all domain types used across
//! the Rust workspace. Every other crate depends on `amos-core`.

pub mod config;
pub mod error;
pub mod token;
pub mod types;

pub use config::AppConfig;
pub use error::{AmosError, Result};
pub use token::economics;
