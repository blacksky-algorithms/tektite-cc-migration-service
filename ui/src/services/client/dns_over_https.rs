use anyhow::Result;
use async_trait::async_trait;
use lru::LruCache;
use reqwest::Client;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{error, info, warn};

#[cfg(target_arch = "wasm32")]
use js_sys;

use super::errors::ResolveError;
use super::types::{CachedDnsResponse, CloudflareDoHResponse};

/// Get current time in milliseconds since UNIX epoch (WASM compatible)
#[cfg(target_arch = "wasm32")]
fn current_time_millis() -> u64 {
    js_sys::Date::now() as u64
}

/// DNS resolver trait for handle resolution
#[async_trait(?Send)] // Allow non-Send futures for WASM compatibility
pub trait DnsResolver {
    async fn resolve_txt(&self, domain: &str) -> Result<Vec<String>, ResolveError>;
}

/// DNS-over-HTTPS resolver with caching and fallback support
#[derive(Clone)]
pub struct DnsOverHttpsResolver {
    http_client: Client,
    primary_endpoint: String,
    fallback_endpoints: Vec<String>,
    cache: Arc<Mutex<LruCache<String, CachedDnsResponse>>>,
    timeout: Duration,
}

impl DnsOverHttpsResolver {
    /// Create a new DNS-over-HTTPS resolver
    pub fn new() -> Self {
        Self {
            http_client: {
                Client::builder()
                    .user_agent("atproto-migration-service/1.0")
                    .build()
                    .expect("Failed to create HTTP client")
            },
            primary_endpoint: "https://mozilla.cloudflare-dns.com/dns-query".to_string(),
            fallback_endpoints: vec![
                "https://cloudflare-dns.com/dns-query".to_string(),
                "https://dns.google/resolve".to_string(),
                "https://dns.quad9.net:5053/dns-query".to_string(),
            ],
            cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))),
            timeout: Duration::from_secs(5),
        }
    }

    /// Create a resolver with custom endpoints for testing
    pub fn with_endpoints(primary: String, fallbacks: Vec<String>) -> Self {
        Self {
            http_client: {
                Client::builder()
                    .user_agent("atproto-migration-service/1.0")
                    .build()
                    .expect("Failed to create HTTP client")
            },
            primary_endpoint: primary,
            fallback_endpoints: fallbacks,
            cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))),
            timeout: Duration::from_secs(5),
        }
    }

    /// Resolve TXT records from a single endpoint
    async fn resolve_txt_single(
        &self,
        endpoint: &str,
        domain: &str,
    ) -> Result<Vec<String>, ResolveError> {
        let url = format!("{}?name={}&type=16", endpoint, domain);

        info!("Resolving TXT records for {} via {}", domain, endpoint);

        let response: CloudflareDoHResponse = self
            .http_client
            .get(&url)
            .header("accept", "application/dns-json")
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|e| ResolveError::HttpRequestFailed {
                error: format!("HTTP request failed: {}", e),
            })?
            .json()
            .await
            .map_err(|e| ResolveError::JsonParseError {
                error: format!("JSON parse error: {}", e),
            })?;

        // Check DNS response status
        if response.status != 0 {
            return Err(ResolveError::DnsQueryFailed {
                status: response.status,
                domain: domain.to_string(),
            });
        }

        // Extract TXT records and cache TTL
        let mut min_ttl = u32::MAX;
        let txt_records: Vec<String> = response
            .answer
            .unwrap_or_default()
            .into_iter()
            .filter_map(|record| {
                if record.record_type == 16 {
                    // TXT = 16
                    min_ttl = min_ttl.min(record.ttl);
                    // Remove outer quotes from JSON string: "\"did=abc\"" -> "did=abc"
                    Some(record.data.trim_matches('"').to_string())
                } else {
                    None
                }
            })
            .collect();

        // Cache the response with appropriate TTL
        if !txt_records.is_empty() && min_ttl > 0 {
            let cache_entry = CachedDnsResponse {
                records: txt_records.clone(),
                expires_at: current_time_millis() + (min_ttl as u64 * 1000), // Convert seconds to milliseconds
            };

            if let Ok(mut cache) = self.cache.lock() {
                cache.put(domain.to_string(), cache_entry);
            }
        }

        Ok(txt_records)
    }

    /// Check cache for existing DNS response
    fn check_cache(&self, domain: &str) -> Option<Vec<String>> {
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(cached) = cache.get(domain) {
                if cached.expires_at > current_time_millis() {
                    return Some(cached.records.clone());
                } else {
                    // Remove expired entry
                    cache.pop(domain);
                }
            }
        }
        None
    }
}

