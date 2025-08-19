//! User Interface Components  
//!
//! This module contains reusable Dioxus components for the migration service UI:
//!
//! - **forms**: Migration forms for login, PDS selection, and configuration
//! - **display**: Progress indicators, status displays, and information components
//! - **inputs**: Validated input fields and form controls
//! - **layout**: Navigation and page layout components
//!
//! All components are designed to work within the Dioxus framework and support
//! both server-side and WASM deployment targets.

pub mod display;
pub mod forms;
pub mod inputs;
pub mod layout;
