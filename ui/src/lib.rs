//! This crate contains all shared UI components for the migration service.

pub mod app;
pub use app::MigrationService;

pub mod components;
pub mod migration;
pub mod services;
pub mod utils;

// Re-export console logging macros for easy use throughout the crate
// The macros are exported directly from this crate due to #[macro_export]

// All functionality now organized in the new structure
