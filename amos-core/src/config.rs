//! Application configuration loaded from env vars, files, and defaults.
//!
//! Uses the [`config`] crate to layer: defaults < config file < env vars.

use serde::Deserialize;
use secrecy::SecretString;

/// Root configuration for the AMOS Rust core.
#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub solana: SolanaConfig,
    #[serde(default)]
    pub bedrock: BedrockConfig,
    #[serde(default)]
    pub agent: AgentConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,
    /// Base URL of the existing Rails app (for hybrid proxying).
    #[serde(default = "default_rails_url")]
    pub rails_url: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            grpc_port: default_grpc_port(),
            rails_url: default_rails_url(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: SecretString,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self { url: default_redis_url() }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SolanaConfig {
    #[serde(default = "default_solana_rpc")]
    pub rpc_url: String,
    #[serde(default = "default_solana_ws")]
    pub ws_url: String,
    #[serde(default = "default_treasury_program")]
    pub treasury_program_id: String,
    #[serde(default = "default_governance_program")]
    pub governance_program_id: String,
    #[serde(default = "default_bounty_program")]
    pub bounty_program_id: String,
}

impl Default for SolanaConfig {
    fn default() -> Self {
        Self {
            rpc_url: default_solana_rpc(),
            ws_url: default_solana_ws(),
            treasury_program_id: default_treasury_program(),
            governance_program_id: default_governance_program(),
            bounty_program_id: default_bounty_program(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct BedrockConfig {
    #[serde(default = "default_aws_region")]
    pub aws_region: String,
    pub aws_access_key_id: Option<SecretString>,
    pub aws_secret_access_key: Option<SecretString>,
    #[serde(default = "default_model")]
    pub default_model: String,
    #[serde(default = "default_chat_model")]
    pub chat_model: String,
    #[serde(default = "default_voice_model")]
    pub voice_model: String,
}

impl Default for BedrockConfig {
    fn default() -> Self {
        Self {
            aws_region: default_aws_region(),
            aws_access_key_id: None,
            aws_secret_access_key: None,
            default_model: default_model(),
            chat_model: default_chat_model(),
            voice_model: default_voice_model(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    /// Maximum iterations for the V3 agent loop before forced stop.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    /// Maximum context tokens before compaction.
    #[serde(default = "default_max_context_tokens")]
    pub max_context_tokens: usize,
    /// Token budget per autonomous loop cycle.
    #[serde(default = "default_token_budget")]
    pub token_budget: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: default_max_iterations(),
            max_context_tokens: default_max_context_tokens(),
            token_budget: default_token_budget(),
        }
    }
}

// ── Defaults ─────────────────────────────────────────────────────────────

fn default_host() -> String { "0.0.0.0".into() }
fn default_port() -> u16 { 3000 }
fn default_grpc_port() -> u16 { 4001 }
fn default_rails_url() -> String { "http://localhost:5001".into() }
fn default_pool_size() -> u32 { 20 }
fn default_redis_url() -> String { "redis://127.0.0.1:6379".into() }
fn default_solana_rpc() -> String { "https://api.devnet.solana.com".into() }
fn default_solana_ws() -> String { "wss://api.devnet.solana.com".into() }
fn default_treasury_program() -> String { "3p2MqHiQVLWfvvfU7psLyEsLLVzbGwqa3bSG7avKqiYP".into() }
fn default_governance_program() -> String { "AQEf6P1qhKC2dCTMhqRh2rmKNpcQsR4ahwT1MvSoSehu".into() }
fn default_bounty_program() -> String { "AmosBnty111111111111111111111111111111111111".into() }
fn default_aws_region() -> String { "us-west-2".into() }
fn default_model() -> String { "us.anthropic.claude-sonnet-4-20250514-v1:0".into() }
fn default_chat_model() -> String { "us.anthropic.claude-sonnet-4-20250514-v1:0".into() }
fn default_voice_model() -> String { "us.anthropic.claude-3-5-haiku-20241022-v1:0".into() }
fn default_max_iterations() -> usize { 25 }
fn default_max_context_tokens() -> usize { 200_000 }
fn default_token_budget() -> usize { 30_000 }

impl AppConfig {
    /// Load configuration from environment variables and optional config files.
    ///
    /// Layering order (later overrides earlier):
    /// 1. Compiled defaults (above)
    /// 2. `config/default.toml` (if present)
    /// 3. `config/{AMOS_ENV}.toml` (if present)
    /// 4. Environment variables prefixed with `AMOS_`
    pub fn load() -> crate::Result<Self> {
        dotenvy::dotenv().ok();

        let env = std::env::var("AMOS_ENV").unwrap_or_else(|_| "development".into());

        let settings = config::Config::builder()
            .add_source(config::File::with_name("config/default").required(false))
            .add_source(config::File::with_name(&format!("config/{env}")).required(false))
            .add_source(
                config::Environment::with_prefix("AMOS")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()
            .map_err(|e| crate::AmosError::Config(e.to_string()))?;

        settings
            .try_deserialize()
            .map_err(|e| crate::AmosError::Config(e.to_string()))
    }
}
