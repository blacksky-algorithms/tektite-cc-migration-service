//! Account Operations for Migration
//!
//! This module handles account creation and status checking operations
//! for the migration process.

#[cfg(feature = "web")]
use crate::services::client::{
    ClientAccountStatusResponse, ClientCreateAccountRequest, ClientSessionCredentials,
    MigrationClient, PdsClient,
};

use crate::console_info;

/// NEWBOLD.md Step: goat account create --pds-host $NEWPDSHOST --existing-did $ACCOUNTDID --handle $NEWHANDLE --password $NEWPASSWORD --email $NEWEMAIL --invite-code $INVITECODE --service-auth $SERVICEAUTH (line 40-47)
/// Create account using client-side operations (with fallback resumption logic)
#[cfg(feature = "web")]
pub async fn create_account_client_side(
    migration_client: &MigrationClient,
    request: ClientCreateAccountRequest,
) -> Result<ClientSessionCredentials, String> {
    // Implements: goat account create --pds-host $NEWPDSHOST --existing-did $ACCOUNTDID --handle $NEWHANDLE --password $NEWPASSWORD --email $NEWEMAIL --invite-code $INVITECODE --service-auth $SERVICEAUTH
    match migration_client
        .create_account_new_pds(request.clone())
        .await
    {
        Ok(response) => {
            if response.success {
                response
                    .session
                    .ok_or_else(|| "No session returned from account creation".to_string())
            } else if response.resumable
                && response
                    .error_code
                    .as_ref()
                    .map(|c| c == "AlreadyExists")
                    .unwrap_or(false)
            {
                // For AlreadyExists during migration, according to AT Protocol spec,
                // the create account request with service auth token should succeed and return
                // session credentials for the existing account during migration scenarios
                console_info!("[Migration] Account creation failed with AlreadyExists - checking if response contains session for existing account");

                // Even if account creation "failed" due to AlreadyExists, check if we got session credentials
                if let Some(session) = response.session {
                    console_info!("[Migration] AlreadyExists response included session credentials for existing account");
                    Ok(session)
                } else {
                    // True failure - no session provided for existing account
                    Err(format!("Account creation failed with AlreadyExists but no session provided for resumption: {}", response.message))
                }
            } else {
                Err(response.message)
            }
        }
        Err(error) => Err(format!("Account creation failed: {}", error)),
    }
}

/// Check account status using client-side operations
#[cfg(feature = "web")]
pub async fn check_account_status_client_side(
    session: &ClientSessionCredentials,
) -> Result<ClientAccountStatusResponse, String> {
    let pds_client = PdsClient::new();
    match pds_client.check_account_status(session).await {
        Ok(response) => {
            if response.success {
                Ok(response)
            } else {
                Err(response.message)
            }
        }
        Err(error) => Err(format!("Account status check failed: {}", error)),
    }
}