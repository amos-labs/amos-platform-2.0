//! Axum server setup and configuration

use crate::{
    automations::engine::AutomationEngine,
    bedrock::BedrockClient,
    canvas::CanvasEngine,
    documents::DocumentProcessor,
    embeddings::EmbeddingService,
    geo::GeoLocator,
    image_gen::ImageGenClient,
    integrations::{etl::EtlPipeline, executor::ApiExecutor},
    openclaw::AgentManager,
    packages, routes,
    state::AppState,
    storage::{StorageClient, StorageConfig},
    task_queue::TaskQueue,
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
            tracing::warn!(
                "Bedrock client unavailable (canvas generation will use static templates): {}",
                e
            );
            None
        }
    };

    // Initialize credential vault (AES-256-GCM encryption)
    let vault = Arc::new(amos_core::CredentialVault::from_env()?);
    tracing::info!("Credential vault initialized");

    // Initialize integration subsystem (with vault for encrypted credential resolution)
    let api_executor = Arc::new(ApiExecutor::with_vault(db_pool.clone(), vault.clone()));
    let etl_pipeline = Arc::new(EtlPipeline::new(db_pool.clone()));

    // Initialize embedding service (for semantic search in memory/knowledge base)
    let embedding_service = {
        use secrecy::ExposeSecret;
        config.embedding.api_key.as_ref().map(|key| {
            let svc = EmbeddingService::new(
                key.expose_secret().to_string(),
                config.embedding.api_base.clone(),
                config.embedding.model.clone(),
            );
            tracing::info!(
                model = %config.embedding.model,
                "Embedding service initialized (semantic search enabled)"
            );
            Arc::new(svc)
        })
    };
    if embedding_service.is_none() {
        tracing::info!("Embedding service disabled (AMOS__EMBEDDING__API_KEY not set)");
    }

    // Initialize automation engine
    let automation_http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    let automation_engine = Arc::new(AutomationEngine::new(
        db_pool.clone(),
        task_queue.clone(),
        automation_http_client,
    ));

    let mut tool_registry = ToolRegistry::default_registry(
        db_pool.clone(),
        config.clone(),
        task_queue.clone(),
        bedrock,
        api_executor.clone(),
        etl_pipeline.clone(),
        embedding_service.clone(),
        automation_engine.clone(),
    );

    // Load configured packages and register their tools (AMOS_PACKAGES env var).
    // Tools are registered now (before Arc wrapping) and tagged with package names.
    let configured_packages =
        packages::load_and_register_packages(&mut tool_registry, db_pool.clone(), config.clone());

    let agent_manager = Arc::new(AgentManager::new(db_pool.clone(), config.clone()).await?);

    // Initialize file storage
    let storage_config = StorageConfig::from_env();
    let storage = Arc::new(StorageClient::new(storage_config).await?);

    // Initialize document processor (extract + export pipeline)
    let document_processor = Arc::new(DocumentProcessor::new());
    tracing::info!("Document processor initialized (PDF + DOCX extraction/export)");

    // Initialize IP geolocation service
    let geo_locator = Arc::new(GeoLocator::new());
    tracing::info!("GeoLocator initialized (IP-based location with caching)");

    // Initialize image generation (Google Imagen API)
    let image_gen = ImageGenClient::from_env().map(|client| {
        tracing::info!("Image generation client initialized (Google Imagen)");
        Arc::new(client)
    });
    if image_gen.is_none() {
        tracing::info!("Image generation disabled (GOOGLE_CLOUD_PROJECT not set)");
    }

    // Start the automation cron loop (checks scheduled automations every 60s)
    automation_engine.clone().start();
    tracing::info!("Automation engine started (cron scheduler active)");

    // Create event channel for schema → automation decoupling
    let automation_event_tx = automation_engine.create_event_channel();

    // Create application state
    let state = Arc::new(AppState {
        db_pool,
        redis: redis_client,
        config: config.clone(),
        canvas_engine,
        tool_registry: Arc::new(tool_registry),
        agent_manager,
        task_queue,
        storage,
        document_processor,
        image_gen,
        api_executor,
        etl_pipeline,
        vault,
        geo_locator,
        embedding_service,
        automation_engine: automation_engine.clone(),
        automation_event_tx,
    });

    // Activate packages (bootstrap schemas, collect routes)
    let package_routes = packages::activate_packages(&configured_packages, state.clone()).await?;

    // Build router with all routes
    let mut api_routes = routes::build_routes(state.clone());

    // Nest package routes under /api/v1/pkg/{package_name}/
    for (pkg_name, router) in package_routes {
        let path = format!("/api/v1/pkg/{pkg_name}");
        tracing::info!("Mounting package routes at {path}");
        api_routes = api_routes.nest(&path, router);
    }

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
    #[allow(deprecated)]
    let middleware_stack = ServiceBuilder::new()
        .layer(trace_layer)
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::new(Duration::from_secs(60)));

    // Configure static file serving with SPA fallback.
    // Resolve static dir: AMOS_STATIC_DIR env > ./static (cwd) > compile-time fallback.
    let static_dir = std::env::var("AMOS_STATIC_DIR")
        .map(std::path::PathBuf::from)
        .ok()
        .filter(|p| p.exists())
        .or_else(|| {
            let cwd = std::path::PathBuf::from("./static");
            if cwd.exists() {
                Some(cwd)
            } else {
                None
            }
        })
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"));
    tracing::info!(path = %static_dir.display(), "Serving static files from");
    let serve_dir = ServeDir::new(&static_dir).append_index_html_on_directories(true);

    // Build the application router
    // API routes take precedence over static files
    let app = Router::new()
        .merge(api_routes)
        .fallback_service(serve_dir)
        .layer(middleware_stack);

    Ok(app)
}
