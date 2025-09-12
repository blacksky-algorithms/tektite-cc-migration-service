//! WASM HTTP client using browser fetch API

use crate::services::streaming::traits::BrowserStream;
use crate::{console_debug, console_error, console_info};
use js_sys::Uint8Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, Headers, Request, RequestInit, Response};

/// WASM HTTP client for browser-based requests
pub struct WasmHttpClient;

impl WasmHttpClient {
    /// Create a new WASM HTTP client
    pub fn new() -> Self {
        Self
    }

    /// Get a streaming response from a URL
    pub async fn get_stream(&self, url: &str) -> Result<BrowserStream, String> {
        console_info!("[WasmHttpClient] Creating fetch request for: {}", url);

        let window = window().ok_or("No window object")?;

        let opts = RequestInit::new();
        opts.set_method("GET");

        // Note: Removed compression headers to avoid potential ReadableStream issues in WASM
        // let headers = Headers::new()
        //     .map_err(|e| format!("Failed to create headers: {:?}", e))?;
        // headers.set("Accept-Encoding", "gzip, deflate, br")
        //     .map_err(|e| format!("Failed to set Accept-Encoding header: {:?}", e))?;
        // opts.set_headers(&headers);

        let request = Request::new_with_str_and_init(url, &opts).map_err(|e| {
            console_error!("[WasmHttpClient] Failed to create request: {:?}", e);
            format!("Failed to create request: {:?}", e)
        })?;

        console_debug!("[WasmHttpClient] Sending fetch request");
        let promise = window.fetch_with_request(&request);
        let response = JsFuture::from(promise).await.map_err(|e| {
            console_error!("[WasmHttpClient] Fetch failed: {:?}", e);
            format!("Fetch failed: {:?}", e)
        })?;

        let response: Response = response.dyn_into().map_err(|_| {
            console_error!("[WasmHttpClient] Failed to cast to Response");
            "Failed to cast to Response"
        })?;

        console_debug!(
            "[WasmHttpClient] Response received: {} {}",
            response.status(),
            response.status_text()
        );

        if !response.ok() {
            let error_msg = format!(
                "HTTP error: {} {}",
                response.status(),
                response.status_text()
            );
            console_error!("[WasmHttpClient] {}", error_msg);
            return Err(error_msg);
        }

        // Check if response has a body
        let has_body = response.body().is_some();
        console_debug!("[WasmHttpClient] Response body available: {}", has_body);

        if !has_body {
            console_error!("[WasmHttpClient] Response body is null");
            return Err("Response body is null".to_string());
        }

        console_debug!("[WasmHttpClient] Creating BrowserStream from response");
        BrowserStream::from_response(response).map_err(|e| {
            console_error!("[WasmHttpClient] Failed to create stream: {:?}", e);
            format!("Failed to create stream: {:?}", e)
        })
    }

