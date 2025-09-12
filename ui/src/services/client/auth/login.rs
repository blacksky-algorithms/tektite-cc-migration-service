use anyhow::Result;
use serde_json::json;
use tracing::{error, info, instrument};

use crate::services::client::session::JwtUtils;
use crate::services::client::types::*;
use crate::services::client::{ClientError, PdsClient};

/// Core createSession implementation that all login functions use
#[instrument(skip(client, password, auth_factor_token), err)]
pub async fn create_session_core(
    client: &PdsClient,
    identifier: &str,
    password: &str,
    pds_url: &str,
    auth_factor_token: Option<&str>,
    allow_takendown: Option<bool>,
) -> Result<ClientLoginResponse, ClientError> {
    info!(
        "Creating session at PDS: {} for identifier: {}",
        pds_url, identifier
    );

    let session_url = format!("{}/xrpc/com.atproto.server.createSession", pds_url);

    // Build request body with all parameters
    let mut request_body = json!({
        "identifier": identifier,
        "password": password,
    });

    if let Some(token) = auth_factor_token {
        request_body["authFactorToken"] = serde_json::Value::String(token.to_string());
    }

    if let Some(allow) = allow_takendown {
        request_body["allowTakendown"] = serde_json::Value::Bool(allow);
    }

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

        // Check for active status and handle takendown accounts
        let is_active = session_data["active"].as_bool().unwrap_or(true);
        let status = session_data["status"].as_str();

        // If account is not active and we're not allowing takendown, fail
        if !is_active && allow_takendown != Some(true) {
            let status_msg = status.unwrap_or("unknown");
            return Ok(ClientLoginResponse {
                success: false,
                message: format!("Account is not active (status: {})", status_msg),
                did: Some(session_data["did"].as_str().unwrap_or_default().to_string()),
                session: None,
            });
        }

        // Extract tokens
        let access_jwt = session_data["accessJwt"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let refresh_jwt = session_data["refreshJwt"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        // Check if we got valid tokens
        if access_jwt.is_empty() || refresh_jwt.is_empty() {
            return Ok(ClientLoginResponse {
                success: false,
                message: "Login succeeded but no session tokens provided".to_string(),
                did: Some(session_data["did"].as_str().unwrap_or_default().to_string()),
                session: None,
            });
        }

        // Parse JWT for expiration
        let expires_at = JwtUtils::get_expiration(&access_jwt);

        let session = ClientSessionCredentials {
            did: session_data["did"].as_str().unwrap_or_default().to_string(),
            handle: session_data["handle"]
                .as_str()
                .unwrap_or(identifier)
                .to_string(),
            pds: pds_url.to_string(),
            access_jwt,
            refresh_jwt,
            expires_at,
        };

        info!(
            "Login successful for DID: {} (active: {}, status: {:?})",
            session.did, is_active, status
        );

        Ok(ClientLoginResponse {
            success: true,
            message: if !is_active {
                format!(
                    "Login successful but account is {}",
                    status.unwrap_or("inactive")
                )
            } else {
                "Login successful".to_string()
            },
            did: Some(session.did.clone()),
            session: Some(session),
        })
    } else {
        // Handle error responses
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|e| format!("Failed to read error response: {}", e));

        error!("Login failed with status {}: {}", status, error_text);

        // Try to parse structured error response
        if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
            let error_code = error_json["error"].as_str().unwrap_or("Unknown");
            let message = error_json["message"].as_str().unwrap_or(&error_text);

            // Check for specific error codes
            if error_code == "AuthFactorTokenRequired" {
                info!("Login requires 2FA for identifier: {}", identifier);
                return Ok(ClientLoginResponse {
                    success: false,
                    message: "Two-factor authentication required".to_string(),
                    did: None,
                    session: None,
                });
            }

            Ok(ClientLoginResponse {
                success: false,
                message: format!("{}: {}", error_code, message),
                did: None,
                session: None,
            })
        } else {
            Ok(ClientLoginResponse {
                success: false,
                message: format!("Login failed: {}", error_text),
                did: None,
                session: None,
            })
        }
    }
}

/// Main login implementation with handle/DID resolution
#[instrument(skip(client, password), err)]
pub async fn login_impl(
    client: &PdsClient,
    identifier: &str,
    password: &str,
) -> Result<ClientLoginResponse, ClientError> {
    info!("Starting login for identifier: {}", identifier);

    // First resolve identifier to DID and PDS URL if needed
    let (_did, pds_url) = if identifier.starts_with("did:") {
        // If it's already a DID, resolve to PDS
        let did = identifier.to_string();
        let pds_url = client.resolve_pds_from_did(&did).await?;
        (did, pds_url)
    } else {
        // If it's a handle, resolve to DID and PDS
        let resolved_did = client
            .identity_resolver
            .resolve_handle(identifier)
            .await
            .map_err(ClientError::ResolutionFailed)?;
        let pds_url = client.resolve_pds_from_did(&resolved_did).await?;
        (resolved_did, pds_url)
    };

    // Use the core implementation
    create_session_core(
        client, identifier, password, &pds_url, None, // No auth factor token
        None, // Default takendown behavior
    )
    .await
}

/// Full implementation with all createSession parameters
#[instrument(skip(client, password, auth_factor_token), err)]
pub async fn try_login_before_creation_full_impl(
    client: &PdsClient,
    handle: &str,
    password: &str,
    pds_url: &str,
    auth_factor_token: Option<&str>,
    allow_takendown: Option<bool>,
) -> Result<ClientLoginResponse, ClientError> {
    info!(
        "Attempting login to PDS: {} for handle: {}",
        pds_url, handle
    );

    // Use the core implementation directly
    create_session_core(
        client,
        handle,
        password,
        pds_url,
        auth_factor_token,
        allow_takendown,
    )
    .await
}

/// Simplified login for migration (no auth factor, default takendown)
#[instrument(skip(client, password), err)]
pub async fn try_login_before_creation_impl(
    client: &PdsClient,
    handle: &str,
    password: &str,
    pds_url: &str,
) -> Result<ClientLoginResponse, ClientError> {
    try_login_before_creation_full_impl(
        client, handle, password, pds_url, None, // No auth factor token
        None, // Default takendown behavior
    )
    .await
}
