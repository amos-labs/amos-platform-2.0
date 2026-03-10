//! AMOS Agent binary - standalone autonomous agent.
//!
//! Connects to the AMOS Harness using the same protocol as any external agent.
//! Provides local tools (think, remember, plan, web_search, file I/O) and
//! accesses harness tools via HTTP.

use amos_agent::{
    agent_card::{AgentCard, agent_card_router},
    agent_loop::{self, LoopConfig},
    config::{AgentConfig, Cli},
    harness_client::HarnessClient,
    memory::MemoryStore,
    provider,
    tools::ToolContext,
};
use clap::Parser;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{debug, error, info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI args
    let cli = Cli::parse();
    let config = AgentConfig::from(cli);

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(&config.log_level)
        .init();

    info!(
        name = %config.agent_name,
        harness = %config.harness_url,
        port = config.agent_port,
        "Starting AMOS Agent"
    );

    // Initialize memory store
    let memory = Arc::new(
        MemoryStore::open(&config.memory_db)
            .map_err(|e| anyhow::anyhow!("Failed to open memory database: {e}"))?
    );
    let mem_count = memory.count().unwrap_or(0);
    info!(path = %config.memory_db, memories = mem_count, "Memory store initialized");

    // Create tool context
    let tool_ctx = ToolContext {
        memory: memory.clone(),
        brave_api_key: config.brave_api_key.clone(),
        work_dir: config.work_dir.clone(),
    };

    // Initialize harness client
    let mut harness = HarnessClient::new(&config.harness_url, config.agent_token.clone());

    // Try to register with the harness
    let card_url = format!("http://localhost:{}/.well-known/agent.json", config.agent_port);
    match harness.register(&config.agent_name, Some(&card_url)).await {
        Ok(()) => {
            info!(
                tools = harness.harness_tools.len(),
                "Connected to harness, {} tools available",
                harness.harness_tools.len()
            );
        }
        Err(e) => {
            warn!("Could not connect to harness: {e}. Running in standalone mode.");
        }
    }

    // Start the Agent Card server in the background
    let agent_card = AgentCard {
        url: format!("http://localhost:{}", config.agent_port),
        ..AgentCard::default()
    };
    let card_router = agent_card_router(agent_card);
    let card_port = config.agent_port;

    tokio::spawn(async move {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", card_port))
            .await
            .expect("Failed to bind Agent Card server");
        info!(port = card_port, "Agent Card server listening");
        axum::serve(listener, card_router).await.ok();
    });

    // Start heartbeat loop in background
    let heartbeat_harness_url = config.harness_url.clone();
    let heartbeat_token = config.agent_token.clone();
    tokio::spawn(async move {
        let client = HarnessClient::new(&heartbeat_harness_url, heartbeat_token);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            if let Err(e) = client.heartbeat().await {
                debug!("Heartbeat failed: {e}");
            }
        }
    });

    // Interactive mode: read from stdin
    info!("AMOS Agent ready. Type a message to begin (Ctrl+C to quit).");

    let stdin = tokio::io::stdin();
    let reader = tokio::io::BufReader::new(stdin);

    use tokio::io::AsyncBufReadExt;
    let mut lines = reader.lines();

    loop {
        eprint!("\n> ");
        match lines.next_line().await {
            Ok(Some(line)) => {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                if line == "/quit" || line == "/exit" {
                    info!("Goodbye!");
                    break;
                }

                // Create a provider for this request
                let model_provider = match provider::create_provider(
                    &config.model_provider,
                    &config.model_id,
                    config.api_base.as_deref(),
                    config.api_key.as_deref(),
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Error creating model provider: {e}");
                        continue;
                    }
                };

                let loop_config = LoopConfig {
                    max_iterations: config.max_iterations,
                    model_id: config.model_id.clone(),
                    ..Default::default()
                };

                // Set up event channel for streaming output
                let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);

                // Print streaming output
                let print_handle = tokio::spawn(async move {
                    while let Some(event) = event_rx.recv().await {
                        match event {
                            agent_loop::AgentEvent::TextDelta { content } => {
                                eprint!("{}", content);
                            }
                            agent_loop::AgentEvent::ToolStart { tool_name, is_local } => {
                                let loc = if is_local { "local" } else { "harness" };
                                eprintln!("\n[{loc}] {tool_name}...");
                            }
                            agent_loop::AgentEvent::ToolEnd { tool_name, duration_ms, is_error } => {
                                if is_error {
                                    eprintln!("[error] {tool_name} failed ({duration_ms}ms)");
                                }
                            }
                            agent_loop::AgentEvent::Error { message } => {
                                eprintln!("\n[ERROR] {message}");
                            }
                            _ => {}
                        }
                    }
                });

                // Run the agent loop
                match agent_loop::run_agent_loop(
                    &loop_config,
                    model_provider.as_ref(),
                    &tool_ctx,
                    Some(&harness),
                    &line,
                    Some(event_tx),
                )
                .await
                {
                    Ok(_final_text) => {
                        eprintln!(); // newline after streaming
                    }
                    Err(e) => {
                        eprintln!("\nAgent error: {e}");
                    }
                }

                let _ = print_handle.await;
            }
            Ok(None) => break, // EOF
            Err(e) => {
                error!("Input error: {e}");
                break;
            }
        }
    }

    Ok(())
}
