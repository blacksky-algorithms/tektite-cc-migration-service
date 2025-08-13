use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use tracing::{error, info, instrument};

use super::identity_resolver::WebIdentityResolver;
use super::session::JwtUtils;
use super::types::*;
use super::errors::ClientError;

/// Client for ATProto PDS operations
pub struct PdsClient {
    http_client: Client,
    identity_resolver: WebIdentityResolver,
}

impl PdsClient {
    /// Create a new PDS client
    pub fn new() -> Self {
        Self {
            http_client: {
                #[cfg(target_arch = "wasm32")]
                {
                    Client::builder()
                        .user_agent("atproto-migration-service/1.0")
                        .build()
                        .expect("Failed to create HTTP client")
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    Client::builder()
                        .timeout(std::time::Duration::from_secs(30))
                        .user_agent("atproto-migration-service/1.0")
                        .build()
                        .expect("Failed to create HTTP client")
                }
            },
            identity_resolver: WebIdentityResolver::new(),
        }
    }

    /// Login to a PDS using identifier and password
    // NEWBOLD.md Step: goat account login --pds-host $NEWPDSHOST -u $ACCOUNTDID -p $NEWPASSWORD (line 52)
    // Implements: Creates session on PDS for specified account identifier
    #[instrument(skip(self, password), err)]
    pub async fn login(&self, identifier: &str, password: &str) -> Result<ClientLoginResponse, ClientError> {
        info!("Starting login for identifier: {}", identifier);
        
        // First resolve identifier to DID if it's a handle
        let (did, pds_url) = if identifier.starts_with("did:") {
            // If it's already a DID, we need to resolve the DID document to find PDS
            let did = identifier.to_string();
            let pds_url = self.resolve_pds_from_did(&did).await?;
            (did, pds_url)
        } else {
            // If it's a handle, resolve to DID first
            let resolved_did = self.identity_resolver
                .resolve_handle(identifier)
                .await
                .map_err(ClientError::ResolutionFailed)?;
            
            let pds_url = self.resolve_pds_from_did(&resolved_did).await?;
            (resolved_did, pds_url)
        };
        
        // Call ATProto createSession
        // NEWBOLD.md: com.atproto.server.createSession for authentication 
        let session_url = format!("{}/xrpc/com.atproto.server.createSession", pds_url);
        let request_body = json!({
            "identifier": identifier,
            "password": password
        });

        info!("Calling createSession at: {}", session_url);

        let response = self.http_client
            .post(&session_url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to call createSession: {}", e),
            })?;

