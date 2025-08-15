//! Migration strategy trait definition

use crate::features::migration::types::MigrationAction;
use crate::services::{
    blob::blob_fallback_manager::FallbackBlobManager,
    client::{ClientMissingBlob, ClientSessionCredentials},
    errors::MigrationResult,
};
use async_trait::async_trait;
use dioxus::prelude::*;

/// Result of a blob migration operation
#[derive(Debug, Clone)]
pub struct BlobMigrationResult {
    pub total_blobs: u32,
    pub uploaded_blobs: u32,
    pub failed_blobs: Vec<BlobFailure>,
    pub total_bytes_processed: u64,
    pub strategy_used: String,
}

/// Details of a failed blob migration
#[derive(Debug, Clone)]
pub struct BlobFailure {
    pub cid: String,
    pub operation: String,
    pub error: String,
}

/// Strategy pattern for blob migration implementations
#[async_trait(?Send)]
pub trait MigrationStrategy {
    /// Execute the migration strategy
    async fn migrate(
        &self,
        blobs: Vec<ClientMissingBlob>,
        old_session: ClientSessionCredentials,
        new_session: ClientSessionCredentials,
        blob_manager: &mut FallbackBlobManager,
        dispatch: &EventHandler<MigrationAction>,
    ) -> MigrationResult<BlobMigrationResult>;

    /// Get the strategy name
    fn name(&self) -> &'static str;

    /// Check if this strategy supports the given blob count
    fn supports_blob_count(&self, count: u32) -> bool;

    /// Check if this strategy supports the given storage backend
    fn supports_storage_backend(&self, backend: &str) -> bool;

    /// Get the priority of this strategy (higher is better)
    fn priority(&self) -> u32;

    /// Estimate the memory usage for the given blob count
    fn estimate_memory_usage(&self, blob_count: u32) -> u64;
}
