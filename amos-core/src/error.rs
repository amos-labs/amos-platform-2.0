//! Unified error types for the AMOS platform.
//!
//! Every subsystem maps its errors into [`AmosError`] so callers get
//! a single, consistent error surface.

use thiserror::Error;

/// Convenience alias used throughout the workspace.
pub type Result<T> = std::result::Result<T, AmosError>;

/// Top-level error enum for the AMOS platform.
#[derive(Error, Debug)]
pub enum AmosError {
    // ── Token Economics ──────────────────────────────────────────────
    #[error("Arithmetic overflow in token calculation: {context}")]
    ArithmeticOverflow { context: String },

    #[error("Insufficient stake: have {have}, need {need}")]
    InsufficientStake { have: u64, need: u64 },

    #[error("Stake too recent: {days_held} days held, need {days_required}")]
    StakeTooRecent { days_held: u64, days_required: u64 },

    #[error("Decay rate {rate_bps} bps out of allowed range [{min_bps}, {max_bps}]")]
    DecayRateOutOfRange {
        rate_bps: u64,
        min_bps: u64,
        max_bps: u64,
    },

    #[error("No revenue available to claim")]
    NoRevenueToClaim,

    #[error("Treasury exhausted: {remaining} tokens remaining")]
    TreasuryExhausted { remaining: u64 },

    #[error("Trust level insufficient: level {current}, need {required}")]
    TrustLevelInsufficient { current: u8, required: u8 },

    #[error("Trust upgrade not eligible: {reason}")]
    TrustUpgradeNotEligible { reason: String },

    #[error("Already at maximum trust level ({level})")]
    AlreadyMaxTrust { level: u8 },

    #[error("Within grace period: {days_remaining} days remaining")]
    WithinGracePeriod { days_remaining: u64 },

    #[error("At decay floor: balance already at minimum preserved amount")]
    AtDecayFloor,

    // ── Agent Runtime ────────────────────────────────────────────────
    #[error("Tool not found: {name}")]
    ToolNotFound { name: String },

    #[error("Tool execution failed: {tool} - {reason}")]
    ToolExecutionFailed { tool: String, reason: String },

    #[error("Model invocation failed: {model} - {reason}")]
    ModelInvocationFailed { model: String, reason: String },

    #[error("Model escalation exhausted after trying: {models_tried:?}")]
    ModelEscalationExhausted { models_tried: Vec<String> },

    #[error("Agent loop exceeded maximum iterations ({max})")]
    AgentLoopExceeded { max: usize },

    #[error("Context window exceeded: {tokens} tokens, max {max_tokens}")]
    ContextWindowExceeded { tokens: usize, max_tokens: usize },

    // ── Database ─────────────────────────────────────────────────────
    #[cfg(feature = "db")]
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[cfg(not(feature = "db"))]
    #[error("Database error: {0}")]
    Database(String),

    // ── HTTP / Network ───────────────────────────────────────────────
    #[cfg(feature = "http")]
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[cfg(not(feature = "http"))]
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("Solana RPC error: {0}")]
    SolanaRpc(String),

    // ── Configuration ────────────────────────────────────────────────
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),

    // ── Authorization ────────────────────────────────────────────────
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    // ── Generic ──────────────────────────────────────────────────────
    #[error("Not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AmosError {
    /// HTTP-style status code for API layer mapping.
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound { .. } => 404,
            Self::Validation(_) => 422,
            Self::InsufficientStake { .. }
            | Self::StakeTooRecent { .. }
            | Self::DecayRateOutOfRange { .. }
            | Self::TrustLevelInsufficient { .. }
            | Self::TrustUpgradeNotEligible { .. }
            | Self::WithinGracePeriod { .. }
            | Self::AtDecayFloor => 422,
            Self::NoRevenueToClaim | Self::TreasuryExhausted { .. } => 409,
            _ => 500,
        }
    }
}
