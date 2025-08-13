//! Unified Fallback Blob Storage Manager
//!
//! This module provides a unified blob storage manager that automatically selects
//! the best available storage backend in the following priority order:
//! 1. OPFS (Origin Private File System) - Best performance, unlimited storage
//! 2. IndexedDB - Good balance of performance and compatibility
//! 3. LocalStorage - Maximum compatibility, limited storage
//!
//! The manager provides comprehensive logging to help users understand which
//! storage backend is being used and why fallbacks occur.

use async_trait::async_trait;
use gloo_console as console;

use crate::services::blob::blob_manager_trait::{BlobManagerTrait, BlobManagerError};

/// Helper function to safely format u64 values for logging to avoid BigInt serialization issues
fn format_bytes(bytes: u64) -> String {
    if bytes == u64::MAX {
        "unlimited".to_string()
    } else {
        bytes.to_string()
    }
}
use crate::services::blob::blob_opfs_storage::OpfsBlobManager;
use crate::services::blob::blob_idb_storage::IdbBlobManager;
use crate::services::blob::blob_storage::BlobManager;

/// The fallback blob storage manager that tries different storage backends
/// in order of preference: OPFS → IndexedDB → LocalStorage
pub struct FallbackBlobManager {
    active_manager: ActiveManager,
}

/// Represents the currently active storage backend
enum ActiveManager {
    Opfs(OpfsBlobManager),
    IndexedDB(IdbBlobManager),
    LocalStorage(BlobManager),
}

