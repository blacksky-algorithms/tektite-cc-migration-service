use anyhow::Result;
use serde_json::json;
use tracing::{error, info, instrument};

use crate::services::client::{ClientError, PdsClient};
use crate::services::client::session::JwtUtils;
use crate::services::client::types::*;

/// Implementation of create_account functionality
/// Create account on a PDS
// NEWBOLD.md Step: goat account create --pds-host $NEWPDSHOST --existing-did $ACCOUNTDID --handle $NEWHANDLE --password $NEWPASSWORD --email $NEWEMAIL --invite-code $INVITECODE --service-auth $SERVICEAUTH (line 40-47)
// Implements: Creates account on new PDS with existing DID using service auth token
#[instrument(skip(client), err)]
pub async fn create_account_impl(
    client: &PdsClient,
    request: ClientCreateAccountRequest,
) -> Result<ClientCreateAccountResponse, ClientError> {
    info!("Creating account for handle: {}", request.handle);

    // Derive PDS URL from handle domain (simplified approach)
    let pds_url = client.derive_pds_url_from_handle(&request.handle);

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

    let mut request_builder = client
        .http_client
        .post(&create_url)
        .header("Content-Type", "application/json")
        .json(&request_body);

    // Add authorization header if service auth token is provided (for existing DID accounts)
    if let Some(service_auth_token) = &request.service_auth_token {
        request_builder =
            request_builder.header("Authorization", format!("Bearer {}", service_auth_token));
    }

    let response = request_builder
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to call createAccount: {}", e),
        })?;

    if response.status().is_success() {
        let account_data: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse response: {}", e),
                })?;

        // Parse JWT to get expiration
        let access_jwt = account_data["accessJwt"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let expires_at = if !access_jwt.is_empty() {
            JwtUtils::get_expiration(&access_jwt)
        } else {
            None
        };

        let session = ClientSessionCredentials {
            did: account_data["did"]
                .as_str()
                .unwrap_or(&request.did)
                .to_string(),
            handle: account_data["handle"]
                .as_str()
                .unwrap_or(&request.handle)
                .to_string(),
            pds: pds_url,
            access_jwt,
            refresh_jwt: account_data["refreshJwt"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
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
        let error_text = response
            .text()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to read error response: {}", e),
            })?;

        // Try to parse structured JSON error response
        let (error_code, resumable, session) =
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
                let error_code = error_json
                    .get("error")
                    .and_then(|e| e.as_str())
                    .map(|s| s.to_string());

                // Check if this is a resumable error (AlreadyExists)
                let resumable = error_code
                    .as_ref()
                    .map(|code| code == "AlreadyExists")
                    .unwrap_or(false);

                // For AlreadyExists during migration, check if session credentials are provided
                let session = if resumable && request.service_auth_token.is_some() {
                    // Some servers may include session credentials in AlreadyExists responses during migration
                    if let (Some(access_jwt), Some(refresh_jwt)) = (
                        error_json.get("accessJwt").and_then(|j| j.as_str()),
                        error_json.get("refreshJwt").and_then(|j| j.as_str()),
                    ) {
                        let expires_at = if !access_jwt.is_empty() {
                            JwtUtils::get_expiration(access_jwt)
                        } else {
                            None
                        };

                        Some(ClientSessionCredentials {
                            did: error_json
                                .get("did")
                                .and_then(|d| d.as_str())
                                .unwrap_or(&request.did)
                                .to_string(),
                            handle: error_json
                                .get("handle")
                                .and_then(|h| h.as_str())
                                .unwrap_or(&request.handle)
                                .to_string(),
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
                message: "Account already exists - resuming with provided credentials"
                    .to_string(),
                session,
                error_code,
                resumable,
            })
        } else {
            error!(
                "Account creation failed with status {}: {}",
                status, error_text
            );
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

/// Implementation of check_account_status functionality
/// Check account status
// NEWBOLD.md Step: goat account status (line 58)
// Implements: Checks migration progress including blobs, records, and validation status
#[instrument(skip(client), err)]
pub async fn check_account_status_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
) -> Result<ClientAccountStatusResponse, ClientError> {
    // NEWBOLD.md: com.atproto.server.checkAccountStatus for migration progress tracking
    let status_url = format!("{}/xrpc/com.atproto.server.checkAccountStatus", session.pds);

    let response = client
        .http_client
        .get(&status_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to check account status: {}", e),
        })?;

    if response.status().is_success() {
        let status_data: serde_json::Value =
            response
                .json()
                .await
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
        let error_text = response
            .text()
            .await
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

/// Implementation of refresh_session functionality
/// Refresh session tokens
#[instrument(skip(client), err)]
pub async fn refresh_session_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
) -> Result<ClientSessionCredentials, ClientError> {
    let refresh_url = format!("{}/xrpc/com.atproto.server.refreshSession", session.pds);

    let response = client
        .http_client
        .post(&refresh_url)
        .header("Authorization", format!("Bearer {}", session.refresh_jwt))
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to refresh session: {}", e),
        })?;

    if response.status().is_success() {
        let refresh_data: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse refresh response: {}", e),
                })?;

        let new_access_jwt = refresh_data["accessJwt"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let expires_at = if !new_access_jwt.is_empty() {
            JwtUtils::get_expiration(&new_access_jwt)
        } else {
            None
        };

        let mut updated_session = session.clone();
        updated_session.access_jwt = new_access_jwt;
        updated_session.refresh_jwt = refresh_data["refreshJwt"]
            .as_str()
            .unwrap_or(&session.refresh_jwt)
            .to_string();
        updated_session.expires_at = expires_at;

        info!(
            "Session refreshed successfully for DID: {}",
            updated_session.did
        );
        Ok(updated_session)
    } else {
        let error_text = response.text().await.unwrap_or_default();
        error!("Session refresh failed: {}", error_text);
        Err(ClientError::SessionExpired)
    }
}

/// Implementation of get_service_auth functionality
/// Generate service auth token for secure account creation on new PDS
/// This implements com.atproto.server.getServiceAuth
// NEWBOLD.md Step: goat account service-auth --lxm com.atproto.server.createAccount --aud $NEWPDSSERVICEDID --duration-sec 3600 (line 33)
// Implements: Generates service auth token for secure account creation on new PDS
#[instrument(skip(client), err)]
pub async fn get_service_auth_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
    aud: &str,         // Target PDS service DID
    lxm: Option<&str>, // Method restriction (e.g. com.atproto.server.createAccount)
    exp: Option<u64>,  // Expiration timestamp
) -> Result<ClientServiceAuthResponse, ClientError> {
    info!(
        "Generating service auth token for audience: {} (method: {:?})",
        aud, lxm
    );

    // NEWBOLD.md: com.atproto.server.getServiceAuth for secure migration auth token
    let mut service_auth_url =
        format!("{}/xrpc/com.atproto.server.getServiceAuth", session.pds);
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

    let response = client
        .http_client
        .get(&service_auth_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to call getServiceAuth: {}", e),
        })?;

    if response.status().is_success() {
        let auth_data: serde_json::Value =
            response
                .json()
                .await
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