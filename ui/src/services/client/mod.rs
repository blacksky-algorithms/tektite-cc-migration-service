// Client-side functionality for ATProto migration service
//
// This module provides a complete client-side implementation for:
// - DNS-over-HTTPS handle resolution
// - PDS authentication and operations
// - Session management with secure storage
// - Identity resolution and validation
//
// This replaces server-side functions to create a fully browser-based migration service.

pub mod api;
pub mod auth;
pub mod dns_over_https;
pub mod errors;
pub mod identity_resolver;
pub mod pds_client;
pub mod session;
pub mod session_refresh;
pub mod types;

#[cfg(test)]
pub mod cursor_test;

// Re-export core types for easy access
pub use types::{
    CachedDnsResponse,
    ClientAccountStatusResponse,
    ClientBlobExportResponse,
    ClientBlobUploadResponse,
    ClientCreateAccountRequest,
    ClientCreateAccountResponse,
    ClientLoginRequest,
    ClientLoginResponse,
    // Blob types
    ClientMissingBlob,
    ClientMissingBlobsResponse,
    ClientPdsProvider,
    // PLC types
    ClientPlcRecommendationResponse,
    ClientPlcSignResponse,
    ClientPlcSubmitResponse,
    ClientPlcTokenResponse,
    // Preferences types
    ClientPreferencesExportResponse,
    ClientPreferencesImportResponse,
    // Repository types
    ClientRepoExportResponse,
    ClientRepoImportResponse,
    // Service Auth types
    ClientServiceAuthRequest,
    ClientServiceAuthResponse,
    ClientSessionCredentials,
    // Sync types
    ClientSyncListBlobsResponse,
    // DNS types
    CloudflareDoHResponse,
    DidDocument,
    DnsAnswer,
    DnsQuestion,
};

// Re-export error types
pub use errors::{ClientError, ClientResult, ResolveError};

// Re-export main client classes
pub use dns_over_https::{DnsOverHttpsResolver, DnsResolver};
pub use identity_resolver::{
    determine_pds_provider_client_side, resolve_handle_client_side, resolve_handle_dns_doh,
    resolve_handle_http, WebIdentityResolver,
};
pub use pds_client::PdsClient;
pub use session::{JwtUtils, MigrationSessionManager, SessionManager};
pub use session_refresh::RefreshableSessionProvider;

/// Convenience factory for creating a complete client setup
pub struct MigrationClient {
    pub identity_resolver: WebIdentityResolver,
    pub pds_client: PdsClient,
    pub session_manager: MigrationSessionManager,
}

impl MigrationClient {
    /// Create a new migration client with all components
    pub fn new() -> Self {
        Self {
            identity_resolver: WebIdentityResolver::new(),
            pds_client: PdsClient::new(),
            session_manager: MigrationSessionManager::new(),
        }
    }

    /// Login to old PDS and store session
    pub async fn login_old_pds(
        &self,
        identifier: &str,
        password: &str,
    ) -> ClientResult<ClientLoginResponse> {
        let response = self.pds_client.login(identifier, password).await?;

        if response.success {
            if let Some(ref session) = response.session {
                self.session_manager.store_old_session(session)?;
            }
        }

        Ok(response)
    }

    /// Create account on new PDS and store session
    pub async fn create_account_new_pds(
        &self,
        request: ClientCreateAccountRequest,
    ) -> ClientResult<ClientCreateAccountResponse> {
        let response = self.pds_client.create_account(request).await?;

        if response.success {
            if let Some(ref session) = response.session {
                self.session_manager.store_new_session(session)?;
            }
        }

        Ok(response)
    }

    /// Resolve handle using DNS-over-HTTPS
    pub async fn resolve_handle(&self, handle: &str) -> ClientResult<String> {
        self.identity_resolver
            .resolve_handle(handle)
            .await
            .map_err(ClientError::from)
    }

    /// Determine PDS provider for handle or DID
    pub async fn determine_provider(&self, handle_or_did: &str) -> ClientPdsProvider {
        self.identity_resolver
            .determine_provider(handle_or_did)
            .await
    }