        if response.status().is_success() {
            let session_data: serde_json::Value = response.json().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse response: {}", e),
                })?;
            
            // Parse JWT to get expiration
            let access_jwt = session_data["accessJwt"].as_str().unwrap_or_default().to_string();
            let expires_at = if !access_jwt.is_empty() {
                JwtUtils::get_expiration(&access_jwt)
            } else {
                None
            };

            let session = ClientSessionCredentials {
                did: session_data["did"].as_str().unwrap_or(&did).to_string(),
                handle: session_data["handle"].as_str().unwrap_or(identifier).to_string(),
                pds: pds_url,
                access_jwt,
                refresh_jwt: session_data["refreshJwt"].as_str().unwrap_or_default().to_string(),
                expires_at,
            };

            info!("Login successful for DID: {}", session.did);
            Ok(ClientLoginResponse {
                success: true,
                message: "Login successful".to_string(),
                did: Some(session.did.clone()),
                session: Some(session),
            })
        } else {
            let status = response.status();
            let error_text = response.text().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to read error response: {}", e),
                })?;
            
            error!("Login failed with status {}: {}", status, error_text);
            Ok(ClientLoginResponse {
                success: false,
                message: format!("Login failed: {}", error_text),
                did: None,
                session: None,
            })
        }
    }

    /// Try to login with new PDS credentials to check if account already exists
    #[instrument(skip(self, password), err)]
    pub async fn try_login_before_creation(&self, handle: &str, password: &str, pds_url: &str) -> Result<ClientLoginResponse, ClientError> {
        info!("Trying login before account creation for handle: {}", handle);
        
        let session_url = format!("{}/xrpc/com.atproto.server.createSession", pds_url);
        let request_body = serde_json::json!({
            "identifier": handle,
            "password": password
        });

        let response = self.http_client
            .post(&session_url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to call createSession: {}", e),
            })?;

        if response.status().is_success() {
            let session_data: serde_json::Value = response.json().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse response: {}", e),
                })?;
            
            // Parse JWT to get expiration
            let access_jwt = session_data["accessJwt"].as_str().unwrap_or_default().to_string();
            let expires_at = if !access_jwt.is_empty() {
                JwtUtils::get_expiration(&access_jwt)
            } else {
                None
            };

            let session = ClientSessionCredentials {
                did: session_data["did"].as_str().unwrap_or_default().to_string(),
                handle: session_data["handle"].as_str().unwrap_or(handle).to_string(),
                pds: pds_url.to_string(),
                access_jwt,
                refresh_jwt: session_data["refreshJwt"].as_str().unwrap_or_default().to_string(),
                expires_at,
            };

            info!("Login successful - account already exists for handle: {}", handle);
            Ok(ClientLoginResponse {
                success: true,
                message: "Account already exists - login successful".to_string(),
                did: Some(session.did.clone()),
                session: Some(session),
            })
        } else {
            info!("Login failed - account does not exist for handle: {}", handle);
            Ok(ClientLoginResponse {
                success: false,
                message: "Account does not exist".to_string(),
                did: None,
                session: None,
            })
        }
    }

    /// Create account on a PDS
    // NEWBOLD.md Step: goat account create --pds-host $NEWPDSHOST --existing-did $ACCOUNTDID --handle $NEWHANDLE --password $NEWPASSWORD --email $NEWEMAIL --invite-code $INVITECODE --service-auth $SERVICEAUTH (line 40-47)
    // Implements: Creates account on new PDS with existing DID using service auth token
    #[instrument(skip(self), err)]
    pub async fn create_account(&self, request: ClientCreateAccountRequest) -> Result<ClientCreateAccountResponse, ClientError> {
        info!("Creating account for handle: {}", request.handle);
        
        // Derive PDS URL from handle domain (simplified approach)
        let pds_url = self.derive_pds_url_from_handle(&request.handle);
        
        // NEWBOLD.md: com.atproto.server.createAccount for account creation with existing DID
        let create_url = format!("{}/xrpc/com.atproto.server.createAccount", pds_url);
        let mut request_body = json!({
            "did": request.did,
            "handle": request.handle,
            "password": request.password,
            "email": request.email
        });

        if let Some(invite_code) = &request.invite_code {
            request_body["inviteCode"] = json!(invite_code);
        }

        let mut request_builder = self.http_client
            .post(&create_url)
            .header("Content-Type", "application/json")
            .json(&request_body);

        // Add authorization header if service auth token is provided (for existing DID accounts)
        if let Some(service_auth_token) = &request.service_auth_token {
            request_builder = request_builder.header("Authorization", format!("Bearer {}", service_auth_token));
        }

        let response = request_builder
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to call createAccount: {}", e),
            })?;

        if response.status().is_success() {
            let account_data: serde_json::Value = response.json().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse response: {}", e),
                })?;
            
            // Parse JWT to get expiration
            let access_jwt = account_data["accessJwt"].as_str().unwrap_or_default().to_string();
            let expires_at = if !access_jwt.is_empty() {
                JwtUtils::get_expiration(&access_jwt)
            } else {
                None
            };

            let session = ClientSessionCredentials {
                did: account_data["did"].as_str().unwrap_or(&request.did).to_string(),
                handle: account_data["handle"].as_str().unwrap_or(&request.handle).to_string(),
                pds: pds_url,
                access_jwt,
                refresh_jwt: account_data["refreshJwt"].as_str().unwrap_or_default().to_string(),
                expires_at,
            };

            info!("Account created successfully for DID: {}", session.did);
            Ok(ClientCreateAccountResponse {
                success: true,
                message: "Account created successfully".to_string(),
                session: Some(session),
                error_code: None,
                resumable: false,
            })
        } else {
            let status = response.status();
            let error_text = response.text().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to read error response: {}", e),
                })?;
            
            // Try to parse structured JSON error response
            let (error_code, resumable, session) = if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
                let error_code = error_json.get("error")
                    .and_then(|e| e.as_str())
                    .map(|s| s.to_string());
                
                // Check if this is a resumable error (AlreadyExists)
                let resumable = error_code.as_ref()
                    .map(|code| code == "AlreadyExists")
                    .unwrap_or(false);
                
                // For AlreadyExists during migration, check if session credentials are provided
                let session = if resumable && request.service_auth_token.is_some() {
                    // Some servers may include session credentials in AlreadyExists responses during migration
                    if let (Some(access_jwt), Some(refresh_jwt)) = (
                        error_json.get("accessJwt").and_then(|j| j.as_str()),
                        error_json.get("refreshJwt").and_then(|j| j.as_str())
                    ) {
                        let expires_at = if !access_jwt.is_empty() {
                            JwtUtils::get_expiration(access_jwt)
                        } else {
                            None
                        };

                        Some(ClientSessionCredentials {
                            did: error_json.get("did").and_then(|d| d.as_str()).unwrap_or(&request.did).to_string(),
                            handle: error_json.get("handle").and_then(|h| h.as_str()).unwrap_or(&request.handle).to_string(),
                            pds: pds_url.clone(),
                            access_jwt: access_jwt.to_string(),
                            refresh_jwt: refresh_jwt.to_string(),
                            expires_at,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };
                
                (error_code, resumable, session)
            } else {
                (None, false, None)
            };
            
            if session.is_some() {
                // Special case: AlreadyExists with session credentials provided (successful resumption)
                info!("Account already exists, but session credentials provided for resumption");
                Ok(ClientCreateAccountResponse {
                    success: true, // Mark as success since we got session credentials
                    message: "Account already exists - resuming with provided credentials".to_string(),
                    session,
                    error_code,
                    resumable,
                })
            } else {
                error!("Account creation failed with status {}: {}", status, error_text);
                Ok(ClientCreateAccountResponse {
                    success: false,
                    message: format!("Account creation failed: {}", error_text),
                    session: None,
                    error_code,
                    resumable,
                })
            }
        }
    }

    /// Check account status
    // NEWBOLD.md Step: goat account status (line 58)
    // Implements: Checks migration progress including blobs, records, and validation status
    #[instrument(skip(self), err)]
    pub async fn check_account_status(&self, session: &ClientSessionCredentials) -> Result<ClientAccountStatusResponse, ClientError> {
        // NEWBOLD.md: com.atproto.server.checkAccountStatus for migration progress tracking
        let status_url = format!("{}/xrpc/com.atproto.server.checkAccountStatus", session.pds);

        let response = self.http_client
            .get(&status_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to check account status: {}", e),
            })?;

        if response.status().is_success() {
            let status_data: serde_json::Value = response.json().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse status response: {}", e),
                })?;

            let activated = status_data["activated"].as_bool();
            let expected_blobs = status_data["expectedBlobs"].as_i64();
            let imported_blobs = status_data["importedBlobs"].as_i64();
            let indexed_records = status_data["indexedRecords"].as_i64();
            let private_state_values = status_data["privateStateValues"].as_i64();
            let repo_blocks = status_data["repoBlocks"].as_i64();
            let repo_commit = status_data["repoCommit"].as_str().map(|s| s.to_string());
            let repo_rev = status_data["repoRev"].as_str().map(|s| s.to_string());
            let valid_did = status_data["validDid"].as_bool();

            Ok(ClientAccountStatusResponse {
                success: true,
                message: "Account status retrieved".to_string(),
                activated,
                expected_blobs,
                imported_blobs,
                indexed_records,
                private_state_values,
                repo_blocks,
                repo_commit,
                repo_rev,
                valid_did,
            })
        } else {
            let error_text = response.text().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to read error response: {}", e),
                })?;

            Ok(ClientAccountStatusResponse {
                success: false,
                message: format!("Status check failed: {}", error_text),
                activated: None,
                expected_blobs: None,
                imported_blobs: None,
                indexed_records: None,
                private_state_values: None,
                repo_blocks: None,
                repo_commit: None,
                repo_rev: None,
                valid_did: None,
            })
        }
    }

    /// Refresh session tokens
    #[instrument(skip(self), err)]
    pub async fn refresh_session(&self, session: &ClientSessionCredentials) -> Result<ClientSessionCredentials, ClientError> {
        let refresh_url = format!("{}/xrpc/com.atproto.server.refreshSession", session.pds);

        let response = self.http_client
            .post(&refresh_url)
            .header("Authorization", format!("Bearer {}", session.refresh_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to refresh session: {}", e),
            })?;

        if response.status().is_success() {
            let refresh_data: serde_json::Value = response.json().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse refresh response: {}", e),
                })?;

            let new_access_jwt = refresh_data["accessJwt"].as_str().unwrap_or_default().to_string();
            let expires_at = if !new_access_jwt.is_empty() {
                JwtUtils::get_expiration(&new_access_jwt)
            } else {
                None
            };

            let mut updated_session = session.clone();
            updated_session.access_jwt = new_access_jwt;
            updated_session.refresh_jwt = refresh_data["refreshJwt"].as_str().unwrap_or(&session.refresh_jwt).to_string();
            updated_session.expires_at = expires_at;

            info!("Session refreshed successfully for DID: {}", updated_session.did);
            Ok(updated_session)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Session refresh failed: {}", error_text);
            Err(ClientError::SessionExpired)
        }
    }

    /// Resolve PDS URL from DID by resolving the DID document
    async fn resolve_pds_from_did(&self, did: &str) -> Result<String, ClientError> {
        info!("Resolving PDS URL from DID: {}", did);
        
        // Handle different DID methods
        if did.starts_with("did:plc:") || did.starts_with("did:web:") {
            // Use the identity resolver to get the PDS endpoint from the DID document
            match self.identity_resolver.resolve_did_to_pds_endpoint(did).await {
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
            let domain = format!("{}.{}", parts[parts.len()-2], parts[parts.len()-1]);
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

        let response = self.http_client
            .get(&describe_url)
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to describe server: {}", e),
            })?;

        if response.status().is_success() {
            let server_info = response.json().await
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
    pub async fn export_repository(&self, session: &ClientSessionCredentials) -> Result<ClientRepoExportResponse, ClientError> {
        info!("Exporting repository for DID: {}", session.did);

        // NEWBOLD.md: com.atproto.sync.getRepo for repository export
        let export_url = format!("{}/xrpc/com.atproto.sync.getRepo?did={}", session.pds, session.did);

        let response = self.http_client
            .get(&export_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to export repository: {}", e),
            })?;

        if response.status().is_success() {
            let car_bytes = response.bytes().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to read CAR data: {}", e),
                })?;

            let car_data = car_bytes.to_vec();
            let car_size = car_data.len() as u64;

            info!("Repository exported successfully, size: {} bytes", car_size.to_string());

            Ok(ClientRepoExportResponse {
                success: true,
                message: "Repository exported successfully".to_string(),
                car_data: Some(car_data),
                car_size: Some(car_size),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Repository export failed: {}", error_text);

            Ok(ClientRepoExportResponse {
                success: false,
                message: format!("Repository export failed: {}", error_text),
                car_data: None,
                car_size: None,
            })
        }
    }

    /// Import repository to PDS from CAR file
    // NEWBOLD.md Step: goat repo import ./did:plc:do2ar6uqzrvyzq3wevji6fbe.20250625142552.car (line 81)
    // Implements: Imports repository CAR file to new PDS
    #[instrument(skip(self), err)]
    pub async fn import_repository(&self, session: &ClientSessionCredentials, car_data: Vec<u8>) -> Result<ClientRepoImportResponse, ClientError> {
        info!("Importing repository for DID: {}, CAR size: {} bytes", session.did, car_data.len());

        // NEWBOLD.md: com.atproto.repo.importRepo for CAR file import
        let import_url = format!("{}/xrpc/com.atproto.repo.importRepo", session.pds);

        let response = self.http_client
            .post(&import_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .header("Content-Type", "application/vnd.ipld.car")
            .body(car_data)
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to import repository: {}", e),
            })?;

        if response.status().is_success() {
            info!("Repository imported successfully");

            Ok(ClientRepoImportResponse {
                success: true,
                message: "Repository imported successfully".to_string(),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Repository import failed: {}", error_text);

            Ok(ClientRepoImportResponse {
                success: false,
                message: format!("Repository import failed: {}", error_text),
            })
        }
    }

    /// Get list of missing blobs for account
    // NEWBOLD.md Step: goat account missing-blobs (line 86)
    // Implements: Lists missing blobs that need migration to new PDS
    #[instrument(skip(self), err)]
    pub async fn get_missing_blobs(&self, session: &ClientSessionCredentials, cursor: Option<String>, limit: Option<i64>) -> Result<ClientMissingBlobsResponse, ClientError> {
        info!("Getting missing blobs for DID: {}", session.did);

        // NEWBOLD.md: com.atproto.repo.listMissingBlobs for migration-specific blob enumeration
        let mut missing_blobs_url = format!("{}/xrpc/com.atproto.repo.listMissingBlobs", session.pds);
        let mut query_params = Vec::new();
        
        if let Some(cursor) = cursor {
            query_params.push(format!("cursor={}", cursor));
        }
        if let Some(limit) = limit {
            query_params.push(format!("limit={}", limit));
        }
        
        if !query_params.is_empty() {
            missing_blobs_url.push('?');
            missing_blobs_url.push_str(&query_params.join("&"));
        }

        let response = self.http_client
            .get(&missing_blobs_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to get missing blobs: {}", e),
            })?;

        if response.status().is_success() {
            let blobs_data: serde_json::Value = response.json().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse missing blobs response: {}", e),
                })?;

            // Parse the blobs from the response using proper deserialization
            let missing_blobs = if let Some(blobs_array) = blobs_data.get("blobs").and_then(|b| b.as_array()) {
                blobs_array.iter()
                    .filter_map(|blob| {
                        serde_json::from_value::<ClientMissingBlob>(blob.clone()).ok()
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let cursor = blobs_data.get("cursor").and_then(|c| c.as_str()).map(|s| s.to_string());

            info!("Found {} missing blobs", missing_blobs.len().to_string());

            Ok(ClientMissingBlobsResponse {
                success: true,
                message: format!("Found {} missing blobs", missing_blobs.len()),
                missing_blobs: Some(missing_blobs),
                cursor,
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Failed to get missing blobs: {}", error_text);

            Ok(ClientMissingBlobsResponse {
                success: false,
                message: format!("Failed to get missing blobs: {}", error_text),
                missing_blobs: None,
                cursor: None,
            })
        }
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
        info!("Listing all blobs for DID: {} (sync.listBlobs)", did);

        // NEWBOLD.md: com.atproto.sync.listBlobs for Go goat compatible full blob enumeration
        let mut list_blobs_url = format!("{}/xrpc/com.atproto.sync.listBlobs", session.pds);
        let mut query_params = Vec::new();
        
        // Required parameter
        query_params.push(format!("did={}", did));
        
        // Optional parameters
        if let Some(cursor) = cursor {
            query_params.push(format!("cursor={}", cursor));
        }
        if let Some(limit) = limit {
            query_params.push(format!("limit={}", limit));
        }
        if let Some(since) = since {
            query_params.push(format!("since={}", since));
        }
        
        list_blobs_url.push('?');
        list_blobs_url.push_str(&query_params.join("&"));

        let response = self.http_client
            .get(&list_blobs_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to list blobs: {}", e),
            })?;

        if response.status().is_success() {
            let blobs_data: serde_json::Value = response.json().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse list blobs response: {}", e),
                })?;

            // Parse the CIDs array from the response (matches Go []string structure)
            let cids = if let Some(cids_array) = blobs_data.get("cids").and_then(|c| c.as_array()) {
                cids_array.iter()
                    .filter_map(|cid| cid.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                Vec::new()
            };

            let cursor = blobs_data.get("cursor").and_then(|c| c.as_str()).map(|s| s.to_string());

            info!("Found {} blobs in repository", cids.len());

            Ok(ClientSyncListBlobsResponse {
                success: true,
                message: format!("Found {} blobs", cids.len()),
                cids: Some(cids),
                cursor,
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Failed to list blobs: {}", error_text);

            Ok(ClientSyncListBlobsResponse {
                success: false,
                message: format!("Failed to list blobs: {}", error_text),
                cids: None,
                cursor: None,
            })
        }
    }

    /// Export/download a blob from PDS
    // NEWBOLD.md Step: goat blob export $ACCOUNTDID (line 98) - individual blob download
    // Implements: Downloads individual blob using com.atproto.sync.getBlob
    #[instrument(skip(self), err)]
    pub async fn export_blob(&self, session: &ClientSessionCredentials, cid: String) -> Result<ClientBlobExportResponse, ClientError> {
        info!("Exporting blob {} from DID: {}", cid, session.did);

        // NEWBOLD.md: com.atproto.sync.getBlob for individual blob retrieval
        let export_url = format!("{}/xrpc/com.atproto.sync.getBlob?did={}&cid={}", session.pds, session.did, cid);

        let response = self.http_client
            .get(&export_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to export blob: {}", e),
            })?;

        if response.status().is_success() {
            let blob_bytes = response.bytes().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to read blob data: {}", e),
                })?;

            let blob_data = blob_bytes.to_vec();
            info!("Blob {} exported successfully, size: {} bytes", cid, blob_data.len().to_string());

            Ok(ClientBlobExportResponse {
                success: true,
                message: "Blob exported successfully".to_string(),
                blob_data: Some(blob_data),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Blob export failed: {}", error_text);

            Ok(ClientBlobExportResponse {
                success: false,
                message: format!("Blob export failed: {}", error_text),
                blob_data: None,
            })
        }
    }

    /// Upload a blob to PDS
    // NEWBOLD.md Step: goat blob upload {} (line 104) - individual blob upload
    // Implements: Uploads individual blob using com.atproto.repo.uploadBlob
    #[instrument(skip(self), err)]
    pub async fn upload_blob(&self, session: &ClientSessionCredentials, cid: String, blob_data: Vec<u8>) -> Result<ClientBlobUploadResponse, ClientError> {
        info!("Uploading blob {} to DID: {}, size: {} bytes", cid, session.did, blob_data.len());

        // NEWBOLD.md: com.atproto.repo.uploadBlob for individual blob upload
        let upload_url = format!("{}/xrpc/com.atproto.repo.uploadBlob", session.pds);

        let response = self.http_client
            .post(&upload_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .header("Content-Type", "application/octet-stream")
            .body(blob_data)
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to upload blob: {}", e),
            })?;

        if response.status().is_success() {
            info!("Blob {} uploaded successfully", cid);

            Ok(ClientBlobUploadResponse {
                success: true,
                message: "Blob uploaded successfully".to_string(),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Blob upload failed: {}", error_text);

            Ok(ClientBlobUploadResponse {
                success: false,
                message: format!("Blob upload failed: {}", error_text),
            })
        }
    }

    /// Export preferences from PDS
    // NEWBOLD.md Step: goat bsky prefs export > prefs.json (line 115)
    // Implements: Exports Bluesky preferences for migration
    #[instrument(skip(self), err)]
    pub async fn export_preferences(&self, session: &ClientSessionCredentials) -> Result<ClientPreferencesExportResponse, ClientError> {
        info!("Exporting preferences for DID: {}", session.did);

        // NEWBOLD.md: app.bsky.actor.getPreferences for preferences export
        let preferences_url = format!("{}/xrpc/app.bsky.actor.getPreferences", session.pds);

        let response = self.http_client
            .get(&preferences_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to export preferences: {}", e),
            })?;

        if response.status().is_success() {
            let preferences_data: serde_json::Value = response.json().await
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
    pub async fn import_preferences(&self, session: &ClientSessionCredentials, preferences_json: String) -> Result<ClientPreferencesImportResponse, ClientError> {
        info!("Importing preferences for DID: {}", session.did);

        // NEWBOLD.md: app.bsky.actor.putPreferences for preferences import
        let preferences_url = format!("{}/xrpc/app.bsky.actor.putPreferences", session.pds);

        // Parse the preferences JSON to extract just the preferences array
        let preferences_data: serde_json::Value = serde_json::from_str(&preferences_json)
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to parse preferences JSON: {}", e),
            })?;

        // Extract the preferences array and send it directly as the request body
        // This matches goat's ActorPutPreferences_Input{Preferences: prefsArray}
        let request_body = serde_json::json!({
            "preferences": preferences_data.get("preferences").unwrap_or(&serde_json::json!([]))
        });

        let response = self.http_client
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
    pub async fn get_plc_recommendation(&self, session: &ClientSessionCredentials) -> Result<ClientPlcRecommendationResponse, ClientError> {
        info!("Getting PLC recommendation for DID: {}", session.did);

        let plc_url = format!("{}/xrpc/com.atproto.identity.getRecommendedDidCredentials", session.pds);

        let response = self.http_client
            .get(&plc_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to get PLC recommendation: {}", e),
            })?;

        if response.status().is_success() {
            let plc_data: serde_json::Value = response.json().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse PLC recommendation response: {}", e),
                })?;

            info!("PLC recommendation retrieved successfully");

            Ok(ClientPlcRecommendationResponse {
                success: true,
                message: "PLC recommendation retrieved successfully".to_string(),
                plc_unsigned: Some(plc_data.to_string()),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("PLC recommendation failed: {}", error_text);

            Ok(ClientPlcRecommendationResponse {
                success: false,
                message: format!("PLC recommendation failed: {}", error_text),
                plc_unsigned: None,
            })
        }
    }

    /// Request PLC token from PDS
    #[instrument(skip(self), err)]
    pub async fn request_plc_token(&self, session: &ClientSessionCredentials) -> Result<ClientPlcTokenResponse, ClientError> {
        info!("Requesting PLC token for DID: {}", session.did);

        let token_url = format!("{}/xrpc/com.atproto.identity.requestPlcOperationSignature", session.pds);

        let response = self.http_client
            .post(&token_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to request PLC token: {}", e),
            })?;

        if response.status().is_success() {
            info!("PLC token requested successfully - check email for token");

            Ok(ClientPlcTokenResponse {
                success: true,
                message: "PLC token sent to email. Check your email for verification code.".to_string(),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("PLC token request failed: {}", error_text);

            Ok(ClientPlcTokenResponse {
                success: false,
                message: format!("PLC token request failed: {}", error_text),
            })
        }
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
        info!("Signing PLC operation for DID: {}", session.did);

        // Parse the unsigned PLC operation
        let plc_unsigned_value: serde_json::Value = serde_json::from_str(&plc_unsigned)
            .map_err(|e| ClientError::NetworkError {
                message: format!("Invalid unsigned PLC operation: {}", e),
            })?;

        // Construct the PLC signing endpoint URL
        // NEWBOLD.md: com.atproto.identity.signPlcOperation for PLC operation signing
        let sign_url = format!("{}/xrpc/com.atproto.identity.signPlcOperation", session.pds);

        // Create structured payload matching AT Protocol IdentitySignPlcOperation_Input schema
        let payload = json!({
            "alsoKnownAs": plc_unsigned_value.get("alsoKnownAs"),
            "rotationKeys": plc_unsigned_value.get("rotationKeys"),
            "services": plc_unsigned_value.get("services"),
            "verificationMethods": plc_unsigned_value.get("verificationMethods"),
            "token": token
        });

        info!("Making PLC signing request to: {}", sign_url);

        let response = self.http_client
            .post(&sign_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to sign PLC operation: {}", e),
            })?;

        if response.status().is_success() {
            let json_response: serde_json::Value = response.json().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse sign response: {}", e),
                })?;

            info!("PLC operation signing response received");

            // Extract the 'operation' field from the response (matches Go implementation)
            let operation = json_response.get("operation")
                .ok_or_else(|| ClientError::NetworkError {
                    message: "No 'operation' field in response".to_string(),
                })?;

            // Convert signed operation to pretty JSON string
            let plc_signed = serde_json::to_string_pretty(operation)
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to serialize signed operation: {}", e),
                })?;

            info!("PLC operation signed successfully");

            Ok(ClientPlcSignResponse {
                success: true,
                message: "PLC operation signed successfully".to_string(),
                plc_signed: Some(plc_signed),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("PLC signing failed: {}", error_text);

            Ok(ClientPlcSignResponse {
                success: false,
                message: format!("PLC signing failed: {}", error_text),
                plc_signed: None,
            })
        }
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
        info!("Submitting PLC operation for DID: {}", session.did);

        // Parse the signed PLC operation
        let plc_signed_value: serde_json::Value = serde_json::from_str(&plc_signed)
            .map_err(|e| ClientError::NetworkError {
                message: format!("Invalid signed PLC operation: {}", e),
            })?;

        // Construct the PLC submission endpoint URL
        // NEWBOLD.md: com.atproto.identity.submitPlcOperation for PLC operation submission
        let submit_url = format!("{}/xrpc/com.atproto.identity.submitPlcOperation", session.pds);

        // Wrap signed operation in IdentitySubmitPlcOperation_Input structure (matches Go implementation)
        let submission_payload = json!({
            "operation": plc_signed_value
        });

        info!("Making PLC submission request to: {}", submit_url);

        let response = self.http_client
            .post(&submit_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .header("Content-Type", "application/json")
            .json(&submission_payload)
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to submit PLC operation: {}", e),
            })?;

        if response.status().is_success() {
            info!("PLC operation submitted successfully");

            Ok(ClientPlcSubmitResponse {
                success: true,
                message: "PLC operation submitted successfully".to_string(),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("PLC submission failed: {}", error_text);

            Ok(ClientPlcSubmitResponse {
                success: false,
                message: format!("PLC submission failed: {}", error_text),
            })
        }
    }

    /// Activate account on PDS
    // NEWBOLD.md Step: goat account activate (line 157)
    // Implements: Activates account after successful PLC transition
    #[instrument(skip(self, session), err)]
    pub async fn activate_account(
        &self,
        session: &ClientSessionCredentials,
    ) -> Result<ClientActivationResponse, ClientError> {
        info!("Activating account for DID: {}", session.did);

        // Construct the account activation endpoint URL
        // NEWBOLD.md: com.atproto.server.activateAccount for final account activation
        let activate_url = format!("{}/xrpc/com.atproto.server.activateAccount", session.pds);

        info!("Making account activation request to: {}", activate_url);

        // Make the request - this is a POST with no body (AT Protocol requirement)
        let response = self.http_client
            .post(&activate_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to activate account: {}", e),
            })?;

        if response.status().is_success() {
            info!("Account activated successfully");

            Ok(ClientActivationResponse {
                success: true,
                message: "Account activated successfully".to_string(),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Account activation failed: {}", error_text);

            Ok(ClientActivationResponse {
                success: false,
                message: format!("Account activation failed: {}", error_text),
            })
        }
    }

    /// Deactivate account on PDS
    // NEWBOLD.md Step: goat account deactivate (line 163)
    // Implements: Deactivates old account after successful migration
    #[instrument(skip(self, session), err)]
    pub async fn deactivate_account(
        &self,
        session: &ClientSessionCredentials,
    ) -> Result<ClientDeactivationResponse, ClientError> {
        info!("Deactivating account for DID: {}", session.did);

        // Construct the account deactivation endpoint URL
        // NEWBOLD.md: com.atproto.server.deactivateAccount for old account deactivation
        let deactivate_url = format!("{}/xrpc/com.atproto.server.deactivateAccount", session.pds);

        info!("Making account deactivation request to: {}", deactivate_url);

        // Make the request - this is a POST with empty body
        let response = self.http_client
            .post(&deactivate_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .header("Content-Type", "application/json")
            .json(&json!({}))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to deactivate account: {}", e),
            })?;

        if response.status().is_success() {
            info!("Account deactivated successfully");

            Ok(ClientDeactivationResponse {
                success: true,
                message: "Account deactivated successfully".to_string(),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Account deactivation failed: {}", error_text);

            Ok(ClientDeactivationResponse {
                success: false,
                message: format!("Account deactivation failed: {}", error_text),
            })
        }
    }

    /// Generate service auth token for secure account creation on new PDS
    /// This implements com.atproto.server.getServiceAuth
    // NEWBOLD.md Step: goat account service-auth --lxm com.atproto.server.createAccount --aud $NEWPDSSERVICEDID --duration-sec 3600 (line 33)
    // Implements: Generates service auth token for secure account creation on new PDS
    #[instrument(skip(self), err)]
    pub async fn get_service_auth(
        &self, 
        session: &ClientSessionCredentials,
        aud: &str,           // Target PDS service DID  
        lxm: Option<&str>,   // Method restriction (e.g. com.atproto.server.createAccount)
        exp: Option<u64>     // Expiration timestamp
    ) -> Result<ClientServiceAuthResponse, ClientError> {
        info!("Generating service auth token for audience: {} (method: {:?})", aud, lxm);
        
        // NEWBOLD.md: com.atproto.server.getServiceAuth for secure migration auth token
        let mut service_auth_url = format!("{}/xrpc/com.atproto.server.getServiceAuth", session.pds);
        let mut query_params = Vec::new();
        
        // Required parameter: aud (audience - target PDS service DID)
        query_params.push(format!("aud={}", aud));
        
        // Optional parameter: lxm (method restriction)
        if let Some(method) = lxm {
            query_params.push(format!("lxm={}", method));
        }
        
        // Optional parameter: exp (expiration timestamp) 
        if let Some(expiration) = exp {
            query_params.push(format!("exp={}", expiration));
        }
        
        // Build URL with query parameters (GET request, not POST)
        if !query_params.is_empty() {
            service_auth_url.push('?');
            service_auth_url.push_str(&query_params.join("&"));
        }

        let response = self.http_client
            .get(&service_auth_url)
            .header("Authorization", format!("Bearer {}", session.access_jwt))
            .send()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to call getServiceAuth: {}", e),
            })?;

        if response.status().is_success() {
            let auth_data: serde_json::Value = response.json().await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse service auth response: {}", e),
                })?;

            let token = auth_data["token"].as_str().unwrap_or_default().to_string();
            
            if token.is_empty() {
                error!("Service auth token generation returned empty token");
                return Ok(ClientServiceAuthResponse {
                    success: false,
                    message: "Service auth token generation failed: empty token".to_string(),
                    token: None,
                });
            }

            info!("Service auth token generated successfully");
            Ok(ClientServiceAuthResponse {
                success: true,
                message: "Service auth token generated successfully".to_string(),
                token: Some(token),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            error!("Service auth token generation failed: {}", error_text);

            Ok(ClientServiceAuthResponse {
                success: false,
                message: format!("Service auth token generation failed: {}", error_text),
                token: None,
            })
        }
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
        
        assert_eq!(client.derive_pds_url_from_handle("user.bsky.social"), "https://bsky.social");
        assert_eq!(client.derive_pds_url_from_handle("user.blacksky.app"), "https://blacksky.app");
        assert_eq!(client.derive_pds_url_from_handle("user.example.com"), "https://example.com");
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
        let result = client.resolve_pds_from_did("did:web:fake-nonexistent-domain.com").await;
        assert!(result.is_err(), "Fake web DID should fail resolution");
        
        // Test unsupported DID method
        let result = client.resolve_pds_from_did("did:unknown:test").await;
        assert!(result.is_err(), "Unsupported DID method should fail");
        
        // The error should be PdsOperationFailed for unsupported methods
        match result {
            Err(ClientError::PdsOperationFailed { operation, message: _ }) => {
                assert_eq!(operation, "resolve_pds");
            }
            _ => panic!("Expected PdsOperationFailed error for unsupported DID method"),
        }
    }

    // Note: Integration tests with real PDS endpoints would require valid credentials
    // and should be run separately from unit tests
}