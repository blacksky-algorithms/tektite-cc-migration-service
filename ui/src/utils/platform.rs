//! Platform Detection Utilities
//!
//! This module provides browser and platform detection capabilities to enable
//! browser-specific optimizations for blob migration and storage handling.
//!
//! Key features:
//! - Browser detection (Chrome, Firefox, Safari)
//! - Mobile/desktop platform detection  
//! - PWA/Home Screen App detection
//! - Storage persistence detection

use crate::{console_debug, console_warn};
use web_sys::window;

/// Supported browser types with specific storage characteristics
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrowserType {
    /// Chrome: 60% disk quota, auto persistence heuristics
    Chrome,
    /// Firefox: 10GB group limit, user prompt for persistence
    Firefox,
    /// Safari: 60% quota but 7-day deletion, needs Home Screen App for persistence
    Safari,
    /// Unknown/other browsers
    Unknown,
}

impl BrowserType {
    /// Get the display name of the browser
    pub fn name(&self) -> &'static str {
        match self {
            BrowserType::Chrome => "Chrome",
            BrowserType::Firefox => "Firefox",
            BrowserType::Safari => "Safari",
            BrowserType::Unknown => "Unknown",
        }
    }
}

/// App installation/access state
#[derive(Debug, Clone, PartialEq)]
pub enum AppInstallState {
    /// Installed as PWA/Home Screen App (persistent storage)
    InstalledPwa,
    /// Launched from bookmark (potentially persistent)
    LaunchedFromBookmark,
    /// Direct URL access (less persistent)
    DirectAccess,
    /// Regular browser tab (least persistent)
    BrowserTab,
}

/// Detect the current browser type based on user agent
pub fn detect_browser() -> BrowserType {
    let user_agent = window()
        .and_then(|w| w.navigator().user_agent().ok())
        .unwrap_or_default();

    console_debug!("User agent: {}", user_agent);

    // Order matters: Check Chrome first since Safari contains "Chrome" in UA string
    if user_agent.contains("Chrome") && !user_agent.contains("Edg") && !user_agent.contains("OPR") {
        BrowserType::Chrome
    } else if user_agent.contains("Firefox") {
        BrowserType::Firefox
    } else if user_agent.contains("Safari") && !user_agent.contains("Chrome") {
        BrowserType::Safari
    } else {
        BrowserType::Unknown
    }
}

/// Check if running on a mobile platform
pub fn is_mobile_platform() -> bool {
    // Check if mobile device using user agent
    let is_mobile = js_sys::eval(
        r#"/iPhone|iPad|iPod|Android|webOS|BlackBerry|IEMobile|Opera Mini/i.test(navigator.userAgent)"#
    )
    .ok()
    .and_then(|v| v.as_bool())
    .unwrap_or(false);

    console_debug!("Mobile platform detection: {}", is_mobile);
    is_mobile
}

/// Check if running as a PWA/Home Screen App
pub fn is_home_screen_app() -> bool {
    // Check if running in standalone mode (PWA)
    // This works for both iOS home screen apps and installed PWAs
    let is_pwa = js_sys::eval(
        r#"(window.matchMedia('(display-mode: standalone)').matches || 
            window.navigator.standalone === true ||
            window.matchMedia('(display-mode: fullscreen)').matches)"#,
    )
    .ok()
    .and_then(|v| v.as_bool())
    .unwrap_or(false);

    console_debug!("PWA detection: {}", is_pwa);
    is_pwa
}

/// Get comprehensive app installation state
pub fn get_app_install_state() -> AppInstallState {
    let is_pwa = is_home_screen_app();

    // Check if there's a UTM parameter indicating installation
    let has_install_param = window()
        .and_then(|w| w.location().search().ok())
        .map(|search| search.contains("utm_source=homescreen") || search.contains("pwa=1"))
        .unwrap_or(false);

    // Check referrer to see if launched from home
    let direct_access = window()
        .and_then(|w| w.document())
        .map(|d| d.referrer())
        .map(|r| r.is_empty())
        .unwrap_or(false);

    match (is_pwa, has_install_param, direct_access) {
        (true, _, _) => AppInstallState::InstalledPwa,
        (false, true, _) => AppInstallState::LaunchedFromBookmark,
        (false, false, true) => AppInstallState::DirectAccess,
        _ => AppInstallState::BrowserTab,
    }
}

