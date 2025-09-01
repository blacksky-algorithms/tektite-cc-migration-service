use cid::Cid;
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use js_sys;

/// Get current time in seconds since UNIX epoch (WASM compatible)
#[cfg(target_arch = "wasm32")]
fn current_time_secs() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

/// DNS-over-HTTPS response structure matching Cloudflare's API
#[derive(Deserialize, Debug, Clone)]
pub struct CloudflareDoHResponse {
    #[serde(rename = "Status")]
    pub status: u32,
    #[serde(rename = "TC")]
    pub tc: bool,
    #[serde(rename = "RD")]
    pub rd: bool,
    #[serde(rename = "RA")]
    pub ra: bool,
    #[serde(rename = "AD")]
    pub ad: bool,
    #[serde(rename = "CD")]
    pub cd: bool,
    #[serde(rename = "Question")]
    pub question: Vec<DnsQuestion>,
    #[serde(rename = "Answer")]
    pub answer: Option<Vec<DnsAnswer>>,
}

/// DNS question structure
#[derive(Deserialize, Debug, Clone)]
pub struct DnsQuestion {
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: u16,
}

/// DNS answer structure
#[derive(Deserialize, Debug, Clone)]
pub struct DnsAnswer {
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: u16,
    #[serde(rename = "TTL")]
    pub ttl: u32,
    pub data: String,
}

/// Cached DNS response with expiration
#[derive(Debug, Clone)]
pub struct CachedDnsResponse {
    pub records: Vec<String>,
    pub expires_at: u64, // Milliseconds since UNIX epoch for WASM compatibility
}

/// Client-side session credentials (mirrors API SessionCredentials)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientSessionCredentials {
    pub did: String,
    pub handle: String,
    pub pds: String,
    #[serde(rename = "accessJwt")]
    pub access_jwt: String,
    #[serde(rename = "refreshJwt")]
    pub refresh_jwt: String,
    pub expires_at: Option<u64>,
}

impl ClientSessionCredentials {
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = current_time_secs();
            now >= expires_at
        } else {
            false
        }
    }

    pub fn needs_refresh(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = current_time_secs();
            // Refresh if within 5 minutes of expiry
            now >= (expires_at - 300)
        } else {
            false
        }
    }
}

/// Client-side login request
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientLoginRequest {
    pub identifier: String,
    pub password: String,
}

/// Client-side login response (mirrors API response structure)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientLoginResponse {
    pub success: bool,
    pub message: String,
    pub did: Option<String>,
    pub session: Option<ClientSessionCredentials>,
}

/// Account creation request
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientCreateAccountRequest {
    pub did: String,
    pub handle: String,
    pub password: String,
    pub email: String,
    #[serde(rename = "inviteCode")]
    pub invite_code: Option<String>,
    #[serde(skip)] // Not part of AT Protocol API - used for Authorization header
    pub service_auth_token: Option<String>, // For creating accounts with existing DIDs
}

/// Account creation response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientCreateAccountResponse {
    pub success: bool,
    pub message: String,
    pub session: Option<ClientSessionCredentials>,
    pub error_code: Option<String>, // AT Protocol error codes like "AlreadyExists"
    pub resumable: bool,            // Whether migration can be resumed from this error
}

/// PDS provider information (mirrors API PdsProvider)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientPdsProvider {
    None,
    Bluesky,
    BlackSky,
    Other(String),
}

/// DID Document structure (simplified for client use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidDocument {
    pub id: String,
    pub service: Vec<DidService>,
}

/// DID Service entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidService {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
    #[serde(rename = "serviceEndpoint")]
    pub service_endpoint: String,
}

impl DidDocument {
    /// Extract PDS endpoints from service array
    pub fn pds_endpoints(&self) -> Vec<String> {
        self.service
            .iter()
            .filter(|service| service.service_type == "AtprotoPersonalDataServer")
            .map(|service| service.service_endpoint.clone())
            .collect()
    }
}

/// Repository export response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientRepoExportResponse {
    pub success: bool,
    pub message: String,
    pub car_data: Option<Vec<u8>>,
    pub car_size: Option<u64>,
}

/// Repository import response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientRepoImportResponse {
    pub success: bool,
    pub message: String,
}

/// Missing blob information
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientMissingBlob {
    pub cid: Cid,
    #[serde(rename = "recordUri")]
    pub record_uri: String,
}

