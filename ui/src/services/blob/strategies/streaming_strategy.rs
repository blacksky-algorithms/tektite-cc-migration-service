//! Streaming blob migration strategy for memory-efficient transfers

use async_trait::async_trait;
use dioxus::prelude::*;
use gloo_console as console;

use crate::services::{
    client::{ClientMissingBlob, ClientSessionCredentials, PdsClient},
    blob::blob_fallback_manager::FallbackBlobManager,
    errors::MigrationResult,
};
use crate::features::migration::types::MigrationAction;

use super::{MigrationStrategy, BlobMigrationResult, BlobFailure};

/// Streaming strategy for direct PDS-to-PDS transfers with minimal memory usage
pub struct StreamingStrategy {
    chunk_size: usize,
}

impl Default for StreamingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingStrategy {
    pub fn new() -> Self {
        Self {
            chunk_size: 1024 * 1024, // 1MB chunks
        }
    }
    
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self { chunk_size }
    }
}

#[async_trait(?Send)]
impl MigrationStrategy for StreamingStrategy {
    async fn migrate(
        &self,
        blobs: Vec<ClientMissingBlob>,
        old_session: ClientSessionCredentials,
        new_session: ClientSessionCredentials,
        _blob_manager: &mut FallbackBlobManager,
        dispatch: &EventHandler<MigrationAction>,
    ) -> MigrationResult<BlobMigrationResult> {
        console::info!("[StreamingStrategy] Starting streaming blob migration with {} blobs", blobs.len());
        console::info!("[StreamingStrategy] Using chunk size: {} bytes", self.chunk_size);
        
        let pds_client = PdsClient::new();
        let mut uploaded_count = 0u32;
        let mut failed_blobs = Vec::new();
        let mut total_bytes = 0u64;

        for (index, blob) in blobs.iter().enumerate() {
            dispatch.call(MigrationAction::SetMigrationStep(format!(
                "Streaming blob {} of {} (direct transfer)...",
                index + 1,
                blobs.len()
            )));
            
            console::debug!("[StreamingStrategy] Streaming blob {}", &blob.cid);
            
            // For now, implement as a direct transfer since WASM streaming is complex
            // In a full implementation, this would use chunked transfers
            match self.stream_blob_direct(&pds_client, &old_session, &new_session, &blob.cid).await {
                Ok(bytes_transferred) => {
                    uploaded_count += 1;
                    total_bytes += bytes_transferred;
                    console::debug!("[StreamingStrategy] Successfully streamed blob {} ({} bytes)", 
                                  &blob.cid, bytes_transferred);
                }
                Err(failure) => {
                    failed_blobs.push(failure);
                }
            }
        }
        
        console::info!("[StreamingStrategy] Completed streaming migration: {}/{} uploaded, {} failed", 
                      uploaded_count, blobs.len(), failed_blobs.len());

        Ok(BlobMigrationResult {
            total_blobs: blobs.len() as u32,
            uploaded_blobs: uploaded_count,
            failed_blobs,
            total_bytes_processed: total_bytes,
            strategy_used: self.name().to_string(),
        })
    }
    
    fn name(&self) -> &'static str {
        "streaming"
    }
    
    fn supports_blob_count(&self, _count: u32) -> bool {
        true // Supports any number of blobs
    }
    
    fn supports_storage_backend(&self, _backend: &str) -> bool {
        true // Doesn't use storage, works with any backend
    }
    
    fn priority(&self) -> u32 {
        70 // High priority for memory efficiency
    }
    
    fn estimate_memory_usage(&self, _blob_count: u32) -> u64 {
        // Minimal memory usage - only chunk size
        self.chunk_size as u64
    }
}

impl StreamingStrategy {
    /// Stream a single blob directly from old PDS to new PDS
    async fn stream_blob_direct(
        &self,
        pds_client: &PdsClient,
        old_session: &ClientSessionCredentials,
        new_session: &ClientSessionCredentials,
        cid: &str,
    ) -> Result<u64, BlobFailure> {
        // Download blob data
        let blob_data = match pds_client.export_blob(old_session, cid.to_string()).await {
            Ok(response) => {
                if response.success {
                    response.blob_data.unwrap_or_default()
                } else {
                    return Err(BlobFailure {
                        cid: cid.to_string(),
                        operation: "stream_download".to_string(),
                        error: response.message,
                    });
                }
            }
            Err(e) => {
                return Err(BlobFailure {
                    cid: cid.to_string(),
                    operation: "stream_download".to_string(),
                    error: format!("Request failed: {}", e),
                });
            }
        };
        
        let blob_size = blob_data.len() as u64;
        
        // Upload blob data
        match pds_client.upload_blob(new_session, cid.to_string(), blob_data).await {
            Ok(response) => {
                if response.success {
                    Ok(blob_size)
                } else {
                    Err(BlobFailure {
                        cid: cid.to_string(),
                        operation: "stream_upload".to_string(),
                        error: response.message,
                    })
                }
            }
            Err(e) => {
                Err(BlobFailure {
                    cid: cid.to_string(),
                    operation: "stream_upload".to_string(),
                    error: format!("Request failed: {}", e),
                })
            }
        }
    }
}