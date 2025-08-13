//! IndexedDB Blob Storage Manager for Migration Service
//!
//! This module provides blob storage functionality using IndexedDB via the `idb` crate
//! as a fallback between OPFS and LocalStorage. IndexedDB provides better performance
//! and larger storage capacity than LocalStorage while being more widely supported than OPFS.

use crate::services::config::get_global_config;
use gloo_console as console;

/// Helper function to safely format u64 values for logging to avoid BigInt serialization issues
fn format_bytes(bytes: u64) -> String {
    bytes.to_string()
}
use idb::{Database, DatabaseEvent, Error as IdbError, Factory, ObjectStoreParams, TransactionMode, KeyPath};
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::JsValue;

/// Database name for blob storage
const DB_NAME: &str = "migration_blob_storage";

/// Database version
const DB_VERSION: u32 = 1;

/// Object store name for blobs
const BLOB_STORE_NAME: &str = "blobs";

/// Object store name for metadata
const METADATA_STORE_NAME: &str = "metadata";

/// IndexedDB storage error types
#[derive(Debug, Clone)]
pub enum IdbBlobError {
    DatabaseError(String),
    NotSupported,
    NotFound(String),
    StorageQuotaExceeded,
    SerializationError(String),
    TransactionError(String),
    Unknown(String),
}

impl std::fmt::Display for IdbBlobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdbBlobError::DatabaseError(msg) => write!(f, "IndexedDB Database Error: {}", msg),
            IdbBlobError::NotSupported => write!(f, "IndexedDB is not supported in this environment"),
            IdbBlobError::NotFound(cid) => write!(f, "Blob not found in IndexedDB: {}", cid),
            IdbBlobError::StorageQuotaExceeded => write!(f, "IndexedDB storage quota exceeded"),
            IdbBlobError::SerializationError(msg) => write!(f, "IndexedDB serialization error: {}", msg),
            IdbBlobError::TransactionError(msg) => write!(f, "IndexedDB transaction error: {}", msg),
            IdbBlobError::Unknown(msg) => write!(f, "Unknown IndexedDB error: {}", msg),
        }
    }
}

impl From<IdbError> for IdbBlobError {
    fn from(err: IdbError) -> Self {
        let error_msg = format!("{:?}", err);
        console::debug!("[IdbBlobStorage] 🔄 Converting IDB error: {}", &error_msg);
        
        // Enhanced error classification with better logging
        if error_msg.contains("QuotaExceededError") || error_msg.contains("quota") || error_msg.contains("storage") {
            console::warn!("[IdbBlobStorage] 💾 Storage quota exceeded - user may need to clear browser data or request more storage");
            IdbBlobError::StorageQuotaExceeded
        } else if error_msg.contains("NotSupportedError") || error_msg.contains("not supported") {
            console::error!("[IdbBlobStorage] 🚫 IndexedDB not supported in this browser/context");
            IdbBlobError::NotSupported
        } else if error_msg.contains("NotFoundError") || error_msg.contains("not found") {
            console::debug!("[IdbBlobStorage] 🔍 Record not found in IndexedDB (this may be expected)");
            IdbBlobError::NotFound("Record not found in IndexedDB".to_string())
        } else if error_msg.contains("VersionError") || error_msg.contains("version") {
            console::error!("[IdbBlobStorage] 🔄 Database version conflict - may need to handle upgrade");
            IdbBlobError::DatabaseError(format!("Database version error: {}", error_msg))
        } else if error_msg.contains("InvalidStateError") || error_msg.contains("invalid state") {
            console::error!("[IdbBlobStorage] ⚠️ Invalid database state - transaction may have failed");
            IdbBlobError::TransactionError(format!("Invalid state error: {}", error_msg))
        } else if error_msg.contains("AbortError") || error_msg.contains("abort") {
            console::warn!("[IdbBlobStorage] 🛑 Database operation was aborted");
            IdbBlobError::TransactionError(format!("Operation aborted: {}", error_msg))
        } else if error_msg.contains("TimeoutError") || error_msg.contains("timeout") {
            console::warn!("[IdbBlobStorage] ⏱️ Database operation timed out");
            IdbBlobError::TransactionError(format!("Operation timeout: {}", error_msg))
        } else {
            console::error!("[IdbBlobStorage] ❓ Unknown IndexedDB error type: {}", &error_msg);
            IdbBlobError::DatabaseError(error_msg)
        }
    }
}

