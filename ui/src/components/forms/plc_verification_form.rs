use dioxus::prelude::*;
use gloo_console as console;

use crate::components::input::{InputType, ValidatedInput};
use crate::features::migration::*;

use crate::services::client::PdsClient;
use crate::features::migration::{
    storage::LocalStorageManager,
    logic::convert_session_to_client,
};

#[derive(Props, PartialEq, Clone)]
pub struct PlcVerificationFormProps {
    pub state: Signal<MigrationState>,
    pub dispatch: EventHandler<MigrationAction>,
}

#[component]
pub fn PlcVerificationForm(props: PlcVerificationFormProps) -> Element {
    let state = props.state;
    let dispatch = props.dispatch;
    let handle = format!("{}{}",state().get_handle_prefix(),state().get_domain_suffix());

    rsx! {
        div {
            class: "migration-form form-4",

            h2 {
                class: "form-title",
                "Step 4: PLC Token Verification"
            }

            div {
                class: "display-section",
                label {
                    class: "input-label",
                    "Original Handle from Form 1:"
                }
                div {
                    class: "display-value",
                    "{state().form1.original_handle}"
                }
            }

            div {
                class: "instruction-section",
                p {
                    class: "instruction-text",
                    strong { "Check the email for " }
                    strong {
                        style: "color: #8b5cf6;",
                        "{state().form1.original_handle}"
                    }
                    br {}
                    br {}
                    "📧 Look for an email with the subject: "
                    strong { "\"PLC Update Operation Requested\"" }
                    br {}
                    "🔍 Check your spam/junk folder if you don't see it"
                    br {}
                    "🎯 The verification token will look something like: "
                    code {
                        style: "background-color: #f3f4f6; padding: 2px 6px; border-radius: 4px; font-family: monospace;",
                        "A1B2C-3D4E5"
                    }
                    br {}
                    br {}
                    "⏰ "
                    em { "Copy and paste it below to complete your migration." }
                }
            }

            div {
                class: "input-section",
                label {
                    class: "input-label",
                    "Email Verification Code:"
                }
                ValidatedInput {
                    value: state().form4.verification_code,
                    placeholder: "Enter verification code from email".to_string(),
                    input_type: InputType::Text,
                    input_class: "input-field".to_string(),
                    input_style: "".to_string(),
                    disabled: state().form4.is_verifying,
                    on_change: move |code: String| {
                        dispatch.call(MigrationAction::SetPlcVerificationCode(code));
                    }
                }
            }

            div {
                class: "button-section",
                button {
                    class: "verify-button",
                    disabled: {
                        state().form4.is_verifying ||
                        state().form4.verification_code.trim().is_empty() ||
                        state().form4.plc_unsigned.trim().is_empty()
                    },
                    onclick: move |_| {
                        let current_state = state();
                        let verification_code = current_state.form4.verification_code.clone();
                        let plc_unsigned = current_state.form4.plc_unsigned.clone();

                        dispatch.call(MigrationAction::SetPlcVerifying(true));
                        dispatch.call(MigrationAction::SetMigrationError(None));

                        spawn(async move {
                            console::info!("[Form4] Starting PLC operation signing with verification code");

                            // Create PDS client for API calls
                            let pds_client = PdsClient::new();

                            // Get old and new sessions from localStorage
                            let old_session_result = LocalStorageManager::get_old_session()
                                .map_err(|_| "Failed to get old PDS session")
                                .map(|session| convert_session_to_client(&session));

                            let new_session_result = LocalStorageManager::get_new_session()
                                .map_err(|_| "Failed to get new PDS session")
                                .map(|session| convert_session_to_client(&session));

                            let old_session = match old_session_result {
                                Ok(session) => session,
                                Err(error) => {
                                    console::error!("[Form4] Failed to get old session: {}", error);
                                    dispatch.call(MigrationAction::SetMigrationError(Some(error.to_string())));
                                    dispatch.call(MigrationAction::SetPlcVerifying(false));
                                    return;
                                }
                            };

                            let new_session = match new_session_result {
                                Ok(session) => session,
                                Err(error) => {
                                    console::error!("[Form4] Failed to get new session: {}", error);
                                    dispatch.call(MigrationAction::SetMigrationError(Some(error.to_string())));
                                    dispatch.call(MigrationAction::SetPlcVerifying(false));
                                    return;
                                }
                            };

                            // Step 17: Sign PLC operation with verification code
                            console::info!("[Form4] Step 17: Signing PLC operation");
                            dispatch.call(MigrationAction::SetMigrationStep("Signing PLC operation...".to_string()));

                            let plc_signed = match pds_client.sign_plc_operation(&old_session, plc_unsigned, verification_code).await {
                                Ok(response) => {
                                    if response.success {
                                        console::info!("[Form4] PLC operation signed successfully");
                                        response.plc_signed.unwrap_or_default()
                                    } else {
                                        let error_msg = response.message.clone();
                                        console::error!("[Form4] PLC signing failed: {}", error_msg);
                                        dispatch.call(MigrationAction::SetMigrationError(Some(response.message)));
                                        dispatch.call(MigrationAction::SetPlcVerifying(false));
                                        return;
                                    }
                                }
                                Err(e) => {
                                    console::error!("[Form4] PLC signing client operation failed: {}", format!("{}", e));
                                    dispatch.call(MigrationAction::SetMigrationError(Some(format!("Failed to sign PLC operation: {}", e))));
                                    dispatch.call(MigrationAction::SetPlcVerifying(false));
                                    return;
                                }
                            };

                            // Update PLC progress
                            let mut plc_progress = current_state.plc_progress.clone();
                            plc_progress.operation_signed = true;
                            dispatch.call(MigrationAction::SetPlcProgress(plc_progress.clone()));

                            // Step 18: Submit PLC operation to new PDS
                            console::info!("[Form4] Step 18: Submitting PLC operation");
                            dispatch.call(MigrationAction::SetMigrationStep("Submitting PLC operation...".to_string()));

                            match pds_client.submit_plc_operation(&new_session, plc_signed).await {
                                Ok(response) => {
                                    if response.success {
                                        console::info!("[Form4] PLC operation submitted successfully");
                                    } else {
                                        let error_msg = response.message.clone();
                                        console::error!("[Form4] PLC submission failed: {}", error_msg);
                                        dispatch.call(MigrationAction::SetMigrationError(Some(response.message)));
                                        dispatch.call(MigrationAction::SetPlcVerifying(false));
                                        return;
                                    }
                                }
                                Err(e) => {
                                    console::error!("[Form4] PLC submission client operation failed: {}", format!("{}", e));
                                    dispatch.call(MigrationAction::SetMigrationError(Some(format!("Failed to submit PLC operation: {}", e))));
                                    dispatch.call(MigrationAction::SetPlcVerifying(false));
                                    return;
                                }
                            };

                            // Update PLC progress
                            plc_progress.operation_submitted = true;
                            dispatch.call(MigrationAction::SetPlcProgress(plc_progress.clone()));

                            // Step 19: Activate account on new PDS
                            console::info!("[Form4] Step 19: Activating account on new PDS");
                            dispatch.call(MigrationAction::SetMigrationStep("Activating account on new PDS...".to_string()));

                            match pds_client.activate_account(&new_session).await {
                                Ok(response) => {
                                    if response.success {
                                        console::info!("[Form4] New account activated successfully");
                                    } else {
                                        let error_msg = response.message.clone();
                                        console::error!("[Form4] Account activation failed: {}", error_msg);
                                        dispatch.call(MigrationAction::SetMigrationError(Some(response.message)));
                                        dispatch.call(MigrationAction::SetPlcVerifying(false));
                                        return;
                                    }
                                }
                                Err(e) => {
                                    console::error!("[Form4] Account activation client operation failed: {}", format!("{}", e));
                                    dispatch.call(MigrationAction::SetMigrationError(Some(format!("Failed to activate new account: {}", e))));
                                    dispatch.call(MigrationAction::SetPlcVerifying(false));
                                    return;
                                }
                            };

                            // Update migration progress
                            let mut migration_progress = current_state.migration_progress.clone();
                            migration_progress.new_account_activated = true;
                            dispatch.call(MigrationAction::SetMigrationProgress(migration_progress.clone()));

                            // Step 20: Deactivate account on old PDS
                            console::info!("[Form4] Step 20: Deactivating account on old PDS");
                            dispatch.call(MigrationAction::SetMigrationStep("Deactivating account on old PDS...".to_string()));

                            // Get old session again for deactivation
                            let old_session_for_deactivation = match LocalStorageManager::get_old_session()
                                .map_err(|_| "Failed to get old PDS session")
                                .map(|session| convert_session_to_client(&session)) {
                                Ok(session) => session,
                                Err(error) => {
                                    console::warn!("[Form4] Failed to get old session for deactivation: {}", error);
                                    // This is not critical - migration is essentially complete
                                    dispatch.call(MigrationAction::SetMigrationStep("Migration completed! (Note: Could not deactivate old account - please do this manually)".to_string()));
                                    dispatch.call(MigrationAction::SetPlcVerifying(false));
                                    return;
                                }
                            };

                            match pds_client.deactivate_account(&old_session_for_deactivation).await {
                                Ok(response) => {
                                    if response.success {
                                        console::info!("[Form4] Old account deactivated successfully");

                                        // Update final migration progress
                                        migration_progress.old_account_deactivated = true;
                                        dispatch.call(MigrationAction::SetMigrationProgress(migration_progress));

                                        dispatch.call(MigrationAction::SetMigrationStep("Migration completed successfully! Your account has been migrated to the new PDS.".to_string()));
                                    } else {
                                        let error_msg = response.message.clone();
                                        console::warn!("[Form4] Old account deactivation failed: {}", error_msg);
                                        dispatch.call(MigrationAction::SetMigrationStep(format!("Migration completed! New account activated, but old account deactivation failed: {}. Please deactivate it manually.", response.message)));
                                    }
                                }
                                Err(e) => {
                                    console::warn!("[Form4] Old account deactivation client operation failed: {}", format!("{}", e));
                                    dispatch.call(MigrationAction::SetMigrationStep("Migration completed! New account activated, but could not deactivate old account. Please deactivate it manually.".to_string()));
                                }
                            };

                            console::info!("[Form4] Migration process completed!");
                            dispatch.call(MigrationAction::SetPlcVerifying(false));
                            dispatch.call(MigrationAction::SetMigrationCompleted(true));
                        });
                    },
                    if state().form4.is_verifying {
                        "Verifying..."
                    } else {
                        "Verify and Complete Migration"
                    }
                }
            }

            div {
                class: "verification-info",
                if state().migration_completed {
                    div {
                        class: "migration-complete-alert",
                        div {
                            class: "success-icon",
                            "🎉"
                        }
                        h3 {
                            class: "success-title",
                            "Migration Complete!"
                        }
                        p {
                            class: "success-message",
                            "Your account has been successfully migrated to the new PDS. You can now use your new handle and all your data has been transferred."
                        }
                        div {
                            class: "next-steps",
                            "Next steps:",
                            ul {
                                li { "Update your handle in any external applications" }
                                li { "Verify your posts and follows are intact" }
                                li { "Your old account has been deactivated" }
                                li { "If you see an invalid handle error, please make a post/skeet with your new handle @{handle}" }
                                li { "You might see an invalid handle warning for about 20 minutes. This is a feature with the PDS servers." }
                            }
                        }
                    }
                } else if state().form4.is_verifying {
                    div {
                        class: "verification-progress",
                        "{state().migration_step}"
                    }
                } else if let Some(error) = &state().migration_error {
                    div {
                        class: "verification-error",
                        "Error: {error}"
                    }
                } else {
                    div {
                        class: "verification-description",
                        "Enter the verification code from your email to complete the identity transfer and finalize the migration."
                    }
                }
            }
        }
    }
}