    /// Post data to a URL
    pub async fn post_data(
        &self,
        url: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<Response, String> {
        self.post_data_with_auth(url, data, content_type, None)
            .await
    }

    /// Post data to a URL with optional authorization header
    pub async fn post_data_with_auth(
        &self,
        url: &str,
        data: Vec<u8>,
        content_type: &str,
        auth_token: Option<&str>,
    ) -> Result<Response, String> {
        console_debug!(
            "[WasmHttpClient] POST request to: {} ({} bytes)",
            url,
            data.len()
        );

        let window = window().ok_or("No window object")?;

        let opts = RequestInit::new();
        opts.set_method("POST");

        // Convert data to Uint8Array
        let uint8_array = Uint8Array::from(&data[..]);
        let js_value: JsValue = uint8_array.into();
        opts.set_body(&js_value);

        // Set headers
        let headers = Headers::new().map_err(|e| format!("Failed to create headers: {:?}", e))?;
        headers
            .set("Content-Type", content_type)
            .map_err(|e| format!("Failed to set header: {:?}", e))?;

        // For streaming uploads, let browser handle chunked encoding automatically
        // Don't set Content-Length as we want chunked transfer for streaming
        console_debug!("[WasmHttpClient] Using browser's automatic chunked encoding for upload");

        // Add authorization header if provided
        if let Some(token) = auth_token {
            headers
                .set("Authorization", &format!("Bearer {}", token))
                .map_err(|e| format!("Failed to set Authorization header: {:?}", e))?;
            console_debug!("[WasmHttpClient] Authorization header added");
        }
        opts.set_headers(&headers);

        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| format!("Failed to create request: {:?}", e))?;

        console_debug!("[WasmHttpClient] Sending POST request");
        let promise = window.fetch_with_request(&request);
        let response = JsFuture::from(promise).await.map_err(|e| {
            console_error!("[WasmHttpClient] POST request failed: {:?}", e);
            format!("Fetch failed: {:?}", e)
        })?;

        let response: Response = response
            .dyn_into()
            .map_err(|_| "Failed to cast to Response")?;

        let status = response.status();
        let status_text = response.status_text();
        console_debug!("[WasmHttpClient] Response: {} {}", status, status_text);

        if !response.ok() {
            match status {
                401 => {
                    console_error!("[WasmHttpClient] Authentication failed (401)");
                    return Err(format!("Authentication failed (401 Unauthorized): {}. Check if access token is valid and has required permissions.", status_text));
                }
                504 => {
                    console_error!("[WasmHttpClient] Gateway timeout (504) - server took too long to respond");
                    return Err("Gateway timeout (504): The server took too long to respond. This may be due to server overload or network issues. Please try again later.".to_string());
                }
                500 => {
                    console_error!("[WasmHttpClient] Server error (500)");
                    return Err(format!("HTTP error: {} {}", status, status_text));
                }
                _ => {
                    // Generic error handling for all other status codes
                    console_error!("[WasmHttpClient] HTTP error: {} {}", status, status_text);
                    return Err(format!("HTTP error: {} {}", status, status_text));
                }
            }
        }

        console_debug!("[WasmHttpClient] POST request completed successfully");
        Ok(response)
    }

    /// Get JSON data from a URL
    pub async fn get_json<T: for<'de> serde::Deserialize<'de>>(
        &self,
        url: &str,
    ) -> Result<T, String> {
        self.get_json_with_auth(url, None).await
    }

    /// Get JSON data from a URL with optional authorization header
    pub async fn get_json_with_auth<T: for<'de> serde::Deserialize<'de>>(
        &self,
        url: &str,
        auth_token: Option<&str>,
    ) -> Result<T, String> {
        let window = window().ok_or("No window object")?;

        let opts = RequestInit::new();
        opts.set_method("GET");

        // Note: Removed compression headers to avoid potential issues in WASM
        let headers = Headers::new().map_err(|e| format!("Failed to create headers: {:?}", e))?;
        // headers.set("Accept-Encoding", "gzip, deflate, br")
        //     .map_err(|e| format!("Failed to set Accept-Encoding header: {:?}", e))?;
        headers
            .set("Accept", "application/json")
            .map_err(|e| format!("Failed to set Accept header: {:?}", e))?;

        // Add authorization header if provided
        if let Some(token) = auth_token {
            headers
                .set("Authorization", &format!("Bearer {}", token))
                .map_err(|e| format!("Failed to set Authorization header: {:?}", e))?;
        }
        opts.set_headers(&headers);

        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| format!("Failed to create request: {:?}", e))?;

        let promise = window.fetch_with_request(&request);
        let response = JsFuture::from(promise)
            .await
            .map_err(|e| format!("Fetch failed: {:?}", e))?;

        let response: Response = response
            .dyn_into()
            .map_err(|_| "Failed to cast to Response")?;

        if !response.ok() {
            let status = response.status();
            let status_text = response.status_text();
            if status == 401 {
                return Err(format!("Authentication failed (401 Unauthorized): {}. Check if access token is valid and has required permissions.", status_text));
            }
            return Err(format!("HTTP error: {} {}", status, status_text));
        }

        let json_promise = response
            .json()
            .map_err(|e| format!("Failed to get JSON: {:?}", e))?;
        let json_value = JsFuture::from(json_promise)
            .await
            .map_err(|e| format!("Failed to parse JSON: {:?}", e))?;

        serde_wasm_bindgen::from_value(json_value)
            .map_err(|e| format!("Failed to deserialize JSON: {:?}", e))
    }
}

impl Default for WasmHttpClient {
    fn default() -> Self {
        Self::new()
    }
}