#[async_trait(?Send)]
impl DnsResolver for DnsOverHttpsResolver {
    async fn resolve_txt(&self, domain: &str) -> Result<Vec<String>, ResolveError> {
        // Check cache first
        if let Some(cached_records) = self.check_cache(domain) {
            info!("DNS cache hit for {}", domain);
            return Ok(cached_records);
        }

        // Try primary endpoint
        match self
            .resolve_txt_single(&self.primary_endpoint, domain)
            .await
        {
            Ok(result) => {
                info!("Primary DoH endpoint succeeded for {}", domain);
                return Ok(result);
            }
            Err(e) => {
                warn!("Primary DoH endpoint failed for {}: {}", domain, e);
            }
        }

        // Try fallback endpoints
        for (i, endpoint) in self.fallback_endpoints.iter().enumerate() {
            match self.resolve_txt_single(endpoint, domain).await {
                Ok(result) => {
                    info!("Fallback DoH endpoint {} succeeded for {}", i, domain);
                    return Ok(result);
                }
                Err(e) => {
                    warn!("Fallback DoH endpoint {} failed for {}: {}", i, domain, e);
                }
            }
        }

        error!("All DoH endpoints failed for {}", domain);
        Err(ResolveError::AllDnsEndpointsFailed {
            domain: domain.to_string(),
        })
    }
}

impl Default for DnsOverHttpsResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_doh_resolver_rudyfraser() {
        let resolver = DnsOverHttpsResolver::new();
        let result = resolver
            .resolve_txt("_atproto.rudyfraser.com")
            .await
            .unwrap();
        assert_eq!(result, vec!["did=did:plc:w4xbfzo7kqfes5zb7r6qv3rw"]);
    }

    #[tokio::test]
    async fn test_doh_resolver_torrho() {
        let resolver = DnsOverHttpsResolver::new();
        let result = resolver.resolve_txt("_atproto.torrho.com").await.unwrap();
        assert_eq!(result, vec!["did=did:plc:n6jx25m5pr3bndqtmjot62xw"]);
    }

    #[tokio::test]
    async fn test_dns_caching() {
        let resolver = DnsOverHttpsResolver::new();

        // First call should hit network
        let start = std::time::Instant::now();
        let result1 = resolver
            .resolve_txt("_atproto.rudyfraser.com")
            .await
            .unwrap();
        let first_duration = start.elapsed();

        // Second call should hit cache
        let start = std::time::Instant::now();
        let result2 = resolver
            .resolve_txt("_atproto.rudyfraser.com")
            .await
            .unwrap();
        let second_duration = start.elapsed();

        assert_eq!(result1, result2);
        assert!(second_duration < first_duration / 2); // Cache should be much faster
    }

    #[tokio::test]
    async fn test_fallback_endpoints() {
        // Test with resolver that has primary endpoint disabled
        let resolver = DnsOverHttpsResolver::with_endpoints(
            "https://nonexistent.example.com/dns-query".to_string(),
            vec![
                "https://cloudflare-dns.com/dns-query".to_string(),
                "https://dns.google/resolve".to_string(),
            ],
        );

        // Should succeed via fallback
        let result = resolver
            .resolve_txt("_atproto.rudyfraser.com")
            .await
            .unwrap();
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn test_nonexistent_domain() {
        let resolver = DnsOverHttpsResolver::new();
        let result = resolver
            .resolve_txt("_atproto.nonexistent-domain-12345.com")
            .await;

        // Should return error for nonexistent domain
        match result {
            Err(ResolveError::DnsQueryFailed { status, .. }) => {
                assert!(status == 3); // NXDOMAIN
            }
            Err(ResolveError::AllDnsEndpointsFailed { .. }) => {
                // Also acceptable if all endpoints fail consistently
            }
            _ => panic!("Expected DNS query failure for nonexistent domain"),
        }
    }
}
