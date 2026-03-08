use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;
use amos_core::token::decay::{
    apply_daily_decay, calculate_dynamic_decay_rate, PlatformEconomics, StakeContext, VaultTier,
};
use amos_core::token::emission::daily_emission_for_day;
use amos_core::token::revenue::split_usdc_revenue;

/// Token economy statistics response
#[derive(Debug, Serialize)]
pub struct TokenStats {
    /// Total supply of AMOS tokens
    pub total_supply: f64,

    /// Circulating supply
    pub circulating_supply: f64,

    /// Total tokens burned
    pub total_burned: f64,

    /// Current decay rate (percentage)
    pub current_decay_rate: f64,

    /// Total tokens staked
    pub total_staked: f64,

    /// Current epoch
    pub current_epoch: u64,

    /// Treasury balance
    pub treasury_balance: f64,
}

/// Decay rate response with explanation
#[derive(Debug, Serialize)]
pub struct DecayRateResponse {
    /// Current dynamic decay rate
    pub decay_rate: f64,

    /// Explanation of how rate was calculated
    pub explanation: String,

    /// Components that influenced the rate
    pub components: DecayRateComponents,
}

#[derive(Debug, Serialize)]
pub struct DecayRateComponents {
    /// Base decay rate
    pub base_rate: f64,

    /// Platform economics
    pub monthly_revenue_cents: u64,
    pub monthly_costs_cents: u64,
}

/// Request body for calculate-decay endpoint
#[derive(Debug, Deserialize)]
pub struct CalculateDecayRequest {
    /// Current balance
    pub current_balance: u64,
    /// Original balance at time of award
    pub original_balance: u64,
    /// Days since stake was first registered
    pub tenure_days: u64,
    /// Vault tier: "none", "bronze", "silver", "gold", "permanent"
    #[serde(default)]
    pub vault_tier: String,
    /// Days since last platform activity
    #[serde(default)]
    pub days_inactive: u64,
}

/// Decay result response (serializable version of core DecayResult)
#[derive(Debug, Serialize)]
pub struct DecayResultResponse {
    /// Tokens removed this cycle
    pub tokens_decayed: u64,
    /// Tokens burned (portion of decayed)
    pub tokens_burned: u64,
    /// Tokens recycled back to treasury
    pub tokens_recycled: u64,
    /// New balance after decay
    pub new_balance: u64,
    /// The effective annual decay rate that was applied (bps)
    pub effective_rate_bps: u64,
    /// Whether decay was skipped
    pub skipped: bool,
    /// Reason if skipped
    pub skip_reason: Option<String>,
}

/// Request body for revenue-split endpoint
#[derive(Debug, Deserialize)]
pub struct RevenueSplitRequest {
    /// Amount of USDC to distribute (in cents)
    pub amount_cents: u64,
}

/// Revenue distribution response (serializable version)
#[derive(Debug, Serialize)]
pub struct UsdcRevenueDistributionResponse {
    /// Total amount received
    pub total_amount: u64,
    /// 50% to holder pool
    pub holder_amount: u64,
    /// 40% to R&D multisig
    pub rnd_amount: u64,
    /// 5% to operations multisig
    pub ops_amount: u64,
    /// 5% to reserve
    pub reserve_amount: u64,
}

/// Daily emission response (serializable version)
#[derive(Debug, Serialize)]
pub struct DailyEmissionResponse {
    /// Day index (0-based from program start)
    pub day_index: u64,
    /// Current halving epoch
    pub halving_epoch: u64,
    /// Total AMOS available for distribution today
    pub emission: u64,
}

/// Token statistics handler
/// Returns overview of the token economy
pub async fn stats_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TokenStats>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Token stats request");

    // TODO: Fetch real data from database and Solana
    // For now, return placeholder values

    let stats = TokenStats {
        total_supply: 1_000_000_000.0,
        circulating_supply: 500_000_000.0,
        total_burned: 50_000_000.0,
        current_decay_rate: 0.0001,
        total_staked: 200_000_000.0,
        current_epoch: 42,
        treasury_balance: 1_500_000.0,
    };

    Ok(Json(stats))
}

/// Decay rate handler
/// Returns current dynamic decay rate with explanation
pub async fn decay_rate_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DecayRateResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Decay rate request");

    // TODO: Get current platform economics from database
    // For now, use placeholder values
    let economics = PlatformEconomics {
        monthly_revenue_cents: 100_000_00, // $100k
        monthly_costs_cents: 85_000_00,    // $85k
    };

    // Calculate dynamic decay rate
    let decay_rate_bps = calculate_dynamic_decay_rate(&economics);
    let decay_rate = decay_rate_bps as f64 / 10_000.0;

    let response = DecayRateResponse {
        decay_rate,
        explanation: format!(
            "Decay rate calculated based on monthly revenue (${:.2}) and costs (${:.2})",
            economics.monthly_revenue_cents as f64 / 100.0,
            economics.monthly_costs_cents as f64 / 100.0
        ),
        components: DecayRateComponents {
            base_rate: 0.10,
            monthly_revenue_cents: economics.monthly_revenue_cents,
            monthly_costs_cents: economics.monthly_costs_cents,
        },
    };

    Ok(Json(response))
}

