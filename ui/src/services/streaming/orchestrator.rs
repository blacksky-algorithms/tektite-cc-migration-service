//! WASM-first sync orchestrator implementing the channel-tee pattern

use super::traits::*;
use crate::{console_debug, console_error, console_info, console_warn};
use futures_util::StreamExt;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Progress update information for granular tracking
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub item_id: Option<String>,
    pub phase: ProgressPhase,
    pub bytes_processed: u64,
    pub total_bytes_estimate: u64,
    pub event: ProgressEvent,
}

/// Different phases of the sync operation
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressPhase {
    Starting,    // Item is starting to be processed
    Downloading, // Data is being downloaded from source
    Uploading,   // Data is being uploaded to target
    Completing,  // Item processing is finishing
}

/// Progress events that can occur
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressEvent {
    Started,   // Item/phase has started
    Progress,  // Incremental progress update
    Completed, // Item/phase has completed
}

#[cfg(not(target_arch = "wasm32"))]
use tokio::time::{timeout, Duration};

/// Optimal channel capacity for memory efficiency in WASM environment
/// Reduced from 64 to 16 to detect backpressure faster and prevent memory buildup
const CHANNEL_CAPACITY: usize = 16;

/// Stream timeout for detecting stalled streams (used in non-WASM builds)
#[cfg(not(target_arch = "wasm32"))]
const STREAM_TIMEOUT_SECS: u64 = 30;

