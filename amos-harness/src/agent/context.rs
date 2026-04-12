//! Agent Context Loader — parses AGENT_CONTEXT.md into typed Rust structs.
//!
//! Loads the YAML blocks from AGENT_CONTEXT.md to configure autonomous agents
//! with correct protocol parameters. Validates parsed values against the
//! constants in `amos-core/src/token/economics.rs`.

use amos_core::token::economics;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

/// Parsed protocol context from AGENT_CONTEXT.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub token_params: TokenParams,
    pub trust_levels: TrustConfig,
    pub bounty_params: BountyParams,
    pub emission_schedule: EmissionSchedule,
    pub bounty_lifecycle: Vec<String>,
}

/// Token supply and allocation parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenParams {
    pub total_supply: u64,
    pub treasury_allocation: u64,
    pub reserve_allocation: u64,
}

/// Trust system configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustConfig {
    pub max_level: u8,
    pub levels: HashMap<u8, TrustLevel>,
}

/// Parameters for a single trust level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustLevel {
    pub max_points: u64,
    pub daily_bounty_limit: u32,
}

/// Bounty system parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BountyParams {
    pub min_quality_score: u8,
    pub max_bounty_points: u64,
    pub max_daily_bounties: u64,
    pub reviewer_reward_bps: u64,
    pub contribution_multipliers: HashMap<String, u64>,
}

/// Emission schedule parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionSchedule {
    pub initial_daily_emission: u64,
    pub halving_interval_days: u64,
    pub minimum_daily_emission: u64,
    pub max_halving_epochs: u64,
}

impl AgentContext {
    /// Load agent context from the AGENT_CONTEXT.md file.
    ///
    /// Parses the YAML blocks and validates against economics.rs constants.
    pub fn load(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read AGENT_CONTEXT.md at {}: {e}", path.display()))?;

        Self::parse(&content)
    }

    /// Parse agent context from markdown content.
    pub fn parse(content: &str) -> Result<Self, String> {
        let ctx = Self {
            token_params: Self::parse_token_params(content)?,
            trust_levels: Self::parse_trust_config(content)?,
            bounty_params: Self::parse_bounty_params(content)?,
            emission_schedule: Self::parse_emission_schedule(content)?,
            bounty_lifecycle: vec![
                "DISCOVER".into(),
                "ASSESS".into(),
                "CLAIM".into(),
                "EXECUTE".into(),
                "SUBMIT".into(),
                "VERIFY".into(),
                "EARN".into(),
                "REPEAT".into(),
            ],
        };

        ctx.validate()?;
        Ok(ctx)
    }

    /// Validate parsed context against economics.rs constants.
    pub fn validate(&self) -> Result<(), String> {
        let mut errors = Vec::new();

        // Token supply
        if self.token_params.total_supply != economics::TOTAL_SUPPLY {
            errors.push(format!(
                "total_supply mismatch: context={} economics={}",
                self.token_params.total_supply,
                economics::TOTAL_SUPPLY,
            ));
        }
        if self.token_params.treasury_allocation != economics::TREASURY_ALLOCATION {
            errors.push(format!(
                "treasury_allocation mismatch: context={} economics={}",
                self.token_params.treasury_allocation,
                economics::TREASURY_ALLOCATION,
            ));
        }
        if self.token_params.reserve_allocation != economics::RESERVE_ALLOCATION {
            errors.push(format!(
                "reserve_allocation mismatch: context={} economics={}",
                self.token_params.reserve_allocation,
                economics::RESERVE_ALLOCATION,
            ));
        }

        // Bounty params
        if self.bounty_params.min_quality_score != economics::MIN_QUALITY_SCORE {
            errors.push(format!(
                "min_quality_score mismatch: context={} economics={}",
                self.bounty_params.min_quality_score,
                economics::MIN_QUALITY_SCORE,
            ));
        }
        if self.bounty_params.max_bounty_points != economics::MAX_BOUNTY_POINTS {
            errors.push(format!(
                "max_bounty_points mismatch: context={} economics={}",
                self.bounty_params.max_bounty_points,
                economics::MAX_BOUNTY_POINTS,
            ));
        }

        // Emission schedule
        if self.emission_schedule.initial_daily_emission != economics::INITIAL_DAILY_EMISSION {
            errors.push(format!(
                "initial_daily_emission mismatch: context={} economics={}",
                self.emission_schedule.initial_daily_emission,
                economics::INITIAL_DAILY_EMISSION,
            ));
        }
        if self.emission_schedule.halving_interval_days != economics::HALVING_INTERVAL_DAYS {
            errors.push(format!(
                "halving_interval_days mismatch: context={} economics={}",
                self.emission_schedule.halving_interval_days,
                economics::HALVING_INTERVAL_DAYS,
            ));
        }

        if errors.is_empty() {
            info!("Agent context validated against economics.rs constants");
            Ok(())
        } else {
            let msg = format!(
                "Agent context validation failed ({} errors):\n  {}",
                errors.len(),
                errors.join("\n  ")
            );
            Err(msg)
        }
    }

