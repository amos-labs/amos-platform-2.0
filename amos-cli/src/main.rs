use clap::{Parser, Subcommand};
use std::error::Error;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "amos")]
#[command(about = "AMOS Admin CLI - Manage Harness and Platform services", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Harness commands (per-customer AI business OS)
    #[command(subcommand)]
    Harness(HarnessCommands),

    /// Platform commands (central token economics/governance/billing)
    #[command(subcommand)]
    Platform(PlatformCommands),

    /// Check all services health
    Health,

    /// Show current configuration
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },

    /// Check migration status (what's moved to Rust vs Rails)
    Migrate {
        #[command(subcommand)]
        action: MigrateCommands,
    },
}

#[derive(Subcommand)]
enum HarnessCommands {
    /// Start the local harness (delegates to amos-harness binary)
    Start {
        /// Port for HTTP API
        #[arg(long, default_value = "3000")]
        port: u16,
    },

    /// Check harness health
    Status {
        /// Harness URL
        #[arg(long, default_value = "http://localhost:3000")]
        url: String,
    },

    /// Canvas management
    #[command(subcommand)]
    Canvas(CanvasCommands),

    /// Bot management
    #[command(subcommand)]
    Bots(BotsCommands),

    /// External agent management
    #[command(subcommand)]
    Agents(AgentsCommands),
}

#[derive(Subcommand)]
enum CanvasCommands {
    /// List all canvases
    List {
        /// Harness URL
        #[arg(long, default_value = "http://localhost:3000")]
        url: String,
    },
}

#[derive(Subcommand)]
enum BotsCommands {
    /// List managed bots
    List {
        /// Harness URL
        #[arg(long, default_value = "http://localhost:3000")]
        url: String,
    },
}

#[derive(Subcommand)]
enum AgentsCommands {
    /// List external agents
    List {
        /// Harness URL
        #[arg(long, default_value = "http://localhost:3000")]
        url: String,
    },
}

#[derive(Subcommand)]
enum PlatformCommands {
    /// Start the platform server
    Start {
        /// HTTP port
        #[arg(long, default_value = "4000")]
        http_port: u16,

        /// gRPC port
        #[arg(long, default_value = "4001")]
        grpc_port: u16,
    },

    /// Check platform health
    Status {
        /// Platform URL
        #[arg(long, default_value = "http://localhost:4000")]
        url: String,
    },

    /// Token economics commands
    #[command(subcommand)]
    Token(TokenCommands),

    /// Provisioning commands
    #[command(subcommand)]
    Provision(ProvisionCommands),
}

#[derive(Subcommand)]
enum TokenCommands {
    /// Show token economy statistics
    Stats {
        /// Show historical data
        #[arg(short, long)]
        history: bool,
    },

    /// Show current decay rate and explanation
    Decay {
        /// Show decay for specific timestamp
        #[arg(short, long)]
        timestamp: Option<i64>,
    },

    /// Simulate decay over time for a given stake
    Simulate {
        /// Initial stake amount
        #[arg(short, long)]
        stake: f64,

        /// Number of days to simulate
        #[arg(short, long, default_value = "365")]
        days: u32,

        /// Output format (table, csv, json)
        #[arg(short, long, default_value = "table")]
        format: String,
    },
}

#[derive(Subcommand)]
enum ProvisionCommands {
    /// Provision a new harness for a customer
    Create {
        /// Customer name
        #[arg(long)]
        customer: String,

        /// Platform URL
        #[arg(long, default_value = "http://localhost:4000")]
        url: String,
    },

    /// List all provisioned harnesses
    List {
        /// Platform URL
        #[arg(long, default_value = "http://localhost:4000")]
        url: String,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current configuration
    Show {
        /// Show secrets (use with caution)
        #[arg(short, long)]
        secrets: bool,
    },
}

#[derive(Subcommand)]
enum MigrateCommands {
    /// Check migration status
    Check,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Set up tracing subscriber
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Load configuration
    let config = amos_core::AppConfig::load()?;
    info!("Configuration loaded successfully");

