//! Client-side migration logic using DNS-over-HTTPS and direct PDS operations
//! This replaces server-side functions with browser-based implementations

use crate::migration::steps::blob::execute_streaming_blob_migration;
#[cfg(feature = "web")]
use crate::services::client::{
    ClientCreateAccountRequest, ClientSessionCredentials, JwtUtils,
    MigrationClient,
};
// use reqwest::Client;
use dioxus::prelude::*;
// Import console macros from our crate
use crate::{console_error, console_info, console_warn};

use crate::migration::{
    account_operations::{check_account_status_client_side, create_account_client_side},
    resume_handlers::{can_resume_migration, get_migration_checkpoint},
    session_management::convert_to_api_session,
    steps::{plc::setup_plc_transition_client_side, preferences::migrate_preferences_client_side, repository::migrate_repository_client_side}, 
    storage::LocalStorageManager,
    types::{MigrationAction, MigrationState, MigrationCheckpoint},
    validation::verify_and_complete_blob_migration,
};
// blob_opfs_storage::OpfsBlobManager, blob_storage::create_blob_manager,

/// Client-side migration execution
#[cfg(feature = "web")]
pub async fn execute_migration_client_side(
    state: MigrationState,
    dispatch: EventHandler<MigrationAction>,
) {
    console_info!("[Migration] Starting client-side migration process");

    let migration_client = MigrationClient::new();

    // Step 1: Get old PDS session from localStorage
    console_info!("[Migration] Step 1: Getting old PDS session from localStorage");
    let old_session_api = match LocalStorageManager::get_old_session() {
        Ok(session) => {
            console_info!(
                "{}",
                format!(
                    "[Migration] Old PDS session loaded successfully: {}",
                    session.did.clone()
                )
            );
            session
        }
        Err(error) => {
            console_error!(
                "{}",
                format!(
                    "[Migration] Failed to get old PDS session: {}",
                    error.to_string()
                )
            );
            dispatch.call(MigrationAction::SetMigrationError(Some(
                "Failed to get old PDS session from storage".to_string(),
            )));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    };

    // Convert session to client session and check token expiration
    let old_session = LocalStorageManager::session_to_client(&old_session_api);

    // Check if token is expired or needs refresh
    if JwtUtils::is_expired(&old_session.access_jwt) {
        console_error!("[Migration] Old PDS session token is expired");
        dispatch.call(MigrationAction::SetMigrationError(Some(
            "Session token has expired. Please log in again.".to_string(),
        )));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    } else if JwtUtils::needs_refresh(&old_session.access_jwt) {
        console_warn!(
            "[Migration] Old PDS session token needs refresh, but continuing with migration"
        );
    }

    // Step 2: Get target PDS DID from form2 (via describe server)
    console_info!("[Migration] Step 2: Getting target PDS DID");
    let target_pds_url = state.form2.pds_url.clone();
    if target_pds_url.is_empty() {
        console_error!("[Migration] No target PDS URL specified");
        dispatch.call(MigrationAction::SetMigrationError(Some(
            "No target PDS URL specified".to_string(),
        )));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    // NEWBOLD.md Step: goat pds describe $NEWPDSHOST (line 11)
    // Get target PDS DID by calling the describe server endpoint
    // This implements: goat pds describe https://bsky.social
    dispatch.call(MigrationAction::SetMigrationStep(
        "Getting target PDS information...".to_string(),
    ));

    let target_pds_did = match migration_client
        .pds_client
        .describe_server(&target_pds_url)
        .await
    {
        Ok(response) => {
            if let Some(did) = response.get("did").and_then(|d| d.as_str()) {
                console_info!("{}", format!("[Migration] Target PDS DID: {}", did));
                did.to_string()
            } else {
                console_error!("[Migration] No DID found in PDS describe response");
                dispatch.call(MigrationAction::SetMigrationError(Some(
                    "Target PDS does not provide DID information".to_string(),
                )));
                dispatch.call(MigrationAction::SetMigrating(false));
                return;
            }
        }
        Err(e) => {
            console_error!(
                "{}",
                format!("[Migration] Failed to describe target PDS: {}", e)
            );
            dispatch.call(MigrationAction::SetMigrationError(Some(format!(
                "Failed to get target PDS information: {}",
                e
            ))));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    };

    // NEWBOLD.md Step: goat account service-auth --lxm com.atproto.server.createAccount --aud $NEWPDSSERVICEDID --duration-sec 3600 (line 33)
    // Step 3: Generate service auth token for DID ownership proof
    console_info!("[Migration] Step 3: Generating service auth token for DID ownership");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Generating service auth token...".to_string(),
    ));

    // Request a service auth token from the old PDS
    // This is a JWT that proves we own the DID and can migrate it
    // Implements: goat account service-auth --lxm com.atproto.server.createAccount --aud $NEWPDSSERVICEDID --duration-sec 3600
    let service_auth_token =
        match request_service_auth_token(&migration_client, &old_session, &target_pds_did).await {
            Ok(token) => {
                console_info!("[Migration] Service auth token generated successfully");
                token
            }
            Err(e) => {
                console_error!(
                    "{}",
                    format!("[Migration] Failed to generate service auth token: {}", e)
                );
                dispatch.call(MigrationAction::SetMigrationError(Some(format!(
                    "Failed to generate service auth token: {}",
                    e
                ))));
                dispatch.call(MigrationAction::SetMigrating(false));
                return;
            }
        };

    // Step 4: Try login first, then create account on new PDS (with resumption logic)
    console_info!("[Migration] Step 4: Checking if account exists on new PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Checking if account already exists...".to_string(),
    ));

    // Derive PDS URL for login attempt
    let new_pds_url = migration_client
        .pds_client
        .derive_pds_url_from_handle(&state.form3.handle);

    // NEWBOLD.md Step: goat account login --pds-host $NEWPDSHOST -u $ACCOUNTDID -p $NEWPASSWORD (line 52)
    // Try to login first to check if account already exists
    // Implements: goat account login --pds-host $NEWPDSHOST -u $ACCOUNTDID -p $NEWPASSWORD
    let login_result = migration_client
        .pds_client
        .try_login_before_creation(&state.form3.handle, &state.form3.password, &new_pds_url)
        .await;

    let new_session = match login_result {
        Ok(login_response) => {
            if login_response.success && login_response.session.is_some() {
                // Account already exists - check if we can resume migration
                console_info!("[Migration] Account already exists. Checking migration progress...");
                dispatch.call(MigrationAction::SetMigrationStep(
                    "Account already exists. Checking migration progress...".to_string(),
                ));

                let existing_session = login_response.session.unwrap();

                // Check if migration can be resumed
                match can_resume_migration(&existing_session).await {
                    Ok(true) => {
                        // Migration can be resumed - determine checkpoint
                        console_info!("[Migration] Migration can be resumed from existing account");
                        match get_migration_checkpoint(&existing_session).await {
                            Ok(checkpoint) => {
                                let checkpoint_name = match checkpoint {
                                    MigrationCheckpoint::AccountCreated => "AccountCreated",
                                    MigrationCheckpoint::RepoMigrated => "RepoMigrated",
                                    MigrationCheckpoint::BlobsMigrated => "BlobsMigrated",
                                    MigrationCheckpoint::PreferencesMigrated => {
                                        "PreferencesMigrated"
                                    }
                                    MigrationCheckpoint::PlcReady => "PlcReady",
                                };
                                console_info!(
                                    "{}",
                                    format!(
                                        "[Migration] Resuming from checkpoint: {}",
                                        checkpoint_name
                                    )
                                );

                                // Store the existing session and resume from appropriate step
                                if let Err(error) = LocalStorageManager::store_client_session_as_new(
                                    &existing_session,
                                ) {
                                    console_warn!(
                                        "{}",
                                        format!(
                                            "[Migration] Failed to store existing session: {}",
                                            error.to_string()
                                        )
                                    );
                                }
                                dispatch.call(MigrationAction::SetNewPdsSession(Some(
                                    convert_to_api_session(&existing_session),
                                )));

                                // Resume migration from appropriate checkpoint
                                let resume_result = match checkpoint {
                                    MigrationCheckpoint::AccountCreated => {
                                        dispatch.call(MigrationAction::SetMigrationStep(
                                            "⟳ Resuming from repository migration...".to_string(),
                                        ));
                                        resume_from_repo_migration(
                                            &old_session,
                                            &existing_session,
                                            &dispatch,
                                            &state,
                                        )
                                        .await
                                    }
                                    MigrationCheckpoint::RepoMigrated => {
                                        dispatch.call(MigrationAction::SetMigrationStep(
                                            "⟳ Resuming from blob migration...".to_string(),
                                        ));
                                        resume_from_blob_migration(
                                            &old_session,
                                            &existing_session,
                                            &dispatch,
                                            &state,
                                        )
                                        .await
                                    }
                                    MigrationCheckpoint::BlobsMigrated => {
                                        dispatch.call(MigrationAction::SetMigrationStep(
                                            "⟳ Resuming from preferences migration...".to_string(),
                                        ));
                                        resume_from_preferences_migration(
                                            &old_session,
                                            &existing_session,
                                            &dispatch,
                                            &state,
                                        )
                                        .await
                                    }
                                    MigrationCheckpoint::PreferencesMigrated => {
                                        dispatch.call(MigrationAction::SetMigrationStep(
                                            "⟳ Resuming from PLC operations...".to_string(),
                                        ));
                                        resume_from_plc_operations(
                                            &old_session,
                                            &existing_session,
                                            &dispatch,
                                            &state,
                                        )
                                        .await
                                    }
                                    MigrationCheckpoint::PlcReady => {
                                        dispatch.call(MigrationAction::SetMigrationStep(
                                            "⟳ Resuming from PLC operations...".to_string(),
                                        ));
                                        resume_from_plc_operations(
                                            &old_session,
                                            &existing_session,
                                            &dispatch,
                                            &state,
                                        )
                                        .await
                                    }
                                };

                                match resume_result {
                                    Ok(_) => {
                                        console_info!("[Migration] Migration resumed successfully");
                                        dispatch.call(MigrationAction::SetMigrating(false));
                                    }
                                    Err(error) => {
                                        console_error!(
                                            "{}",
                                            format!(
                                                "[Migration] Failed to resume migration: {}",
                                                error.clone()
                                            )
                                        );
                                        dispatch
                                            .call(MigrationAction::SetMigrationError(Some(error)));
                                        dispatch.call(MigrationAction::SetMigrating(false));
                                    }
                                }
                                return;
                            }
                            Err(error) => {
                                console_error!(
                                    "{}",
                                    format!(
                                        "[Migration] Failed to determine checkpoint: {}",
                                        error.clone()
                                    )
                                );
                                dispatch.call(MigrationAction::SetMigrationError(Some(format!(
                                    "Failed to determine resumption point: {}",
                                    error
                                ))));
                                dispatch.call(MigrationAction::SetMigrating(false));
                                return;
                            }
                        }
                    }
                    Ok(false) => {
                        console_error!("[Migration] Account exists but migration cannot be resumed (account may be activated)");
                        dispatch.call(MigrationAction::SetMigrationError(Some("Account already exists and cannot be used for migration. The account may already be activated.".to_string())));
                        dispatch.call(MigrationAction::SetMigrating(false));
                        return;
                    }
                    Err(error) => {
                        console_error!(
                            "{}",
                            format!(
                                "[Migration] Failed to check resumption status: {}",
                                error.clone()
                            )
                        );
                        dispatch.call(MigrationAction::SetMigrationError(Some(format!(
                            "Failed to check if migration can be resumed: {}",
                            error
                        ))));
                        dispatch.call(MigrationAction::SetMigrating(false));
                        return;
                    }
                }
            } else {
                // Account doesn't exist, proceed with creation
                console_info!(
                    "[Migration] Account doesn't exist, proceeding with account creation"
                );
                dispatch.call(MigrationAction::SetMigrationStep(
                    "Creating account on new PDS...".to_string(),
                ));

                let create_account_request = ClientCreateAccountRequest {
                    did: old_session.did.clone(),
                    handle: state.form3.handle.clone(),
                    password: state.form3.password.clone(),
                    email: state.form3.email.clone(),
                    invite_code: if state.form3.invite_code.trim().is_empty() {
                        None
                    } else {
                        Some(state.form3.invite_code.clone())
                    },
                    service_auth_token: Some(service_auth_token),
                };

                match create_account_client_side(&migration_client, create_account_request.clone())
                    .await
                {
                    Ok(session) => {
                        console_info!("[Migration] Account created successfully on new PDS");
                        session
                    }
                    Err(error) => {
                        console_error!(
                            "{}",
                            format!("[Migration] Failed to create account: {}", error.clone())
                        );
                        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
                        dispatch.call(MigrationAction::SetMigrating(false));
                        return;
                    }
                }
            }
        }
        Err(error) => {
            console_error!(
                "{}",
                format!("[Migration] Failed to check account existence: {}", error)
            );
            dispatch.call(MigrationAction::SetMigrationError(Some(format!(
                "Failed to check if account exists: {}",
                error
            ))));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    };

    // Step 5: Store new PDS session in localStorage
    console_info!("[Migration] Step 5: Storing new PDS session in localStorage");
    if let Err(error) = LocalStorageManager::store_client_session_as_new(&new_session) {
        console_warn!(
            "{}",
            format!(
                "[Migration] Failed to store new PDS session: {}",
                error.to_string()
            )
        );
    }
    dispatch.call(MigrationAction::SetNewPdsSession(Some(
        convert_to_api_session(&new_session),
    )));

    // Step 6: Verify account status
    console_info!("[Migration] Step 6: Verifying account status");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Verifying account status...".to_string(),
    ));

    match check_account_status_client_side(&new_session).await {
        Ok(status_response) => {
            if let Some(activated) = status_response.activated {
                if activated {
                    console_error!("[Migration] Account is already activated before migration");
                    dispatch.call(MigrationAction::SetMigrationError(Some("Account is already activated before migration. Try again with a different handle.".to_string())));
                    dispatch.call(MigrationAction::SetMigrating(false));
                    return;
                }
            }
            console_info!(
                "[Migration] Account status verification successful - account is not activated"
            );
        }
        Err(error) => {
            console_error!(
                "{}",
                format!(
                    "[Migration] Failed to check account status: {}",
                    error.clone()
                )
            );
            dispatch.call(MigrationAction::SetMigrationError(Some(error)));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    }

    // Phase 2: Content migration
    console_info!("[Migration] Starting Phase 2: Content and Identity Migration");

    // Execute repository migration
    if let Err(error) =
        migrate_repository_client_side(&old_session, &new_session, &dispatch).await
    {
        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    // Execute blob migration using streaming architecture
    if let Err(error) =
        execute_streaming_blob_migration(&old_session, &new_session, &dispatch, &state).await
    {
        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    // Verify blob migration completion and automatically retry missing blobs
    if let Err(error) =
        verify_and_complete_blob_migration(&old_session, &new_session, &dispatch, &state).await
    {
        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    // Execute preferences migration
    if let Err(error) =
        migrate_preferences_client_side(&old_session, &new_session, &dispatch, &state).await
    {
        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    // Execute PLC setup and transition to Form 4
    if let Err(error) =
        setup_plc_transition_client_side(&old_session, &new_session, &dispatch, &state).await
    {
        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    console_info!("[Migration] Client-side migration completed successfully");
}



/// Request a service auth token from the old PDS for migration
#[cfg(feature = "web")]
async fn request_service_auth_token(
    migration_client: &MigrationClient,
    old_session: &ClientSessionCredentials,
    target_pds_did: &str,
) -> Result<String, String> {
    // Use the new PDS client method to generate service auth token
    console_info!(
        "{}",
        format!(
            "[Migration] Requesting service auth token for target PDS: {}",
            target_pds_did
        )
    );

    let exp_timestamp = (js_sys::Date::now() / 1000.0) as u64 + 3600; // 1 hour expiration

    match migration_client
        .pds_client
        .get_service_auth(
            old_session,
            target_pds_did,
            Some("com.atproto.server.createAccount"),
            Some(exp_timestamp),
        )
        .await
    {
        Ok(response) => {
            if response.success {
                if let Some(token) = response.token {
                    console_info!("[Migration] Service auth token generated successfully");
                    Ok(token)
                } else {
                    let error_msg = "Service auth token generation succeeded but returned no token";
                    console_error!("{}", error_msg);
                    Err(error_msg.to_string())
                }
            } else {
                let error_msg =
                    format!("Service auth token generation failed: {}", response.message);
                console_error!("{}", &error_msg);
                Err(error_msg)
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to call getServiceAuth: {}", e);
            console_error!("{}", &error_msg);
            Err(error_msg)
        }
    }
}




/// Resume migration from repository migration step
#[cfg(feature = "web")]
async fn resume_from_repo_migration(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    console_info!("[Migration] Resuming from repository migration - continuing full chain");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Resuming migration from repository step...".to_string(),
    ));

    // Execute repository migration
    migrate_repository_client_side(old_session, new_session, dispatch).await?;

    // Continue with blob migration using streaming architecture
    execute_streaming_blob_migration(old_session, new_session, dispatch, state).await?;

    // Verify blob migration completion and automatically retry missing blobs
    verify_and_complete_blob_migration(old_session, new_session, dispatch, state).await?;

    // Continue with preferences migration
    migrate_preferences_client_side(old_session, new_session, dispatch, state).await?;

    // Continue with PLC setup (this sends the email for Form 4)
    setup_plc_transition_client_side(old_session, new_session, dispatch, state).await?;

    console_info!("[Migration] Repository migration resumption completed full chain");
    Ok(())
}

/// Resume migration from blob migration step  
#[cfg(feature = "web")]
async fn resume_from_blob_migration(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    console_info!("[Migration] Resuming from blob migration - continuing to preferences and PLC");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Resuming migration from blob step...".to_string(),
    ));

    // Execute blob migration using streaming architecture
    execute_streaming_blob_migration(old_session, new_session, dispatch, state).await?;

    // Verify blob migration completion and automatically retry missing blobs
    verify_and_complete_blob_migration(old_session, new_session, dispatch, state).await?;

    // Continue with preferences migration
    migrate_preferences_client_side(old_session, new_session, dispatch, state).await?;

    // Continue with PLC setup (this sends the email for Form 4)
    setup_plc_transition_client_side(old_session, new_session, dispatch, state).await?;

    console_info!("[Migration] Blob migration resumption completed full chain");
    Ok(())
}

/// Resume migration from preferences migration step
#[cfg(feature = "web")]
async fn resume_from_preferences_migration(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    console_info!("[Migration] Resuming from preferences migration - continuing to PLC");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Resuming migration from preferences step...".to_string(),
    ));

    // Verify blob migration completion and automatically retry missing blobs (in case blobs were missed)
    verify_and_complete_blob_migration(old_session, new_session, dispatch, state).await?;

    // Execute preferences migration
    migrate_preferences_client_side(old_session, new_session, dispatch, state).await?;

    // Continue with PLC setup (this sends the email for Form 4)
    setup_plc_transition_client_side(old_session, new_session, dispatch, state).await?;

    console_info!("[Migration] Preferences migration resumption completed full chain");
    Ok(())
}

/// Resume migration from PLC operations step
#[cfg(feature = "web")]
async fn resume_from_plc_operations(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    console_info!("[Migration] Resuming from PLC operations - final step");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Resuming migration from PLC step...".to_string(),
    ));

    // Verify blob migration completion and automatically retry missing blobs (final verification before PLC)
    verify_and_complete_blob_migration(old_session, new_session, dispatch, state).await?;

    // Execute PLC setup (this sends the email for Form 4)
    setup_plc_transition_client_side(old_session, new_session, dispatch, state).await?;

    console_info!("[Migration] PLC operations resumption completed");
    Ok(())
}


