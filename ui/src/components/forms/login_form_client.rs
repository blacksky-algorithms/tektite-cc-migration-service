//! Client-side login form using DNS-over-HTTPS and direct PDS operations

#[cfg(feature = "web")]
use crate::services::client::{MigrationClient, ClientPdsProvider, JwtUtils};

use dioxus::prelude::*;
use gloo_console as console;

use crate::components::{
    input::{InputType, ValidatedInput},
    display::ProviderDisplay,
};
use crate::features::migration::{
    storage::LocalStorageManager,
    *,
};

#[derive(Props, PartialEq, Clone)]
pub struct ClientLoginFormComponentProps {
    pub state: Signal<MigrationState>,
    pub dispatch: EventHandler<MigrationAction>,
}

#[cfg(feature = "web")]
#[component]
pub fn ClientLoginFormComponent(props: ClientLoginFormComponentProps) -> Element {
    let state = props.state;
    let dispatch = props.dispatch;

    rsx! {
        div {
            class: "migration-form form-1",

            h2 {
                class: "form-title",
                "Step 1: Login to Current PDS (Client-Side)"
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
                                let migration_client = MigrationClient::new();
                                let provider = migration_client.determine_provider(&data).await;
                                
                                dispatch.call(MigrationAction::SetProvider(provider));
                                dispatch.call(MigrationAction::SetLoading(false));
                            });
                        } else {
                            dispatch.call(MigrationAction::SetProvider(ClientPdsProvider::None));
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
                        let password_value = current_state.form1.password.trim().to_string();

                        // Store the original handle for later use
                        dispatch.call(MigrationAction::SetOriginalHandle(handle_value.clone()));
                        dispatch.call(MigrationAction::SetAuthenticating(true));
                        dispatch.call(MigrationAction::SetLoginResponse(None));

                        spawn(async move {
                            let migration_client = MigrationClient::new();
                            match migration_client.pds_client.login(&handle_value, &password_value).await {
                                Ok(response) => {
                                    if response.success {
                                        if let Some(ref client_session) = response.session {
                                            // Check if token is expired or will expire soon
                                            if JwtUtils::needs_refresh(&client_session.access_jwt) {
                                                console::warn!("JWT token needs refresh, but continuing with login");
                                            }
                                            
                                            // Store session in localStorage as "old_pds_session" for migration
                                            match LocalStorageManager::store_client_session_as_old(client_session) {
                                                Ok(()) => {
                                                    console::info!("Client-side login successful - session stored in localStorage");
                                                    dispatch.call(MigrationAction::SetSessionStored(true));
                                                }
                                                Err(e) => {
                                                    console::error!("Failed to store session:", format!("{:?}", e));
                                                    dispatch.call(MigrationAction::SetLoginResponse(Some(PdsLoginResponse {
                                                        success: false,
                                                        message: format!("Failed to store session: {:?}", e),
                                                        did: None,
                                                        session: None,
                                                    })));
                                                    dispatch.call(MigrationAction::SetAuthenticating(false));
                                                    return;
                                                }
                                            }
                                        } else {
                                            console::error!("Login successful but no session returned");
                                            dispatch.call(MigrationAction::SetLoginResponse(Some(PdsLoginResponse {
                                                success: false,
                                                message: "Login successful but no session returned".to_string(),
                                                did: None,
                                                session: None,
                                            })));
                                            dispatch.call(MigrationAction::SetAuthenticating(false));
                                            return;
                                        }
                                    }
                                    
                                    // Convert client response to API response format for compatibility
                                    let api_response = PdsLoginResponse {
                                        success: response.success,
                                        message: response.message,
                                        did: response.did,
                                        session: response.session.map(|s| SessionCredentials {
                                            did: s.did,
                                            handle: s.handle,
                                            pds: s.pds,
                                            access_jwt: s.access_jwt,
                                            refresh_jwt: s.refresh_jwt,
                                        }),
                                    };
                                    dispatch.call(MigrationAction::SetLoginResponse(Some(api_response)));
                                }
                                Err(e) => {
                                    console::error!("Client-side login failed:", format!("{}", e));
                                    dispatch.call(MigrationAction::SetLoginResponse(Some(PdsLoginResponse {
                                        success: false,
                                        message: format!("Client-side login error: {}", e),
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
                        "Login (Client-Side)"
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
                            "✓ Login successful (Client-Side DNS-over-HTTPS)"
                        }
                    }
                }
            }
        }
    }
}

// Fallback for when client-side feature is disabled
#[cfg(not(feature = "web"))]
#[component]
pub fn ClientLoginFormComponent(_props: ClientLoginFormComponentProps) -> Element {
    rsx! {
        div {
            class: "migration-form form-1",
            h2 {
                class: "form-title",
                "Client-Side Migration Not Available"
            }
            p {
                "Client-side migration is not enabled. Please enable the 'web' feature."
            }
        }
    }
}