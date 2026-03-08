//! Customer billing and subscription management.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Customer account record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub organization: Option<String>,
    pub plan: Plan,
    pub created_at: DateTime<Utc>,
    pub harness_id: Option<String>,
}

/// Subscription plan tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plan {
    /// Free tier: limited features for evaluation.
    Free,
    /// Starter: $99/month, suitable for small teams.
    Starter,
    /// Growth: $499/month, suitable for growing companies.
    Growth,
    /// Enterprise: custom pricing, full features + support.
    Enterprise,
}

impl Plan {
    /// Get the resource limits for this plan.
    pub fn limits(&self) -> PlanLimits {
        match self {
            Plan::Free => PlanLimits {
                max_conversations_per_month: 100,
                max_bots: 1,
                max_integrations: 2,
                storage_gb: 1,
                max_users: 1,
                support_level: "community".into(),
            },
            Plan::Starter => PlanLimits {
                max_conversations_per_month: 5_000,
                max_bots: 5,
                max_integrations: 10,
                storage_gb: 10,
                max_users: 5,
                support_level: "email".into(),
            },
            Plan::Growth => PlanLimits {
                max_conversations_per_month: 50_000,
                max_bots: 20,
                max_integrations: 50,
                storage_gb: 100,
                max_users: 25,
                support_level: "priority".into(),
            },
            Plan::Enterprise => PlanLimits {
                max_conversations_per_month: u64::MAX,
                max_bots: u64::MAX,
                max_integrations: u64::MAX,
                storage_gb: u64::MAX,
                max_users: u64::MAX,
                support_level: "dedicated".into(),
            },
        }
    }

    /// Monthly price in cents (USD).
    pub fn monthly_price_cents(&self) -> u64 {
        match self {
            Plan::Free => 0,
            Plan::Starter => 99_00,
            Plan::Growth => 499_00,
            Plan::Enterprise => 0, // Custom pricing
        }
    }
}

/// Resource limits for a subscription plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanLimits {
    pub max_conversations_per_month: u64,
    pub max_bots: u64,
    pub max_integrations: u64,
    pub storage_gb: u64,
    pub max_users: u64,
    pub support_level: String,
}

/// Active subscription record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub plan: Plan,
    pub status: SubscriptionStatus,
    pub started_at: DateTime<Utc>,
    pub current_period_start: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub cancel_at: Option<DateTime<Utc>>,
}

/// Subscription status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    PastDue,
    Canceled,
    Trialing,
}

/// Usage metrics for billing period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMetrics {
    /// Total conversations in current billing period.
    pub conversations: u64,
    /// Total tokens processed (input + output).
    pub tokens_used: u64,
    /// Currently running bots.
    pub bots_running: u64,
    /// Storage used in GB.
    pub storage_used_gb: u64,
}

impl UsageMetrics {
    /// Check if usage exceeds plan limits.
    pub fn exceeds_limits(&self, limits: &PlanLimits) -> Vec<String> {
        let mut violations = Vec::new();

        if self.conversations > limits.max_conversations_per_month {
            violations.push(format!(
                "Conversations: {} > {} limit",
                self.conversations, limits.max_conversations_per_month
            ));
        }

        if self.bots_running > limits.max_bots {
            violations.push(format!(
                "Bots: {} > {} limit",
                self.bots_running, limits.max_bots
            ));
        }

        if self.storage_used_gb > limits.storage_gb {
            violations.push(format!(
                "Storage: {} GB > {} GB limit",
                self.storage_used_gb, limits.storage_gb
            ));
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_limits_are_progressive() {
        let free = Plan::Free.limits();
        let starter = Plan::Starter.limits();
        let growth = Plan::Growth.limits();

        assert!(starter.max_conversations_per_month > free.max_conversations_per_month);
        assert!(growth.max_conversations_per_month > starter.max_conversations_per_month);
    }

    #[test]
    fn usage_violations_detected() {
        let limits = Plan::Starter.limits();
        let usage = UsageMetrics {
            conversations: 10_000, // Exceeds 5,000 limit
            tokens_used: 1_000_000,
            bots_running: 3,
            storage_used_gb: 5,
        };

        let violations = usage.exceeds_limits(&limits);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("Conversations"));
    }
}
