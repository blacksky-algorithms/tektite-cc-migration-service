//! Migration resume handlers for continuing interrupted migrations

#[cfg(feature = "web")]
use crate::services::client::ClientSessionCredentials;
use dioxus::prelude::*;
use crate::{console_info, console_warn};

use crate::features::migration::{
    steps::{
        blob::execute_smart_blob_migration, plc::setup_plc_transition_client_side,
        preferences::migrate_preferences_client_side, repository::migrate_repository_client_side,
    },
    types::*,
};

/// Check if a migration can be resumed from a previous session
pub async fn can_resume_migration(session: &ClientSessionCredentials) -> Result<bool, String> {
    use crate::services::client::SessionManager;

    // Check if there's stored migration progress data
    let session_manager = SessionManager::new_persistent("migration_storage");

    // Try to load migration progress from browser storage
    match session_manager.get_migration_progress(&session.did) {
        Ok(Some(progress)) => {
            // Check if migration is resumable and has a valid checkpoint
            let is_resumable = progress.migration_resumable
                && progress.last_checkpoint.is_some()
                && !progress.new_account_activated; // Don't resume if already completed

            console_info!(
                "Migration resume check for {}: resumable={}, checkpoint={:?}",
                session.did.clone(),
                is_resumable,
                progress.last_checkpoint
            );

            Ok(is_resumable)
        }
        Ok(None) => {
            // No stored progress found
            Ok(false)
        }
        Err(e) => {
            console_warn!("{}", format!("Failed to check migration progress: {}", e));
            Ok(false) // Fail safely - don't resume if we can't check
        }
    }
}

/// Determine which migration step to resume from based on progress
pub fn get_resume_checkpoint(
    progress: &crate::features::migration::types::MigrationProgress,
) -> Option<crate::features::migration::types::MigrationCheckpoint> {
    use crate::features::migration::types::MigrationCheckpoint;

    // Check progress in reverse order to find the last incomplete step
    if !progress.old_account_deactivated && progress.plc_submitted {
        Some(MigrationCheckpoint::PlcReady)
    } else if !progress.plc_submitted && progress.preferences_imported {
        Some(MigrationCheckpoint::PreferencesMigrated)
    } else if !progress.preferences_imported && progress.blobs_imported {
        Some(MigrationCheckpoint::BlobsMigrated)
    } else if !progress.blobs_imported && progress.repo_imported {
        Some(MigrationCheckpoint::RepoMigrated)
    } else if !progress.repo_imported && progress.new_account_activated {
        Some(MigrationCheckpoint::AccountCreated)
    } else {
        None // No valid checkpoint found
    }
}

/// Resume migration from the appropriate checkpoint
pub async fn resume_migration_from_checkpoint(
    state: MigrationState,
    dispatch: EventHandler<MigrationAction>,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
) -> Result<(), String> {
    use crate::features::migration::types::MigrationCheckpoint;
    use crate::services::client::SessionManager;

    let session_manager = SessionManager::new_persistent("migration_storage");

    // Load migration progress
    let progress = session_manager
        .get_migration_progress(&old_session.did)
        .map_err(|e| format!("Failed to load migration progress: {}", e))?
        .ok_or_else(|| "No migration progress found".to_string())?;

    // Determine checkpoint
    let checkpoint = get_resume_checkpoint(&progress)
        .ok_or_else(|| "No valid checkpoint found for resumption".to_string())?;

    let checkpoint_name = match checkpoint {
        MigrationCheckpoint::AccountCreated => "AccountCreated",
        MigrationCheckpoint::RepoMigrated => "RepoMigrated",
        MigrationCheckpoint::BlobsMigrated => "BlobsMigrated",
        MigrationCheckpoint::PreferencesMigrated => "PreferencesMigrated",
        MigrationCheckpoint::PlcReady => "PlcReady",
    };
    console_info!("{}", format!("Resuming migration from checkpoint: {}", checkpoint_name));

    // Resume from appropriate checkpoint
    match checkpoint {
        MigrationCheckpoint::AccountCreated => {
            resume_from_repo_migration(state, dispatch, old_session, new_session).await
        }
        MigrationCheckpoint::RepoMigrated => {
            resume_from_blob_migration(state, dispatch, old_session, new_session).await
        }
        MigrationCheckpoint::BlobsMigrated => {
            resume_from_preferences_migration(state, dispatch, old_session, new_session).await
        }
        MigrationCheckpoint::PreferencesMigrated => {
            resume_from_plc_operations(state, dispatch, old_session, new_session).await
        }
        MigrationCheckpoint::PlcReady => {
            console_info!(
                "Migration is at PLC ready checkpoint - manual form submission required"
            );
            Ok(()) // User needs to complete form submission manually
        }
    }
}

/// Resume migration from repository step
pub async fn resume_from_repo_migration(
    state: MigrationState,
    dispatch: EventHandler<MigrationAction>,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
) -> Result<(), String> {
    console_info!("[Resume] Resuming from repository migration step");

    // Complete repository migration
    migrate_repository_client_side(&old_session, &new_session, &dispatch).await?;

    // Continue with blob migration
    execute_smart_blob_migration(&old_session, &new_session, &dispatch, &state).await?;

    // Continue with preferences
    migrate_preferences_client_side(&old_session, &new_session, &dispatch, &state).await?;

    // Complete with PLC setup
    setup_plc_transition_client_side(&old_session, &new_session, &dispatch, &state).await?;

    Ok(())
}

/// Resume migration from blob step
pub async fn resume_from_blob_migration(
    state: MigrationState,
    dispatch: EventHandler<MigrationAction>,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
) -> Result<(), String> {
    console_info!("[Resume] Resuming from blob migration step");

    // Complete blob migration
    execute_smart_blob_migration(&old_session, &new_session, &dispatch, &state).await?;

    // Continue with preferences
    migrate_preferences_client_side(&old_session, &new_session, &dispatch, &state).await?;

    // Complete with PLC setup
    setup_plc_transition_client_side(&old_session, &new_session, &dispatch, &state).await?;

    Ok(())
}

/// Resume migration from preferences step
pub async fn resume_from_preferences_migration(
    state: MigrationState,
    dispatch: EventHandler<MigrationAction>,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
) -> Result<(), String> {
    console_info!("[Resume] Resuming from preferences migration step");

    // Complete preferences migration
    migrate_preferences_client_side(&old_session, &new_session, &dispatch, &state).await?;

    // Complete with PLC setup
    setup_plc_transition_client_side(&old_session, &new_session, &dispatch, &state).await?;

    Ok(())
}

/// Resume migration from PLC operations
pub async fn resume_from_plc_operations(
    state: MigrationState,
    dispatch: EventHandler<MigrationAction>,
    old_session: ClientSessionCredentials,
    new_session: ClientSessionCredentials,
) -> Result<(), String> {
    console_info!("[Resume] Resuming from PLC operations step");

    // Complete PLC setup
    setup_plc_transition_client_side(&old_session, &new_session, &dispatch, &state).await?;

    Ok(())
}
