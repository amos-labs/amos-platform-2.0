//! Model registry and escalation chain
//!
//! Manages available LLM models and provides escalation logic for retries.

use serde::{Deserialize, Serialize};

/// Information about an LLM model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Unique model identifier (Bedrock model ID)
    pub id: String,

    /// Human-readable name
    pub display_name: String,

    /// Provider (e.g., "Anthropic", "DeepSeek", "Alibaba")
    pub provider: String,

    /// Context window size in tokens
    pub context_window: usize,

    /// Cost per 1k input tokens (in USD)
    pub cost_per_1k_input: f64,

    /// Cost per 1k output tokens (in USD)
    pub cost_per_1k_output: f64,

    /// Tier for escalation (lower = cheaper/faster, higher = more capable)
    pub tier: u8,

    /// Optional API base URL for custom models (None = use AWS Bedrock).
    /// OpenAI-compatible endpoint (e.g., "http://gpu-server:8000/v1").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,

    /// Whether this model is customer-owned (no compute markup in billing).
    #[serde(default)]
    pub customer_owned: bool,
}

/// Registry of available models
pub struct ModelRegistry {
    models: Vec<ModelInfo>,
}

impl ModelRegistry {
    /// Create a new model registry with all available models
    pub fn new() -> Self {
        let models = vec![
            // Tier 1: Fastest, cheapest (for simple tasks)
            ModelInfo {
                id: "us.anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
                display_name: "Claude 3.5 Haiku".to_string(),
                provider: "Anthropic".to_string(),
                context_window: 200_000,
                cost_per_1k_input: 0.001,
                cost_per_1k_output: 0.005,
                tier: 1,
                api_base: None,
                customer_owned: false,
            },
            // Tier 2: Balanced performance (default)
            ModelInfo {
                id: "us.anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
                display_name: "Claude Sonnet 4".to_string(),
                provider: "Anthropic".to_string(),
                context_window: 200_000,
                cost_per_1k_input: 0.003,
                cost_per_1k_output: 0.015,
                tier: 2,
                api_base: None,
                customer_owned: false,
            },
            // Tier 3: Most capable (for complex tasks)
            ModelInfo {
                id: "us.anthropic.claude-opus-4-20250514-v1:0".to_string(),
                display_name: "Claude Opus 4".to_string(),
                provider: "Anthropic".to_string(),
                context_window: 200_000,
                cost_per_1k_input: 0.015,
                cost_per_1k_output: 0.075,
                tier: 3,
                api_base: None,
                customer_owned: false,
            },
        ];

        Self { models }
    }

    /// Create a model registry with built-in models plus custom providers.
    ///
    /// Custom models are loaded from the `CustomModelsConfig` in the app config.
    /// They integrate into the existing tier/escalation system.
    pub fn with_custom_models(custom_config: &amos_core::config::CustomModelsConfig) -> Self {
        let mut registry = Self::new();

        if !custom_config.enabled {
            return registry;
        }

        for provider in &custom_config.providers {
            let model = ModelInfo {
                id: format!("custom:{}", provider.name),
                display_name: provider.display_name.clone(),
                provider: format!("Custom ({})", provider.name),
                context_window: provider.context_window,
                cost_per_1k_input: provider.cost_per_1k_input,
                cost_per_1k_output: provider.cost_per_1k_output,
                tier: provider.tier,
                api_base: Some(provider.api_base.clone()),
                customer_owned: provider.customer_owned,
            };
            tracing::info!(
                "Registered custom model: {} (tier {}, endpoint: {})",
                model.display_name, model.tier, provider.api_base
            );
            registry.models.push(model);
        }

        registry
    }