/// Maximum retry attempts for failed operations
const MAX_RETRY_ATTEMPTS: u32 = 3;

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
    pub async fn sync_with_tee<S, T, B, P>(
        &self,
        source: S,
        target: T,
        storage: B,
        mut progress_callback: Option<P>,
    ) -> Result<SyncResult, Box<dyn Error>>
    where
        S: DataSource + 'static,
        T: DataTarget + 'static,
        B: StorageBackend + 'static,
        S::Item: Clone + ToString,
        P: FnMut(ProgressUpdate) + 'static, // Enhanced progress callback with detailed phase information
    {
        console_info!("[SyncOrchestrator] Starting WASM sync with channel-tee pattern");

        // Get items to sync
        let items = source.list_items().await?;
        let missing = target.list_missing().await?;

        // Filter items if we have a missing list
        let items_to_sync: Vec<S::Item> = if missing.is_empty() {
            items
        } else {
            items
                .into_iter()
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

        // Create shared storage reference
        let storage = Arc::new(Mutex::new(storage));

        // Process each item with retry logic
        for item in items_to_sync {
            let id = item.to_string();
            console_info!("[SyncOrchestrator] Processing item: {}", id);

            // Invoke progress callback at the START of processing each new item
            if let Some(ref mut callback) = progress_callback {
                console_debug!(
                    "[SyncOrchestrator] Invoking progress callback for starting item: {}",
                    id
                );
                callback(ProgressUpdate {
                    item_id: Some(id.clone()),
                    phase: ProgressPhase::Starting,
                    bytes_processed: 0,
                    total_bytes_estimate: 1000000, // rough estimate
                    event: ProgressEvent::Started,
                });
            }

            let mut retry_count = 0;
            let mut last_error = String::new();
            let mut success = false;

            while retry_count <= MAX_RETRY_ATTEMPTS && !success {
                match self
                    .process_single_item(
                        &source,
                        &target,
                        Arc::clone(&storage),
                        &item,
                        &mut progress_callback,
                    )
                    .await
                {
                    Ok(bytes_processed) => {
                        total_bytes_processed += bytes_processed;
                        successful_items += 1;
                        success = true;

                        // Invoke progress callback for successful item completion
                        if let Some(ref mut callback) = progress_callback {
                            console_debug!("[SyncOrchestrator] Invoking progress callback for completed item: {} ({} bytes)", id, bytes_processed);
                            callback(ProgressUpdate {
                                item_id: Some(id.clone()),
                                phase: ProgressPhase::Completing,
                                bytes_processed,
                                total_bytes_estimate: bytes_processed,
                                event: ProgressEvent::Completed,
                            });
                        }

                        if retry_count > 0 {
                            console_info!(
                                "[SyncOrchestrator] Successfully processed item: {} ({} bytes) after {} retries",
                                id, bytes_processed, retry_count
                            );
                        } else {
                            console_info!(
                                "[SyncOrchestrator] Successfully processed item: {} ({} bytes)",
                                id,
                                bytes_processed
                            );
                        }
                    }
                    Err(e) => {
                        last_error = e.to_string();
                        retry_count += 1;

                        if retry_count <= MAX_RETRY_ATTEMPTS {
                            console_debug!(
                                "[SyncOrchestrator] Failed to process item {} (attempt {}): {}. Analyzing error...",
                                id, retry_count, last_error
                            );

                            // Parse rate limit error for intelligent retry
                            let delay_ms = if last_error.starts_with("RATE_LIMIT:429:") {
                                // Extract retry-after from error message
                                // Format: "RATE_LIMIT:429:{retry_after}:..."
                                let parts: Vec<&str> = last_error.split(':').collect();
                                let retry_after_secs = parts
                                    .get(2)
                                    .and_then(|s| s.parse::<u64>().ok())
                                    .unwrap_or(60);

                                // Add jitter to prevent thundering herd
                                let jitter = (retry_count as u64) * 1000; // 1-3 seconds jitter
                                let delay = (retry_after_secs * 1000) + jitter;

                                console_info!(
                                    "[SyncOrchestrator] Rate limit detected for {}, waiting {}s as instructed by server (plus {}ms jitter)",
                                    id, retry_after_secs, jitter
                                );
                                delay
                            } else if last_error.contains("Gateway timeout (504)") {
                                // Actual gateway timeout - use exponential backoff
                                let base_delay = 2000; // 2 seconds base
                                let exponential_delay = base_delay * (2_u64.pow(retry_count - 1));
                                console_info!(
                                    "[SyncOrchestrator] Gateway timeout for {}, using exponential backoff: {}ms",
                                    id, exponential_delay
                                );
                                exponential_delay
                            } else {
                                // Other errors - progressive delay
                                1000 * retry_count as u64
                            };

                            #[cfg(target_arch = "wasm32")]
                            gloo_timers::future::TimeoutFuture::new(delay_ms as u32).await;
                            #[cfg(not(target_arch = "wasm32"))]
                            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                        } else {
                            console_error!(
                                "[SyncOrchestrator] Failed to process item {} after {} attempts: {}",
                                id, retry_count, last_error
                            );
                        }
                    }
                }
            }

            if !success {
                failed_items.push(SyncFailure {
                    item_id: id,
                    error: format!(
                        "Failed after {} retries: {}",
                        MAX_RETRY_ATTEMPTS, last_error
                    ),
                });
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

    /// Convenience method for sync without progress callback
    pub async fn sync_with_tee_simple<S, T, B>(
        &self,
        source: S,
        target: T,
        storage: B,
    ) -> Result<SyncResult, Box<dyn Error>>
    where
        S: DataSource + 'static,
        T: DataTarget + 'static,
        B: StorageBackend + 'static,
        S::Item: Clone + ToString,
    {
        self.sync_with_tee::<S, T, B, fn(ProgressUpdate)>(source, target, storage, None)
            .await
    }

    /// Process a single item using the WASM channel-tee pattern
    async fn process_single_item<S, T, B, P>(
        &self,
        source: &S,
        target: &T,
        storage: Arc<Mutex<B>>,
        item: &S::Item,
        progress_callback: &mut Option<P>,
    ) -> Result<u64, Box<dyn Error>>
    where
        S: DataSource,
        T: DataTarget,
        B: StorageBackend,
        S::Item: Clone + ToString,
        P: FnMut(ProgressUpdate) + 'static,
    {
        let id = item.to_string();
        let stream = source.fetch_stream(item).await?;

        // Create the tee for storage and upload (2 outputs)
        let (tee, mut receivers) = ChannelTee::<CHANNEL_CAPACITY>::new(2);
        let mut storage_rx = receivers.pop().unwrap();
        let mut upload_rx = receivers.pop().unwrap();

        let storage_id = id.clone();
        let upload_id = id.clone();
        let tee_id = id.clone(); // Clone for tee task
        let main_id = id.clone(); // Clone for main function use
        let storage_clone = Arc::clone(&storage);
        let storage_clone2 = Arc::clone(&storage);

        // Create shared progress callback for all tasks (borrow instead of take to keep available)
        let shared_progress_cb = Arc::new(Mutex::new(progress_callback.as_mut()));

        // Clone the shared progress callback for tasks
        let progress_cb_tee = Arc::clone(&shared_progress_cb);
        let progress_cb_upload = Arc::clone(&shared_progress_cb);

        // Task 1: Read stream and tee to channels with progress reporting
        let tee_task = async move {
            let mut offset = 0;
            let mut total_bytes = 0u64;
            let mut last_progress_report = 0u64;
            let mut chunk_count = 0u32;

            console_debug!("[SyncOrchestrator] Starting stream tee for {}", tee_id);

            // Stream processing loop with conditional timeout for non-WASM
            #[cfg(target_arch = "wasm32")]
            let stream_iter = futures_util::stream::unfold(stream, |mut s| async {
                console_debug!("[SyncOrchestrator] Calling stream.next() in unfold");
                let result = s.next().await;
                match &result {
                    Some(Ok(chunk)) => {
                        console_debug!(
                            "[SyncOrchestrator] Stream unfold received chunk: {} bytes",
                            chunk.len()
                        );
                    }
                    Some(Err(e)) => {
                        console_error!("[SyncOrchestrator] Stream unfold received error: {}", e);
                    }
                    None => {
                        console_info!("[SyncOrchestrator] Stream unfold completed (None)");
                    }
                }
                result.map(|chunk| (chunk, s))
            });

            // #[cfg(not(target_arch = "wasm32"))]
            // let stream_iter = futures_util::stream::unfold(stream, |mut s| async {
            //     match timeout(Duration::from_secs(STREAM_TIMEOUT_SECS), s.next()).await {
            //         Ok(result) => result.map(|chunk| (chunk, s)),
            //         Err(_) => Some((
            //             Err("Stream timeout - no data received for 30 seconds".to_string()),
            //             s,
            //         )),
            //     }
            // });

            futures_util::pin_mut!(stream_iter);
            console_debug!(
                "[SyncOrchestrator] Starting stream iteration for {}",
                tee_id
            );

            while let Some(chunk_result) = stream_iter.next().await {
                console_debug!("[SyncOrchestrator] Received chunk result for {}", tee_id);

                let chunk = chunk_result.map_err(|e| {
                    let error_msg = format!("Stream error for {}: {}", tee_id, e);
                    console_error!("[SyncOrchestrator] {}", error_msg);
                    error_msg
                })?;

                chunk_count += 1;
                let chunk_size = chunk.len();
                console_debug!(
                    "[SyncOrchestrator] Processing chunk {} for {} ({} bytes)",
                    chunk_count,
                    tee_id,
                    chunk_size
                );

                let data_chunk = DataChunk {
                    id: tee_id.clone(),
                    data: chunk.clone(),
                    offset,
                    total_size: None,
                };
                total_bytes += chunk_size as u64;
                offset += chunk_size;

                // Report progress more frequently: every 64KB, every 5 chunks, or at completion
                const PROGRESS_INTERVAL_KB: u64 = 64 * 1024; // 64KB intervals for more responsive progress
                if total_bytes - last_progress_report >= PROGRESS_INTERVAL_KB
                    || chunk_count.is_multiple_of(5)
                // Report every 5 chunks instead of 10
                {
                    console_info!(
                        "[SyncOrchestrator] Progress: {} bytes processed, {} chunks for {}",
                        total_bytes,
                        chunk_count,
                        tee_id
                    );
                    last_progress_report = total_bytes;

                    // Call progress callback during streaming for real-time UI updates
                    if let Ok(mut cb_guard) = progress_cb_tee.try_lock() {
                        if let Some(ref mut callback) = *cb_guard {
                            console_debug!("[SyncOrchestrator] Calling progress callback during streaming: {} bytes", total_bytes);
                            callback(ProgressUpdate {
                                item_id: Some(tee_id.clone()),
                                phase: ProgressPhase::Downloading,
                                bytes_processed: total_bytes,
                                total_bytes_estimate: total_bytes + 1000000, // rough estimate
                                event: ProgressEvent::Progress,
                            });
                        }
                    }
                }

                console_debug!(
                    "[SyncOrchestrator] Sending chunk {} to tee for {}",
                    chunk_count,
                    tee_id
                );
                tee.send(data_chunk).await.map_err(|e| {
                    let error_msg =
                        format!("Tee send error for {} chunk {}: {}", tee_id, chunk_count, e);
                    console_error!("[SyncOrchestrator] {}", error_msg);
                    error_msg
                })?;
                console_debug!(
                    "[SyncOrchestrator] Successfully sent chunk {} to tee for {}",
                    chunk_count,
                    tee_id
                );
            }

            console_info!(
                "[SyncOrchestrator] Stream tee completed for {} ({} bytes, {} chunks)",
                tee_id,
                total_bytes,
                chunk_count
            );

            // Final progress callback to ensure download phase completion is reported
            let mut cb_guard = progress_cb_tee.lock().await;
            if let Some(ref mut callback) = *cb_guard {
                console_debug!("[SyncOrchestrator] Calling final download progress callback: {} bytes (streaming complete)", total_bytes);
                callback(ProgressUpdate {
                    item_id: Some(tee_id.clone()),
                    phase: ProgressPhase::Downloading,
                    bytes_processed: total_bytes,
                    total_bytes_estimate: total_bytes,
                    event: ProgressEvent::Completed,
                });
            }

            Ok::<u64, Box<dyn Error>>(total_bytes)
        };

        // Task 2: Storage
        let storage_task = async move {
            console_info!(
                "[SyncOrchestrator] Starting storage task for {}",
                storage_id
            );
            let mut chunk_count = 0u32;
            let mut total_stored_bytes = 0u64;

            while let Some(chunk) = storage_rx.recv().await {
                chunk_count += 1;
                let chunk_size = chunk.data.len() as u64;
                total_stored_bytes += chunk_size;

                console_debug!(
                    "[SyncOrchestrator] Storage receiving chunk {} for {} ({} bytes)",
                    chunk_count,
                    storage_id,
                    chunk_size
                );

                let mut storage_guard = storage_clone.lock().await;
                storage_guard.write_chunk(&chunk).await.map_err(|e| {
                    let error_msg = format!(
                        "Storage write error for {} chunk {}: {}",
                        storage_id, chunk_count, e
                    );
                    console_error!("[SyncOrchestrator] {}", error_msg);
                    error_msg
                })?;

                if chunk_count.is_multiple_of(100) {
                    console_info!(
                        "[SyncOrchestrator] Storage progress: {} chunks ({} bytes) for {}",
                        chunk_count,
                        total_stored_bytes,
                        storage_id
                    );
                }
            }

            console_info!("[SyncOrchestrator] Storage received all chunks for {} ({} chunks, {} bytes), finalizing...", storage_id, chunk_count, total_stored_bytes);

            let mut storage_guard = storage_clone.lock().await;
            storage_guard.finalize(&storage_id).await.map_err(|e| {
                let error_msg = format!("Storage finalize error for {}: {}", storage_id, e);
                console_error!("[SyncOrchestrator] {}", error_msg);
                error_msg
            })?;

            console_info!(
                "[SyncOrchestrator] Storage task completed for {} ({} chunks, {} bytes)",
                storage_id,
                chunk_count,
                total_stored_bytes
            );
            Ok::<_, Box<dyn Error>>(())
        };

        // Task 3: Upload from storage (read data from storage backend and upload)
        let upload_task = async move {
            console_info!("[SyncOrchestrator] Starting upload task for {}", upload_id);
            let mut received_chunks = 0u32;

            // Wait for upload_rx to close (indicating storage is complete)
            console_debug!(
                "[SyncOrchestrator] Waiting for upload_rx to close for {}",
                upload_id
            );
            while upload_rx.recv().await.is_some() {
                received_chunks += 1;
                if received_chunks.is_multiple_of(100) {
                    console_debug!(
                        "[SyncOrchestrator] Upload task received {} notification chunks for {}",
                        received_chunks,
                        upload_id
                    );
                }
                // We don't actually use the chunks here - they're handled by storage
                // This just ensures we wait for the channel to close
            }

            console_info!("[SyncOrchestrator] Upload_rx closed after {} notifications, reading data from storage for upload: {}", received_chunks, upload_id);

            // Read the complete data from storage
            let storage_guard = storage_clone2.lock().await;
            console_debug!(
                "[SyncOrchestrator] Acquired storage lock for reading {}",
                upload_id
            );

            match storage_guard.read_data(&upload_id).await {
                Ok(data) => {
                    let data_size = data.len();
                    console_info!(
                        "[SyncOrchestrator] Read {} bytes from storage for {}",
                        data_size,
                        upload_id
                    );

                    if !data.is_empty() {
                        // Upload start progress callback
                        if let Ok(mut cb_guard) = progress_cb_upload.try_lock() {
                            if let Some(ref mut callback) = *cb_guard {
                                callback(ProgressUpdate {
                                    item_id: Some(upload_id.clone()),
                                    phase: ProgressPhase::Uploading,
                                    bytes_processed: 0,
                                    total_bytes_estimate: data_size as u64,
                                    event: ProgressEvent::Started,
                                });
                            }
                        }

                        console_info!(
                            "[SyncOrchestrator] Uploading {} bytes for {}",
                            data_size,
                            upload_id
                        );
                        target
                            .upload_data(upload_id.clone(), data, "application/octet-stream")
                            .await
                            .map_err(|e| {
                                let error_msg = format!("Upload error for {}: {}", upload_id, e);
                                console_error!("[SyncOrchestrator] {}", error_msg);
                                error_msg
                            })?;
                        console_info!(
                            "[SyncOrchestrator] Successfully uploaded {} bytes for {}",
                            data_size,
                            upload_id
                        );

                        // Upload completion progress callback
                        if let Ok(mut cb_guard) = progress_cb_upload.try_lock() {
                            if let Some(ref mut callback) = *cb_guard {
                                callback(ProgressUpdate {
                                    item_id: Some(upload_id.clone()),
                                    phase: ProgressPhase::Uploading,
                                    bytes_processed: data_size as u64,
                                    total_bytes_estimate: data_size as u64,
                                    event: ProgressEvent::Completed,
                                });
                            }
                        }
                    } else {
                        console_warn!(
                            "[SyncOrchestrator] No data to upload for {} (empty storage)",
                            upload_id
                        );
                    }
                }
                Err(e) => {
                    let error_msg =
                        format!("Failed to read data from storage for {}: {}", upload_id, e);
                    console_error!("[SyncOrchestrator] {}", error_msg);
                    return Err(error_msg.into());
                }
            }

            console_info!("[SyncOrchestrator] Upload task completed for {}", upload_id);
            Ok::<_, Box<dyn Error>>(())
        };

        // Wait for all tasks to complete using futures::join! (WASM-compatible)
        console_info!(
            "[SyncOrchestrator] Waiting for all 3 tasks to complete for {}",
            main_id
        );
        let (tee_result, storage_result, upload_result) =
            futures_util::future::join3(tee_task, storage_task, upload_task).await;

        console_info!(
            "[SyncOrchestrator] All tasks completed for {}, checking results",
            main_id
        );

        let total_bytes = tee_result.map_err(|e| {
            let error_msg = format!("Tee task failed for {}: {}", main_id, e);
            console_error!("[SyncOrchestrator] {}", error_msg);
            error_msg
        })?;
        console_info!(
            "[SyncOrchestrator] Tee task successful for {} ({} bytes)",
            main_id,
            total_bytes
        );

        storage_result.map_err(|e| {
            let error_msg = format!("Storage task failed for {}: {}", main_id, e);
            console_error!("[SyncOrchestrator] {}", error_msg);
            error_msg
        })?;
        console_info!("[SyncOrchestrator] Storage task successful for {}", main_id);

        upload_result.map_err(|e| {
            let error_msg = format!("Upload task failed for {}: {}", main_id, e);
            console_error!("[SyncOrchestrator] {}", error_msg);
            error_msg
        })?;
        console_info!("[SyncOrchestrator] Upload task successful for {}", main_id);

        console_info!(
            "[SyncOrchestrator] All tasks completed successfully for {} ({} bytes total)",
            main_id,
            total_bytes
        );
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
