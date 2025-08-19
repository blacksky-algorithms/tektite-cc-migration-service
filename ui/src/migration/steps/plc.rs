//! PLC (Personal Learning Certificate) transition setup step

use crate::console_info;
#[cfg(feature = "web")]
use crate::services::client::{ClientSessionCredentials, PdsClient};
use dioxus::prelude::*;

use crate::migration::types::*;

/// Set up PLC transition by getting recommendation and requesting token
// NEWBOLD.md Steps: goat account plc recommended > plc_recommended.json (line 127) + goat account plc request-token (line 134)
// Implements: PLC identity transition setup for DID document update
pub async fn setup_plc_transition_client_side(
    old_session: &ClientSessionCredentials,
    new_session: &ClientSessionCredentials,
    dispatch: &EventHandler<MigrationAction>,
    state: &MigrationState,
) -> Result<(), String> {
    // Step 16: Get PLC recommendation from new PDS
    // NEWBOLD.md Step: goat account plc recommended > plc_recommended.json (line 127)
    // Implements: Gets recommended DID credentials from new PDS for PLC transition
    console_info!("[Migration] Step 16: Getting PLC recommendation from new PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Getting PLC recommendation from new PDS...".to_string(),
    ));

    let pds_client = PdsClient::new();

    let plc_unsigned = match pds_client.get_plc_recommendation(new_session).await {
        Ok(response) => {
            if response.success {
                console_info!("[Migration] PLC recommendation retrieved successfully");

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
    // NEWBOLD.md Step: goat account plc request-token (line 134)
    // Implements: Requests PLC signing token via email for identity transition
    console_info!("[Migration] Step 17: Requesting PLC token from old PDS");
    dispatch.call(MigrationAction::SetMigrationStep(
        "Requesting PLC token from old PDS...".to_string(),
    ));

    match pds_client.request_plc_token(old_session).await {
        Ok(response) => {
            if response.success {
                console_info!("[Migration] PLC token requested successfully - showing Form 4");

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

                console_info!("[Migration] Migration paused at Form 4 for PLC token verification");
                Ok(())
            } else {
                Err(response.message)
            }
        }
        Err(e) => Err(format!("Failed to request PLC token: {}", e)),
    }
}
