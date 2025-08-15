use serde::{Deserialize, Serialize};
use std::fmt;

/// Client-side resolution errors (mirrors API ResolveError but for client use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolveError {
    /// SSL/TLS Protocol Error
    SslProtocolError { url: String },
    /// Origin resolution failed
    OriginResolutionFailed { error: String },
    /// HTTP request failed
    HttpRequestFailed { error: String },
    /// JSON parsing error
    JsonParseError { error: String },
    /// DNS query failed with status code
    DnsQueryFailed { status: u32, domain: String },
    /// No DIDs found in DNS records
    NoDIDsFound { domain: String },
    /// Multiple DIDs found (conflict)
    MultipleDIDsFound { domain: String, dids: Vec<String> },
    /// Conflicting DIDs between DNS and HTTP
    ConflictingDIDsFound { handle: String, dids: Vec<String> },
    /// Invalid DID format
    InvalidDidFormat { value: String, source: String },
    /// All DNS endpoints failed
    AllDnsEndpointsFailed { domain: String },
    /// Network timeout
    Timeout { operation: String },
    /// Invalid handle format
    InvalidHandle { handle: String },
    /// PDS endpoint not found
    PdsEndpointNotFound { did: String },
    /// DID document resolution failed
    DidDocumentResolutionFailed { did: String, error: String },
    /// Unsupported DID method
    UnsupportedDidMethod { did: String },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::SslProtocolError { url } => {
                write!(f, "SSL Protocol Error: {}", url)
            }
            ResolveError::OriginResolutionFailed { error } => {
                write!(f, "Window Origin Resolution failed: {}", error)
            }
            ResolveError::HttpRequestFailed { error } => {
                write!(f, "HTTP request failed: {}", error)
            }
            ResolveError::JsonParseError { error } => {
                write!(f, "JSON parse error: {}", error)
            }
            ResolveError::DnsQueryFailed { status, domain } => {
                write!(f, "DNS query failed for {} with status {}", domain, status)
            }
            ResolveError::NoDIDsFound { domain } => {
                write!(f, "No DIDs found for domain {}", domain)
            }
            ResolveError::MultipleDIDsFound { domain, dids } => {
                write!(f, "Multiple DIDs found for {}: {:?}", domain, dids)
            }
            ResolveError::ConflictingDIDsFound { handle, dids } => {
                write!(f, "Conflicting DIDs for handle {}: {:?}", handle, dids)
            }
            ResolveError::InvalidDidFormat { value, source } => {
                write!(f, "Invalid DID format '{}' from {}", value, source)
            }
            ResolveError::AllDnsEndpointsFailed { domain } => {
                write!(f, "All DNS endpoints failed for {}", domain)
            }
            ResolveError::Timeout { operation } => {
                write!(f, "Operation timed out: {}", operation)
            }
            ResolveError::InvalidHandle { handle } => {
                write!(f, "Invalid handle format: {}", handle)
            }
            ResolveError::PdsEndpointNotFound { did } => {
                write!(f, "PDS endpoint not found for DID: {}", did)
            }
            ResolveError::DidDocumentResolutionFailed { did, error } => {
                write!(f, "DID document resolution failed for {}: {}", did, error)
            }
            ResolveError::UnsupportedDidMethod { did } => {
                write!(f, "Unsupported DID method: {}", did)
            }
        }
    }
}

impl std::error::Error for ResolveError {}

/// AT Protocol error from server response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ATProtocolError {
    pub error: String,   // AT Protocol error code (e.g. "InvalidRequest")
    pub message: String, // Human readable error message
}

/// Rate limiting information from response headers
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    pub limit: Option<i32>,     // ratelimit-limit header
    pub reset: Option<u64>,     // ratelimit-reset header (unix timestamp)
    pub policy: Option<String>, // ratelimit-policy header
}

impl RateLimitInfo {
    /// Create from reqwest response headers
    #[cfg(target_arch = "wasm32")]
    pub fn from_response(response: &reqwest::Response) -> Option<Self> {
        let headers = response.headers();

        let limit = headers
            .get("ratelimit-limit")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok());

