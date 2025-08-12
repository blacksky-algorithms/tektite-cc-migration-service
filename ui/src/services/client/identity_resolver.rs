use anyhow::Result;
use reqwest::Client;
use std::collections::HashSet;
use tracing::{info, warn, debug, instrument};

use super::dns_over_https::{DnsResolver, DnsOverHttpsResolver};
use super::errors::ResolveError;
use super::types::ClientPdsProvider;

/// Check if a handle is potentially valid and worth resolving
fn should_resolve_handle(handle: &str) -> bool {
    // Basic validation to prevent unnecessary network calls
    handle.len() > 6 &&  // Minimum viable handle length (e.g., "a.b.co")
    handle.contains('.') &&
    handle.chars().last().is_some_and(|c| c.is_alphabetic()) &&
    !handle.ends_with('.') &&  // Don't resolve incomplete handles like "torrho."
    handle.split('.').count() >= 2 &&  // Must have at least domain.tld
    !handle.contains(' ')  // No spaces allowed
}

/// Resolve handle to DID using DNS-over-HTTPS
#[instrument(skip(doh_resolver))]
pub async fn resolve_handle_dns_doh(
    doh_resolver: &dyn DnsResolver,
    handle: &str,
) -> Result<String, ResolveError> {
    let dns_domain = format!("_atproto.{}", handle);
    let txt_records = doh_resolver.resolve_txt(&dns_domain).await?;
    
    info!("Retrieved {} TXT records for {}", txt_records.len(), dns_domain);
    
    // Extract DIDs from TXT records
    let dids: HashSet<String> = txt_records
        .iter()
        .filter_map(|record| {
            // Parse "did=did:plc:abc123" format
            record.strip_prefix("did=").map(|did| did.to_string())
        })
        .collect();
    
    if dids.is_empty() {
        return Err(ResolveError::NoDIDsFound { domain: dns_domain });
    }
    
    if dids.len() > 1 {
        return Err(ResolveError::MultipleDIDsFound { 
            domain: dns_domain,
            dids: dids.into_iter().collect(),
        });
    }
    
    Ok(dids.into_iter().next().unwrap())
}

/// Resolve handle to DID using HTTP well-known endpoint
#[instrument(skip(http_client))]
pub async fn resolve_handle_http(
    http_client: &Client,
    handle: &str,
) -> Result<String, ResolveError> {
    let well_known_url = format!("https://{}/.well-known/atproto-did", handle);
    
    info!("Fetching DID from well-known endpoint: {}", well_known_url);
    
    let response = http_client
        .get(&well_known_url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| ResolveError::HttpRequestFailed { 
            error: format!("Failed to fetch {}: {}", well_known_url, e)
        })?;

    if !response.status().is_success() {
        return Err(ResolveError::HttpRequestFailed {
            error: format!("HTTP {} for {}", response.status(), well_known_url)
        });
    }

    let did_text = response
        .text()
        .await
        .map_err(|e| ResolveError::HttpRequestFailed { 
            error: format!("Failed to read response body: {}", e)
        })?
        .trim()
        .to_string();

    if did_text.starts_with("did:") {
        Ok(did_text)
    } else {
        Err(ResolveError::InvalidDidFormat { 
            value: did_text,
            source: well_known_url,
        })
    }
}