    // Parse CLI arguments
    let cli = Cli::parse();

    // Route to appropriate handler
    match cli.command {
        Commands::Harness(cmd) => {
            handle_harness_command(config, cmd).await?;
        }
        Commands::Platform(cmd) => {
            handle_platform_command(config, cmd).await?;
        }
        Commands::Health => {
            handle_health_command(config).await?;
        }
        Commands::Config { action } => {
            handle_config_command(config, action).await?;
        }
        Commands::Migrate { action } => {
            handle_migrate_command(config, action).await?;
        }
    }

    Ok(())
}

// ============================================================================
// Harness Command Handlers
// ============================================================================

async fn handle_harness_command(
    _config: amos_core::AppConfig,
    cmd: HarnessCommands,
) -> Result<(), Box<dyn Error>> {
    match cmd {
        HarnessCommands::Start { port } => {
            info!("Starting AMOS Harness on port {}", port);
            println!("Starting harness server...");
            println!("This will delegate to: cargo run --bin amos-harness");
            println!("Port: {}", port);
            println!("\n[Not yet implemented - will spawn amos-harness binary]");
            Ok(())
        }
        HarnessCommands::Status { url } => {
            println!("Checking harness status at: {}", url);
            let client = reqwest::Client::new();
            match client.get(format!("{}/health", url)).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        println!("Status: Healthy");
                        if let Ok(body) = response.text().await {
                            println!("Response: {}", body);
                        }
                    } else {
                        println!("Status: Unhealthy (HTTP {})", response.status());
                    }
                }
                Err(e) => {
                    println!("Status: Unreachable");
                    println!("Error: {}", e);
                }
            }
            Ok(())
        }
        HarnessCommands::Canvas(canvas_cmd) => match canvas_cmd {
            CanvasCommands::List { url } => {
                println!("Listing canvases from: {}", url);
                let client = reqwest::Client::new();
                match client.get(format!("{}/api/canvases", url)).send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            if let Ok(body) = response.text().await {
                                println!("{}", body);
                            }
                        } else {
                            println!("Error: HTTP {}", response.status());
                        }
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
                Ok(())
            }
        },
        HarnessCommands::Bots(bots_cmd) => match bots_cmd {
            BotsCommands::List { url } => {
                println!("Listing managed bots from: {}", url);
                let client = reqwest::Client::new();
                match client.get(format!("{}/api/bots", url)).send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            if let Ok(body) = response.text().await {
                                println!("{}", body);
                            }
                        } else {
                            println!("Error: HTTP {}", response.status());
                        }
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
                Ok(())
            }
        },
        HarnessCommands::Agents(agents_cmd) => match agents_cmd {
            AgentsCommands::List { url } => {
                println!("Listing external agents from: {}", url);
                let client = reqwest::Client::new();
                match client.get(format!("{}/api/agents", url)).send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            if let Ok(body) = response.text().await {
                                println!("{}", body);
                            }
                        } else {
                            println!("Error: HTTP {}", response.status());
                        }
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
                Ok(())
            }
        },
    }
}

// ============================================================================
// Platform Command Handlers
// ============================================================================