/// Blob metadata stored in IndexedDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobMetadata {
    pub cid: String,
    pub size: u64,
    pub stored_at: u64, // timestamp
}

/// Storage usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdbStorageStats {
    pub total_blobs: u32,
    pub total_bytes: u64,
    pub last_updated: u64,
}

/// IndexedDB Blob Storage Manager
pub struct IdbBlobManager {
    database: Database,
}

impl IdbBlobManager {
    /// Create a new IndexedDB blob manager
    pub async fn new() -> Result<Self, IdbBlobError> {
        console::info!("[IdbBlobManager] Initializing IndexedDB blob storage");

        // Get IndexedDB factory
        let factory = Factory::new().map_err(|e| {
            console::error!("[IdbBlobManager] Failed to create IDB factory: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?;

        console::debug!("[IdbBlobManager] IDB factory created successfully");

        // Open database with upgrade handler
        let mut open_request = factory.open(DB_NAME, Some(DB_VERSION)).map_err(|e| {
            console::error!("[IdbBlobManager] Failed to open database: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?;

        console::debug!("[IdbBlobManager] Database open request created");

        // Set up database upgrade handler
        open_request.on_upgrade_needed(|event| {
            console::info!("[IdbBlobManager] Setting up database schema");
            
            let database = event.database().unwrap();
            
            // Create blob object store
            let mut blob_store_params = ObjectStoreParams::new();
            blob_store_params.key_path(Some(KeyPath::new_single("cid")));
            
            let _blob_store = database
                .create_object_store(BLOB_STORE_NAME, blob_store_params)
                .unwrap();
            console::debug!("[IdbBlobManager] Created blob object store");

            // Create metadata object store
            let mut metadata_store_params = ObjectStoreParams::new();
            metadata_store_params.key_path(Some(KeyPath::new_single("key")));
            
            let _metadata_store = database
                .create_object_store(METADATA_STORE_NAME, metadata_store_params)
                .unwrap();
            console::debug!("[IdbBlobManager] Created metadata object store");

            console::info!("[IdbBlobManager] Database schema setup completed");
        });

        // Await database opening
        console::debug!("[IdbBlobManager] Awaiting database open...");
        let database = open_request.await.map_err(|e| {
            console::error!("[IdbBlobManager] Database open failed: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?;

        console::info!("[IdbBlobManager] IndexedDB blob manager initialized successfully");

        Ok(Self { database })
    }

    /// Store a blob in IndexedDB
    pub async fn store_blob(&self, cid: &str, data: Vec<u8>) -> Result<(), IdbBlobError> {
        console::info!("[IdbBlobManager] Storing blob {} ({} bytes)", cid, format_bytes(data.len() as u64));

        // Check if we would exceed storage quota
        let current_usage = self.get_storage_usage().await.unwrap_or(0);
        let config = get_global_config();
        if current_usage + data.len() as u64 > config.storage.indexeddb_limit {
            console::error!("[IdbBlobManager] Storage quota would be exceeded");
            return Err(IdbBlobError::StorageQuotaExceeded);
        }

        // Create transaction
        let transaction = self.database
            .transaction(&[BLOB_STORE_NAME, METADATA_STORE_NAME], TransactionMode::ReadWrite)
            .map_err(|e| {
                console::error!("[IdbBlobManager] Failed to create transaction: {}", format!("{:?}", e));
                IdbBlobError::TransactionError(format!("Failed to create transaction: {:?}", e))
            })?;

        console::debug!("[IdbBlobManager] Transaction created for blob {}", cid);

        // Get object stores
        let blob_store = transaction.object_store(BLOB_STORE_NAME).map_err(|e| {
            console::error!("[IdbBlobManager] Failed to get blob store: {}", format!("{:?}", e));
            IdbBlobError::TransactionError(format!("Failed to get blob store: {:?}", e))
        })?;

        let metadata_store = transaction.object_store(METADATA_STORE_NAME).map_err(|e| {
            console::error!("[IdbBlobManager] Failed to get metadata store: {}", format!("{:?}", e));
            IdbBlobError::TransactionError(format!("Failed to get metadata store: {:?}", e))
        })?;

        // Create blob record
        let blob_record = serde_json::json!({
            "cid": cid,
            "data": data,
            "size": data.len(),
            "stored_at": js_sys::Date::now() as u64
        });

        // Store blob data
        let blob_js_value = blob_record.serialize(&Serializer::json_compatible()).map_err(|e| {
            console::error!("[IdbBlobManager] Failed to serialize blob: {}", format!("{:?}", e));
            IdbBlobError::SerializationError(format!("Failed to serialize blob: {:?}", e))
        })?;

        console::debug!("[IdbBlobManager] Serialized blob data for {}", cid);

        blob_store.put(&blob_js_value, None).map_err(|e| {
            console::error!("[IdbBlobManager] Failed to store blob: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?.await.map_err(|e| {
            console::error!("[IdbBlobManager] Failed to complete blob storage: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?;

        console::debug!("[IdbBlobManager] Blob {} stored in object store", cid);

        // Create metadata record
        let metadata = BlobMetadata {
            cid: cid.to_string(),
            size: data.len() as u64,
            stored_at: js_sys::Date::now() as u64,
        };

        let metadata_record = serde_json::json!({
            "key": cid,
            "metadata": metadata
        });

        let metadata_js_value = metadata_record.serialize(&Serializer::json_compatible()).map_err(|e| {
            console::error!("[IdbBlobManager] Failed to serialize metadata: {}", format!("{:?}", e));
            IdbBlobError::SerializationError(format!("Failed to serialize metadata: {:?}", e))
        })?;

        metadata_store.put(&metadata_js_value, None).map_err(|e| {
            console::error!("[IdbBlobManager] Failed to store metadata: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?.await.map_err(|e| {
            console::error!("[IdbBlobManager] Failed to complete metadata storage: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?;

        console::debug!("[IdbBlobManager] Metadata stored for blob {}", cid);

        // Commit transaction
        transaction.commit().map_err(|e| {
            console::error!("[IdbBlobManager] Failed to commit transaction: {}", format!("{:?}", e));
            IdbBlobError::TransactionError(format!("Failed to commit transaction: {:?}", e))
        })?.await.map_err(|e| {
            console::error!("[IdbBlobManager] Failed to complete transaction commit: {}", format!("{:?}", e));
            IdbBlobError::TransactionError(format!("Failed to complete transaction commit: {:?}", e))
        })?;

        console::info!("[IdbBlobManager] Successfully stored blob {} ({} bytes)", cid, format_bytes(data.len() as u64));
        Ok(())
    }

    /// Store blob with retry logic
    pub async fn store_blob_with_retry(&self, cid: &str, data: Vec<u8>) -> Result<(), IdbBlobError> {
        let config = get_global_config();
        const BASE_DELAY_MS: u64 = 1000;
        
        console::info!("[IdbBlobManager] Storing blob {} with retry logic ({} bytes)", cid, format_bytes(data.len() as u64));
        
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < config.retry.storage_retries {
            attempts += 1;
            console::debug!("[IdbBlobManager] Attempt {} for blob {}", attempts, cid);

            match self.store_blob(cid, data.clone()).await {
                Ok(()) => {
                    console::info!("[IdbBlobManager] Successfully stored blob {} on attempt {}", cid, attempts);
                    return Ok(());
                }
                Err(e) => {
                    console::warn!("[IdbBlobManager] Attempt {} failed for blob {}: {}", attempts, cid, format!("{}", e));
                    last_error = Some(e);

                    if attempts < config.retry.storage_retries {
                        let delay_ms = BASE_DELAY_MS * (2_u64.pow(attempts - 1));
                        console::info!("[IdbBlobManager] Retrying in {} ms", delay_ms);
                        // Note: In WASM, we typically don't need actual delays for retry
                    }
                }
            }
        }

        let error = last_error.unwrap_or_else(|| IdbBlobError::Unknown("Unknown retry error".to_string()));
        console::error!("[IdbBlobManager] Failed to store blob {} after {} attempts: {}", cid, config.retry.storage_retries, format!("{}", error));
        Err(error)
    }

    /// Retrieve a blob from IndexedDB
    pub async fn retrieve_blob(&self, cid: &str) -> Result<Vec<u8>, IdbBlobError> {
        console::info!("[IdbBlobManager] Retrieving blob {}", cid);

        // Create read-only transaction
        let transaction = self.database
            .transaction(&[BLOB_STORE_NAME], TransactionMode::ReadOnly)
            .map_err(|e| {
                console::error!("[IdbBlobManager] Failed to create read transaction: {}", format!("{:?}", e));
                IdbBlobError::TransactionError(format!("Failed to create read transaction: {:?}", e))
            })?;

        let blob_store = transaction.object_store(BLOB_STORE_NAME).map_err(|e| {
            console::error!("[IdbBlobManager] Failed to get blob store for read: {}", format!("{:?}", e));
            IdbBlobError::TransactionError(format!("Failed to get blob store for read: {:?}", e))
        })?;

        console::debug!("[IdbBlobManager] Querying blob store for {}", cid);

        // Get blob record
        let cid_js = JsValue::from_str(cid);
        let blob_record: Option<JsValue> = blob_store.get(cid_js).map_err(|e| {
            console::error!("[IdbBlobManager] Failed to query blob: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?.await.map_err(|e| {
            console::error!("[IdbBlobManager] Failed to complete blob query: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?;

        // Wait for transaction to complete
        transaction.await.map_err(|e| {
            console::error!("[IdbBlobManager] Transaction failed during blob retrieval: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?;

        match blob_record {
            Some(record) => {
                console::debug!("[IdbBlobManager] Found blob record for {}", cid);
                
                // Deserialize the blob record
                let blob_data: serde_json::Value = serde_wasm_bindgen::from_value(record).map_err(|e| {
                    console::error!("[IdbBlobManager] Failed to deserialize blob record: {}", format!("{:?}", e));
                    IdbBlobError::SerializationError(format!("Failed to deserialize blob record: {:?}", e))
                })?;

                // Extract data array
                let data_array = blob_data.get("data")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| {
                        console::error!("[IdbBlobManager] Blob record missing or invalid data field");
                        IdbBlobError::SerializationError("Blob record missing or invalid data field".to_string())
                    })?;

                // Convert to Vec<u8>
                let data: Result<Vec<u8>, _> = data_array.iter()
                    .map(|v| v.as_u64().map(|n| n as u8))
                    .collect::<Option<Vec<u8>>>()
                    .ok_or_else(|| {
                        console::error!("[IdbBlobManager] Invalid data format in blob record");
                        IdbBlobError::SerializationError("Invalid data format in blob record".to_string())
                    });

                let data = data?;
                console::info!("[IdbBlobManager] Successfully retrieved blob {} ({} bytes)", cid, format_bytes(data.len() as u64));
                Ok(data)
            }
            None => {
                console::warn!("[IdbBlobManager] Blob {} not found in IndexedDB", cid);
                Err(IdbBlobError::NotFound(cid.to_string()))
            }
        }
    }

    /// Check if a blob exists in IndexedDB
    pub async fn has_blob(&self, cid: &str) -> bool {
        console::debug!("[IdbBlobManager] Checking if blob {} exists", cid);

        match self.get_blob_metadata(cid).await {
            Ok(_) => {
                console::debug!("[IdbBlobManager] Blob {} exists", cid);
                true
            }
            Err(_) => {
                console::debug!("[IdbBlobManager] Blob {} does not exist", cid);
                false
            }
        }
    }

    /// Get blob metadata
    async fn get_blob_metadata(&self, cid: &str) -> Result<BlobMetadata, IdbBlobError> {
        console::debug!("[IdbBlobManager] Retrieving metadata for blob {}", cid);

        let transaction = self.database
            .transaction(&[METADATA_STORE_NAME], TransactionMode::ReadOnly)
            .map_err(|e| IdbBlobError::TransactionError(format!("Failed to create metadata transaction: {:?}", e)))?;

        let metadata_store = transaction.object_store(METADATA_STORE_NAME)
            .map_err(|e| IdbBlobError::TransactionError(format!("Failed to get metadata store: {:?}", e)))?;

        let cid_js = JsValue::from_str(cid);
        let metadata_record: Option<JsValue> = metadata_store.get(cid_js)
            .map_err(IdbBlobError::from)?
            .await
            .map_err(IdbBlobError::from)?;

        transaction.await.map_err(IdbBlobError::from)?;

        match metadata_record {
            Some(record) => {
                let metadata_data: serde_json::Value = serde_wasm_bindgen::from_value(record)
                    .map_err(|e| IdbBlobError::SerializationError(format!("Failed to deserialize metadata: {:?}", e)))?;

                let metadata: BlobMetadata = serde_json::from_value(
                    metadata_data.get("metadata").unwrap_or(&serde_json::Value::Null).clone()
                ).map_err(|e| IdbBlobError::SerializationError(format!("Invalid metadata format: {:?}", e)))?;

                console::debug!("[IdbBlobManager] Retrieved metadata for blob {}", cid);
                Ok(metadata)
            }
            None => {
                console::debug!("[IdbBlobManager] No metadata found for blob {}", cid);
                Err(IdbBlobError::NotFound(cid.to_string()))
            }
        }
    }

    /// Get current storage usage in bytes
    pub async fn get_storage_usage(&self) -> Result<u64, IdbBlobError> {
        console::debug!("[IdbBlobManager] 📊 Calculating storage usage by iterating through metadata");

        let transaction = self.database
            .transaction(&[METADATA_STORE_NAME], TransactionMode::ReadOnly)
            .map_err(|e| {
                console::error!("[IdbBlobManager] ❌ Failed to create usage transaction: {}", format!("{:?}", e));
                IdbBlobError::TransactionError(format!("Failed to create usage transaction: {:?}", e))
            })?;

        let metadata_store = transaction.object_store(METADATA_STORE_NAME)
            .map_err(|e| {
                console::error!("[IdbBlobManager] ❌ Failed to get metadata store for usage: {}", format!("{:?}", e));
                IdbBlobError::TransactionError(format!("Failed to get metadata store for usage: {:?}", e))
            })?;

        console::debug!("[IdbBlobManager] 🔍 Opening cursor to iterate through all blob metadata...");
        
        // Open cursor to iterate through all metadata records
        let cursor_request = metadata_store.open_cursor(None, None)
            .map_err(|e| {
                console::error!("[IdbBlobManager] ❌ Failed to open cursor: {}", format!("{:?}", e));
                IdbBlobError::from(e)
            })?;
            
        let mut cursor_option = cursor_request.await
            .map_err(|e| {
                console::error!("[IdbBlobManager] ❌ Failed to get initial cursor: {}", format!("{:?}", e));
                IdbBlobError::from(e)
            })?;
        
        let mut total_bytes = 0u64;
        let mut blob_count = 0u32;
        
        // Iterate through all records
        while let Some(cursor) = cursor_option {
            console::debug!("[IdbBlobManager] 📝 Processing metadata record...");
            
            // Get the current record value
            let value = match cursor.value() {
                Ok(v) => v,
                Err(e) => {
                    console::warn!("[IdbBlobManager] ⚠️ Failed to get cursor value: {}", format!("{:?}", e));
                    // Skip this record and move to next
                    cursor_option = cursor.next(None)
                        .map_err(|e| {
                            console::error!("[IdbBlobManager] ❌ Failed to advance cursor after error: {}", format!("{:?}", e));
                            IdbBlobError::from(e)
                        })?
                        .await
                        .map_err(|e| {
                            console::error!("[IdbBlobManager] ❌ Failed to await cursor advance after error: {}", format!("{:?}", e));
                            IdbBlobError::from(e)
                        })?;
                    continue;
                }
            };
            match serde_wasm_bindgen::from_value::<serde_json::Value>(value) {
                Ok(metadata_data) => {
                    if let Some(metadata_obj) = metadata_data.get("metadata") {
                        match serde_json::from_value::<BlobMetadata>(metadata_obj.clone()) {
                            Ok(metadata) => {
                                total_bytes += metadata.size;
                                blob_count += 1;
                                console::debug!("[IdbBlobManager] 📊 Blob {} contributes {} bytes", metadata.cid, metadata.size.to_string());
                            }
                            Err(e) => {
                                console::warn!("[IdbBlobManager] ⚠️ Failed to parse metadata record: {}", format!("{:?}", e));
                                // Continue with other records
                            }
                        }
                    } else {
                        console::warn!("[IdbBlobManager] ⚠️ Metadata record missing 'metadata' field");
                    }
                }
                Err(e) => {
                    console::warn!("[IdbBlobManager] ⚠️ Failed to deserialize metadata record: {}", format!("{:?}", e));
                    // Continue with other records
                }
            }
            
            // Move to next record using next()
            cursor_option = cursor.next(None)
                .map_err(|e| {
                    console::error!("[IdbBlobManager] ❌ Failed to advance cursor: {}", format!("{:?}", e));
                    IdbBlobError::from(e)
                })?
                .await
                .map_err(|e| {
                    console::error!("[IdbBlobManager] ❌ Failed to await cursor advance: {}", format!("{:?}", e));
                    IdbBlobError::from(e)
                })?;
        }

        console::debug!("[IdbBlobManager] ✅ Cursor iteration completed");

        // Wait for transaction to complete
        transaction.await.map_err(|e| {
            console::error!("[IdbBlobManager] ❌ Transaction failed during storage usage calculation: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?;

        console::info!("[IdbBlobManager] 📊 Storage usage calculated: {} bytes across {} blobs", format_bytes(total_bytes), blob_count);
        Ok(total_bytes)
    }

    /// Clean up all blobs from IndexedDB
    pub async fn cleanup_blobs(&self) -> Result<(), IdbBlobError> {
        console::info!("[IdbBlobManager] 🧹 Starting cleanup of all blobs from IndexedDB");

        // Get storage usage before cleanup for reporting
        let usage_before = self.get_storage_usage().await.unwrap_or(0);
        console::info!("[IdbBlobManager] 📊 Storage usage before cleanup: {} bytes", format_bytes(usage_before));

        console::debug!("[IdbBlobManager] 📝 Creating cleanup transaction...");
        let transaction = self.database
            .transaction(&[BLOB_STORE_NAME, METADATA_STORE_NAME], TransactionMode::ReadWrite)
            .map_err(|e| {
                console::error!("[IdbBlobManager] ❌ Failed to create cleanup transaction: {}", format!("{:?}", e));
                IdbBlobError::TransactionError(format!("Failed to create cleanup transaction: {:?}", e))
            })?;

        console::debug!("[IdbBlobManager] 🗂️ Getting object stores for cleanup...");
        let blob_store = transaction.object_store(BLOB_STORE_NAME)
            .map_err(|e| {
                console::error!("[IdbBlobManager] ❌ Failed to get blob store for cleanup: {}", format!("{:?}", e));
                IdbBlobError::TransactionError(format!("Failed to get blob store for cleanup: {:?}", e))
            })?;

        let metadata_store = transaction.object_store(METADATA_STORE_NAME)
            .map_err(|e| {
                console::error!("[IdbBlobManager] ❌ Failed to get metadata store for cleanup: {}", format!("{:?}", e));
                IdbBlobError::TransactionError(format!("Failed to get metadata store for cleanup: {:?}", e))
            })?;

        console::debug!("[IdbBlobManager] 🗑️ Clearing blob data store...");
        blob_store.clear().map_err(|e| {
            console::error!("[IdbBlobManager] ❌ Failed to clear blob store: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?.await.map_err(|e| {
            console::error!("[IdbBlobManager] ❌ Failed to complete blob store clear: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?;

        console::debug!("[IdbBlobManager] 🗑️ Clearing metadata store...");
        metadata_store.clear().map_err(|e| {
            console::error!("[IdbBlobManager] ❌ Failed to clear metadata store: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?.await.map_err(|e| {
            console::error!("[IdbBlobManager] ❌ Failed to complete metadata store clear: {}", format!("{:?}", e));
            IdbBlobError::from(e)
        })?;

        console::debug!("[IdbBlobManager] 💾 Committing cleanup transaction...");
        transaction.commit().map_err(|e| {
            console::error!("[IdbBlobManager] ❌ Failed to commit cleanup transaction: {}", format!("{:?}", e));
            IdbBlobError::TransactionError(format!("Failed to commit cleanup: {:?}", e))
        })?.await.map_err(|e| {
            console::error!("[IdbBlobManager] ❌ Failed to complete cleanup commit: {}", format!("{:?}", e));
            IdbBlobError::TransactionError(format!("Failed to complete cleanup: {:?}", e))
        })?;

        console::info!("[IdbBlobManager] ✅ Successfully cleaned up all blobs from IndexedDB");
        console::info!("[IdbBlobManager] 📊 Freed approximately {} bytes of storage", format_bytes(usage_before));
        Ok(())
    }

    /// Get the storage backend name
    pub fn storage_name(&self) -> &'static str {
        "IndexedDB"
    }
}