// use api::{export_blob, upload_blob, MissingBlob, SessionCredentials};
use crate::features::migration::*;
use crate::services::blob::blob_fallback_manager::FallbackBlobManager;
use crate::services::blob::blob_manager_trait::BlobManagerTrait;
use crate::services::blob::strategies::{BlobFailure, BlobMigrationResult, StrategySelector};
use crate::services::client::PdsClient;
use crate::services::client::{ClientMissingBlob, ClientSessionCredentials};
use crate::services::config::get_global_config;
use dioxus::prelude::*;
// Import console macros from our crate
use crate::{console_error, console_info, console_warn, console_debug};

// Enhanced imports for streaming and concurrent operations
use futures_util::{stream, StreamExt};
use reqwest::Client;
use std::sync::Arc;

// Tokio semaphore for concurrency control (works in WASM with current setup)
use tokio::sync::Semaphore;

/// Threshold for deciding between streaming vs storage-based transfer (5MB)
/// Currently unused since ClientMissingBlob doesn't include size info
#[allow(dead_code)]
const LARGE_BLOB_THRESHOLD: u64 = 5 * 1024 * 1024;

/// Smart blob migration using automatic strategy selection
///
/// Automatically chooses optimal migration approach based on:
/// - Blob count and estimated size
/// - Available storage capacity  
/// - Active storage backend capabilities
pub async fn smart_blob_migration(
    missing_blobs: Vec<ClientMissingBlob>,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
    blob_manager: &mut FallbackBlobManager,
    dispatch: &EventHandler<MigrationAction>,
) -> Result<BlobMigrationResult, String> {
    console_info!(
        "{}",
        format!(
            "[SmartBlobMigration] Starting smart blob migration with {} blobs",
            missing_blobs.len().to_string()
        )
    );

    // Get available memory estimate (in WASM this is challenging, so we use a conservative estimate)
    let available_memory = Some(100 * 1024 * 1024); // 100MB conservative estimate

    // Select optimal strategy
    let strategy =
        StrategySelector::select_strategy(&missing_blobs, blob_manager, available_memory);

    console_info!(
        "{}",
        format!(
            "[SmartBlobMigration] Using '{}' strategy for migration",
            strategy.name()
        )
    );

    // Execute the migration using the selected strategy
    match strategy
        .migrate(
            missing_blobs,
            old_session,
            new_session,
            blob_manager,
            dispatch,
        )
        .await
    {
        Ok(result) => {
            console_info!(
                "{}",
                format!(
                    "[SmartBlobMigration] Migration completed successfully with '{}' strategy",
                    &result.strategy_used
                )
            );
            Ok(result)
        }
        Err(e) => {
            console_error!("{}", format!("[SmartBlobMigration] Migration failed: {}", &e.to_string()));
            Err(format!("Smart migration failed: {}", e))
        }
    }
}

