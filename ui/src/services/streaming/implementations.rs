//! WASM-first implementations of streaming traits for repository and blob migration

use super::browser_storage::BrowserStorage;
use super::traits::*;
use super::wasm_http_client::WasmHttpClient;
use crate::services::client::ClientSessionCredentials;
use crate::{console_debug, console_error, console_info, console_warn};
use async_trait::async_trait;
use std::error::Error;

// ============================================================================
// Repository Implementations
// ============================================================================

/// Repository data source - fetches repository CAR data from source PDS using WASM
pub struct RepoSource {
    pub pds_url: String,
    pub did: String,
    pub since: Option<String>,
    pub client: WasmHttpClient,
}

impl RepoSource {
    pub fn new(session: &ClientSessionCredentials) -> Self {
        Self {
            pds_url: session.pds.clone(),
            did: session.did.clone(),
            since: None,
            client: WasmHttpClient::new(),
        }
    }

    pub fn with_since(mut self, since: String) -> Self {
        self.since = Some(since);
        self
    }
}

#[async_trait(?Send)]
impl DataSource for RepoSource {
    type Item = String; // DID

    async fn list_items(&self) -> Result<Vec<Self::Item>, Box<dyn Error>> {
        // For repo sync, we just return the DID
        Ok(vec![self.did.clone()])
    }

    async fn fetch_stream(&self, _item: &Self::Item) -> Result<BrowserStream, Box<dyn Error>> {
        let mut url = format!(
            "{}/xrpc/com.atproto.sync.getRepo?did={}",
            self.pds_url, self.did
        );
        if let Some(ref since) = self.since {
            url.push_str(&format!("&since={}", since));
        }

        console_info!("[RepoSource] Fetching repository from: {}", url);

        // Repository CAR files - compression headers removed to fix ReadableStream hanging in WASM
        // The WasmHttpClient uses direct fetch without Accept-Encoding headers
        let stream = self
            .client
            .get_stream(&url)
            .await
            .map_err(|e| format!("Failed to fetch repo stream: {}", e))?;

        console_info!("[RepoSource] Repository stream established successfully");
        Ok(stream)
    }
}

/// Repository data target - uploads repository CAR data to target PDS using WASM
pub struct RepoTarget {
    pub pds_url: String,
    pub client: WasmHttpClient,
    pub access_token: String,
}

impl RepoTarget {
    pub fn new(session: &ClientSessionCredentials) -> Self {
        Self {
            pds_url: session.pds.clone(),
            client: WasmHttpClient::new(),
            access_token: session.access_jwt.clone(),
        }
    }
}

#[async_trait(?Send)]
impl DataTarget for RepoTarget {
    async fn upload_data(
        &self,
        _id: String,
        data: Vec<u8>,
        _content_type: &str,
    ) -> Result<(), Box<dyn Error>> {
        let url = format!("{}/xrpc/com.atproto.repo.importRepo", self.pds_url);

        console_info!(
            "[RepoTarget] Uploading repository to: {} with authentication",
            url
        );

        self.client
            .post_data_with_auth(
                &url,
                data,
                "application/vnd.ipld.car",
                Some(&self.access_token),
            )
            .await
            .map_err(|e| format!("Failed to upload repo: {}", e))?;

        console_info!("[RepoTarget] Repository upload completed successfully");
        Ok(())
    }

    async fn upload_chunk(
        &self,
        id: String,
        chunk: Vec<u8>,
        offset: usize,
        is_final: bool,
        content_type: &str,
    ) -> Result<(), Box<dyn Error>> {
        // For repository uploads, we need to collect all chunks since importRepo expects complete CAR data
        // However, we can still benefit from not double-buffering by using a temporary storage mechanism

        if is_final {
            console_debug!(
                "[RepoTarget] Received final chunk for {}, uploading complete repository",
                id
            );
            // This is the complete data accumulated by the orchestrator
            if !chunk.is_empty() {
                self.upload_data(id, chunk, content_type).await?;
            }
        } else {
            console_debug!(
                "[RepoTarget] Received chunk for {} at offset {} ({} bytes)",
                id,
                offset,
                chunk.len()
            );
            // For non-final chunks, we just log progress - actual upload happens with final chunk
            // In a future improvement, we could stream to storage here
        }
        Ok(())
    }

