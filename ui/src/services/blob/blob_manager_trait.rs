//! Blob Manager trait abstraction
//!
//! This module provides a common interface for different blob storage backends
//! (OPFS, IndexedDB) to enable seamless fallback between storage methods.

use async_trait::async_trait;

/// Common error type for blob operations
#[derive(Debug, Clone)]
pub enum BlobManagerError {
    StorageError(String),
    NotFound(String),
    BlobNotFound(String),
    QuotaExceeded(String),
    SecurityError(String),
    Unknown(String),
}

impl std::fmt::Display for BlobManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlobManagerError::StorageError(msg) => write!(f, "Storage Error: {}", msg),
            BlobManagerError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            BlobManagerError::BlobNotFound(msg) => write!(f, "Blob Not Found: {}", msg),
            BlobManagerError::QuotaExceeded(msg) => write!(f, "Quota Exceeded: {}", msg),
            BlobManagerError::SecurityError(msg) => write!(f, "Security Error: {}", msg),
            BlobManagerError::Unknown(msg) => write!(f, "Unknown Error: {}", msg),
        }
    }
}

// Add conversion from different storage manager error types
impl From<crate::services::blob::blob_idb_storage::IdbBlobError> for BlobManagerError {
    fn from(err: crate::services::blob::blob_idb_storage::IdbBlobError) -> Self {
        match err {
            crate::services::blob::blob_idb_storage::IdbBlobError::NotFound(cid) => {
                BlobManagerError::BlobNotFound(cid)
            }
            crate::services::blob::blob_idb_storage::IdbBlobError::StorageQuotaExceeded => {
                BlobManagerError::QuotaExceeded("IndexedDB quota exceeded".to_string())
            }
            crate::services::blob::blob_idb_storage::IdbBlobError::NotSupported => {
                BlobManagerError::StorageError("IndexedDB not supported".to_string())
            }
            _ => BlobManagerError::StorageError(format!("IndexedDB error: {}", err)),
        }
    }
}

impl From<crate::services::blob::blob_storage::BlobError> for BlobManagerError {
    fn from(err: crate::services::blob::blob_storage::BlobError) -> Self {
        match err {
            crate::services::blob::blob_storage::BlobError::BlobNotFound(cid) => {
                BlobManagerError::BlobNotFound(cid)
            }
            crate::services::blob::blob_storage::BlobError::StorageQuotaExceeded => {
                BlobManagerError::QuotaExceeded("LocalStorage quota exceeded".to_string())
            }
            crate::services::blob::blob_storage::BlobError::WebStorageNotSupported => {
                BlobManagerError::StorageError("LocalStorage not supported".to_string())
            }
            _ => BlobManagerError::StorageError(format!("LocalStorage error: {}", err)),
        }
    }
}

impl From<crate::services::blob::blob_opfs_storage::OpfsError> for BlobManagerError {
    fn from(err: crate::services::blob::blob_opfs_storage::OpfsError) -> Self {
        match err {
            crate::services::blob::blob_opfs_storage::OpfsError::NotFound(cid) => {
                BlobManagerError::BlobNotFound(cid)
            }
            crate::services::blob::blob_opfs_storage::OpfsError::Storage(msg) => {
                if msg.contains("quota") || msg.contains("storage") {
                    BlobManagerError::QuotaExceeded(format!("OPFS quota exceeded: {}", msg))
                } else {
                    BlobManagerError::StorageError(format!("OPFS storage error: {}", msg))
                }
            }
            crate::services::blob::blob_opfs_storage::OpfsError::InvalidData(msg) => {
                BlobManagerError::StorageError(format!("OPFS invalid data: {}", msg))
            }
        }
    }
}

/// Trait for blob storage managers
#[async_trait(?Send)]
pub trait BlobManagerTrait {
    /// Store a blob with the given CID and data
    async fn store_blob(&self, cid: &str, data: Vec<u8>) -> Result<(), BlobManagerError>;

    /// Store blob with retry logic
    async fn store_blob_with_retry(&self, cid: &str, data: Vec<u8>)
        -> Result<(), BlobManagerError>;

    /// Retrieve a blob by CID
    async fn retrieve_blob(&self, cid: &str) -> Result<Vec<u8>, BlobManagerError>;

    /// Check if a blob exists
    async fn has_blob(&self, cid: &str) -> bool;

    /// Clean up all stored blobs
    async fn cleanup_blobs(&self) -> Result<(), BlobManagerError>;

    /// Get storage usage information
    async fn get_storage_usage(&self) -> Result<u64, BlobManagerError>;

    /// Get the name of the storage backend
    fn storage_name(&self) -> &'static str;

    /// List all stored blob CIDs
    /// Returns a vector of CIDs that are currently stored in this backend
    async fn list_stored_blobs(&self) -> Result<Vec<String>, BlobManagerError>;
}

