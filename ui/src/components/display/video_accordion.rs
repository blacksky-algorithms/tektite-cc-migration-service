use dioxus::prelude::*;

#[component]
pub fn VideoAccordion() -> Element {
    let mut is_expanded = use_signal(|| false);

    rsx! {
        div {
            class: "video-accordion",

            // Accordion Header/Toggle Button
            button {
                class: "video-accordion-header",
                onclick: move |_| {
                    is_expanded.set(!is_expanded());
                },
                "aria-expanded": "{is_expanded()}",
                "aria-controls": "video-accordion-content",

                div {
                    class: "video-accordion-title",
                    span {
                        class: "video-accordion-icon",
                        if is_expanded() { "📹 ▼" } else { "📹 ▶" }
                    }
                    span {
                        class: "video-accordion-text",
                        "Tutorial: How to Migrate Your Account"
                    }
                }

                div {
                    class: "video-accordion-subtitle",
                    "BlackSky Algorithms - tektite.cc Account Migration Demonstration"
                }
            }

            // Accordion Content
            if is_expanded() {
                div {
                    id: "video-accordion-content",
                    class: "video-accordion-content",
                    "aria-hidden": "false",

                    div {
                        class: "video-accordion-body",
                        div {
                            class: "video-wrapper",
                            iframe {
                                width: "560",
                                height: "315",
                                src: "https://www.youtube-nocookie.com/embed/_SdmiCRYeZA?si=xLDX-VGgdZziQ9uw",
                                title: "YouTube video player - BlackSky Algorithms - tektite.cc Account Migration Demonstration",
                                r#frame_border: "0",
                                allow: "accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share",
                                referrerpolicy: "strict-origin-when-cross-origin",
                                allowfullscreen: true
                            }
                        }
                        p {
                            class: "video-description",
                            "This tutorial demonstrates the complete account migration process from start to finish. Watch this before beginning your migration for the best experience."
                        }
                    }
                }
            }
        }
    }
}