/// Handles the complete blob migration process with progress tracking
pub async fn migrate_blobs_with_progress(
    missing_blobs: Vec<ClientMissingBlob>,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
    blob_manager: &mut FallbackBlobManager,
    dispatch: &EventHandler<MigrationAction>,
) -> Result<BlobMigrationResult, String> {
    let total_blobs = missing_blobs.len();
    let mut downloaded_blobs = Vec::new();
    let mut total_blob_bytes = 0u64;
    let mut failed_blobs = Vec::new();

    console_info!(
        "{}",
        format!(
            "[BlobMigration] Starting migration of {} blobs",
            total_blobs
        )
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
                console_info!(
                    "{}",
                    format!(
                        "[BlobMigration] Downloaded blob {} ({} bytes)",
                        &missing_blob.cid,
                        blob_size
                    )
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
                console_error!(
                    "{}",
                    format!(
                        "[BlobMigration] Failed to download blob {}: {}",
                        &missing_blob.cid,
                        &error
                    )
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

    console_info!(
        "{}",
        format!(
            "[BlobMigration] Downloaded {} blobs successfully, {} failed",
            downloaded_blobs.len(),
            failed_blobs.len()
        )
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
                console_info!("{}", format!("[BlobMigration] Uploaded blob {} to new PDS", cid));
            }
            Err(error) => {
                console_error!("{}", format!("[BlobMigration] Failed to upload blob {}: {}", cid, &error));
                failed_blobs.push(BlobFailure {
                    cid: cid.to_string(),
                    operation: "upload".to_string(),
                    error: error.clone(),
                });
            }
        }
    }

    console_info!(
        "{}",
        format!(
            "[BlobMigration] Uploaded {} blobs successfully",
            uploaded_count
        )
    );

    Ok(BlobMigrationResult {
        total_blobs: total_blobs as u32,
        uploaded_blobs: uploaded_count,
        failed_blobs,
        total_bytes_processed: total_blob_bytes,
        strategy_used: "storage".to_string(),
    })
}

/// Downloads a single blob from the old PDS and caches it in LocalStorage
async fn download_and_cache_blob(
    cid: &str,
    old_session: &ClientSessionCredentials,
    blob_manager: &mut FallbackBlobManager,
) -> Result<(Vec<u8>, u64), String> {
    let pds_client = PdsClient::new();
    // Download from old PDS
    let blob_data = match pds_client
        .export_blob(&old_session.clone(), cid.to_string())
        .await
    {
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

    // Cache in storage with retry logic using the fallback manager
    match blob_manager
        .store_blob_with_retry(cid, blob_data.clone())
        .await
    {
        Ok(()) => Ok((blob_data, blob_size)),
        Err(error) => {
            // Try to free up space and retry once if quota exceeded
            if matches!(
                error,
                crate::services::blob::blob_manager_trait::BlobManagerError::QuotaExceeded(_)
            ) {
                console_warn!("[BlobMigration] Storage quota exceeded, attempting cleanup...");
                match blob_manager.cleanup_blobs().await {
                    Ok(()) => {
                        console_info!(
                            "[BlobMigration] Cleanup successful, retrying blob storage..."
                        );
                        match blob_manager
                            .store_blob_with_retry(cid, blob_data.clone())
                            .await
                        {
                            Ok(()) => Ok((blob_data, blob_size)),
                            Err(e) => Err(format!("Failed to cache blob after cleanup: {}", e)),
                        }
                    }
                    Err(cleanup_err) => Err(format!(
                        "Failed to cleanup storage and cache blob: {} (cleanup error: {})",
                        error, cleanup_err
                    )),
                }
            } else {
                Err(format!("Failed to cache blob: {}", error))
            }
        }
    }
}

/// Uploads a single blob to the new PDS
async fn upload_blob_to_pds(
    cid: &str,
    blob_data: &[u8],
    new_session: &ClientSessionCredentials,
) -> Result<(), String> {
    let pds_client = PdsClient::new();
    match pds_client
        .upload_blob(&new_session.clone(), cid.to_string(), blob_data.to_vec())
        .await
    {
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

/// Download a blob directly from old PDS to new PDS with retry and concurrency
/// This replaces streaming approach for better reliability and fallback handling
pub async fn migrate_blob_direct(
    client: &Client,
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    cid: &str,
) -> Result<(), String> {
    console_info!(
        "{}",
        format!(
            "[DirectMigration] 📦 Starting direct blob migration for {}",
            cid
        )
    );

    let config = get_global_config();
    let mut attempts = 0;

    while attempts < config.retry.migration_retries {
        attempts += 1;
        console_debug!(
            "{}",
            format!("[DirectMigration] Attempt {} for blob {}", attempts, cid)
        );

        // Download blob data from old PDS
        console_debug!("[DirectMigration] 📥 Downloading blob from old PDS...");
        let download_url = format!("{}/xrpc/com.atproto.sync.getBlob", old_session.pds);

        let blob_data = match client
            .get(&download_url)
            .bearer_auth(&old_session.access_jwt)
            .query(&[("did", &old_session.did), ("cid", &cid.to_string())])
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    match response.bytes().await {
                        Ok(bytes) => {
                            console_debug!(
                                "{}",
                                format!(
                                    "[DirectMigration] ✅ Downloaded {} bytes for blob {}",
                                    bytes.len(),
                                    cid
                                )
                            );
                            bytes.to_vec()
                        }
                        Err(e) => {
                            console_warn!("{}", format!("[DirectMigration] ⚠️ Failed to read response bytes on attempt {}: {}", attempts, e.to_string()));
                            if attempts >= config.retry.migration_retries {
                                return Err(format!("Failed to read response bytes: {}", e));
                            }
                            continue;
                        }
                    }
                } else {
                    let error = format!("Download failed with status: {}", response.status());
                    console_warn!(
                        "{}",
                        format!(
                            "[DirectMigration] ⚠️ Download failed on attempt {}: {}",
                            attempts,
                            &error
                        )
                    );
                    if attempts >= config.retry.migration_retries {
                        return Err(error);
                    }
                    continue;
                }
            }
            Err(e) => {
                console_warn!(
                    "{}",
                    format!(
                        "[DirectMigration] ⚠️ Download request failed on attempt {}: {}",
                        attempts,
                        e.to_string()
                    )
                );
                if attempts >= config.retry.migration_retries {
                    return Err(format!("Download request failed: {}", e));
                }
                continue;
            }
        };

        // Upload blob data to new PDS
        console_debug!("[DirectMigration] 📤 Uploading blob to new PDS...");
        let upload_url = format!("{}/xrpc/com.atproto.repo.uploadBlob", new_session.pds);

        match client
            .post(&upload_url)
            .bearer_auth(&new_session.access_jwt)
            .header("Content-Type", "application/octet-stream")
            .body(blob_data)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    console_info!(
                        "{}",
                        format!(
                            "[DirectMigration] ✅ Successfully migrated blob {} on attempt {}",
                            cid,
                            attempts
                        )
                    );
                    return Ok(());
                } else {
                    let error = format!("Upload failed with status: {}", response.status());
                    console_warn!(
                        "{}",
                        format!(
                            "[DirectMigration] ⚠️ Upload failed on attempt {}: {}",
                            attempts,
                            &error
                        )
                    );
                    if attempts >= config.retry.migration_retries {
                        return Err(error);
                    }
                }
            }
            Err(e) => {
                console_warn!(
                    "{}",
                    format!(
                        "[DirectMigration] ⚠️ Upload request failed on attempt {}: {}",
                        attempts,
                        e.to_string()
                    )
                );
                if attempts >= config.retry.migration_retries {
                    return Err(format!("Upload request failed: {}", e));
                }
            }
        }

        // Brief delay before retry (not implemented in WASM, but logged)
        if attempts < config.retry.migration_retries {
            console_debug!(
                "{}",
                format!("[DirectMigration] 🔄 Retrying blob {} migration...", cid)
            );
        }
    }

    Err(format!(
        "Failed to migrate blob {} after {} attempts",
        cid, config.retry.migration_retries
    ))
}