async fn handle_platform_command(
    config: amos_core::AppConfig,
    cmd: PlatformCommands,
) -> Result<(), Box<dyn Error>> {
    match cmd {
        PlatformCommands::Start {
            http_port,
            grpc_port,
        } => {
            info!("Starting AMOS Platform");
            println!("Starting platform server...");
            println!("HTTP Port: {}", http_port);
            println!("gRPC Port: {}", grpc_port);
            println!("This will delegate to: cargo run --bin amos-platform");
            println!("\n[Not yet implemented - will spawn amos-platform binary]");
            Ok(())
        }
        PlatformCommands::Status { url } => {
            println!("Checking platform status at: {}", url);
            let client = reqwest::Client::new();
            match client.get(format!("{}/health", url)).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        println!("Status: Healthy");
                        if let Ok(body) = response.text().await {
                            println!("Response: {}", body);
                        }
                    } else {
                        println!("Status: Unhealthy (HTTP {})", response.status());
                    }
                }
                Err(e) => {
                    println!("Status: Unreachable");
                    println!("Error: {}", e);
                }
            }
            Ok(())
        }
        PlatformCommands::Token(token_cmd) => {
            handle_token_command(config, token_cmd).await?;
            Ok(())
        }
        PlatformCommands::Provision(provision_cmd) => match provision_cmd {
            ProvisionCommands::Create { customer, url } => {
                println!("Provisioning new harness for customer: {}", customer);
                let client = reqwest::Client::new();
                let payload = serde_json::json!({
                    "customer_name": customer,
                });
                match client
                    .post(format!("{}/api/provision", url))
                    .json(&payload)
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status().is_success() {
                            println!("Successfully provisioned harness");
                            if let Ok(body) = response.text().await {
                                println!("{}", body);
                            }
                        } else {
                            println!("Error: HTTP {}", response.status());
                        }
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
                Ok(())
            }
            ProvisionCommands::List { url } => {
                println!("Listing provisioned harnesses from: {}", url);
                let client = reqwest::Client::new();
                match client.get(format!("{}/api/provision", url)).send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            if let Ok(body) = response.text().await {
                                println!("{}", body);
                            }
                        } else {
                            println!("Error: HTTP {}", response.status());
                        }
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
                Ok(())
            }
        },
    }
}

// ============================================================================
// Token Command Handlers (using amos_core functions)
// ============================================================================

