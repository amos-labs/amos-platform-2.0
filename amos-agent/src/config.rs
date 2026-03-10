//! Agent configuration and CLI argument parsing.

use clap::Parser;
use serde::{Deserialize, Serialize};

/// AMOS Agent - Standalone autonomous agent for the AMOS ecosystem.
#[derive(Parser, Debug)]
#[command(name = "amos-agent", about = "AMOS autonomous agent")]
pub struct Cli {
    /// Harness URL to connect to
    #[arg(long, env = "AMOS_HARNESS_URL", default_value = "http://localhost:3000")]
    pub harness_url: String,

    /// Agent name for registration
    #[arg(long, env = "AMOS_AGENT_NAME", default_value = "amos-agent")]
    pub agent_name: String,

    /// Port to serve Agent Card on
    #[arg(long, env = "AMOS_AGENT_PORT", default_value = "3100")]
    pub agent_port: u16,

    /// Model provider: "bedrock" or "openai"
    #[arg(long, env = "AMOS_MODEL_PROVIDER", default_value = "bedrock")]
    pub model_provider: String,

    /// Model ID to use (e.g. "anthropic.claude-sonnet-4-20250514-v1:0" or "gpt-4")
    #[arg(long, env = "AMOS_MODEL_ID", default_value = "anthropic.claude-sonnet-4-20250514-v1:0")]
    pub model_id: String,

    /// OpenAI-compatible API base URL (for non-Bedrock providers)
    #[arg(long, env = "AMOS_API_BASE")]
    pub api_base: Option<String>,

    /// API key for the model provider
    #[arg(long, env = "AMOS_API_KEY")]
    pub api_key: Option<String>,

    /// Path to SQLite memory database
    #[arg(long, env = "AMOS_MEMORY_DB", default_value = "amos_agent_memory.db")]
    pub memory_db: String,

    /// Brave Search API key for web search tool
    #[arg(long, env = "BRAVE_API_KEY")]
    pub brave_api_key: Option<String>,

    /// Maximum agent loop iterations
    #[arg(long, env = "AMOS_MAX_ITERATIONS", default_value = "25")]
    pub max_iterations: usize,

    /// Agent API token for harness authentication
    #[arg(long, env = "AMOS_AGENT_TOKEN")]
    pub agent_token: Option<String>,

    /// Working directory for file operations
    #[arg(long, env = "AMOS_WORK_DIR", default_value = ".")]
    pub work_dir: String,

    /// Log level
    #[arg(long, env = "RUST_LOG", default_value = "info")]
    pub log_level: String,
}

/// Runtime agent configuration (built from CLI + defaults).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub harness_url: String,
    pub agent_name: String,
    pub agent_port: u16,
    pub model_provider: String,
    pub model_id: String,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub memory_db: String,
    pub brave_api_key: Option<String>,
    pub max_iterations: usize,
    pub agent_token: Option<String>,
    pub work_dir: String,
    pub log_level: String,
}

impl From<Cli> for AgentConfig {
    fn from(cli: Cli) -> Self {
        Self {
            harness_url: cli.harness_url,
            agent_name: cli.agent_name,
            agent_port: cli.agent_port,
            model_provider: cli.model_provider,
            model_id: cli.model_id,
            api_base: cli.api_base,
            api_key: cli.api_key,
            memory_db: cli.memory_db,
            brave_api_key: cli.brave_api_key,
            max_iterations: cli.max_iterations,
            agent_token: cli.agent_token,
            work_dir: cli.work_dir,
            log_level: cli.log_level,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_cli_defaults() {
        let cli = Cli::parse_from(["amos-agent"]);
        let config = AgentConfig::from(cli);
        assert_eq!(config.harness_url, "http://localhost:3000");
        assert_eq!(config.agent_name, "amos-agent");
        assert_eq!(config.agent_port, 3100);
        assert_eq!(config.model_provider, "bedrock");
        assert_eq!(config.max_iterations, 25);
    }
}
