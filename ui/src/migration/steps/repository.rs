//! Repository migration step - WASM-first implementation

use crate::services::client::{ClientSessionCredentials, PdsClient, RefreshableSessionProvider};
use crate::services::streaming::{BufferedStorage, RepoSource, RepoTarget, SyncOrchestrator};
use crate::{console_debug, console_error, console_info, console_warn};
use dioxus::prelude::*;
use std::sync::Arc;

use crate::migration::types::*;

/// Migrate repository from old PDS to new PDS using new streaming architecture
// NEWBOLD.md Steps: goat repo export $ACCOUNTDID (line 76) + goat repo import ./did:plc:do2ar6uqzrvyzq3wevji6fbe.20250625142552.car (line 81)
// Implements: Complete repository migration using streaming with channel-tee pattern
pub async fn migrate_repository_client_side(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
) -> Result<(), String> {
    console_info!("[Migration] Starting repository migration using streaming architecture");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Starting repository migration with streaming...".to_string(),
    ));

    // Create WASM streaming orchestrator
    let orchestrator = SyncOrchestrator::new();

    // Create PdsClient for session refresh
    let pds_client = Arc::new(PdsClient::new());

    // Wrap new session in RefreshableSessionProvider for automatic token refresh
    let new_session_provider =
        RefreshableSessionProvider::new(new_session.clone(), Arc::clone(&pds_client));

    // Create source, target, and storage using WASM clients
    let source = RepoSource::new(old_session);
    let target = RepoTarget::new(new_session_provider);
    let storage = BufferedStorage::new(format!("repos/{}", old_session.did))
        .await
        .map_err(|e| format!("Failed to create storage: {}", e))?;

    // Update progress - starting export
    console_info!("[Migration] Step 7: Streaming repository from old PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Streaming repository from old PDS...".to_string(),
    ));

    let repo_progress = RepoProgress {
        export_complete: false,
        import_complete: false,
        car_size: 0,
        error: None,
    };
    dispatch.call(MigrationAction::SetRepoProgress(repo_progress));

    // Create progress callback to update repo progress in real-time
    // Wrapper to convert old callback signature to new ProgressUpdate format
    let legacy_progress_callback = {
        let dispatch_clone = *dispatch;
        move |current_item_id: Option<String>, bytes_processed: u64, total_estimate: u64| {
            console_info!(
                "[Migration] Progress callback invoked: {} bytes processed, {} estimated total",
                bytes_processed,
                total_estimate
            );

            // Update repository progress during streaming
            let repo_progress = RepoProgress {
                export_complete: false, // Still in progress
                import_complete: false,
                car_size: bytes_processed,
                error: None,
            };
            console_debug!(
                "[Migration] Dispatching SetRepoProgress with {} bytes",
                bytes_processed
            );
            dispatch_clone.call(MigrationAction::SetRepoProgress(repo_progress));

            // Also update BlobProgress during repository streaming since repos contain blobs
            // Estimate blob counts based on data size (rough approximation: ~10KB average blob size)
            let estimated_blobs = if bytes_processed > 0 {
                std::cmp::max(1, (bytes_processed / 10_000) as u32) // Minimum 1 blob if any data
            } else {
                0
            };

            let _current_time = js_sys::Date::now() as u64;
            let blob_progress = BlobProgress {
                total_blobs: if total_estimate > 0 {
                    std::cmp::max(1, (total_estimate / 10_000) as u32)
                } else {
                    estimated_blobs // Use processed as estimate if no total available
                },
                processed_blobs: estimated_blobs,
                total_bytes: total_estimate.max(bytes_processed),
                processed_bytes: bytes_processed,
                current_blob_cid: current_item_id.clone(),
                current_blob_progress: if total_estimate > 0 && bytes_processed > 0 {
                    Some((bytes_processed as f64 / total_estimate as f64 * 100.0).min(100.0))
                } else {
                    None
                },
                error: None,
            };
            console_debug!(
                "[Migration] Dispatching SetBlobProgress with {} blobs ({} bytes)",
                estimated_blobs,
                bytes_processed
            );
            dispatch_clone.call(MigrationAction::SetBlobProgress(blob_progress));

            // Also update migration step with progress
            if bytes_processed > 0 {
                let step_message = if let Some(ref item_id) = current_item_id {
                    format!(
                        "Streaming repository {} with blobs... {} bytes processed",
                        item_id.chars().take(12).collect::<String>() + "...",
                        bytes_processed
                    )
                } else {
                    format!(
                        "Streaming repository with blobs... {} bytes processed",
                        bytes_processed
                    )
                };
                console_debug!("[Migration] Dispatching SetMigrationStep: {}", step_message);
                dispatch_clone.call(MigrationAction::SetMigrationStep(step_message));
            } else {
                console_warn!("[Migration] Progress callback invoked with 0 bytes processed");
            }
        }
    };

    // Create new-format progress callback that wraps the legacy one
    let progress_callback = {
        move |progress_update: crate::services::streaming::ProgressUpdate| {
            // Convert ProgressUpdate back to legacy format and call the legacy callback
            legacy_progress_callback(
                progress_update.item_id,
                progress_update.bytes_processed,
                progress_update.total_bytes_estimate,
            );
        }
    };

    // Execute WASM streaming migration with error boundary
    console_info!(
        "[Migration] Starting orchestrator.sync_with_tee with comprehensive error handling"
    );

    let migration_result = {
        console_info!("[Migration] Starting sync operation with enhanced error handling");

        // Execute the sync operation with comprehensive error handling
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Return a future that we can await
            orchestrator.sync_with_tee(source, target, storage, Some(progress_callback))
        })) {
            Ok(future) => future.await,
            Err(_) => {
                console_error!("[Migration] Panic occurred during sync operation setup");
                Err("Sync operation panicked during setup".into())
            }
        }
    };

    match migration_result {
        Ok(result) => {
            console_info!(
                "[Migration] Repository streaming completed successfully: {} bytes processed",
                result.total_bytes_processed
            );

            // Update progress - both export and import complete
            let repo_progress = RepoProgress {
                export_complete: true,
                import_complete: true,
                car_size: result.total_bytes_processed,
                error: None,
            };
            dispatch.call(MigrationAction::SetRepoProgress(repo_progress));

            dispatch.call(MigrationAction::SetMigrationStep(
                "Repository migration completed successfully".to_string(),
            ));

            if !result.failed_items.is_empty() {
                console_info!(
                    "[Migration] Warning: {} items failed during migration",
                    result.failed_items.len()
                );
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Repository streaming migration failed: {}", e);
            console_info!("[Migration] {}", error_msg);

            // Update progress with error
            let repo_progress = RepoProgress {
                export_complete: false,
                import_complete: false,
                car_size: 0,
                error: Some(error_msg.clone()),
            };
            dispatch.call(MigrationAction::SetRepoProgress(repo_progress));

            Err(error_msg)
        }
    }
}