/// Check if storage persistence is likely available
pub fn is_persistent_storage_likely() -> bool {
    let browser = detect_browser();
    let install_state = get_app_install_state();

    match (browser, install_state) {
        // Chrome: Auto-granted based on engagement
        (BrowserType::Chrome, AppInstallState::InstalledPwa) => true,
        (BrowserType::Chrome, AppInstallState::LaunchedFromBookmark) => true,

        // Firefox: User prompted, PWA usually gets it
        (BrowserType::Firefox, AppInstallState::InstalledPwa) => true,

        // Safari: Only reliable with Home Screen Apps
        (BrowserType::Safari, AppInstallState::InstalledPwa) => true,

        // Everything else is uncertain
        _ => false,
    }
}

/// Get platform-aware memory limits for WASM
pub fn get_platform_memory_limits() -> (u64, u64) {
    let is_mobile = is_mobile_platform();
    let browser = detect_browser();

    match (is_mobile, browser) {
        // Mobile Safari is most restrictive
        (true, BrowserType::Safari) => (128 * 1024 * 1024, 256 * 1024 * 1024), // 128MB-256MB

        // Other mobile browsers
        (true, _) => (256 * 1024 * 1024, 512 * 1024 * 1024), // 256MB-512MB

        // Desktop browsers - much more generous
        (false, BrowserType::Chrome) => (1024 * 1024 * 1024, 2048 * 1024 * 1024), // 1GB-2GB
        (false, BrowserType::Firefox) => (1024 * 1024 * 1024, 2048 * 1024 * 1024), // 1GB-2GB
        (false, BrowserType::Safari) => (512 * 1024 * 1024, 1024 * 1024 * 1024),  // 512MB-1GB
        (false, _) => (512 * 1024 * 1024, 1024 * 1024 * 1024), // Conservative for unknown
    }
}

/// Check if user should be warned about Safari's 7-day deletion policy
pub fn should_warn_about_safari_deletion() -> bool {
    let browser = detect_browser();
    let is_pwa = is_home_screen_app();

    match (browser, is_pwa) {
        (BrowserType::Safari, false) => {
            console_warn!(
                "⚠️ Safari will delete stored data after 7 days without user interaction"
            );
            true
        }
        _ => false,
    }
}

/// Get browser-specific storage backend preferences
pub fn get_storage_backend_preferences() -> Vec<&'static str> {
    let browser = detect_browser();
    let is_pwa = is_home_screen_app();

    match (browser, is_pwa) {
        // Safari PWA - all backends work well
        (BrowserType::Safari, true) => vec!["OPFS", "IndexedDB", "LocalStorage"],

        // Safari browser - warn and prefer minimal storage
        (BrowserType::Safari, false) => {
            should_warn_about_safari_deletion();
            vec!["OPFS", "IndexedDB", "LocalStorage"]
        }

        // Firefox - good with all, but watch group limits
        (BrowserType::Firefox, _) => vec!["OPFS", "IndexedDB", "LocalStorage"],

        // Chrome - excellent with all backends
        (BrowserType::Chrome, _) => vec!["OPFS", "IndexedDB", "LocalStorage"],

        // Unknown - conservative approach
        _ => vec!["OPFS", "IndexedDB", "LocalStorage"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_type_name() {
        assert_eq!(BrowserType::Chrome.name(), "Chrome");
        assert_eq!(BrowserType::Firefox.name(), "Firefox");
        assert_eq!(BrowserType::Safari.name(), "Safari");
        assert_eq!(BrowserType::Unknown.name(), "Unknown");
    }

    #[test]
    fn test_memory_limits_are_sensible() {
        let (min, max) = get_platform_memory_limits();
        assert!(min <= max);
        assert!(min >= 128 * 1024 * 1024); // At least 128MB
        assert!(max <= 4096 * 1024 * 1024); // No more than 4GB (WASM32 limit)
    }
}
