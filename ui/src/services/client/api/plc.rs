//! PLC (Public Ledger of Credentials) and Identity operations for ATProto
//!
//! This module contains all identity-related operations including:
//! - PLC recommendations and token management
//! - PLC operation signing and submission
//! - Account activation and deactivation

use anyhow::Result;
use serde_json::json;
use tracing::{error, info, instrument};

use crate::services::client::errors::ClientError;
use crate::services::client::types::*;
use crate::services::client::PdsClient;

/// Get PLC recommendation from PDS
#[instrument(skip(client), err)]
pub async fn get_plc_recommendation_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
) -> Result<ClientPlcRecommendationResponse, ClientError> {
    info!("Getting PLC recommendation for DID: {}", session.did);

    let plc_url = format!(
        "{}/xrpc/com.atproto.identity.getRecommendedDidCredentials",
        session.pds
    );

    let response = client
        .http_client
        .get(&plc_url)
        .header("Authorization", format!("Bearer {}", session.access_jwt))
        .send()
        .await
        .map_err(|e| ClientError::NetworkError {
            message: format!("Failed to get PLC recommendation: {}", e),
        })?;

    if response.status().is_success() {
        let plc_data: serde_json::Value =
            response
                .json()
                .await
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
#[instrument(skip(client), err)]
pub async fn request_plc_token_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
) -> Result<ClientPlcTokenResponse, ClientError> {
    info!("Requesting PLC token for DID: {}", session.did);

    let token_url = format!(
        "{}/xrpc/com.atproto.identity.requestPlcOperationSignature",
        session.pds
    );

    let response = client
        .http_client
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
            message: "PLC token sent to email. Check your email for verification code."
                .to_string(),
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
#[instrument(skip(client, session, plc_unsigned, token), err)]
pub async fn sign_plc_operation_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
    plc_unsigned: String,
    token: String,
) -> Result<ClientPlcSignResponse, ClientError> {
    info!("Signing PLC operation for DID: {}", session.did);

    // Parse the unsigned PLC operation
    let plc_unsigned_value: serde_json::Value =
        serde_json::from_str(&plc_unsigned).map_err(|e| ClientError::NetworkError {
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

    let response = client
        .http_client
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
        let json_response: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| ClientError::NetworkError {
                    message: format!("Failed to parse sign response: {}", e),
                })?;

        info!("PLC operation signing response received");

        // Extract the 'operation' field from the response (matches Go implementation)
        let operation =
            json_response
                .get("operation")
                .ok_or_else(|| ClientError::NetworkError {
                    message: "No 'operation' field in response".to_string(),
                })?;

        // Convert signed operation to pretty JSON string
        let plc_signed =
            serde_json::to_string_pretty(operation).map_err(|e| ClientError::NetworkError {
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
#[instrument(skip(client, session, plc_signed), err)]
pub async fn submit_plc_operation_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
    plc_signed: String,
) -> Result<ClientPlcSubmitResponse, ClientError> {
    info!("Submitting PLC operation for DID: {}", session.did);

    // Parse the signed PLC operation
    let plc_signed_value: serde_json::Value =
        serde_json::from_str(&plc_signed).map_err(|e| ClientError::NetworkError {
            message: format!("Invalid signed PLC operation: {}", e),
        })?;

    // Construct the PLC submission endpoint URL
    // NEWBOLD.md: com.atproto.identity.submitPlcOperation for PLC operation submission
    let submit_url = format!(
        "{}/xrpc/com.atproto.identity.submitPlcOperation",
        session.pds
    );

    // Wrap signed operation in IdentitySubmitPlcOperation_Input structure (matches Go implementation)
    let submission_payload = json!({
        "operation": plc_signed_value
    });

    info!("Making PLC submission request to: {}", submit_url);

    let response = client
        .http_client
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
#[instrument(skip(client, session), err)]
pub async fn activate_account_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
) -> Result<ClientActivationResponse, ClientError> {
    info!("Activating account for DID: {}", session.did);

    // Construct the account activation endpoint URL
    // NEWBOLD.md: com.atproto.server.activateAccount for final account activation
    let activate_url = format!("{}/xrpc/com.atproto.server.activateAccount", session.pds);

    info!("Making account activation request to: {}", activate_url);

    // Make the request - this is a POST with no body (AT Protocol requirement)
    let response = client
        .http_client
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
#[instrument(skip(client, session), err)]
pub async fn deactivate_account_impl(
    client: &PdsClient,
    session: &ClientSessionCredentials,
) -> Result<ClientDeactivationResponse, ClientError> {
    info!("Deactivating account for DID: {}", session.did);

    // Construct the account deactivation endpoint URL
    // NEWBOLD.md: com.atproto.server.deactivateAccount for old account deactivation
    let deactivate_url = format!("{}/xrpc/com.atproto.server.deactivateAccount", session.pds);

    info!("Making account deactivation request to: {}", deactivate_url);

    // Make the request - this is a POST with empty body
    let response = client
        .http_client
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