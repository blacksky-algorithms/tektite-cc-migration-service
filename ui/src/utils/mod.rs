//! Utility Functions and Cross-Cutting Concerns
//!
//! This module provides utility functions and macros used throughout the application:
//!
//! - **console_macros**: WASM-compatible logging macros for browser console output
//! - **handle_suggestions**: ATProto handle validation and suggestion utilities
//! - **platform**: Platform detection and WASM environment helpers
//! - **serialization**: JSON serialization utilities for WASM compatibility
//! - **validation**: Form validation and data validation utilities
//!
//! These utilities are designed to work consistently across server-side and WASM
//! deployment targets.

pub mod console_macros;
pub mod handle_suggestions;
pub mod platform;
pub mod serialization;
pub mod validation;

pub use platform::*;
pub use serialization::*;
pub use validation::*;
