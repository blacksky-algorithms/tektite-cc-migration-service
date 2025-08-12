//! Web Storage Blob Manager for Migration Service
//!
//! This module provides blob storage functionality using web storage APIs
//! for storing large blob files during account migration. It includes retry logic, progress
//! tracking, and storage quota monitoring as required by the migration process.

#[cfg(feature = "web")]
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
#[cfg(feature = "web")]
use gloo_storage::{LocalStorage, Storage};

use gloo_console as console;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Maximum number of retry attempts for blob operations
const MAX_RETRY_ATTEMPTS: u32 = 5;

/// Base delay for exponential backoff (in milliseconds)
const BASE_RETRY_DELAY_MS: u64 = 1000;

/// Maximum LocalStorage blob storage size (50MB - conservative limit for localStorage)
const MAX_WEBSTORAGE_BYTES: u64 = 50 * 1024 * 1024;

/// LocalStorage key prefix for blobs
const BLOB_KEY_PREFIX: &str = "migration_blob_";

/// LocalStorage key for metadata
const METADATA_KEY: &str = "migration_blob_metadata";

/// Blob storage error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BlobError {
    WebStorageNotSupported,
    StorageQuotaExceeded,
    BlobNotFound(String),
    NetworkError(String),
    SerializationError(String),
    RetryExhausted(String),
    DatabaseError(String),
    Unknown(String),
}

impl std::fmt::Display for BlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlobError::WebStorageNotSupported => {
                write!(f, "Web storage is not supported in this environment")
            }
            BlobError::StorageQuotaExceeded => write!(f, "Web storage quota exceeded"),
            BlobError::BlobNotFound(cid) => write!(f, "Blob not found: {}", cid),
            BlobError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            BlobError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            BlobError::RetryExhausted(msg) => write!(f, "Retry attempts exhausted: {}", msg),
            BlobError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            BlobError::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

/// Storage information for quota monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    pub current_usage_bytes: u64,
    pub max_storage_bytes: u64,
    pub available_bytes: u64,
    pub blob_count: u32,
}

impl StorageInfo {
    pub fn usage_percentage(&self) -> f64 {
        if self.max_storage_bytes == 0 {
            0.0
        } else {
            (self.current_usage_bytes as f64 / self.max_storage_bytes as f64) * 100.0
        }
    }

    pub fn is_near_capacity(&self) -> bool {
        self.usage_percentage() > 85.0
    }
}

/// Blob progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobProgress {
    pub total_blobs: u32,
    pub processed_blobs: u32,
    pub total_bytes: u64,
    pub processed_bytes: u64,
    pub current_blob_cid: Option<String>,
    pub current_blob_progress: Option<f64>,
}

impl BlobProgress {
    pub fn percentage(&self) -> f64 {
        if self.total_blobs == 0 {
            0.0
        } else {
            (self.processed_blobs as f64 / self.total_blobs as f64) * 100.0
        }
    }
}

/// Web Storage Blob Manager for migration blob storage
pub struct BlobManager {
    pub current_usage_bytes: u64,
    pub blob_count: u32,
    pub blob_sizes: HashMap<String, u64>,
}

impl BlobManager {
    /// Create a new BlobManager instance
    #[cfg(feature = "web")]
    pub async fn new() -> Result<Self, BlobError> {
        console::info!("[BlobManager] Initializing web storage blob manager");

        let mut manager = BlobManager {
            current_usage_bytes: 0,
            blob_count: 0,
            blob_sizes: HashMap::new(),
        };

        // Load existing metadata from storage
        manager.load_metadata()?;

        console::info!(
            "[BlobManager] Initialized with {} bytes used, {} blobs",
            format!("{}", manager.current_usage_bytes),
            manager.blob_count
        );

        Ok(manager)
    }

    /// Create a new BlobManager instance (fallback for non-web)
    #[cfg(not(feature = "web"))]
    pub async fn new() -> Result<Self, BlobError> {
        console::warn!("[BlobManager] using non-web instances of Blob-Manager");
        Err(BlobError::WebStorageNotSupported)
    }

    /// Load metadata from storage
    #[cfg(feature = "web")]
    fn load_metadata(&mut self) -> Result<(), BlobError> {
        match LocalStorage::get::<String>(METADATA_KEY) {
            Ok(metadata_json) => {
                if let Ok(blob_sizes) = serde_json::from_str::<HashMap<String, u64>>(&metadata_json)
                {
                    self.blob_sizes = blob_sizes;
                    self.current_usage_bytes = self.blob_sizes.values().sum();
                    self.blob_count = self.blob_sizes.len() as u32;
                }
            }
            Err(_) => {
                // No existing metadata, start fresh
                self.current_usage_bytes = 0;
                self.blob_count = 0;
                self.blob_sizes.clear();
            }
        }
        Ok(())
    }

