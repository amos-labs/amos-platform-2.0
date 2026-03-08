/// AMOS Bounty Program Instructions Module
///
/// This module organizes all instruction handlers by category:
/// - admin: Program initialization and governance
/// - distribution: Core bounty submission and token distribution
/// - decay: Token decay mechanics
/// - trust: AI agent trust system management

pub mod admin;
pub mod distribution;
pub mod decay;
pub mod trust;

pub use admin::*;
pub use distribution::*;
pub use decay::*;
pub use trust::*;
