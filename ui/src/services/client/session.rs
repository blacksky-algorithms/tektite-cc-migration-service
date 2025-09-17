use anyhow::Result;
use gloo_storage::{LocalStorage, SessionStorage, Storage};
use tracing::{info, warn};

use super::errors::ClientError;
use super::types::{current_time_secs, ClientSessionCredentials};
use crate::migration::types::MigrationProgress;

/// Session manager for secure credential storage and management
pub struct SessionManager {
    storage_key: String,
    use_session_storage: bool, // Use sessionStorage instead of localStorage for security
}

impl SessionManager {
    /// Create a new session manager with sessionStorage (secure by default)
    pub fn new(storage_key: &str) -> Self {
        Self {
            storage_key: storage_key.to_string(),
            use_session_storage: true,
        }
    }

    /// Create a session manager with localStorage (for persistent sessions)
    pub fn new_persistent(storage_key: &str) -> Self {
        Self {
            storage_key: storage_key.to_string(),
            use_session_storage: false,
        }
    }

    /// Store session credentials securely
    pub fn store_session(&self, session: &ClientSessionCredentials) -> Result<(), ClientError> {
        let session_json =
            serde_json::to_string(session).map_err(|e| ClientError::SerializationError {
                message: format!("Failed to serialize session: {}", e),
            })?;

        if self.use_session_storage {
            SessionStorage::set(&self.storage_key, session_json).map_err(|e| {
                ClientError::StorageError {
                    message: format!("Failed to store session in sessionStorage: {:?}", e),
                }
            })?;
        } else {
            LocalStorage::set(&self.storage_key, session_json).map_err(|e| {
                ClientError::StorageError {
                    message: format!("Failed to store session in localStorage: {:?}", e),
                }
            })?;
        }

        info!("Session stored securely for DID: {}", session.did);
        Ok(())
    }

    /// Get stored session credentials with validation
    pub fn get_session(&self) -> Result<Option<ClientSessionCredentials>, ClientError> {
        let session_json = if self.use_session_storage {
            match SessionStorage::get::<String>(&self.storage_key) {
                Ok(json) => json,
                Err(_) => return Ok(None),
            }
        } else {
            match LocalStorage::get::<String>(&self.storage_key) {
                Ok(json) => json,
                Err(_) => return Ok(None),
            }
        };

        let session: ClientSessionCredentials =
            serde_json::from_str(&session_json).map_err(|e| ClientError::SerializationError {
                message: format!("Failed to deserialize session: {}", e),
            })?;

        // Check if session is expired
        if session.is_expired() {
            warn!("Stored session is expired for DID: {}", session.did);
            self.clear_session()?;
            return Ok(None);
        }

        Ok(Some(session))
    }

    /// Clear stored session
    pub fn clear_session(&self) -> Result<(), ClientError> {
        if self.use_session_storage {
            SessionStorage::delete(&self.storage_key);
        } else {
            LocalStorage::delete(&self.storage_key);
        }
        info!("Session cleared");
        Ok(())
    }

    /// Check if session needs refresh
    pub fn needs_refresh(&self) -> Result<bool, ClientError> {
        if let Some(session) = self.get_session()? {
            Ok(session.needs_refresh())
        } else {
            Ok(false)
        }
    }

    /// Update session with new tokens (for token refresh)
    pub fn update_session_tokens(
        &self,
        access_jwt: String,
        refresh_jwt: String,
        expires_at: Option<u64>,
    ) -> Result<(), ClientError> {
        if let Some(mut session) = self.get_session()? {
            session.access_jwt = access_jwt;
            session.refresh_jwt = refresh_jwt;
            session.expires_at = expires_at;
            self.store_session(&session)?;
            info!("Session tokens updated for DID: {}", session.did);
        }
        Ok(())
    }

    /// Store migration progress for a DID
    pub fn store_migration_progress(
        &self,
        did: &str,
        progress: &MigrationProgress,
    ) -> Result<(), ClientError> {
        let storage_key = format!("migration_progress_{}", did);
        let progress_json =
            serde_json::to_string(progress).map_err(|e| ClientError::SerializationError {
                message: format!("Failed to serialize migration progress: {}", e),
            })?;

        // Always use localStorage for migration progress (needs persistence across sessions)
        LocalStorage::set(&storage_key, progress_json).map_err(|e| ClientError::StorageError {
            message: format!("Failed to store migration progress: {:?}", e),
        })?;

        info!("Migration progress stored for DID: {}", did);
        Ok(())
    }

    /// Get migration progress for a DID
    pub fn get_migration_progress(
        &self,
        did: &str,
    ) -> Result<Option<MigrationProgress>, ClientError> {
        let storage_key = format!("migration_progress_{}", did);

        let progress_json = match LocalStorage::get::<String>(&storage_key) {
            Ok(json) => json,
            Err(_) => return Ok(None),
        };

        let progress: MigrationProgress =
            serde_json::from_str(&progress_json).map_err(|e| ClientError::SerializationError {
                message: format!("Failed to deserialize migration progress: {}", e),
            })?;

        Ok(Some(progress))
    }

    /// Clear migration progress for a DID
    pub fn clear_migration_progress(&self, did: &str) -> Result<(), ClientError> {
        let storage_key = format!("migration_progress_{}", did);
        LocalStorage::delete(&storage_key);
        info!("Migration progress cleared for DID: {}", did);
        Ok(())
    }
}

/// JWT token utilities
pub struct JwtUtils;

