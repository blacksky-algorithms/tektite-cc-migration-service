//! Captcha gate component for PDS account creation verification
//!
//! When a PDS returns `phoneVerificationRequired: true` in its describeServer response,
//! account creation requires a verification code obtained from the PDS captcha gate.
//!
//! This component embeds the PDS gate/signup page in an iframe (matching PDS MOOver's approach)
//! and listens for the redirect with the verification code.

use dioxus::prelude::*;
use wasm_bindgen::prelude::*;

use crate::{console_error, console_info, console_warn};

#[derive(Props, PartialEq, Clone)]
pub struct CaptchaGateProps {
    /// The PDS URL (e.g., "https://blacksky.app")
    pub pds_url: String,
    /// The handle being registered
    pub handle: String,
    /// Called with the verification code on success
    pub on_success: EventHandler<String>,
    /// Called with an error message on failure
    pub on_error: EventHandler<String>,
}

#[component]
pub fn CaptchaGate(props: CaptchaGateProps) -> Element {
    let pds_url = props.pds_url.clone();
    let handle = props.handle.clone();
    let on_success = props.on_success;
    let on_error = props.on_error;

    // Generate a random state parameter for CSRF protection
    let captcha_state = use_signal(|| generate_random_state());
    let mut is_loading = use_signal(|| true);
    let mut completed = use_signal(|| false);

    let state_value = captcha_state();

    // Build the gate URL following the same pattern as PDS MOOver
    let redirect_url = get_origin();
    let gate_url = format!(
        "{}/gate/signup?state={}&handle={}&redirect_url={}",
        pds_url,
        url_encode(&state_value),
        url_encode(&handle),
        url_encode(&redirect_url),
    );

    console_info!("[Captcha] Loading gate URL: {}", gate_url);

    // Listen for postMessage events from the iframe (cross-origin communication)
    // Also poll for same-origin URL changes as fallback
    let state_for_listener = state_value.clone();
    use_effect(move || {
        let state_param = state_for_listener.clone();
        let completed_val = completed();

        if completed_val {
            return;
        }

        // Set up a message event listener for cross-origin iframe communication
        let closure = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
            // Try to parse the message data as a URL with code/state params
            if let Some(data) = event.data().as_string() {
                if let Ok(url) = web_sys::Url::new(&data) {
                    let search_params = url.search_params();
                    let url_state = search_params.get("state");
                    let code = search_params.get("code");

                    if let (Some(url_state), Some(_code)) = (url_state, code) {
                        if url_state == state_param {
                            console_info!("[Captcha] Received verification code via postMessage");
                            // We'll handle this via the poll approach instead
                            // since EventHandler can't be called from a JS closure directly
                        } else {
                            console_warn!("[Captcha] State mismatch in postMessage");
                        }
                    }
                }
            }
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);

        let window = web_sys::window().unwrap();
        let _ =
            window.add_event_listener_with_callback("message", closure.as_ref().unchecked_ref());
        closure.forget(); // Leak closure since we need it to live for the component lifetime
    });

    // Poll the iframe for URL changes (same approach as PDS MOOver)
    let state_for_poll = state_value.clone();
    use_effect(move || {
        let state_param = state_for_poll.clone();

        // Set up polling interval to check iframe URL
        let interval_closure = Closure::wrap(Box::new(move || {
            if completed() {
                return;
            }

            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();

            if let Some(iframe_el) = document.get_element_by_id("captcha-gate-iframe") {
                if let Ok(iframe) = iframe_el.dyn_into::<web_sys::HtmlIFrameElement>() {
                    if let Some(content_window) = iframe.content_window() {
                        // Try to read the iframe location (will fail for cross-origin)
                        if let Ok(location) = content_window.location().href() {
                            if let Ok(url) = web_sys::Url::new(&location) {
                                let search_params = url.search_params();
                                let url_state = search_params.get("state");
                                let code = search_params.get("code");

                                if let Some(url_state) = url_state {
                                    if url_state == state_param {
                                        if let Some(code) = code {
                                            console_info!(
                                                "[Captcha] Verification code received from iframe redirect"
                                            );
                                            completed.set(true);
                                            on_success.call(code);
                                        } else {
                                            console_error!("[Captcha] No code in redirect URL");
                                            on_error.call(
                                                "No verification code returned from captcha"
                                                    .to_string(),
                                            );
                                        }
                                    } else if !url_state.is_empty() {
                                        console_error!(
                                            "[Captcha] State mismatch - possible security issue"
                                        );
                                        on_error.call(
                                            "State mismatch - possible security issue".to_string(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }) as Box<dyn FnMut()>);

        let window = web_sys::window().unwrap();
        let interval_id = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                interval_closure.as_ref().unchecked_ref(),
                100, // Poll every 100ms like PDS MOOver
            )
            .unwrap_or(0);

        interval_closure.forget();

        // Return cleanup (clear interval on unmount)
        // Note: Dioxus use_effect doesn't support cleanup returns in the same way,
        // but the completed flag prevents further processing
        let _ = interval_id;
    });

    rsx! {
        div {
            class: "captcha-gate-wrapper",
            style: "position: relative; width: 100%; margin: 16px 0;",

            div {
                style: "margin-bottom: 8px; font-size: 0.85rem; color: #ccc;",
                "Verification required by the target PDS. Please complete the captcha below:"
            }

            div {
                style: "position: relative; width: 100%; height: 420px; background: white; border: 1px solid #444; border-radius: 8px; overflow: hidden;",

                iframe {
                    id: "captcha-gate-iframe",
                    src: "{gate_url}",
                    title: "Captcha Verification",
                    style: "width: 100%; height: 100%; border: none;",
                    onload: move |_| {
                        is_loading.set(false);
                    }
                }

                if is_loading() {
                    div {
                        style: "position: absolute; top: 0; left: 0; right: 0; bottom: 0; background: rgba(255,255,255,0.9); display: flex; align-items: center; justify-content: center; color: #666;",
                        "Loading verification..."
                    }
                }
            }
        }
    }
}

/// Generate a random hex state string for CSRF protection
fn generate_random_state() -> String {
    let window = web_sys::window().unwrap();
    let crypto = window.crypto().unwrap();
    let mut buf = [0u8; 32];
    let _ = crypto.get_random_values_with_u8_array(&mut buf);
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Get the current window origin
fn get_origin() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_else(|| "https://tektite.cc".to_string())
}

/// URL-encode a string
fn url_encode(s: &str) -> String {
    js_sys::encode_uri_component(s).into()
}
