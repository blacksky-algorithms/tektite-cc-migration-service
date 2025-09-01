mod storage_estimator;
mod unified_config;

use crate::console_warn;

pub use storage_estimator::{
    get_storage_estimate, try_get_storage_estimate, StorageEstimate, StorageEstimatorError,
};
pub use unified_config::*;

#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub storage: StorageConfig,
    pub concurrency: ConcurrencyConfig,
    pub retry: RetryConfig,
    pub blob: BlobConfig,
    pub architecture: MigrationArchitecture,
}

/// Migration architecture choice (WASM-first)
#[derive(Debug, Clone, PartialEq)]
pub enum MigrationArchitecture {
    /// Traditional approach: download -> store -> upload separately
    Traditional,
    /// Streaming approach: use channel-tee pattern for simultaneous operations (WASM-compatible)
    Streaming,
}

#[derive(Debug, Clone)]
pub struct BlobConfig {
    pub enumeration_method: BlobEnumerationMethod,
    pub verification_delay_ms: u64,
    pub max_verification_attempts: u32,
    pub verification_backoff_ms: u64,
}

/// Method for enumerating blobs during migration
#[derive(Debug, Clone, PartialEq)]
pub enum BlobEnumerationMethod {
    /// Use com.atproto.repo.listMissingBlobs (migration-optimized, default)
    MissingBlobs,
    /// Use com.atproto.sync.listBlobs (full enumeration, matches Go goat)
    SyncListBlobs,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub local_storage_limit: u64,
    pub indexeddb_limit: u64,
    pub opfs_limit: u64,
}

#[derive(Debug, Clone)]
pub struct ConcurrencyConfig {
    pub max_concurrent_transfers: usize,
    pub opfs_concurrency: usize,
    pub indexeddb_concurrency: usize,
    pub localstorage_concurrency: usize,
}

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub storage_retries: u32,
    pub migration_retries: u32,
}

impl Default for BlobConfig {
    fn default() -> Self {
        Self {
            enumeration_method: BlobEnumerationMethod::MissingBlobs, // Default to migration-optimized
            verification_delay_ms: 3000, // 3 seconds initial delay after uploads
            max_verification_attempts: 5, // Try up to 5 times to verify uploads
            verification_backoff_ms: 2000, // 2 seconds linear backoff between attempts
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            local_storage_limit: 50 * 1024 * 1024, // 50MB
            indexeddb_limit: 1024 * 1024 * 1024,   // 1GB
            opfs_limit: u64::MAX,                  // No limit for OPFS
        }
    }
}

impl StorageConfig {
    /// Conservative defaults for wasm32-unknown-unknown target
    pub fn conservative_defaults() -> Self {
        Self {
            local_storage_limit: 5 * 1024 * 1024, // 5MB (very conservative)
            indexeddb_limit: 50 * 1024 * 1024,    // 50MB (conservative)
            opfs_limit: 100 * 1024 * 1024,        // 100MB (conservative)
        }
    }
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_concurrent_transfers: 10,
            opfs_concurrency: 10,
            indexeddb_concurrency: 5,
            localstorage_concurrency: 1,
        }
    }
}

impl ConcurrencyConfig {
    /// Conservative defaults for wasm32-unknown-unknown target
    pub fn conservative_defaults() -> Self {
        Self {
            max_concurrent_transfers: 5, // Reduced from 10
            opfs_concurrency: 5,         // Reduced from 10
            indexeddb_concurrency: 3,    // Reduced from 5
            localstorage_concurrency: 1, // Keep at 1 (unchanged)
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            storage_retries: 3,
            migration_retries: 3,
        }
    }
}

impl RetryConfig {
    /// Conservative defaults for wasm32-unknown-unknown target
    pub fn conservative_defaults() -> Self {
        Self {
            max_attempts: 3,      // Reduced from 5
            storage_retries: 2,   // Reduced from 3
            migration_retries: 2, // Reduced from 3
        }
    }
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for MigrationArchitecture {
    fn default() -> Self {
        Self::Streaming // Default to WASM-optimized streaming architecture
    }
}

impl MigrationConfig {
    /// Create a new configuration optimized for WASM environment
    pub fn new() -> Self {
        Self {
            storage: StorageConfig::conservative_defaults(),
            concurrency: ConcurrencyConfig::conservative_defaults(),
            retry: RetryConfig::conservative_defaults(),
            blob: BlobConfig::default(),
            architecture: MigrationArchitecture::Streaming, // Default to streaming for WASM
        }
    }

    /// Create configuration enhanced with browser storage information when available
    pub async fn new_with_browser_storage() -> Self {
        match try_get_storage_estimate().await {
            Some(estimate) => Self::from_storage_estimate(estimate),
            None => Self::new(), // Fallback to conservative defaults
        }
    }

    /// Create configuration from browser storage estimate
    fn from_storage_estimate(estimate: StorageEstimate) -> Self {
        let available = estimate.available_bytes();

        // Calculate dynamic limits based on available storage
        let local_storage_limit = std::cmp::min(5 * 1024 * 1024, (available as f64 * 0.1) as u64);
        let indexeddb_limit = std::cmp::min(200 * 1024 * 1024, (available as f64 * 0.3) as u64);
        let opfs_limit = std::cmp::min(500 * 1024 * 1024, (available as f64 * 0.5) as u64);

        Self {
            storage: StorageConfig {
                local_storage_limit,
                indexeddb_limit,
                opfs_limit,
            },
            concurrency: if available > 500 * 1024 * 1024 {
                // High storage available - use normal concurrency
                ConcurrencyConfig::default()
            } else {
                // Limited storage - use conservative concurrency
                ConcurrencyConfig::conservative_defaults()
            },
            retry: RetryConfig::conservative_defaults(),
            blob: BlobConfig::default(),
            architecture: MigrationArchitecture::Streaming, // Always use streaming for WASM
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.concurrency.max_concurrent_transfers == 0 {
            return Err("max_concurrent_transfers must be greater than 0".to_string());
        }

        if self.retry.max_attempts == 0 {
            return Err("max_attempts must be greater than 0".to_string());
        }

        if self.storage.local_storage_limit == 0 {
            return Err("local_storage_limit must be greater than 0".to_string());
        }

        Ok(())
    }
}

use std::sync::OnceLock;

static GLOBAL_CONFIG: OnceLock<MigrationConfig> = OnceLock::new();

/// Get the global configuration, initialized with conservative defaults
pub fn get_global_config() -> MigrationConfig {
    GLOBAL_CONFIG
        .get_or_init(|| {
            let config = MigrationConfig::new();
            if let Err(e) = config.validate() {
                console_warn!("Invalid configuration: {}", e);
                MigrationConfig::new()
            } else {
                config
            }
        })
        .clone()
}

/// Initialize global configuration with browser storage integration (async version)
/// Call this early in your application startup for best results
pub async fn init_global_config_with_browser_storage() {
    // With OnceLock, initialization happens automatically on first access
    // This function serves as a way to trigger initialization early if needed
    let _ = get_global_config();
}

/// Get or create configuration with browser storage integration
/// This is the preferred method when you can use async
pub async fn get_config_with_browser_storage() -> MigrationConfig {
    let config = MigrationConfig::new_with_browser_storage().await;
    if let Err(e) = config.validate() {
        console_warn!("Invalid configuration: {}", e);
        MigrationConfig::new()
    } else {
        config
    }
}
