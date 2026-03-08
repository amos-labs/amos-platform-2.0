//! Solana RPC client for on-chain integration.

use amos_core::{AmosError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

/// Wrapper around Solana RPC client.
pub struct SolanaClient {
    rpc: RpcClient,
    rpc_url: String,
}

impl SolanaClient {
    /// Create a new Solana client connected to the given RPC endpoint.
    pub fn new(rpc_url: &str) -> Result<Self> {
        let rpc = RpcClient::new_with_commitment(
            rpc_url.to_string(),
            CommitmentConfig::confirmed(),
        );

        Ok(Self {
            rpc,
            rpc_url: rpc_url.to_string(),
        })
    }

    /// Health check: verify RPC is reachable.
    pub async fn health_check(&self) -> Result<()> {
        // Spawn blocking since RPC client is sync
        let rpc_url = self.rpc_url.clone();
        tokio::task::spawn_blocking(move || {
            let rpc = RpcClient::new_with_commitment(
                rpc_url,
                CommitmentConfig::confirmed(),
            );
            rpc.get_health()
                .map_err(|e| AmosError::SolanaRpc(format!("Health check failed: {}", e)))
        })
        .await
        .map_err(|e| AmosError::Internal(format!("Tokio join error: {}", e)))??;

        Ok(())
    }

    /// Get the treasury state from on-chain program.
    ///
    /// TODO: Implement actual account deserialization.
    pub async fn get_treasury_state(&self) -> Result<TreasuryState> {
        // Placeholder implementation
        Ok(TreasuryState {
            total_supply: 100_000_000,
            circulating_supply: 40_000_000,
            treasury_balance: 60_000_000,
            holder_pool_usdc: 500_000_00, // $500k USDC
            total_staked: 15_000_000,
            current_emission_rate: 16_000,
            last_emission_day: 0,
        })
    }

    /// Get stake record for a wallet address.
    ///
    /// TODO: Implement actual PDA lookup and deserialization.
    pub async fn get_stake_record(&self, wallet: &str) -> Result<Option<StakeRecord>> {
        // Placeholder implementation
        Ok(None)
    }

    /// Get all active governance proposals.
    ///
    /// TODO: Implement actual program account querying.
    pub async fn get_governance_proposals(&self) -> Result<Vec<OnChainProposal>> {
        // Placeholder implementation
        Ok(vec![])
    }

    /// Submit a bounty proof transaction.
    ///
    /// TODO: Implement actual transaction building and signing.
    pub async fn submit_bounty_proof(
        &self,
        bounty_id: u64,
        contributor: &str,
        evidence_hash: [u8; 32],
    ) -> Result<String> {
        // Placeholder: return fake transaction signature
        Ok(format!(
            "{}...{}",
            &hex::encode(&evidence_hash[..4]),
            &hex::encode(&evidence_hash[28..])
        ))
    }
}

// PPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPP
// ON-CHAIN DATA TYPES
// PPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPP

/// Treasury program state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryState {
    pub total_supply: u64,
    pub circulating_supply: u64,
    pub treasury_balance: u64,
    pub holder_pool_usdc: u64,
    pub total_staked: u64,
    pub current_emission_rate: u64,
    pub last_emission_day: u64,
}

/// Individual stake record from on-chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeRecord {
    pub wallet: String,
    pub amount: u64,
    pub staked_at: DateTime<Utc>,
    pub last_decay_at: DateTime<Utc>,
    pub vault_tier: u8, // 0 = none, 1 = bronze, 2 = silver, 3 = gold, 4 = permanent
    pub delegated_to: Option<String>,
}

/// On-chain governance proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainProposal {
    pub proposal_id: u64,
    pub proposer: String,
    pub title_hash: [u8; 32],
    pub voting_starts_at: i64,
    pub voting_ends_at: i64,
    pub votes_for: u64,
    pub votes_against: u64,
    pub executed: bool,
}

/// Bounty submission proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BountyProof {
    pub bounty_id: u64,
    pub contributor: String,
    pub contribution_type: u8,
    pub points: u64,
    pub evidence_hash: [u8; 32],
    pub reviewer: String,
    pub submitted_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solana_client_can_be_created() {
        let client = SolanaClient::new("https://api.devnet.solana.com");
        assert!(client.is_ok());
    }
}
