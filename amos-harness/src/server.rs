//! Axum server setup and configuration

use crate::{
    agent::BedrockClient, canvas::CanvasEngine, documents::DocumentProcessor,
    image_gen::ImageGenClient, integrations::{etl::EtlPipeline, executor::ApiExecutor},
    middleware, openclaw::AgentManager, routes,
    state::AppState, storage::{StorageClient, StorageConfig}, task_queue::TaskQueue,
    tools::ToolRegistry,
};
use amos_core::{AppConfig, Result};
use axum::{
    http::{
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
        Method,
    },
    Router,
};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    services::ServeDir,
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

/// Create and configure the Axum server
pub async fn create_server(
    config: Arc<AppConfig>,
    db_pool: PgPool,
    redis_client: redis::Client,
) -> Result<Router> {
    // Initialize components
    let canvas_engine = Arc::new(CanvasEngine::new(db_pool.clone(), config.clone()));
    let task_queue = Arc::new(TaskQueue::new(db_pool.clone()));

    // Create a shared Bedrock client for canvas generation (and potentially other tools)
    let bedrock = match BedrockClient::new(None, None, None) {
        Ok(client) => {
            tracing::info!("Bedrock client initialized for canvas generation");
            Some(Arc::new(client))
        }
        Err(e) => {
            tracing::warn!("Bedrock client unavailable (canvas generation will use static templates): {}", e);
            None
        }
    };

    // Initialize integration subsystem
    let api_executor = Arc::new(ApiExecutor::new(db_pool.clone()));
    let etl_pipeline = Arc::new(EtlPipeline::new(db_pool.clone()));

    let tool_registry = Arc::new(ToolRegistry::default_registry(
        db_pool.clone(),
        config.clone(),
        task_queue.clone(),
        bedrock,
        api_executor.clone(),
        etl_pipeline.clone(),
    ));
    let agent_manager = Arc::new(AgentManager::new(db_pool.clone(), config.clone()).await?);

    // Initialize file storage
    let storage_config = StorageConfig::from_env();
    let storage = Arc::new(StorageClient::new(storage_config).await?);

    // Initialize document processor (extract + export pipeline)
    let document_processor = Arc::new(DocumentProcessor::new());
    tracing::info!("Document processor initialized (PDF + DOCX extraction/export)");

    // Initialize image generation (Google Imagen API)
    let image_gen = ImageGenClient::from_env().map(|client| {
        tracing::info!("Image generation client initialized (Google Imagen)");
        Arc::new(client)
    });
    if image_gen.is_none() {
        tracing::info!("Image generation disabled (GOOGLE_CLOUD_PROJECT not set)");
    }

    // Create application state
    let state = Arc::new(AppState {
        db_pool,
        redis: redis_client,
        config: config.clone(),
        canvas_engine,
        tool_registry,
        agent_manager,
        task_queue,
        storage,
        document_processor,
        image_gen,
        active_chats: Arc::new(dashmap::DashMap::new()),
        api_executor,
        etl_pipeline,
    });

    // Build router with all routes
    let api_routes = routes::build_routes(state.clone());

    // Configure CORS (using permissive settings for now)
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE])
        .allow_credentials(false)
        .max_age(Duration::from_secs(3600));

    // Configure tracing
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_response(
            DefaultOnResponse::new()
                .level(Level::INFO)
                .latency_unit(LatencyUnit::Millis),
        );

    // Build middleware stack
    let middleware_stack = ServiceBuilder::new()
        .layer(trace_layer)
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::new(Duration::from_secs(60)))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::error_handler::handle_error,
        ));

    // Configure static file serving with SPA fallback
    // Resolve the static dir relative to the harness crate, not the cwd
    let static_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");
    let serve_dir = ServeDir::new(&static_dir)
        .not_found_service(ServeDir::new(&static_dir).append_index_html_on_directories(true));

    // Build the application router
    // API routes take precedence over static files
    let app = Router::new()
        .merge(api_routes)
        .fallback_service(serve_dir)
        .layer(middleware_stack);

    Ok(app)
}
