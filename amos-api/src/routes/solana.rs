use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;

/// Treasury state response
#[derive(Debug, Serialize)]
pub struct TreasuryState {
    /// Treasury wallet address
    pub address: String,

    /// USDC balance in treasury
    pub usdc_balance: f64,

    /// AMOS token balance
    pub amos_balance: f64,

    /// Total value locked (USD)
    pub tvl: f64,

    /// Last updated timestamp
    pub last_updated: String,

    /// On-chain program ID
    pub program_id: String,
}

/// Stake record response
#[derive(Debug, Serialize)]
pub struct StakeRecord {
    /// Wallet address
    pub wallet_address: String,

    /// Amount staked
    pub staked_amount: f64,

    /// Stake tier
    pub tier: String,

    /// Stake start date
    pub stake_start: String,

    /// Lock duration in days
    pub lock_duration_days: u32,

    /// Unlock date (if locked)
    pub unlock_date: Option<String>,

    /// Accumulated rewards
    pub accumulated_rewards: f64,

    /// Last reward claim
    pub last_claim: Option<String>,
}

/// Wallet verification request
#[derive(Debug, Deserialize)]
pub struct VerifyWalletRequest {
    /// Wallet public key
    pub wallet_address: String,

    /// Message that was signed
    pub message: String,

    /// Ed25519 signature
    pub signature: String,
}

/// Wallet verification response
#[derive(Debug, Serialize)]
pub struct VerifyWalletResponse {
    /// Whether signature is valid
    pub valid: bool,

    /// Wallet address (if valid)
    pub wallet_address: Option<String>,

    /// Error message (if invalid)
    pub error: Option<String>,
}

/// Treasury state handler
/// Returns current on-chain treasury state
pub async fn treasury_state_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TreasuryState>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Treasury state request");

    // TODO: Connect to Solana RPC
    // TODO: Fetch real treasury state from on-chain program
    // TODO: Cache result in Redis for performance

    // Check cache first
    // let cache_key = "solana:treasury_state";
    // if let Ok(cached) = get_from_redis_cache(&state.redis, cache_key).await {
    //     return Ok(Json(cached));
    // }

    // Placeholder response
    let treasury_state = TreasuryState {
        address: "AmosT1eAsUrY111111111111111111111111111111".to_string(),
        usdc_balance: 1_500_000.0,
        amos_balance: 50_000_000.0,
        tvl: 2_500_000.0,
        last_updated: chrono::Utc::now().to_rfc3339(),
        program_id: "AmosProg1Am111111111111111111111111111111".to_string(),
    };

    // TODO: Cache result
    // cache_in_redis(&state.redis, cache_key, &treasury_state, 60).await?;

    Ok(Json(treasury_state))
}

/// Stake record handler
/// Returns on-chain stake information for a wallet
pub async fn stake_record_handler(
    State(state): State<Arc<AppState>>,
    Path(wallet): Path<String>,
) -> Result<Json<StakeRecord>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Stake record request: wallet={}", wallet);

    // Validate wallet address format (basic check)
    if wallet.len() < 32 || wallet.len() > 44 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid wallet address format"})),
        ));
    }

    // TODO: Connect to Solana RPC
    // TODO: Fetch stake account from on-chain program
    // TODO: Parse stake data

    // Check cache
    // let cache_key = format!("solana:stake:{}", wallet);
    // if let Ok(cached) = get_from_redis_cache(&state.redis, &cache_key).await {
    //     return Ok(Json(cached));
    // }

    // Placeholder response
    let stake_record = StakeRecord {
        wallet_address: wallet.clone(),
        staked_amount: 10_000.0,
        tier: "Silver".to_string(),
        stake_start: chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::days(30))
            .unwrap()
            .to_rfc3339(),
        lock_duration_days: 90,
        unlock_date: Some(
            chrono::Utc::now()
                .checked_add_signed(chrono::Duration::days(60))
                .unwrap()
                .to_rfc3339(),
        ),
        accumulated_rewards: 250.0,
        last_claim: Some(
            chrono::Utc::now()
                .checked_sub_signed(chrono::Duration::days(7))
                .unwrap()
                .to_rfc3339(),
        ),
    };

    // TODO: Cache result
    // cache_in_redis(&state.redis, &cache_key, &stake_record, 300).await?;

    Ok(Json(stake_record))
}

