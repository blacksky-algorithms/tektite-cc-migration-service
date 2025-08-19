//! Migration Resume Handlers
//!
//! This module handles resuming migrations from various checkpoints
//! when migrations are interrupted and need to continue from where they left off.

#[cfg(feature = "web")]
use crate::services::client::{ClientSessionCredentials, PdsClient};

use crate::migration::types::MigrationCheckpoint;

/// Check if migration can be resumed based on account status
#[cfg(feature = "web")]
pub async fn can_resume_migration(session: &ClientSessionCredentials) -> Result<bool, String> {
    let pds_client = PdsClient::new();
    match pds_client.check_account_status(session).await {
        Ok(response) => {
            if response.success {
                // Account is resumable if it exists but is not activated
                let is_resumable = response.activated == Some(false);
                Ok(is_resumable)
            } else {
                Ok(false)
            }
        }
        Err(_) => Ok(false), // If we can't check status, assume not resumable
    }
}

/// Determine migration checkpoint based on account status
#[cfg(feature = "web")]
pub async fn get_migration_checkpoint(
    session: &ClientSessionCredentials,
) -> Result<MigrationCheckpoint, String> {
    let pds_client = PdsClient::new();
    match pds_client.check_account_status(session).await {
        Ok(response) => {
            if !response.success {
                return Err("Failed to get account status".to_string());
            }

            // Determine checkpoint based on repo migration status
            if is_repo_migrated(session).await {
                // Repository migrated, assume we need to check for blob migration completion
                // We'll let the blob verification logic handle the actual checking
                Ok(MigrationCheckpoint::RepoMigrated)
            } else {
                // Account exists but repo not migrated
                Ok(MigrationCheckpoint::AccountCreated)
            }
        }
        Err(e) => Err(format!("Failed to check account status: {}", e)),
    }
}

/// Check if repository has been migrated
#[cfg(feature = "web")]
pub async fn is_repo_migrated(session: &ClientSessionCredentials) -> bool {
    let pds_client = PdsClient::new();
    if let Ok(response) = pds_client.check_account_status(session).await {
        if response.success {
            // Repository is considered migrated if repo_blocks > 2
            return response.repo_blocks.unwrap_or(0) > 2;
        }
    }
    false
}