    /// Generate a system prompt snippet with protocol context for autonomous agents.
    pub fn to_system_prompt(&self) -> String {
        format!(
            "## AMOS Protocol Context\n\
             - Total supply: {} AMOS (fixed, mint disabled)\n\
             - Treasury: {} AMOS (distributed via bounties)\n\
             - Daily emission: {} AMOS/day (halving every {} days)\n\
             - Minimum quality score: {}/100\n\
             - Maximum bounty points: {}\n\
             - Trust levels: 1-{} (earn through verified work)\n\
             \n\
             ## Bounty Lifecycle\n\
             DISCOVER → ASSESS → CLAIM → EXECUTE → SUBMIT → VERIFY → EARN → REPEAT\n\
             \n\
             ## Contribution Multipliers\n\
             {}\n",
            self.token_params.total_supply,
            self.token_params.treasury_allocation,
            self.emission_schedule.initial_daily_emission,
            self.emission_schedule.halving_interval_days,
            self.bounty_params.min_quality_score,
            self.bounty_params.max_bounty_points,
            self.trust_levels.max_level,
            self.bounty_params
                .contribution_multipliers
                .iter()
                .map(|(k, v)| format!("- {k}: {}%", v / 100))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }

    /// Get the daily bounty limit for a given trust level.
    pub fn daily_bounty_limit(&self, trust_level: u8) -> u32 {
        self.trust_levels
            .levels
            .get(&trust_level)
            .map(|l| l.daily_bounty_limit)
            .unwrap_or(3) // conservative default
    }

    // ── Parsers for YAML blocks ────────────────────────────────────────

    fn parse_token_params(content: &str) -> Result<TokenParams, String> {
        // Extract values from the YAML block in section 2
        let total_supply = Self::extract_yaml_u64(content, "total_supply")
            .unwrap_or(economics::TOTAL_SUPPLY);

        // Parse allocations from the allocation block
        let treasury = Self::extract_yaml_u64(content, "bounty_treasury")
            .unwrap_or(economics::TREASURY_ALLOCATION);
        let reserve = Self::extract_yaml_u64(content, "emergency_reserve")
            .unwrap_or(economics::RESERVE_ALLOCATION);

        Ok(TokenParams {
            total_supply,
            treasury_allocation: treasury,
            reserve_allocation: reserve,
        })
    }

    fn parse_trust_config(content: &str) -> Result<TrustConfig, String> {
        let mut levels = HashMap::new();

        // Parse trust level parameters from the YAML blocks
        let level_configs = [
            (1u8, 100u64, 3u32),
            (2, 200, 5),
            (3, 500, 10),
            (4, 1000, 15),
            (5, 2000, 25),
        ];

        for (level, default_points, default_limit) in level_configs {
            let key = format!("level_{}", level);
            let max_points =
                Self::extract_nested_yaml_u64(content, &key, "max_points").unwrap_or(default_points);
            let daily_limit = Self::extract_nested_yaml_u64(content, &key, "daily_bounty_limit")
                .unwrap_or(default_limit as u64) as u32;

            levels.insert(
                level,
                TrustLevel {
                    max_points,
                    daily_bounty_limit: daily_limit,
                },
            );
        }

        Ok(TrustConfig {
            max_level: 5,
            levels,
        })
    }

    fn parse_bounty_params(content: &str) -> Result<BountyParams, String> {
        let min_quality = Self::extract_yaml_u64(content, "min_quality_score")
            .unwrap_or(economics::MIN_QUALITY_SCORE as u64) as u8;
        let max_points = Self::extract_yaml_u64(content, "max_bounty_points")
            .unwrap_or(economics::MAX_BOUNTY_POINTS);
        let max_daily = Self::extract_yaml_u64(content, "max_daily_bounties")
            .unwrap_or(economics::MAX_DAILY_BOUNTIES_PER_OPERATOR);

        let mut multipliers = HashMap::new();
        multipliers.insert("infrastructure".into(), economics::MULTIPLIER_INFRA_BPS);
        multipliers.insert("bug_fix".into(), economics::MULTIPLIER_BUG_FIX_BPS);
        multipliers.insert("testing_qa".into(), economics::MULTIPLIER_TESTING_BPS);
        multipliers.insert("feature".into(), economics::MULTIPLIER_FEATURE_BPS);
        multipliers.insert("design".into(), economics::MULTIPLIER_DESIGN_BPS);
        multipliers.insert("content_marketing".into(), economics::MULTIPLIER_CONTENT_BPS);
        multipliers.insert("documentation".into(), economics::MULTIPLIER_DOCS_BPS);
        multipliers.insert("support".into(), economics::MULTIPLIER_SUPPORT_BPS);

        Ok(BountyParams {
            min_quality_score: min_quality,
            max_bounty_points: max_points,
            max_daily_bounties: max_daily,
            reviewer_reward_bps: economics::REVIEWER_REWARD_BPS,
            contribution_multipliers: multipliers,
        })
    }

    fn parse_emission_schedule(content: &str) -> Result<EmissionSchedule, String> {
        let initial = Self::extract_yaml_u64(content, "initial_daily_emission")
            .unwrap_or(economics::INITIAL_DAILY_EMISSION);
        let halving = Self::extract_yaml_u64(content, "halving_interval")
            .unwrap_or(economics::HALVING_INTERVAL_DAYS);
        let minimum = Self::extract_yaml_u64(content, "minimum_daily_emission")
            .unwrap_or(economics::MINIMUM_DAILY_EMISSION);
        let max_epochs = Self::extract_yaml_u64(content, "max_halving_epochs")
            .unwrap_or(economics::MAX_HALVING_EPOCHS);

        Ok(EmissionSchedule {
            initial_daily_emission: initial,
            halving_interval_days: halving,
            minimum_daily_emission: minimum,
            max_halving_epochs: max_epochs,
        })
    }

    // ── Utility: extract numeric values from pseudo-YAML in markdown ───

    fn extract_yaml_u64(content: &str, key: &str) -> Option<u64> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(&format!("{key}:")) || trimmed.starts_with(&format!("  {key}:"))
            {
                // Extract the numeric part, handling comments and formatting
                let value_part = trimmed.split(':').nth(1)?;
                let cleaned = value_part
                    .split('#')
                    .next()?
                    .trim()
                    .replace(',', "")
                    .replace("\"", "");
                return cleaned.parse::<u64>().ok();
            }
        }
        None
    }

    fn extract_nested_yaml_u64(content: &str, parent_key: &str, child_key: &str) -> Option<u64> {
        // Find lines matching "parent_key: { ... child_key: VALUE ... }"
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with(&format!("{parent_key}:")) && trimmed.contains(child_key) {
                // Inline format: key: { child: value, ... }
                let brace_content = trimmed.split('{').nth(1)?.split('}').next()?;
                for pair in brace_content.split(',') {
                    let parts: Vec<&str> = pair.split(':').collect();
                    if parts.len() == 2 && parts[0].trim() == child_key {
                        let cleaned = parts[1].trim().replace(',', "");
                        return cleaned.parse::<u64>().ok();
                    }
                }
            }
        }
        None
    }
}

