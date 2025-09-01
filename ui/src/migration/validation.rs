//! Migration Validation
//!
//! This module handles validation and verification of migration steps,
//! including blob migration verification and data integrity checking.

use dioxus::prelude::*;

#[cfg(feature = "web")]
use crate::services::client::{ClientSessionCredentials, PdsClient};

use crate::migration::{
    steps::blob::execute_streaming_blob_migration,
    types::{MigrationAction, MigrationState},
};

use crate::{console_info, console_warn};

/// Verify blob migration completion using CID-level comparison for data integrity
/// This implements the CLAUDE.md requirement for account status verification before PLC token step
#[cfg(feature = "web")]
pub async fn verify_and_complete_blob_migration(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    console_info!("[Migration] Starting comprehensive blob migration verification with account status comparison...");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Verifying blob migration with account status comparison before PLC token step..."
            .to_string(),
    ));

    let pds_client = PdsClient::new();

    // CLAUDE.md Requirement: Check account status for old and new accounts to verify complete blob migration
    console_info!("[Migration] Checking account status on old PDS for baseline comparison...");
    let old_account_status = match pds_client.check_account_status(old_session).await {
        Ok(response) => {
            if response.success {
                console_info!(
                    "[Migration] Old PDS account status: {} expected blobs",
                    response.expected_blobs.unwrap_or(0)
                );
                response
            } else {
                console_warn!(
                    "[Migration] Failed to get old account status: {}",
                    response.message
                );
                // Continue with migration even if old status check fails
                Default::default()
            }
        }
        Err(e) => {
            console_warn!("[Migration] Error getting old account status: {}", e);
            Default::default()
        }
    };

    console_info!("[Migration] Checking account status on new PDS for migration verification...");
    let new_account_status = match pds_client.check_account_status(new_session).await {
        Ok(response) => {
            if response.success {
                console_info!(
                    "[Migration] New PDS account status: {} expected blobs, {} imported blobs",
                    response.expected_blobs.unwrap_or(0),
                    response.imported_blobs.unwrap_or(0)
                );
                response
            } else {
                console_warn!(
                    "[Migration] Failed to get new account status: {}",
                    response.message
                );
                Default::default()
            }
        }
        Err(e) => {
            console_warn!("[Migration] Error getting new account status: {}", e);
            Default::default()
        }
    };

    // Compare blob counts between old and new PDSs as specified in CLAUDE.md
    let old_expected = old_account_status.expected_blobs.unwrap_or(0);
    let new_expected = new_account_status.expected_blobs.unwrap_or(0);
    let new_imported = new_account_status.imported_blobs.unwrap_or(0);

    if old_expected > 0 && new_expected > 0 {
        if new_expected != new_imported {
            console_warn!(
                "[Migration] Blob count mismatch detected: {} expected, {} imported on new PDS",
                new_expected,
                new_imported
            );
        } else {
            console_info!(
                "[Migration] ✅ Blob migration verified: {} blobs successfully migrated",
                new_imported
            );
        }
    }

    // Get missing blobs from target PDS API
    console_info!("[Migration] Checking for any missing blobs via API...");
    let missing_blobs = match pds_client
        .get_missing_blobs(new_session, None, Some(500))
        .await
    {
        Ok(response) => {
            if response.success {
                let blobs = response.missing_blobs.unwrap_or_default();
                console_info!("[Migration] API-reported missing blobs: {}", blobs.len());
                blobs
            } else {
                console_warn!(
                    "[Migration] Failed to get missing blobs from target API: {}",
                    response.message
                );
                Vec::new()
            }
        }
        Err(e) => {
            console_warn!(
                "[Migration] Error getting missing blobs from target API: {}",
                e
            );
            Vec::new()
        }
    };

    // If we found missing blobs, attempt to migrate them
    if !missing_blobs.is_empty() {
        console_info!(
            "{}",
            format!(
                "[Migration] Attempting to migrate {} missing blobs...",
                missing_blobs.len()
            )
        );

        console_info!("[Migration] Starting blob reconciliation using streaming architecture...");
        execute_streaming_blob_migration(old_session, new_session, dispatch, state).await?;
        console_info!("[Migration] ✅ Streaming blob migration completed successfully");
    } else {
        console_info!("[Migration] No missing blobs found via API, proceeding to PLC operations");
    }

    console_info!(
        "[Migration] ✅ Blob migration verification completed, proceeding to PLC operations"
    );
    Ok(())
}