impl FallbackBlobManager {
    /// Create a new fallback blob manager by trying each storage backend in priority order
    pub async fn new() -> Result<Self, BlobManagerError> {
        console::info!("🚀 [FallbackBlobManager] Initializing unified blob storage with intelligent fallback");
        console::info!("📋 [FallbackBlobManager] Priority order: OPFS (optimal) → IndexedDB (balanced) → LocalStorage (compatible)");
        
        // Phase 1: Try OPFS (Origin Private File System)
        console::info!("1️⃣ [FallbackBlobManager] Attempting OPFS initialization...");
        console::debug!("💡 [FallbackBlobManager] OPFS advantages: unlimited storage, direct file access, optimal performance");
        console::debug!("⚠️ [FallbackBlobManager] OPFS limitations: Chrome 86+, Firefox 111+, requires secure context");
        
        match OpfsBlobManager::new().await {
            Ok(manager) => {
                console::info!("✅ [FallbackBlobManager] OPFS storage initialized successfully!");
                console::info!("🚀 [FallbackBlobManager] Active backend: OPFS - optimal performance for large blob operations");
                
                // Test basic functionality to ensure it's working
                match manager.get_storage_usage().await {
                    Ok(usage) => {
                        console::info!("📊 [FallbackBlobManager] OPFS storage usage: {} bytes", format_bytes(usage));
                        console::debug!("✅ [FallbackBlobManager] OPFS functionality test passed");
                        return Ok(Self {
                            active_manager: ActiveManager::Opfs(manager),
                        });
                    }
                    Err(e) => {
                        console::warn!("⚠️ [FallbackBlobManager] OPFS usage check failed: {}", format!("{}", e));
                        console::info!("🔄 [FallbackBlobManager] Proceeding with OPFS despite usage check failure");
                        return Ok(Self {
                            active_manager: ActiveManager::Opfs(manager),
                        });
                    }
                }
            }
            Err(opfs_error) => {
                let error_msg = format!("{}", opfs_error);
                if error_msg.contains("SecurityError") || error_msg.contains("Security error") {
                    console::warn!("🔒 [FallbackBlobManager] OPFS blocked by security policy");
                    console::debug!("💡 [FallbackBlobManager] Common causes: private browsing, cross-origin context, browser restrictions");
                } else if error_msg.contains("NotSupportedError") {
                    console::warn!("🚫 [FallbackBlobManager] OPFS not supported in this browser");
                    console::debug!("💡 [FallbackBlobManager] OPFS requires modern browsers (Chrome 86+, Firefox 111+)");
                } else {
                    console::warn!("❌ [FallbackBlobManager] OPFS initialization failed: {}", error_msg);
                }
            }
        }

        // Phase 2: Try IndexedDB
        console::info!("2️⃣ [FallbackBlobManager] Attempting IndexedDB initialization...");
        console::debug!("💡 [FallbackBlobManager] IndexedDB advantages: ~1GB quota, structured storage, universal support");
        console::debug!("⚠️ [FallbackBlobManager] IndexedDB limitations: complex API, transaction overhead, quota varies by browser");
        
        match IdbBlobManager::new().await {
            Ok(manager) => {
                console::info!("✅ [FallbackBlobManager] IndexedDB storage initialized successfully!");
                console::info!("⚖️ [FallbackBlobManager] Active backend: IndexedDB - balanced performance and compatibility");
                
                // Test basic functionality
                match manager.get_storage_usage().await {
                    Ok(usage) => {
                        console::info!("📊 [FallbackBlobManager] IndexedDB storage usage: {} bytes", format_bytes(usage));
                        let usage_mb = usage as f64 / 1_048_576.0;
                        console::info!("📈 [FallbackBlobManager] IndexedDB usage: {:.2} MB (typical quota: ~1GB)", usage_mb);
                        console::debug!("✅ [FallbackBlobManager] IndexedDB functionality test passed");
                    }
                    Err(e) => {
                        console::warn!("⚠️ [FallbackBlobManager] IndexedDB usage check failed: {}", format!("{}", e));
                        console::info!("🔄 [FallbackBlobManager] Proceeding with IndexedDB despite usage check failure");
                    }
                }
                
                return Ok(Self {
                    active_manager: ActiveManager::IndexedDB(manager),
                });
            }
            Err(idb_error) => {
                let error_msg = format!("{}", idb_error);
                if error_msg.contains("NotSupportedError") {
                    console::warn!("🚫 [FallbackBlobManager] IndexedDB not supported in this browser");
                    console::debug!("💡 [FallbackBlobManager] This is extremely rare - IndexedDB has near-universal support");
                } else if error_msg.contains("quota") || error_msg.contains("QuotaExceededError") {
                    console::warn!("💾 [FallbackBlobManager] IndexedDB quota exceeded");
                    console::debug!("💡 [FallbackBlobManager] User may need to clear browser data or request more storage");
                } else if error_msg.contains("VersionError") {
                    console::warn!("🔄 [FallbackBlobManager] IndexedDB version conflict");
                    console::debug!("💡 [FallbackBlobManager] This may resolve on page refresh or browser restart");
                } else {
                    console::warn!("❌ [FallbackBlobManager] IndexedDB initialization failed: {}", error_msg);
                }
            }
        }

        // Phase 3: Final fallback to LocalStorage
        console::info!("3️⃣ [FallbackBlobManager] Attempting LocalStorage initialization (final fallback)...");
        console::debug!("💡 [FallbackBlobManager] LocalStorage advantages: universal support, simple API, no permissions needed");
        console::warn!("⚠️ [FallbackBlobManager] LocalStorage limitations: ~5-10MB quota, base64 overhead (33%), synchronous API");
        console::warn!("🚨 [FallbackBlobManager] Performance warning: LocalStorage may block UI for large operations");
        
        match BlobManager::new().await {
            Ok(manager) => {
                console::info!("✅ [FallbackBlobManager] LocalStorage initialized successfully!");
                console::warn!("⚠️ [FallbackBlobManager] Active backend: LocalStorage - maximum compatibility but limited capacity");
                
                // Check available storage space
                let storage_info = manager.get_storage_info().await.unwrap_or_else(|_| {
                    console::warn!("🔍 [FallbackBlobManager] Could not retrieve LocalStorage info, using defaults");
                    crate::services::blob::blob_storage::StorageInfo {
                        current_usage_bytes: 0,
                        max_storage_bytes: 50 * 1024 * 1024, // 50MB
                        available_bytes: 50 * 1024 * 1024,
                        blob_count: 0,
                    }
                });
                
                let usage_mb = storage_info.current_usage_bytes as f64 / 1_048_576.0;
                let available_mb = storage_info.available_bytes as f64 / 1_048_576.0;
                let usage_pct = storage_info.usage_percentage();
                
                console::info!("📊 [FallbackBlobManager] LocalStorage status:");
                console::info!("   📈 Usage: {:.1} MB ({:.1}% of quota)", usage_mb, usage_pct);
                console::info!("   💾 Available: {:.1} MB", available_mb);
                console::info!("   📦 Stored blobs: {}", storage_info.blob_count);
                
                if storage_info.is_near_capacity() {
                    console::warn!("⚠️ [FallbackBlobManager] LocalStorage is near capacity - large blob migrations may fail");
                    console::info!("💡 [FallbackBlobManager] Consider clearing browser data or reducing blob sizes");
                }
                
                Ok(Self {
                    active_manager: ActiveManager::LocalStorage(manager),
                })
            }
            Err(ls_error) => {
                let error_msg = format!(
                    "🔥 [FallbackBlobManager] ALL storage backends failed! This is critical.\n\
                     📋 Failure summary:\n\
                      OPFS: Failed to initialize (likely browser support issue)\n\
                      IndexedDB: Failed to initialize (likely quota or permission issue)\n\
                      LocalStorage: {} (critical - universal support expected)\n\
                     \n\
                     💡 Possible causes:\n\
                      Browser in private/incognito mode with strict storage policies\n\
                      All storage quotas completely exhausted\n\
                      Browser security settings blocking all storage APIs\n\
                      Corrupted browser profile or storage databases\n\
                      Extension or security software interfering\n\
                     \n\
                     🚨 Blob storage is completely unavailable - migration cannot proceed!",
                    ls_error
                );
                console::error!("{}", &error_msg);
                Err(BlobManagerError::StorageError(error_msg))
            }
        }
    }

