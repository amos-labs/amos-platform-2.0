//! Solana RPC client for bounty settlement and fee distribution.
//!
//! Connects to devnet (or mainnet) and submits `submit_bounty_proof` transactions
//! to the on-chain AMOS Bounty Program, completing the economic loop:
//! relay approves bounty → on-chain token distribution → agent receives tokens.

use amos_core::{AmosError, Result};
use sha2::{Digest, Sha256};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program::ID as SYSTEM_PROGRAM_ID,
    transaction::Transaction,
};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};

// Well-known program IDs
const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const SPL_ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

// PDA seeds (must match amos-solana/programs/amos-bounty/src/constants.rs)
const BOUNTY_CONFIG_SEED: &[u8] = b"bounty_config";
const DAILY_POOL_SEED: &[u8] = b"daily_pool";
const BOUNTY_PROOF_SEED: &[u8] = b"bounty_proof";
const OPERATOR_STATS_SEED: &[u8] = b"operator_stats";
const AGENT_TRUST_SEED: &[u8] = b"agent_trust";

/// Wrapper around Solana RPC client for relay operations.
pub struct SolanaClient {
    rpc: Arc<RpcClient>,
    rpc_url: String,
    /// Bounty program ID
    pub bounty_program_id: Pubkey,
    /// Oracle keypair for signing settlement transactions
    oracle_keypair: Option<Keypair>,
    /// AMOS SPL token mint
    mint: Option<Pubkey>,
    /// Treasury token account
    treasury_token_account: Option<Pubkey>,
}

/// Result of a successful bounty settlement on-chain.
#[derive(Debug, Clone)]
pub struct SettlementResult {
    /// Solana transaction signature
    pub tx_signature: String,
    /// Tokens distributed to the operator
    pub operator_tokens: u64,
    /// Tokens distributed to the reviewer
    pub reviewer_tokens: u64,
}

/// Parameters for settling a bounty on-chain.
#[derive(Debug)]
pub struct SettlementParams {
    /// Unique bounty ID (will be hashed to [u8; 32])
    pub bounty_id: String,
    /// Agent's Solana wallet address (operator)
    pub agent_wallet: String,
    /// Reviewer's Solana wallet address
    pub reviewer_wallet: String,
    /// Base contribution points (derived from reward amount)
    pub base_points: u16,
    /// Quality score (0-100)
    pub quality_score: u8,
    /// Contribution type (0=bug_fix, 1=feature, etc.)
    pub contribution_type: u8,
    /// Whether the worker is an autonomous agent
    pub is_agent: bool,
    /// Agent ID bytes (for trust tracking)
    pub agent_id: [u8; 32],
    /// SHA-256 hash of the submission evidence
    pub evidence_hash: [u8; 32],
}

impl SolanaClient {
    /// Create a new Solana client connected to the given RPC endpoint.
    pub fn new(rpc_url: &str, bounty_program_id: &str) -> Result<Self> {
        let rpc =
            RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());

        let bounty_program_id = Pubkey::from_str(bounty_program_id)
            .map_err(|e| AmosError::SolanaRpc(format!("Invalid bounty program ID: {}", e)))?;

