//! Blob migration step using streaming architecture

#[cfg(feature = "web")]
use crate::services::client::{ClientSessionCredentials, PdsClient, RefreshableSessionProvider};
use crate::services::streaming::{
    BlobSource, BlobTarget, BufferedStorage, DataSource, DataTarget, ProgressEvent, ProgressPhase,
    ProgressUpdate, SyncOrchestrator,
};
use crate::{console_error, console_info, console_warn};
use dioxus::prelude::*;
use std::sync::Arc;

use crate::migration::types::*;

pub async fn execute_streaming_blob_migration(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    console_info!("[Migration] Starting blob migration using streaming architecture");

    // UPDATE UI IMMEDIATELY before any async operations
    dispatch.call(MigrationAction::SetMigrationStep(
        "Listing blobs from source PDS...".to_string(),
    ));

    // Create WASM streaming orchestrator
    let orchestrator = SyncOrchestrator::new();

    // Create PdsClient for session refresh
    let pds_client = Arc::new(PdsClient::new());

    // Wrap sessions in RefreshableSessionProvider for automatic token refresh
    let new_session_provider =
        RefreshableSessionProvider::new(new_session.clone(), Arc::clone(&pds_client));

    // Create source and target using WASM clients
    let source = BlobSource::new(old_session);
    let target = BlobTarget::new(new_session_provider);

    // Show progress during source listing
    dispatch.call(MigrationAction::SetMigrationStep(
        "Fetching blob list from source PDS (this may take a moment for large accounts)..."
            .to_string(),
    ));

    // Pre-fetch blob counts with timeout
    let source_items = source
        .list_items()
        .await
        .map_err(|e| format!("Failed to list source blobs: {}", e))?;

    // Early exit if no blobs
    if source_items.is_empty() {
        console_info!("[Migration] No blobs to migrate, skipping blob phase");
        dispatch.call(MigrationAction::SetMigrationStep(
            "No blobs found - skipping blob migration".to_string(),
        ));
        return Ok(());
    }

    // Update with actual count
    dispatch.call(MigrationAction::SetMigrationStep(format!(
        "Found {} blobs, checking for missing blobs...",
        source_items.len()
    )));

    let missing_items = target
        .list_missing()
        .await
        .map_err(|e| format!("Failed to list missing blobs: {}", e))?;

    // Calculate the actual number of blobs that will be processed
    let initial_total_blobs = if missing_items.is_empty() {
        source_items.len()
    } else {
        missing_items.len() // Use missing items count if available
    } as u32;

    console_info!("[Migration] Pre-fetched blob counts: {} source blobs, {} missing blobs, {} will be processed", 
        source_items.len(), missing_items.len(), initial_total_blobs);

    // Initialize WASM storage backend
    let storage = BufferedStorage::new(format!("blobs/{}", old_session.did))
        .await
        .map_err(|e| format!("Failed to create blob storage: {}", e))?;

    // Update initial progress
    let mut migration_progress = state.migration_progress.clone();
    migration_progress.missing_blobs_checked = false;
    dispatch.call(MigrationAction::SetMigrationProgress(
        migration_progress.clone(),
    ));

    // Execute streaming migration with compression for blobs
    console_info!("[Migration] Executing streaming blob migration");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Streaming blobs with channel-tee pattern...".to_string(),
    ));

    // Create simple progress callback like in working commit 065e5938
    let progress_callback = {
        let dispatch_clone = *dispatch;
        let mut completed_blobs: u32 = 0;
        let mut total_bytes: u64 = 0;
        let mut processed_bytes: u64 = 0;
        let mut last_ui_update_time: Option<u64> = None;

        console_info!(
            "[DEBUG Dynamic Total] Initial total set from pre-fetch: {}",
            initial_total_blobs
        );

        move |progress_update: ProgressUpdate| {
            let current_time = js_sys::Date::now() as u64;

            // DEBUG: Log all progress events to understand what we're receiving
            crate::console_info!(
                "[PROGRESS-DEBUG] Received event: phase={:?}, event={:?}, item_id={:?}, bytes={}",
                progress_update.phase,
                progress_update.event,
                progress_update.item_id,
                progress_update.bytes_processed
            );

            // Update simple counters based on progress phase
            match (&progress_update.phase, &progress_update.event) {
                // Primary completion pattern: Completing phase with Completed event
                (ProgressPhase::Completing, ProgressEvent::Completed) => {
                    // Safety check to prevent overflow
                    if completed_blobs < initial_total_blobs {
                        completed_blobs += 1;
                        processed_bytes += progress_update.bytes_processed;
                        total_bytes = total_bytes.max(processed_bytes);
                        crate::console_info!("[BLOB-COMPLETE] Blob completed (Completing->Completed): {}/{} blobs - {}",
                            completed_blobs, initial_total_blobs,
                            progress_update.item_id.as_ref().unwrap_or(&"unknown".to_string()));
                    } else {
                        crate::console_warn!(
                            "[BLOB-COMPLETE] Attempted to increment beyond total_blobs: {}/{}",
                            completed_blobs,
                            initial_total_blobs
                        );
                    }
                }
                // Uploading phase completion: track bytes but don't increment blob counter (avoid double-counting)
                (ProgressPhase::Uploading, ProgressEvent::Completed) => {
                    // Update bytes processed but don't increment blob count (final count happens in Completing phase)
                    processed_bytes += progress_update.bytes_processed;
                    total_bytes = total_bytes.max(processed_bytes);
                    crate::console_info!(
                        "[BLOB-UPLOAD] Blob upload completed: {} bytes - {}",
                        progress_update.bytes_processed,
                        progress_update
                            .item_id
                            .as_ref()
                            .unwrap_or(&"unknown".to_string())
                    );
                }
                // Progress updates for byte tracking
                (ProgressPhase::Downloading, ProgressEvent::Progress)
                | (ProgressPhase::Uploading, ProgressEvent::Progress) => {
                    // Update bytes processed but don't increment blob count yet
                    if progress_update.bytes_processed > 0 {
                        processed_bytes = processed_bytes.max(progress_update.bytes_processed);
                        total_bytes = total_bytes.max(progress_update.total_bytes_estimate);
                    }
                }
                // Log all other events for debugging
                _ => {
                    // Skip logging for non-completion events to reduce noise
                }
            }

            // Throttle UI updates to prevent overwhelming the render loop (reduced to 50ms for responsiveness)
            let should_update_ui = match last_ui_update_time {
                Some(last_time) => current_time - last_time >= 50,
                None => true,
            };

            // Force immediate UI update when blobs complete for better responsiveness
            let force_update_on_completion = matches!(
                (&progress_update.phase, &progress_update.event),
                (ProgressPhase::Completing, ProgressEvent::Completed)
                    | (ProgressPhase::Uploading, ProgressEvent::Completed)
            );

            if should_update_ui || force_update_on_completion {
                // Create simple blob progress like commit 065e5938
                let blob_progress = BlobProgress {
                    total_blobs: initial_total_blobs,
                    processed_blobs: completed_blobs,
                    total_bytes,
                    processed_bytes,
                    current_blob_cid: progress_update.item_id.clone(),
                    current_blob_progress: if progress_update.total_bytes_estimate > 0 {
                        Some(
                            (progress_update.bytes_processed as f64
                                / progress_update.total_bytes_estimate as f64)
                                * 100.0,
                        )
                    } else {
                        None
                    },
                    error: None,
                };

                // Simple debug logging
                let progress_percentage = if blob_progress.total_blobs > 0 {
                    (blob_progress.processed_blobs as f64 / blob_progress.total_blobs as f64)
                        * 100.0
                } else {
                    0.0
                };

                crate::console_info!(
                    "[DEBUG SetBlobProgress] Dispatching: {}/{} blobs ({:.1}%)",
                    blob_progress.processed_blobs,
                    blob_progress.total_blobs,
                    progress_percentage
                );

                // Dispatch simple progress update
                dispatch_clone.call(MigrationAction::SetBlobProgress(blob_progress));

                // Only update timestamp for regular updates, not forced updates
                if !force_update_on_completion {
                    last_ui_update_time = Some(current_time);
                }

                // Enhanced migration step messages with completion indicators
                if let Some(ref cid) = progress_update.item_id {
                    let step_message = match (&progress_update.phase, &progress_update.event) {
                        (ProgressPhase::Completing, ProgressEvent::Completed)
                        | (ProgressPhase::Uploading, ProgressEvent::Completed) => {
                            if completed_blobs >= initial_total_blobs {
                                "✅ All blobs completed successfully!".to_string()
                            } else {
                                format!(
                                    "✅ Completed blob {} ({}/{} blobs)",
                                    cid.chars().take(12).collect::<String>() + "...",
                                    completed_blobs,
                                    initial_total_blobs
                                )
                            }
                        }
                        (phase, _) => {
                            let phase_text = match phase {
                                ProgressPhase::Starting => "Starting",
                                ProgressPhase::Downloading => "Downloading",
                                ProgressPhase::Uploading => "Uploading",
                                ProgressPhase::Completing => "Completing",
                            };

                            format!(
                                "{} blob {} ({}/{} blobs)",
                                phase_text,
                                cid.chars().take(12).collect::<String>() + "...",
                                completed_blobs,
                                initial_total_blobs
                            )
                        }
                    };

                    dispatch_clone.call(MigrationAction::SetMigrationStep(step_message));
                }
            }
        }
    };

    match orchestrator
        .sync_with_tee(source, target, storage, Some(progress_callback))
        .await
    {
        Ok(result) => {
            console_info!(
                "[Migration] Streaming blob migration completed successfully: {}/{} items, {} bytes processed",
                result.successful_items,
                result.total_items,
                result.total_bytes_processed
            );

            // Update final progress
            let mut migration_progress = state.migration_progress.clone();
            migration_progress.missing_blobs_checked = true;
            migration_progress.total_blob_count = result.total_items;
            migration_progress.blobs_imported = true;
            migration_progress.imported_blob_count = result.successful_items;
            dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

            // Update final blob progress with simplified structure like commit 065e5938
            let final_blob_progress = BlobProgress {
                total_blobs: result.total_items,
                processed_blobs: result.successful_items,
                total_bytes: result.total_bytes_processed,
                processed_bytes: result.total_bytes_processed,
                current_blob_cid: None,
                current_blob_progress: None,
                error: None,
            };

            // DEBUG: Log final blob progress to troubleshoot UI freeze
            console_info!(
                "[DEBUG Final SetBlobProgress] Dispatching: {}/{} blobs, {}/{} bytes",
                final_blob_progress.processed_blobs,
                final_blob_progress.total_blobs,
                final_blob_progress.processed_bytes,
                final_blob_progress.total_bytes
            );

            dispatch.call(MigrationAction::SetBlobProgress(final_blob_progress));

            dispatch.call(MigrationAction::SetMigrationStep(
                "Blob streaming migration completed successfully".to_string(),
            ));

            if !result.failed_items.is_empty() {
                console_warn!(
                    "[Migration] Some blobs failed during streaming migration: {} failures",
                    result.failed_items.len()
                );
                for failure in &result.failed_items {
                    console_warn!(
                        "[Migration] Failed blob {}: {}",
                        failure.item_id,
                        failure.error
                    );
                }
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Streaming blob migration failed: {}", e);
            console_error!("[Migration] {}", error_msg);

            // Update progress with error
            let mut migration_progress = state.migration_progress.clone();
            migration_progress.blobs_imported = false;
            dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

            Err(error_msg)
        }
    }
}