    /// Get information about the currently active storage backend
    pub fn get_active_backend_info(&self) -> (&'static str, &'static str) {
        match &self.active_manager {
            ActiveManager::Opfs(_) => (
                "OPFS",
                "Origin Private File System - optimal performance, unlimited storage"
            ),
            ActiveManager::IndexedDB(_) => (
                "IndexedDB", 
                "Browser database - good performance, ~1GB quota"
            ),
            ActiveManager::LocalStorage(_) => (
                "LocalStorage",
                "Browser key-value storage - maximum compatibility, ~5-10MB limit"
            ),
        }
    }

    /// Log detailed capabilities and recommendations for the active backend
    pub fn log_active_backend_capabilities(&self) {
        let (name, description) = self.get_active_backend_info();
        
        console::info!("🔧 [FallbackBlobManager] Active Storage Backend: {}", name);
        console::info!("📝 [FallbackBlobManager] Description: {}", description);
        
        match &self.active_manager {
            ActiveManager::Opfs(_) => {
                console::info!("✨ [FallbackBlobManager] OPFS Capabilities:");
                console::info!("    Unlimited storage (subject to disk space)");
                console::info!("    Direct file access (optimal for large blobs)");
                console::info!("    Asynchronous operations (non-blocking)");
                console::info!("    Persistent across browser sessions");
                console::info!("    Best performance for blob operations");
                console::info!("💡 [FallbackBlobManager] Recommendations:");
                console::info!("    Suitable for large blob migrations (>100MB)");
                console::info!("    No special handling needed for blob sizes");
                console::info!("    Consider parallel operations for maximum throughput");
            }
            ActiveManager::IndexedDB(_) => {
                console::info!("⚖️ [FallbackBlobManager] IndexedDB Capabilities:");
                console::info!("    ~1GB storage quota (varies by browser)");
                console::info!("    Structured data storage with transactions");
                console::info!("    Asynchronous operations (non-blocking)");
                console::info!("    Good performance for medium-sized blobs");
                console::info!("    Universal browser support");
                console::info!("💡 [FallbackBlobManager] Recommendations:");
                console::info!("    Monitor quota usage during large migrations");
                console::info!("    Consider chunking blobs larger than 100MB");
                console::info!("    Good balance of performance and compatibility");
            }
            ActiveManager::LocalStorage(_) => {
                console::warn!("⚠️ [FallbackBlobManager] LocalStorage Limitations:");
                console::warn!("    ~5-10MB storage quota (browser dependent)");
                console::warn!("    Base64 encoding overhead (33% size increase)");
                console::warn!("    Synchronous API (may block UI)");
                console::warn!("    Not suitable for large blobs (>5MB each)");
                console::info!("💡 [FallbackBlobManager] Recommendations:");
                console::info!("    Limit blob migrations to small files only");
                console::info!("    Monitor quota usage carefully");
                console::info!("    Consider chunking or progressive processing");
                console::info!("    Warn users about potential UI blocking");
            }
        }
    }

