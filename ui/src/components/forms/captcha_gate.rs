//! Captcha gate component for PDS account creation verification
//!
//! When a PDS returns `phoneVerificationRequired: true` in its describeServer response,
//! account creation requires a verification code obtained from the PDS captcha gate.
//!
//! This component embeds the PDS gate/signup page in an iframe. After the user completes
//! the captcha, the PDS redirects back to our origin with `code` and `state` query params.
//! A small inline script in index.html detects this and sends a postMessage to the parent.
//! This component listens for that message to extract the verification code.

use dioxus::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use crate::{console_error, console_info};

/// Result from the captcha message listener, communicated via shared state
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
    let result_cell: Signal<Rc<RefCell<CaptchaResult>>> =
        use_signal(|| Rc::new(RefCell::new(CaptchaResult::Pending)));

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

    // Set up postMessage listener once
    let state_for_listener = state_value.clone();
    use_effect(move || {
        let state_param = state_for_listener.clone();
        let result_rc = result_cell();
        let result_writer = result_rc.clone();

        // Listen for postMessage from the iframe's captcha callback script
        let listener = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
            // Check if already completed
            {
                let current = result_writer.borrow();
                if !matches!(*current, CaptchaResult::Pending) {
                    return;
                }
            }

            // Parse the message data
            let data = event.data();
            let msg_type = js_sys::Reflect::get(&data, &JsValue::from_str("type")).ok();
            let msg_code = js_sys::Reflect::get(&data, &JsValue::from_str("code")).ok();
            let msg_state = js_sys::Reflect::get(&data, &JsValue::from_str("state")).ok();

            if let (Some(typ), Some(code), Some(state)) = (msg_type, msg_code, msg_state) {
                let type_str = typ.as_string().unwrap_or_default();
                if type_str != "captcha-callback" {
                    return;
                }

                let state_str = state.as_string().unwrap_or_default();
                let code_str = code.as_string().unwrap_or_default();

                if state_str == state_param {
                    if !code_str.is_empty() {
                        console_info!("[Captcha] Received verification code via postMessage");
                        *result_writer.borrow_mut() = CaptchaResult::Success(code_str);
                    } else {
                        *result_writer.borrow_mut() =
                            CaptchaResult::Error("No verification code returned".to_string());
                    }
                } else if !state_str.is_empty() {
                    console_error!(
                        "[Captcha] State mismatch: expected {}, got {}",
                        state_param,
                        state_str
                    );
                    *result_writer.borrow_mut() =
                        CaptchaResult::Error("State mismatch - possible security issue".to_string());
                }
            }
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);

        let window = web_sys::window().unwrap();
        window
            .add_event_listener_with_callback("message", listener.as_ref().unchecked_ref())
            .ok();
        listener.forget();
    });

    // Watch the shared result and dispatch Dioxus events when it changes
    let result_rc_for_watch = result_cell();
    use_effect(move || {
        let result = result_rc_for_watch.borrow().clone();
        match result {
            CaptchaResult::Pending => {}
            CaptchaResult::Success(code) => {
                console_info!("[Captcha] Verification code received from iframe redirect");
                on_success.call(code);
            }
            CaptchaResult::Error(err) => {
                console_error!("[Captcha] Error: {}", err);
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
