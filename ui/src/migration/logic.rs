//! Client-side migration logic using DNS-over-HTTPS and direct PDS operations
//! This replaces server-side functions with browser-based implementations
use crate::migration::steps::blob::execute_streaming_blob_migration;
#[cfg(feature = "web")]
use crate::services::client::{
    ClientCreateAccountRequest, ClientSessionCredentials, JwtUtils, MigrationClient,
};
// use reqwest::Client;
use dioxus::prelude::*;
// Import console macros from our crate
use crate::{console_error, console_info, console_warn};

use crate::migration::{
    account_operations::{check_account_status_client_side, create_account_client_side},
    session_management::convert_to_api_session,
    steps::{
        plc::setup_plc_transition_client_side, preferences::migrate_preferences_client_side,
        repository::migrate_repository_client_side,
    },
    storage::LocalStorageManager,
    types::{MigrationAction, MigrationState},
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

    // Use the PDS URL from form 2 (user already provided it)
    let new_pds_url = state.form2.pds_url.clone();

    // Derive PDS URL for login attempt
    // let new_pds_url = migration_client
    //     .pds_client
    //     .derive_pds_url_from_handle(&state.form3.handle);

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
                // Account already exists - proceed with migration anyway as per CLAUDE.md
                console_info!("[Migration] Account already exists. Proceeding with migration...");
                dispatch.call(MigrationAction::SetMigrationStep(
                    "Account already exists. Proceeding with migration...".to_string(),
                ));

                let existing_session = login_response.session.unwrap();

                // Store the existing session for use in migration
                if let Err(error) =
                    LocalStorageManager::store_client_session_as_new(&existing_session)
                {
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

                existing_session
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
                        // Check if this is the specific "AlreadyExists without session" error
                        if error.contains("Account creation failed with AlreadyExists but no session provided for resumption") {
                            console_info!("[Migration] Account exists but no session provided - retrying login with provided credentials");
                            dispatch.call(MigrationAction::SetMigrationStep(
                                "Account already exists. Attempting to login with provided credentials...".to_string(),
                            ));

                            // Retry login with the credentials from Form 3
                            match migration_client
                                .pds_client
                                .try_login_before_creation(
                                    &state.form3.handle,
                                    &state.form3.password,
                                    &new_pds_url
                                )
                                .await
                            {
                                Ok(retry_response) if retry_response.success && retry_response.session.is_some() => {
                                    console_info!("[Migration] Login retry successful - proceeding with existing account");
                                    dispatch.call(MigrationAction::SetMigrationStep(
                                        "Successfully logged into existing account. Continuing migration...".to_string(),
                                    ));

                                    let session = retry_response.session.unwrap();
                                    // Store and continue with migration
                                    if let Err(e) = LocalStorageManager::store_client_session_as_new(&session) {
                                        console_warn!("Failed to store session: {}", e);
                                    }
                                    dispatch.call(MigrationAction::SetNewPdsSession(Some(
                                        convert_to_api_session(&session),
                                    )));
                                    session
                                }
                                Ok(_) => {
                                    // Login still failed
                                    console_error!("[Migration] Account exists but cannot authenticate with provided credentials");
                                    dispatch.call(MigrationAction::SetMigrationError(Some(
                                        "Account already exists on new PDS but authentication failed. Please verify your password matches the existing account.".to_string()
                                    )));
                                    dispatch.call(MigrationAction::SetMigrating(false));
                                    return;
                                }
                                Err(e) => {
                                    console_error!("[Migration] Failed to retry login: {}", e);
                                    dispatch.call(MigrationAction::SetMigrationError(Some(
                                        format!("Account exists but login failed: {}. Please verify your credentials match the existing account.", e)
                                    )));
                                    dispatch.call(MigrationAction::SetMigrating(false));
                                    return;
                                }
                            }
                        } else {
                            // Other errors - fail as before
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
    if let Err(error) = migrate_repository_client_side(&old_session, &new_session, &dispatch).await
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

    console_info!(
        "[MILESTONE] Client-side migration data phase completed successfully - timestamp: {}",
        js_sys::Date::now()
    );
    console_info!("[Migration] ⚠️  Migration continues with PLC operations in Form4 - NOT setting is_migrating=false yet");
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
