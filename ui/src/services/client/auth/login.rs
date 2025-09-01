use anyhow::Result;
use serde_json::json;
use tracing::{error, info};

use crate::console_debug;
use crate::services::client::session::JwtUtils;
use crate::services::client::types::*;
use crate::services::client::{ClientError, PdsClient};

/// Implementation of login functionality
pub async fn login_impl(
    client: &PdsClient,
    identifier: &str,
    password: &str,
) -> Result<ClientLoginResponse, ClientError> {
    info!("Starting login for identifier: {}", identifier);

    // First resolve identifier to DID if it's a handle
    let (did, pds_url) = if identifier.starts_with("did:") {
        // If it's already a DID, we need to resolve the DID document to find PDS
        let did = identifier.to_string();
        let pds_url = client.resolve_pds_from_did(&did).await?;
        (did, pds_url)
    } else {
        // If it's a handle, resolve to DID first
        let resolved_did = client
            .identity_resolver
            .resolve_handle(identifier)
            .await
            .map_err(ClientError::ResolutionFailed)?;

        let pds_url = client.resolve_pds_from_did(&resolved_did).await?;
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

    let response = client
        .http_client
        .post(&session_url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to call createSession: {}", e),
        })?;

    if response.status().is_success() {
        let session_data: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse response: {}", e),
                })?;

        // Parse JWT to get expiration
        let access_jwt = session_data["accessJwt"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let expires_at = if !access_jwt.is_empty() {
            JwtUtils::get_expiration(&access_jwt)
        } else {
            None
        };

        let session = ClientSessionCredentials {
            did: session_data["did"].as_str().unwrap_or(&did).to_string(),
            handle: session_data["handle"]
                .as_str()
                .unwrap_or(identifier)
                .to_string(),
            pds: pds_url,
            access_jwt,
            refresh_jwt: session_data["refreshJwt"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
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
        let error_text = response
            .text()
            .await
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

/// Implementation of try_login_before_creation functionality  
pub async fn try_login_before_creation_impl(
    client: &PdsClient,
    identifier: &str,
    password: &str,
    pds_url: &str,
) -> Result<ClientLoginResponse, ClientError> {
    info!(
        "Attempting login before account creation for identifier: {}",
        identifier
    );

    // Call ATProto createSession to check if account already exists
    let session_url = format!("{}/xrpc/com.atproto.server.createSession", pds_url);
    let request_body = json!({
        "identifier": identifier,
        "password": password
    });

    console_debug!("Calling createSession at: {}", session_url);

    let response = client
        .http_client
        .post(&session_url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to call createSession: {}", e),
        })?;

    if response.status().is_success() {
        // Account exists and login succeeded
        let session_data: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse response: {}", e),
                })?;

        // Parse JWT to get expiration
        let access_jwt = session_data["accessJwt"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let expires_at = if !access_jwt.is_empty() {
            JwtUtils::get_expiration(&access_jwt)
        } else {
            None
        };

        let session = ClientSessionCredentials {
            did: session_data["did"]
                .as_str()
                .unwrap_or(identifier)
                .to_string(),
            handle: session_data["handle"]
                .as_str()
                .unwrap_or(identifier)
                .to_string(),
            pds: pds_url.to_string(),
            access_jwt,
            refresh_jwt: session_data["refreshJwt"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            expires_at,
        };

        info!("Account exists, login successful for DID: {}", session.did);
        Ok(ClientLoginResponse {
            success: true,
            message: "Account exists - login successful".to_string(),
            did: Some(session.did.clone()),
            session: Some(session),
        })
    } else if response.status().as_u16() == 401 {
        // Account doesn't exist or wrong credentials - this is expected for new account creation flow
        info!("Account doesn't exist on target PDS - can proceed with creation");
        Ok(ClientLoginResponse {
            success: false,
            message: "Account doesn't exist - can create new account".to_string(),
            did: None,
            session: None,
        })
    } else {
        // Other error
        let status = response.status();
        let error_text = response
            .text()
            .await
            .map_err(|e| ClientError::NetworkError {
                message: format!("Failed to read error response: {}", e),
            })?;

        error!(
            "Unexpected error during login check - status {}: {}",
            status, error_text
        );
        Ok(ClientLoginResponse {
            success: false,
            message: format!("Error checking account existence: {}", error_text),
            did: None,
            session: None,
        })
    }
}
