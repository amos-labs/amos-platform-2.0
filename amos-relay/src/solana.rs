//! Solana RPC client for bounty settlement and fee distribution.

use amos_core::{AmosError, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};
use std::str::FromStr;
use std::sync::Arc;

/// Wrapper around Solana RPC client for relay operations.
pub struct SolanaClient {
    rpc: Arc<RpcClient>,
    rpc_url: String,
    /// Bounty program ID
    pub bounty_program_id: Pubkey,
}

impl SolanaClient {
    /// Create a new Solana client connected to the given RPC endpoint.
    ///
    /// # Arguments
    /// * `rpc_url` - Solana RPC endpoint URL
    /// * `bounty_program_id` - Program ID for the bounty program
    pub fn new(rpc_url: &str, bounty_program_id: &str) -> Result<Self> {
        let rpc = RpcClient::new_with_commitment(
            rpc_url.to_string(),
            CommitmentConfig::confirmed(),
        );

        let bounty_program_id = Pubkey::from_str(bounty_program_id)
            .map_err(|e| AmosError::SolanaRpc(format!("Invalid bounty program ID: {}", e)))?;

        Ok(Self {
            rpc: Arc::new(rpc),
            rpc_url: rpc_url.to_string(),
            bounty_program_id,
        })
    }

    /// Health check: verify RPC is reachable.
    pub async fn health_check(&self) -> Result<()> {
        // Spawn blocking since RPC client is sync
        let rpc_url = self.rpc_url.clone();
        tokio::task::spawn_blocking(move || {
            let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
            rpc.get_health()
                .map_err(|e| AmosError::SolanaRpc(format!("Health check failed: {}", e)))
        })
        .await
        .map_err(|e| AmosError::Internal(format!("Tokio join error: {}", e)))??;

        Ok(())
    }

    /// Process bounty payout on-chain.
    ///
    /// TODO: Implement this function to:
    /// 1. Build a transaction to transfer tokens from bounty escrow to agent wallet
    /// 2. Deduct protocol fee (3%)
    /// 3. Distribute fee to holder pool (70%), treasury (20%), and ops/burn (10%)
    /// 4. Sign and submit the transaction
    ///
    /// # Arguments
    /// * `bounty_id` - Unique bounty identifier
    /// * `agent_wallet` - Agent's receiving wallet address
    /// * `reward_amount` - Total reward amount in tokens
    /// * `protocol_fee` - Protocol fee amount to deduct
    pub async fn process_bounty_payout(
        &self,
        _bounty_id: &str,
        _agent_wallet: &str,
        _reward_amount: u64,
        _protocol_fee: u64,
    ) -> Result<String> {
        // TODO: Implement on-chain settlement
        Ok("TODO_TRANSACTION_SIGNATURE".to_string())
    }

    /// Burn protocol fees (ops/burn share).
    ///
    /// TODO: Implement this function to:
    /// 1. Build a transaction to burn tokens from the ops pool
    /// 2. Sign and submit the transaction
    ///
    /// # Arguments
    /// * `amount` - Amount of tokens to burn
    pub async fn burn_protocol_fees(&self, _amount: u64) -> Result<String> {
        // TODO: Implement token burning
        Ok("TODO_BURN_SIGNATURE".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solana_client_can_be_created() {
        let client = SolanaClient::new(
            "https://api.devnet.solana.com",
            "11111111111111111111111111111111", // placeholder program ID
        );
        assert!(client.is_ok());
    }

    #[test]
    fn test_invalid_program_id() {
        let client = SolanaClient::new("https://api.devnet.solana.com", "invalid_pubkey");
        assert!(client.is_err());
    }
}
