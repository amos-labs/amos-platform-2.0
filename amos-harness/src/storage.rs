//! File storage abstraction
//!
//! Supports local filesystem storage (default) with optional S3 support.
//! Files are stored with a UUID-based key and served via the uploads API.

use amos_core::{AmosError, Result};
use std::path::PathBuf;

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub backend: StorageBackend,
}

/// Storage backend type
#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// Local filesystem storage (default for development)
    Local {
        /// Directory where uploaded files are stored
        base_dir: PathBuf,
    },
    // Future: S3, GCS, etc.
}

impl Default for StorageConfig {
    fn default() -> Self {
        // Default to a `data/uploads` directory relative to the harness crate root
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data").join("uploads");
        Self {
            backend: StorageBackend::Local { base_dir: base },
        }
    }
}

impl StorageConfig {
    /// Create a config from environment variables.
    ///
    /// - `AMOS_STORAGE_DIR` overrides the local upload directory
    pub fn from_env() -> Self {
        let base_dir = std::env::var("AMOS_STORAGE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("data")
                    .join("uploads")
            });

        Self {
            backend: StorageBackend::Local { base_dir },
        }
    }
}

/// Storage client for file operations
#[derive(Clone, Debug)]
pub struct StorageClient {
    config: StorageConfig,
}

impl StorageClient {
    /// Create a new storage client, ensuring the backing store is ready.
    pub fn new(config: StorageConfig) -> Result<Self> {
        match &config.backend {
            StorageBackend::Local { base_dir } => {
                std::fs::create_dir_all(base_dir).map_err(|e| {
                    AmosError::Internal(format!(
                        "Failed to create upload directory {}: {e}",
                        base_dir.display()
                    ))
                })?;
                tracing::info!("Storage: local filesystem at {}", base_dir.display());
            }
        }
        Ok(Self { config })
    }

    /// Upload file data, returning the storage key on success.
    pub async fn upload(&self, key: &str, data: &[u8], _content_type: &str) -> Result<String> {
        match &self.config.backend {
            StorageBackend::Local { base_dir } => {
                let path = base_dir.join(key);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| AmosError::Internal(format!("create dir: {e}")))?;
                }
                tokio::fs::write(&path, data)
                    .await
                    .map_err(|e| AmosError::Internal(format!("write file: {e}")))?;
                Ok(key.to_string())
            }
        }
    }

    /// Read a file by its storage key.
    pub async fn read(&self, key: &str) -> Result<Vec<u8>> {
        match &self.config.backend {
            StorageBackend::Local { base_dir } => {
                let path = base_dir.join(key);
                tokio::fs::read(&path)
                    .await
                    .map_err(|e| AmosError::Internal(format!("read file '{}': {e}", key)))
            }
        }
    }

    /// Delete a file by its storage key.
    pub async fn delete(&self, key: &str) -> Result<()> {
        match &self.config.backend {
            StorageBackend::Local { base_dir } => {
                let path = base_dir.join(key);
                if path.exists() {
                    tokio::fs::remove_file(&path)
                        .await
                        .map_err(|e| AmosError::Internal(format!("delete file '{}': {e}", key)))?;
                }
                Ok(())
            }
        }
    }

    /// Check whether a file exists.
    pub async fn exists(&self, key: &str) -> bool {
        match &self.config.backend {
            StorageBackend::Local { base_dir } => base_dir.join(key).exists(),
        }
    }
}