/// Parallel DNS + HTTP resolution with validation (mirrors server-side logic)
#[instrument(skip(doh_resolver, http_client))]
pub async fn resolve_handle_client_side(
    handle: &str,
    doh_resolver: &dyn DnsResolver,
    http_client: &Client,
) -> Result<String, ResolveError> {
    // Validate handle before making network calls
    if !should_resolve_handle(handle) {
        return Err(ResolveError::InvalidHandle { 
            handle: handle.to_string() 
        });
    }
    
    info!("Starting parallel handle resolution for {}", handle);
    
    let (dns_result, http_result) = tokio::join!(
        resolve_handle_dns_doh(doh_resolver, handle),
        resolve_handle_http(http_client, handle)
    );
    
    // Collect successful results
    let mut results = Vec::new();
    
    match &dns_result {
        Ok(did) => {
            info!("DNS resolution succeeded for {}: {}", handle, did);
            results.push(did.clone());
        }
        Err(e) => warn!("DNS resolution failed for {}: {}", handle, e),
    }
    
    match &http_result {
        Ok(did) => {
            info!("HTTP resolution succeeded for {}: {}", handle, did);
            results.push(did.clone());
        }
        Err(e) => {
            // Check if this is a CORS error (common and expected for many domains)
            let error_msg = format!("{}", e);
            if error_msg.contains("CORS") || 
               error_msg.contains("Cross-Origin") || 
               error_msg.contains("error sending request") ||
               error_msg.contains("Failed to fetch") {
                // CORS errors are expected for domains that don't configure CORS for .well-known
                debug!("HTTP resolution failed for {} due to CORS/network restriction (expected): {}", handle, e);
            } else {
                warn!("HTTP resolution failed for {}: {}", handle, e);
            }
        },
    }
    
    if results.is_empty() {
        return Err(ResolveError::NoDIDsFound { 
            domain: format!("both DNS and HTTP failed for {}", handle)
        });
    }
    
    // Success if we have at least one result
    if results.len() == 1 {
        info!("Single resolution method succeeded for {}: {}", handle, results[0]);
        return Ok(results[0].clone());
    }
    
    // If we have multiple results, validate they agree
    let first_did: &String = &results[0];
    if results.iter().all(|did| did == first_did) {
        info!("Multiple resolution methods agree for {}: {}", handle, first_did);
        Ok(first_did.clone())
    } else {
        // Log conflict but still return the first successful result
        // This is more user-friendly than failing completely
        warn!("Resolution methods disagree for {} - DNS: {:?}, HTTP: {:?}", 
              handle, dns_result, http_result);
        info!("Using first successful result for {}: {}", handle, results[0]);
        Ok(results[0].clone())
    }
}

/// Determine PDS provider from handle or DID (mirrors API logic)
pub async fn determine_pds_provider_client_side(
    handle_or_did: &str,
    doh_resolver: &dyn DnsResolver,
    http_client: &Client,
) -> ClientPdsProvider {
    // If it's already a DID, try to resolve the DID document
    if handle_or_did.starts_with("did:") {
        return determine_provider_from_did(handle_or_did).await;
    }

    
    // If it's not a valid handle, don't make network calls
    if !should_resolve_handle(handle_or_did) {
        return determine_provider_from_handle_domain(handle_or_did);
    }
    
    // If it's a handle, determine provider from domain regardless of resolution success
    // This is because bsky.social handles should be identified as Bluesky even if resolution succeeds
    let provider_from_domain = determine_provider_from_handle_domain(handle_or_did);
    
    // Only use DID-based provider determination for custom domains
    match provider_from_domain {
        ClientPdsProvider::Other(_) => {
            // For custom domains, try to resolve and get provider from DID
            match resolve_handle_client_side(handle_or_did, doh_resolver, http_client).await {
                Ok(did) => determine_provider_from_did(&did).await,
                Err(_) => provider_from_domain, // Fallback to domain heuristics
            }
        }
        _ => provider_from_domain, // Use domain-based provider (Bluesky, BlackSky, etc.)
    }
}

/// Determine PDS provider from DID document (placeholder - would need full DID resolution)
async fn determine_provider_from_did(did: &str) -> ClientPdsProvider {
    // For now, return Other with the DID
    // TODO: Implement proper DID document resolution to find PDS endpoints
    info!("Would resolve DID document for: {}", did);
    ClientPdsProvider::Other(format!("DID: {}", did))
}

/// Determine PDS provider from handle domain heuristics
fn determine_provider_from_handle_domain(handle: &str) -> ClientPdsProvider {
    let domain = if let Some(domain_part) = handle.split('.').next_back() {
        // Get the last two parts for domains like "user.bsky.social"
        let parts: Vec<&str> = handle.split('.').collect();
        if parts.len() >= 2 {
            format!("{}.{}", parts[parts.len()-2], parts[parts.len()-1])
        } else {
            domain_part.to_string()
        }
    } else {
        return ClientPdsProvider::None;
    };

    match domain.as_str() {
        "bsky.social" | "bsky.network" => ClientPdsProvider::Bluesky,
        "blacksky.app" => ClientPdsProvider::BlackSky,
        _ => ClientPdsProvider::Other(format!("Domain: {}", domain)),
    }
}