/// Verify wallet handler
/// Verifies wallet ownership via Ed25519 signature
pub async fn verify_wallet_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<VerifyWalletRequest>,
) -> Result<Json<VerifyWalletResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Verify wallet request: wallet={}", request.wallet_address);

    // TODO: Implement actual Ed25519 signature verification
    // TODO: Verify message format and timestamp
    // TODO: Check signature against public key

    // Placeholder verification logic
    let valid = verify_signature(&request.wallet_address, &request.message, &request.signature);

    let response = if valid {
        VerifyWalletResponse {
            valid: true,
            wallet_address: Some(request.wallet_address),
            error: None,
        }
    } else {
        VerifyWalletResponse {
            valid: false,
            wallet_address: None,
            error: Some("Invalid signature".to_string()),
        }
    };

    Ok(Json(response))
}

/// Stub function for signature verification
/// TODO: Implement with ed25519-dalek or similar
fn verify_signature(_wallet: &str, _message: &str, _signature: &str) -> bool {
    // Placeholder: always return true for now
    // In production, this should:
    // 1. Decode base58 wallet address to get public key
    // 2. Decode base58 signature
    // 3. Verify Ed25519 signature against message and public key
    // 4. Check message timestamp to prevent replay attacks
    tracing::warn!("Signature verification not yet implemented - returning true");
    true
}

// Helper functions for Redis caching (to be implemented)
// async fn get_from_redis_cache<T: serde::de::DeserializeOwned>(
//     redis: &redis::Client,
//     key: &str,
// ) -> Result<T, Box<dyn std::error::Error>> {
//     let mut conn = redis.get_connection()?;
//     let data: String = redis::cmd("GET").arg(key).query(&mut conn)?;
//     let result: T = serde_json::from_str(&data)?;
//     Ok(result)
// }

// async fn cache_in_redis<T: serde::Serialize>(
//     redis: &redis::Client,
//     key: &str,
//     value: &T,
//     ttl_seconds: usize,
// ) -> Result<(), Box<dyn std::error::Error>> {
//     let mut conn = redis.get_connection()?;
//     let serialized = serde_json::to_string(value)?;
//     redis::cmd("SETEX")
//         .arg(key)
//         .arg(ttl_seconds)
//         .arg(serialized)
//         .query(&mut conn)?;
//     Ok(())
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_wallet_request_deserialize() {
        let json = r#"{
            "wallet_address": "AmosWa11et1111111111111111111111111111111",
            "message": "Sign this message to verify ownership",
            "signature": "base58sighere"
        }"#;
        let request: VerifyWalletRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.wallet_address, "AmosWa11et1111111111111111111111111111111");
    }

    #[test]
    fn test_treasury_state_serialize() {
        let state = TreasuryState {
            address: "test".to_string(),
            usdc_balance: 1000.0,
            amos_balance: 5000.0,
            tvl: 2000.0,
            last_updated: "2026-03-04T00:00:00Z".to_string(),
            program_id: "prog123".to_string(),
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("usdc_balance"));
    }

    #[test]
    fn test_stake_record_serialize() {
        let record = StakeRecord {
            wallet_address: "wallet123".to_string(),
            staked_amount: 1000.0,
            tier: "Silver".to_string(),
            stake_start: "2026-01-01T00:00:00Z".to_string(),
            lock_duration_days: 90,
            unlock_date: Some("2026-04-01T00:00:00Z".to_string()),
            accumulated_rewards: 50.0,
            last_claim: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("staked_amount"));
    }
}
