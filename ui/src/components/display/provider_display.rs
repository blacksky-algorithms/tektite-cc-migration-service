use crate::components::display::LoadingIndicator;
use dioxus::prelude::*;
use crate::services::client::ClientPdsProvider;

const BLUESKY_LOGO: Asset = asset!("/assets/img/bluesky_logo.svg");
const BLACKSKY_LOGO: Asset = asset!("/assets/img/blacksky_logo.svg");

#[derive(Props, PartialEq, Clone)]
pub struct ProviderDisplayProps {
    pub provider: ClientPdsProvider,
    pub handle: String,
    pub is_loading: bool,
}

#[component]
pub fn ProviderDisplay(props: ProviderDisplayProps) -> Element {
    if props.is_loading {
        rsx! {
            LoadingIndicator { message: "Resolving...".to_string() }
        }
    } else {
        match props.provider {
            ClientPdsProvider::Other(ref pds_url) => rsx! {
                div {
                    class: "provider-info other",
                    div {
                        class: "provider-logo other-pds",
                        "PDS"
                    }
                    "✓ Custom PDS: {pds_url}"
                }
            },
            ClientPdsProvider::Bluesky => rsx! {
                div {
                    class: "provider-info bluesky",
                    img {
                        class: "provider-logo",
                        src: BLUESKY_LOGO,
                        alt: "Bluesky Logo"
                    }
                    "✓ Bluesky Account"
                }
            },
            ClientPdsProvider::BlackSky => rsx! {
                div {
                    class: "provider-info blacksky",
                    img {
                        class: "provider-logo",
                        src: BLACKSKY_LOGO,
                        alt: "BlackSky Logo"
                    }
                    "✓ BlackSky Account"
                }
            },
            ClientPdsProvider::None => {
                if !props.handle.is_empty() && !props.handle.starts_with("did:") {
                    rsx! {
                        div {
                            class: "provider-info none",
                            "⚠ Unknown provider"
                        }
                    }
                } else {
                    rsx! { div {} }
                }
            }
        }
    }
}
