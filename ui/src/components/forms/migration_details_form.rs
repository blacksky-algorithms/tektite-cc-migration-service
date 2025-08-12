use dioxus::prelude::*;

use crate::components::{
    display::BlobProgressDisplay,
    input::{EmailValidationFeedback, HandleValidationFeedback, InputType,
        PasswordValidationFeedback, ValidatedInput},
};
use crate::features::migration::{
    form_validation::{get_form3_validation_message, validate_form3_complete},
    *,
};
use crate::utils::validation::{
    email_validation_class, email_validation_style, password_validation_class,
    password_validation_style, validation_class, validation_style,
};

// Import client-side components  
#[cfg(feature = "web")]
use crate::services::client::WebIdentityResolver;

// Import the appropriate migration logic based on feature flags
#[cfg(feature = "web")]
use crate::features::migration::logic::execute_migration_client_side;

#[cfg(not(feature = "web"))]
use crate::features::migration::logic::execute_migration;

#[derive(Props, PartialEq, Clone)]
pub struct MigrationDetailsFormProps {
    pub state: Signal<MigrationState>,
    pub dispatch: EventHandler<MigrationAction>,
}

#[component]
pub fn MigrationDetailsForm(props: MigrationDetailsFormProps) -> Element {
    let state = props.state;
    let dispatch = props.dispatch;

    rsx! {
        div {
            class: "migration-form form-3",

            h2 {
                class: "form-title",
                "Step 3: Migration Details"
            }

            div {
                class: "display-section",
                label {
                    class: "input-label",
                    "New PDS Host:"
                }
                div {
                    class: "display-value",
                    "{state().form2.pds_url}"
                }
            }

            div {
                class: "input-section",
                label {
                    class: "input-label",
                    "New PDS Handle:"
                }
                div {
                    class: "handle-input-container",
                    ValidatedInput {
                        value: state().get_handle_prefix(),
                        placeholder: state().get_handle_prefix_placeholder(),
                        input_type: InputType::Text,
                        input_class: format!("{} handle-prefix-input", validation_class(&state().validations.handle)),
                        input_style: validation_style(&state().validations.handle).to_string(),
                        disabled: state().is_migrating || state().current_step == FormStep::PlcVerification,
                        on_change: move |prefix_value: String| {
                            // Combine prefix with domain suffix to create full handle
                            let domain_suffix = state().get_domain_suffix();
                            let full_handle = format!("{}{}", prefix_value, domain_suffix);
                            
                            dispatch.call(MigrationAction::SetNewHandle(full_handle.clone()));

                            // Validate handle availability if prefix is not empty
                            if !prefix_value.trim().is_empty() {
                                dispatch.call(MigrationAction::SetHandleValidation(HandleValidation::Checking));
                                dispatch.call(MigrationAction::SetCheckingHandle(true));

                                #[cfg(feature = "web")]
                                spawn(async move {
                                    let identity_resolver = WebIdentityResolver::new();
                                    match identity_resolver.resolve_handle(&full_handle).await {
                                        Ok(_did) => {
                                            // Handle resolves to a DID - it's unavailable (taken)
                                            dispatch.call(MigrationAction::SetHandleValidation(HandleValidation::Unavailable));
                                        }
                                        Err(_) => {
                                            // Handle doesn't resolve - it's available (not taken)
                                            dispatch.call(MigrationAction::SetHandleValidation(HandleValidation::Available));
                                        }
                                    }
                                    dispatch.call(MigrationAction::SetCheckingHandle(false));
                                });

                                #[cfg(not(feature = "web"))]
                                spawn(async move {
                                    // Fallback for when client-side migration is disabled
                                    dispatch.call(MigrationAction::SetHandleValidation(HandleValidation::Error));
                                    dispatch.call(MigrationAction::SetCheckingHandle(false));
                                });
                            } else {
                                dispatch.call(MigrationAction::SetHandleValidation(HandleValidation::None));
                                dispatch.call(MigrationAction::SetCheckingHandle(false));
                            }
                        }
                    }
                    span {
                        class: "handle-domain-suffix",
                        "{state().get_domain_suffix()}"
                    }
                }

                // Handle validation feedback
                HandleValidationFeedback {
                    validation: state().validations.handle,
                    is_checking: state().form3.is_checking_handle
                }
            }

            div {
                class: "input-section",
                label {
                    class: "input-label",
                    "New Password:"
                }
                ValidatedInput {
                    value: state().form3.password,
                    placeholder: "Enter new password".to_string(),
                    input_type: InputType::Password,
                    input_class: password_validation_class(&state().validate_passwords()).to_string(),
                    input_style: password_validation_style(&state().validate_passwords()).to_string(),
                    disabled: state().is_migrating || state().current_step == FormStep::PlcVerification,
                    on_change: move |password_value: String| {
                        dispatch.call(MigrationAction::SetNewPassword(password_value));
                    }
                }
            }

            div {
                class: "input-section",
                label {
                    class: "input-label",
                    "Confirm New Password:"
                }
                ValidatedInput {
                    value: state().form3.password_confirm,
                    placeholder: "Confirm new password".to_string(),
                    input_type: InputType::Password,
                    input_class: password_validation_class(&state().validate_passwords()).to_string(),
                    input_style: password_validation_style(&state().validate_passwords()).to_string(),
                    disabled: state().is_migrating || state().current_step == FormStep::PlcVerification,
                    on_change: move |confirm_value: String| {
                        dispatch.call(MigrationAction::SetNewPasswordConfirm(confirm_value));
                    }
                }

                // Password validation feedback
                PasswordValidationFeedback {
                    validation: state().validate_passwords()
                }
            }

            div {
                class: "input-section",
                label {
                    class: "input-label",
                    "Email Address:"
                }
                ValidatedInput {
                    value: state().form3.email,
                    placeholder: "your.email@example.com".to_string(),
                    input_type: InputType::Email,
                    input_class: email_validation_class(&state().validate_email()).to_string(),
                    input_style: email_validation_style(&state().validate_email()).to_string(),
                    disabled: state().is_migrating || state().current_step == FormStep::PlcVerification,
                    on_change: move |email_value: String| {
                        dispatch.call(MigrationAction::SetEmailAddress(email_value));
                    }
                }

                // Email validation feedback
                EmailValidationFeedback {
                    validation: state().validate_email()
                }
            }

            div {
                class: "input-section",
                label {
                    class: "input-label",
                    "Invite Code:"
                }
                ValidatedInput {
                    value: state().form3.invite_code,
                    placeholder: "Enter invite code (if required)".to_string(),
                    input_type: InputType::Text,
                    input_class: "input-field".to_string(),
                    input_style: "".to_string(),
                    disabled: state().is_migrating || state().current_step == FormStep::PlcVerification,
                    on_change: move |code: String| {
                        dispatch.call(MigrationAction::SetInviteCode(code));
                    }
                }
            }

            div {
                class: "button-section",
                button {
                    class: "migrate-button",
                    disabled: {
                        let current_state = state();
                        current_state.is_migrating || !validate_form3_complete(&current_state)
                    },
                    onclick: move |_| {
                        let current_state = state();
                        dispatch.call(MigrationAction::SetMigrating(true));
                        dispatch.call(MigrationAction::SetMigrationError(None));
                        dispatch.call(MigrationAction::SetMigrationStep("Starting migration...".to_string()));

                        // Use the appropriate migration execution based on feature flags
                        #[cfg(feature = "web")]
                        spawn(execute_migration_client_side(current_state, dispatch));

                        #[cfg(not(feature = "web"))]
                        spawn(execute_migration(current_state, dispatch));
                    },
                    if state().is_migrating {
                        "Migrating..."
                    } else {
                        "Migrate"
                    }
                }
            }

            div {
                class: "migration-info",
                if state().is_migrating {
                    div {
                        class: "migration-progress",
                        "{state().migration_step}"

                        // Show detailed blob progress if blob migration is in progress
                        if state().blob_progress.total_blobs > 0 && state().migration_step.contains("blob") {
                            BlobProgressDisplay {
                                blob_progress: state().blob_progress.clone(),
                                migration_step: state().migration_step.clone()
                            }
                        }
                    }
                } else if let Some(error) = &state().migration_error {
                    div {
                        class: "migration-error",
                        "Error: {error}"
                    }
                } else if let Some(validation_msg) = get_form3_validation_message(&state()) {
                    div {
                        class: "validation-error",
                        "⚠️ {validation_msg}"
                    }
                } else if state().new_pds_session.is_some() {
                    div {
                        class: "migration-success",
                        "Migration setup completed successfully! New PDS session stored."

                        // Show post-migration instructions for FQDN handles
                        if state().is_original_handle_fqdn() {
                            div {
                                class: "post-migration-instructions",
                                h4 {
                                    class: "instructions-title",
                                    "Post-Migration DNS Setup Instructions"
                                }
                                p {
                                    "Since you used a custom domain handle ({state().form1.original_handle}), please complete these final steps:"
                                }
                                ol {
                                    class: "instructions-list",
                                    li {
                                        "Login with your new handle: {state().form3.handle} at "
                                        a {
                                            href: "https://blacksky.community/",
                                            target: "_blank",
                                            "https://blacksky.community/"
                                        }
                                    }
                                    li {
                                        "Go to Settings > Account > Handle > \"I Have My Own Domain\""
                                    }
                                    li {
                                        "Enter your original domain: {state().form1.original_handle}"
                                    }
                                    li {
                                        "Click \"Verify DNS Record\" to complete the setup"
                                    }
                                }
                                p {
                                    class: "instructions-note",
                                    "After DNS verification, you'll be able to use your original domain handle again."
                                }
                            }
                        }
                    }
                } else {
                    div {
                        class: "migration-description",
                        "This will migrate your account to the new PDS using the Manual Account Migration process."
                    }
                }
            }
        }
    }
}
