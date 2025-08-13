use dioxus::prelude::*;
use gloo_console as console;

// New import paths after refactoring
use crate::components::forms::{MigrationDetailsForm, PdsSelectionForm, PlcVerificationForm};
use crate::features::migration::{MigrationAction, MigrationState, FormStep};

#[cfg(feature = "web")]
use crate::components::forms::ClientLoginFormComponent;

// Feature will temporarily alias LocalStorageManager until we update it
use crate::features::migration::storage::LocalStorageManager;

const MIGRATION_SERVICE_CSS: Asset = asset!("/assets/styling/migration_service.css");

/// Render the appropriate login form based on feature flags
fn render_login_form(state: Signal<MigrationState>, dispatch: EventHandler<MigrationAction>) -> Element {
    #[cfg(feature = "web")]
    {
        rsx! {
            ClientLoginFormComponent {
                state: state,
                dispatch: dispatch
            }
        }
    }
    
    #[cfg(not(feature = "web"))]
    {
        rsx! {
            div { "Login form not available for non-web features" }
        }
    }
}

#[component]
pub fn MigrationService() -> Element {
    // Consolidated state management
    let mut state = use_signal(MigrationState::default);

    // Check for incomplete migration on startup
    use_effect(move || {
        if LocalStorageManager::has_incomplete_migration() {
            console::info!(
                "[Migration Service] Incomplete migration detected - resumability available"
            );
            // Could dispatch an action to show resume dialog
        }
    });

    // Dispatch function for actions
    let dispatch = EventHandler::new(move |action: MigrationAction| {
        state.with_mut(|s| {
            *s = s.clone().reduce(action);
        });
    });

    rsx! {
        document::Link { rel: "stylesheet", href: MIGRATION_SERVICE_CSS }

        div {
            class: "migration-service-container",

            h1 {
                class: "migration-title",
                "PDS Migration Service"
            }

            // Recommendations Banner
            div {
                class: "recommendations-banner",
                div {
                    class: "banner-header",
                    "⚠️ Important Recommendations"
                }
                ul {
                    class: "recommendation-list",
                    li { "📱➡️💻 Use a laptop or desktop computer for the best experience" }
                    li { "🌐 Use Chrome or a Chromium-based browser for optimal compatibility" }
                    li { "🔐 If you have 2FA enable, please disable it before migration" }
                    li { "📶 If using a mobile device, ensure you have a stable Wi-Fi connection" }
                    li { "⚠️ Use this tool at your own risk - we are not liable for any data loss" }
                }
            }

            // Form 1: Login to Current PDS - Using Client-side by default  
            div {
                class: if state().current_step == FormStep::PlcVerification { "form-frozen" } else { "" },
                {render_login_form(state, dispatch)}
            }

            // Form 2: New PDS URL (shown only after successful login)
            if state().should_show_form2() {
                div {
                    class: if state().current_step == FormStep::PlcVerification { "form-frozen" } else { "" },
                    PdsSelectionForm {
                        state: state,
                        dispatch: dispatch
                    }
                }
            }

            // Form 3: Migration Details (shown after form 2 is submitted)
            if state().should_show_form3() {
                div {
                    class: if state().current_step == FormStep::PlcVerification { "form-frozen" } else { "" },
                    MigrationDetailsForm {
                        state: state,
                        dispatch: dispatch
                    }
                }
            }

            // Form 4: PLC Token Verification (shown during PLC verification step)
            if state().should_show_form4() {
                PlcVerificationForm {
                    state: state,
                    dispatch: dispatch
                }
            }
        }
    }
}