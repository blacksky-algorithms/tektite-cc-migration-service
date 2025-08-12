use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct LoadingIndicatorProps {
    pub message: String,
}

#[component]
pub fn LoadingIndicator(props: LoadingIndicatorProps) -> Element {
    rsx! {
        div {
            class: "loading-indicator",
            "⏳ {props.message}"
        }
    }
}
