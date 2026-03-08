//! # AMOS Harness
//!
//! The per-customer AI harness that serves as their single business interface.
//!
//! AMOS Harness is an AI-native business operating system deployed per-customer.
//! It provides:
//! - Conversational + canvas interface (the customer's ONLY UI)
//! - AI agent that builds workflows, automations, integrations, and apps
//! - Control plane for OpenClaw agents (autonomous AI employees)
//! - Task queue with internal sub-agents and external bounties
//!
//! ## Architecture
//!
//! The harness consists of several key components:
//!
//! - **Agent**: V3 event-driven agent loop with model escalation and streaming
//! - **Canvas**: Dynamic UI generation and rendering engine
//! - **Tools**: Extensible tool system for platform, canvas, web, and system operations
//! - **OpenClaw**: Autonomous AI agent management and orchestration
//! - **Task Queue**: Unified task system with internal sub-agents and external bounties
//! - **Integrations**: Connector framework for third-party services
//! - **Memory**: Working memory with salience-based attention

pub mod agent;
pub mod canvas;
pub mod documents;
pub mod image_gen;
pub mod integrations;
pub mod memory;
pub mod middleware;
pub mod openclaw;
pub mod revisions;
pub mod routes;
pub mod schema;
pub mod server;
pub mod sessions;
pub mod sites;
pub mod storage;
pub mod state;
pub mod task_queue;
pub mod tools;

// Re-export commonly used types
pub use server::create_server;
pub use state::AppState;

// Re-export agent types
pub use agent::{
    loop_runner::{AgentEvent, AgentLoop, LoopConfig},
    model_registry::{ModelInfo, ModelRegistry},
};

// Re-export canvas types
pub use canvas::{
    types::{Canvas, CanvasResponse, CanvasType},
    CanvasEngine,
};

// Re-export tool types
pub use tools::{Tool, ToolRegistry, ToolResult};

// Re-export OpenClaw types
pub use openclaw::{AgentConfig, AgentManager, AgentStatus};

// Re-export task queue types
pub use task_queue::{Task, TaskCategory, TaskQueue, TaskStatus};

// Re-export document processing types
pub use documents::{DocumentExporter, DocumentProcessor, ExportFormat, ExtractionResult};

// Re-export image generation types
pub use image_gen::ImageGenClient;

use amos_core::Result;

/// Version of the AMOS Harness
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the harness with the given configuration
pub async fn init() -> Result<()> {
    // Any global initialization can go here
    Ok(())
}
