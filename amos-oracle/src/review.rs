//! Review path: evaluate completed work against mission alignment.
//!
//! Runs AFTER the mechanical QA bot. Oracle's job is not "did tests pass" —
//! it is "does this work advance the mission." QA bot's judgment is consumed
//! as one input; Oracle produces the final mission-layer verdict.
//!
//! Structure mirrors [`crate::intake`] — prompt assembly, LLM call, structured
//! parse, guards, event log write. Review-specific:
//! - Confidence threshold is tighter (0.85 vs 0.80) because approval is
//!   immediate spend.
//! - Output must include `false_approve_vs_false_reject_weighting` — empty
//!   fails validation.
//!
//! MVP: scaffolded entry point + request shape. Real LLM wiring lands in a
//! subsequent commit once intake is proven end-to-end.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::OracleAgent;
use crate::decision::Decision;
use crate::{OracleError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub bounty_id: Uuid,
    pub bounty_title: String,
    pub bounty_description: String,
    pub bounty_category: String,
    pub bounty_contribution_type_id: u8,
    pub qa_evidence: serde_json::Value,
    pub proof: serde_json::Value,
    pub revision_count: u8,
}

pub async fn evaluate(_agent: &OracleAgent, _request: ReviewRequest) -> Result<Decision> {
    Err(OracleError::Internal(
        "review::evaluate not yet implemented — intake path is priority; review lands after \
         intake is proven end-to-end in production"
            .into(),
    ))
}
