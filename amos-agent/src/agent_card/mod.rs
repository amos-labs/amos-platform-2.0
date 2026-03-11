//! Agent Card server - serves `/.well-known/agent.json` for discoverability.
//!
//! The Agent Card follows the A2A (Agent-to-Agent) protocol pattern,
//! advertising the agent's capabilities, endpoints, and metadata.

use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};

/// Agent Card served at `/.well-known/agent.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Agent name
    pub name: String,
    /// Short description
    pub description: String,
    /// Agent version
    pub version: String,
    /// URL where this agent can be reached
    pub url: String,
    /// List of capabilities
    pub capabilities: Vec<AgentCapability>,
    /// Authentication requirements
    pub auth: AgentAuth,
    /// Protocol version
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapability {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAuth {
    /// Authentication scheme: "bearer", "none"
    pub scheme: String,
}

impl Default for AgentCard {
    fn default() -> Self {
        Self {
            name: "amos-agent".to_string(),
            description: "AMOS autonomous agent with local tools, memory, and harness integration"
                .to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            url: "http://localhost:3100".to_string(),
            capabilities: vec![
                AgentCapability {
                    name: "chat".to_string(),
                    description: "Interactive conversational AI".to_string(),
                },
                AgentCapability {
                    name: "task_execution".to_string(),
                    description: "Autonomous task execution with tool use".to_string(),
                },
                AgentCapability {
                    name: "web_search".to_string(),
                    description: "Real-time web search".to_string(),
                },
                AgentCapability {
                    name: "memory".to_string(),
                    description: "Persistent cross-session memory".to_string(),
                },
                AgentCapability {
                    name: "file_operations".to_string(),
                    description: "Local file read/write".to_string(),
                },
            ],
            auth: AgentAuth {
                scheme: "bearer".to_string(),
            },
            protocol_version: "1.0".to_string(),
        }
    }
}

/// Create the Agent Card router.
pub fn agent_card_router(card: AgentCard) -> Router {
    Router::new().route(
        "/.well-known/agent.json",
        get(move || {
            let c = card.clone();
            async move { Json(c) }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_agent_card() {
        let card = AgentCard::default();
        assert_eq!(card.name, "amos-agent");
        assert_eq!(card.capabilities.len(), 5);
        assert_eq!(card.auth.scheme, "bearer");
    }

    #[test]
    fn test_agent_card_serialization() {
        let card = AgentCard::default();
        let json = serde_json::to_string_pretty(&card).unwrap();
        assert!(json.contains("amos-agent"));
        assert!(json.contains("web_search"));

        // Roundtrip
        let deserialized: AgentCard = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, card.name);
    }
}
