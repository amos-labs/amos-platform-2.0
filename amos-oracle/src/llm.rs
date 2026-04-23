//! LLM client interface + stub.
//!
//! Oracle talks to an LLM through this trait. The real impl (forthcoming)
//! wraps AWS Bedrock — either directly or via the existing amos-agent sidecar.
//! For now the trait is stubbable so the rest of the Oracle can be wired,
//! tested, and reviewed without any live LLM dependency.
//!
//! The interface is deliberately minimal: system prompt + user message →
//! string response. Oracle does its own JSON parsing of the response because
//! the structured-output schema is Oracle's concern, not the LLM client's.

use async_trait::async_trait;

use crate::Result;

/// Non-streaming LLM completion.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Produce a completion given system + user messages. Oracle calls this
    /// with temperature=0 (or equivalent), expecting deterministic structured
    /// JSON output.
    async fn complete(&self, system_prompt: &str, user_message: &str) -> Result<String>;

    /// Identifier like "bedrock:anthropic.claude-opus-4-20250514-v1:0" — stored
    /// with every decision for audit / replay / drift analysis.
    fn model_version(&self) -> String;
}

/// Test-only stub that returns a fixed response. Useful for unit tests of the
/// intake / review wiring without a live Bedrock dependency.
#[cfg(test)]
pub struct StubLlmClient {
    pub canned_response: String,
    pub model_version: String,
}

#[cfg(test)]
impl StubLlmClient {
    pub fn new(canned_response: impl Into<String>) -> Self {
        Self {
            canned_response: canned_response.into(),
            model_version: "stub:canned-response".into(),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl LlmClient for StubLlmClient {
    async fn complete(&self, _system_prompt: &str, _user_message: &str) -> Result<String> {
        Ok(self.canned_response.clone())
    }

    fn model_version(&self) -> String {
        self.model_version.clone()
    }
}