/// Web-based identity resolver combining DNS-over-HTTPS and HTTP resolution
pub struct WebIdentityResolver {
    pub dns_resolver: DnsOverHttpsResolver,
    pub http_client: Client,
    pub plc_hostname: String,
}

impl WebIdentityResolver {
    /// Create a new web identity resolver
    pub fn new() -> Self {
        Self {
            dns_resolver: DnsOverHttpsResolver::new(),
            http_client: {
                #[cfg(target_arch = "wasm32")]
                {
                    Client::builder()
                        .user_agent("atproto-migration-service/1.0")
                        .build()
                        .expect("Failed to create HTTP client")
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    Client::builder()
                        .timeout(std::time::Duration::from_secs(10))
                        .user_agent("atproto-migration-service/1.0")
                        .build()
                        .expect("Failed to create HTTP client")
                }
            },
            plc_hostname: "plc.directory".to_string(),
        }
    }
    
    /// Resolve handle to DID using both DNS and HTTP methods
    pub async fn resolve_handle(&self, handle: &str) -> Result<String, ResolveError> {
        resolve_handle_client_side(handle, &self.dns_resolver, &self.http_client).await
    }

    /// Determine PDS provider for a handle or DID
    pub async fn determine_provider(&self, handle_or_did: &str) -> ClientPdsProvider {
        determine_pds_provider_client_side(handle_or_did, &self.dns_resolver, &self.http_client).await
    }

    /// Validate handle format
    pub fn is_valid_handle(&self, handle: &str) -> bool {
        // Basic handle validation - should contain at least one dot and valid characters
        if handle.is_empty() || !handle.contains('.') {
            return false;
        }

        // Check for valid characters (alphanumeric, dots, hyphens)
        handle.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
    }

    /// Validate DID format
    pub fn is_valid_did(&self, did: &str) -> bool {
        // Basic DID validation - should start with "did:" and have proper structure
        if !did.starts_with("did:") {
            return false;
        }

        let parts: Vec<&str> = did.split(':').collect();
        parts.len() >= 3 && !parts[1].is_empty() && !parts[2].is_empty()
    }
}

impl Default for WebIdentityResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_resolution_end_to_end() {
        let identity_resolver = WebIdentityResolver::new();
        let did = identity_resolver.resolve_handle("rudyfraser.com").await.unwrap();
        assert_eq!(did, "did:plc:w4xbfzo7kqfes5zb7r6qv3rw");
    }

    #[tokio::test]
    async fn test_handle_resolution_torrho() {
        let identity_resolver = WebIdentityResolver::new();
        let did = identity_resolver.resolve_handle("torrho.com").await.unwrap();
        assert_eq!(did, "did:plc:n6jx25m5pr3bndqtmjot62xw");
    }

    #[tokio::test]
    async fn test_provider_determination() {
        let identity_resolver = WebIdentityResolver::new();
        
        // Test known domains
        let provider = identity_resolver.determine_provider("test.bsky.social").await;
        assert_eq!(provider, ClientPdsProvider::Bluesky);
        
        let provider = identity_resolver.determine_provider("test.blacksky.app").await;
        assert_eq!(provider, ClientPdsProvider::BlackSky);
        
        let provider = identity_resolver.determine_provider("test.example.com").await;
        matches!(provider, ClientPdsProvider::Other(_));
    }

    #[test]
    fn test_handle_validation() {
        let resolver = WebIdentityResolver::new();
        
        assert!(resolver.is_valid_handle("user.bsky.social"));
        assert!(resolver.is_valid_handle("test-user.example.com"));
        assert!(!resolver.is_valid_handle(""));
        assert!(!resolver.is_valid_handle("nodomainpart"));
        assert!(!resolver.is_valid_handle("invalid@handle.com"));
    }

    #[test]
    fn test_did_validation() {
        let resolver = WebIdentityResolver::new();
        
        assert!(resolver.is_valid_did("did:plc:abcd1234"));
        assert!(resolver.is_valid_did("did:web:example.com"));
        assert!(!resolver.is_valid_did(""));
        assert!(!resolver.is_valid_did("not-a-did"));
        assert!(!resolver.is_valid_did("did:"));
        assert!(!resolver.is_valid_did("did:onlymethod"));
    }
}