    /// Check if storage is under pressure (>80% usage)
    /// Uses browser's StorageManager API when available
    pub async fn is_storage_under_pressure(&self) -> bool {
        if let Some(estimate) = crate::services::config::try_get_storage_estimate().await {
            console::debug!("📊 [FallbackBlobManager] Checking storage pressure: {:.1}% used", estimate.usage_percentage * 100.0);
            estimate.is_near_capacity()
        } else {
            console::debug!("📊 [FallbackBlobManager] Cannot determine storage pressure - StorageManager unavailable");
            false // Assume no pressure when we can't measure
        }
    }
    
    /// Check if a blob of given size would fit in current storage
    pub async fn can_fit_blob(&self, blob_size: u64) -> bool {
        if let Some(estimate) = crate::services::config::try_get_storage_estimate().await {
            let can_fit = estimate.can_fit_blob(blob_size);
            console::debug!("📊 [FallbackBlobManager] Blob size {} bytes: {} fit (available: {} bytes)", 
                          blob_size, if can_fit { "CAN" } else { "CANNOT" }, estimate.available_bytes());
            can_fit
        } else {
            console::debug!("📊 [FallbackBlobManager] Cannot check blob fit - StorageManager unavailable, assuming OK");
            true // Assume it fits when we can't measure
        }
    }

    /// Attempt to estimate storage capacity for planning blob migrations
    /// Uses browser's StorageManager API when available, falls back to conservative estimates
    pub async fn estimate_storage_capacity(&self) -> Result<u64, BlobManagerError> {
        console::debug!("📊 [FallbackBlobManager] Estimating storage capacity for active backend");
        
        // Try to get real browser storage information first
        if let Some(estimate) = crate::services::config::try_get_storage_estimate().await {
            console::info!("📊 [FallbackBlobManager] Using real browser storage data: {} bytes available", format_bytes(estimate.available_bytes()));
            
            // Return available storage, but apply backend-specific limits
            let available = estimate.available_bytes();
            return match &self.active_manager {
                ActiveManager::Opfs(_) => {
                    console::debug!("📊 [FallbackBlobManager] OPFS: using full available storage");
                    Ok(available) // OPFS can use all available storage
                }
                ActiveManager::IndexedDB(_) => {
                    console::debug!("📊 [FallbackBlobManager] IndexedDB: using 80% of available storage");
                    // IndexedDB should leave some headroom for other storage
                    Ok((available as f64 * 0.8) as u64)
                }
                ActiveManager::LocalStorage(_) => {
                    console::debug!("📊 [FallbackBlobManager] LocalStorage: using 10% of available storage");
                    // LocalStorage should be very conservative due to base64 overhead
                    let ls_limit = (available as f64 * 0.1) as u64;
                    Ok(std::cmp::min(ls_limit, 10 * 1024 * 1024)) // Cap at 10MB
                }
            };
        }
        
        // Fallback to hardcoded estimates when StorageManager unavailable
        console::debug!("📊 [FallbackBlobManager] StorageManager unavailable, using conservative estimates");
        
        match &self.active_manager {
            ActiveManager::Opfs(_) => {
                console::debug!("📊 [FallbackBlobManager] OPFS capacity: effectively unlimited (fallback)");
                // OPFS doesn't have a fixed limit, return a very large number
                Ok(u64::MAX)
            }
            ActiveManager::IndexedDB(_) => {
                console::debug!("📊 [FallbackBlobManager] IndexedDB capacity: ~1GB typical (fallback)");
                // IndexedDB quota varies but typically around 1GB
                Ok(1024 * 1024 * 1024) // 1GB
            }
            ActiveManager::LocalStorage(_) => {
                console::debug!("📊 [FallbackBlobManager] LocalStorage capacity: ~5-10MB with base64 overhead (fallback)");
                // LocalStorage has strict limits, account for base64 overhead
                Ok(50 * 1024 * 1024) // 50MB conservative estimate
            }
        }
    }

