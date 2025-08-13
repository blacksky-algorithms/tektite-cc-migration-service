//! Concurrent blob migration strategy for high throughput scenarios

use async_trait::async_trait;
use dioxus::prelude::*;
use futures_util::{stream, StreamExt};
use gloo_console as console;
use std::sync::{Arc, atomic::{AtomicU32, AtomicU64, Ordering}};
use tokio::sync::{Semaphore, Mutex};

#[cfg(target_arch = "wasm32")]
use js_sys;

#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

/// Helper function to safely format u64 values for logging to avoid BigInt serialization issues
fn format_bytes(bytes: u64) -> String {
    bytes.to_string()
}

/// Helper function to safely format numeric values for logging to avoid BigInt serialization issues
fn format_number<T: std::fmt::Display>(value: T) -> String {
    value.to_string()
}

use crate::services::{
    client::{ClientMissingBlob, ClientSessionCredentials, PdsClient},
    blob::blob_fallback_manager::FallbackBlobManager,
    config::get_global_config,
    errors::MigrationResult,
};
use crate::features::migration::types::{MigrationAction, BlobProgress};

use super::{MigrationStrategy, BlobMigrationResult, BlobFailure};

/// Get current time in milliseconds since UNIX epoch (WASM compatible)
#[cfg(target_arch = "wasm32")]
fn current_time_millis() -> u64 {
    js_sys::Date::now() as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Get current time in seconds since UNIX epoch (WASM compatible)
#[cfg(target_arch = "wasm32")]
fn current_time_secs() -> f64 {
    js_sys::Date::now() / 1000.0
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

/// Shared progress tracking for concurrent operations
#[derive(Debug)]
pub(crate) struct ProgressTracker {
    processed_blobs: AtomicU32,
    processed_bytes: AtomicU64,
    current_blob_cid: Mutex<Option<String>>,
    start_time_millis: u64,
    last_update_time_millis: Mutex<u64>,
}

impl ProgressTracker {
    pub(crate) fn new() -> Self {
        let now = current_time_millis();
        Self {
            processed_blobs: AtomicU32::new(0),
            processed_bytes: AtomicU64::new(0),
            current_blob_cid: Mutex::new(None),
            start_time_millis: now,
            last_update_time_millis: Mutex::new(now),
        }
    }

    pub(crate) fn record_blob_completion(&self, cid: String, bytes: u64) {
        let new_count = self.processed_blobs.fetch_add(1, Ordering::SeqCst) + 1;
        self.processed_bytes.fetch_add(bytes, Ordering::SeqCst);
        if let Ok(mut current) = self.current_blob_cid.try_lock() {
            *current = Some(cid.clone());
        }
        console::debug!("[ProgressTracker] Recorded completion: blob {} ({} bytes) - total processed: {}", 
                       cid, format_bytes(bytes), format_number(new_count));
    }

    pub(crate) async fn should_update_progress(&self) -> bool {
        if let Ok(mut last_update) = self.last_update_time_millis.try_lock() {
            let now = current_time_millis();
            let elapsed_millis = now.saturating_sub(*last_update);
            let processed = self.processed_blobs.load(Ordering::SeqCst);
            
            // Update conditions:
            // - First blob completion (always show)
            // - Every 100ms for responsive UI updates
            // - Every 5th blob (ensure regular updates)
            // - Always update for first 10 blobs (important for early feedback)
            if processed == 1 || 
               elapsed_millis >= 100 || 
               processed % 5 == 0 ||
               processed <= 10 {
                console::debug!("[ProgressTracker] Triggering UI update: processed={}, elapsed={}ms", format_number(processed), format_number(elapsed_millis));
                *last_update = now;
                return true;
            }
        }
        false
    }

    pub(crate) async fn get_current_progress(&self, total_blobs: u32) -> BlobProgress {
        let processed = self.processed_blobs.load(Ordering::SeqCst);
        let bytes = self.processed_bytes.load(Ordering::SeqCst);
        let current_cid = if let Ok(cid_guard) = self.current_blob_cid.try_lock() {
            cid_guard.clone()
        } else {
            None
        };

        // Calculate progress percentage with more precision
        let progress_percent = if total_blobs > 0 {
            Some((processed as f64 / total_blobs as f64 * 100.0).min(100.0))
        } else {
            Some(0.0)
        };

        BlobProgress {
            total_blobs,
            processed_blobs: processed,
            total_bytes: 0, // We don't know total bytes up front in concurrent strategy
            processed_bytes: bytes,
            current_blob_cid: current_cid,
            current_blob_progress: progress_percent,
            error: None,
        }
    }

    pub(crate) fn get_throughput_info(&self) -> (f64, Option<u64>) {
        let processed = self.processed_blobs.load(Ordering::SeqCst);
        let bytes = self.processed_bytes.load(Ordering::SeqCst);
        
        let elapsed_millis = current_time_millis().saturating_sub(self.start_time_millis);
        let elapsed_secs = elapsed_millis as f64 / 1000.0;
        
        if elapsed_secs > 0.0 {
            let blobs_per_sec = processed as f64 / elapsed_secs;
            let bytes_per_sec = bytes as f64 / elapsed_secs;
            (blobs_per_sec, Some(bytes_per_sec as u64))
        } else {
            (0.0, None)
        }
    }
}

/// High-concurrency strategy for migrating many blobs quickly
pub struct ConcurrentStrategy {
    max_concurrent: usize,
}

impl Default for ConcurrentStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl ConcurrentStrategy {
    pub fn new() -> Self {
        let config = get_global_config();
        Self {
            max_concurrent: config.concurrency.max_concurrent_transfers,
        }
    }
    
    pub fn with_concurrency(max_concurrent: usize) -> Self {
        Self { max_concurrent }
    }
}

#[async_trait(?Send)]
impl MigrationStrategy for ConcurrentStrategy {
    async fn migrate(
        &self,
        blobs: Vec<ClientMissingBlob>,
        old_session: ClientSessionCredentials,
        new_session: ClientSessionCredentials,
        _blob_manager: &mut FallbackBlobManager,
        dispatch: &EventHandler<MigrationAction>,
    ) -> MigrationResult<BlobMigrationResult> {
        console::info!("[ConcurrentStrategy] Starting concurrent blob migration with {} blobs", blobs.len());
        console::info!("[ConcurrentStrategy] Max concurrent transfers: {}", self.max_concurrent);
        
        let _pds_client = PdsClient::new();
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));
        let progress_tracker = Arc::new(ProgressTracker::new());
        let mut uploaded_count = 0u32;
        let mut failed_blobs = Vec::new();
        let mut total_bytes = 0u64;

        // Initial progress update
        dispatch.call(MigrationAction::SetMigrationStep(
            "Starting concurrent blob migration...".to_string()
        ));
        dispatch.call(MigrationAction::SetBlobProgress(BlobProgress {
            total_blobs: blobs.len() as u32,
            processed_blobs: 0,
            total_bytes: 0,
            processed_bytes: 0,
            current_blob_cid: None,
            current_blob_progress: Some(0.0),
            error: None,
        }));

        // Process blobs concurrently using a stream
        let results: Vec<_> = stream::iter(blobs.iter().enumerate())
            .map(|(index, blob)| {
                let semaphore = Arc::clone(&semaphore);
                let progress_tracker = Arc::clone(&progress_tracker);
                let pds_client = PdsClient::new();
                let old_session = old_session.clone();
                let new_session = new_session.clone();
                let dispatch = *dispatch;
                let blob = blob.clone();
                let total_blobs = blobs.len() as u32;
                
                async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    
                    console::info!("[ConcurrentStrategy] Processing blob {} ({}/{})", &blob.cid, index + 1, total_blobs);

                    // Download blob from old PDS
                    let blob_data = match pds_client.export_blob(&old_session, blob.cid.clone()).await {
                        Ok(response) => {
                            if response.success {
                                response.blob_data.unwrap_or_default()
                            } else {
                                return Ok::<Result<u64, BlobFailure>, String>(Err(BlobFailure {
                                    cid: blob.cid.clone(),
                                    operation: "download".to_string(),
                                    error: response.message,
                                }));
                            }
                        }
                        Err(e) => {
                            return Ok::<Result<u64, BlobFailure>, String>(Err(BlobFailure {
                                cid: blob.cid.clone(),
                                operation: "download".to_string(),
                                error: format!("Request failed: {}", e),
                            }));
                        }
                    };

                    let blob_size = blob_data.len() as u64;
                    
                    // Upload blob to new PDS directly (bypass storage for concurrent strategy)
                    match pds_client.upload_blob(&new_session, blob.cid.clone(), blob_data).await {
                        Ok(response) => {
                            if response.success {
                                console::info!("[ConcurrentStrategy] Successfully migrated blob {} ({} bytes)", &blob.cid, format_bytes(blob_size));
                                
                                // Record completion and update progress if needed
                                progress_tracker.record_blob_completion(blob.cid.clone(), blob_size);
                                
                                if progress_tracker.should_update_progress().await {
                                    let progress = progress_tracker.get_current_progress(total_blobs).await;
                                    let (blobs_per_sec, bytes_per_sec) = progress_tracker.get_throughput_info();
                                    
                                    console::info!("[ConcurrentStrategy] Dispatching UI progress update: {}/{} ({:.1}%)", 
                                                 progress.processed_blobs, progress.total_blobs, 
                                                 progress.current_blob_progress.unwrap_or(0.0));
                                    
                                    // Generate informative progress message
                                    let progress_msg = if let Some(bps) = bytes_per_sec {
                                        format!("Migrating blobs: {}/{} ({:.1}%) - {:.1} blobs/sec, {:.1} MB/sec", 
                                               progress.processed_blobs, 
                                               progress.total_blobs,
                                               progress.current_blob_progress.unwrap_or(0.0),
                                               blobs_per_sec,
                                               bps as f64 / 1_048_576.0)
                                    } else {
                                        format!("Migrating blobs: {}/{} ({:.1}%)", 
                                               progress.processed_blobs, 
                                               progress.total_blobs,
                                               progress.current_blob_progress.unwrap_or(0.0))
                                    };
                                    
                                    dispatch.call(MigrationAction::SetMigrationStep(progress_msg));
                                    dispatch.call(MigrationAction::SetBlobProgress(progress));
                                } else {
                                    console::debug!("[ConcurrentStrategy] Progress update skipped for blob {}", &blob.cid);
                                }
                                
                                Ok(Ok(blob_size))
                            } else {
                                Ok::<Result<u64, BlobFailure>, String>(Err(BlobFailure {
                                    cid: blob.cid.clone(),
                                    operation: "upload".to_string(),
                                    error: response.message,
                                }))
                            }
                        }
                        Err(e) => {
                            Ok::<Result<u64, BlobFailure>, String>(Err(BlobFailure {
                                cid: blob.cid.clone(),
                                operation: "upload".to_string(),
                                error: format!("Request failed: {}", e),
                            }))
                        }
                    }
                }
            })
            .buffer_unordered(self.max_concurrent)
            .collect()
            .await;

        // Process results
        for result in results {
            match result.unwrap() {
                Ok(bytes) => {
                    uploaded_count += 1;
                    total_bytes += bytes;
                }
                Err(failure) => {
                    failed_blobs.push(failure);
                }
            }
        }
        
        // Final progress update
        let final_progress = progress_tracker.get_current_progress(blobs.len() as u32).await;
        let (blobs_per_sec, bytes_per_sec) = progress_tracker.get_throughput_info();
        
        console::info!("[ConcurrentStrategy] Final progress update: {}/{} ({:.1}%)", 
                     final_progress.processed_blobs, final_progress.total_blobs, 
                     final_progress.current_blob_progress.unwrap_or(0.0));
        
        dispatch.call(MigrationAction::SetBlobProgress(final_progress));
        
        // Final completion message with throughput stats
        let completion_msg = if let Some(bps) = bytes_per_sec {
            format!("Completed blob migration: {}/{} uploaded - {:.1} blobs/sec, {:.1} MB/sec", 
                   uploaded_count, 
                   blobs.len(),
                   blobs_per_sec,
                   bps as f64 / 1_048_576.0)
        } else {
            format!("Completed blob migration: {}/{} uploaded", uploaded_count, blobs.len())
        };
        
        dispatch.call(MigrationAction::SetMigrationStep(completion_msg));
        
        console::info!("[ConcurrentStrategy] Completed concurrent migration: {}/{} uploaded, {} failed", 
                      uploaded_count, blobs.len(), failed_blobs.len());
        
        if let Some(bps) = bytes_per_sec {
            console::info!("[ConcurrentStrategy] Throughput: {:.1} blobs/sec, {:.1} MB/sec", 
                          blobs_per_sec, bps as f64 / 1_048_576.0);
        }

        Ok(BlobMigrationResult {
            total_blobs: blobs.len() as u32,
            uploaded_blobs: uploaded_count,
            failed_blobs,
            total_bytes_processed: total_bytes,
            strategy_used: self.name().to_string(),
        })
    }
    
    fn name(&self) -> &'static str {
        "concurrent"
    }
    
    fn supports_blob_count(&self, count: u32) -> bool {
        count >= 10 // Best for many blobs
    }
    
    fn supports_storage_backend(&self, _backend: &str) -> bool {
        true // Works with any backend since it doesn't use storage
    }
    
    fn priority(&self) -> u32 {
        80 // High priority for many blobs
    }
    
    fn estimate_memory_usage(&self, _blob_count: u32) -> u64 {
        // Minimal memory usage since we don't store blobs
        1024 * 1024 // 1MB base overhead
    }
}