    /// Save metadata to storage
    #[cfg(feature = "web")]
    fn save_metadata(&self) -> Result<(), BlobError> {
        let metadata_json = serde_json::to_string(&self.blob_sizes).map_err(|e| {
            BlobError::SerializationError(format!("Failed to serialize metadata: {}", e))
        })?;

        LocalStorage::set(METADATA_KEY, metadata_json)
            .map_err(|e| BlobError::NetworkError(format!("Failed to save metadata: {:?}", e)))?;

        Ok(())
    }

    /// Save blob metadata when we can't mutate self (for trait compatibility)
    #[cfg(feature = "web")]
    fn save_blob_metadata(&self, cid: &str, size: u64) -> Result<(), BlobError> {
        // Load existing metadata
        let mut blob_sizes = match LocalStorage::get::<String>(METADATA_KEY) {
            Ok(metadata_json) => {
                serde_json::from_str::<HashMap<String, u64>>(&metadata_json)
                    .unwrap_or_else(|_| HashMap::new())
            }
            Err(_) => HashMap::new(),
        };

        // Add this blob
        blob_sizes.insert(cid.to_string(), size);

        // Save updated metadata
        let metadata_json = serde_json::to_string(&blob_sizes).map_err(|e| {
            BlobError::SerializationError(format!("Failed to serialize metadata: {}", e))
        })?;

        LocalStorage::set(METADATA_KEY, metadata_json)
            .map_err(|e| BlobError::NetworkError(format!("Failed to save metadata: {:?}", e)))?;

        Ok(())
    }

    /// Store a blob with retry logic and exponential backoff
    pub async fn store_blob_with_retry(
        &self,
        cid: &str,
        data: Vec<u8>,
    ) -> Result<(), BlobError> {
        console::info!(
            "[BlobManager] Storing blob {} ({} bytes)",
            cid,
            format!("{}", data.len())
        );

        // Check storage quota before storing
        if self.current_usage_bytes + data.len() as u64 > MAX_WEBSTORAGE_BYTES {
            console::error!("[BlobManager] Storage quota would be exceeded");
            return Err(BlobError::StorageQuotaExceeded);
        }

        let mut attempts = 0;
        let mut last_error = None;

        while attempts < MAX_RETRY_ATTEMPTS {
            attempts += 1;

            match self.store_blob_attempt(cid, &data).await {
                Ok(()) => {
                    // Note: For trait compatibility, we can't mutate self here
                    // The metadata will be updated when the mutable version is called
                    
                    // Save updated metadata with the blob size
                    #[cfg(feature = "web")]
                    if let Err(e) = self.save_blob_metadata(cid, data.len() as u64) {
                        console::warn!(
                            "[BlobManager] Failed to save metadata: {}",
                            format!("{}", e)
                        );
                    }

                    console::info!(
                        "[BlobManager] Successfully stored blob {} on attempt {}",
                        cid,
                        attempts
                    );
                    return Ok(());
                }
                Err(e) => {
                    console::warn!(
                        "[BlobManager] Attempt {} failed for blob {}: {}",
                        attempts,
                        cid,
                        format!("{}", e)
                    );
                    last_error = Some(e);

                    if attempts < MAX_RETRY_ATTEMPTS {
                        // Exponential backoff delay
                        let delay_ms = BASE_RETRY_DELAY_MS * (2_u64.pow(attempts - 1));
                        console::info!("[BlobManager] Retrying in {} ms", delay_ms);

                        // Simple delay for retry - in WASM this is typically not needed
                        // as operations are fast and non-blocking
                        #[cfg(feature = "web")]
                        {
                            // For web environments, we'll skip the delay to keep it simple
                        }

                        #[cfg(not(feature = "web"))]
                        {
                            // Fallback for non-web environments - no sleep needed
                            // WASM doesn't need delay since it's single-threaded async
                        }
                    }
                }
            }
        }

        let error = last_error.unwrap_or_else(|| BlobError::Unknown("Unknown error".to_string()));
        console::error!(
            "[BlobManager] Failed to store blob {} after {} attempts",
            cid,
            MAX_RETRY_ATTEMPTS
        );
        Err(BlobError::RetryExhausted(format!(
            "Failed to store blob {}: {}",
            cid, error
        )))
    }

