//! Application state shared across all request handlers

use crate::{
    automations::{engine::AutomationEngine, TriggerEvent},
    canvas::CanvasEngine,
    documents::DocumentProcessor,
    embeddings::EmbeddingService,
    geo::GeoLocator,
    image_gen::ImageGenClient,
    integrations::{etl::EtlPipeline, executor::ApiExecutor},
    openclaw::AgentManager,
    storage::StorageClient,
    task_queue::TaskQueue,
    tools::ToolRegistry,
};
use amos_core::{AppConfig, CredentialVault};
use sqlx::PgPool;
use std::sync::Arc;

/// Shared application state
///
/// This struct holds all shared resources that are accessible from route handlers.
/// It is wrapped in an Arc to allow cheap cloning across async tasks.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool
    pub db_pool: PgPool,

    /// Redis client for caching and pub/sub
    pub redis: redis::Client,

    /// Application configuration
    pub config: Arc<AppConfig>,

    /// Canvas rendering and generation engine
    pub canvas_engine: Arc<CanvasEngine>,

    /// Tool registry for agent execution
    pub tool_registry: Arc<ToolRegistry>,

    /// OpenClaw agent manager for autonomous AI agent orchestration
    pub agent_manager: Arc<AgentManager>,

    /// Task queue for background work (internal sub-agents + external bounties)
    pub task_queue: Arc<TaskQueue>,

    /// File storage client (local filesystem or S3)
    pub storage: Arc<StorageClient>,

    /// Document processor for extracting text from uploaded files (PDF, DOCX, etc.)
    pub document_processor: Arc<DocumentProcessor>,

    /// Image generation client (Google Imagen API)
    /// `None` if credentials are not configured.
    pub image_gen: Option<Arc<ImageGenClient>>,

    /// Universal API executor for making authenticated calls to external APIs
    pub api_executor: Arc<ApiExecutor>,

    /// ETL pipeline for syncing external API data into AMOS collections
    pub etl_pipeline: Arc<EtlPipeline>,

    /// Credential vault for AES-256-GCM encrypted secret storage
    pub vault: Arc<CredentialVault>,

    /// IP geolocation service (cached lookups)
    pub geo_locator: Arc<GeoLocator>,

    /// Embedding service for semantic search (OpenAI-compatible API).
    /// `None` if `AMOS__EMBEDDING__API_KEY` is not set.
    pub embedding_service: Option<Arc<EmbeddingService>>,

    /// Automation engine for event-driven triggers and scheduled actions
    pub automation_engine: Arc<AutomationEngine>,

    /// Channel for schema CRUD events → automation engine (breaks async type cycle)
    pub automation_event_tx: tokio::sync::mpsc::UnboundedSender<TriggerEvent>,
}

impl AppState {
    /// Get a Redis connection from the pool
    pub fn get_redis_connection(&self) -> Result<redis::Connection, redis::RedisError> {
        self.redis.get_connection()
    }

    /// Get an async Redis connection
    #[allow(deprecated)]
    pub async fn get_redis_async_connection(
        &self,
    ) -> Result<redis::aio::Connection, redis::RedisError> {
        self.redis.get_async_connection().await
    }
}
