//! Migration orchestrator - coordinates the execution of migration steps

#[cfg(feature = "web")]
use crate::services::client::ClientSessionCredentials;
use dioxus::prelude::*;
use gloo_console as console;

use crate::features::migration::{
    storage::LocalStorageManager,
    types::*,
    steps::{
        repository::migrate_repository_client_side,
        blob::execute_smart_blob_migration,
        preferences::migrate_preferences_client_side,
        plc::setup_plc_transition_client_side,
    },
    resume::{
        can_resume_migration,
        resume_from_repo_migration,
        resume_from_blob_migration,
        resume_from_preferences_migration,
        resume_from_plc_operations,
    },
};

/// Main migration orchestrator that coordinates all migration steps
pub async fn execute_migration_client_side(state: MigrationState, dispatch: EventHandler<MigrationAction>) {
    console::info!("[Migration] Starting client-side migration");
    
    // Step 1: Get old PDS session from localStorage
    console::info!("[Migration] Step 1: Getting old PDS session from localStorage");
    let old_session = match LocalStorageManager::get_old_session() {
        Ok(session) => {
            console::info!("[Migration] Old PDS session loaded successfully: {}", session.did.clone());
            convert_session_to_client(&session)
        }
        Err(error) => {
            console::error!("[Migration] Failed to get old PDS session: {}", error.to_string());
            dispatch.call(MigrationAction::SetMigrationError(Some("Failed to get old PDS session from storage".to_string())));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    };
    
    // Get new PDS session from state
    let new_session_api = match state.new_pds_session.as_ref() {
        Some(session) => session.clone(),
        None => {
            console::error!("[Migration] Missing new PDS session");
            dispatch.call(MigrationAction::SetMigrationError(Some("Missing new PDS session credentials".to_string())));
            return;
        }
    };
    
    let new_session = convert_session_to_client(&new_session_api);

    // Check if we can resume from a previous migration
    match can_resume_migration(&new_session).await {
        Ok(true) => {
            console::info!("[Migration] Previous migration detected - checking resumption point");
            if let Err(e) = attempt_resume_migration(&state, &dispatch, &old_session, &new_session).await {
                console::warn!("[Migration] Resume failed: {} - starting fresh migration", e);
            } else {
                console::info!("[Migration] Successfully resumed previous migration");
                return;
            }
        }
        Ok(false) => {
            console::info!("[Migration] No previous migration found - starting fresh");
        }
        Err(e) => {
            console::warn!("[Migration] Failed to check resume capability: {} - starting fresh", e);
        }
    }

    // Execute the full migration pipeline
    if let Err(e) = execute_full_migration(&state, &dispatch, &old_session, &new_session).await {
        console::error!("[Migration] Migration failed: {}", &e);
        dispatch.call(MigrationAction::SetMigrationError(Some(e)));
        return;
    }

    console::info!("[Migration] Migration completed successfully!");
    dispatch.call(MigrationAction::SetMigrationCompleted(true));
}

async fn attempt_resume_migration(
    state: &MigrationState,
    dispatch: &EventHandler<MigrationAction>,
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
) -> Result<(), String> {
    let checkpoint = get_migration_checkpoint(new_session).await?;
    
    match checkpoint {
        MigrationCheckpoint::RepositoryMigration => {
            resume_from_repo_migration(state.clone(), *dispatch, old_session.clone(), new_session.clone()).await
        }
        MigrationCheckpoint::BlobMigration => {
            resume_from_blob_migration(state.clone(), *dispatch, old_session.clone(), new_session.clone()).await
        }
        MigrationCheckpoint::PreferencesMigration => {
            resume_from_preferences_migration(state.clone(), *dispatch, old_session.clone(), new_session.clone()).await
        }
        MigrationCheckpoint::PlcOperations => {
            resume_from_plc_operations(state.clone(), *dispatch, old_session.clone(), new_session.clone()).await
        }
    }
}

async fn execute_full_migration(
    state: &MigrationState,
    dispatch: &EventHandler<MigrationAction>,
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
) -> Result<(), String> {
    // Step 1: Repository migration
    console::info!("[Migration] Phase 1: Repository Migration");
    migrate_repository_client_side(old_session, new_session, dispatch).await?;
    
    // Step 2: Blob migration using smart strategy
    console::info!("[Migration] Phase 2: Blob Migration");
    execute_smart_blob_migration(old_session, new_session, dispatch, state).await?;
    
    // Step 3: Preferences migration
    console::info!("[Migration] Phase 3: Preferences Migration");
    migrate_preferences_client_side(old_session, new_session, dispatch, state).await?;
    
    // Step 4: PLC transition
    console::info!("[Migration] Phase 4: PLC Transition Setup");
    setup_plc_transition_client_side(old_session, new_session, dispatch).await?;
    
    // Clear any stored migration progress
    // Clear any stored migration progress would go here if needed
    console::info!("[Migration] Migration completed successfully");
    
    Ok(())
}

/// Convert SessionCredentials to ClientSessionCredentials
pub fn convert_session_to_client(session: &SessionCredentials) -> ClientSessionCredentials {
    ClientSessionCredentials {
        did: session.did.clone(),
        handle: session.handle.clone(),
        pds: session.pds.clone(),
        access_jwt: session.access_jwt.clone(),
        refresh_jwt: session.refresh_jwt.clone(),
        expires_at: None, // Will be parsed from JWT if available
    }
}

#[derive(Debug, Clone)]
enum MigrationCheckpoint {
    RepositoryMigration,
    BlobMigration,
    PreferencesMigration,
    PlcOperations,
}

async fn get_migration_checkpoint(session: &ClientSessionCredentials) -> Result<MigrationCheckpoint, String> {
    // Check which step we need to resume from
    if !is_repo_migrated(session).await {
        return Ok(MigrationCheckpoint::RepositoryMigration);
    }
    
    if !is_blobs_migrated(session).await {
        return Ok(MigrationCheckpoint::BlobMigration);
    }
    
    // For now, assume preferences and PLC are quick operations that don't need checkpointing
    Ok(MigrationCheckpoint::PreferencesMigration)
}

async fn is_repo_migrated(_session: &ClientSessionCredentials) -> bool {
    // For now, return false to always start from the beginning
    // TODO: Implement proper checkpoint detection
    false
}

async fn is_blobs_migrated(session: &ClientSessionCredentials) -> bool {
    // Check if there are any missing blobs
    let client = crate::services::client::PdsClient::new();
    match client.get_missing_blobs(session, None, None).await {
        Ok(response) => {
            response.success && response.missing_blobs.unwrap_or_default().is_empty()
        }
        Err(_) => false,
    }
}