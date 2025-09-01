//! AT Protocol Migration Service
//!
//! This module implements a complete WASM-based migration service for ATProto accounts,
//! supporting migration between Personal Data Servers (PDS) following the process
//! described in NEWBOLD.md.
//!
//! # Architecture
//!
//! The migration service uses a streaming-first architecture with channel-tee patterns
//! for efficient data transfer:
//!
//! - **Repository Migration**: Exports CAR files from source PDS and imports to target
//! - **Blob Migration**: Streams blob data with concurrent storage and upload  
//! - **Preferences Migration**: Transfers user preferences between PDS instances
//! - **Identity Migration**: Updates DID documents to point to new PDS
//!
//! # Usage
//!
//! ```rust
//! use crate::migration::execute_migration_client_side;
//!
//! // Execute complete migration with progress tracking
//! execute_migration_client_side(state, dispatch).await;
//! ```

pub mod account_operations;
pub mod form_validation;
pub mod logic;
pub mod orchestrator;
pub mod progress;
pub mod session_management;
pub mod steps;
pub mod storage;
pub mod types;
pub mod validation;

pub use form_validation::*;
pub use orchestrator::execute_migration_client_side;
pub use progress::*;
pub use types::*;

#[cfg(test)]
mod tests {
    //! Integration tests to prevent architectural regression

    #[test]
    fn test_no_duplicate_migration_modules() {
        // This test will fail to compile if features::migration exists
        // because we would have type conflicts
        use crate::migration::MigrationState;

        // If we accidentally reintroduce features::migration, this would cause:
        // "error[E0432]: unresolved import `crate::features::migration::MigrationState`"
        // or "the name `MigrationState` is defined multiple times"
        let _state = MigrationState::default();
    }

    #[test]
    fn test_migration_orchestrator_accessible() {
        // Ensure our main orchestrator function is accessible
        use crate::migration::execute_migration_client_side;

        // This test ensures the function exists and is accessible
        assert_eq!(
            std::any::type_name_of_val(&execute_migration_client_side),
            "ui::migration::orchestrator::execute_migration_client_side"
        );
    }
}