    /// Attempt to switch to a fallback backend when the current backend fails
    /// This enables mid-operation recovery and cross-backend data migration
    pub async fn try_fallback_backend(&mut self, reason: &str) -> Result<(), BlobManagerError> {
        let (current_backend, _) = self.get_active_backend_info();
        console::warn!("🔄 [FallbackBlobManager] Attempting backend fallback from {} due to: {}", current_backend, reason);
        
        // Determine next available backend based on current state
        let next_backend_result = match &self.active_manager {
            ActiveManager::Opfs(_) => {
                console::info!("⬇️ [FallbackBlobManager] Falling back from OPFS to IndexedDB...");
                IdbBlobManager::new().await.map(ActiveManager::IndexedDB)
                    .map_err(BlobManagerError::from)
            }
            ActiveManager::IndexedDB(_) => {
                console::info!("⬇️ [FallbackBlobManager] Falling back from IndexedDB to LocalStorage...");
                BlobManager::new().await.map(ActiveManager::LocalStorage)
                    .map_err(BlobManagerError::from)
            }
            ActiveManager::LocalStorage(_) => {
                console::error!("🚨 [FallbackBlobManager] Already at final fallback (LocalStorage) - cannot fall back further!");
                return Err(BlobManagerError::StorageError(
                    "No further fallback backends available - all storage options exhausted".to_string()
                ));
            }
        };

        match next_backend_result {
            Ok(new_active_manager) => {
                let old_backend = current_backend;
                self.active_manager = new_active_manager;
                let (new_backend, new_description) = self.get_active_backend_info();
                
                console::info!("✅ [FallbackBlobManager] Successfully switched from {} to {}", old_backend, new_backend);
                console::info!("📋 [FallbackBlobManager] New backend: {}", new_description);
                
                // Log capabilities of new backend
                self.log_active_backend_capabilities();
                
                Ok(())
            }
            Err(e) => {
                console::error!("❌ [FallbackBlobManager] Fallback to next backend failed: {}", e.to_string());
                Err(e)
            }
        }
    }

