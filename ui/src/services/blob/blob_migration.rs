// use api::{export_blob, upload_blob, MissingBlob, SessionCredentials};
use dioxus::prelude::*;
use gloo_console as console;
use crate::services::client::{ClientMissingBlob, ClientSessionCredentials};
use crate::services::blob::blob_storage::{BlobError, BlobManager};
use crate::features::migration::*;
use crate::services::client::PdsClient;

/// Handles the complete blob migration process with progress tracking
pub async fn migrate_blobs_with_progress(
    missing_blobs: Vec<ClientMissingBlob>,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
    blob_manager: &mut BlobManager,
    dispatch: &EventHandler<MigrationAction>,
) -> Result<BlobMigrationResult, String> {
    let total_blobs = missing_blobs.len();
    let mut downloaded_blobs = Vec::new();
    let mut total_blob_bytes = 0u64;
    let mut failed_blobs = Vec::new();

    console::info!(
        "[BlobMigration] Starting migration of {} blobs",
        total_blobs
    );

    // Phase 1: Download and cache blobs
    for (index, missing_blob) in missing_blobs.iter().enumerate() {
        dispatch.call(MigrationAction::SetMigrationStep(format!(
            "Downloading blob {} of {} to LocalStorage...",
            index + 1,
            total_blobs
        )));

        // Update progress
        update_blob_progress(
            dispatch,
            total_blobs as u32,
            index as u32,
            total_blob_bytes,
            Some(missing_blob.cid.clone()),
            Some(0.0),
        );

        match download_and_cache_blob(&missing_blob.cid, &old_session, blob_manager).await {
            Ok((blob_data, blob_size)) => {
                total_blob_bytes += blob_size;
                downloaded_blobs.push((missing_blob.cid.clone(), blob_data));
                console::info!(
                    "[BlobMigration] Downloaded blob {} ({} bytes)",
                    &missing_blob.cid,
                    blob_size
                );

                // Update progress to 100% for this blob
                update_blob_progress(
                    dispatch,
                    total_blobs as u32,
                    (index + 1) as u32,
                    total_blob_bytes,
                    Some(missing_blob.cid.clone()),
                    Some(100.0),
                );
            }
            Err(error) => {
                console::error!(
                    "[BlobMigration] Failed to download blob {}: {}",
                    &missing_blob.cid,
                    &error
                );
                failed_blobs.push(BlobFailure {
                    cid: missing_blob.cid.clone(),
                    operation: "download".to_string(),
                    error: error.clone(),
                });

                // Continue with other blobs, but track the failure
            }
        }
    }

    console::info!(
        "[BlobMigration] Downloaded {} blobs successfully, {} failed",
        downloaded_blobs.len(),
        failed_blobs.len()
    );

    // Phase 2: Upload blobs to new PDS
    let mut uploaded_count = 0u32;

    for (index, (cid, blob_data)) in downloaded_blobs.iter().enumerate() {
        dispatch.call(MigrationAction::SetMigrationStep(format!(
            "Uploading blob {} of {} to new PDS...",
            index + 1,
            downloaded_blobs.len()
        )));

        match upload_blob_to_pds(cid, blob_data, &new_session).await {
            Ok(()) => {
                uploaded_count += 1;
                console::info!("[BlobMigration] Uploaded blob {} to new PDS", cid);
            }
            Err(error) => {
                console::error!("[BlobMigration] Failed to upload blob {}: {}", cid, &error);
                failed_blobs.push(BlobFailure {
                    cid: cid.to_string(),
                    operation: "upload".to_string(),
                    error: error.clone(),
                });
            }
        }
    }

    console::info!(
        "[BlobMigration] Uploaded {} blobs successfully",
        uploaded_count
    );

    Ok(BlobMigrationResult {
        total_blobs: total_blobs as u32,
        downloaded_blobs: downloaded_blobs.len() as u32,
        uploaded_blobs: uploaded_count,
        failed_blobs,
        total_bytes: total_blob_bytes,
    })
}

