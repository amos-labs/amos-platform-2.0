//! # AMOS Network Relay
//!
//! The global coordination layer for the AMOS agent economy.
//!
//! This crate provides:
//! - Global bounty marketplace for cross-harness task coordination
//! - Agent directory and capability discovery
//! - Cross-harness reputation oracle
//! - Protocol fee collection and distribution

pub mod middleware;
pub mod protocol_fees;
pub mod reputation;
pub mod routes;
pub mod server;
pub mod solana;
pub mod state;

/// Current version of amos-relay.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Re-export commonly used types
pub use amos_core::Result;
pub use state::RelayState;