    /// Get a model by ID
    pub fn get(&self, id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == id)
    }

    /// Get the cheapest model (for starting escalation chain)
    pub fn get_cheapest(&self) -> &ModelInfo {
        self.models
            .iter()
            .min_by(|a, b| a.tier.cmp(&b.tier))
            .expect("No models available")
    }

    /// Get the most capable model
    pub fn get_most_capable(&self) -> &ModelInfo {
        self.models
            .iter()
            .max_by(|a, b| a.tier.cmp(&b.tier))
            .expect("No models available")
    }

    /// Get all models in a tier
    pub fn get_by_tier(&self, tier: u8) -> Vec<&ModelInfo> {
        self.models.iter().filter(|m| m.tier == tier).collect()
    }

    /// Escalate to the next tier
    ///
    /// Returns the next model in the escalation chain, or None if already at max
    pub fn escalate(&self, current_model_id: &str) -> Option<&ModelInfo> {
        let current = self.get(current_model_id)?;
        let next_tier = current.tier + 1;

        // Find the first model in the next tier
        self.models.iter().find(|m| m.tier == next_tier)
    }

    /// Get the cheapest model at or above the given tier.
    ///
    /// Used for content-type pre-routing: e.g. Document blocks require at least
    /// tier 2 (Sonnet) because Haiku doesn't support native PDF document blocks.
    pub fn get_minimum_tier(&self, min_tier: u8) -> &ModelInfo {
        self.models
            .iter()
            .filter(|m| m.tier >= min_tier)
            .min_by_key(|m| m.tier)
            .unwrap_or_else(|| self.get_most_capable())
    }

    /// Get all models sorted by tier
    pub fn list_all(&self) -> Vec<&ModelInfo> {
        let mut models: Vec<_> = self.models.iter().collect();
        models.sort_by_key(|m| m.tier);
        models
    }

    /// Calculate cost for a conversation
    pub fn calculate_cost(&self, model_id: &str, input_tokens: u64, output_tokens: u64) -> f64 {
        if let Some(model) = self.get(model_id) {
            let input_cost = (input_tokens as f64 / 1000.0) * model.cost_per_1k_input;
            let output_cost = (output_tokens as f64 / 1000.0) * model.cost_per_1k_output;
            input_cost + output_cost
        } else {
            0.0
        }
    }

    /// Get all customer-owned models.
    pub fn get_customer_owned(&self) -> Vec<&ModelInfo> {
        self.models.iter().filter(|m| m.customer_owned).collect()
    }

    /// Check if a model is a custom (non-Bedrock) model.
    pub fn is_custom_model(&self, model_id: &str) -> bool {
        model_id.starts_with("custom:")
    }

    /// Get the API base URL for a model (None means use Bedrock).
    pub fn get_api_base(&self, model_id: &str) -> Option<String> {
        self.get(model_id).and_then(|m| m.api_base.clone())
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// MODEL ROUTER — Pre-routing logic for selecting the right model
// ═══════════════════════════════════════════════════════════════════════════

/// Why a particular model was selected by the router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingReason {
    /// Default cheapest model, no special signals detected.
    Default,
    /// Message contains document/image content blocks (Haiku can't handle PDFs).
    DocumentContent,
    /// User intent keywords detected (deep thinking, analyze, etc.) → Sonnet.
    ComplexIntent,
    /// User intent keywords for very complex tasks → Opus.
    ExpertIntent,
    /// Message is long (> threshold words) suggesting complexity → Sonnet.
    LongMessage,
    /// Custom model explicitly configured for this tier.
    CustomModel,
}

impl std::fmt::Display for RoutingReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => write!(f, "default (simple chat)"),
            Self::DocumentContent => write!(f, "document/image content requires Sonnet+"),
            Self::ComplexIntent => write!(f, "complex intent keywords detected → Sonnet"),
            Self::ExpertIntent => write!(f, "expert-level intent keywords detected → Opus"),
            Self::LongMessage => write!(f, "long message suggests complexity → Sonnet"),
            Self::CustomModel => write!(f, "custom model configured for this tier"),
        }
    }
}

/// Result of model routing.
pub struct RoutingDecision {
    pub model_id: String,
    pub display_name: String,
    pub reason: RoutingReason,
    pub tier: u8,
}

/// Keywords that signal complex intent → Sonnet (tier 2).
const SONNET_KEYWORDS: &[&str] = &[
    "analyze", "analyse", "analysis",
    "strategy", "strategic",
    "comprehensive", "thorough",
    "deep dive", "in-depth", "in depth",
    "architecture", "architect",
    "refactor", "rewrite", "redesign",
    "multi-step", "multistep",
    "compare and contrast", "trade-offs", "tradeoffs",
    "evaluate", "assessment",
    "debug", "diagnose", "troubleshoot",
    "optimize", "optimise", "optimization",
    "review", "audit",
    "plan", "planning", "roadmap",
    "explain in detail", "walk me through",
    "summarize this document", "summarise this document",
];

/// Keywords that signal expert-level intent → Opus (tier 3).
const OPUS_KEYWORDS: &[&str] = &[
    "deep thinking", "think deeply", "think hard",
    "expert", "expert-level", "expert level",
    "build", "implement", "create a full", "build me",
    "complex", "complicated", "intricate",
    "from scratch", "end-to-end", "end to end",
    "production-ready", "production ready",
    "best possible", "highest quality",
    "critical", "mission-critical", "mission critical",
];

/// Long message threshold (word count) — suggests complexity.
const LONG_MESSAGE_THRESHOLD: usize = 200;