impl JwtUtils {
    /// Parse JWT expiration time (basic implementation without verification)
    pub fn get_expiration(jwt: &str) -> Option<u64> {
        let parts: Vec<&str> = jwt.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        // Decode the payload (second part)
        let payload_b64 = parts[1];

        // Add padding if needed
        let padded = match payload_b64.len() % 4 {
            2 => format!("{}==", payload_b64),
            3 => format!("{}=", payload_b64),
            _ => payload_b64.to_string(),
        };

        // Decode base64
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&padded)
            .ok()?;
        let payload_str = String::from_utf8(decoded).ok()?;

        // Parse JSON to get exp claim
        let payload: serde_json::Value = serde_json::from_str(&payload_str).ok()?;
        payload.get("exp")?.as_u64()
    }

    /// Check if JWT is expired
    pub fn is_expired(jwt: &str) -> bool {
        if let Some(exp) = Self::get_expiration(jwt) {
            let now = current_time_secs();
            now >= exp
        } else {
            true // Assume expired if we can't parse
        }
    }

    /// Check if JWT needs refresh (within 5 minutes of expiry)
    pub fn needs_refresh(jwt: &str) -> bool {
        if let Some(exp) = Self::get_expiration(jwt) {
            let now = current_time_secs();
            now >= (exp - 300) // 5 minutes before expiry
        } else {
            true
        }
    }
}

/// Migration-specific session manager
pub struct MigrationSessionManager {
    old_session_manager: SessionManager,
    new_session_manager: SessionManager,
}

impl MigrationSessionManager {
    /// Create managers for old and new PDS sessions
    pub fn new() -> Self {
        Self {
            old_session_manager: SessionManager::new("old_pds_session"),
            new_session_manager: SessionManager::new("new_pds_session"),
        }
    }

    /// Store old PDS session
    pub fn store_old_session(&self, session: &ClientSessionCredentials) -> Result<(), ClientError> {
        self.old_session_manager.store_session(session)
    }

    /// Store new PDS session
    pub fn store_new_session(&self, session: &ClientSessionCredentials) -> Result<(), ClientError> {
        self.new_session_manager.store_session(session)
    }

    /// Get old PDS session
    pub fn get_old_session(&self) -> Result<Option<ClientSessionCredentials>, ClientError> {
        self.old_session_manager.get_session()
    }

    /// Get new PDS session
    pub fn get_new_session(&self) -> Result<Option<ClientSessionCredentials>, ClientError> {
        self.new_session_manager.get_session()
    }

    /// Clear all migration sessions
    pub fn clear_all_sessions(&self) -> Result<(), ClientError> {
        self.old_session_manager.clear_session()?;
        self.new_session_manager.clear_session()?;
        info!("All migration sessions cleared");
        Ok(())
    }

    /// Check if migration can continue (both sessions valid)
    pub fn can_continue_migration(&self) -> Result<bool, ClientError> {
        let old_session = self.get_old_session()?;
        let new_session = self.get_new_session()?;

        Ok(old_session.is_some() && new_session.is_some())
    }
}

impl Default for MigrationSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session() -> ClientSessionCredentials {
        ClientSessionCredentials {
            did: "did:plc:test123".to_string(),
            handle: "test.example.com".to_string(),
            pds: "https://test.pds.example.com".to_string(),
            access_jwt: "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJkaWQ6cGxjOnRlc3QxMjMiLCJpYXQiOjE2MjM5NzY0MDAsImV4cCI6OTk5OTk5OTk5OX0.test".to_string(),
            refresh_jwt: "refresh_token".to_string(),
            expires_at: Some(9999999999), // Far future
        }
    }

    #[test]
    fn test_session_storage_and_retrieval() {
        let manager = SessionManager::new("test_session");
        let session = create_test_session();

        // Store session
        manager.store_session(&session).unwrap();

        // Retrieve session
        let retrieved = manager.get_session().unwrap().unwrap();
        assert_eq!(retrieved.did, session.did);
        assert_eq!(retrieved.handle, session.handle);

        // Clear session
        manager.clear_session().unwrap();
        assert!(manager.get_session().unwrap().is_none());
    }

    #[test]
    fn test_jwt_utilities() {
        // Test JWT with expiration in far future
        let jwt = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ0ZXN0IiwiaWF0IjoxNjIzOTc2NDAwLCJleHAiOjk5OTk5OTk5OTl9.test";

        assert!(!JwtUtils::is_expired(jwt));
        assert!(!JwtUtils::needs_refresh(jwt));

        // Test JWT with expiration in past
        let expired_jwt = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ0ZXN0IiwiaWF0IjoxNjIzOTc2NDAwLCJleHAiOjE2MjM5NzY0MDB9.test";

        assert!(JwtUtils::is_expired(expired_jwt));
        assert!(JwtUtils::needs_refresh(expired_jwt));
    }

    #[test]
    fn test_migration_session_manager() {
        let migration_manager = MigrationSessionManager::new();
        let old_session = create_test_session();
        let mut new_session = create_test_session();
        new_session.did = "did:plc:new123".to_string();

        // Store both sessions
        migration_manager.store_old_session(&old_session).unwrap();
        migration_manager.store_new_session(&new_session).unwrap();

        // Check if migration can continue
        assert!(migration_manager.can_continue_migration().unwrap());

        // Verify sessions are different
        let retrieved_old = migration_manager.get_old_session().unwrap().unwrap();
        let retrieved_new = migration_manager.get_new_session().unwrap().unwrap();
        assert_ne!(retrieved_old.did, retrieved_new.did);

        // Clear all sessions
        migration_manager.clear_all_sessions().unwrap();
        assert!(!migration_manager.can_continue_migration().unwrap());
    }
}
