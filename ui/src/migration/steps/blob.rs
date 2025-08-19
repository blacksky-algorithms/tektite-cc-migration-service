//! Blob migration step using streaming architecture

use crate::services::streaming::{SyncOrchestrator, BlobSource, BlobTarget, BufferedStorage};
#[cfg(feature = "web")]
use crate::services::client::ClientSessionCredentials;
use crate::{console_error, console_info, console_warn};
use dioxus::prelude::*;

use crate::migration::types::*;

pub async fn execute_streaming_blob_migration(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    console_info!("[Migration] Starting blob migration using streaming architecture");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Starting blob migration with streaming...".to_string(),
    ));

    // Create WASM streaming orchestrator
    let orchestrator = SyncOrchestrator::new();
    
    // Create source and target using WASM clients
    let source = BlobSource::new(old_session);
    let target = BlobTarget::new(new_session);
    
    // Initialize WASM storage backend
    let storage = BufferedStorage::new(format!("blobs/{}", old_session.did))
        .await
        .map_err(|e| format!("Failed to create blob storage: {}", e))?;
    
    // Update initial progress
    let mut migration_progress = state.migration_progress.clone();
    migration_progress.missing_blobs_checked = false;
    dispatch.call(MigrationAction::SetMigrationProgress(migration_progress.clone()));
    
    // Execute streaming migration with compression for blobs
    console_info!("[Migration] Executing streaming blob migration");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Streaming blobs with channel-tee pattern...".to_string(),
    ));
    
    match orchestrator.sync_with_tee(source, target, storage).await {
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
                        failure.item_id, failure.error
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
