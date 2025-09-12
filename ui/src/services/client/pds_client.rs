use anyhow::Result;
use cid::Cid;
use reqwest::Client;
use tracing::{error, info, instrument};

use super::errors::ClientError;
use super::identity_resolver::WebIdentityResolver;
use super::types::*;

/// Client for ATProto PDS operations
#[derive(Clone)]
pub struct PdsClient {
    pub(crate) http_client: Client,
    pub(crate) identity_resolver: WebIdentityResolver,
}

impl PdsClient {
    /// Create a new PDS client
    pub fn new() -> Self {
        Self {
            http_client: {
                Client::builder()
                    .user_agent("tektite-cc-atproto-migration-service/1.0")
                    .build()
                    .expect("Failed to create HTTP client")
            },
            identity_resolver: WebIdentityResolver::new(),
        }
    }

    /// Login to a PDS using identifier and password
    // NEWBOLD.md Step: goat account login --pds-host $NEWPDSHOST -u $ACCOUNTDID -p $NEWPASSWORD (line 52)
    // Implements: Creates session on PDS for specified account identifier
    #[instrument(skip(self, password), err)]
    pub async fn login(
        &self,
        identifier: &str,
        password: &str,
    ) -> Result<ClientLoginResponse, ClientError> {
        crate::services::client::auth::login_impl(self, identifier, password).await
    }

    /// Try to login with full options including auth factor and takendown support
    pub async fn try_login_before_creation_full(
        &self,
        handle: &str,
        password: &str,
        pds_url: &str,
        auth_factor_token: Option<&str>,
        allow_takendown: Option<bool>,
    ) -> Result<ClientLoginResponse, ClientError> {
        crate::services::client::auth::try_login_before_creation_full_impl(
            self,
            handle,
            password,
            pds_url,
            auth_factor_token,
            allow_takendown,
        )
        .await
    }

    /// Original function now calls the full implementation
    pub async fn try_login_before_creation(
        &self,
        handle: &str,
        password: &str,
        pds_url: &str,
    ) -> Result<ClientLoginResponse, ClientError> {
        self.try_login_before_creation_full(
            handle, password, pds_url, None, // No auth factor token
            None, // Default takendown behavior
        )
        .await
    }

    // /// Try to login with new PDS credentials to check if account already exists
    // #[instrument(skip(self, password), err)]
    // pub async fn try_login_before_creation(
    //     &self,
    //     handle: &str,
    //     password: &str,
    //     pds_url: &str,
    // ) -> Result<ClientLoginResponse, ClientError> {
    //     crate::services::client::auth::try_login_before_creation_impl(
    //         self, handle, password, pds_url,
    //     )
    //     .await
    // }

    /// Create account on a PDS
    // NEWBOLD.md Step: goat account create --pds-host $NEWPDSHOST --existing-did $ACCOUNTDID --handle $NEWHANDLE --password $NEWPASSWORD --email $NEWEMAIL --invite-code $INVITECODE --service-auth $SERVICEAUTH (line 40-47)
    // Implements: Creates account on new PDS with existing DID using service auth token
    #[instrument(skip(self), err)]
    pub async fn create_account(
        &self,
        request: ClientCreateAccountRequest,
    ) -> Result<ClientCreateAccountResponse, ClientError> {
        crate::services::client::auth::create_account_impl(self, request).await
    }

    /// Check account status
    // NEWBOLD.md Step: goat account status (line 58)
    // Implements: Checks migration progress including blobs, records, and validation status
    #[instrument(skip(self), err)]
    pub async fn check_account_status(
        &self,
        session: &ClientSessionCredentials,
    ) -> Result<ClientAccountStatusResponse, ClientError> {
        crate::services::client::auth::check_account_status_impl(self, session).await
    }

    /// Refresh session tokens
    #[instrument(skip(self), err)]
    pub async fn refresh_session(
        &self,
        session: &ClientSessionCredentials,
    ) -> Result<ClientSessionCredentials, ClientError> {
        crate::services::client::auth::refresh_session_impl(self, session).await
    }

