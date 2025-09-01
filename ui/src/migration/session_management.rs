//! Session Management for Migration
//!
//! This module handles conversion between different session credential formats
//! used throughout the migration process.

#[cfg(feature = "web")]
use crate::services::client::ClientSessionCredentials;

use crate::migration::types::SessionCredentials;

/// Convert client session to API session format for compatibility
pub fn convert_to_api_session(client_session: &ClientSessionCredentials) -> SessionCredentials {
    SessionCredentials {
        did: client_session.did.clone(),
        handle: client_session.handle.clone(),
        pds: client_session.pds.clone(),
        access_jwt: client_session.access_jwt.clone(),
        refresh_jwt: client_session.refresh_jwt.clone(),
    }
}

/// Convert api:: to Client
#[cfg(feature = "web")]
pub fn convert_from_api_session(api_session: SessionCredentials) -> ClientSessionCredentials {
    ClientSessionCredentials {
        did: api_session.did.clone(),
        handle: api_session.handle.clone(),
        pds: api_session.pds.clone(),
        access_jwt: api_session.access_jwt.clone(),
        refresh_jwt: api_session.refresh_jwt.clone(),
        expires_at: None, // Will be parsed from JWT if available
    }
}

/// Convert local  to Client
#[cfg(feature = "web")]
pub fn convert_session_to_client(session: &SessionCredentials) -> ClientSessionCredentials {
    ClientSessionCredentials {
        did: session.did.clone(),
        handle: session.handle.clone(),
        pds: session.pds.clone(),
        access_jwt: session.access_jwt.clone(),
        refresh_jwt: session.refresh_jwt.clone(),
        expires_at: None, // Will be parsed from JWT if available
    }
}