/// Downloads a single blob from the old PDS and caches it in LocalStorage
async fn download_and_cache_blob(
    cid: &str,
    old_session: &ClientSessionCredentials,
    blob_manager: &mut BlobManager,
) -> Result<(Vec<u8>, u64), String> {
    let pds_client = PdsClient::new();
    // Download from old PDS
    let blob_data = match pds_client.export_blob(&old_session.clone(), cid.to_string()).await {
        Ok(response) => {
            if response.success {
                response.blob_data.unwrap_or_default()
            } else {
                return Err(response.message);
            }
        }
        Err(e) => return Err(format!("Export blob API call failed: {}", e)),
    };

    let blob_size = blob_data.len() as u64;

    // Cache in LocalStorage with retry logic
    match blob_manager
        .store_blob_with_retry(cid, blob_data.clone())
        .await
    {
        Ok(()) => Ok((blob_data, blob_size)),
        Err(BlobError::StorageQuotaExceeded) => {
            // Try to free up space and retry once
            blob_manager
                .cleanup_oldest_blobs()
                .await
                .map_err(|e| format!("{}", e))?;
            match blob_manager
                .store_blob_with_retry(cid, blob_data.clone())
                .await
            {
                Ok(()) => Ok((blob_data, blob_size)),
                Err(e) => Err(format!("Failed to cache blob after cleanup: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to cache blob: {}", e)),
    }
}

/// Uploads a single blob to the new PDS
async fn upload_blob_to_pds(
    cid: &str,
    blob_data: &[u8],
    new_session: &ClientSessionCredentials,
) -> Result<(), String> {
    let pds_client = PdsClient::new();
    match pds_client.upload_blob(&new_session.clone(), cid.to_string(), blob_data.to_vec()).await {
        Ok(response) => {
            if response.success {
                Ok(())
            } else {
                Err(response.message)
            }
        }
        Err(e) => Err(format!("Upload blob API call failed: {}", e)),
    }
}

/// Updates blob migration progress for the UI
fn update_blob_progress(
    dispatch: &EventHandler<MigrationAction>,
    total_blobs: u32,
    processed_blobs: u32,
    total_bytes: u64,
    current_blob_cid: Option<String>,
    current_blob_progress: Option<f32>,
) {
    let blob_progress = BlobProgress {
        total_blobs,
        processed_blobs,
        total_bytes,
        processed_bytes: total_bytes, // For simplicity, assume processed = total so far
        current_blob_cid,
        current_blob_progress: current_blob_progress.map(|p| p as f64),
        error: None,
    };
    dispatch.call(MigrationAction::SetBlobProgress(blob_progress));
}

/// Calculates estimated time remaining for blob migration
pub fn estimate_blob_migration_time(
    processed_blobs: u32,
    total_blobs: u32,
    elapsed_seconds: u64,
) -> Option<u64> {
    if processed_blobs == 0 || elapsed_seconds == 0 {
        return None;
    }

    let blobs_per_second = processed_blobs as f64 / elapsed_seconds as f64;
    let remaining_blobs = total_blobs.saturating_sub(processed_blobs) as f64;
    let estimated_seconds = (remaining_blobs / blobs_per_second) as u64;

    Some(estimated_seconds)
}

/// Formats blob migration statistics for display
pub fn format_blob_migration_stats(result: &BlobMigrationResult) -> String {
    format!(
        "Migrated {}/{} blobs ({:.1} MB total). {} uploads successful, {} failed.",
        result.uploaded_blobs,
        result.total_blobs,
        result.total_bytes as f64 / 1_048_576.0, // Convert to MB
        result.uploaded_blobs,
        result.failed_blobs.len()
    )
}

/// Result of blob migration operation
#[derive(Debug, Clone)]
pub struct BlobMigrationResult {
    pub total_blobs: u32,
    pub downloaded_blobs: u32,
    pub uploaded_blobs: u32,
    pub failed_blobs: Vec<BlobFailure>,
    pub total_bytes: u64,
}

/// Information about a failed blob operation
#[derive(Debug, Clone)]
pub struct BlobFailure {
    pub cid: String,
    pub operation: String, // "download" or "upload"
    pub error: String,
}

/// Cleanup strategy for LocalStorage when approaching quota limits
pub enum CleanupStrategy {
    /// Remove oldest blobs first (by timestamp)
    OldestFirst,
    /// Remove largest blobs first
    LargestFirst,
    /// Remove blobs that failed to upload
    FailedOnly,
}

/// Implements cleanup strategies for blob storage management
impl BlobManager {
    /// Cleanup oldest blobs to free up space
    pub async fn cleanup_oldest_blobs(&mut self) -> Result<(), BlobError> {
        console::info!("[BlobMigration] Cleaning up oldest blobs to free space");

        // For now, implement a simple cleanup - remove all cached blobs
        // In a more sophisticated implementation, we'd track timestamps
        self.cleanup_blobs().await
    }

    /// Get current storage usage statistics
    pub async fn get_storage_stats(&self) -> Result<StorageStats, BlobError> {
        // This would query LocalStorage to get current usage
        // For now, return basic stats
        Ok(StorageStats {
            total_blobs: 0,
            total_bytes: 0,
            available_bytes: 50 * 1024 * 1024, // 50MB default limit
        })
    }
}

/// Statistics about current blob storage usage
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_blobs: u32,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_blob_migration_time() {
        // Test normal case
        assert_eq!(estimate_blob_migration_time(5, 10, 10), Some(10));

        // Test edge cases
        assert_eq!(estimate_blob_migration_time(0, 10, 10), None);
        assert_eq!(estimate_blob_migration_time(5, 10, 0), None);
        assert_eq!(estimate_blob_migration_time(10, 10, 10), Some(0));
    }

    #[test]
    fn test_format_blob_migration_stats() {
        let result = BlobMigrationResult {
            total_blobs: 10,
            downloaded_blobs: 8,
            uploaded_blobs: 7,
            failed_blobs: vec![],
            total_bytes: 1_048_576, // 1 MB
        };

        let formatted = format_blob_migration_stats(&result);
        assert!(formatted.contains("7/10"));
        assert!(formatted.contains("1.0 MB"));
        assert!(formatted.contains("7 uploads successful"));
    }
}