async fn handle_token_command(
    _config: amos_core::AppConfig,
    cmd: TokenCommands,
) -> Result<(), Box<dyn Error>> {
    match cmd {
        TokenCommands::Stats { history } => {
            println!("Token Economy Statistics");
            println!("========================\n");

            // Use amos_core token economics
            use amos_core::token::economics::*;
            println!("Total Supply: {} AMOS", TOTAL_SUPPLY);
            println!("Grace Period: {} days", GRACE_PERIOD_DAYS);
            println!("Halving Interval: {} days", HALVING_INTERVAL_DAYS);
            println!(
                "Initial Daily Emission: {} AMOS/day",
                INITIAL_DAILY_EMISSION
            );

            if history {
                println!("\nHistorical data will be fetched from database...");
            }
        }
        TokenCommands::Decay { timestamp } => {
            println!("Token Decay Rate");
            println!("================\n");

            use amos_core::token::decay::{calculate_dynamic_decay_rate, PlatformEconomics};
            use amos_core::token::economics::BASE_DECAY_RATE_BPS;

            if let Some(ts) = timestamp {
                println!("Calculating decay for timestamp: {}", ts);
            } else {
                println!("Calculating current decay rate...");
            }

            // Placeholder economics for display
            let econ = PlatformEconomics {
                monthly_revenue_cents: 5_000_000,
                monthly_costs_cents: 5_000_000,
            };
            let rate_bps = calculate_dynamic_decay_rate(&econ);
            println!(
                "Current dynamic decay rate: {:.2}% per year ({} bps)",
                rate_bps as f64 / 100.0,
                rate_bps
            );
            println!(
                "Base decay rate: {:.2}% per year ({} bps)",
                BASE_DECAY_RATE_BPS as f64 / 100.0,
                BASE_DECAY_RATE_BPS
            );
            println!("\nExplanation: The decay rate adjusts based on platform profitability,");
            println!("ranging from 2% (highly profitable) to 25% (sustained losses).");
        }
        TokenCommands::Simulate {
            stake,
            days,
            format,
        } => {
            println!(
                "Simulating decay for {:.2} AMOS over {} days\n",
                stake, days
            );

            use amos_core::token::decay::{
                apply_daily_decay, calculate_dynamic_decay_rate, PlatformEconomics, StakeContext,
                VaultTier,
            };

            // Convert stake from f64 to u64 (assuming AMOS has no decimals on-chain)
            let original_balance = stake as u64;
            let mut current_balance = original_balance;

            // Placeholder economics (breakeven scenario)
            let econ = PlatformEconomics {
                monthly_revenue_cents: 5_000_000,
                monthly_costs_cents: 5_000_000,
            };
            let base_rate_bps = calculate_dynamic_decay_rate(&econ);

            match format.as_str() {
                "json" => {
                    println!("{{");
                    println!("  \"simulation\": [");
                }
                "csv" => {
                    println!("day,stake,decay_rate_pct,daily_loss");
                }
                _ => {
                    println!(
                        "{:<8} {:<15} {:<15} {:<15}",
                        "Day", "Stake", "Decay Rate %", "Daily Loss"
                    );
                    println!("{}", "-".repeat(60));
                }
            }

            for day in 0..=days {
                let context = StakeContext {
                    tenure_days: day as u64,
                    current_balance,
                    original_balance,
                    vault_tier: VaultTier::None,
                    days_inactive: 0,
                };

                let result = apply_daily_decay(base_rate_bps, &context);
                let daily_loss = result.tokens_decayed;
                let effective_rate_pct = result.effective_rate_bps as f64 / 100.0;

                match format.as_str() {
                    "json" => {
                        let comma = if day < days { "," } else { "" };
                        println!(
                            "    {{\"day\": {}, \"stake\": {}, \"decay_rate_pct\": {:.6}, \"daily_loss\": {}}}{}",
                            day, current_balance, effective_rate_pct, daily_loss, comma
                        );
                    }
                    "csv" => {
                        println!(
                            "{},{},{:.6},{}",
                            day, current_balance, effective_rate_pct, daily_loss
                        );
                    }
                    _ => {
                        if day % 30 == 0 || day == days {
                            println!(
                                "{:<8} {:<15} {:<15.6} {:<15}",
                                day, current_balance, effective_rate_pct, daily_loss
                            );
                        }
                    }
                }

                current_balance = result.new_balance;
            }

            if format == "json" {
                println!("  ]");
                println!("}}");
            } else if format == "table" {
                println!(
                    "\nFinal stake after {} days: {} AMOS",
                    days, current_balance
                );
                let total_decay = original_balance - current_balance;
                println!(
                    "Total decay: {} AMOS ({:.2}%)",
                    total_decay,
                    (total_decay as f64 / original_balance as f64) * 100.0
                );
            }
        }
    }
    Ok(())
}

// ============================================================================
// Utility Command Handlers
// ============================================================================

async fn handle_health_command(config: amos_core::AppConfig) -> Result<(), Box<dyn Error>> {
    use secrecy::ExposeSecret;

    println!("AMOS Health Check");
    println!("=================\n");

    let client = reqwest::Client::new();

    // Check Harness
    println!("Harness (port 3000): Checking...");
    match client.get("http://localhost:3000/health").send().await {
        Ok(response) => {
            if response.status().is_success() {
                println!("  Status: Healthy");
            } else {
                println!("  Status: Unhealthy (HTTP {})", response.status());
            }
        }
        Err(_) => {
            println!("  Status: Unreachable");
        }
    }

    // Check Platform
    println!("\nPlatform (port 4000): Checking...");
    match client.get("http://localhost:4000/health").send().await {
        Ok(response) => {
            if response.status().is_success() {
                println!("  Status: Healthy");
            } else {
                println!("  Status: Unhealthy (HTTP {})", response.status());
            }
        }
        Err(_) => {
            println!("  Status: Unreachable");
        }
    }

    // Check PostgreSQL
    println!("\nPostgreSQL: Checking...");
    println!("  URL: {}", config.database.url.expose_secret());
    println!("  Status: (not yet implemented)");

    // Check Redis
    println!("\nRedis: Checking...");
    println!("  URL: {}", config.redis.url);
    println!("  Status: (not yet implemented)");

    Ok(())
}

