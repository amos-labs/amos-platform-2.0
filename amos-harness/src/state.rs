//! Application state shared across all request handlers

use crate::{
    canvas::CanvasEngine,
    documents::DocumentProcessor,
    image_gen::ImageGenClient,
    integrations::{
        etl::EtlPipeline,
        executor::ApiExecutor,
    },
    openclaw::AgentManager,
    storage::StorageClient,
    task_queue::TaskQueue,
    tools::ToolRegistry,
};
use amos_core::AppConfig;
use dashmap::DashMap;
use sqlx::PgPool;
use std::sync::{
    atomic::AtomicBool,
    Arc,
};

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

    /// Active chat cancellation flags, keyed by chat_id.
    /// Set the `AtomicBool` to `true` to cancel a running agent loop.
    pub active_chats: Arc<DashMap<String, Arc<AtomicBool>>>,

    /// Universal API executor for making authenticated calls to external APIs
    pub api_executor: Arc<ApiExecutor>,

    /// ETL pipeline for syncing external API data into AMOS collections
    pub etl_pipeline: Arc<EtlPipeline>,
}

impl AppState {
    /// Get a Redis connection from the pool
    pub fn get_redis_connection(&self) -> Result<redis::Connection, redis::RedisError> {
        self.redis.get_connection()
    }

    /// Get an async Redis connection
    pub async fn get_redis_async_connection(
        &self,
    ) -> Result<redis::aio::Connection, redis::RedisError> {
        self.redis.get_async_connection().await
    }
}
