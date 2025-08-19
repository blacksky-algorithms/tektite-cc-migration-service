//! WASM-first sync orchestrator implementing the channel-tee pattern

use super::traits::*;
use crate::{console_info, console_error, console_debug};
use futures_util::StreamExt;
use std::error::Error;

/// WASM-first sync orchestrator for repository and blob migration
pub struct SyncOrchestrator;

impl SyncOrchestrator {
    /// Create a new sync orchestrator
    pub fn new() -> Self {
        Self
    }
    
    /// Generic sync method using channel-tee pattern for WASM
    /// 
    /// This method implements the WASM streaming architecture:
    /// 1. Fetch data from source using BrowserStream
    /// 2. Use channel-tee to duplicate stream to storage and upload
    /// 3. Run storage and upload concurrently using futures::join!
    pub async fn sync_with_tee<S, T, B>(
        &self,
        source: S,
        target: T,
        mut storage: B,
    ) -> Result<SyncResult, Box<dyn Error>>
    where
        S: DataSource + 'static,
        T: DataTarget + 'static,  
        B: StorageBackend + 'static,
        S::Item: Clone + ToString,
    {
        console_info!("[SyncOrchestrator] Starting WASM sync with channel-tee pattern");
        
        // Get items to sync
        let items = source.list_items().await?;
        let missing = target.list_missing().await?;
        
        // Filter items if we have a missing list
        let items_to_sync: Vec<S::Item> = if missing.is_empty() {
            items
        } else {
            items.into_iter()
                .filter(|item| missing.contains(&item.to_string()))
                .collect()
        };
        
        console_info!(
            "[SyncOrchestrator] Processing {} items for sync",
            items_to_sync.len()
        );
        
        let mut total_bytes_processed = 0u64;
        let mut successful_items = 0u32;
        let mut failed_items = Vec::new();
        
        // Process each item
        for item in items_to_sync {
            let id = item.to_string();
            console_debug!("[SyncOrchestrator] Processing item: {}", id);
            
            match self.process_single_item(&source, &target, &mut storage, &item).await {
                Ok(bytes_processed) => {
                    total_bytes_processed += bytes_processed;
                    successful_items += 1;
                    console_debug!(
                        "[SyncOrchestrator] Successfully processed item: {} ({} bytes)",
                        id, bytes_processed
                    );
                }
                Err(e) => {
                    console_error!(
                        "[SyncOrchestrator] Failed to process item {}: {}",
                        id, e
                    );
                    failed_items.push(SyncFailure {
                        item_id: id,
                        error: e.to_string(),
                    });
                }
            }
        }
        
        console_info!(
            "[SyncOrchestrator] Sync completed: {}/{} successful, {} failed, {} bytes total",
            successful_items,
            successful_items + failed_items.len() as u32,
            failed_items.len(),
            total_bytes_processed
        );
        
        Ok(SyncResult {
            total_items: successful_items + failed_items.len() as u32,
            successful_items,
            failed_items,
            total_bytes_processed,
        })
    }
    
    /// Process a single item using the WASM channel-tee pattern
    async fn process_single_item<S, T, B>(
        &self,
        source: &S,
        target: &T,
        storage: &mut B,
        item: &S::Item,
    ) -> Result<u64, Box<dyn Error>>
    where
        S: DataSource,
        T: DataTarget,
        B: StorageBackend,
        S::Item: Clone + ToString,
    {
        let id = item.to_string();
        let mut stream = source.fetch_stream(item).await?;
        
        // Create the tee for storage and upload (2 outputs)
        let (tee, mut receivers) = ChannelTee::new(100, 2);
        let mut storage_rx = receivers.pop().unwrap();
        let mut upload_rx = receivers.pop().unwrap();
        
        let storage_id = id.clone();
        let upload_id = id.clone();
        
        // Task 1: Read stream and tee to channels
        let tee_task = async {
            let mut offset = 0;
            let mut total_bytes = 0u64;
            
            console_debug!("[SyncOrchestrator] Starting stream tee for {}", id);
            
            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result.map_err(|e| format!("Stream error: {}", e))?;
                let data_chunk = DataChunk {
                    id: id.clone(),
                    data: chunk.clone(),
                    offset,
                    total_size: None,
                };
                total_bytes += chunk.len() as u64;
                offset += chunk.len();
                tee.send(data_chunk).await
                    .map_err(|e| format!("Tee send error: {}", e))?;
            }
            
            console_debug!(
                "[SyncOrchestrator] Stream tee completed for {} ({} bytes)",
                id, total_bytes
            );
            
            Ok::<u64, Box<dyn Error>>(total_bytes)
        };
        
        // Task 2: Storage
        let storage_task = async move {
            console_debug!("[SyncOrchestrator] Starting storage task for {}", storage_id);
            
            while let Some(chunk) = storage_rx.recv().await {
                storage.write_chunk(&chunk).await
                    .map_err(|e| format!("Storage write error: {}", e))?;
            }
            
            storage.finalize(&storage_id).await
                .map_err(|e| format!("Storage finalize error: {}", e))?;
            
            console_debug!("[SyncOrchestrator] Storage task completed for {}", storage_id);
            Ok::<_, Box<dyn Error>>(())
        };
        
        // Task 3: Upload (collect data first, then upload)
        let upload_task = async move {
            console_debug!("[SyncOrchestrator] Starting upload task for {}", upload_id);
            
            let mut buffer = Vec::new();
            while let Some(chunk) = upload_rx.recv().await {
                buffer.extend_from_slice(&chunk.data);
            }
            
            if !buffer.is_empty() {
                target.upload_data(upload_id.clone(), buffer, "application/octet-stream").await
                    .map_err(|e| format!("Upload error: {}", e))?;
            }
            
            console_debug!("[SyncOrchestrator] Upload task completed for {}", upload_id);
            Ok::<_, Box<dyn Error>>(())
        };
        
        // Wait for all tasks to complete using futures::join! (WASM-compatible)
        let (tee_result, storage_result, upload_result) = 
            futures_util::future::join3(tee_task, storage_task, upload_task).await;
        
        let total_bytes = tee_result?;
        storage_result?;
        upload_result?;
        
        Ok(total_bytes)
    }
}

impl Default for SyncOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a sync operation
#[derive(Debug)]
pub struct SyncResult {
    pub total_items: u32,
    pub successful_items: u32,  
    pub failed_items: Vec<SyncFailure>,
    pub total_bytes_processed: u64,
}

/// Information about a failed sync item
#[derive(Debug)]
pub struct SyncFailure {
    pub item_id: String,
    pub error: String,
}