async fn handle_config_command(
    config: amos_core::AppConfig,
    cmd: ConfigCommands,
) -> Result<(), Box<dyn Error>> {
    match cmd {
        ConfigCommands::Show { secrets } => {
            use secrecy::ExposeSecret;

            println!("AMOS Configuration");
            println!("==================\n");

            let environment = std::env::var("AMOS_ENV").unwrap_or_else(|_| "development".into());
            println!("Environment: {}", environment);

            println!("\nServer:");
            println!("  Host: {}", config.server.host);
            println!("  Port: {}", config.server.port);
            println!("  gRPC Port: {}", config.server.grpc_port);
            println!("  Rails URL: {}", config.server.rails_url);

            println!("\nDatabase:");
            println!(
                "  URL: {}",
                if secrets {
                    config.database.url.expose_secret()
                } else {
                    "[REDACTED]"
                }
            );
            println!("  Pool size: {}", config.database.pool_size);

            println!("\nRedis:");
            println!(
                "  URL: {}",
                if secrets {
                    &config.redis.url
                } else {
                    "[REDACTED]"
                }
            );

            println!("\nSolana:");
            println!("  RPC URL: {}", config.solana.rpc_url);
            println!("  WS URL: {}", config.solana.ws_url);
            println!("  Treasury Program: {}", config.solana.treasury_program_id);
            println!(
                "  Governance Program: {}",
                config.solana.governance_program_id
            );
            println!("  Bounty Program: {}", config.solana.bounty_program_id);

            println!("\nBedrock:");
            println!("  AWS Region: {}", config.bedrock.aws_region);
            println!(
                "  AWS Access Key: {}",
                if secrets {
                    config
                        .bedrock
                        .aws_access_key_id
                        .as_ref()
                        .map(|s| s.expose_secret() as &str)
                        .unwrap_or("[NOT SET]")
                } else {
                    "[REDACTED]"
                }
            );
            println!(
                "  AWS Secret Key: {}",
                if secrets {
                    config
                        .bedrock
                        .aws_secret_access_key
                        .as_ref()
                        .map(|_| "[SET]")
                        .unwrap_or("[NOT SET]")
                } else {
                    "[REDACTED]"
                }
            );
            println!("  Default Model: {}", config.bedrock.default_model);
            println!("  Chat Model: {}", config.bedrock.chat_model);
            println!("  Voice Model: {}", config.bedrock.voice_model);

            println!("\nAgent:");
            println!("  Max Iterations: {}", config.agent.max_iterations);
            println!("  Max Context Tokens: {}", config.agent.max_context_tokens);
            println!("  Token Budget: {}", config.agent.token_budget);

            if !secrets {
                println!("\nUse --secrets to show sensitive values (use with caution)");
            }
        }
    }
    Ok(())
}

async fn handle_migrate_command(
    _config: amos_core::AppConfig,
    cmd: MigrateCommands,
) -> Result<(), Box<dyn Error>> {
    match cmd {
        MigrateCommands::Check => {
            println!("Migration Status Check");
            println!("======================\n");

            println!("Phase 0: Foundation");
            println!("  [x] Workspace structure");
            println!("  [x] Core types and config");
            println!("  [x] Token economics");
            println!("  [x] Solana programs");
            println!("  [x] Split into harness + platform");
            println!("  [x] CLI restructured");
            println!("  [x] Docker Compose\n");

            println!("Phase 1: Harness Implementation");
            println!("  [ ] Canvas management API");
            println!("  [ ] Bot orchestration");
            println!("  [ ] External agent integration");
            println!("  [ ] Customer isolation\n");

            println!("Phase 2: Platform Implementation");
            println!("  [ ] Token economics API");
            println!("  [ ] Governance endpoints");
            println!("  [ ] Billing integration");
            println!("  [ ] Provisioning automation\n");

            println!("See MIGRATION_CHECKLIST.md for full details.");

            // TODO: Query database to get actual migration status
            println!("\nDatabase migration status: (not yet implemented)");
        }
    }
    Ok(())
}