    /// Advanced storage operation with automatic fallback on failure
    /// This provides resilient blob storage that can recover from backend failures
    pub async fn store_blob_with_fallback(&mut self, cid: &str, data: Vec<u8>) -> Result<(), BlobManagerError> {
        console::debug!("🔄 [FallbackBlobManager] Storing blob {} with automatic fallback capability ({} bytes)", cid, data.len());
        
        // Check if storage is under pressure before attempting to store
        if self.is_storage_under_pressure().await {
            console::warn!("⚠️ [FallbackBlobManager] Storage is under pressure (>80% used) - attempting storage anyway but may trigger fallback");
        }
        
        // Check if blob would fit
        if !self.can_fit_blob(data.len() as u64).await {
            console::warn!("⚠️ [FallbackBlobManager] Blob may not fit in available storage - attempting anyway");
        }
        
        let max_fallback_attempts = 3; // Try current backend + 2 fallbacks max
        let mut attempt_count = 0;
        
        loop {
            attempt_count += 1;
            let (current_backend, _) = self.get_active_backend_info();
            
            console::debug!("🎯 [FallbackBlobManager] Storage attempt {} using {} backend", attempt_count, current_backend);
            
            // Try storing with current backend
            match self.store_blob_with_retry(cid, data.clone()).await {
                Ok(()) => {
                    console::info!("✅ [FallbackBlobManager] Successfully stored blob {} using {} backend on attempt {}", 
                                   cid, current_backend, attempt_count);
                    return Ok(());
                }
                Err(error) => {
                    console::warn!("⚠️ [FallbackBlobManager] Storage attempt {} failed with {}: {}", 
                                   attempt_count, current_backend, error.to_string());
                    
                    // Check if we should attempt fallback
                    if attempt_count >= max_fallback_attempts {
                        console::error!("🚨 [FallbackBlobManager] Exhausted all fallback attempts ({}) for blob {}", 
                                        max_fallback_attempts, cid);
                        return Err(error);
                    }
                    
                    // Determine if error is fallback-worthy
                    let should_fallback = match &error {
                        BlobManagerError::QuotaExceeded(_) => {
                            console::info!("💾 [FallbackBlobManager] Quota exceeded - fallback recommended");
                            true
                        }
                        BlobManagerError::StorageError(msg) if msg.contains("quota") || msg.contains("storage") => {
                            console::info!("💾 [FallbackBlobManager] Storage error detected - fallback recommended");
                            true
                        }
                        BlobManagerError::StorageError(msg) if msg.contains("failed") || msg.contains("error") => {
                            console::info!("❌ [FallbackBlobManager] Backend failure detected - fallback recommended");
                            true
                        }
                        _ => {
                            console::debug!("🤔 [FallbackBlobManager] Error not suitable for fallback: {}", error.to_string());
                            false
                        }
                    };
                    
                    if should_fallback {
                        // Attempt to switch to fallback backend
                        match self.try_fallback_backend(&format!("Storage failure: {}", error)).await {
                            Ok(()) => {
                                let (new_backend, _) = self.get_active_backend_info();
                                console::info!("🔄 [FallbackBlobManager] Successfully switched to {} - retrying blob storage", new_backend);
                                continue; // Retry with new backend
                            }
                            Err(fallback_error) => {
                                console::error!("💥 [FallbackBlobManager] Fallback failed: {}", fallback_error.to_string());
                                return Err(error); // Return original error
                            }
                        }
                    } else {
                        // Error not suitable for fallback, return immediately
                        return Err(error);
                    }
                }
            }
        }
    }

    /// Cross-backend blob recovery - attempt to retrieve a blob from any available backend
    /// This is useful when blobs might be stored in different backends during migrations
    pub async fn retrieve_blob_with_fallback(&self, cid: &str) -> Result<Vec<u8>, BlobManagerError> {
        console::debug!("🔍 [FallbackBlobManager] Attempting cross-backend blob retrieval for {}", cid);
        
        // First try current backend
        let (current_backend, _) = self.get_active_backend_info();
        console::debug!("🎯 [FallbackBlobManager] Trying current backend: {}", current_backend);
        
        match self.retrieve_blob(cid).await {
            Ok(data) => {
                console::info!("✅ [FallbackBlobManager] Found blob {} in current backend ({})", cid, current_backend);
                return Ok(data);
            }
            Err(error) => {
                console::debug!("⚠️ [FallbackBlobManager] Current backend ({}) doesn't have blob {}: {}", current_backend, cid, error.to_string());
            }
        }
        
        // Try other backends if current backend doesn't have the blob
        console::info!("🔄 [FallbackBlobManager] Searching other backends for blob {}", cid);
        
        // Try OPFS if not current
        if !matches!(&self.active_manager, ActiveManager::Opfs(_)) {
            console::debug!("🔍 [FallbackBlobManager] Checking OPFS backend for blob {}", cid);
            if let Ok(opfs_manager) = OpfsBlobManager::new().await {
                if let Ok(data) = opfs_manager.retrieve_blob(cid).await {
                    console::info!("✅ [FallbackBlobManager] Found blob {} in OPFS backup", cid);
                    return Ok(data);
                }
            }
        }
        
        // Try IndexedDB if not current
        if !matches!(&self.active_manager, ActiveManager::IndexedDB(_)) {
            console::debug!("🔍 [FallbackBlobManager] Checking IndexedDB backend for blob {}", cid);
            if let Ok(idb_manager) = IdbBlobManager::new().await {
                if let Ok(data) = idb_manager.retrieve_blob(cid).await {
                    console::info!("✅ [FallbackBlobManager] Found blob {} in IndexedDB backup", cid);
                    return Ok(data);
                }
            }
        }
        
        // Try LocalStorage if not current
        if !matches!(&self.active_manager, ActiveManager::LocalStorage(_)) {
            console::debug!("🔍 [FallbackBlobManager] Checking LocalStorage backend for blob {}", cid);
            if let Ok(ls_manager) = BlobManager::new().await {
                if let Ok(data) = ls_manager.retrieve_blob(cid).await {
                    console::info!("✅ [FallbackBlobManager] Found blob {} in LocalStorage backup", cid);
                    return Ok(data);
                }
            }
        }
        
        console::warn!("❌ [FallbackBlobManager] Blob {} not found in any backend", cid);
        Err(BlobManagerError::BlobNotFound(format!("Blob {} not found in any storage backend", cid)))
    }