        Ok(Self {
            rpc: Arc::new(rpc),
            rpc_url: rpc_url.to_string(),
            bounty_program_id,
            oracle_keypair: None,
            mint: None,
            treasury_token_account: None,
        })
    }

    /// Load the oracle keypair from a JSON file (Solana CLI format).
    pub fn load_oracle_keypair(&mut self, keypair_path: &str) -> Result<()> {
        let keypair_bytes = std::fs::read_to_string(keypair_path).map_err(|e| {
            AmosError::Internal(format!(
                "Failed to read oracle keypair at '{}': {}",
                keypair_path, e
            ))
        })?;

        let bytes: Vec<u8> = serde_json::from_str(&keypair_bytes).map_err(|e| {
            AmosError::Internal(format!("Invalid keypair JSON format: {}", e))
        })?;

        self.oracle_keypair = Some(
            Keypair::try_from(bytes.as_slice())
                .map_err(|e| AmosError::Internal(format!("Invalid keypair bytes: {}", e)))?,
        );

        info!(
            oracle = %self.oracle_keypair.as_ref().unwrap().pubkey(),
            "Oracle keypair loaded"
        );
        Ok(())
    }

    /// Set the AMOS token mint address.
    pub fn set_mint(&mut self, mint_address: &str) -> Result<()> {
        self.mint = Some(
            Pubkey::from_str(mint_address)
                .map_err(|e| AmosError::Internal(format!("Invalid mint address: {}", e)))?,
        );
        Ok(())
    }

    /// Set the treasury token account.
    pub fn set_treasury(&mut self, treasury_address: &str) -> Result<()> {
        self.treasury_token_account = Some(
            Pubkey::from_str(treasury_address)
                .map_err(|e| AmosError::Internal(format!("Invalid treasury address: {}", e)))?,
        );
        Ok(())
    }

    /// Health check: verify RPC is reachable.
    pub async fn health_check(&self) -> Result<()> {
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

    /// Check if settlement is fully configured (keypair + mint + treasury).
    pub fn is_settlement_ready(&self) -> bool {
        self.oracle_keypair.is_some()
            && self.mint.is_some()
            && self.treasury_token_account.is_some()
    }

    /// Process bounty payout on-chain via `submit_bounty_proof`.
    ///
    /// Builds and submits a transaction to the AMOS Bounty Program that:
    /// 1. Records the bounty proof on-chain
    /// 2. Distributes tokens from treasury to the agent (95%) and reviewer (5%)
    /// 3. Updates operator stats and agent trust records
    pub async fn process_bounty_payout(
        &self,
        params: &SettlementParams,
    ) -> Result<SettlementResult> {
        let oracle = self.oracle_keypair.as_ref().ok_or_else(|| {
            AmosError::Internal("Oracle keypair not configured — cannot settle bounties".into())
        })?;
        let mint = self.mint.ok_or_else(|| {
            AmosError::Internal("Mint address not configured".into())
        })?;
        let treasury = self.treasury_token_account.ok_or_else(|| {
            AmosError::Internal("Treasury token account not configured".into())
        })?;

        let operator = Pubkey::from_str(&params.agent_wallet).map_err(|e| {
            AmosError::Validation(format!("Invalid agent wallet: {}", e))
        })?;
        let reviewer = Pubkey::from_str(&params.reviewer_wallet).map_err(|e| {
            AmosError::Validation(format!("Invalid reviewer wallet: {}", e))
        })?;

        let program_id = self.bounty_program_id;

        // Hash the bounty UUID to get a fixed 32-byte ID
        let bounty_id_bytes = hash_to_32_bytes(&params.bounty_id);

        // Derive all PDAs
        let (config_pda, _) =
            Pubkey::find_program_address(&[BOUNTY_CONFIG_SEED], &program_id);

        // Day index from config start_time — we fetch it from the config account
        // For now, use current unix timestamp / 86400 as approximate day index
        let day_index = (chrono::Utc::now().timestamp() / 86400) as u32;
        let (daily_pool_pda, _) = Pubkey::find_program_address(
            &[DAILY_POOL_SEED, &day_index.to_le_bytes()],
            &program_id,
        );

        let (bounty_proof_pda, _) =
            Pubkey::find_program_address(&[BOUNTY_PROOF_SEED, &bounty_id_bytes], &program_id);

        let (operator_stats_pda, _) =
            Pubkey::find_program_address(&[OPERATOR_STATS_SEED, operator.as_ref()], &program_id);

        // Agent trust record (only meaningful if is_agent)
        let (agent_trust_pda, _) =
            Pubkey::find_program_address(&[AGENT_TRUST_SEED, &params.agent_id], &program_id);

        // Derive associated token accounts for operator and reviewer
        let operator_ata = derive_associated_token_account(&operator, &mint);
        let reviewer_ata = derive_associated_token_account(&reviewer, &mint);

        let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).unwrap();

        // Build instruction data: 8-byte Anchor discriminator + borsh-serialized args
        let instruction_data = build_submit_bounty_proof_data(
            &bounty_id_bytes,
            params.base_points,
            params.quality_score,
            params.contribution_type,
            params.is_agent,
            &params.agent_id,
            &reviewer,
            &params.evidence_hash,
        );

        // Build account metas (order must match the Anchor context struct)
        let accounts = vec![
            AccountMeta::new(config_pda, false),
            AccountMeta::new(daily_pool_pda, false),
            AccountMeta::new(bounty_proof_pda, false),
            AccountMeta::new(operator_stats_pda, false),
            AccountMeta::new_readonly(operator, false),
            AccountMeta::new(agent_trust_pda, false), // UncheckedAccount, always passed
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(treasury, false),
            AccountMeta::new(operator_ata, false),
            AccountMeta::new(reviewer_ata, false),
            AccountMeta::new_readonly(oracle.pubkey(), true), // signer
            AccountMeta::new_readonly(token_program, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ];

        let instruction = Instruction {
            program_id,
            accounts,
            data: instruction_data,
        };

        // Build, sign, and send transaction
        let rpc = self.rpc.clone();
        let oracle_keypair_bytes = oracle.to_bytes();

        let tx_signature = tokio::task::spawn_blocking(move || {
            let oracle_kp = Keypair::try_from(oracle_keypair_bytes.as_slice())
                .map_err(|e| AmosError::Internal(format!("Keypair reconstruction: {}", e)))?;

            let recent_blockhash = rpc
                .get_latest_blockhash()
                .map_err(|e| AmosError::SolanaRpc(format!("Failed to get blockhash: {}", e)))?;

            let tx = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&oracle_kp.pubkey()),
                &[&oracle_kp],
                recent_blockhash,
            );

            let sig = rpc
                .send_and_confirm_transaction(&tx)
                .map_err(|e| AmosError::SolanaRpc(format!("Transaction failed: {}", e)))?;

            Ok::<String, AmosError>(sig.to_string())
        })
        .await
        .map_err(|e| AmosError::Internal(format!("Tokio join error: {}", e)))??;

        info!(
            bounty_id = %params.bounty_id,
            tx = %tx_signature,
            agent = %params.agent_wallet,
            "Bounty settlement transaction confirmed on-chain"
        );

        Ok(SettlementResult {
            tx_signature,
            operator_tokens: 0, // Actual amount determined by on-chain pool math
            reviewer_tokens: 0,
        })
    }

    /// Burn protocol fees (ops/burn share) by sending tokens to the burn address.
    pub async fn burn_protocol_fees(&self, amount: u64) -> Result<String> {
        if amount == 0 {
            return Ok("no_burn_needed".to_string());
        }

        // For the burn, we need the oracle to sign a token burn instruction
        // against the ops pool token account. For now, log and return a marker
        // indicating the burn is pending on-chain integration.
        warn!(
            amount,
            "Protocol fee burn not yet integrated — amount recorded in fee ledger"
        );
        Ok(format!("pending_burn_{}", amount))
    }
}

