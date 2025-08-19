//! WASM HTTP client using browser fetch API

use crate::services::streaming::traits::BrowserStream;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, Response, Headers, window};
use js_sys::Uint8Array;

/// WASM HTTP client for browser-based requests
pub struct WasmHttpClient;

impl WasmHttpClient {
    /// Create a new WASM HTTP client
    pub fn new() -> Self {
        Self
    }
    
    /// Get a streaming response from a URL
    pub async fn get_stream(&self, url: &str) -> Result<BrowserStream, String> {
        let window = window().ok_or("No window object")?;
        
        let opts = RequestInit::new();
        opts.set_method("GET");
        
        // Add compression headers for better transfer efficiency
        let headers = Headers::new()
            .map_err(|e| format!("Failed to create headers: {:?}", e))?;
        headers.set("Accept-Encoding", "gzip, deflate, br")
            .map_err(|e| format!("Failed to set Accept-Encoding header: {:?}", e))?;
        opts.set_headers(&headers);
        
        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| format!("Failed to create request: {:?}", e))?;
        
        let promise = window.fetch_with_request(&request);
        let response = JsFuture::from(promise)
            .await
            .map_err(|e| format!("Fetch failed: {:?}", e))?;
            
        let response: Response = response.dyn_into()
            .map_err(|_| "Failed to cast to Response")?;
        
        if !response.ok() {
            return Err(format!("HTTP error: {} {}", response.status(), response.status_text()));
        }
        
        BrowserStream::from_response(response)
            .map_err(|e| format!("Failed to create stream: {:?}", e))
    }
    
    /// Post data to a URL
    pub async fn post_data(&self, url: &str, data: Vec<u8>, content_type: &str) -> Result<Response, String> {
        let window = window().ok_or("No window object")?;
        
        let opts = RequestInit::new();
        opts.set_method("POST");
        
        // Convert data to Uint8Array
        let uint8_array = Uint8Array::from(&data[..]);
        let js_value: JsValue = uint8_array.into();
        opts.set_body(&js_value);
        
        // Set headers
        let headers = Headers::new()
            .map_err(|e| format!("Failed to create headers: {:?}", e))?;
        headers.set("Content-Type", content_type)
            .map_err(|e| format!("Failed to set header: {:?}", e))?;
        opts.set_headers(&headers);
        
        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| format!("Failed to create request: {:?}", e))?;
            
        let promise = window.fetch_with_request(&request);
        let response = JsFuture::from(promise)
            .await
            .map_err(|e| format!("Fetch failed: {:?}", e))?;
            
        let response: Response = response.dyn_into()
            .map_err(|_| "Failed to cast to Response")?;
        
        if !response.ok() {
            return Err(format!("HTTP error: {} {}", response.status(), response.status_text()));
        }
        
        Ok(response)
    }
    
    /// Get JSON data from a URL
    pub async fn get_json<T: for<'de> serde::Deserialize<'de>>(&self, url: &str) -> Result<T, String> {
        let window = window().ok_or("No window object")?;
        
        let opts = RequestInit::new();
        opts.set_method("GET");
        
        // Add compression headers for better transfer efficiency
        let headers = Headers::new()
            .map_err(|e| format!("Failed to create headers: {:?}", e))?;
        headers.set("Accept-Encoding", "gzip, deflate, br")
            .map_err(|e| format!("Failed to set Accept-Encoding header: {:?}", e))?;
        headers.set("Accept", "application/json")
            .map_err(|e| format!("Failed to set Accept header: {:?}", e))?;
        opts.set_headers(&headers);
        
        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|e| format!("Failed to create request: {:?}", e))?;
        
        let promise = window.fetch_with_request(&request);
        let response = JsFuture::from(promise)
            .await
            .map_err(|e| format!("Fetch failed: {:?}", e))?;
            
        let response: Response = response.dyn_into()
            .map_err(|_| "Failed to cast to Response")?;
        
        if !response.ok() {
            return Err(format!("HTTP error: {} {}", response.status(), response.status_text()));
        }
        
        let json_promise = response.json()
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