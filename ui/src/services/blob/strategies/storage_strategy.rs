//! Storage-based blob migration strategy with retry capability

use async_trait::async_trait;
use dioxus::prelude::*;
// Import console macros from our crate
use crate::{console_info, console_warn, console_debug};

use crate::features::migration::types::MigrationAction;
use crate::services::{
    blob::{blob_fallback_manager::FallbackBlobManager, blob_manager_trait::BlobManagerTrait},
    client::{ClientMissingBlob, ClientSessionCredentials, PdsClient},
    errors::MigrationResult,
};

use super::{BlobFailure, BlobMigrationResult, MigrationStrategy};

/// Storage-based strategy that caches blobs locally before upload
pub struct StorageStrategy {
    use_local_cache: bool,
}

impl Default for StorageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageStrategy {
    pub fn new() -> Self {
        Self {
            use_local_cache: true,
        }
    }

    pub fn with_cache(use_local_cache: bool) -> Self {
        Self { use_local_cache }
    }
}

#[async_trait(?Send)]
impl MigrationStrategy for StorageStrategy {
    async fn migrate(
        &self,
        blobs: Vec<ClientMissingBlob>,
        old_session: ClientSessionCredentials,
        new_session: ClientSessionCredentials,
        blob_manager: &mut FallbackBlobManager,
        dispatch: &EventHandler<MigrationAction>,
    ) -> MigrationResult<BlobMigrationResult> {
        console_info!(
            "{}",
            format!(
                "[StorageStrategy] Starting storage-based blob migration with {} blobs",
                blobs.len()
            )
        );

        let pds_client = PdsClient::new();
        let mut uploaded_count = 0u32;
        let mut failed_blobs = Vec::new();
        let mut total_bytes = 0u64;
        let backend_name = blob_manager.storage_name();

        // Phase 1: Download and store blobs
        console_info!(
            "{}",
            format!(
                "[StorageStrategy] Phase 1: Downloading and caching {} blobs to {}",
                blobs.len(),
                backend_name
            )
        );

        let mut cached_blobs = Vec::new();

        for (index, blob) in blobs.iter().enumerate() {
            dispatch.call(MigrationAction::SetMigrationStep(format!(
                "Caching blob {} of {} to {} storage...",
                index + 1,
                blobs.len(),
                backend_name
            )));

            // Download blob from old PDS
            match pds_client.export_blob(&old_session, blob.cid.clone()).await {
                Ok(response) => {
                    if response.success {
                        let blob_data = response.blob_data.unwrap_or_default();
                        let blob_size = blob_data.len() as u64;
                        total_bytes += blob_size;

                        if self.use_local_cache {
                            // Store in local cache with retry logic
                            match blob_manager
                                .store_blob_with_retry(&blob.cid, blob_data.clone())
                                .await
                            {
                                Ok(()) => {
                                    console_debug!(
                                        "{}",
                                        format!(
                                            "[StorageStrategy] Cached blob {} ({} bytes) in {}",
                                            &blob.cid,
                                            blob_size,
                                            backend_name
                                        )
                                    );
                                    cached_blobs.push((blob.cid.clone(), blob_data));
                                }
                                Err(e) => {
                                    console_warn!(
                                        "[StorageStrategy] Failed to cache blob {}: {}",
                                        &blob.cid,
                                        &e.to_string()
                                    );
                                    failed_blobs.push(BlobFailure {
                                        cid: blob.cid.clone(),
                                        operation: "cache".to_string(),
                                        error: format!("{}", e),
                                    });
                                }
                            }
                        } else {
                            // Skip caching, use direct upload
                            cached_blobs.push((blob.cid.clone(), blob_data));
                        }
                    } else {
                        failed_blobs.push(BlobFailure {
                            cid: blob.cid.clone(),
                            operation: "download".to_string(),
                            error: response.message,
                        });
                    }
                }
                Err(e) => {
                    failed_blobs.push(BlobFailure {
                        cid: blob.cid.clone(),
                        operation: "download".to_string(),
                        error: format!("Request failed: {}", e),
                    });
                }
            }
        }

        // Phase 2: Upload blobs from cache to new PDS
        console_info!(
            "{}",
            format!(
                "[StorageStrategy] Phase 2: Uploading {} cached blobs to new PDS",
                cached_blobs.len()
            )
        );

        for (index, (cid, blob_data)) in cached_blobs.iter().enumerate() {
            dispatch.call(MigrationAction::SetMigrationStep(format!(
                "Uploading blob {} of {} from {} storage...",
                index + 1,
                cached_blobs.len(),
                backend_name
            )));

            match pds_client
                .upload_blob(&new_session, cid.clone(), blob_data.clone())
                .await
            {
                Ok(response) => {
                    if response.success {
                        uploaded_count += 1;
                        console_debug!(
                            "{}",
                            format!(
                                "[StorageStrategy] Uploaded blob {} from {} storage",
                                cid,
                                backend_name
                            )
                        );
                    } else {
                        failed_blobs.push(BlobFailure {
                            cid: cid.clone(),
                            operation: "upload".to_string(),
                            error: response.message,
                        });
                    }
                }
                Err(e) => {
                    failed_blobs.push(BlobFailure {
                        cid: cid.clone(),
                        operation: "upload".to_string(),
                        error: format!("Request failed: {}", e),
                    });
                }
            }
        }

        // Phase 3: Cleanup cached blobs if using local cache
        if self.use_local_cache {
            console_info!(
                "{}",
                format!(
                    "[StorageStrategy] Phase 3: Cleaning up {} cached blobs",
                    cached_blobs.len()
                )
            );
            if let Err(e) = blob_manager.cleanup_blobs().await {
                console_warn!(
                    "[StorageStrategy] Failed to cleanup cached blobs: {}",
                    &e.to_string()
                );
            }
        }

        console_info!(
            "{}",
            format!(
                "[StorageStrategy] Completed storage-based migration: {}/{} uploaded, {} failed",
                uploaded_count,
                blobs.len(),
                failed_blobs.len()
            )
        );

        Ok(BlobMigrationResult {
            total_blobs: blobs.len() as u32,
            uploaded_blobs: uploaded_count,
            failed_blobs,
            total_bytes_processed: total_bytes,
            strategy_used: self.name().to_string(),
        })
    }

    fn name(&self) -> &'static str {
        "storage"
    }

    fn supports_blob_count(&self, count: u32) -> bool {
        count <= 50 // Best for moderate number of blobs
    }

    fn supports_storage_backend(&self, backend: &str) -> bool {
        // Supports all backends except when caching is disabled
        self.use_local_cache || backend == "none"
    }

    fn priority(&self) -> u32 {
        60 // Medium priority
    }

    fn estimate_memory_usage(&self, blob_count: u32) -> u64 {
        if self.use_local_cache {
            // Estimate based on average blob size (assume 1MB per blob)
            blob_count as u64 * 1024 * 1024
        } else {
            // Minimal memory usage for direct transfer
            1024 * 1024 // 1MB base
        }
    }
}