    /// Migrate blobs between backends (useful for upgrading storage or recovering from failures)
    pub async fn migrate_blobs_between_backends(&mut self, from_backend: &str, to_backend: &str) -> Result<u32, BlobManagerError> {
        console::info!("🚚 [FallbackBlobManager] Starting cross-backend migration from {} to {}", from_backend, to_backend);
        
        // This is a placeholder for cross-backend migration
        // In a full implementation, this would:
        // 1. List all blobs in the source backend
        // 2. Read each blob from source
        // 3. Write each blob to destination
        // 4. Verify integrity
        // 5. Clean up source (optionally)
        
        console::warn!("⚠️ [FallbackBlobManager] Cross-backend migration is not yet implemented");
        console::info!("💡 [FallbackBlobManager] For now, use retrieve_blob_with_fallback to access blobs from any backend");
        
        Ok(0) // Return 0 migrated blobs for now
    }
}

/// Implement the unified BlobManagerTrait for the fallback manager
#[async_trait(?Send)]
impl BlobManagerTrait for FallbackBlobManager {
    async fn store_blob(&self, cid: &str, data: Vec<u8>) -> Result<(), BlobManagerError> {
        console::debug!("💾 [FallbackBlobManager] Storing blob {} ({} bytes)", cid, data.len());
        
        match &self.active_manager {
            ActiveManager::Opfs(manager) => manager.store_blob(cid, data).await.map_err(BlobManagerError::from),
            ActiveManager::IndexedDB(manager) => manager.store_blob(cid, data).await.map_err(BlobManagerError::from),
            ActiveManager::LocalStorage(manager) => manager.store_blob(cid, data).await,
        }
    }

    async fn store_blob_with_retry(&self, cid: &str, data: Vec<u8>) -> Result<(), BlobManagerError> {
        console::debug!("🔄 [FallbackBlobManager] Storing blob {} with retry logic ({} bytes)", cid, data.len());
        
        match &self.active_manager {
            ActiveManager::Opfs(manager) => manager.store_blob_with_retry(cid, data).await.map_err(BlobManagerError::from),
            ActiveManager::IndexedDB(manager) => manager.store_blob_with_retry(cid, data).await.map_err(BlobManagerError::from),
            ActiveManager::LocalStorage(manager) => manager.store_blob_with_retry(cid, data).await.map_err(BlobManagerError::from),
        }
    }

    async fn retrieve_blob(&self, cid: &str) -> Result<Vec<u8>, BlobManagerError> {
        console::debug!("📖 [FallbackBlobManager] Retrieving blob {}", cid);
        
        match &self.active_manager {
            ActiveManager::Opfs(manager) => manager.retrieve_blob(cid).await.map_err(BlobManagerError::from),
            ActiveManager::IndexedDB(manager) => manager.retrieve_blob(cid).await.map_err(BlobManagerError::from),
            ActiveManager::LocalStorage(manager) => manager.retrieve_blob(cid).await,
        }
    }