/// Trait for providing protocol context to autonomous agents.
pub trait ContextProvider: Send + Sync {
    /// Get the full agent context.
    fn agent_context(&self) -> &AgentContext;

    /// Get daily bounty limit for a trust level.
    fn daily_bounty_limit(&self, trust_level: u8) -> u32;

    /// Get system prompt snippet with protocol parameters.
    fn protocol_prompt(&self) -> String;
}

/// Default context provider that loads from a file.
pub struct FileContextProvider {
    context: AgentContext,
}

impl FileContextProvider {
    /// Load context from the given path. Falls back to economics.rs defaults
    /// if the file is not found.
    pub fn new(path: &Path) -> Self {
        let context = match AgentContext::load(path) {
            Ok(ctx) => {
                info!(path = %path.display(), "Agent context loaded and validated");
                ctx
            }
            Err(e) => {
                warn!("Failed to load agent context: {e}. Using economics.rs defaults.");
                Self::defaults_from_economics()
            }
        };
        Self { context }
    }

    /// Build a context purely from economics.rs constants (no file needed).
    fn defaults_from_economics() -> AgentContext {
        let mut trust_levels = HashMap::new();
        trust_levels.insert(
            1,
            TrustLevel {
                max_points: 100,
                daily_bounty_limit: 3,
            },
        );
        trust_levels.insert(
            2,
            TrustLevel {
                max_points: 200,
                daily_bounty_limit: 5,
            },
        );
        trust_levels.insert(
            3,
            TrustLevel {
                max_points: 500,
                daily_bounty_limit: 10,
            },
        );
        trust_levels.insert(
            4,
            TrustLevel {
                max_points: 1000,
                daily_bounty_limit: 15,
            },
        );
        trust_levels.insert(
            5,
            TrustLevel {
                max_points: 2000,
                daily_bounty_limit: 25,
            },
        );

        let mut multipliers = HashMap::new();
        multipliers.insert("infrastructure".into(), economics::MULTIPLIER_INFRA_BPS);
        multipliers.insert("bug_fix".into(), economics::MULTIPLIER_BUG_FIX_BPS);
        multipliers.insert("testing_qa".into(), economics::MULTIPLIER_TESTING_BPS);
        multipliers.insert("feature".into(), economics::MULTIPLIER_FEATURE_BPS);
        multipliers.insert("design".into(), economics::MULTIPLIER_DESIGN_BPS);
        multipliers.insert("content_marketing".into(), economics::MULTIPLIER_CONTENT_BPS);
        multipliers.insert("documentation".into(), economics::MULTIPLIER_DOCS_BPS);
        multipliers.insert("support".into(), economics::MULTIPLIER_SUPPORT_BPS);

        AgentContext {
            token_params: TokenParams {
                total_supply: economics::TOTAL_SUPPLY,
                treasury_allocation: economics::TREASURY_ALLOCATION,
                reserve_allocation: economics::RESERVE_ALLOCATION,
            },
            trust_levels: TrustConfig {
                max_level: 5,
                levels: trust_levels,
            },
            bounty_params: BountyParams {
                min_quality_score: economics::MIN_QUALITY_SCORE,
                max_bounty_points: economics::MAX_BOUNTY_POINTS,
                max_daily_bounties: economics::MAX_DAILY_BOUNTIES_PER_OPERATOR,
                reviewer_reward_bps: economics::REVIEWER_REWARD_BPS,
                contribution_multipliers: multipliers,
            },
            emission_schedule: EmissionSchedule {
                initial_daily_emission: economics::INITIAL_DAILY_EMISSION,
                halving_interval_days: economics::HALVING_INTERVAL_DAYS,
                minimum_daily_emission: economics::MINIMUM_DAILY_EMISSION,
                max_halving_epochs: economics::MAX_HALVING_EPOCHS,
            },
            bounty_lifecycle: vec![
                "DISCOVER".into(),
                "ASSESS".into(),
                "CLAIM".into(),
                "EXECUTE".into(),
                "SUBMIT".into(),
                "VERIFY".into(),
                "EARN".into(),
                "REPEAT".into(),
            ],
        }
    }
}