/// Missing blobs response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientMissingBlobsResponse {
    pub success: bool,
    pub message: String,
    pub missing_blobs: Option<Vec<ClientMissingBlob>>,
    pub cursor: Option<String>,
}

/// Blob export response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientBlobExportResponse {
    pub success: bool,
    pub message: String,
    pub blob_data: Option<Vec<u8>>,
}

/// Blob upload response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientBlobUploadResponse {
    pub success: bool,
    pub message: String,
}

/// Blob streaming export response (no blob_data field for memory efficiency)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientBlobStreamResponse {
    pub success: bool,
    pub message: String,
    pub blob_size: Option<u64>,
}

/// Preferences export response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientPreferencesExportResponse {
    pub success: bool,
    pub message: String,
    pub preferences_json: Option<String>,
}

/// Preferences import response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientPreferencesImportResponse {
    pub success: bool,
    pub message: String,
}

/// PLC recommendation response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientPlcRecommendationResponse {
    pub success: bool,
    pub message: String,
    pub plc_unsigned: Option<String>,
}

/// PLC token response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientPlcTokenResponse {
    pub success: bool,
    pub message: String,
}

/// PLC sign response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientPlcSignResponse {
    pub success: bool,
    pub message: String,
    pub plc_signed: Option<String>,
}

/// PLC submit response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientPlcSubmitResponse {
    pub success: bool,
    pub message: String,
}

/// Account activation response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientActivationResponse {
    pub success: bool,
    pub message: String,
}

/// Account deactivation response
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientDeactivationResponse {
    pub success: bool,
    pub message: String,
}

/// Account status response
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ClientAccountStatusResponse {
    pub success: bool,
    pub message: String,
    pub activated: Option<bool>,
    #[serde(rename = "expectedBlobs")]
    pub expected_blobs: Option<i64>,
    #[serde(rename = "importedBlobs")]
    pub imported_blobs: Option<i64>,
    #[serde(rename = "indexedRecords")]
    pub indexed_records: Option<i64>,
    #[serde(rename = "privateStateValues")]
    pub private_state_values: Option<i64>,
    #[serde(rename = "repoBlocks")]
    pub repo_blocks: Option<i64>,
    #[serde(rename = "repoCommit")]
    pub repo_commit: Option<String>,
    #[serde(rename = "repoRev")]
    pub repo_rev: Option<String>,
    #[serde(rename = "validDid")]
    pub valid_did: Option<bool>,
}

/// Service auth request for secure account creation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientServiceAuthRequest {
    pub aud: String,         // Target PDS service DID (audience)
    pub lxm: Option<String>, // Method restriction (e.g. "com.atproto.server.createAccount")
    #[serde(rename = "exp")]
    pub expires_at: Option<u64>, // Expiration timestamp (unix seconds)
}

impl ClientServiceAuthRequest {
    /// Create a new service auth request for account creation
    pub fn for_account_creation(target_pds_did: &str, expires_in_seconds: Option<u64>) -> Self {
        let expires_at = if let Some(duration) = expires_in_seconds {
            Some(current_time_secs() + duration)
        } else {
            Some(current_time_secs() + 3600) // Default 1 hour
        };

        Self {
            aud: target_pds_did.to_string(),
            lxm: Some("com.atproto.server.createAccount".to_string()),
            expires_at,
        }
    }

    /// Create a generic service auth request
    pub fn new(
        target_pds_did: &str,
        method: Option<&str>,
        expires_in_seconds: Option<u64>,
    ) -> Self {
        let expires_at = expires_in_seconds.map(|duration| current_time_secs() + duration);

        Self {
            aud: target_pds_did.to_string(),
            lxm: method.map(|s| s.to_string()),
            expires_at,
        }
    }
}

/// Service auth response containing the JWT token
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientServiceAuthResponse {
    pub success: bool,
    pub message: String,
    pub token: Option<String>, // The service auth JWT token
}

/// Sync list blobs response (matches Go SyncListBlobs_Output)
/// This is used by com.atproto.sync.listBlobs for full blob enumeration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientSyncListBlobsResponse {
    pub success: bool,
    pub message: String,
    pub cids: Option<Vec<Cid>>, // CID list with proper CID type validation
    pub cursor: Option<String>, // Pagination cursor (matches Go *string)
}