    /// Resolve PDS URL from DID by resolving the DID document
    pub(crate) async fn resolve_pds_from_did(&self, did: &str) -> Result<String, ClientError> {
        info!("Resolving PDS URL from DID: {}", did);

        // Handle different DID methods
        if did.starts_with("did:plc:") || did.starts_with("did:web:") {
            // Use the identity resolver to get the PDS endpoint from the DID document
            match self
                .identity_resolver
                .resolve_did_to_pds_endpoint(did)
                .await
            {
                Ok(pds_url) => {
                    info!("Resolved DID {} to PDS: {}", did, pds_url);
                    Ok(pds_url)
                }
                Err(e) => {
                    error!("Failed to resolve DID {} to PDS endpoint: {}", did, e);
                    // Convert ResolveError to ClientError
                    Err(ClientError::ResolutionFailed(e))
                }
            }
        } else {
            // Fallback for unsupported DID methods
            error!("Unsupported DID method: {}", did);
            Err(ClientError::PdsOperationFailed {
                operation: "resolve_pds".to_string(),
                message: format!("Unsupported DID method: {}", did),
            })
        }
    }

    /// Derive PDS URL from handle domain (simplified approach)
    pub fn derive_pds_url_from_handle(&self, handle: &str) -> String {
        let parts: Vec<&str> = handle.split('.').collect();
        if parts.len() >= 2 {
            let domain = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
            match domain.as_str() {
                "bsky.social" => "https://bsky.social".to_string(),
                "blacksky.app" => "https://blacksky.app".to_string(),
                _ => format!("https://{}", domain), // Assume domain hosts PDS
            }
        } else {
            "https://bsky.social".to_string() // Fallback
        }
    }

