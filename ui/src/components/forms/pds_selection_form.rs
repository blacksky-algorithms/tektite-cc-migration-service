use dioxus::prelude::*;

// Client-side PDS operations
#[cfg(feature = "web")]
use crate::services::client::compat::{describe_server, resolve_handle_shared};

use crate::components::{
    display::LoadingIndicator,
    inputs::{InputType, ValidatedInput},
};
use crate::migration::*;

#[derive(Props, PartialEq, Clone)]
pub struct PdsSelectionFormProps {
    pub state: Signal<MigrationState>,
    pub dispatch: EventHandler<MigrationAction>,
}

#[component]
pub fn PdsSelectionForm(props: PdsSelectionFormProps) -> Element {
    let state = props.state;
    let dispatch = props.dispatch;

    rsx! {
        div {
            class: "migration-form form-2",

            h2 {
                class: "form-title",
                "Step 2: New PDS Host"
            }

            div {
                class: "button-section",
                button {
                    class: "validate-button",
                    style: "margin-bottom: 16px; background-color: #7c3aed;",
                    disabled: state().form2_submitted(),
                    onclick: move |_| {
                        dispatch.call(MigrationAction::SetNewPdsUrl("https://blacksky.app".to_string()));
                        // Trigger PDS describe for Blacksky
                        let url = "https://blacksky.app".to_string();
                        dispatch.call(MigrationAction::SetDescribingPds(true));
                        spawn(async move {
                            #[cfg(feature = "web")]
                            {
                                match describe_server(url).await {
                                    Ok(server_info) => {
                                        // Parse the JSON response to PdsDescribeResponse
                                        match serde_json::from_value::<PdsDescribeResponse>(server_info) {
                                            Ok(response) => {
                                                dispatch.call(MigrationAction::SetPdsDescribeResponse(Some(response.clone())));
                                                dispatch.call(MigrationAction::SetForm2Submitted(true));

                                                // Auto-populate smart handle suggestion if available
                                                let current_state = state();
                                                if let Some(suggested_handle) = current_state.suggest_handle() {
                                                    // Check if the suggested handle is available
                                                    match resolve_handle_shared(suggested_handle.clone()).await {
                                                        Ok(provider) => {
                                                            match provider {
                                                                crate::services::client::ClientPdsProvider::None => {
                                                                    // Handle is available, auto-populate it
                                                                    dispatch.call(MigrationAction::SetNewHandle(suggested_handle));
                                                                    dispatch.call(MigrationAction::SetHandleValidation(HandleValidation::Available));
                                                                }
                                                                _ => {
                                                                    // Handle is unavailable, leave empty
                                                                    // User will see it as placeholder with unavailable styling
                                                                }
                                                            }
                                                        }
                                                        Err(_) => {
                                                            // Error checking, leave empty
                                                        }
                                                    }
                                                }
                                            }
                                            Err(_) => {
                                                dispatch.call(MigrationAction::SetPdsDescribeResponse(None));
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        dispatch.call(MigrationAction::SetPdsDescribeResponse(None));
                                    }
                                }
                            }

                            #[cfg(not(feature = "web"))]
                            {
                                // Fallback - set error state
                                dispatch.call(MigrationAction::SetPdsDescribeResponse(None));
                            }

                            dispatch.call(MigrationAction::SetDescribingPds(false));
                        });
                    },
                    "Migrate to Blacksky"
                }
            }

            div {
                class: "input-section",
                label {
                    class: "input-label",
                    "New PDS Host URL:"
                }
                ValidatedInput {
                    value: state().form2.pds_url,
                    placeholder: "https://new-pds.example.com".to_string(),
                    input_type: InputType::Text,
                    input_class: "input-field".to_string(),
                    input_style: "".to_string(),
                    disabled: state().form2_submitted(),
                    on_change: move |url: String| {
                        dispatch.call(MigrationAction::SetNewPdsUrl(url.clone()));

                        // Reset describe response when URL changes
                        dispatch.call(MigrationAction::SetPdsDescribeResponse(None));
                        dispatch.call(MigrationAction::SetForm2Submitted(false));

                        // Trigger PDS describe if URL is not empty
                        if !url.trim().is_empty() {
                            dispatch.call(MigrationAction::SetDescribingPds(true));
                            spawn(async move {
                                #[cfg(feature = "web")]
                                {
                                    match describe_server(url).await {
                                        Ok(server_info) => {
                                            // Parse the JSON response to PdsDescribeResponse
                                            match serde_json::from_value::<PdsDescribeResponse>(server_info) {
                                                Ok(response) => {
                                                    dispatch.call(MigrationAction::SetPdsDescribeResponse(Some(response.clone())));
                                                    dispatch.call(MigrationAction::SetForm2Submitted(true));

                                                    // Auto-populate smart handle suggestion if available
                                                    let current_state = state();
                                                    if let Some(suggested_handle) = current_state.suggest_handle() {
                                                        // Check if the suggested handle is available
                                                        match resolve_handle_shared(suggested_handle.clone()).await {
                                                            Ok(provider) => {
                                                                match provider {
                                                                    crate::services::client::ClientPdsProvider::None => {
                                                                        // Handle is available, auto-populate it
                                                                        dispatch.call(MigrationAction::SetNewHandle(suggested_handle));
                                                                        dispatch.call(MigrationAction::SetHandleValidation(HandleValidation::Available));
                                                                    }
                                                                    _ => {
                                                                        // Handle is unavailable, leave empty
                                                                        // User will see it as placeholder with unavailable styling
                                                                    }
                                                                }
                                                            }
                                                            Err(_) => {
                                                                // Error checking, leave empty
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(_) => {
                                                    dispatch.call(MigrationAction::SetPdsDescribeResponse(None));
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            dispatch.call(MigrationAction::SetPdsDescribeResponse(None));
                                        }
                                    }
                                }

                                #[cfg(not(feature = "web"))]
                                {
                                    // Fallback - set error state
                                    dispatch.call(MigrationAction::SetPdsDescribeResponse(None));
                                }

                                dispatch.call(MigrationAction::SetDescribingPds(false));
                            });
                        }
                    }
                }
            }

            // Show PDS describe status
            if state().form2.is_describing {
                LoadingIndicator { message: "Describing PDS server...".to_string() }
            } else if let Some(describe_response) = &state().form2.describe_response {
                div {
                    class: "validation-result success",
                    div {
                        style: "font-weight: bold; margin-bottom: 8px;",
                        "✓ PDS Server Found: {describe_response.did}"
                    }
                    div {
                        style: "margin-bottom: 4px;",
                        "Available Domains: {describe_response.available_user_domains.join(\", \")}"
                    }
                    if let Some(invite_required) = describe_response.invite_code_required {
                        div {
                            style: "margin-bottom: 4px;",
                            if invite_required {
                                "⚠️ Invite code required"
                            } else {
                                "✓ No invite code required"
                            }
                        }
                    }
                    if let Some(links) = &describe_response.links {
                        if links.privacy_policy.is_some() || links.terms_of_service.is_some() {
                            div {
                                style: "margin-top: 8px; font-size: 0.75rem; color: #D3FC51;",
                                div {
                                    style: "margin-bottom: 4px;",
                                    "Policy documents:"
                                }
                                ul {
                                    style: "margin: 0; padding-left: 16px; list-style: none;",
                                    if let Some(privacy_url) = &links.privacy_policy {
                                        li {
                                            style: "margin-bottom: 2px;",
                                            a {
                                                href: "{privacy_url}",
                                                target: "_blank",
                                                style: "color: #D3FC51; text-decoration: underline;",
                                                "Privacy Policy"
                                            }
                                        }
                                    }
                                    if let Some(terms_url) = &links.terms_of_service {
                                        li {
                                            style: "margin-bottom: 2px;",
                                            a {
                                                href: "{terms_url}",
                                                target: "_blank",
                                                style: "color: #D3FC51; text-decoration: underline;",
                                                "Terms of Service"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if !state().form2.pds_url.trim().is_empty() && !state().form2.is_describing {
                div {
                    class: "validation-result error",
                    "✗ Unable to describe PDS server. Please check the URL."
                }
            }
        }
    }
}
