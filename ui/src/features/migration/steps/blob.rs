//! Blob migration step using smart migration strategies

#[cfg(feature = "web")]
use crate::services::client::{ClientSessionCredentials, ClientMissingBlob, PdsClient};
use crate::services::blob::blob_migration::smart_blob_migration;
use crate::services::config::{get_global_config, BlobEnumerationMethod};
use dioxus::prelude::*;
use gloo_console as console;

use crate::features::migration::types::*;

/// Enumerate blobs using the configured method (listMissingBlobs or syncListBlobs)
async fn enumerate_blobs(
    pds_client: &PdsClient,
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
) -> Result<Vec<ClientMissingBlob>, String> {
    let config = get_global_config();
    
    match config.blob.enumeration_method {
        BlobEnumerationMethod::MissingBlobs => {
            console::info!("[Migration] Using listMissingBlobs (migration-optimized method)");
            enumerate_missing_blobs(pds_client, new_session).await
        }
        BlobEnumerationMethod::SyncListBlobs => {
            console::info!("[Migration] Using syncListBlobs (Go goat compatible method)");
            enumerate_sync_list_blobs(pds_client, old_session).await
        }
    }
}

/// Enumerate blobs using com.atproto.repo.listMissingBlobs (migration-optimized)
// NEWBOLD.md Step: goat account missing-blobs (line 86)
// Implements: Lists specific blobs missing on new PDS that need migration
async fn enumerate_missing_blobs(
    pds_client: &PdsClient,
    new_session: &ClientSessionCredentials,
) -> Result<Vec<ClientMissingBlob>, String> {
    let mut missing_blobs = Vec::new();
    let mut cursor: Option<String> = None;
    
    loop {
        console::debug!("[Migration] Fetching missing blobs batch with cursor: {:?}", cursor.as_ref());
        let current_cursor = cursor.clone();
        // NEWBOLD.md: goat account missing-blobs - paginated enumeration with cursor
        match pds_client.get_missing_blobs(new_session, current_cursor, Some(500)).await {
            Ok(response) => {
                if response.success {
                    let mut batch_blobs = response.missing_blobs.unwrap_or_default();
                    console::debug!("[Migration] Received {} blobs in this batch", batch_blobs.len());
                    missing_blobs.append(&mut batch_blobs);

                    // Check for pagination continuation - matches Go goat pattern:
                    // if resp.Cursor != nil && *resp.Cursor != ""
                    cursor = if let Some(next_cursor) = response.cursor {
                        if !next_cursor.is_empty() {
                            Some(next_cursor) // Continue with next cursor
                        } else {
                            break; // Empty cursor means no more pages
                        }
                    } else {
                        break; // No cursor means no more pages
                    };
                } else {
                    return Err(response.message);
                }
            }
            Err(e) => return Err(format!("Failed to check missing blobs: {}", e)),
        }
    }
    
    Ok(missing_blobs)
}

/// Enumerate blobs using com.atproto.sync.listBlobs (Go goat compatible)
// NEWBOLD.md Alternative: Compatible with goat blob export enumeration pattern
// Implements: Full blob enumeration like Go goat for complete repository listing
async fn enumerate_sync_list_blobs(
    pds_client: &PdsClient,
    old_session: &ClientSessionCredentials,
) -> Result<Vec<ClientMissingBlob>, String> {
    let mut all_blobs = Vec::new();
    let mut cursor: Option<String> = None;
    
    loop {
        console::debug!("[Migration] Fetching blobs batch with cursor: {:?}", cursor.as_ref());
        let current_cursor = cursor.clone();
        // NEWBOLD.md: Compatible with goat blob export $ACCOUNTDID enumeration pattern
        match pds_client.sync_list_blobs(old_session, &old_session.did, current_cursor, Some(500), None).await {
            Ok(response) => {
                if response.success {
                    let batch_cids = response.cids.unwrap_or_default();
                    console::debug!("[Migration] Received {} CIDs in this batch", batch_cids.len());
                    
                    // Convert CIDs to ClientMissingBlob format for compatibility
                    for cid in batch_cids {
                        all_blobs.push(ClientMissingBlob {
                            cid: cid.clone(),
                            record_uri: format!("at://{}", old_session.did), // Generic URI for sync list blobs
                        });
                    }

                    // Check for pagination continuation - matches Go goat pattern:
                    // if resp.Cursor != nil && *resp.Cursor != ""
                    cursor = if let Some(next_cursor) = response.cursor {
                        if !next_cursor.is_empty() {
                            Some(next_cursor) // Continue with next cursor
                        } else {
                            break; // Empty cursor means no more pages
                        }
                    } else {
                        break; // No cursor means no more pages
                    };
                } else {
                    return Err(response.message);
                }
            }
            Err(e) => return Err(format!("Failed to list blobs: {}", e)),
        }
    }
    
    Ok(all_blobs)
}