/// Concurrent blob migration using streaming with adaptive concurrency limits
pub async fn migrate_blobs_concurrent_adaptive(
    missing_blobs: Vec<ClientMissingBlob>,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
    _blob_manager: &mut FallbackBlobManager,
    dispatch: &EventHandler<MigrationAction>,
    max_concurrent: usize,
) -> Result<BlobMigrationResult, String> {
    let total_blobs = missing_blobs.len();
    console_info!(
        "{}",
        format!(
            "[ConcurrentMigration] 🚀 Starting concurrent migration of {} blobs with max {} concurrent transfers (adaptive)",
            total_blobs,
            max_concurrent
        )
    );

    // Create shared client and semaphore for adaptive concurrency control
    let client = Arc::new(Client::new());
    let semaphore = Arc::new(Semaphore::new(max_concurrent));

    let mut successful_transfers = 0u32;
    let mut failed_blobs = Vec::new();
    let total_bytes = 0u64;

    dispatch.call(MigrationAction::SetMigrationStep(format!(
        "Processing {} blobs with concurrent streaming transfer...",
        total_blobs
    )));

    // Process all blobs with streaming (concurrent)
    console_info!(
        "{}",
        format!(
            "[ConcurrentMigration] 🌊 Processing {} blobs with concurrent streaming...",
            total_blobs
        )
    );
    let stream_results = stream::iter(missing_blobs.into_iter().map(|blob| {
        let client = client.clone();
        let semaphore = semaphore.clone();
        let old_session = old_session.clone();
        let new_session = new_session.clone();

        migrate_single_blob_streaming(client, semaphore, blob, old_session, new_session)
    }))
    .buffer_unordered(max_concurrent)
    .collect::<Vec<_>>()
    .await;

    // Process streaming results
    for result in stream_results {
        match result {
            Ok(cid) => {
                successful_transfers += 1;
                console_info!(
                    "{}",
                    format!(
                        "[ConcurrentMigration] ✅ Streamed blob {} successfully",
                        cid
                    )
                );
            }
            Err(failure) => {
                console_error!(
                    "{}",
                    format!(
                        "[ConcurrentMigration] ❌ Streaming failed for {}: {}",
                        &failure.cid,
                        &failure.error
                    )
                );
                failed_blobs.push(failure);
            }
        }
    }

    let result = BlobMigrationResult {
        total_blobs: total_blobs as u32,
        uploaded_blobs: successful_transfers,
        failed_blobs,
        total_bytes_processed: total_bytes,
        strategy_used: "concurrent".to_string(),
    };

    console_info!(
        "{}",
        format!(
            "[ConcurrentMigration] 🏁 Concurrent migration completed: {}/{} successful, {} failed",
            successful_transfers,
            total_blobs,
            result.failed_blobs.len()
        )
    );

    Ok(result)
}