    async fn has_blob(&self, cid: &str) -> bool {
        console::debug!("🔍 [FallbackBlobManager] Checking if blob {} exists", cid);
        
        match &self.active_manager {
            ActiveManager::Opfs(manager) => manager.has_blob(cid).await,
            ActiveManager::IndexedDB(manager) => manager.has_blob(cid).await,
            ActiveManager::LocalStorage(manager) => manager.has_blob(cid).await,
        }
    }

    async fn cleanup_blobs(&self) -> Result<(), BlobManagerError> {
        console::info!("🧹 [FallbackBlobManager] Cleaning up all blobs from active backend");
        
        match &self.active_manager {
            ActiveManager::Opfs(manager) => manager.cleanup_blobs().await.map_err(BlobManagerError::from),
            ActiveManager::IndexedDB(manager) => manager.cleanup_blobs().await.map_err(BlobManagerError::from),
            ActiveManager::LocalStorage(manager) => manager.cleanup_blobs().await,
        }
    }

    async fn get_storage_usage(&self) -> Result<u64, BlobManagerError> {
        console::debug!("📊 [FallbackBlobManager] Getting storage usage from active backend");
        
        match &self.active_manager {
            ActiveManager::Opfs(manager) => manager.get_storage_usage().await.map_err(BlobManagerError::from),
            ActiveManager::IndexedDB(manager) => manager.get_storage_usage().await.map_err(BlobManagerError::from),
            ActiveManager::LocalStorage(manager) => manager.get_storage_usage().await,
        }
    }

    fn storage_name(&self) -> &'static str {
        match &self.active_manager {
            ActiveManager::Opfs(manager) => manager.storage_name(),
            ActiveManager::IndexedDB(manager) => manager.storage_name(),
            ActiveManager::LocalStorage(manager) => manager.storage_name(),
        }
    }
}

/// Helper function to create a fallback blob manager with comprehensive initialization logging
pub async fn create_fallback_blob_manager() -> Result<FallbackBlobManager, BlobManagerError> {
    console::info!("🚀 [create_fallback_blob_manager] Starting intelligent blob storage initialization");
    
    let manager = FallbackBlobManager::new().await?;
    
    // Log detailed information about the selected backend
    let (backend_name, _) = manager.get_active_backend_info();
    console::info!("✅ [create_fallback_blob_manager] Blob storage initialized with {} backend", backend_name);
    
    // Log capabilities and recommendations
    manager.log_active_backend_capabilities();
    
    // Estimate and log capacity information
    if let Ok(capacity) = manager.estimate_storage_capacity().await {
        if capacity == u64::MAX {
            console::info!("📊 [create_fallback_blob_manager] Estimated capacity: Unlimited (OPFS)");
        } else {
            let capacity_mb = capacity as f64 / 1_048_576.0;
            console::info!("📊 [create_fallback_blob_manager] Estimated capacity: {} bytes ({:.0} MB)", format_bytes(capacity), capacity_mb);
        }
    }
    
    console::info!("🎉 [create_fallback_blob_manager] Fallback blob manager ready for operations");
    Ok(manager)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fallback_manager_creation() {
        // This test will try to create a manager and should succeed with at least LocalStorage
        let result = FallbackBlobManager::new().await;
        assert!(result.is_ok(), "Fallback manager should succeed with at least one backend");
    }

    #[tokio::test]
    async fn test_backend_info() {
        if let Ok(manager) = FallbackBlobManager::new().await {
            let (name, description) = manager.get_active_backend_info();
            assert!(!name.is_empty(), "Backend name should not be empty");
            assert!(!description.is_empty(), "Backend description should not be empty");
        }
    }

    #[tokio::test]
    async fn test_storage_capacity_estimation() {
        if let Ok(manager) = FallbackBlobManager::new().await {
            let capacity = manager.estimate_storage_capacity().await;
            assert!(capacity.is_ok(), "Should be able to estimate capacity");
            assert!(capacity.unwrap() > 0, "Capacity should be greater than 0");
        }
    }
}