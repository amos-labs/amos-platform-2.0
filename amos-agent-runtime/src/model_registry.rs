//! # Model Registry
//!
//! Central registry of all LLM models available via AWS Bedrock.
//! Supports fallback/escalation chains: cheap → balanced → most capable.

use amos_core::types::ModelInfo;
use std::collections::HashMap;

/// Registry of all available models with escalation tiers.
pub struct ModelRegistry {
    models: HashMap<String, ModelInfo>,
    /// Ordered escalation chain: tier 0 → tier 1 → tier 2.
    escalation_chain: Vec<String>,
}

impl ModelRegistry {
    /// Build the default 2026-era Bedrock model registry.
    pub fn default_registry() -> Self {
        let mut models = HashMap::new();
        let mut escalation_chain = Vec::new();

        // ── Tier 0: Cheapest / fastest (try first) ──────────────────
        let haiku = ModelInfo {
            id: "us.anthropic.claude-3-5-haiku-20241022-v1:0".into(),
            provider: "anthropic".into(),
            name: "Claude 3.5 Haiku".into(),
            context_window: 200_000,
            max_output_tokens: 8_192,
            supports_tools: true,
            supports_streaming: true,
            supports_vision: true,
            cost_per_input_1k: 0.001,
            cost_per_output_1k: 0.005,
            tier: 0,
        };
        escalation_chain.push(haiku.id.clone());
        models.insert(haiku.id.clone(), haiku);

        // ── Tier 1: Balanced (escalate here on failure) ─────────────
        let sonnet = ModelInfo {
            id: "us.anthropic.claude-sonnet-4-20250514-v1:0".into(),
            provider: "anthropic".into(),
            name: "Claude Sonnet 4".into(),
            context_window: 200_000,
            max_output_tokens: 16_384,
            supports_tools: true,
            supports_streaming: true,
            supports_vision: true,
            cost_per_input_1k: 0.003,
            cost_per_output_1k: 0.015,
            tier: 1,
        };
        escalation_chain.push(sonnet.id.clone());
        models.insert(sonnet.id.clone(), sonnet);

        // ── Tier 2: Most capable (last resort) ──────────────────────
        let opus = ModelInfo {
            id: "us.anthropic.claude-opus-4-20250514-v1:0".into(),
            provider: "anthropic".into(),
            name: "Claude Opus 4".into(),
            context_window: 200_000,
            max_output_tokens: 32_768,
            supports_tools: true,
            supports_streaming: true,
            supports_vision: true,
            cost_per_input_1k: 0.015,
            cost_per_output_1k: 0.075,
            tier: 2,
        };
        escalation_chain.push(opus.id.clone());
        models.insert(opus.id.clone(), opus);

        // ── Additional models (not in default escalation chain) ─────
        let deepseek = ModelInfo {
            id: "us.deepseek.r1-v1:0".into(),
            provider: "deepseek".into(),
            name: "DeepSeek R1".into(),
            context_window: 128_000,
            max_output_tokens: 8_192,
            supports_tools: false,
            supports_streaming: true,
            supports_vision: false,
            cost_per_input_1k: 0.0014,
            cost_per_output_1k: 0.0028,
            tier: 0,
        };
        models.insert(deepseek.id.clone(), deepseek);

        let qwen = ModelInfo {
            id: "us.amazon.qwen-3-next-80b-v1:0".into(),
            provider: "qwen".into(),
            name: "Qwen 3 Next 80B".into(),
            context_window: 128_000,
            max_output_tokens: 8_192,
            supports_tools: true,
            supports_streaming: true,
            supports_vision: false,
            cost_per_input_1k: 0.002,
            cost_per_output_1k: 0.006,
            tier: 0,
        };
        models.insert(qwen.id.clone(), qwen);

        Self {
            models,
            escalation_chain,
        }
    }

    /// Get model info by ID.
    pub fn get(&self, model_id: &str) -> Option<&ModelInfo> {
        self.models.get(model_id)
    }

    /// Get the next model in the escalation chain.
    /// Returns `None` if already at the top tier.
    pub fn escalate(&self, current_model_id: &str) -> Option<&ModelInfo> {
        let current_idx = self
            .escalation_chain
            .iter()
            .position(|id| id == current_model_id)?;

        let next_idx = current_idx + 1;
        if next_idx < self.escalation_chain.len() {
            let next_id = &self.escalation_chain[next_idx];
            self.models.get(next_id)
        } else {
            None
        }
    }

    /// Get the cheapest model (tier 0) — default starting model.
    pub fn cheapest(&self) -> Option<&ModelInfo> {
        self.escalation_chain
            .first()
            .and_then(|id| self.models.get(id))
    }

    /// Get the most capable model (highest tier).
    pub fn most_capable(&self) -> Option<&ModelInfo> {
        self.escalation_chain
            .last()
            .and_then(|id| self.models.get(id))
    }

    /// List all model IDs in the escalation chain.
    pub fn escalation_chain(&self) -> &[String] {
        &self.escalation_chain
    }

    /// List all registered model IDs.
    pub fn all_model_ids(&self) -> Vec<&str> {
        self.models.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_escalation_chain() {
        let reg = ModelRegistry::default_registry();
        assert_eq!(reg.escalation_chain().len(), 3);
    }

    #[test]
    fn escalation_progresses_cheapest_to_most_capable() {
        let reg = ModelRegistry::default_registry();
        let cheapest = reg.cheapest().unwrap();
        assert_eq!(cheapest.tier, 0);

        let next = reg.escalate(&cheapest.id).unwrap();
        assert_eq!(next.tier, 1);

        let top = reg.escalate(&next.id).unwrap();
        assert_eq!(top.tier, 2);

        assert!(reg.escalate(&top.id).is_none());
    }
}
