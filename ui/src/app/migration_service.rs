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

            // Form 1: Login to Current PDS - Using Client-side by default  
            {render_login_form(state, dispatch)}

            // Form 2: New PDS URL (shown only after successful login)
            if state().should_show_form2() {
                PdsSelectionForm {
                    state: state,
                    dispatch: dispatch
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