//! # AMOS Agent
//!
//! Standalone autonomous agent for the AMOS ecosystem.
//!
//! The agent operates as an independent binary that communicates with the
//! AMOS Harness over the same protocol used by any external agent (OpenClaw,
//! third-party, etc.). One protocol, no shortcuts.
//!
//! ## Architecture
//!
//! ```text
//! amos-agent binary
//! +-- Agent Loop (think -> act -> observe cycle)
//! +-- Local Tools (think, remember/recall, plan, web_search, read/write file)
//! +-- Memory Store (SQLite - persistent local memory)
//! +-- Harness Client (HTTP - register, pull tasks, report results, heartbeat)
//! +-- Agent Card Server (/.well-known/agent.json)
//! +-- Model Provider (Bedrock, OpenAI-compatible, BYOK)
//! ```
//!
//! ## Design Principles
//!
//! 1. **One Protocol** - The AMOS agent uses the exact same protocol as any
//!    external agent. We eat our own dog food.
//! 2. **Local Autonomy** - The agent has its own tools (think, remember, plan,
//!    web search, file I/O) that run locally without the harness.
//! 3. **Harness Tools via HTTP** - Harness-provided tools (database, canvas,
//!    documents, integrations) are accessed via the harness tool execution API.
//! 4. **Memory** - SQLite-backed persistent memory for cross-session recall.
//! 5. **Discoverable** - Serves an Agent Card at `/.well-known/agent.json`.

pub mod agent_card;
pub mod agent_loop;
pub mod anthropic;
pub mod bedrock;
pub mod commands;
pub mod compact;
pub mod config;
pub mod harness_client;
pub mod mcp;
pub mod memory;
pub mod model_registry;
pub mod provider;
pub mod routes;
pub mod task_consumer;
pub mod tools;
pub mod vertex;
