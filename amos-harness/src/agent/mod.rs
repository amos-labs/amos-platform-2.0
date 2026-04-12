//! Agent subsystem for autonomous bounty execution.
//!
//! This module provides the infrastructure for agents to operate autonomously:
//! - **Context**: Parses AGENT_CONTEXT.md for protocol parameters
//! - **Autonomous**: Background loop for bounty discovery → execution → submission

pub mod autonomous;
pub mod context;