/// Execute smart blob migration with intelligent strategy selection
pub async fn execute_smart_blob_migration(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    // Step 9: Enumerate blobs using configured method
    console::info!("[Migration] Step 9: Enumerating blobs for migration");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Enumerating blobs for migration...".to_string(),
    ));

    let pds_client = PdsClient::new();

    // Enumerate blobs using the configured method (listMissingBlobs or syncListBlobs)
    let missing_blobs = match enumerate_blobs(&pds_client, old_session, new_session).await {
        Ok(blobs) => blobs,
        Err(e) => return Err(e),
    };

    console::info!("[Migration] Found {} missing blobs across all pages", missing_blobs.len().to_string());

    // Update migration progress
    let mut migration_progress = state.migration_progress.clone();
    migration_progress.missing_blobs_checked = true;
    migration_progress.total_blob_count = missing_blobs.len() as u32;
    dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

    // Steps 10-13: Smart blob migration with intelligent fallback storage
    // NEWBOLD.md Steps: goat blob export $ACCOUNTDID (line 98) + goat blob upload {} (line 104)
    // Implements: Export blobs from old PDS and upload to new PDS with intelligent strategies
    if !missing_blobs.is_empty() {
        console::info!("[Migration] Steps 10-13: Starting smart blob migration with intelligent storage fallback");
        dispatch.call(MigrationAction::SetMigrationStep(
            "Initializing intelligent blob storage for smart migration...".to_string(),
        ));

        // Initialize intelligent fallback blob manager
        let mut blob_manager = match crate::services::blob::blob_fallback_manager::create_fallback_blob_manager().await {
            Ok(manager) => {
                let (backend_name, backend_description) = manager.get_active_backend_info();
                console::info!("[Migration] Blob storage initialized with {} backend", backend_name);
                console::info!("[Migration] Backend details: {}", backend_description);
                manager
            },
            Err(e) => {
                return Err(format!(
                    "Failed to initialize blob storage (all backends failed): {}",
                    e
                ))
            }
        };

        // Use smart blob migration with intelligent strategy selection
        // NEWBOLD.md: Equivalent to: fd . ./account_blobs/ | parallel -j1 goat blob upload {} (line 104)
        // Implements: Parallel blob upload with retry logic and intelligent storage fallback
        console::info!("[Migration] Executing smart blob migration with {} blobs", missing_blobs.len().to_string());
        match smart_blob_migration(
            missing_blobs,
            old_session.clone(),
            new_session.clone(),
            &mut blob_manager,
            dispatch,
        ).await {
            Ok(result) => {
                console::info!("[Migration] Smart blob migration completed successfully");
                console::info!("[Migration] Results: {}/{} blobs uploaded, {} failed", 
                              result.uploaded_blobs, result.total_blobs, result.failed_blobs.len());
                
                // Update final migration progress
                let mut migration_progress = state.migration_progress.clone();
                migration_progress.blobs_imported = true;
                migration_progress.imported_blob_count = result.uploaded_blobs;
                dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

                if !result.failed_blobs.is_empty() {
                    console::warn!("[Migration] Some blobs failed to migrate: {} failures", result.failed_blobs.len().to_string());
                    for failure in &result.failed_blobs {
                        console::warn!("[Migration] Failed blob {}: {} ({})", &failure.cid, &failure.operation, &failure.error);
                    }
                }
                
                Ok(())
            }
            Err(error) => {
                console::error!("[Migration] Smart blob migration failed: {}", &error);
                Err(format!("Smart blob migration failed: {}", error))
            }
        }
    } else {
        console::info!("[Migration] No missing blobs found - skipping blob migration");
        
        // Update migration progress for empty case
        let mut migration_progress = state.migration_progress.clone();
        migration_progress.missing_blobs_checked = true;
        migration_progress.total_blob_count = 0;
        migration_progress.blobs_imported = true;
        dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));
        
        Ok(())
    }
}