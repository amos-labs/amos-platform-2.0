//! # AMOS Agent Runtime
//!
//! V3 single-agent runtime inspired by Pi's "trust the model" philosophy.
//!
//! Architecture:
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │                AgentLoop                     │
//! │  ┌─────────┐  ┌───────────┐  ┌──────────┐  │
//! │  │ Bedrock │──│ Tool Exec │──│ Memory   │  │
//! │  │ Client  │  │  Registry │  │ Manager  │  │
//! │  └─────────┘  └───────────┘  └──────────┘  │
//! │       │              │              │        │
//! │       ▼              ▼              ▼        │
//! │  ┌─────────────────────────────────────┐    │
//! │  │        12 Composable Tools          │    │
//! │  │  platform_{create,query,update,exec}│    │
//! │  │  web_search, view_web_page          │    │
//! │  │  read_file, bash, browser_use       │    │
//! │  │  load_canvas, remember_this,        │    │
//! │  │  search_memory                      │    │
//! │  └─────────────────────────────────────┘    │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! The loop is simple by design:
//! 1. Stream LLM response
//! 2. Execute any tool calls
//! 3. Add results to conversation
//! 4. Repeat until done or max iterations

pub mod agent_loop;
pub mod bedrock;
pub mod model_registry;
pub mod prompt_builder;
pub mod tools;
pub mod memory;

pub use agent_loop::AgentLoop;
pub use bedrock::BedrockClient;
pub use model_registry::ModelRegistry;
