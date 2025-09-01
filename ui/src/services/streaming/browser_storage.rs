//! Browser storage implementation using OPFS + IndexedDB with opfs crate

use crate::services::streaming::traits::{DataChunk, StorageBackend};
use crate::{console_debug, console_error, console_info, console_warn};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use opfs::persistent::{app_specific_dir, DirectoryHandle};
use opfs::{
    CreateWritableOptions, DirectoryHandle as _, FileHandle as _, GetDirectoryHandleOptions,
    GetFileHandleOptions, WritableFileStream as _,
};
use rexie::{ObjectStore, Rexie, TransactionMode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

#[derive(Debug, Serialize, Deserialize)]
struct StoredChunk {
    id: String,
    offset: usize,
    data: Vec<u8>,
}

/// Browser storage backend supporting both OPFS and IndexedDB using the opfs crate
pub struct BrowserStorage {
    db: Rexie,
    opfs_root: Option<DirectoryHandle>,
    buffers: HashMap<String, Vec<u8>>,
}

impl BrowserStorage {
    pub async fn new() -> Result<Self, String> {
        // Initialize IndexedDB
        let db = Rexie::builder("atproto-sync")
            .version(1)
            .add_object_store(
                ObjectStore::new("chunks")
                    .key_path("id")
                    .auto_increment(false),
            )
            .add_object_store(
                ObjectStore::new("repos")
                    .key_path("did")
                    .auto_increment(false),
            )
            .add_object_store(
                ObjectStore::new("blobs")
                    .key_path("cid")
                    .auto_increment(false),
            )
            .build()
            .await
            .map_err(|e| format!("Failed to open IndexedDB: {:?}", e))?;

        // Try to initialize OPFS
        let opfs_root = match app_specific_dir().await {
            Ok(root) => {
                console_info!("OPFS available, using for primary storage");
                Some(root)
            }
            Err(_) => {
                console_warn!("OPFS not available, falling back to IndexedDB");
                None
            }
        };

        Ok(Self {
            db,
            opfs_root,
            buffers: HashMap::new(),
        })
    }

    /// Write a chunk of data to storage
    pub async fn write_chunk(&self, id: &str, offset: usize, data: &[u8]) -> Result<(), String> {
        if let Some(ref root) = self.opfs_root {
            self.write_to_opfs_with_crate(root, id, offset, data).await
        } else {
            self.write_to_indexeddb(id, offset, data).await
        }
    }

    async fn write_to_opfs_with_crate(
        &self,
        root: &DirectoryHandle,
        id: &str,
        offset: usize,
        data: &[u8],
    ) -> Result<(), String> {
        // Get or create directory for sync data
        let sync_dir_options = GetDirectoryHandleOptions { create: true };
        let sync_dir = root
            .get_directory_handle_with_options("atproto-sync", &sync_dir_options)
            .await
            .map_err(|e| format!("Failed to get sync directory: {:?}", e))?;

        // Get or create file for this ID
        let file_name = format!("{}.data", id);
        let file_options = GetFileHandleOptions { create: true };
        let mut file = sync_dir
            .get_file_handle_with_options(&file_name, &file_options)
            .await
            .map_err(|e| format!("Failed to get file: {:?}", e))?;

        // Create a writable stream
        let writable_options = CreateWritableOptions {
            keep_existing_data: true,
        };
        let mut writable = file
            .create_writable_with_options(&writable_options)
            .await
            .map_err(|e| format!("Failed to create writable: {:?}", e))?;

        // Seek to the offset if needed
        if offset > 0 {
            writable
                .seek(offset)
                .await
                .map_err(|e| format!("Failed to seek: {:?}", e))?;
        }

        // Write the data
        writable
            .write_at_cursor_pos(data.to_vec())
            .await
            .map_err(|e| format!("Failed to write data: {:?}", e))?;

        // Close the stream (commits the write)
        writable
            .close()
            .await
            .map_err(|e| format!("Failed to close writable: {:?}", e))?;

        Ok(())
    }

    async fn write_to_indexeddb(&self, id: &str, offset: usize, data: &[u8]) -> Result<(), String> {
        let tx = self
            .db
            .transaction(&["chunks"], TransactionMode::ReadWrite)
            .map_err(|e| format!("Failed to create transaction: {:?}", e))?;

        let store = tx
            .store("chunks")
            .map_err(|e| format!("Failed to get store: {:?}", e))?;

        let chunk = StoredChunk {
            id: format!("{}-{}", id, offset),
            offset,
            data: data.to_vec(),
        };

        let value = serde_wasm_bindgen::to_value(&chunk)
            .map_err(|e| format!("Failed to serialize: {:?}", e))?;

        store
            .put(&value, None)
            .await
            .map_err(|e| format!("Failed to put: {:?}", e))?;

        tx.done()
            .await
            .map_err(|e| format!("Transaction failed: {:?}", e))?;

        Ok(())
    }

    /// Read all data for an ID
    pub async fn read_data(&self, id: &str) -> Result<Vec<u8>, String> {
        if let Some(ref root) = self.opfs_root {
            self.read_from_opfs(root, id).await
        } else {
            self.read_from_indexeddb(id).await
        }
    }

    /// Read back from OPFS using the opfs crate
    async fn read_from_opfs(&self, root: &DirectoryHandle, id: &str) -> Result<Vec<u8>, String> {
        let sync_dir_options = GetDirectoryHandleOptions { create: false };
        let sync_dir = root
            .get_directory_handle_with_options("atproto-sync", &sync_dir_options)
            .await
            .map_err(|e| format!("Failed to get directory: {:?}", e))?;

        let file_name = format!("{}.data", id);
        let file_options = GetFileHandleOptions { create: false };
        let file = sync_dir
            .get_file_handle_with_options(&file_name, &file_options)
            .await
            .map_err(|e| format!("Failed to get file: {:?}", e))?;

        let data = file
            .read()
            .await
            .map_err(|e| format!("Failed to read file: {:?}", e))?;

        Ok(data)
    }

    async fn read_from_indexeddb(&self, id: &str) -> Result<Vec<u8>, String> {
        let tx = self
            .db
            .transaction(&["chunks"], TransactionMode::ReadOnly)
            .map_err(|e| format!("Failed to create transaction: {:?}", e))?;

        let store = tx
            .store("chunks")
            .map_err(|e| format!("Failed to get store: {:?}", e))?;

        // Get all chunks for this ID (simplified - in real implementation would handle pagination)
        let all_values = store
            .get_all(None, None, Some(100), None)
            .await
            .map_err(|e| format!("Failed to get chunks: {:?}", e))?;

        let mut chunks = Vec::new();
        for (_, value) in all_values {
            if let Ok(chunk) = serde_wasm_bindgen::from_value::<StoredChunk>(value) {
                if chunk.id.starts_with(id) {
                    chunks.push((chunk.offset, chunk.data));
                }
            }
        }

        // Sort by offset and combine
        chunks.sort_by_key(|(offset, _)| *offset);
        let mut result = Vec::new();
        for (_, data) in chunks {
            result.extend(data);
        }

        Ok(result)
    }

    /// Write a complete stream to OPFS, useful for repo/blob sync
    pub async fn write_stream_to_opfs(
        &self,
        id: &str,
        mut data_stream: impl Stream<Item = Result<Bytes, String>> + Unpin,
    ) -> Result<(), String> {
        let root = self.opfs_root.as_ref().ok_or("OPFS not available")?;

        let sync_dir_options = GetDirectoryHandleOptions { create: true };
        let sync_dir = root
            .get_directory_handle_with_options("atproto-sync", &sync_dir_options)
            .await
            .map_err(|e| format!("Failed to get directory: {:?}", e))?;

        let file_name = format!("{}.data", id);
        let file_options = GetFileHandleOptions { create: true };
        let mut file = sync_dir
            .get_file_handle_with_options(&file_name, &file_options)
            .await
            .map_err(|e| format!("Failed to get file: {:?}", e))?;

        let writable_options = CreateWritableOptions {
            keep_existing_data: false,
        };
        let mut writable = file
            .create_writable_with_options(&writable_options)
            .await
            .map_err(|e| format!("Failed to create writable: {:?}", e))?;

        // Stream data directly to file
        while let Some(chunk_result) = data_stream.next().await {
            let chunk = chunk_result?;
            writable
                .write_at_cursor_pos(chunk.to_vec())
                .await
                .map_err(|e| format!("Failed to write chunk: {:?}", e))?;
        }

        writable
            .close()
            .await
            .map_err(|e| format!("Failed to close: {:?}", e))?;

        Ok(())
    }

    /// Delete from OPFS or IndexedDB
    pub async fn delete(&self, id: &str) -> Result<(), String> {
        if let Some(ref root) = self.opfs_root {
            self.delete_from_opfs(root, id).await
        } else {
            self.delete_from_indexeddb(id).await
        }
    }

    /// Delete from OPFS
    async fn delete_from_opfs(&self, root: &DirectoryHandle, id: &str) -> Result<(), String> {
        let sync_dir_options = GetDirectoryHandleOptions { create: false };
        let mut sync_dir = root
            .get_directory_handle_with_options("atproto-sync", &sync_dir_options)
            .await
            .map_err(|e| format!("Failed to get directory: {:?}", e))?;

        let file_name = format!("{}.data", id);
        sync_dir
            .remove_entry(&file_name)
            .await
            .map_err(|e| format!("Failed to delete file: {:?}", e))?;

        Ok(())
    }

    /// Delete from IndexedDB
    async fn delete_from_indexeddb(&self, id: &str) -> Result<(), String> {
        let tx = self
            .db
            .transaction(&["chunks"], TransactionMode::ReadWrite)
            .map_err(|e| format!("Failed to create transaction: {:?}", e))?;

        let store = tx
            .store("chunks")
            .map_err(|e| format!("Failed to get store: {:?}", e))?;

        // Delete all chunks for this ID
        let all_values = store
            .get_all(None, None, Some(1000), None)
            .await
            .map_err(|e| format!("Failed to get chunks: {:?}", e))?;

        for (key, value) in all_values {
            if let Ok(chunk) = serde_wasm_bindgen::from_value::<StoredChunk>(value) {
                if chunk.id.starts_with(id) {
                    store
                        .delete(&key)
                        .await
                        .map_err(|e| format!("Failed to delete chunk: {:?}", e))?;
                }
            }
        }

        tx.done()
            .await
            .map_err(|e| format!("Transaction failed: {:?}", e))?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl StorageBackend for BrowserStorage {
    async fn write_chunk(&mut self, chunk: &DataChunk) -> Result<(), Box<dyn Error>> {
        let chunk_size = chunk.data.len();
        console_debug!(
            "[BrowserStorage] Writing chunk for {} at offset {} ({} bytes)",
            chunk.id,
            chunk.offset,
            chunk_size
        );

        // For OPFS, we can write directly; for IndexedDB, buffer in memory first
        if self.opfs_root.is_some() {
            console_debug!(
                "[BrowserStorage] Using OPFS for {} chunk at offset {}",
                chunk.id,
                chunk.offset
            );
            // Direct write to OPFS
            BrowserStorage::write_chunk(self, &chunk.id, chunk.offset, &chunk.data)
                .await
                .map_err(|e| {
                    console_error!("[BrowserStorage] OPFS write failed for {}: {}", chunk.id, e);
                    e.into()
                })
        } else {
            console_debug!(
                "[BrowserStorage] Using memory buffer for {} chunk at offset {}",
                chunk.id,
                chunk.offset
            );
            // Buffer chunks in memory for IndexedDB
            let buffer = self.buffers.entry(chunk.id.clone()).or_default();
            let old_buffer_size = buffer.len();

            // Ensure buffer is large enough
            let required_size = chunk.offset + chunk.data.len();
            if buffer.len() < required_size {
                buffer.resize(required_size, 0);
                console_debug!(
                    "[BrowserStorage] Expanded buffer for {} from {} to {} bytes",
                    chunk.id,
                    old_buffer_size,
                    required_size
                );
            }

            // Write chunk data at correct offset
            buffer[chunk.offset..chunk.offset + chunk.data.len()].copy_from_slice(&chunk.data);
            console_debug!(
                "[BrowserStorage] Wrote {} bytes to buffer for {} at offset {}",
                chunk_size,
                chunk.id,
                chunk.offset
            );

            // Memory usage tracking
            let total_buffer_size: usize = self.buffers.values().map(|b| b.len()).sum();
            if total_buffer_size > 10 * 1024 * 1024 {
                // Log if over 10MB
                console_warn!(
                    "[BrowserStorage] High memory usage: {} bytes total in buffers",
                    total_buffer_size
                );
            }

            Ok(())
        }
    }

    async fn finalize(&mut self, id: &str) -> Result<(), Box<dyn Error>> {
        console_info!("[BrowserStorage] Finalizing storage for {}", id);

        if self.opfs_root.is_some() {
            console_debug!("[BrowserStorage] OPFS writes already finalized for {}", id);
            // OPFS writes are already finalized
            Ok(())
        } else {
            // Write buffered data to IndexedDB
            if let Some(buffer) = self.buffers.remove(id) {
                let buffer_size = buffer.len();
                console_info!(
                    "[BrowserStorage] Writing {} bytes from buffer to IndexedDB for {}",
                    buffer_size,
                    id
                );

                BrowserStorage::write_chunk(self, id, 0, &buffer)
                    .await
                    .map_err(|e| -> Box<dyn Error> {
                        console_error!(
                            "[BrowserStorage] IndexedDB finalize failed for {}: {}",
                            id,
                            e
                        );
                        e.into()
                    })?;

                console_info!(
                    "[BrowserStorage] Successfully wrote {} bytes to IndexedDB for {}",
                    buffer_size,
                    id
                );
                Ok(())
            } else {
                console_warn!("[BrowserStorage] No buffer found to finalize for {}", id);
                Ok(())
            }
        }
    }

    async fn read_data(&self, id: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        console_info!("[BrowserStorage] Reading data for {}", id);

        // Check buffer first (for IndexedDB case)
        if let Some(buffer) = self.buffers.get(id) {
            let buffer_size = buffer.len();
            console_info!(
                "[BrowserStorage] Found {} bytes in memory buffer for {}",
                buffer_size,
                id
            );
            return Ok(buffer.clone());
        }

        // Otherwise read from storage
        console_debug!("[BrowserStorage] Reading from storage backend for {}", id);
        let data = BrowserStorage::read_data(self, id)
            .await
            .map_err(|e| -> Box<dyn Error> {
                console_error!("[BrowserStorage] Storage read failed for {}: {}", id, e);
                e.into()
            })?;

        let data_size = data.len();
        console_info!(
            "[BrowserStorage] Successfully read {} bytes from storage for {}",
            data_size,
            id
        );
        Ok(data)
    }
}
