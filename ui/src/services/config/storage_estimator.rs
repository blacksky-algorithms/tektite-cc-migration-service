//! Storage Manager Integration
//!
//! This module provides bindings to the browser's StorageManager.estimate() API
//! to get real storage quota and usage information for better storage management.

use js_sys::Object;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, Navigator, StorageManager};

/// Storage estimate information from the browser's StorageManager API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageEstimate {
    /// Total storage quota available (in bytes)
    pub quota: u64,
    /// Current storage usage (in bytes)  
    pub usage: u64,
    /// Percentage of quota used (0.0 to 1.0)
    pub usage_percentage: f64,
}

impl StorageEstimate {
    /// Check if storage is approaching capacity (>80%)
    pub fn is_near_capacity(&self) -> bool {
        self.usage_percentage > 0.8
    }

    /// Get available storage space (quota - usage)
    pub fn available_bytes(&self) -> u64 {
        self.quota.saturating_sub(self.usage)
    }

    /// Check if a blob of given size would fit
    pub fn can_fit_blob(&self, blob_size: u64) -> bool {
        self.available_bytes() >= blob_size
    }
}

/// Errors that can occur when getting storage estimates
#[derive(Debug, Clone)]
pub enum StorageEstimatorError {
    /// StorageManager API not supported in this browser/context
    NotSupported,
    /// JavaScript error occurred during API call
    JavaScriptError(String),
    /// Invalid response format from browser
    InvalidResponse,
    /// Browser denied access (private browsing, etc.)
    AccessDenied,
}

impl std::fmt::Display for StorageEstimatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageEstimatorError::NotSupported => write!(f, "StorageManager API not supported"),
            StorageEstimatorError::JavaScriptError(msg) => write!(f, "JavaScript error: {}", msg),
            StorageEstimatorError::InvalidResponse => {
                write!(f, "Invalid response from StorageManager")
            }
            StorageEstimatorError::AccessDenied => write!(f, "Access denied to StorageManager"),
        }
    }
}

impl std::error::Error for StorageEstimatorError {}

/// Get storage estimate from browser's StorageManager API
pub async fn get_storage_estimate() -> Result<StorageEstimate, StorageEstimatorError> {
    // Check if we have access to the window object
    let window = window().ok_or(StorageEstimatorError::NotSupported)?;

    // Get navigator
    let navigator: Navigator = window.navigator();

    // Get StorageManager - this will fail if not supported
    let storage_manager: StorageManager = navigator.storage();

    // Call estimate() method - returns a Promise
    let estimate_promise = storage_manager.estimate().map_err(|e| {
        StorageEstimatorError::JavaScriptError(format!("Failed to call estimate(): {:?}", e))
    })?;

    // Await the promise
    let result = JsFuture::from(estimate_promise).await.map_err(|e| {
        StorageEstimatorError::JavaScriptError(format!("estimate() promise failed: {:?}", e))
    })?;

    // Parse the result
    parse_storage_estimate_result(result)
}

/// Parse the JavaScript result from StorageManager.estimate()
fn parse_storage_estimate_result(
    result: JsValue,
) -> Result<StorageEstimate, StorageEstimatorError> {
    // Convert JsValue to Object for property access
    let obj = Object::from(result);

    // Get quota property
    let quota_js = js_sys::Reflect::get(&obj, &"quota".into())
        .map_err(|_| StorageEstimatorError::InvalidResponse)?;
    let quota = quota_js
        .as_f64()
        .ok_or(StorageEstimatorError::InvalidResponse)? as u64;

    // Get usage property
    let usage_js = js_sys::Reflect::get(&obj, &"usage".into())
        .map_err(|_| StorageEstimatorError::InvalidResponse)?;
    let usage = usage_js
        .as_f64()
        .ok_or(StorageEstimatorError::InvalidResponse)? as u64;

    // Calculate usage percentage
    let usage_percentage = if quota > 0 {
        usage as f64 / quota as f64
    } else {
        0.0
    };

    Ok(StorageEstimate {
        quota,
        usage,
        usage_percentage,
    })
}


/// Get storage estimate with graceful fallback
/// Returns None if the API is not supported or fails
pub async fn try_get_storage_estimate() -> Option<StorageEstimate> {
    (get_storage_estimate().await).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_estimate_calculations() {
        let estimate = StorageEstimate {
            quota: 1000,
            usage: 800,
            usage_percentage: 0.8,
        };

        assert_eq!(estimate.available_bytes(), 200);
        assert!(estimate.is_near_capacity());
        assert!(!estimate.can_fit_blob(300));
        assert!(estimate.can_fit_blob(100));
    }

    #[test]
    fn test_storage_estimate_edge_cases() {
        let estimate = StorageEstimate {
            quota: 0,
            usage: 0,
            usage_percentage: 0.0,
        };

        assert_eq!(estimate.available_bytes(), 0);
        assert!(!estimate.is_near_capacity());
        assert!(!estimate.can_fit_blob(1));
    }
}