/// Parse vault tier from string
fn parse_vault_tier(tier_str: &str) -> VaultTier {
    match tier_str.to_lowercase().as_str() {
        "bronze" => VaultTier::Bronze,
        "silver" => VaultTier::Silver,
        "gold" => VaultTier::Gold,
        "permanent" => VaultTier::Permanent,
        _ => VaultTier::None,
    }
}

/// Calculate decay handler
/// Calculates decay for a given stake context
pub async fn calculate_decay_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CalculateDecayRequest>,
) -> Result<Json<DecayResultResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Calculate decay request: balance={}", request.current_balance);

    // Validate balance
    if request.current_balance == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Balance must be positive"})),
        ));
    }

    // Build stake context from request
    let context = StakeContext {
        tenure_days: request.tenure_days,
        current_balance: request.current_balance,
        original_balance: request.original_balance,
        vault_tier: parse_vault_tier(&request.vault_tier),
        days_inactive: request.days_inactive,
    };

    // TODO: Get current platform economics from database
    let economics = PlatformEconomics {
        monthly_revenue_cents: 100_000_00,
        monthly_costs_cents: 85_000_00,
    };

    let base_annual_rate_bps = calculate_dynamic_decay_rate(&economics);

    // Apply daily decay
    let result = apply_daily_decay(base_annual_rate_bps, &context);

    // Convert to response type
    let response = DecayResultResponse {
        tokens_decayed: result.tokens_decayed,
        tokens_burned: result.tokens_burned,
        tokens_recycled: result.tokens_recycled,
        new_balance: result.new_balance,
        effective_rate_bps: result.effective_rate_bps,
        skipped: result.skipped,
        skip_reason: result.skip_reason,
    };

    Ok(Json(response))
}

/// Revenue split handler
/// Calculates revenue distribution for a given USDC amount
pub async fn revenue_split_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RevenueSplitRequest>,
) -> Result<Json<UsdcRevenueDistributionResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Revenue split request: amount={}", request.amount_cents);

    // Validate amount
    if request.amount_cents == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Amount must be positive"})),
        ));
    }

    // Calculate revenue distribution
    let distribution = split_usdc_revenue(request.amount_cents).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    // Convert to response type
    let response = UsdcRevenueDistributionResponse {
        total_amount: distribution.total_amount,
        holder_amount: distribution.holder_amount,
        rnd_amount: distribution.rnd_amount,
        ops_amount: distribution.ops_amount,
        reserve_amount: distribution.reserve_amount,
    };

    Ok(Json(response))
}

/// Emission handler
/// Returns current daily emission information
pub async fn emission_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DailyEmissionResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Emission request");

    // TODO: Get current day index from database or calculate from program start
    // For now, use day 0 (first day)
    let day_index = 0;

    let emission = daily_emission_for_day(day_index);

    // Convert to response type
    let response = DailyEmissionResponse {
        day_index: emission.day_index,
        halving_epoch: emission.halving_epoch,
        emission: emission.emission,
    };

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_decay_request_deserialize() {
        let json = r#"{
            "current_balance": 1000,
            "original_balance": 1000,
            "tenure_days": 400
        }"#;
        let request: CalculateDecayRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.current_balance, 1000);
        assert_eq!(request.tenure_days, 400);
    }

    #[test]
    fn test_revenue_split_request_deserialize() {
        let json = r#"{"amount_cents": 500000}"#;
        let request: RevenueSplitRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.amount_cents, 500000);
    }

    #[test]
    fn test_token_stats_serialize() {
        let stats = TokenStats {
            total_supply: 1_000_000_000.0,
            circulating_supply: 500_000_000.0,
            total_burned: 50_000_000.0,
            current_decay_rate: 0.0001,
            total_staked: 200_000_000.0,
            current_epoch: 42,
            treasury_balance: 1_500_000.0,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("total_supply"));
    }

    #[test]
    fn test_parse_vault_tier() {
        assert_eq!(parse_vault_tier("bronze"), VaultTier::Bronze);
        assert_eq!(parse_vault_tier("SILVER"), VaultTier::Silver);
        assert_eq!(parse_vault_tier("gold"), VaultTier::Gold);
        assert_eq!(parse_vault_tier("permanent"), VaultTier::Permanent);
        assert_eq!(parse_vault_tier("none"), VaultTier::None);
        assert_eq!(parse_vault_tier("invalid"), VaultTier::None);
    }
}
