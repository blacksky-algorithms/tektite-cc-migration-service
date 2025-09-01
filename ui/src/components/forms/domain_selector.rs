use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
pub struct DomainSelectorProps {
    pub domains: Vec<String>,
    pub selected_domain: String,
    pub disabled: bool,
    pub on_change: EventHandler<String>,
}

#[component]
pub fn DomainSelector(props: DomainSelectorProps) -> Element {
    let domains = props.domains;
    let selected = props.selected_domain;
    let disabled = props.disabled;
    let on_change = props.on_change;

    // If only one domain, show it as a static element
    if domains.len() <= 1 {
        rsx! {
            span {
                class: "handle-domain-suffix",
                "{selected}"
            }
        }
    } else {
        // Multiple domains - show dropdown
        rsx! {
            select {
                class: "domain-selector",
                value: "{selected}",
                disabled: disabled,
                onchange: move |evt| {
                    on_change.call(evt.value());
                },
                for domain in domains {
                    option {
                        value: "{domain}",
                        selected: domain == selected,
                        "{domain}"
                    }
                }
            }
        }
    }
}
