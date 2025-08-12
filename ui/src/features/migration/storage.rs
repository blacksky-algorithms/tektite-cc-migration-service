use crate::features::migration::*;
use gloo_storage::errors::StorageError;
use gloo_storage::{LocalStorage, Storage};
use serde::{Deserialize, Serialize};

#[cfg(feature = "web")]
use crate::services::client::ClientSessionCredentials;

#[derive(Serialize, Deserialize, Clone)]
pub struct PlcOperationData {
    pub unsigned: String,
    pub signed: Option<String>,
    pub verification_code: Option<String>,
    pub status: PlcOperationStatus,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum PlcOperationStatus {
    Pending,
    Signed,
    Submitted,
    Completed,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MigrationProgressData {
    pub current_step: FormStep,
    pub completed_steps: Vec<String>,
    pub blob_migration_status: BlobMigrationStatus,
    pub total_blobs: u32,
    pub processed_blobs: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum BlobMigrationStatus {
    NotStarted,
    InProgress,
    Completed,
    Error(String),
}

pub struct LocalStorageManager;

impl LocalStorageManager {
    // Session Management
    pub fn store_old_session(session: &SessionCredentials) -> Result<(), StorageError> {
        LocalStorage::set("old_pds_session", session)
    }

    pub fn store_new_session(session: &SessionCredentials) -> Result<(), StorageError> {
        LocalStorage::set("new_pds_session", session)
    }

    pub fn get_old_session() -> Result<SessionCredentials, StorageError> {
        LocalStorage::get("old_pds_session")
    }

    pub fn get_new_session() -> Result<SessionCredentials, StorageError> {
        LocalStorage::get("new_pds_session")
    }

    // PLC Operation Management
    pub fn store_plc_operation(data: &PlcOperationData) -> Result<(), StorageError> {
        LocalStorage::set("plc_operation_data", data)
    }

    pub fn get_plc_operation() -> Result<PlcOperationData, StorageError> {
        LocalStorage::get("plc_operation_data")
    }

    // Preferences Backup
    pub fn store_user_preferences(preferences: &serde_json::Value) -> Result<(), StorageError> {
        LocalStorage::set("user_preferences", preferences)
    }

    pub fn get_user_preferences() -> Result<serde_json::Value, StorageError> {
        LocalStorage::get("user_preferences")
    }

    // Migration Progress Tracking
    pub fn store_migration_progress(progress: &MigrationProgressData) -> Result<(), StorageError> {
        LocalStorage::set("migration_progress", progress)
    }

    pub fn get_migration_progress() -> Result<MigrationProgressData, StorageError> {
        LocalStorage::get("migration_progress")
    }

    // Cleanup
    pub fn clear_migration_data() -> Result<(), StorageError> {
        LocalStorage::delete("old_pds_session");
        LocalStorage::delete("new_pds_session");
        LocalStorage::delete("plc_operation_data");
        LocalStorage::delete("user_preferences");
        LocalStorage::delete("migration_progress");
        Ok(())
    }

    // Resume Migration Check
    pub fn has_incomplete_migration() -> bool {
        if let Ok(progress) = Self::get_migration_progress() {
            !matches!(
                progress.blob_migration_status,
                BlobMigrationStatus::Completed
            )
        } else {
            false
        }
    }

    // Session type conversion utilities for client-side migration
    #[cfg(feature = "web")]
    pub fn store_client_session_as_old(client_session: &ClientSessionCredentials) -> Result<(), StorageError> {
        let session = Self::client_to_session(client_session);
        Self::store_old_session(&session)
    }

    #[cfg(feature = "web")]
    pub fn store_client_session_as_new(client_session: &ClientSessionCredentials) -> Result<(), StorageError> {
        let session = Self::client_to_session(client_session);
        Self::store_new_session(&session)
    }

    #[cfg(feature = "web")]
    pub fn client_to_session(client_session: &ClientSessionCredentials) -> SessionCredentials {
        SessionCredentials {
            did: client_session.did.clone(),
            handle: client_session.handle.clone(),
            pds: client_session.pds.clone(),
            access_jwt: client_session.access_jwt.clone(),
            refresh_jwt: client_session.refresh_jwt.clone(),
        }
    }

    #[cfg(feature = "web")]
    pub fn session_to_client(session: &SessionCredentials) -> ClientSessionCredentials {
        ClientSessionCredentials {
            did: session.did.clone(),
            handle: session.handle.clone(),
            pds: session.pds.clone(),
            access_jwt: session.access_jwt.clone(),
            refresh_jwt: session.refresh_jwt.clone(),
            expires_at: None, // Will be parsed from JWT if available
        }
    }
}