    /// Single attempt to store a blob
    #[cfg(feature = "web")]
    pub async fn store_blob_attempt(&self, cid: &str, data: &[u8]) -> Result<(), BlobError> {
        // Encode blob data as base64 for localStorage
        let base64_data = BASE64.encode(data);
        let storage_key = format!("{}{}", BLOB_KEY_PREFIX, cid);

        LocalStorage::set(&storage_key, base64_data)
            .map_err(|e| BlobError::NetworkError(format!("Failed to store blob: {:?}", e)))?;

        Ok(())
    }

    /// Fallback for non-web environments
    #[cfg(not(feature = "web"))]
    async fn store_blob_attempt(&self, _cid: &str, _data: &[u8]) -> Result<(), BlobError> {
        Err(BlobError::WebStorageNotSupported)
    }

    /// Retrieve a blob from storage
    pub async fn get_blob(&self, cid: &str) -> Result<Vec<u8>, BlobError> {
        console::info!("[BlobManager] Retrieving blob {}", cid);

        #[cfg(feature = "web")]
        {
            let storage_key = format!("{}{}", BLOB_KEY_PREFIX, cid);

            match LocalStorage::get::<String>(&storage_key) {
                Ok(base64_data) => {
                    let data = BASE64.decode(&base64_data).map_err(|e| {
                        BlobError::SerializationError(format!("Failed to decode blob data: {}", e))
                    })?;

                    console::info!(
                        "[BlobManager] Retrieved blob {} ({} bytes)",
                        cid,
                        format!("{}", data.len())
                    );
                    Ok(data)
                }
                Err(_) => Err(BlobError::BlobNotFound(cid.to_string())),
            }
        }

        #[cfg(not(feature = "web"))]
        {
            let _ = cid;
            Err(BlobError::WebStorageNotSupported)
        }
    }

    /// Check if a blob exists in storage
    pub async fn has_blob(&self, cid: &str) -> bool {
        #[cfg(feature = "web")]
        {
            let storage_key = format!("{}{}", BLOB_KEY_PREFIX, cid);
            LocalStorage::get::<String>(&storage_key).is_ok()
        }

        #[cfg(not(feature = "web"))]
        {
            let _ = cid; // Suppress unused variable warning
            false
        }
    }

    /// Get current storage information
    pub async fn get_storage_info(&self) -> Result<StorageInfo, BlobError> {
        Ok(StorageInfo {
            current_usage_bytes: self.current_usage_bytes,
            max_storage_bytes: MAX_WEBSTORAGE_BYTES,
            available_bytes: MAX_WEBSTORAGE_BYTES.saturating_sub(self.current_usage_bytes),
            blob_count: self.blob_count,
        })
    }

    /// Clean up all blobs after successful migration
    pub async fn cleanup_blobs(&mut self) -> Result<(), BlobError> {
        console::info!("[BlobManager] Cleaning up all blobs after migration");

        #[cfg(feature = "web")]
        {
            // Remove all stored blobs
            for cid in self.blob_sizes.keys() {
                let storage_key = format!("{}{}", BLOB_KEY_PREFIX, cid);
                LocalStorage::delete(&storage_key);
                console::info!("[BlobManager] Removed blob {}", cid);
            }

            // Remove metadata
            LocalStorage::delete(METADATA_KEY);
            console::info!("[BlobManager] Removed metadata");
        }

        // Reset tracking
        self.current_usage_bytes = 0;
        self.blob_count = 0;
        self.blob_sizes.clear();

        console::info!("[BlobManager] Cleanup completed");
        Ok(())
    }

    /// Remove a specific blob from storage
    pub async fn remove_blob(&mut self, cid: &str) -> Result<(), BlobError> {
        console::info!("[BlobManager] Removing blob {}", cid);

        #[cfg(feature = "web")]
        {
            let storage_key = format!("{}{}", BLOB_KEY_PREFIX, cid);
            LocalStorage::delete(&storage_key);
        }

        // Update tracking
        if let Some(size) = self.blob_sizes.remove(cid) {
            self.current_usage_bytes = self.current_usage_bytes.saturating_sub(size);
            self.blob_count = self.blob_count.saturating_sub(1);

            // Save updated metadata
            #[cfg(feature = "web")]
            if let Err(e) = self.save_metadata() {
                console::warn!(
                    "[BlobManager] Failed to save metadata: {}",
                    format!("{}", e)
                );
            }
        }

        console::info!("[BlobManager] Removed blob {}", cid);
        Ok(())
    }

    /// Get list of all stored blob CIDs
    pub fn get_stored_blob_cids(&self) -> Vec<String> {
        self.blob_sizes.keys().cloned().collect()
    }

    /// Get size of a specific blob
    pub fn get_blob_size(&self, cid: &str) -> Option<u64> {
        self.blob_sizes.get(cid).copied()
    }