        let reset = headers
            .get("ratelimit-reset")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok());

        let policy = headers
            .get("ratelimit-policy")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        if limit.is_some() || reset.is_some() || policy.is_some() {
            Some(Self {
                limit,
                reset,
                policy,
            })
        } else {
            None
        }
    }

    /// Calculate seconds until rate limit resets
    pub fn retry_after_seconds(&self) -> Option<u64> {
        if let Some(reset) = self.reset {
            use js_sys::Date;
            let now = (Date::now() / 1000.0) as u64;
            if reset > now {
                Some(reset - now)
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Client-side operation errors
#[derive(Debug, Clone)]
pub enum ClientError {
    /// Resolution failed
    ResolutionFailed(ResolveError),
    /// AT Protocol error from server
    ATProtocolError {
        status_code: u16,
        error: ATProtocolError,
    },
    /// Rate limited by server
    RateLimited { info: RateLimitInfo },
    /// Authentication failed (401)
    AuthenticationFailed { message: String },
    /// Network error
    NetworkError { message: String },
    /// Serialization error
    SerializationError { message: String },
    /// General API error
    ApiError { message: String },
    /// Storage error
    StorageError { message: String },
    /// Invalid credentials (403)
    InvalidCredentials,
    /// Session expired
    SessionExpired,
    /// Resource not found (404)
    ResourceNotFound { resource: String },
    /// Server error (5xx)
    ServerError { status_code: u16, message: String },
    /// PDS operation failed
    PdsOperationFailed { operation: String, message: String },
    /// Invalid response format
    InvalidResponse { expected: String, got: String },
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::ResolutionFailed(err) => {
                write!(f, "Resolution failed: {}", err)
            }
            ClientError::ATProtocolError { status_code, error } => {
                write!(
                    f,
                    "AT Protocol error {}: {} ({})",
                    status_code, error.error, error.message
                )
            }
            ClientError::RateLimited { info } => {
                if let Some(retry_after) = info.retry_after_seconds() {
                    write!(f, "Rate limited: retry after {} seconds", retry_after)
                } else {
                    write!(f, "Rate limited")
                }
            }
            ClientError::AuthenticationFailed { message } => {
                write!(f, "Authentication failed: {}", message)
            }
            ClientError::NetworkError { message } => {
                write!(f, "Network error: {}", message)
            }
            ClientError::SerializationError { message } => {
                write!(f, "Serialization error: {}", message)
            }
            ClientError::ApiError { message } => {
                write!(f, "API error: {}", message)
            }
            ClientError::StorageError { message } => {
                write!(f, "Storage error: {}", message)
            }
            ClientError::InvalidCredentials => {
                write!(f, "Invalid credentials")
            }
            ClientError::SessionExpired => {
                write!(f, "Session expired")
            }
            ClientError::ResourceNotFound { resource } => {
                write!(f, "Resource not found: {}", resource)
            }
            ClientError::ServerError {
                status_code,
                message,
            } => {
                write!(f, "Server error {}: {}", status_code, message)
            }
            ClientError::PdsOperationFailed { operation, message } => {
                write!(f, "PDS operation '{}' failed: {}", operation, message)
            }
            ClientError::InvalidResponse { expected, got } => {
                write!(
                    f,
                    "Invalid response format: expected {}, got {}",
                    expected, got
                )
            }
        }
    }
}

impl std::error::Error for ClientError {}

impl From<ResolveError> for ClientError {
    fn from(err: ResolveError) -> Self {
        ClientError::ResolutionFailed(err)
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(err: serde_json::Error) -> Self {
        ClientError::SerializationError {
            message: err.to_string(),
        }
    }
}

/// Helper to create ClientError from HTTP response
pub async fn error_from_response(response: reqwest::Response, operation: &str) -> ClientError {
    let status_code = response.status().as_u16();

    // Check for rate limiting first
    if status_code == 429 {
        if let Some(rate_limit_info) = RateLimitInfo::from_response(&response) {
            return ClientError::RateLimited {
                info: rate_limit_info,
            };
        }
    }

    // Try to parse AT Protocol error response
    let response_text = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            return ClientError::NetworkError {
                message: format!("Failed to read error response: {}", e),
            };
        }
    };

    // Try to parse as AT Protocol error
    if let Ok(at_error) = serde_json::from_str::<ATProtocolError>(&response_text) {
        return ClientError::ATProtocolError {
            status_code,
            error: at_error,
        };
    }

    // Fall back to status code categorization
    match status_code {
        401 => ClientError::AuthenticationFailed {
            message: format!("{}: {}", operation, response_text),
        },
        403 => ClientError::InvalidCredentials,
        404 => ClientError::ResourceNotFound {
            resource: operation.to_string(),
        },
        429 => ClientError::RateLimited {
            info: RateLimitInfo {
                limit: None,
                reset: None,
                policy: None,
            },
        },
        500..=599 => ClientError::ServerError {
            status_code,
            message: response_text,
        },
        _ => ClientError::PdsOperationFailed {
            operation: operation.to_string(),
            message: format!("HTTP {}: {}", status_code, response_text),
        },
    }
}

/// Result type for client operations
pub type ClientResult<T> = Result<T, ClientError>;
