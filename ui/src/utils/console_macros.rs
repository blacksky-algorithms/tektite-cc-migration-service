/// Macros for properly formatted console logging
/// These macros wrap gloo_console functions and handle formatting properly
/// to prevent BigInt serialization issues in WASM environments.
#[macro_export]
macro_rules! console_info {
    ($fmt:expr) => {
        gloo_console::info!($fmt)
    };
    ($fmt:expr, $($arg:tt)*) => {
        gloo_console::info!(format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! console_log {
    ($fmt:expr) => {
        gloo_console::log!($fmt)
    };
    ($fmt:expr, $($arg:tt)*) => {
        gloo_console::log!(format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! console_warn {
    ($fmt:expr) => {
        gloo_console::warn!($fmt)
    };
    ($fmt:expr, $($arg:tt)*) => {
        gloo_console::warn!(format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! console_error {
    ($fmt:expr) => {
        gloo_console::error!($fmt)
    };
    ($fmt:expr, $($arg:tt)*) => {
        gloo_console::error!(format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! console_debug {
    ($fmt:expr) => {
        gloo_console::debug!($fmt)
    };
    ($fmt:expr, $($arg:tt)*) => {
        gloo_console::debug!(format!($fmt, $($arg)*))
    };
}