    async fn list_missing(&self) -> Result<Vec<String>, Box<dyn Error>> {
        // Repository import doesn't need to check for missing items
        Ok(vec![])
    }
}

// ============================================================================
// Blob Implementations
// ============================================================================

/// Blob data source - fetches blob data from source PDS using WASM
pub struct BlobSource {
    pub pds_url: String,
    pub did: String,
    pub client: WasmHttpClient,
}

impl BlobSource {
    pub fn new(session: &ClientSessionCredentials) -> Self {
        Self {
            pds_url: session.pds.clone(),
            did: session.did.clone(),
            client: WasmHttpClient::new(),
        }
    }
}

#[async_trait(?Send)]
impl DataSource for BlobSource {
    type Item = String; // CID

    async fn list_items(&self) -> Result<Vec<Self::Item>, Box<dyn Error>> {
        let mut all_cids = Vec::new();
        let mut cursor: Option<String> = None;
        const BATCH_SIZE: i64 = 100; // Small batches to avoid timeouts

        console_info!(
            "[BlobSource] Starting paginated blob listing for DID: {}",
            self.did
        );

        loop {
            // Build URL with pagination parameters
            let mut url = format!(
                "{}/xrpc/com.atproto.sync.listBlobs?did={}&limit={}",
                self.pds_url, self.did, BATCH_SIZE
            );

            if let Some(ref c) = cursor {
                url.push_str(&format!("&cursor={}", c));
            }

            console_debug!("[BlobSource] Fetching blob batch from: {}", url);

            #[derive(serde::Deserialize)]
            struct ListBlobsOutput {
                cids: Vec<String>,
                cursor: Option<String>,
            }

            // Add timeout to detect hangs (using WASM-compatible approach)
            let result = {
                #[cfg(target_arch = "wasm32")]
                {
                    // For WASM, we rely on browser fetch timeout instead of implementing our own
                    self.client.get_json::<ListBlobsOutput>(&url).await
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    use tokio::time::{timeout, Duration};
                    // For non-WASM, use tokio timeout
                    timeout(Duration::from_secs(30), self.client.get_json::<ListBlobsOutput>(&url))
                        .await
                        .map_err(|_| "Request timeout".to_string())
                        .and_then(|result| result)
                }
            };

            match result {
                Ok(response) => {
                    let batch_count = response.cids.len();
                    all_cids.extend(response.cids);

                    console_info!(
                        "[BlobSource] Fetched {} blobs in this batch, {} total so far",
                        batch_count,
                        all_cids.len()
                    );

                    cursor = response.cursor;

                    // If no cursor or empty cursor, we're done
                    if cursor.is_none() || cursor.as_ref().is_some_and(|c| c.is_empty()) {
                        break;
                    }

                    // Small delay between requests to avoid overwhelming the server
                    #[cfg(target_arch = "wasm32")]
                    gloo_timers::future::TimeoutFuture::new(100).await;
                    #[cfg(not(target_arch = "wasm32"))]
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                Err(e) => {
                    // If we have some results, return them; otherwise propagate error
                    if !all_cids.is_empty() {
                        console_warn!(
                            "[BlobSource] Error fetching batch, returning {} blobs collected so far: {}",
                            all_cids.len(), e
                        );
                        break;
                    } else {
                        return Err(format!("Failed to list blobs: {}", e).into());
                    }
                }
            }
        }

        console_info!(
            "[BlobSource] Completed blob listing: {} total blobs",
            all_cids.len()
        );
        Ok(all_cids)
    }

    async fn fetch_stream(&self, cid: &Self::Item) -> Result<BrowserStream, Box<dyn Error>> {
        let url = format!(
            "{}/xrpc/com.atproto.sync.getBlob?did={}&cid={}",
            self.pds_url, self.did, cid
        );

        console_debug!("[BlobSource] Fetching blob {} from: {}", cid, url);

        // Blobs (images, videos) - compression headers removed to fix ReadableStream hanging in WASM
        // The WasmHttpClient uses direct fetch without Accept-Encoding headers
        let stream = self
            .client
            .get_stream(&url)
            .await
            .map_err(|e| format!("Failed to fetch blob stream: {}", e))?;

        Ok(stream)
    }
}

/// Blob data target - uploads blob data to target PDS using WASM
pub struct BlobTarget {
    pub pds_url: String,
    pub client: WasmHttpClient,
    pub access_token: String,
}

impl BlobTarget {
    pub fn new(session: &ClientSessionCredentials) -> Self {
        Self {
            pds_url: session.pds.clone(),
            client: WasmHttpClient::new(),
            access_token: session.access_jwt.clone(),
        }
    }
}

#[async_trait(?Send)]
impl DataTarget for BlobTarget {
    async fn upload_data(
        &self,
        cid: String,
        data: Vec<u8>,
        _content_type: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Basic token validation
        if self.access_token.is_empty() {
            return Err("Access token is empty - authentication required for blob upload".into());
        }

        // Basic JWT format validation (should have 3 parts separated by dots)
        let token_parts: Vec<&str> = self.access_token.split('.').collect();
        if token_parts.len() != 3 {
            return Err("Invalid JWT token format - expected 3 parts separated by dots".into());
        }

        let url = format!("{}/xrpc/com.atproto.repo.uploadBlob", self.pds_url);

        console_debug!(
            "[BlobTarget] Uploading blob {} to: {} with authentication (token length: {})",
            cid,
            url,
            self.access_token.len()
        );

        self.client
            .post_data_with_auth(
                &url,
                data,
                "application/octet-stream",
                Some(&self.access_token),
            )
            .await
            .map_err(|e| {
                console_error!("[BlobTarget] Upload failed for blob {}: {}", cid, e);

                // Check if this is a rate limiting error (504 Gateway Timeout)
                if e.contains("Gateway timeout (504)") {
                    format!("RATE_LIMIT:Failed to upload blob {}: {}", cid, e)
                } else {
                    format!("Failed to upload blob {}: {}", cid, e)
                }
            })?;

        console_debug!("[BlobTarget] Blob {} upload completed", cid);
        Ok(())
    }