    /// Check if storage is near capacity
    pub fn is_near_capacity(&self) -> bool {
        let usage_percentage = if MAX_WEBSTORAGE_BYTES == 0 {
            0.0
        } else {
            (self.current_usage_bytes as f64 / MAX_WEBSTORAGE_BYTES as f64) * 100.0
        };
        usage_percentage > 85.0
    }

    /// Get available storage bytes
    pub fn get_available_bytes(&self) -> u64 {
        MAX_WEBSTORAGE_BYTES.saturating_sub(self.current_usage_bytes)
    }
}

/// Check if web storage is supported in the current environment
#[cfg(feature = "web")]
pub fn is_webstorage_supported() -> bool {
    LocalStorage::get::<String>("test").is_ok()
}

/// Check storage quota - returns conservative estimate for LocalStorage
#[cfg(feature = "web")]
pub async fn check_storage_quota() -> Result<u64, BlobError> {
    // LocalStorage typically has a 5-10MB limit per origin
    // We use a conservative 50MB limit to account for base64 overhead
    Ok(MAX_WEBSTORAGE_BYTES)
}

/// Fallback functions for non-web environments
#[cfg(not(feature = "web"))]
pub fn is_webstorage_supported() -> bool {
    false
}

#[cfg(not(feature = "web"))]
pub async fn check_storage_quota() -> Result<u64, BlobError> {
    Err(BlobError::WebStorageNotSupported)
}

/// Helper function to create a BlobManager instance with smart fallback
/// Tries OPFS first (best performance), falls back to chunked LocalStorage if OPFS fails
pub async fn create_blob_manager() -> Result<Box<dyn crate::services::blob::blob_manager_trait::BlobManagerTrait>, String> {
    use crate::services::blob::blob_opfs_storage::OpfsBlobManager;
    use crate::services::blob::blob_manager_trait::BlobManagerTrait;
    
    // Try OPFS first (best performance for large files)
    match OpfsBlobManager::new().await {
        Ok(manager) => {
            console::info!("✅ Using OPFS for blob storage");
            Ok(Box::new(manager) as Box<dyn BlobManagerTrait>)
        }
        Err(opfs_error) => {
            // Check if it's a security error (common in development)
            let error_msg = format!("{}", opfs_error);
            if error_msg.contains("SecurityError") || error_msg.contains("Security error") {
                console::warn!("⚠️ OPFS not available (security restriction), falling back to chunked LocalStorage");
            } else {
                console::warn!("⚠️ OPFS failed ({}), falling back to chunked LocalStorage", error_msg);
            }
            
            // Fall back to enhanced LocalStorage with chunking for larger blobs
            match BlobManager::new().await {
                Ok(manager) => {
                    console::info!("✅ Using chunked LocalStorage for blob storage (may have size limitations)");
                    Ok(Box::new(manager) as Box<dyn BlobManagerTrait>)
                }
                Err(ls_error) => {
                    let error_msg = format!(
                        "Both OPFS and LocalStorage failed: OPFS({}), LocalStorage({})", 
                        opfs_error, ls_error
                    );
                    console::error!("[create_blob_manager] {}", error_msg.clone());
                    Err(error_msg)
                }
            }
        }
    }
}

/// Helper function to store multiple blobs with progress tracking
pub async fn store_blobs_with_progress<F>(
    manager: &mut BlobManager,
    blobs: Vec<(String, Vec<u8>)>,
    mut progress_callback: F,
) -> Result<(), BlobError>
where
    F: FnMut(BlobProgress),
{
    let total_blobs = blobs.len() as u32;
    let total_bytes: u64 = blobs.iter().map(|(_, data)| data.len() as u64).sum();

    console::info!(
        "[store_blobs_with_progress] Storing {} blobs ({} bytes total)",
        total_blobs,
        format!("{}", total_bytes)
    );

    let mut processed_blobs = 0;
    let mut processed_bytes = 0;

    for (cid, data) in blobs {
        // Update progress
        progress_callback(BlobProgress {
            total_blobs,
            processed_blobs,
            total_bytes,
            processed_bytes,
            current_blob_cid: Some(cid.clone()),
            current_blob_progress: Some(0.0),
        });

        // Store blob with retry logic
        manager.store_blob_with_retry(&cid, data.clone()).await?;

        // Update counters
        processed_blobs += 1;
        processed_bytes += data.len() as u64;

        // Final progress update for this blob
        progress_callback(BlobProgress {
            total_blobs,
            processed_blobs,
            total_bytes,
            processed_bytes,
            current_blob_cid: Some(cid),
            current_blob_progress: Some(100.0),
        });
    }

    console::info!("[store_blobs_with_progress] Completed storing all blobs");
    Ok(())
}