impl ModelRegistry {
    /// Route a message to the appropriate model based on content and intent.
    ///
    /// Routing priority (highest minimum tier wins):
    /// 1. Document/Image content → tier 2 (Sonnet) minimum
    /// 2. Opus-level keywords → tier 3 (Opus)
    /// 3. Sonnet-level keywords → tier 2 (Sonnet)
    /// 4. Long message → tier 2 (Sonnet)
    /// 5. Default → tier 1 (Haiku)
    pub fn route(
        &self,
        user_message: &str,
        has_documents: bool,
        has_images: bool,
    ) -> RoutingDecision {
        let mut min_tier: u8 = 1;
        let mut reason = RoutingReason::Default;

        // ── Content-type routing ──────────────────────────────────────
        // Haiku doesn't support native PDF document blocks
        if has_documents {
            min_tier = min_tier.max(2);
            reason = RoutingReason::DocumentContent;
        }

        // Images work on Haiku but complex image analysis is better on Sonnet
        // (only escalate for images if they're combined with analytical intent)
        if has_images && min_tier < 2 {
            let lower = user_message.to_lowercase();
            let is_analytical = SONNET_KEYWORDS.iter().any(|kw| lower.contains(kw));
            if is_analytical {
                min_tier = 2;
                reason = RoutingReason::ComplexIntent;
            }
        }

        // ── Intent-based routing (keyword matching) ───────────────────
        let lower = user_message.to_lowercase();

        // Check Opus keywords first (higher tier)
        if OPUS_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
            if min_tier < 3 {
                min_tier = 3;
                reason = RoutingReason::ExpertIntent;
            }
        }
        // Then Sonnet keywords
        else if SONNET_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
            if min_tier < 2 {
                min_tier = 2;
                reason = RoutingReason::ComplexIntent;
            }
        }

        // ── Message length routing ────────────────────────────────────
        let word_count = user_message.split_whitespace().count();
        if word_count > LONG_MESSAGE_THRESHOLD && min_tier < 2 {
            min_tier = 2;
            reason = RoutingReason::LongMessage;
        }

        // ── Resolve to actual model ───────────────────────────────────
        let model = self.get_minimum_tier(min_tier);
        RoutingDecision {
            model_id: model.id.clone(),
            display_name: model.display_name.clone(),
            reason,
            tier: model.tier,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escalation_chain() {
        let registry = ModelRegistry::new();
        let haiku = registry.get_cheapest();
        assert_eq!(haiku.tier, 1);

        let sonnet = registry.escalate(&haiku.id).unwrap();
        assert_eq!(sonnet.tier, 2);

        let opus = registry.escalate(&sonnet.id).unwrap();
        assert_eq!(opus.tier, 3);

        // No further escalation possible
        assert!(registry.escalate(&opus.id).is_none());
    }

    #[test]
    fn test_minimum_tier() {
        let registry = ModelRegistry::new();

        // Minimum tier 1 should return Haiku (cheapest)
        let min1 = registry.get_minimum_tier(1);
        assert_eq!(min1.tier, 1);

        // Minimum tier 2 should return Sonnet
        let min2 = registry.get_minimum_tier(2);
        assert_eq!(min2.tier, 2);

        // Minimum tier 3 should return Opus
        let min3 = registry.get_minimum_tier(3);
        assert_eq!(min3.tier, 3);

        // Minimum tier 99 should fall back to most capable
        let min99 = registry.get_minimum_tier(99);
        assert_eq!(min99.tier, 3);
    }

    #[test]
    fn test_cost_calculation() {
        let registry = ModelRegistry::new();
        let haiku = registry.get_cheapest();

        // 1000 input tokens, 1000 output tokens
        let cost = registry.calculate_cost(&haiku.id, 1000, 1000);
        let expected = (1000.0 / 1000.0) * haiku.cost_per_1k_input
            + (1000.0 / 1000.0) * haiku.cost_per_1k_output;
        assert!((cost - expected).abs() < 0.0001);
    }

    // ── Router tests ─────────────────────────────────────────────────

    #[test]
    fn test_route_simple_chat_goes_to_haiku() {
        let registry = ModelRegistry::new();
        let decision = registry.route("hello, how are you?", false, false);
        assert_eq!(decision.tier, 1);
        assert_eq!(decision.reason, RoutingReason::Default);
    }

    #[test]
    fn test_route_document_goes_to_sonnet() {
        let registry = ModelRegistry::new();
        let decision = registry.route("review this file", true, false);
        assert_eq!(decision.tier, 2);
        assert_eq!(decision.reason, RoutingReason::DocumentContent);
    }

    #[test]
    fn test_route_analyze_keyword_goes_to_sonnet() {
        let registry = ModelRegistry::new();
        let decision = registry.route("please analyze this data for me", false, false);
        assert_eq!(decision.tier, 2);
        assert_eq!(decision.reason, RoutingReason::ComplexIntent);
    }

    #[test]
    fn test_route_deep_thinking_goes_to_opus() {
        let registry = ModelRegistry::new();
        let decision = registry.route("I need deep thinking on this problem", false, false);
        assert_eq!(decision.tier, 3);
        assert_eq!(decision.reason, RoutingReason::ExpertIntent);
    }

    #[test]
    fn test_route_build_keyword_goes_to_opus() {
        let registry = ModelRegistry::new();
        let decision = registry.route("build me a REST API from scratch", false, false);
        assert_eq!(decision.tier, 3);
        assert_eq!(decision.reason, RoutingReason::ExpertIntent);
    }

    #[test]
    fn test_route_long_message_goes_to_sonnet() {
        let registry = ModelRegistry::new();
        // Create a message with >200 words (no keywords)
        let long_msg = "word ".repeat(250);
        let decision = registry.route(&long_msg, false, false);
        assert_eq!(decision.tier, 2);
        assert_eq!(decision.reason, RoutingReason::LongMessage);
    }

    #[test]
    fn test_route_opus_keyword_wins_over_document() {
        let registry = ModelRegistry::new();
        // Document wants tier 2, but "build" keyword wants tier 3
        let decision = registry.route("build a production-ready system", true, false);
        assert_eq!(decision.tier, 3);
        assert_eq!(decision.reason, RoutingReason::ExpertIntent);
    }

    #[test]
    fn test_route_case_insensitive() {
        let registry = ModelRegistry::new();
        let decision = registry.route("ANALYZE THIS DATA", false, false);
        assert_eq!(decision.tier, 2);
        assert_eq!(decision.reason, RoutingReason::ComplexIntent);
    }

    #[test]
    fn test_custom_model_registration() {
        use amos_core::config::{CustomModelsConfig, CustomModelProvider};

        let config = CustomModelsConfig {
            enabled: true,
            providers: vec![CustomModelProvider {
                name: "qwen-local".into(),
                display_name: "Qwen3-Next 80B (Local)".into(),
                api_base: "http://localhost:8000/v1".into(),
                api_key: None,
                model_id: "Qwen/Qwen3-Next-80B".into(),
                context_window: 131_072,
                tier: 2,
                cost_per_1k_input: 0.0,
                cost_per_1k_output: 0.0,
                customer_owned: true,
            }],
        };

        let registry = ModelRegistry::with_custom_models(&config);

        // Should have 3 built-in + 1 custom
        assert_eq!(registry.list_all().len(), 4);

        // Custom model should be findable
        let custom = registry.get("custom:qwen-local");
        assert!(custom.is_some());
        let custom = custom.unwrap();
        assert_eq!(custom.display_name, "Qwen3-Next 80B (Local)");
        assert_eq!(custom.tier, 2);
        assert!(custom.customer_owned);
        assert_eq!(custom.api_base.as_deref(), Some("http://localhost:8000/v1"));
    }

    #[test]
    fn test_custom_models_disabled() {
        use amos_core::config::CustomModelsConfig;

        let config = CustomModelsConfig {
            enabled: false,
            providers: vec![],
        };

        let registry = ModelRegistry::with_custom_models(&config);
        assert_eq!(registry.list_all().len(), 3); // Only built-in
    }

    #[test]
    fn test_customer_owned_detection() {
        use amos_core::config::{CustomModelsConfig, CustomModelProvider};

        let config = CustomModelsConfig {
            enabled: true,
            providers: vec![CustomModelProvider {
                name: "sovereign".into(),
                display_name: "Sovereign Qwen".into(),
                api_base: "http://local:8000/v1".into(),
                api_key: None,
                model_id: "qwen".into(),
                context_window: 131_072,
                tier: 2,
                cost_per_1k_input: 0.0,
                cost_per_1k_output: 0.0,
                customer_owned: true,
            }],
        };

        let registry = ModelRegistry::with_custom_models(&config);
        let owned = registry.get_customer_owned();
        assert_eq!(owned.len(), 1);
        assert_eq!(owned[0].display_name, "Sovereign Qwen");

        assert!(registry.is_custom_model("custom:sovereign"));
        assert!(!registry.is_custom_model("us.anthropic.claude-3-5-haiku-20241022-v1:0"));
    }
}
