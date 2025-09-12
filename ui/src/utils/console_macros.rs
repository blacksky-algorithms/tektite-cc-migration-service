/// Macros for properly formatted console logging
/// These macros wrap gloo_console functions and handle formatting properly
/// to prevent BigInt serialization issues in WASM environments.
///
/// Some macros support optional dispatch parameter to capture messages in state.
/// Use the _with_dispatch variants to also send messages to the application state.
#[macro_export]
macro_rules! console_info {
    ($fmt:expr) => {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        gloo_console::info!(format!("[{}] {}", timestamp, $fmt))
    };
    ($fmt:expr, $($arg:tt)*) => {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        gloo_console::info!(format!("[{}] {}", timestamp, format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! console_log {
    ($fmt:expr) => {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        gloo_console::log!(format!("[{}] {}", timestamp, $fmt))
    };
    ($fmt:expr, $($arg:tt)*) => {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        gloo_console::log!(format!("[{}] {}", timestamp, format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! console_warn {
    ($fmt:expr) => {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        gloo_console::warn!(format!("[{}] {}", timestamp, $fmt))
    };
    ($fmt:expr, $($arg:tt)*) => {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        gloo_console::warn!(format!("[{}] {}", timestamp, format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! console_error {
    ($fmt:expr) => {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        gloo_console::error!(format!("[{}] {}", timestamp, $fmt))
    };
    ($fmt:expr, $($arg:tt)*) => {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        gloo_console::error!(format!("[{}] {}", timestamp, format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! console_debug {
    ($fmt:expr) => {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        gloo_console::debug!(format!("[{}] {}", timestamp, $fmt))
    };
    ($fmt:expr, $($arg:tt)*) => {
        let timestamp = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
        gloo_console::debug!(format!("[{}] {}", timestamp, format!($fmt, $($arg)*)))
    };
}

/// Console macros with dispatch support for capturing messages in application state
/// These variants both log to console AND send the message to the dispatch system
#[macro_export]
macro_rules! console_log_with_dispatch {
    ($dispatch:expr, $fmt:expr) => {
        gloo_console::log!($fmt);
        $dispatch.call($crate::migration::MigrationAction::AddConsoleMessage(format!("[LOG] {}", $fmt)));
    };
    ($dispatch:expr, $fmt:expr, $($arg:tt)*) => {
        let formatted = format!($fmt, $($arg)*);
        gloo_console::log!(&formatted);
        $dispatch.call($crate::migration::MigrationAction::AddConsoleMessage(format!("[LOG] {}", formatted)));
    };
}

#[macro_export]
macro_rules! console_debug_with_dispatch {
    ($dispatch:expr, $fmt:expr) => {
        gloo_console::debug!($fmt);
        $dispatch.call($crate::migration::MigrationAction::AddConsoleMessage(format!("[DEBUG] {}", $fmt)));
    };
    ($dispatch:expr, $fmt:expr, $($arg:tt)*) => {
        let formatted = format!($fmt, $($arg)*);
        gloo_console::debug!(&formatted);
        $dispatch.call($crate::migration::MigrationAction::AddConsoleMessage(format!("[DEBUG] {}", formatted)));
    };
}

#[macro_export]
macro_rules! console_info_with_dispatch {
    ($dispatch:expr, $fmt:expr) => {
        gloo_console::info!($fmt);
        $dispatch.call($crate::migration::MigrationAction::AddConsoleMessage(format!("[INFO] {}", $fmt)));
    };
    ($dispatch:expr, $fmt:expr, $($arg:tt)*) => {
        let formatted = format!($fmt, $($arg)*);
        gloo_console::info!(&formatted);
        $dispatch.call($crate::migration::MigrationAction::AddConsoleMessage(format!("[INFO] {}", formatted)));
    };
}

#[macro_export]
macro_rules! console_warn_with_dispatch {
    ($dispatch:expr, $fmt:expr) => {
        gloo_console::warn!($fmt);
        $dispatch.call($crate::migration::MigrationAction::AddConsoleMessage(format!("[WARN] {}", $fmt)));
    };
    ($dispatch:expr, $fmt:expr, $($arg:tt)*) => {
        let formatted = format!($fmt, $($arg)*);
        gloo_console::warn!(&formatted);
        $dispatch.call($crate::migration::MigrationAction::AddConsoleMessage(format!("[WARN] {}", formatted)));
    };
}

#[macro_export]
macro_rules! console_error_with_dispatch {
    ($dispatch:expr, $fmt:expr) => {
        gloo_console::error!($fmt);
        $dispatch.call($crate::migration::MigrationAction::AddConsoleMessage(format!("[ERROR] {}", $fmt)));
    };
    ($dispatch:expr, $fmt:expr, $($arg:tt)*) => {
        let formatted = format!($fmt, $($arg)*);
        gloo_console::error!(&formatted);
        $dispatch.call($crate::migration::MigrationAction::AddConsoleMessage(format!("[ERROR] {}", formatted)));
    };
}