/// Compute the Anchor instruction discriminator for a function name.
/// Format: sha256("global:<function_name>")[0..8]
fn anchor_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{}", name).as_bytes());
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

/// Build the instruction data for `submit_bounty_proof`.
/// Layout: 8-byte discriminator + borsh-serialized fixed-size args.
fn build_submit_bounty_proof_data(
    bounty_id: &[u8; 32],
    base_points: u16,
    quality_score: u8,
    contribution_type: u8,
    is_agent: bool,
    agent_id: &[u8; 32],
    reviewer: &Pubkey,
    evidence_hash: &[u8; 32],
) -> Vec<u8> {
    let disc = anchor_discriminator("submit_bounty_proof");
    let external_reference = [0u8; 64]; // Reserved, zeroed

    let mut data = Vec::with_capacity(8 + 32 + 2 + 1 + 1 + 1 + 32 + 32 + 32 + 64);
    data.extend_from_slice(&disc);
    data.extend_from_slice(bounty_id);
    data.extend_from_slice(&base_points.to_le_bytes());
    data.push(quality_score);
    data.push(contribution_type);
    data.push(is_agent as u8);
    data.extend_from_slice(agent_id);
    data.extend_from_slice(reviewer.as_ref());
    data.extend_from_slice(evidence_hash);
    data.extend_from_slice(&external_reference);
    data
}

/// Hash a string (bounty UUID) to a fixed 32-byte array.
fn hash_to_32_bytes(input: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Derive an Associated Token Account (ATA) address.
fn derive_associated_token_account(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    let ata_program = Pubkey::from_str(SPL_ASSOCIATED_TOKEN_PROGRAM_ID).unwrap();
    let token_program = Pubkey::from_str(SPL_TOKEN_PROGRAM_ID).unwrap();

    let (ata, _) = Pubkey::find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ata_program,
    );
    ata
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solana_client_can_be_created() {
        let client = SolanaClient::new(
            "https://api.devnet.solana.com",
            "AmosBnty111111111111111111111111111111111111",
        );
        assert!(client.is_ok());
    }

    #[test]
    fn test_invalid_program_id() {
        let client = SolanaClient::new("https://api.devnet.solana.com", "invalid_pubkey");
        assert!(client.is_err());
    }

    #[test]
    fn test_anchor_discriminator() {
        let disc = anchor_discriminator("submit_bounty_proof");
        assert_eq!(disc.len(), 8);
        // Discriminator should be deterministic
        assert_eq!(disc, anchor_discriminator("submit_bounty_proof"));
    }

    #[test]
    fn test_instruction_data_length() {
        let bounty_id = [1u8; 32];
        let agent_id = [2u8; 32];
        let evidence_hash = [3u8; 32];
        let reviewer = Pubkey::new_unique();

        let data = build_submit_bounty_proof_data(
            &bounty_id, 100, 80, 1, true, &agent_id, &reviewer, &evidence_hash,
        );

        // 8 + 32 + 2 + 1 + 1 + 1 + 32 + 32 + 32 + 64 = 205
        assert_eq!(data.len(), 205);
    }

    #[test]
    fn test_hash_to_32_bytes() {
        let hash = hash_to_32_bytes("test-bounty-id");
        assert_eq!(hash.len(), 32);
        // Should be deterministic
        assert_eq!(hash, hash_to_32_bytes("test-bounty-id"));
    }

    #[test]
    fn test_settlement_readiness() {
        let client = SolanaClient::new(
            "https://api.devnet.solana.com",
            "AmosBnty111111111111111111111111111111111111",
        )
        .unwrap();

        assert!(!client.is_settlement_ready());
    }

    #[test]
    fn test_ata_derivation() {
        let wallet = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let ata = derive_associated_token_account(&wallet, &mint);
        // ATA should be deterministic
        assert_eq!(ata, derive_associated_token_account(&wallet, &mint));
        // ATA should differ from the wallet
        assert_ne!(ata, wallet);
    }
}
