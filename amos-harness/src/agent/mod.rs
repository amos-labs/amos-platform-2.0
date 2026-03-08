//! Agent runtime and orchestration
//!
//! This module contains the V3 agent loop inspired by Pi's architecture,
//! along with model management and prompt construction.

pub mod bedrock;
pub mod loop_runner;
pub mod model_registry;
pub mod prompt_builder;

pub use bedrock::{BedrockClient, StreamEvent, TokenUsage};
pub use loop_runner::{AgentEvent, AgentLoop, LoopConfig};
pub use model_registry::{ModelInfo, ModelRegistry};
pub use prompt_builder::build_system_prompt;
