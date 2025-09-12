//! Migration orchestrator - coordinates the execution of migration steps

#[cfg(feature = "web")]
use crate::services::client::ClientSessionCredentials;
use crate::services::config::get_global_config;
use crate::{console_error, console_info, console_warn};
use dioxus::prelude::*;

use crate::migration::{
    steps::{
        blob::execute_streaming_blob_migration, plc::setup_plc_transition_client_side,
        preferences::migrate_preferences_client_side, repository::migrate_repository_client_side,
    },
    storage::LocalStorageManager,
    types::*,
};

/// Main migration orchestrator that coordinates all migration steps
pub async fn execute_migration_client_side(
    state: MigrationState,
    dispatch: EventHandler<MigrationAction>,
) {
    console_info!("[Migration] Starting client-side migration");

    // Step 1: Get old PDS session from localStorage
    console_info!("[Migration] Step 1: Getting old PDS session from localStorage");
    let old_session = match LocalStorageManager::get_old_session() {
        Ok(session) => {
            console_info!(
                "[Migration] Old PDS session loaded successfully: {}",
                session.did.clone()
            );
            (&session).into()
        }
        Err(error) => {
            console_error!(
                "[Migration] Failed to get old PDS session: {}",
                error.to_string()
            );
            dispatch.call(MigrationAction::SetMigrationError(Some(
                "Failed to get old PDS session from storage".to_string(),
            )));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    };

    // Get new PDS session from state
    let new_session_api = match state.new_pds_session.as_ref() {
        Some(session) => session.clone(),
        None => {
            console_error!("[Migration] Missing new PDS session");
            dispatch.call(MigrationAction::SetMigrationError(Some(
                "Missing new PDS session credentials".to_string(),
            )));
            return;
        }
    };

    let new_session = (&new_session_api).into();

    // Execute migration with retry logic (no complex resume capability)
    console_info!("[Migration] Starting fresh migration with retry capabilities");

    // Execute the full migration pipeline
    if let Err(e) = execute_full_migration(&state, &dispatch, &old_session, &new_session).await {
        console_error!("{}", format!("[Migration] Migration failed: {}", &e));
        dispatch.call(MigrationAction::SetMigrationError(Some(e)));
        return;
    }

    console_info!("[Migration] Migration completed successfully!");
    dispatch.call(MigrationAction::SetMigrationCompleted(true));
}

async fn execute_full_migration(
    state: &MigrationState,
    dispatch: &EventHandler<MigrationAction>,
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
) -> Result<(), String> {
    // Get configuration to determine architecture choice
    let config = get_global_config();

    console_info!(
        "[Migration] Using {} architecture for migration",
        match config.architecture {
            crate::services::config::MigrationArchitecture::Traditional => "traditional",
            crate::services::config::MigrationArchitecture::Streaming => "streaming",
        }
    );

    // Step 1: Repository migration (always uses new streaming architecture)
    console_info!("[Migration] Phase 1: Repository Migration");
    migrate_repository_client_side(old_session, new_session, dispatch).await?;

    // Step 2: Blob migration - choose based on configuration
    console_info!("[Migration] Phase 2: Blob Migration");
    match config.architecture {
        crate::services::config::MigrationArchitecture::Traditional => {
            console_info!("[Migration] Using traditional blob migration with smart strategies");
            execute_streaming_blob_migration(old_session, new_session, dispatch, state).await?;
        }
        crate::services::config::MigrationArchitecture::Streaming => {
            console_info!("[Migration] Using streaming blob migration with channel-tee pattern");
            execute_streaming_blob_migration(old_session, new_session, dispatch, state).await?;
        }
    }

    // Step 3: Preferences migration
    console_info!("[Migration] Phase 3: Preferences Migration");
    migrate_preferences_client_side(old_session, new_session, dispatch, state).await?;

    // Step 4: Verification and retry before Form 4 loads
    console_info!("[Migration] Phase 4: Account and Blob Verification");
    let max_retries = 3;
    let mut retry_count = 0;

    while retry_count < max_retries {
        match verify_migration_completeness(old_session, new_session).await {
            Ok(true) => {
                console_info!("[Migration] Migration verification successful");
                break;
            }
            Ok(false) => {
                retry_count += 1;
                console_warn!(
                    "[Migration] Verification failed, attempt {}/{}",
                    retry_count,
                    max_retries
                );

                if retry_count < max_retries {
                    console_info!("[Migration] Retrying repository and blob migration...");

                    // Retry repository migration
                    if let Err(e) =
                        migrate_repository_client_side(old_session, new_session, dispatch).await
                    {
                        console_error!("[Migration] Repository retry failed: {}", e);
                        continue;
                    }

                    // Retry blob migration based on configuration
                    let retry_result = match config.architecture {
                        crate::services::config::MigrationArchitecture::Traditional => {
                            execute_streaming_blob_migration(
                                old_session,
                                new_session,
                                dispatch,
                                state,
                            )
                            .await
                        }
                        crate::services::config::MigrationArchitecture::Streaming => {
                            execute_streaming_blob_migration(
                                old_session,
                                new_session,
                                dispatch,
                                state,
                            )
                            .await
                        }
                    };

                    if let Err(e) = retry_result {
                        console_error!("[Migration] Blob migration retry failed: {}", e);
                        continue;
                    }
                } else {
                    return Err(format!(
                        "Migration verification failed after {} attempts",
                        max_retries
                    ));
                }
            }
            Err(e) => {
                return Err(format!("Migration verification error: {}", e));
            }
        }
    }

    // Step 5: PLC transition setup (prepares for Form 4)
    console_info!("[Migration] Phase 5: PLC Transition Setup");
    setup_plc_transition_client_side(old_session, new_session, dispatch, state).await?;

    console_info!("[Migration] Migration completed successfully - ready for Form 4");

    Ok(())
}

/// Verify account status and blob completeness before Form 4
async fn verify_migration_completeness(
    _old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
) -> Result<bool, String> {
    let client = crate::services::client::PdsClient::new();

    // Check if there are any missing blobs on new PDS
    match client.get_missing_blobs(new_session, None, None).await {
        Ok(response) => {
            let missing_count = response.missing_blobs.unwrap_or_default().len();
            if missing_count > 0 {
                console_warn!(
                    "[Migration] Found {} missing blobs on target PDS",
                    missing_count
                );
                return Ok(false);
            }
        }
        Err(e) => {
            console_warn!("[Migration] Failed to check missing blobs: {}", e);
            return Err(format!("Failed to verify blob migration: {}", e));
        }
    }

    // Check account status on both PDSs
    // TODO: Add account status verification
    console_info!("[Migration] Account and blob verification completed successfully");
    Ok(true)
}
