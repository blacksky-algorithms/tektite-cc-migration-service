//! Automatic session token refresh for long-running operations
//!
//! This module provides a wrapper around ClientSessionCredentials that automatically
//! refreshes access tokens when they're close to expiration, preventing session
//! expiration during long-running migrations (like blob uploads that take ~60 minutes).

use super::auth::account::refresh_session_impl;
use super::pds_client::PdsClient;
use super::types::ClientSessionCredentials;
use super::ClientError;
use crate::{console_info, console_warn};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, instrument};

/// A session provider that automatically refreshes tokens when needed
///
/// This wraps a ClientSessionCredentials and provides methods to get a fresh
/// access token, automatically refreshing when the token is close to expiration
/// (within 5 minutes).
///
/// The provider includes a cached token to reduce lock contention during
/// pagination loops where the same token is valid for multiple requests.
pub struct RefreshableSessionProvider {
    session: Arc<Mutex<ClientSessionCredentials>>,
    client: Arc<PdsClient>,
    cached_token: Arc<Mutex<Option<(String, u64)>>>, // (token, valid_until timestamp)
}

impl RefreshableSessionProvider {
    /// Create a new refreshable session provider
    pub fn new(session: ClientSessionCredentials, client: Arc<PdsClient>) -> Self {
        Self {
            session: Arc::new(Mutex::new(session)),
            client,
            cached_token: Arc::new(Mutex::new(None)),
        }
    }

    /// Get current time in seconds
    fn current_time_secs() -> u64 {
        (js_sys::Date::now() / 1000.0) as u64
    }

    /// Get a fresh access token, refreshing if necessary
    ///
    /// This method checks if the current token needs refresh (within 5 minutes of expiry)
    /// and automatically refreshes it before returning. This prevents 401 errors during
    /// long-running operations.
    ///
    /// For pagination loops, this method uses a cached token to reduce lock contention
    /// when the same token is still valid.
    #[instrument(skip(self))]
    pub async fn get_fresh_token(&self) -> Result<String, ClientError> {
        // Check cache first to avoid locking session unnecessarily
        {
            let cache = self.cached_token.lock().await;
            if let Some((token, valid_until)) = cache.as_ref() {
                let now = Self::current_time_secs();
                if now < *valid_until {
                    // Cache hit - return cached token
                    return Ok(token.clone());
                }
            }
        }

        // Cache miss or expired - acquire session lock
        let mut session = self.session.lock().await;

        // Check if token needs refresh (within 5 minutes of expiry)
        if session.needs_refresh() {
            console_info!(
                "[RefreshableSessionProvider] Token needs refresh for DID: {}",
                session.did
            );

            // Attempt to refresh the session
            match refresh_session_impl(&self.client, &session).await {
                Ok(refreshed_session) => {
                    console_info!(
                        "[RefreshableSessionProvider] Successfully refreshed session for DID: {}",
                        refreshed_session.did
                    );
                    *session = refreshed_session;
                }
                Err(e) => {
                    error!(
                        "[RefreshableSessionProvider] Failed to refresh session: {}",
                        e
                    );
                    return Err(e);
                }
            }
        }

        let token = session.access_jwt.clone();

        // Update cache with new token (valid for 4 minutes to ensure refresh happens)
        let valid_until = Self::current_time_secs() + 240; // 4 minutes
        {
            let mut cache = self.cached_token.lock().await;
            *cache = Some((token.clone(), valid_until));
        }

        Ok(token)
    }

    /// Get a fresh access token with retry on failure
    ///
    /// This is a more robust version that retries token refresh if the first attempt fails.
    /// Useful for handling transient network issues during long-running migrations.
    pub async fn get_fresh_token_with_retry(
        &self,
        max_retries: u32,
    ) -> Result<String, ClientError> {
        let mut attempt = 0;
        let mut last_error = None;

        while attempt < max_retries {
            match self.get_fresh_token().await {
                Ok(token) => return Ok(token),
                Err(e) => {
                    attempt += 1;
                    last_error = Some(e);

                    if attempt < max_retries {
                        console_warn!(
                            "[RefreshableSessionProvider] Token refresh failed (attempt {}/{}), retrying...",
                            attempt,
                            max_retries
                        );

                        // Exponential backoff: 1s, 2s, 4s, etc.
                        let delay_ms = (1000 * (2_u64.pow(attempt - 1))).min(10000);
                        gloo_timers::future::TimeoutFuture::new(delay_ms as u32).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(ClientError::SessionExpired))
    }

    /// Force an immediate refresh of the session token
    ///
    /// This is useful when a 401 error is encountered, indicating the token
    /// is definitely expired or invalid. Clears the cache.
    #[instrument(skip(self))]
    pub async fn force_refresh(&self) -> Result<String, ClientError> {
        // Clear cache first
        {
            let mut cache = self.cached_token.lock().await;
            *cache = None;
        }

        let mut session = self.session.lock().await;

        console_info!(
            "[RefreshableSessionProvider] Force refreshing session for DID: {}",
            session.did
        );

        match refresh_session_impl(&self.client, &session).await {
            Ok(refreshed_session) => {
                console_info!(
                    "[RefreshableSessionProvider] Successfully force-refreshed session for DID: {}",
                    refreshed_session.did
                );
                let token = refreshed_session.access_jwt.clone();
                *session = refreshed_session;

                // Update cache with new token
                let valid_until = Self::current_time_secs() + 240; // 4 minutes
                {
                    let mut cache = self.cached_token.lock().await;
                    *cache = Some((token.clone(), valid_until));
                }

                Ok(token)
            }
            Err(e) => {
                error!(
                    "[RefreshableSessionProvider] Failed to force refresh session: {}",
                    e
                );
                Err(e)
            }
        }
    }

    /// Get the current session without refreshing
    ///
    /// Useful for reading session metadata like DID, handle, etc.
    pub async fn get_session(&self) -> ClientSessionCredentials {
        self.session.lock().await.clone()
    }

    /// Check if the session needs refresh without actually refreshing
    pub async fn needs_refresh(&self) -> bool {
        self.session.lock().await.needs_refresh()
    }

    /// Check if the session is expired
    pub async fn is_expired(&self) -> bool {
        self.session.lock().await.is_expired()
    }
}

impl Clone for RefreshableSessionProvider {
    fn clone(&self) -> Self {
        Self {
            session: Arc::clone(&self.session),
            client: Arc::clone(&self.client),
            cached_token: Arc::clone(&self.cached_token),
        }
    }
}
