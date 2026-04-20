//! Probe whether an Anchor instruction is deployed on a given program.
//!
//! Simulates a transaction with only the instruction's discriminator as
//! data. The RPC simulation error tells us whether the instruction exists:
//!
//! - logs contain `InstructionFallbackNotFound` → NOT deployed
//! - any other error (ConstraintViolation, AccountNotInitialized, missing
//!   accounts) → DEPLOYED; we just didn't supply correct accounts, which
//!   is fine — we only care whether the dispatcher recognizes the name.
//!
//! Usage:
//!
//!     AMOS_SOLANA_RPC_URL=https://api.mainnet-beta.solana.com \
//!     cargo run -p amos-relay --bin probe_instruction -- \
//!         <program_id> <instruction_name>
//!
//! This lets us reconstruct "what's actually deployed" when the on-chain
//! Anchor IDL hasn't been uploaded. One-off verification tool for upgrades.

use amos_relay::solana::anchor_discriminator;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSimulateTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::hash::Hash;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::Transaction;
use std::env;
use std::process::ExitCode;
use std::str::FromStr;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!(
            "Usage: {} <program_id> <instruction_name>\n\n\
             Env: AMOS_SOLANA_RPC_URL (default: https://api.mainnet-beta.solana.com)",
            args[0]
        );
        return ExitCode::from(2);
    }

    let program_id_str = &args[1];
    let instruction_name = &args[2];
    let rpc_url = env::var("AMOS_SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into());

    let disc = anchor_discriminator(instruction_name);
    println!("Probing {} on {}", instruction_name, program_id_str);
    println!("Discriminator: {}", hex::encode(disc));

    let program_pk = match Pubkey::from_str(program_id_str) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Invalid program ID '{}': {}", program_id_str, e);
            return ExitCode::from(2);
        }
    };
    // Fee-payer must be a real, funded account — even with sig_verify=false
    // and replace_recent_blockhash=true, the RPC validates the fee-payer
    // exists before reaching the program dispatcher. Use the founder wallet
    // (known funded) as a read-only placeholder; we're not actually paying.
    let fee_payer_str = env::var("AMOS_SOLANA_PROBE_FEE_PAYER")
        .unwrap_or_else(|_| "WxdXw1f1kFMRu8HDf1SE6yjgeWyf3Vb4T63QXMs4yij".into());
    let fee_payer = match Pubkey::from_str(&fee_payer_str) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Invalid fee-payer pubkey '{}': {}", fee_payer_str, e);
            return ExitCode::from(2);
        }
    };

    let ix = Instruction {
        program_id: program_pk,
        accounts: vec![],
        data: disc.to_vec(),
    };

    let message = Message::new_with_blockhash(&[ix], Some(&fee_payer), &Hash::default());
    let mut tx = Transaction::new_unsigned(message);
    tx.signatures = vec![Signature::default()];

    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    let cfg = RpcSimulateTransactionConfig {
        sig_verify: false,
        replace_recent_blockhash: true,
        commitment: Some(CommitmentConfig::confirmed()),
        encoding: None,
        accounts: None,
        min_context_slot: None,
        inner_instructions: false,
    };

    let result = match rpc.simulate_transaction_with_config(&tx, cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("RPC simulation failed: {}", e);
            return ExitCode::from(1);
        }
    };

    let sim = result.value;
    let logs = sim.logs.clone().unwrap_or_default();

    println!("\n--- simulation logs ---");
    for log in &logs {
        println!("  {}", log);
    }
    println!("\n--- err ---");
    println!("  {:?}", sim.err);

    let logs_joined = logs.join(" | ");
    println!("\n--- verdict ---");
    if logs_joined.contains("InstructionFallbackNotFound")
        || logs_joined.contains("Error Code: InstructionFallbackNotFound")
    {
        println!(
            "  {} on {}: NOT DEPLOYED (InstructionFallbackNotFound in logs)",
            instruction_name, program_id_str
        );
    } else if sim.err.is_some() || !logs.is_empty() {
        println!(
            "  {} on {}: DEPLOYED (dispatcher recognized discriminator; expected failure on missing accounts/args)",
            instruction_name, program_id_str
        );
    } else {
        println!(
            "  {} on {}: INDETERMINATE (no logs, no err)",
            instruction_name, program_id_str
        );
    }

    ExitCode::from(0)
}