    async fn list_missing(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let url = format!("{}/xrpc/com.atproto.repo.listMissingBlobs", self.pds_url);

        console_info!(
            "[BlobTarget] Listing missing blobs from: {} with authentication",
            url
        );

        #[derive(serde::Deserialize)]
        struct RecordBlob {
            cid: String,
            #[serde(rename = "recordUri")]
            #[allow(dead_code)]
            record_uri: String,
        }

        #[derive(serde::Deserialize)]
        struct ListMissingBlobsOutput {
            blobs: Vec<RecordBlob>,
            #[allow(dead_code)]
            cursor: Option<String>,
        }

        let response: ListMissingBlobsOutput = self
            .client
            .get_json_with_auth(&url, Some(&self.access_token))
            .await
            .map_err(|e| format!("Failed to list missing blobs: {}", e))?;

        let missing_cids = response
            .blobs
            .into_iter()
            .map(|b| b.cid)
            .collect::<Vec<_>>();
        console_info!("[BlobTarget] Found {} missing blobs", missing_cids.len());

        Ok(missing_cids)
    }
}

// ============================================================================
// Storage Backend Implementations
// ============================================================================

/// In-memory storage backend with buffers (mainly for repos and small blobs)
pub struct BufferedStorage {
    base_path: String,
    browser_storage: BrowserStorage,
}

impl BufferedStorage {
    pub async fn new(base_path: String) -> Result<Self, Box<dyn Error>> {
        let browser_storage = BrowserStorage::new()
            .await
            .map_err(|e| format!("Failed to create browser storage: {}", e))?;

        Ok(Self {
            base_path,
            browser_storage,
        })
    }
}

#[async_trait(?Send)]
impl StorageBackend for BufferedStorage {
    async fn write_chunk(&mut self, chunk: &DataChunk) -> Result<(), Box<dyn Error>> {
        self.browser_storage
            .write_chunk(&chunk.id, chunk.offset, &chunk.data)
            .await
            .map_err(|e| e.into())
    }

    async fn finalize(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
        console_debug!(
            "[BufferedStorage] Finalized item {} in base path: {}",
            id,
            self.base_path
        );
        self.browser_storage.finalize(id).await
    }

    async fn read_data(&self, id: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        self.browser_storage
            .read_data(id)
            .await
            .map_err(|e| e.into())
    }
}
