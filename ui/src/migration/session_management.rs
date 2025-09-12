//! Session Management for Migration
//!
//! This module handles conversion between different session credential formats
//! used throughout the migration process.

#[cfg(feature = "web")]
use crate::services::client::ClientSessionCredentials;

use crate::migration::types::SessionCredentials;

/// Convert client session to API session format for compatibility
impl From<&ClientSessionCredentials> for SessionCredentials {
    fn from(client_session: &ClientSessionCredentials) -> Self {
        SessionCredentials {
            did: client_session.did.clone(),
            handle: client_session.handle.clone(),
            pds: client_session.pds.clone(),
            access_jwt: client_session.access_jwt.clone(),
            refresh_jwt: client_session.refresh_jwt.clone(),
        }
    }
}

/// Convert API session to Client session format
#[cfg(feature = "web")]
impl From<SessionCredentials> for ClientSessionCredentials {
    fn from(api_session: SessionCredentials) -> Self {
        ClientSessionCredentials {
            did: api_session.did,
            handle: api_session.handle,
            pds: api_session.pds,
            access_jwt: api_session.access_jwt,
            refresh_jwt: api_session.refresh_jwt,
            expires_at: None, // Will be parsed from JWT if available
        }
    }
}

/// Convert session reference to Client session format
#[cfg(feature = "web")]
impl From<&SessionCredentials> for ClientSessionCredentials {
    fn from(session: &SessionCredentials) -> Self {
        ClientSessionCredentials {
            did: session.did.clone(),
            handle: session.handle.clone(),
            pds: session.pds.clone(),
            access_jwt: session.access_jwt.clone(),
            refresh_jwt: session.refresh_jwt.clone(),
            expires_at: None, // Will be parsed from JWT if available
        }
    }
}
