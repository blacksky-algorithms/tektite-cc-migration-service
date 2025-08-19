//! Repository migration step - WASM-first implementation

use crate::console_info;
use crate::services::client::ClientSessionCredentials;
use crate::services::streaming::{SyncOrchestrator, RepoSource, RepoTarget, BufferedStorage};
use dioxus::prelude::*;

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
    
    // Create source, target, and storage using WASM clients
    let source = RepoSource::new(old_session);
    let target = RepoTarget::new(new_session);
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

    // Execute WASM streaming migration
    match orchestrator.sync_with_tee(source, target, storage).await {
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