/// Migrate a single blob using streaming
async fn migrate_single_blob_streaming(
    client: Arc<Client>,
    semaphore: Arc<Semaphore>,
    missing_blob: ClientMissingBlob,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
) -> Result<String, BlobFailure> {
    let _permit = semaphore.acquire().await.map_err(|e| BlobFailure {
        cid: missing_blob.cid.clone(),
        operation: "semaphore_acquire".to_string(),
        error: format!("Semaphore error: {}", e),
    })?;

    let cid = missing_blob.cid.clone();
    console_debug!(
        "{}",
        format!("[StreamMigration] 🎫 Acquired permit for blob {}", &cid)
    );

    match migrate_blob_direct(&client, &old_session, &new_session, &cid).await {
        Ok(()) => {
            console_debug!(
                "{}",
                format!("[StreamMigration] 🎫 Releasing permit for blob {}", &cid)
            );
            Ok(cid)
        }
        Err(e) => Err(BlobFailure {
            cid,
            operation: "stream_transfer".to_string(),
            error: e,
        }),
    }
}

/// Migrate a single blob using storage fallback (for small blobs)
/// Currently unused in favor of streaming approach
#[allow(dead_code)]
async fn migrate_single_blob_with_storage(
    missing_blob: ClientMissingBlob,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
) -> Result<(u64, String, Vec<u8>), BlobFailure> {
    let cid = missing_blob.cid.clone(); // Clone before borrowing in console log
    console_debug!(
        "{}",
        format!(
            "[StorageMigration] 💾 Processing blob {} with storage",
            &cid
        )
    );

    // Download blob data
    let pds_client = PdsClient::new();
    let blob_data = match pds_client.export_blob(&old_session, cid.clone()).await {
        Ok(response) => {
            if response.success {
                response.blob_data.unwrap_or_default()
            } else {
                return Err(BlobFailure {
                    cid: cid.clone(),
                    operation: "download".to_string(),
                    error: response.message,
                });
            }
        }
        Err(e) => {
            return Err(BlobFailure {
                cid: cid.clone(),
                operation: "download".to_string(),
                error: format!("Export blob API call failed: {}", e),
            });
        }
    };

    // Upload to new PDS
    match pds_client
        .upload_blob(&new_session, cid.clone(), blob_data.clone())
        .await
    {
        Ok(response) => {
            if response.success {
                Ok((blob_data.len() as u64, cid, blob_data))
            } else {
                Err(BlobFailure {
                    cid: cid.clone(),
                    operation: "upload".to_string(),
                    error: response.message,
                })
            }
        }
        Err(e) => Err(BlobFailure {
            cid: cid.clone(),
            operation: "upload".to_string(),
            error: format!("Upload blob API call failed: {}", e),
        }),
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
        "Migrated {}/{} blobs ({:.1} MB total) using {} strategy. {} uploads successful, {} failed.",
        result.uploaded_blobs,
        result.total_blobs,
        result.total_bytes_processed as f64 / 1_048_576.0, // Convert to MB
        result.strategy_used,
        result.uploaded_blobs,
        result.failed_blobs.len()
    )
}