impl ContextProvider for FileContextProvider {
    fn agent_context(&self) -> &AgentContext {
        &self.context
    }

    fn daily_bounty_limit(&self, trust_level: u8) -> u32 {
        self.context.daily_bounty_limit(trust_level)
    }

    fn protocol_prompt(&self) -> String {
        self.context.to_system_prompt()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_from_economics_validates() {
        let ctx = FileContextProvider::defaults_from_economics();
        assert!(ctx.validate().is_ok());
    }

    #[test]
    fn token_params_match_economics() {
        let ctx = FileContextProvider::defaults_from_economics();
        assert_eq!(ctx.token_params.total_supply, 100_000_000);
        assert_eq!(ctx.token_params.treasury_allocation, 95_000_000);
        assert_eq!(ctx.token_params.reserve_allocation, 5_000_000);
    }

    #[test]
    fn trust_levels_are_complete() {
        let ctx = FileContextProvider::defaults_from_economics();
        assert_eq!(ctx.trust_levels.max_level, 5);
        assert_eq!(ctx.trust_levels.levels.len(), 5);
        assert_eq!(ctx.trust_levels.levels[&1].daily_bounty_limit, 3);
        assert_eq!(ctx.trust_levels.levels[&5].daily_bounty_limit, 25);
    }

    #[test]
    fn bounty_params_match_economics() {
        let ctx = FileContextProvider::defaults_from_economics();
        assert_eq!(ctx.bounty_params.min_quality_score, 30);
        assert_eq!(ctx.bounty_params.max_bounty_points, 2000);
    }

    #[test]
    fn daily_bounty_limit_lookup() {
        let ctx = FileContextProvider::defaults_from_economics();
        assert_eq!(ctx.daily_bounty_limit(1), 3);
        assert_eq!(ctx.daily_bounty_limit(3), 10);
        assert_eq!(ctx.daily_bounty_limit(5), 25);
        assert_eq!(ctx.daily_bounty_limit(99), 3); // unknown = default
    }

    #[test]
    fn system_prompt_generation() {
        let ctx = FileContextProvider::defaults_from_economics();
        let prompt = ctx.to_system_prompt();
        assert!(prompt.contains("100000000"));
        assert!(prompt.contains("95000000"));
        assert!(prompt.contains("DISCOVER"));
    }

    #[test]
    fn parse_yaml_u64_extracts_values() {
        let content = "  total_supply: 100,000,000  # Fixed.\n  bounty_treasury: 95,000,000";
        assert_eq!(AgentContext::extract_yaml_u64(content, "total_supply"), Some(100_000_000));
        assert_eq!(AgentContext::extract_yaml_u64(content, "bounty_treasury"), Some(95_000_000));
        assert_eq!(AgentContext::extract_yaml_u64(content, "nonexistent"), None);
    }

    #[test]
    fn parse_nested_yaml_extracts_values() {
        let content = "  level_1: { max_points: 100,   daily_bounty_limit: 3  }";
        assert_eq!(
            AgentContext::extract_nested_yaml_u64(content, "level_1", "max_points"),
            Some(100)
        );
        assert_eq!(
            AgentContext::extract_nested_yaml_u64(content, "level_1", "daily_bounty_limit"),
            Some(3)
        );
    }

    #[test]
    fn parse_from_agent_context_content() {
        // Minimal AGENT_CONTEXT.md content for parsing
        let content = r#"
## 2. Token Parameters
```yaml
total_supply: 100,000,000
allocation:
  bounty_treasury: 95,000,000
  emergency_reserve: 5,000,000
```

## 5. Trust System
```yaml
level_parameters:
  level_1: { max_points: 100,   daily_bounty_limit: 3  }
  level_2: { max_points: 200,   daily_bounty_limit: 5  }
  level_3: { max_points: 500,   daily_bounty_limit: 10 }
  level_4: { max_points: 1000,  daily_bounty_limit: 15 }
  level_5: { max_points: 2000,  daily_bounty_limit: 25 }
```

## 6. Bounty System
```yaml
min_quality_score: 30
max_bounty_points: 2000
max_daily_bounties: 50
initial_daily_emission: 16,000
halving_interval: 365
minimum_daily_emission: 100
max_halving_epochs: 10
```
"#;
        let ctx = AgentContext::parse(content).unwrap();
        assert_eq!(ctx.token_params.total_supply, 100_000_000);
        assert!(ctx.validate().is_ok());
    }

    #[test]
    fn validation_fails_on_mismatch() {
        let mut ctx = FileContextProvider::defaults_from_economics();
        ctx.token_params.total_supply = 999;
        assert!(ctx.validate().is_err());
    }
}
