//! Client-side migration logic using DNS-over-HTTPS and direct PDS operations
//! This replaces server-side functions with browser-based implementations

#[cfg(feature = "web")]
use crate::services::client::{
    ClientAccountStatusResponse, ClientCreateAccountRequest, ClientSessionCredentials, JwtUtils, MigrationClient, PdsClient
};
// use reqwest::Client;
use dioxus::prelude::*;
use gloo_console as console;

use crate::features::migration::{
    storage::LocalStorageManager,
    *,
};
// blob_opfs_storage::OpfsBlobManager, blob_storage::create_blob_manager,


/// Client-side migration execution
#[cfg(feature = "web")]
pub async fn execute_migration_client_side(state: MigrationState, dispatch: EventHandler<MigrationAction>) {
    console::info!("[Migration] Starting client-side migration process");

    let migration_client = MigrationClient::new();

    // Step 1: Get old PDS session from localStorage
    console::info!("[Migration] Step 1: Getting old PDS session from localStorage");
    let old_session_api = match LocalStorageManager::get_old_session() {
        Ok(session) => {
            console::info!("[Migration] Old PDS session loaded successfully: {}", session.did.clone());
            session
        }
        Err(error) => {
            console::error!("[Migration] Failed to get old PDS session:", format!("{:?}", error));
            dispatch.call(MigrationAction::SetMigrationError(Some("Failed to get old PDS session from storage".to_string())));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    };

    // Convert session to client session and check token expiration
    let old_session = LocalStorageManager::session_to_client(&old_session_api);
    
    // Check if token is expired or needs refresh
    if JwtUtils::is_expired(&old_session.access_jwt) {
        console::error!("[Migration] Old PDS session token is expired");
        dispatch.call(MigrationAction::SetMigrationError(Some("Session token has expired. Please log in again.".to_string())));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    } else if JwtUtils::needs_refresh(&old_session.access_jwt) {
        console::warn!("[Migration] Old PDS session token needs refresh, but continuing with migration");
    }

    // Step 2: Get target PDS DID from form2 (via describe server)
    console::info!("[Migration] Step 2: Getting target PDS DID");
    let target_pds_url = state.form2.pds_url.clone();
    if target_pds_url.is_empty() {
        console::error!("[Migration] No target PDS URL specified");
        dispatch.call(MigrationAction::SetMigrationError(Some("No target PDS URL specified".to_string())));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    // Get target PDS DID by calling the describe server endpoint
    dispatch.call(MigrationAction::SetMigrationStep("Getting target PDS information...".to_string()));
    
    let target_pds_did = match migration_client.pds_client.describe_server(&target_pds_url).await {
        Ok(response) => {
            if let Some(did) = response.get("did").and_then(|d| d.as_str()) {
                console::info!("[Migration] Target PDS DID: {}", did);
                did.to_string()
            } else {
                console::error!("[Migration] No DID found in PDS describe response");
                dispatch.call(MigrationAction::SetMigrationError(Some("Target PDS does not provide DID information".to_string())));
                dispatch.call(MigrationAction::SetMigrating(false));
                return;
            }
        }
        Err(e) => {
            console::error!("[Migration] Failed to describe target PDS: {}", format!("{}", e));
            dispatch.call(MigrationAction::SetMigrationError(Some(format!("Failed to get target PDS information: {}", e))));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    };

    // Step 3: Generate service auth token for DID ownership proof
    console::info!("[Migration] Step 3: Generating service auth token for DID ownership");
    dispatch.call(MigrationAction::SetMigrationStep("Generating service auth token...".to_string()));
    
    // Request a service auth token from the old PDS
    // This is a JWT that proves we own the DID and can migrate it
    let service_auth_token = match request_service_auth_token(&migration_client, &old_session, &target_pds_did).await {
        Ok(token) => {
            console::info!("[Migration] Service auth token generated successfully");
            token
        }
        Err(e) => {
            console::error!("[Migration] Failed to generate service auth token:", format!("{}", e));
            dispatch.call(MigrationAction::SetMigrationError(Some(format!("Failed to generate service auth token: {}", e))));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    };

    // Step 4: Try login first, then create account on new PDS (with resumption logic)
    console::info!("[Migration] Step 4: Checking if account exists on new PDS");
    dispatch.call(MigrationAction::SetMigrationStep("Checking if account already exists...".to_string()));

    // Derive PDS URL for login attempt
    let new_pds_url = migration_client.pds_client.derive_pds_url_from_handle(&state.form3.handle);

    // Try to login first to check if account already exists
    let login_result = migration_client.pds_client.try_login_before_creation(
        &state.form3.handle,
        &state.form3.password,
        &new_pds_url
    ).await;

    let new_session = match login_result {
        Ok(login_response) => {
            if login_response.success && login_response.session.is_some() {
                // Account already exists - check if we can resume migration
                console::info!("[Migration] Account already exists. Checking migration progress...");
                dispatch.call(MigrationAction::SetMigrationStep("Account already exists. Checking migration progress...".to_string()));
                
                let existing_session = login_response.session.unwrap();
                
                // Check if migration can be resumed
                match can_resume_migration(&existing_session).await {
                    Ok(true) => {
                        // Migration can be resumed - determine checkpoint
                        console::info!("[Migration] Migration can be resumed from existing account");
                        match get_migration_checkpoint(&existing_session).await {
                            Ok(checkpoint) => {
                                let checkpoint_name = match checkpoint {
                                    MigrationCheckpoint::AccountCreated => "AccountCreated",
                                    MigrationCheckpoint::RepoMigrated => "RepoMigrated",
                                    MigrationCheckpoint::BlobsMigrated => "BlobsMigrated",
                                    MigrationCheckpoint::PreferencesMigrated => "PreferencesMigrated",
                                    MigrationCheckpoint::PlcReady => "PlcReady",
                                };
                                console::info!("[Migration] Resuming from checkpoint: {}", checkpoint_name);
                                
                                // Store the existing session and resume from appropriate step
                                if let Err(error) = LocalStorageManager::store_client_session_as_new(&existing_session) {
                                    console::warn!("[Migration] Failed to store existing session:", format!("{:?}", error));
                                }
                                dispatch.call(MigrationAction::SetNewPdsSession(Some(convert_to_api_session(&existing_session))));
                                
                                // Resume migration from appropriate checkpoint
                                let resume_result = match checkpoint {
                                    MigrationCheckpoint::AccountCreated => {
                                        dispatch.call(MigrationAction::SetMigrationStep("⟳ Resuming from repository migration...".to_string()));
                                        resume_from_repo_migration(&old_session, &existing_session, &dispatch, &state).await
                                    }
                                    MigrationCheckpoint::RepoMigrated => {
                                        dispatch.call(MigrationAction::SetMigrationStep("⟳ Resuming from blob migration...".to_string()));
                                        resume_from_blob_migration(&old_session, &existing_session, &dispatch, &state).await
                                    }
                                    MigrationCheckpoint::BlobsMigrated => {
                                        dispatch.call(MigrationAction::SetMigrationStep("⟳ Resuming from preferences migration...".to_string()));
                                        resume_from_preferences_migration(&old_session, &existing_session, &dispatch, &state).await
                                    }
                                    MigrationCheckpoint::PreferencesMigrated => {
                                        dispatch.call(MigrationAction::SetMigrationStep("⟳ Resuming from PLC operations...".to_string()));
                                        resume_from_plc_operations(&old_session, &existing_session, &dispatch, &state).await
                                    }
                                    MigrationCheckpoint::PlcReady => {
                                        dispatch.call(MigrationAction::SetMigrationStep("⟳ Resuming from PLC operations...".to_string()));
                                        resume_from_plc_operations(&old_session, &existing_session, &dispatch, &state).await
                                    }
                                };
                                
                                match resume_result {
                                    Ok(_) => {
                                        console::info!("[Migration] Migration resumed successfully");
                                        dispatch.call(MigrationAction::SetMigrating(false));
                                    }
                                    Err(error) => {
                                        console::error!("[Migration] Failed to resume migration: {}", error.clone());
                                        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
                                        dispatch.call(MigrationAction::SetMigrating(false));
                                    }
                                }
                                return;
                            }
                            Err(error) => {
                                console::error!("[Migration] Failed to determine checkpoint: {}", error.clone());
                                dispatch.call(MigrationAction::SetMigrationError(Some(format!("Failed to determine resumption point: {}", error))));
                                dispatch.call(MigrationAction::SetMigrating(false));
                                return;
                            }
                        }
                    }
                    Ok(false) => {
                        console::error!("[Migration] Account exists but migration cannot be resumed (account may be activated)");
                        dispatch.call(MigrationAction::SetMigrationError(Some("Account already exists and cannot be used for migration. The account may already be activated.".to_string())));
                        dispatch.call(MigrationAction::SetMigrating(false));
                        return;
                    }
                    Err(error) => {
                        console::error!("[Migration] Failed to check resumption status: {}", error.clone());
                        dispatch.call(MigrationAction::SetMigrationError(Some(format!("Failed to check if migration can be resumed: {}", error))));
                        dispatch.call(MigrationAction::SetMigrating(false));
                        return;
                    }
                }
            } else {
                // Account doesn't exist, proceed with creation
                console::info!("[Migration] Account doesn't exist, proceeding with account creation");
                dispatch.call(MigrationAction::SetMigrationStep("Creating account on new PDS...".to_string()));
                
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

                match create_account_client_side(&migration_client, create_account_request.clone()).await {
                    Ok(session) => {
                        console::info!("[Migration] Account created successfully on new PDS");
                        session
                    }
                    Err(error) => {
                        console::error!("[Migration] Failed to create account: {}", error.clone());
                        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
                        dispatch.call(MigrationAction::SetMigrating(false));
                        return;
                    }
                }
            }
        }
        Err(error) => {
            console::error!("[Migration] Failed to check account existence: {}", format!("{}", error));
            dispatch.call(MigrationAction::SetMigrationError(Some(format!("Failed to check if account exists: {}", error))));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    };

    // Step 5: Store new PDS session in localStorage
    console::info!("[Migration] Step 5: Storing new PDS session in localStorage");
    if let Err(error) = LocalStorageManager::store_client_session_as_new(&new_session) {
        console::warn!("[Migration] Failed to store new PDS session:", format!("{:?}", error));
    }
    dispatch.call(MigrationAction::SetNewPdsSession(Some(convert_to_api_session(&new_session))));

    // Step 6: Verify account status
    console::info!("[Migration] Step 6: Verifying account status");
    dispatch.call(MigrationAction::SetMigrationStep("Verifying account status...".to_string()));
    
    match check_account_status_client_side(&new_session).await {
        Ok(status_response) => {
            if let Some(activated) = status_response.activated {
                if activated {
                    console::error!("[Migration] Account is already activated before migration");
                    dispatch.call(MigrationAction::SetMigrationError(Some("Account is already activated before migration. Try again with a different handle.".to_string())));
                    dispatch.call(MigrationAction::SetMigrating(false));
                    return;
                }
            }
            console::info!("[Migration] Account status verification successful - account is not activated");
        }
        Err(error) => {
            console::error!("[Migration] Failed to check account status: {}", error.clone());
            dispatch.call(MigrationAction::SetMigrationError(Some(error)));
            dispatch.call(MigrationAction::SetMigrating(false));
            return;
        }
    }

    // Phase 2: Content migration
    console::info!("[Migration] Starting Phase 2: Content and Identity Migration");

    // Execute repository migration
    if let Err(error) = migrate_repository_client_side(&old_session, &new_session, &dispatch, &state).await {
        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    // Execute blob migration
    if let Err(error) = migrate_blobs_client_side(&old_session, &new_session, &dispatch, &state).await {
        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    // Execute preferences migration
    if let Err(error) = migrate_preferences_client_side(&old_session, &new_session, &dispatch, &state).await {
        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    // Execute PLC setup and transition to Form 4
    if let Err(error) = setup_plc_transition_client_side(&old_session, &new_session, &dispatch, &state).await {
        dispatch.call(MigrationAction::SetMigrationError(Some(error)));
        dispatch.call(MigrationAction::SetMigrating(false));
        return;
    }

    console::info!("[Migration] Client-side migration completed successfully");
}

/// Create account using client-side operations (with fallback resumption logic)
#[cfg(feature = "web")]
async fn create_account_client_side(
    migration_client: &MigrationClient, 
    request: ClientCreateAccountRequest
) -> Result<ClientSessionCredentials, String> {
    match migration_client.create_account_new_pds(request.clone()).await {
        Ok(response) => {
            if response.success {
                response.session.ok_or_else(|| "No session returned from account creation".to_string())
            } else if response.resumable && response.error_code.as_ref().map(|c| c == "AlreadyExists").unwrap_or(false) {
                // For AlreadyExists during migration, according to AT Protocol spec,
                // the create account request with service auth token should succeed and return
                // session credentials for the existing account during migration scenarios
                console::info!("[Migration] Account creation failed with AlreadyExists - checking if response contains session for existing account");
                
                // Even if account creation "failed" due to AlreadyExists, check if we got session credentials
                if let Some(session) = response.session {
                    console::info!("[Migration] AlreadyExists response included session credentials for existing account");
                    Ok(session)
                } else {
                    // True failure - no session provided for existing account
                    Err(format!("Account creation failed with AlreadyExists but no session provided for resumption: {}", response.message))
                }
            } else {
                Err(response.message)
            }
        }
        Err(error) => Err(format!("Account creation failed: {}", error)),
    }
}

/// Check account status using client-side operations
#[cfg(feature = "web")]
async fn check_account_status_client_side(session: &ClientSessionCredentials) -> Result<ClientAccountStatusResponse, String> {
    let pds_client = PdsClient::new();
    match pds_client.check_account_status(session).await {
        Ok(response) => {
            if response.success {
                Ok(response)
            } else {
                Err(response.message)
            }
        }
        Err(error) => Err(format!("Account status check failed: {}", error)),
    }
}

/// Migrate repository using client-side operations
#[cfg(feature = "web")]
async fn migrate_repository_client_side(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    // Step 7: Export repository from old PDS
    console::info!("[Migration] Step 7: Exporting repository from old PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Exporting repository from old PDS...".to_string(),
    ));

    let pds_client = PdsClient::new();
    
    let car_data = match pds_client.export_repository(old_session).await {
        Ok(response) => {
            if response.success {
                let car_size = response.car_size.unwrap_or(0);
                console::info!(
                    "[Migration] Repository exported successfully, size: {} bytes",
                    car_size.to_string()
                );

                // Update repo progress
                let repo_progress = RepoProgress {
                    export_complete: true,
                    car_size,
                    ..Default::default()
                };
                dispatch.call(MigrationAction::SetRepoProgress(repo_progress));

                response.car_data.unwrap_or_default()
            } else {
                return Err(response.message);
            }
        }
        Err(e) => return Err(format!("Failed to export repository: {}", e)),
    };

    // Step 8: Import repository to new PDS
    console::info!("[Migration] Step 8: Importing repository to new PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Importing repository to new PDS...".to_string(),
    ));

    match pds_client.import_repository(new_session, car_data).await {
        Ok(response) => {
            if response.success {
                console::info!("[Migration] Repository imported successfully");

                // Update repo progress
                let mut repo_progress = state.repo_progress.clone();
                repo_progress.import_complete = true;
                dispatch.call(MigrationAction::SetRepoProgress(repo_progress));

                // Update migration progress
                let mut migration_progress = state.migration_progress.clone();
                migration_progress.repo_exported = true;
                migration_progress.repo_imported = true;
                dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

                Ok(())
            } else {
                Err(response.message)
            }
        }
        Err(e) => Err(format!("Failed to import repository: {}", e)),
    }
}

/// Migrate blobs using client-side operations
#[cfg(feature = "web")]
async fn migrate_blobs_client_side(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    // Step 9: Check for missing blobs on new PDS
    console::info!("[Migration] Step 9: Checking for missing blobs on new PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Checking missing blobs on new PDS...".to_string(),
    ));

    let pds_client = PdsClient::new();

    let missing_blobs = match pds_client.get_missing_blobs(new_session, None, None).await {
        Ok(response) => {
            if response.success {
                let blobs = response.missing_blobs.unwrap_or_default();
                console::info!("[Migration] Found {} missing blobs", blobs.len());

                // Update migration progress
                let mut migration_progress = state.migration_progress.clone();
                migration_progress.missing_blobs_checked = true;
                migration_progress.total_blob_count = blobs.len() as u32;
                dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

                blobs
            } else {
                return Err(response.message);
            }
        }
        Err(e) => return Err(format!("Failed to check missing blobs: {}", e)),
    };

    // Steps 10-13: Blob migration with OPFS
    if !missing_blobs.is_empty() {
        console::info!("[Migration] Steps 10-13: Starting blob migration process");
        dispatch.call(MigrationAction::SetMigrationStep(
            "Initializing OPFS blob storage...".to_string(),
        ));

        // Initialize OPFS blob manager
        let blob_manager = match crate::services::blob::blob_storage::create_blob_manager().await {
            Ok(manager) => manager,
            Err(e) => {
                return Err(format!(
                    "Failed to initialize blob storage: {}",
                    e
                ))
            }
        };

        // Download and store blobs
        console::info!(
            "[Migration] Step 10-11: Downloading {} blobs to OPFS",
            missing_blobs.len()
        );
        let mut downloaded_blobs = Vec::new();
        let mut total_blob_bytes = 0u64;

        for (index, missing_blob) in missing_blobs.iter().enumerate() {
            dispatch.call(MigrationAction::SetMigrationStep(format!(
                "Downloading blob {} of {} to OPFS...",
                index + 1,
                missing_blobs.len()
            )));

            // Update blob progress
            let blob_progress = BlobProgress {
                total_blobs: missing_blobs.len() as u32,
                processed_blobs: index as u32,
                total_bytes: total_blob_bytes,
                processed_bytes: total_blob_bytes,
                current_blob_cid: Some(missing_blob.cid.clone()),
                current_blob_progress: Some(0.0),
                error: None,
            };
            dispatch.call(MigrationAction::SetBlobProgress(blob_progress));

            // Download blob from old PDS
            match pds_client.export_blob(old_session, missing_blob.cid.clone()).await {
                Ok(response) => {
                    if response.success {
                        let blob_data = response.blob_data.unwrap_or_default();
                        let blob_size = blob_data.len() as u64;
                        total_blob_bytes += blob_size;

                        console::info!(
                            "[Migration] Downloaded blob {} ({} bytes)",
                            &missing_blob.cid,
                            blob_size.to_string()
                        );

                        // Store blob in OPFS with retry logic
                        match blob_manager
                            .store_blob_with_retry(&missing_blob.cid, blob_data.clone())
                            .await
                        {
                            Ok(()) => {
                                console::info!(
                                    "[Migration] Stored blob {} in OPFS",
                                    &missing_blob.cid
                                );
                            }
                            Err(e) => {
                                return Err(format!("Failed to store blob in OPFS: {}", e));
                            }
                        }

                        downloaded_blobs.push((missing_blob.cid.clone(), blob_data));

                        // Update blob progress
                        let blob_progress = BlobProgress {
                            total_blobs: missing_blobs.len() as u32,
                            processed_blobs: (index + 1) as u32,
                            total_bytes: total_blob_bytes,
                            processed_bytes: total_blob_bytes,
                            current_blob_cid: Some(missing_blob.cid.clone()),
                            current_blob_progress: Some(100.0),
                            error: None,
                        };
                        dispatch.call(MigrationAction::SetBlobProgress(blob_progress));
                    } else {
                        return Err(format!("Failed to download blob: {}", response.message));
                    }
                }
                Err(e) => return Err(format!("Failed to download blob: {}", e)),
            }
        }

        // Update migration progress
        let mut migration_progress = state.migration_progress.clone();
        migration_progress.blobs_exported = true;
        migration_progress.total_blob_bytes = total_blob_bytes;
        migration_progress.downloaded_blob_bytes = total_blob_bytes;
        dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

        // Step 12-13: Upload blobs from OPFS to new PDS
        console::info!(
            "[Migration] Step 12-13: Uploading {} blobs from OPFS to new PDS",
            downloaded_blobs.len()
        );

        for (index, (cid, blob_data)) in downloaded_blobs.iter().enumerate() {
            dispatch.call(MigrationAction::SetMigrationStep(format!(
                "Uploading blob {} of {} to new PDS...",
                index + 1,
                downloaded_blobs.len()
            )));

            match pds_client.upload_blob(new_session, cid.clone(), blob_data.clone()).await {
                Ok(response) => {
                    if response.success {
                        console::info!("[Migration] Uploaded blob {} to new PDS", cid);
                    } else {
                        return Err(format!("Failed to upload blob: {}", response.message));
                    }
                }
                Err(e) => return Err(format!("Failed to upload blob: {}", e)),
            }
        }

        // Update migration progress
        let mut migration_progress = state.migration_progress.clone();
        migration_progress.blobs_imported = true;
        migration_progress.imported_blob_count = downloaded_blobs.len() as u32;
        migration_progress.opfs_storage_used = 0; // Not using OPFS in client-side mode

        dispatch.call(MigrationAction::SetMigrationProgress(migration_progress.clone()));

        // Store migration progress in localStorage for resumability
        use crate::features::migration::storage::{
            BlobMigrationStatus, MigrationProgressData,
        };
        let progress_data = MigrationProgressData {
            current_step: FormStep::PlcVerification,
            completed_steps: vec![
                "login".to_string(),
                "pds_selection".to_string(),
                "migration_details".to_string(),
                "blob_migration".to_string(),
            ],
            blob_migration_status: BlobMigrationStatus::Completed,
            total_blobs: migration_progress.total_blob_count,
            processed_blobs: migration_progress.imported_blob_count,
        };

        if let Err(e) = LocalStorageManager::store_migration_progress(&progress_data) {
            console::warn!(
                "[Migration] Failed to store migration progress: {}",
                format!("{:?}", e)
            );
        } else {
            console::info!("[Migration] Migration progress stored for resumability");
        }

        // Clean up OPFS after successful upload
        if let Err(e) = blob_manager.cleanup_blobs().await {
            console::warn!(
                "[Migration] Failed to cleanup OPFS blobs: {}",
                format!("{}", e)
            );
            // Not critical - continue migration
        }
    } else {
        console::info!("[Migration] No missing blobs found - skipping blob migration");
    }

    Ok(())
}

/// Migrate preferences using client-side operations
#[cfg(feature = "web")]
async fn migrate_preferences_client_side(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    // Step 14: Export preferences from old PDS
    console::info!("[Migration] Step 14: Exporting preferences from old PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Exporting preferences from old PDS...".to_string(),
    ));

    let pds_client = PdsClient::new();

    let preferences_json = match pds_client.export_preferences(old_session).await {
        Ok(response) => {
            if response.success {
                console::info!("[Migration] Preferences exported successfully");

                // Update preferences progress
                let prefs_progress = PreferencesProgress {
                    export_complete: true,
                    ..Default::default()
                };
                dispatch.call(MigrationAction::SetPreferencesProgress(prefs_progress));

                response.preferences_json.unwrap_or_default()
            } else {
                return Err(response.message);
            }
        }
        Err(e) => return Err(format!("Failed to export preferences: {}", e)),
    };

    // Step 15: Import preferences to new PDS
    console::info!("[Migration] Step 15: Importing preferences to new PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Importing preferences to new PDS...".to_string(),
    ));

    match pds_client.import_preferences(new_session, preferences_json).await {
        Ok(response) => {
            if response.success {
                console::info!("[Migration] Preferences imported successfully");

                // Update preferences progress
                let mut prefs_progress = state.preferences_progress.clone();
                prefs_progress.import_complete = true;
                dispatch.call(MigrationAction::SetPreferencesProgress(prefs_progress));

                // Update migration progress
                let mut migration_progress = state.migration_progress.clone();
                migration_progress.preferences_exported = true;
                migration_progress.preferences_imported = true;
                dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

                Ok(())
            } else {
                Err(response.message)
            }
        }
        Err(e) => Err(format!("Failed to import preferences: {}", e)),
    }
}

/// Setup PLC operations and transition to Form 4 using client-side operations
#[cfg(feature = "web")]
async fn setup_plc_transition_client_side(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    // Step 16: Get PLC recommendation from new PDS
    console::info!("[Migration] Step 16: Getting PLC recommendation from new PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Getting PLC recommendation from new PDS...".to_string(),
    ));

    let pds_client = PdsClient::new();

    let plc_unsigned = match pds_client.get_plc_recommendation(new_session).await {
        Ok(response) => {
            if response.success {
                console::info!("[Migration] PLC recommendation retrieved successfully");

                // Update PLC progress
                let plc_progress = PlcProgress {
                    recommendation_complete: true,
                    ..Default::default()
                };
                dispatch.call(MigrationAction::SetPlcProgress(plc_progress));

                // Update migration progress
                let mut migration_progress = state.migration_progress.clone();
                migration_progress.plc_recommended = true;
                dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

                response.plc_unsigned.unwrap_or_default()
            } else {
                return Err(response.message);
            }
        }
        Err(e) => return Err(format!("Failed to get PLC recommendation: {}", e)),
    };

    // Step 17: Request PLC token from old PDS - this triggers Form 4
    console::info!("[Migration] Step 17: Requesting PLC token from old PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Requesting PLC token from old PDS...".to_string(),
    ));

    match pds_client.request_plc_token(old_session).await {
        Ok(response) => {
            if response.success {
                console::info!("[Migration] PLC token requested successfully - showing Form 4");

                // Update PLC progress
                let mut plc_progress = state.plc_progress.clone();
                plc_progress.token_requested = true;
                dispatch.call(MigrationAction::SetPlcProgress(plc_progress));

                // Update migration progress
                let mut migration_progress = state.migration_progress.clone();
                migration_progress.plc_token_requested = true;
                dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

                // Set up Form 4 data and transition to PLC verification
                dispatch.call(MigrationAction::SetPlcUnsigned(plc_unsigned.clone()));
                dispatch.call(MigrationAction::SetPlcVerificationCode(String::new()));
                let handle_context = state.form1.original_handle.clone();

                // Update form4 with context
                let mut form4 = state.form4.clone();
                form4.handle_context = handle_context;
                form4.plc_unsigned = plc_unsigned;

                // Transition to Form 4
                dispatch.call(MigrationAction::SetCurrentStep(FormStep::PlcVerification));
                dispatch.call(MigrationAction::SetMigrationStep("PLC token sent to email. Please check your email and enter the verification code in Form 4.".to_string()));
                dispatch.call(MigrationAction::SetMigrating(false)); // End migration here - Form 4 will continue

                console::info!("[Migration] Migration paused at Form 4 for PLC token verification");
                Ok(())
            } else {
                Err(response.message)
            }
        }
        Err(e) => Err(format!("Failed to request PLC token: {}", e)),
    }
}

/// Request a service auth token from the old PDS for migration
#[cfg(feature = "web")]
async fn request_service_auth_token(
    _migration_client: &MigrationClient,
    old_session: &ClientSessionCredentials,
    target_pds_did: &str,
) -> Result<String, String> {
    // The AT Protocol specifies that we need to call com.atproto.server.getServiceAuth
    // to get a proper service auth token for creating accounts with existing DIDs
    let exp_timestamp = (js_sys::Date::now() / 1000.0) as u64 + 3600; // 1 hour expiration
    let service_auth_url = format!(
        "{}/xrpc/com.atproto.server.getServiceAuth?aud={}&exp={}&lxm={}", 
        old_session.pds, 
        target_pds_did,
        exp_timestamp,
        "com.atproto.server.createAccount"
    );

    console::info!("[Migration] Requesting service auth token from: {}", service_auth_url.clone());
    
    // Create a new HTTP client for the service auth request
    use reqwest::Client;
    
    #[cfg(target_arch = "wasm32")]
    let http_client = Client::builder()
        .user_agent("atproto-migration-service/1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    #[cfg(not(target_arch = "wasm32"))]
    let http_client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("atproto-migration-service/1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = http_client
        .get(&service_auth_url)
        .header("Authorization", format!("Bearer {}", old_session.access_jwt))
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();
    if status.is_success() {
        let response_data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        
        if let Some(token) = response_data.get("token").and_then(|t| t.as_str()) {
            Ok(token.to_string())
        } else {
            Err("No token in service auth response".to_string())
        }
    } else {
        let error_text = response.text().await.unwrap_or_default();
        Err(format!("Service auth request failed with status {}: {}", status, error_text))
    }
}

/// Convert client session to API session format for compatibility
fn convert_to_api_session(client_session: &ClientSessionCredentials) -> SessionCredentials {
    SessionCredentials { 
        did: client_session.did.clone(),
        handle: client_session.handle.clone(),
        pds: client_session.pds.clone(),
        access_jwt: client_session.access_jwt.clone(),
        refresh_jwt: client_session.refresh_jwt.clone(),
    }
}

/// Convert api:: to Client
#[cfg(feature = "web")]
pub fn convert_from_api_session(api_session: SessionCredentials) -> ClientSessionCredentials {
    ClientSessionCredentials {
        did: api_session.did.clone(),
        handle: api_session.handle.clone(),
        pds: api_session.pds.clone(),
        access_jwt: api_session.access_jwt.clone(),
        refresh_jwt: api_session.refresh_jwt.clone(),
        expires_at: None, // Will be parsed from JWT if available
    }
}

/// Convert local  to Client
#[cfg(feature = "web")]
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

/// Check if migration can be resumed based on account status
#[cfg(feature = "web")]
async fn can_resume_migration(session: &ClientSessionCredentials) -> Result<bool, String> {
    let pds_client = PdsClient::new();
    match pds_client.check_account_status(session).await {
        Ok(response) => {
            if response.success {
                // Account is resumable if it exists but is not activated
                let is_resumable = response.activated == Some(false);
                Ok(is_resumable)
            } else {
                Ok(false)
            }
        }
        Err(_) => Ok(false), // If we can't check status, assume not resumable
    }
}

/// Determine migration checkpoint based on account status
#[cfg(feature = "web")]
async fn get_migration_checkpoint(session: &ClientSessionCredentials) -> Result<MigrationCheckpoint, String> {
    let pds_client = PdsClient::new();
    match pds_client.check_account_status(session).await {
        Ok(response) => {
            if !response.success {
                return Err("Failed to get account status".to_string());
            }

            // Use helper functions to determine checkpoint
            if is_blobs_migrated(session).await {
                // All blobs imported, check if preferences are done
                Ok(MigrationCheckpoint::PreferencesMigrated)
            } else if response.expected_blobs.unwrap_or(0) > 0 && response.imported_blobs.unwrap_or(0) > 0 {
                // Some blobs imported, continue blob migration
                Ok(MigrationCheckpoint::BlobsMigrated)
            } else if is_repo_migrated(session).await {
                // Repository migrated, need blob migration
                Ok(MigrationCheckpoint::RepoMigrated)
            } else {
                // Account exists but repo not migrated
                Ok(MigrationCheckpoint::AccountCreated)
            }
        }
        Err(e) => Err(format!("Failed to check account status: {}", e)),
    }
}

/// Check if repository has been migrated
#[cfg(feature = "web")]
async fn is_repo_migrated(session: &ClientSessionCredentials) -> bool {
    let pds_client = PdsClient::new();
    if let Ok(response) = pds_client.check_account_status(session).await {
        if response.success {
            // Repository is considered migrated if repo_blocks > 2
            return response.repo_blocks.unwrap_or(0) > 2;
        }
    }
    false
}

/// Check if blobs have been migrated  
#[cfg(feature = "web")]
async fn is_blobs_migrated(session: &ClientSessionCredentials) -> bool {
    let pds_client = PdsClient::new();
    if let Ok(response) = pds_client.check_account_status(session).await {
        if response.success {
            let expected_blobs = response.expected_blobs.unwrap_or(0);
            let imported_blobs = response.imported_blobs.unwrap_or(0);
            // Blobs are migrated if all expected blobs are imported
            return expected_blobs > 0 && imported_blobs >= expected_blobs;
        }
    }
    false
}

/// Resume migration from repository migration step
#[cfg(feature = "web")]
async fn resume_from_repo_migration(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    console::info!("[Migration] Resuming from repository migration - continuing full chain");
    dispatch.call(MigrationAction::SetMigrationStep("Resuming migration from repository step...".to_string()));
    
    // Execute repository migration
    migrate_repository_client_side(old_session, new_session, dispatch, state).await?;

    // Continue with blob migration
    migrate_blobs_client_side(old_session, new_session, dispatch, state).await?;

    // Continue with preferences migration
    migrate_preferences_client_side(old_session, new_session, dispatch, state).await?;

    // Continue with PLC setup (this sends the email for Form 4)
    setup_plc_transition_client_side(old_session, new_session, dispatch, state).await?;

    console::info!("[Migration] Repository migration resumption completed full chain");
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
    console::info!("[Migration] Resuming from blob migration - continuing to preferences and PLC");
    dispatch.call(MigrationAction::SetMigrationStep("Resuming migration from blob step...".to_string()));
    
    // Execute blob migration
    migrate_blobs_client_side(old_session, new_session, dispatch, state).await?;

    // Continue with preferences migration
    migrate_preferences_client_side(old_session, new_session, dispatch, state).await?;

    // Continue with PLC setup (this sends the email for Form 4)
    setup_plc_transition_client_side(old_session, new_session, dispatch, state).await?;

    console::info!("[Migration] Blob migration resumption completed full chain");
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
    console::info!("[Migration] Resuming from preferences migration - continuing to PLC");
    dispatch.call(MigrationAction::SetMigrationStep("Resuming migration from preferences step...".to_string()));
    
    // Execute preferences migration
    migrate_preferences_client_side(old_session, new_session, dispatch, state).await?;

    // Continue with PLC setup (this sends the email for Form 4)
    setup_plc_transition_client_side(old_session, new_session, dispatch, state).await?;

    console::info!("[Migration] Preferences migration resumption completed full chain");
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
    console::info!("[Migration] Resuming from PLC operations - final step");
    dispatch.call(MigrationAction::SetMigrationStep("Resuming migration from PLC step...".to_string()));
    
    // Execute PLC setup (this sends the email for Form 4)
    setup_plc_transition_client_side(old_session, new_session, dispatch, state).await?;

    console::info!("[Migration] PLC operations resumption completed");
    Ok(())
}

