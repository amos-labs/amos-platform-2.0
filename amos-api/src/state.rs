use amos_core::AppConfig;
use redis::Client as RedisClient;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::sync::Arc;

/// Application state shared across all request handlers
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool
    pub db_pool: PgPool,

    /// Redis client for caching and pub/sub
    pub redis: RedisClient,

    /// Agent runtime factory (will hold agent loop factory)
    pub agent_runtime: Arc<AgentRuntime>,

    /// Application configuration
    pub config: Arc<AppConfig>,
}

/// Placeholder for agent runtime - will be implemented with actual agent loop factory
#[derive(Clone)]
pub struct AgentRuntime {
    // TODO: Add actual agent loop factory
    pub max_concurrent_sessions: usize,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self {
            max_concurrent_sessions: 100,
        }
    }
}

impl AppState {
    /// Create AppState from configuration
    /// Connects to database and Redis
    pub async fn from_config(config: AppConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Connect to PostgreSQL
        let db_pool = PgPool::connect(config.database.url.expose_secret())
            .await
            .map_err(|e| format!("Failed to connect to database: {}", e))?;

        tracing::info!("Connected to PostgreSQL");

        // Connect to Redis
        let redis = RedisClient::open(config.redis.url.as_str())
            .map_err(|e| format!("Failed to connect to Redis: {}", e))?;

        // Test Redis connection
        let mut conn = redis.get_connection()
            .map_err(|e| format!("Failed to get Redis connection: {}", e))?;
        redis::cmd("PING")
            .query::<String>(&mut conn)
            .map_err(|e| format!("Failed to ping Redis: {}", e))?;

        tracing::info!("Connected to Redis");

        // Create agent runtime
        let agent_runtime = Arc::new(AgentRuntime::default());

        Ok(Self {
            db_pool,
            redis,
            agent_runtime,
            config: Arc::new(config),
        })
    }

    /// Health check for database connection
    pub async fn check_db_health(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.db_pool)
            .await?;
        Ok(())
    }

    /// Health check for Redis connection
    pub async fn check_redis_health(&self) -> Result<(), redis::RedisError> {
        let mut conn = self.redis.get_connection()?;
        redis::cmd("PING").query::<String>(&mut conn)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_runtime_default() {
        let runtime = AgentRuntime::default();
        assert_eq!(runtime.max_concurrent_sessions, 100);
    }
}
