use std::fmt;
use serde::{Deserialize, Serialize};

/// Client-side resolution errors (mirrors API ResolveError but for client use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolveError {
    /// HTTP request failed
    HttpRequestFailed {
        error: String,
    },
    /// JSON parsing error
    JsonParseError {
        error: String,
    },
    /// DNS query failed with status code
    DnsQueryFailed {
        status: u32,
        domain: String,
    },
    /// No DIDs found in DNS records
    NoDIDsFound {
        domain: String,
    },
    /// Multiple DIDs found (conflict)
    MultipleDIDsFound {
        domain: String,
        dids: Vec<String>,
    },
    /// Conflicting DIDs between DNS and HTTP
    ConflictingDIDsFound {
        handle: String,
        dids: Vec<String>,
    },
    /// Invalid DID format
    InvalidDidFormat {
        value: String,
        source: String,
    },
    /// All DNS endpoints failed
    AllDnsEndpointsFailed {
        domain: String,
    },
    /// Network timeout
    Timeout {
        operation: String,
    },
    /// Invalid handle format
    InvalidHandle {
        handle: String,
    },
    /// PDS endpoint not found
    PdsEndpointNotFound {
        did: String,
    },
    /// DID document resolution failed
    DidDocumentResolutionFailed {
        did: String,
        error: String,
    },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        }
    }
}

impl std::error::Error for ResolveError {}

/// Client-side operation errors
#[derive(Debug, Clone)]
pub enum ClientError {
    /// Resolution failed
    ResolutionFailed(ResolveError),
    /// Authentication failed
    AuthenticationFailed {
        message: String,
    },
    /// Network error
    NetworkError {
        message: String,
    },
    /// Serialization error
    SerializationError {
        message: String,
    },
    /// Storage error
    StorageError {
        message: String,
    },
    /// Invalid credentials
    InvalidCredentials,
    /// Session expired
    SessionExpired,
    /// PDS operation failed
    PdsOperationFailed {
        operation: String,
        message: String,
    },
    /// Invalid response format
    InvalidResponse {
        expected: String,
        got: String,
    },
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::ResolutionFailed(err) => {
                write!(f, "Resolution failed: {}", err)
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
            ClientError::StorageError { message } => {
                write!(f, "Storage error: {}", message)
            }
            ClientError::InvalidCredentials => {
                write!(f, "Invalid credentials")
            }
            ClientError::SessionExpired => {
                write!(f, "Session expired")
            }
            ClientError::PdsOperationFailed { operation, message } => {
                write!(f, "PDS operation '{}' failed: {}", operation, message)
            }
            ClientError::InvalidResponse { expected, got } => {
                write!(f, "Invalid response format: expected {}, got {}", expected, got)
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

/// Result type for client operations
pub type ClientResult<T> = Result<T, ClientError>;