/// OPFS implementation of BlobManagerTrait
#[async_trait(?Send)]
impl BlobManagerTrait for crate::services::blob::blob_opfs_storage::OpfsBlobManager {
    async fn store_blob(&self, cid: &str, data: Vec<u8>) -> Result<(), BlobManagerError> {
        self.store_blob(cid, data)
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn store_blob_with_retry(
        &self,
        cid: &str,
        data: Vec<u8>,
    ) -> Result<(), BlobManagerError> {
        self.store_blob_with_retry(cid, data)
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn retrieve_blob(&self, cid: &str) -> Result<Vec<u8>, BlobManagerError> {
        self.retrieve_blob(cid)
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn has_blob(&self, cid: &str) -> bool {
        self.has_blob(cid).await
    }

    async fn cleanup_blobs(&self) -> Result<(), BlobManagerError> {
        self.cleanup_blobs()
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn get_storage_usage(&self) -> Result<u64, BlobManagerError> {
        self.get_storage_usage()
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    fn storage_name(&self) -> &'static str {
        "OPFS"
    }

    async fn list_stored_blobs(&self) -> Result<Vec<String>, BlobManagerError> {
        self.list_stored_blobs()
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }
}

/// IndexedDB implementation of BlobManagerTrait (fallback between OPFS and LocalStorage)
#[async_trait(?Send)]
impl BlobManagerTrait for crate::services::blob::blob_idb_storage::IdbBlobManager {
    async fn store_blob(&self, cid: &str, data: Vec<u8>) -> Result<(), BlobManagerError> {
        self.store_blob(cid, data)
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn store_blob_with_retry(
        &self,
        cid: &str,
        data: Vec<u8>,
    ) -> Result<(), BlobManagerError> {
        self.store_blob_with_retry(cid, data)
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn retrieve_blob(&self, cid: &str) -> Result<Vec<u8>, BlobManagerError> {
        self.retrieve_blob(cid)
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn has_blob(&self, cid: &str) -> bool {
        self.has_blob(cid).await
    }

    async fn cleanup_blobs(&self) -> Result<(), BlobManagerError> {
        self.cleanup_blobs()
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn get_storage_usage(&self) -> Result<u64, BlobManagerError> {
        self.get_storage_usage()
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    fn storage_name(&self) -> &'static str {
        "IndexedDB"
    }

    async fn list_stored_blobs(&self) -> Result<Vec<String>, BlobManagerError> {
        self.list_stored_blobs()
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }
}

/// LocalStorage implementation of BlobManagerTrait (fallback)
#[async_trait(?Send)]
impl BlobManagerTrait for crate::services::blob::blob_storage::BlobManager {
    async fn store_blob(&self, cid: &str, data: Vec<u8>) -> Result<(), BlobManagerError> {
        // Use the existing store_blob_attempt method
        self.store_blob_attempt(cid, &data)
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn store_blob_with_retry(
        &self,
        cid: &str,
        data: Vec<u8>,
    ) -> Result<(), BlobManagerError> {
        self.store_blob_with_retry(cid, data)
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn retrieve_blob(&self, cid: &str) -> Result<Vec<u8>, BlobManagerError> {
        self.get_blob(cid)
            .await
            .map_err(|e| BlobManagerError::StorageError(e.to_string()))
    }

    async fn has_blob(&self, cid: &str) -> bool {
        self.has_blob(cid).await
    }

    async fn cleanup_blobs(&self) -> Result<(), BlobManagerError> {
        // For LocalStorage, we can clean up without mutating self
        #[cfg(feature = "web")]
        {
            use gloo_storage::{LocalStorage, Storage};

            // Load existing metadata to get blob CIDs
            if let Ok(metadata_json) = LocalStorage::get::<String>("migration_blob_metadata") {
                if let Ok(blob_sizes) =
                    serde_json::from_str::<std::collections::HashMap<String, u64>>(&metadata_json)
                {
                    for cid in blob_sizes.keys() {
                        let storage_key = format!("migration_blob_{}", cid);
                        LocalStorage::delete(&storage_key);
                    }
                }
            }

            // Remove metadata
            LocalStorage::delete("migration_blob_metadata");
        }

        Ok(())
    }

    async fn get_storage_usage(&self) -> Result<u64, BlobManagerError> {
        Ok(self.current_usage_bytes)
    }

    fn storage_name(&self) -> &'static str {
        "LocalStorage"
    }

    async fn list_stored_blobs(&self) -> Result<Vec<String>, BlobManagerError> {
        #[cfg(feature = "web")]
        {
            use gloo_storage::{LocalStorage, Storage};
            
            // Load existing metadata to get blob CIDs
            match LocalStorage::get::<String>("migration_blob_metadata") {
                Ok(metadata_json) => {
                    match serde_json::from_str::<std::collections::HashMap<String, u64>>(&metadata_json) {
                        Ok(blob_sizes) => {
                            let cids: Vec<String> = blob_sizes.keys().cloned().collect();
                            Ok(cids)
                        }
                        Err(e) => Err(BlobManagerError::StorageError(format!(
                            "Failed to parse blob metadata: {}", e
                        )))
                    }
                }
                Err(_) => Ok(Vec::new()), // No metadata means no blobs stored
            }
        }
        #[cfg(not(feature = "web"))]
        {
            Ok(Vec::new())
        }
    }
}