    /// Get PDS server information
    #[instrument(skip(self), err)]
    pub async fn describe_server(&self, pds_url: &str) -> Result<serde_json::Value, ClientError> {
        let describe_url = format!("{}/xrpc/com.atproto.server.describeServer", pds_url);

        let response = self
            .http_client
            .get(&describe_url)
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to describe server: {}", e),
            })?;

        if response.status().is_success() {
            let server_info = response
                .json()
                .await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse server description: {}", e),
                })?;

            Ok(server_info)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(ClientError::PdsOperationFailed {
                operation: "describe_server".to_string(),
                message: format!("Server description failed: {}", error_text),
            })
        }
    }

    /// Export repository from PDS as CAR file
    // NEWBOLD.md Step: goat repo export $ACCOUNTDID (line 76)
    // Implements: Exports repository as CAR file for migration
    #[instrument(skip(self), err)]
    pub async fn export_repository(
        &self,
        session: &ClientSessionCredentials,
    ) -> Result<ClientRepoExportResponse, ClientError> {
        crate::services::client::api::export_repository_impl(self, session).await
    }

    /// Import repository to PDS from CAR file
    // NEWBOLD.md Step: goat repo import ./did:plc:do2ar6uqzrvyzq3wevji6fbe.20250625142552.car (line 81)
    // Implements: Imports repository CAR file to new PDS
    #[instrument(skip(self), err)]
    pub async fn import_repository(
        &self,
        session: &ClientSessionCredentials,
        car_data: Vec<u8>,
    ) -> Result<ClientRepoImportResponse, ClientError> {
        crate::services::client::api::import_repository_impl(self, session, car_data).await
    }

    /// Get list of missing blobs for account
    // NEWBOLD.md Step: goat account missing-blobs (line 86)
    // Implements: Lists missing blobs that need migration to new PDS
    #[instrument(skip(self), err)]
    pub async fn get_missing_blobs(
        &self,
        session: &ClientSessionCredentials,
        cursor: Option<String>,
        limit: Option<i64>,
    ) -> Result<ClientMissingBlobsResponse, ClientError> {
        crate::services::client::api::get_missing_blobs_impl(self, session, cursor, limit).await
    }

    /// List all blobs in repository using com.atproto.sync.listBlobs (matches Go goat)
    /// This method provides full blob enumeration like the Go SyncListBlobs implementation
    // NEWBOLD.md Compatible: Matches goat blob export enumeration pattern for full repository listing
    // Implements: Full blob enumeration using com.atproto.sync.listBlobs (Go goat compatible)
    #[instrument(skip(self), err)]
    pub async fn sync_list_blobs(
        &self,
        session: &ClientSessionCredentials,
        did: &str,
        cursor: Option<String>,
        limit: Option<i64>,
        since: Option<String>,
    ) -> Result<ClientSyncListBlobsResponse, ClientError> {
        crate::services::client::api::sync_list_blobs_impl(self, session, did, cursor, limit, since)
            .await
    }

    /// List ALL blobs from source PDS with automatic pagination (Go goat runBlobExport compatible)
    /// This method provides complete blob enumeration like the Go SyncListBlobs with pagination
    // NEWBOLD.md Compatible: Full blob enumeration with pagination like Go goat blob export
    // Implements: Complete source blob inventory using com.atproto.sync.listBlobs with auto-pagination
    #[instrument(skip(self), err)]
    pub async fn list_all_source_blobs(
        &self,
        session: &ClientSessionCredentials,
        did: &str,
    ) -> Result<Vec<Cid>, ClientError> {
        crate::services::client::api::list_all_source_blobs_impl(self, session, did).await
    }

    /// List ALL blobs from target PDS with automatic pagination (for reconciliation)
    /// This method provides complete blob enumeration for the target PDS to compare with source
    // Implements: Complete target blob inventory using com.atproto.sync.listBlobs with auto-pagination
    #[instrument(skip(self), err)]
    pub async fn list_all_target_blobs(
        &self,
        session: &ClientSessionCredentials,
        did: &str,
    ) -> Result<Vec<Cid>, ClientError> {
        crate::services::client::api::list_all_target_blobs_impl(self, session, did).await
    }

    /// Verify that specific blobs exist on the target PDS using direct getBlob calls
    /// This is more reliable than enumeration for recently uploaded blobs due to eventual consistency
    pub async fn verify_blobs_exist(
        &self,
        session: &ClientSessionCredentials,
        cids: &[Cid],
    ) -> Result<Vec<Cid>, ClientError> {
        crate::services::client::api::verify_blobs_exist_impl(self, session, cids).await
    }

    /// Export/download a blob from PDS
    // NEWBOLD.md Step: goat blob export $ACCOUNTDID (line 98) - individual blob download
    // Implements: Downloads individual blob using com.atproto.sync.getBlob
    #[instrument(skip(self), err)]
    pub async fn export_blob(
        &self,
        session: &ClientSessionCredentials,
        cid: &Cid,
    ) -> Result<ClientBlobExportResponse, ClientError> {
        crate::services::client::api::export_blob_impl(self, session, cid).await
    }

    /// Determine if a blob should use streaming based on size
    /// Helps storage managers decide between memory-efficient vs simple approaches
    pub fn should_use_streaming(blob_size_bytes: u64) -> bool {
        const STREAMING_THRESHOLD: u64 = 10 * 1024 * 1024; // 10MB
        blob_size_bytes > STREAMING_THRESHOLD
    }

    /// Get the streaming threshold for blob operations
    pub fn get_streaming_threshold() -> u64 {
        10 * 1024 * 1024 // 10MB
    }

    /// Stream export/download a blob from PDS (memory efficient for large blobs)
    /// Returns response that can be used to access bytes_stream() - caller handles the stream
    ///
    /// # Example Usage
    /// ```rust,ignore
    /// let response = client.export_blob_stream(session, cid).await?;
    /// let mut stream = response.bytes_stream();
    /// while let Some(chunk) = stream.next().await {
    ///     let bytes = chunk?;
    ///     // Process chunk without loading entire blob in memory
    /// }
    /// ```
    #[instrument(skip(self), err)]
    pub async fn export_blob_stream(
        &self,
        session: &ClientSessionCredentials,
        cid: String,
    ) -> Result<reqwest::Response, ClientError> {
        crate::services::client::api::export_blob_stream_impl(self, session, cid).await
    }

    /// Stream a blob in chunks using enhanced buffering for memory-efficient processing
    /// Uses optimized buffering strategy based on available memory constraints
    ///
    /// # Example Usage
    /// ```rust,ignore
    /// let response = client.export_blob_stream(session, cid).await?;
    /// let chunk_stream = client.stream_blob_chunked(response, 1024 * 1024)?; // 1MB chunks
    ///
    /// use futures::StreamExt;
    /// while let Some(chunk_result) = chunk_stream.next().await {
    ///     let chunk_bytes = chunk_result?;
    ///     // Process each chunk without loading entire blob in memory
    /// }
    /// ```
    pub fn stream_blob_chunked(
        &self,
        response: reqwest::Response,
        chunk_size: usize,
    ) -> Result<impl futures::Stream<Item = Result<bytes::Bytes, ClientError>>, ClientError> {
        crate::services::client::api::stream_blob_chunked_impl(self, response, chunk_size)
    }

    // Helper method removed - now handled in blob.rs module

    /// Upload a blob to PDS
    // NEWBOLD.md Step: goat blob upload {} (line 104) - individual blob upload
    // Implements: Uploads individual blob using com.atproto.repo.uploadBlob
    #[instrument(skip(self), err)]
    pub async fn upload_blob(
        &self,
        session: &ClientSessionCredentials,
        cid: &Cid,
        blob_data: Vec<u8>,
    ) -> Result<ClientBlobUploadResponse, ClientError> {
        crate::services::client::api::upload_blob_impl(self, session, cid, blob_data).await
    }

    /// Stream upload a blob to PDS (memory efficient for large blobs)  
    /// Accepts pre-collected blob data for WASM32 compatibility
    /// For true streaming, use the regular upload_blob method with chunked processing at higher level
    #[instrument(skip(self), err)]
    pub async fn upload_blob_chunked(
        &self,
        session: &ClientSessionCredentials,
        cid: String,
        blob_data: Vec<u8>,
    ) -> Result<ClientBlobUploadResponse, ClientError> {
        crate::services::client::api::upload_blob_chunked_impl(self, session, cid, blob_data).await
    }

    /// Stream upload a blob from a stream of bytes with triple buffer optimization
    /// Uses triple buffering for memory-efficient collection and upload processing
    ///
    /// # Example Usage
    /// ```rust,ignore
    /// use futures::StreamExt;
    ///
    /// let download_response = client.export_blob_stream(&source_session, cid).await?;
    /// let stream = download_response.bytes_stream().map(|chunk| chunk.map_err(Into::into));
    ///
    /// let result = client.upload_blob_stream(&target_session, cid, stream).await?;
    /// ```
    #[instrument(skip(self, stream), err)]
    pub async fn upload_blob_stream<S, E>(
        &self,
        session: &ClientSessionCredentials,
        cid: String,
        stream: S,
    ) -> Result<ClientBlobUploadResponse, ClientError>
    where
        S: futures::Stream<Item = Result<bytes::Bytes, E>> + Unpin,
        E: std::fmt::Display + Send + Sync + 'static,
    {
        crate::services::client::api::upload_blob_stream_impl(self, session, cid, stream).await
    }

    /// Export preferences from PDS
    // NEWBOLD.md Step: goat bsky prefs export > prefs.json (line 115)
    // Implements: Exports Bluesky preferences for migration
    #[instrument(skip(self), err)]
    pub async fn export_preferences(
        &self,
        session: &ClientSessionCredentials,
    ) -> Result<ClientPreferencesExportResponse, ClientError> {
        info!("Exporting preferences for DID: {}", session.did);

        // NEWBOLD.md: app.bsky.actor.getPreferences for preferences export
        let preferences_url = format!("{}/xrpc/app.bsky.actor.getPreferences", session.pds);

        let response = self
            .http_client
            .get(&preferences_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to export preferences: {}", e),
            })?;

        if response.status().is_success() {
            let preferences_data: serde_json::Value =
                response
                    .json()
                    .await
                    .map_err(|e| ClientError::NetworkError {
                        message: format!("Failed to parse preferences response: {}", e),
                    })?;

            info!("Preferences exported successfully");

            Ok(ClientPreferencesExportResponse {
                success: true,
                message: "Preferences exported successfully".to_string(),
                preferences_json: Some(preferences_data.to_string()),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Preferences export failed: {}", error_text);

            Ok(ClientPreferencesExportResponse {
                success: false,
                message: format!("Preferences export failed: {}", error_text),
                preferences_json: None,
            })
        }
    }

    /// Import preferences to PDS
    // NEWBOLD.md Step: goat bsky prefs import prefs.json (line 118)
    // Implements: Imports Bluesky preferences to new PDS
    #[instrument(skip(self), err)]
    pub async fn import_preferences(
        &self,
        session: &ClientSessionCredentials,
        preferences_json: String,
    ) -> Result<ClientPreferencesImportResponse, ClientError> {
        info!("Importing preferences for DID: {}", session.did);

        // NEWBOLD.md: app.bsky.actor.putPreferences for preferences import
        let preferences_url = format!("{}/xrpc/app.bsky.actor.putPreferences", session.pds);

        // Parse the preferences JSON to extract just the preferences array
        let preferences_data: serde_json::Value =
            serde_json::from_str(&preferences_json).map_err(|e| ClientError::NetworkError {
                message: format!("Failed to parse preferences JSON: {}", e),
            })?;

        // Extract the preferences array and send it directly as the request body
        // This matches goat's ActorPutPreferences_Input{Preferences: prefsArray}
        let request_body = serde_json::json!({
            "preferences": preferences_data.get("preferences").unwrap_or(&serde_json::json!([]))
        });

        let response = self
            .http_client
            .post(&preferences_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to import preferences: {}", e),
            })?;

        if response.status().is_success() {
            info!("Preferences imported successfully");

            Ok(ClientPreferencesImportResponse {
                success: true,
                message: "Preferences imported successfully".to_string(),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Preferences import failed: {}", error_text);

            Ok(ClientPreferencesImportResponse {
                success: false,
                message: format!("Preferences import failed: {}", error_text),
            })
        }
    }

    /// Get PLC recommendation from PDS
    #[instrument(skip(self), err)]
    pub async fn get_plc_recommendation(
        &self,
        session: &ClientSessionCredentials,
    ) -> Result<ClientPlcRecommendationResponse, ClientError> {
        crate::services::client::api::get_plc_recommendation_impl(self, session).await
    }

    /// Request PLC token from PDS
    #[instrument(skip(self), err)]
    pub async fn request_plc_token(
        &self,
        session: &ClientSessionCredentials,
    ) -> Result<ClientPlcTokenResponse, ClientError> {
        crate::services::client::api::request_plc_token_impl(self, session).await
    }

    /// Sign PLC operation with verification token
    // NEWBOLD.md Step: goat account plc sign --token $PLCTOKEN ./plc_unsigned.json > plc_signed.json (line 141)
    // Implements: Signs PLC operation with email verification token for identity transition
    #[instrument(skip(self, session, plc_unsigned, token), err)]
    pub async fn sign_plc_operation(
        &self,
        session: &ClientSessionCredentials,
        plc_unsigned: String,
        token: String,
    ) -> Result<ClientPlcSignResponse, ClientError> {
        crate::services::client::api::sign_plc_operation_impl(self, session, plc_unsigned, token)
            .await
    }

    /// Submit PLC operation to PDS
    // NEWBOLD.md Step: goat account plc submit ./plc_signed.json (line 148)
    // Implements: Submits signed PLC operation to complete identity transition
    #[instrument(skip(self, session, plc_signed), err)]
    pub async fn submit_plc_operation(
        &self,
        session: &ClientSessionCredentials,
        plc_signed: String,
    ) -> Result<ClientPlcSubmitResponse, ClientError> {
        crate::services::client::api::submit_plc_operation_impl(self, session, plc_signed).await
    }

    /// Activate account on PDS
    // NEWBOLD.md Step: goat account activate (line 157)
    // Implements: Activates account after successful PLC transition
    #[instrument(skip(self, session), err)]
    pub async fn activate_account(
        &self,
        session: &ClientSessionCredentials,
    ) -> Result<ClientActivationResponse, ClientError> {
        crate::services::client::api::activate_account_impl(self, session).await
    }

    /// Deactivate account on PDS
    // NEWBOLD.md Step: goat account deactivate (line 163)
    // Implements: Deactivates old account after successful migration
    #[instrument(skip(self, session), err)]
    pub async fn deactivate_account(
        &self,
        session: &ClientSessionCredentials,
    ) -> Result<ClientDeactivationResponse, ClientError> {
        crate::services::client::api::deactivate_account_impl(self, session).await
    }

    /// Generate service auth token for secure account creation on new PDS
    /// This implements com.atproto.server.getServiceAuth
    // NEWBOLD.md Step: goat account service-auth --lxm com.atproto.server.createAccount --aud $NEWPDSSERVICEDID --duration-sec 3600 (line 33)
    // Implements: Generates service auth token for secure account creation on new PDS
    #[instrument(skip(self), err)]
    pub async fn get_service_auth(
        &self,
        session: &ClientSessionCredentials,
        aud: &str,         // Target PDS service DID
        lxm: Option<&str>, // Method restriction (e.g. com.atproto.server.createAccount)
        exp: Option<u64>,  // Expiration timestamp
    ) -> Result<ClientServiceAuthResponse, ClientError> {
        crate::services::client::auth::get_service_auth_impl(self, session, aud, lxm, exp).await
    }

    /// Upload a blob with circuit breaker protection
    /// Prevents cascading failures during PDS server issues
    #[instrument(skip(self, blob_data), err)]
    pub async fn upload_blob_with_circuit_breaker(
        &self,
        session: &ClientSessionCredentials,
        cid: String,
        blob_data: Vec<u8>,
    ) -> Result<ClientBlobUploadResponse, ClientError> {
        crate::services::client::api::upload_blob_with_circuit_breaker_impl(
            self, session, cid, blob_data,
        )
        .await
    }

    /// Export a blob with circuit breaker protection
    /// Prevents cascading failures during PDS server issues  
    #[instrument(skip(self), err)]
    pub async fn export_blob_with_circuit_breaker(
        &self,
        session: &ClientSessionCredentials,
        cid: String,
    ) -> Result<ClientBlobExportResponse, ClientError> {
        crate::services::client::api::export_blob_with_circuit_breaker_impl(self, session, cid)
            .await
    }
}

impl Default for PdsClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_pds_url_from_handle() {
        let client = PdsClient::new();

        assert_eq!(
            client.derive_pds_url_from_handle("user.bsky.social"),
            "https://bsky.social"
        );
        assert_eq!(
            client.derive_pds_url_from_handle("user.blacksky.app"),
            "https://blacksky.app"
        );
        assert_eq!(
            client.derive_pds_url_from_handle("user.example.com"),
            "https://example.com"
        );
    }

    #[tokio::test]
    async fn test_resolve_pds_from_did() {
        let client = PdsClient::new();

        // Test PLC DID - This should now try to resolve the actual DID document
        // Since "did:plc:abcd1234" is a fake DID, it will fail resolution
        // This is the correct behavior - we no longer hardcode bsky.social
        let result = client.resolve_pds_from_did("did:plc:abcd1234").await;
        assert!(result.is_err(), "Fake DID should fail resolution");

        // Test Web DID - This should also fail since the domain doesn't exist
        let result = client
            .resolve_pds_from_did("did:web:fake-nonexistent-domain.com")
            .await;
        assert!(result.is_err(), "Fake web DID should fail resolution");

        // Test unsupported DID method
        let result = client.resolve_pds_from_did("did:unknown:test").await;
        assert!(result.is_err(), "Unsupported DID method should fail");

        // The error should be PdsOperationFailed for unsupported methods
        match result {
            Err(ClientError::PdsOperationFailed {
                operation,
                message: _,
            }) => {
                assert_eq!(operation, "resolve_pds");
            }
            _ => panic!("Expected PdsOperationFailed error for unsupported DID method"),
        }
    }

    // Note: Integration tests with real PDS endpoints would require valid credentials
    // and should be run separately from unit tests
}
