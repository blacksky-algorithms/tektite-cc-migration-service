//! Captcha gate component for PDS account creation verification
//!
//! When a PDS returns `phoneVerificationRequired: true` in its describeServer response,
//! account creation requires a verification code obtained from the PDS captcha gate.
//!
//! This component embeds the PDS gate/signup page in an iframe (matching PDS MOOver's approach)
//! and listens for the redirect with the verification code.

use dioxus::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use crate::{console_error, console_info};

/// Result from the captcha polling closure, communicated via shared state
#[derive(Clone, Debug)]
enum CaptchaResult {
    /// Waiting for user to complete captcha
    Pending,
    /// Captcha completed successfully with verification code
    Success(String),
    /// Captcha failed with error message
    Error(String),
}

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

    // Shared result channel between JS closure and Dioxus component
    // This avoids capturing Dioxus signals/event handlers in JS closures
    let result_cell: Signal<Rc<RefCell<CaptchaResult>>> =
        use_signal(|| Rc::new(RefCell::new(CaptchaResult::Pending)));
    // Track the interval ID for cleanup
    let interval_id: Signal<Rc<RefCell<i32>>> = use_signal(|| Rc::new(RefCell::new(0)));

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

    // Set up the polling interval once
    let state_for_poll = state_value.clone();
    use_effect(move || {
        let state_param = state_for_poll.clone();
        let result_rc = result_cell();
        let interval_rc = interval_id();

        // Polling closure only writes to the shared Rc<RefCell<>>, never touches Dioxus signals
        let result_writer = result_rc.clone();
        let interval_ref = interval_rc.clone();
        let interval_closure = Closure::wrap(Box::new(move || {
            // Check if already completed
            {
                let current = result_writer.borrow();
                if !matches!(*current, CaptchaResult::Pending) {
                    return;
                }
            }

            let window = match web_sys::window() {
                Some(w) => w,
                None => return,
            };
            let document = match window.document() {
                Some(d) => d,
                None => return,
            };

            let iframe_el = match document.get_element_by_id("captcha-gate-iframe") {
                Some(el) => el,
                None => return,
            };
            let iframe = match iframe_el.dyn_into::<web_sys::HtmlIFrameElement>() {
                Ok(f) => f,
                Err(_) => return,
            };
            let content_window = match iframe.content_window() {
                Some(w) => w,
                None => return,
            };

            // Try to read the iframe location (will throw for cross-origin until redirect)
            let href = match content_window.location().href() {
                Ok(h) => h,
                Err(_) => return, // Cross-origin - expected while captcha page is shown
            };

            let url = match web_sys::Url::new(&href) {
                Ok(u) => u,
                Err(_) => return,
            };

            let search_params = url.search_params();
            let url_state = search_params.get("state");
            let code = search_params.get("code");

            if let Some(url_state) = url_state {
                if url_state == state_param {
                    if let Some(code) = code {
                        *result_writer.borrow_mut() = CaptchaResult::Success(code);
                        // Clear the interval now that we're done
                        let id = *interval_ref.borrow();
                        if id != 0 {
                            if let Some(w) = web_sys::window() {
                                w.clear_interval_with_handle(id);
                            }
                        }
                    } else {
                        *result_writer.borrow_mut() =
                            CaptchaResult::Error("No verification code returned".to_string());
                    }
                } else if !url_state.is_empty() {
                    *result_writer.borrow_mut() =
                        CaptchaResult::Error("State mismatch - possible security issue".to_string());
                }
            }
        }) as Box<dyn FnMut()>);

        let window = web_sys::window().unwrap();
        let id = window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                interval_closure.as_ref().unchecked_ref(),
                100,
            )
            .unwrap_or(0);

        *interval_rc.borrow_mut() = id;
        interval_closure.forget();
    });

    // Watch the shared result and dispatch Dioxus events when it changes
    // This runs in the Dioxus context where signals and event handlers are safe to use
    let result_rc_for_watch = result_cell();
    let interval_rc_for_cleanup = interval_id();
    use_effect(move || {
        let result = result_rc_for_watch.borrow().clone();
        match result {
            CaptchaResult::Pending => {}
            CaptchaResult::Success(code) => {
                console_info!("[Captcha] Verification code received from iframe redirect");
                // Clear interval
                let id = *interval_rc_for_cleanup.borrow();
                if id != 0 {
                    if let Some(w) = web_sys::window() {
                        w.clear_interval_with_handle(id);
                    }
                }
                on_success.call(code);
            }
            CaptchaResult::Error(err) => {
                console_error!("[Captcha] Error: {}", err);
                let id = *interval_rc_for_cleanup.borrow();
                if id != 0 {
                    if let Some(w) = web_sys::window() {
                        w.clear_interval_with_handle(id);
                    }
                }
                on_error.call(err);
            }
        }
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
