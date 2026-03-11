//! Shared domain types used across all AMOS crates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
// AGENT TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// Unique identifier for a task session with the agent.
pub type SessionId = Uuid;

/// A message in the agent conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Conversation role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A block of content within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    Image {
        source: ImageSource,
    },
    /// Raw document passthrough (e.g. image-heavy PDFs sent directly to Claude
    /// via Bedrock's `document` content block for native vision analysis).
    Document {
        source: DocumentSource,
    },
}

/// Image source for multimodal messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String, // base64
}

/// Document source for native document content blocks (PDF, DOCX, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSource {
    /// Document format: "pdf", "docx", "html", "txt", etc.
    pub format: String,
    /// Human-readable document name (for the API)
    pub name: String,
    /// Base64-encoded raw document bytes
    pub data: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// TOOL TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// Schema definition for a tool exposed to the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    /// Whether this tool requires human confirmation before execution.
    #[serde(default)]
    pub requires_confirmation: bool,
}

/// Result returned from tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
    /// Duration of tool execution.
    pub duration_ms: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// MODEL TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// An LLM model available via AWS Bedrock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub name: String,
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_vision: bool,
    pub cost_per_input_1k: f64,
    pub cost_per_output_1k: f64,
    /// Escalation tier: 0 = cheapest, 1 = balanced, 2 = most capable.
    pub tier: u8,
}

/// Escalation reason detected by the agent loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationReason {
    EmptyResponse,
    GenericFallback,
    HallucinationGuard,
    ToolLoop,
    ComplexRequestNoTools,
    FabricatedData,
    ErrorsIgnored,
}

// ═══════════════════════════════════════════════════════════════════════════
// BOUNTY / CONTRIBUTION TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// Status of a bounty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BountyStatus {
    Open,
    Claimed,
    InProgress,
    InReview,
    Completed,
    Rejected,
    Cancelled,
}

/// A contribution record for token award calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contribution {
    pub id: Uuid,
    pub bounty_id: u64,
    pub operator_wallet: String,
    pub contribution_type: u8,
    pub points: u64,
    pub quality_score: u8,
    pub is_external_agent: bool,
    pub agent_id: Option<u64>,
    pub reviewer_wallet: String,
    pub evidence_hash: [u8; 32],
    pub created_at: DateTime<Utc>,
}

// ═══════════════════════════════════════════════════════════════════════════
// GOVERNANCE TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// Feature proposal status (mirrors on-chain enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Draft,
    Submitted,
    InDevelopment,
    InReview,
    InAbTest,
    AwaitingFeedback,
    AwaitingStewardApproval,
    Approved,
    Merged,
    Rejected,
    Cancelled,
}

/// Quality gate type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateType {
    Benchmark,
    AbTest,
    CustomerFeedback,
    StewardApproval,
    FinalMerge,
}

// ═══════════════════════════════════════════════════════════════════════════
// PLATFORM ENTITY TYPES (Rails bridge)
// ═══════════════════════════════════════════════════════════════════════════

/// Represents a user/account from the Rails app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub name: Option<String>,
    pub wallet_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Represents an organization/tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: i64,
    pub name: String,
    pub subdomain: Option<String>,
}