    /// Get stored old PDS session
    pub fn get_old_session(&self) -> ClientResult<Option<ClientSessionCredentials>> {
        self.session_manager.get_old_session()
    }

    /// Get stored new PDS session
    pub fn get_new_session(&self) -> ClientResult<Option<ClientSessionCredentials>> {
        self.session_manager.get_new_session()
    }

    /// Check if migration can continue (both sessions valid)
    pub fn can_continue_migration(&self) -> ClientResult<bool> {
        self.session_manager.can_continue_migration()
    }

    /// Clear all stored sessions
    pub fn clear_all_sessions(&self) -> ClientResult<()> {
        self.session_manager.clear_all_sessions()
    }
}

impl Default for MigrationClient {
    fn default() -> Self {
        Self::new()
    }
}

// Convenience functions that match the API module exports for easy migration
pub mod compat {
    //! Compatibility functions that mirror the API module structure
    //! to ease migration from server-side to client-side operations

    use super::*;
    use std::sync::{Arc, OnceLock};

    // Singleton instances to avoid repeated allocations in WASM32 linear memory
    static IDENTITY_RESOLVER: OnceLock<Arc<WebIdentityResolver>> = OnceLock::new();
    static PDS_CLIENT: OnceLock<Arc<PdsClient>> = OnceLock::new();

    /// Get or initialize the shared identity resolver
    fn get_identity_resolver() -> Arc<WebIdentityResolver> {
        Arc::clone(IDENTITY_RESOLVER.get_or_init(|| Arc::new(WebIdentityResolver::new())))
    }

    /// Get or initialize the shared PDS client
    fn get_pds_client() -> Arc<PdsClient> {
        Arc::clone(PDS_CLIENT.get_or_init(|| Arc::new(PdsClient::new())))
    }

    /// Resolve handle using client-side DNS-over-HTTPS (replaces api::resolve_handle_shared)
    pub async fn resolve_handle_shared(handle: String) -> ClientResult<ClientPdsProvider> {
        let client = get_identity_resolver();
        let provider = client.determine_provider(&handle).await;
        Ok(provider)
    }

    /// Login to PDS using client-side operations (replaces api::pds_login)
    pub async fn pds_login(form: ClientLoginRequest) -> ClientResult<ClientLoginResponse> {
        let client = get_pds_client();
        client.login(&form.identifier, &form.password).await
    }

    /// Create account using client-side operations (replaces api::create_account)
    pub async fn create_account(
        form: ClientCreateAccountRequest,
    ) -> ClientResult<ClientCreateAccountResponse> {
        let client = get_pds_client();
        client.create_account(form).await
    }

    /// Check account status using client-side operations (replaces api::check_account_status)
    pub async fn check_account_status(
        session: ClientSessionCredentials,
    ) -> ClientResult<ClientAccountStatusResponse> {
        let client = get_pds_client();
        client.check_account_status(&session).await
    }

    /// Describe server using client-side operations (replaces api::describe_server)
    pub async fn describe_server(pds_url: String) -> ClientResult<serde_json::Value> {
        let client = get_pds_client();
        client.describe_server(&pds_url).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migration_client_integration() {
        let client = MigrationClient::new();

        // Test handle resolution
        let result = client.resolve_handle("rudyfraser.com").await;
        match result {
            Ok(did) => {
                assert!(did.starts_with("did:"));
                assert_eq!(did, "did:plc:w4xbfzo7kqfes5zb7r6qv3rw");
            }
            Err(e) => {
                // Network issues in test environment are acceptable
                println!("Handle resolution failed (expected in test): {}", e);
            }
        }

        // Test provider determination
        let provider = client.determine_provider("user.bsky.social").await;
        assert_eq!(provider, ClientPdsProvider::Bluesky);
    }

    #[test]
    fn test_client_validation() {
        let client = MigrationClient::new();

        // Test handle validation
        assert!(client.identity_resolver.is_valid_handle("user.example.com"));
        assert!(!client.identity_resolver.is_valid_handle("invalid"));

        // Test DID validation
        assert!(client.identity_resolver.is_valid_did("did:plc:abcd1234"));
        assert!(!client.identity_resolver.is_valid_did("not-a-did"));
    }
}
