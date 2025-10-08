use crate::console_info;
use dioxus::prelude::*;

// New import paths after refactoring
use crate::components::display::VideoAccordion;
use crate::components::forms::{MigrationDetailsForm, PdsSelectionForm, PlcVerificationForm};
use crate::migration::{FormStep, MigrationAction, MigrationState};

#[cfg(feature = "web")]
use crate::components::forms::ClientLoginFormComponent;

// Feature will temporarily alias LocalStorageManager until we update it
use crate::migration::storage::LocalStorageManager;

const MIGRATION_SERVICE_CSS: Asset = asset!("/assets/styling/migration_service.css");
const BLACK_LOGO: Asset = asset!("/assets/img/Logos/Black/SVG/Black_FullLogo.svg");

/// Render the appropriate login form based on feature flags
fn render_login_form(
    state: Signal<MigrationState>,
    dispatch: EventHandler<MigrationAction>,
) -> Element {
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
            console_info!(
                "[Migration Service] Incomplete migration detected - resumability available"
            );
            // Could dispatch an action to show resume dialog
        }
    });

    // Dispatch function for actions - using in-place reduction to preserve Dioxus Signal reactivity
    let dispatch = EventHandler::new(move |action: MigrationAction| {
        state.with_mut(|s| {
            s.reduce_in_place(action);
        });
    });

    rsx! {
        document::Link { rel: "stylesheet", href: MIGRATION_SERVICE_CSS }

        div {
            class: "migration-service-container",

            div {
                class: "title-container",
                img {
                    class: "title-logo",
                    src: BLACK_LOGO,
                    alt: "BlackSky Logo"
                }
                h1 {
                    class: "migration-title",
                    "PDS Migration Service"
                }
            }

            // Video Tutorial Accordion
            VideoAccordion {}

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
                    li {
                        "📚 For detailed instructions and troubleshooting, see our "
                        a {
                            href: "https://docs.blacksky.community/migrating-to-blacksky-pds-complete-guide",
                            target: "_blank",
                            class: "banner-link",
                            "Complete Migration Guide"
                        }
                    }
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
