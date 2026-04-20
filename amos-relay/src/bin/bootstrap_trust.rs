//! One-shot admin utility to bootstrap an agent's on-chain trust level.
//!
//! Uses the oracle keypair to sign a `bootstrap_trust` instruction against the
//! AMOS Bounty program. Only works for agents that have an existing
//! `agent_trust` PDA with zero completions (on-chain precondition).
//!
//! Usage:
//!
//!     AMOS_SOLANA_RPC_URL=https://api.mainnet-beta.solana.com \
//!     AMOS_SOLANA_BOUNTY_PROGRAM_ID=4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq \
//!     AMOS_SOLANA_ORACLE_KEYPAIR=/path/to/oracle.json \
//!     cargo run -p amos-relay --bin bootstrap_trust -- <wallet_pubkey> <trust_level>
//!
//! This is intentionally a standalone binary rather than an admin endpoint
//! because (1) it is a one-time operation per wallet, (2) it requires the
//! oracle keypair, and (3) a reusable admin endpoint with the same capability
//! is being delivered separately as OPS-BOOTSTRAP-ENDPOINT-001.

use amos_relay::solana::SolanaClient;
use std::env;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!(
            "Usage: {} <wallet_pubkey> <trust_level:1-5>\n\n\
             Env:\n  AMOS_SOLANA_RPC_URL (default: https://api.mainnet-beta.solana.com)\n  \
             AMOS_SOLANA_BOUNTY_PROGRAM_ID (default: 4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq)\n  \
             AMOS_SOLANA_ORACLE_KEYPAIR (required — path to oracle keypair JSON)",
            args[0]
        );
        return ExitCode::from(2);
    }

    let wallet = &args[1];
    let trust_level: u8 = match args[2].parse() {
        Ok(v) if (1..=5).contains(&v) => v,
        _ => {
            eprintln!("trust_level must be an integer 1-5");
            return ExitCode::from(2);
        }
    };

    let rpc_url = env::var("AMOS_SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into());
    let program_id = env::var("AMOS_SOLANA_BOUNTY_PROGRAM_ID")
        .unwrap_or_else(|_| "4XbUwKNMoERKuzzeSKJgATttgHFcjazohuYYgiwj9tsq".into());
    let keypair_path = match env::var("AMOS_SOLANA_ORACLE_KEYPAIR") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("AMOS_SOLANA_ORACLE_KEYPAIR must be set to the oracle keypair JSON path");
            return ExitCode::from(2);
        }
    };

    let mut client = match SolanaClient::new(&rpc_url, &program_id) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to init SolanaClient: {}", e);
            return ExitCode::from(1);
        }
    };
    if let Err(e) = client.load_oracle_keypair(&keypair_path) {
        eprintln!("Failed to load oracle keypair from {}: {}", keypair_path, e);
        return ExitCode::from(1);
    }

    println!("Bootstrapping {} to trust level {}...", wallet, trust_level);
    match client
        .bootstrap_agent_trust_on_chain(wallet, trust_level)
        .await
    {
        Ok(tx) => {
            println!("OK: {}", tx);
            println!("https://solscan.io/tx/{}", tx);
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("FAILED: {}", e);
            ExitCode::from(1)
        }
    }
}
