// Legacy server-side imports removed - using client-side now
use dioxus::prelude::*;
use gloo_storage::{LocalStorage, Storage};
use serde_json;

use crate::components::{
    inputs::{ValidatedInput, InputType},
    display::ProviderDisplay,
};
use crate::migration::types::*;
use crate::services::client::{compat, ClientLoginRequest, ClientPdsProvider, resolve_handle_shared, pds_login};

#[derive(Props, PartialEq, Clone)]
pub struct LoginFormComponentProps {
    pub state: Signal<MigrationState>,
    pub dispatch: EventHandler<MigrationAction>,
}

#[component]
pub fn LoginFormComponent(props: LoginFormComponentProps) -> Element {
    let state = props.state;
    let dispatch = props.dispatch;

    rsx! {
        div {
            class: "migration-form form-1",

            h2 {
                class: "form-title",
                "Step 1: Login to Current PDS"
            }

            // Handle/DID Input Section
            div {
                class: "input-section",
                label {
                    class: "input-label",
                    "Handle or DID:"
                }
                ValidatedInput {
                    value: state().form1.handle,
                    placeholder: "Enter your handle (user.bsky.social) or DID (did:plc:...)".to_string(),
                    input_type: InputType::Text,
                    input_class: "input-field".to_string(),
                    input_style: "".to_string(),
                    disabled: state().session_stored(),
                    on_change: move |data: String| {
                        dispatch.call(MigrationAction::SetHandle(data.clone()));

                        if !data.trim().is_empty() && !data.starts_with("did:") {
                            dispatch.call(MigrationAction::SetLoading(true));
                            spawn(async move {
                                match resolve_handle_shared(data).await {
                                    Ok(pds_provider) => {
                                        dispatch.call(MigrationAction::SetProvider(pds_provider));
                                    }
                                    Err(_) => {
                                        dispatch.call(MigrationAction::SetProvider(PdsProvider::None));
                                    }
                                }
                                dispatch.call(MigrationAction::SetLoading(false));
                            });
                        } else {
                            dispatch.call(MigrationAction::SetProvider(PdsProvider::None));
                            dispatch.call(MigrationAction::SetLoading(false));
                        }
                    }
                }
            }

            // Provider Display
            div {
                class: "provider-section",
                ProviderDisplay {
                    provider: state().form1.provider,
                    handle: state().form1.handle,
                    is_loading: state().form1.is_loading
                }
            }

            // Password Input Section
            div {
                class: "input-section",
                label {
                    class: "input-label",
                    "Password:"
                }
                ValidatedInput {
                    value: state().form1.password,
                    placeholder: "Enter your password".to_string(),
                    input_type: InputType::Password,
                    input_class: "input-field".to_string(),
                    input_style: "".to_string(),
                    disabled: state().session_stored(),
                    on_change: move |data: String| {
                        dispatch.call(MigrationAction::SetPassword(data));
                    }
                }
            }

            // Login Button
            div {
                class: "button-section",
                button {
                    class: "login-button",
                    disabled: state().form1.is_authenticating || state().form1.handle.trim().is_empty() || state().form1.password.trim().is_empty() || state().session_stored(),
                    onclick: move |_| {
                        let current_state = state();
                        let handle_value = current_state.form1.handle.trim().to_string();
                        let form = PdsLoginForm {
                            handle_or_did: handle_value.clone(),
                            password: current_state.form1.password.trim().to_string(),
                        };

                        // Store the original handle for later use
                        dispatch.call(MigrationAction::SetOriginalHandle(handle_value));
                        dispatch.call(MigrationAction::SetAuthenticating(true));
                        dispatch.call(MigrationAction::SetLoginResponse(None));

                        spawn(async move {
                            match pds_login(form).await {
                                Ok(response) => {
                                    if response.success {
                                        // Store session data with key "old_pds_session"
                                        if let Some(session) = &response.session {
                                            if let Ok(session_json) = serde_json::to_string(session) {
                                                let _ = LocalStorage::set("old_pds_session", session_json);
                                                dispatch.call(MigrationAction::SetSessionStored(true));
                                            }
                                        }
                                    }
                                    dispatch.call(MigrationAction::SetLoginResponse(Some(response)));
                                }
                                Err(e) => {
                                    dispatch.call(MigrationAction::SetLoginResponse(Some(PdsLoginResponse {
                                        success: false,
                                        message: format!("Error: {}", e),
                                        did: None,
                                        session: None,
                                    })));
                                }
                            }
                            dispatch.call(MigrationAction::SetAuthenticating(false));
                        });
                    },
                    if state().form1.is_authenticating {
                        "Authenticating..."
                    } else if state().session_stored() {
                        "Session Stored ✓"
                    } else {
                        "Login"
                    }
                }
            }

            // Authentication Result
            if let Some(result) = &state().form1.login_response {
                div {
                    class: if result.success { "auth-result success" } else { "auth-result error" },
                    div {
                        class: "result-message",
                        if result.success { "✓ {result.message}" } else { "✗ {result.message}" }
                    }
                    if result.success && state().session_stored() {
                        div {
                            class: "session-success-notice",
                            "✓ Login successful"
                        }
                    }
                }
            }
        }
    }
}
