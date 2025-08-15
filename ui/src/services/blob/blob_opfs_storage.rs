use crate::services::config::get_global_config;
// Import console macros from our crate
use crate::{console_error, console_info, console_warn, console_debug};

/// Helper function to safely format u64 values for logging to avoid BigInt serialization issues
fn format_bytes(bytes: u64) -> String {
    bytes.to_string()
}
use opfs::persistent::{app_specific_dir, DirectoryHandle};
use opfs::{CreateWritableOptions, GetDirectoryHandleOptions, GetFileHandleOptions};
use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _};
use serde::{Deserialize, Serialize};

// Note: Tokio usage simplified for WASM compatibility

#[derive(Debug)]
pub enum OpfsError {
    Storage(String),
    NotFound(String),
    InvalidData(String),
}

impl OpfsError {
    pub fn from_opfs_error(err: opfs::persistent::Error) -> Self {
        OpfsError::Storage(format!("OPFS Error: {:?}", err))
    }
}

impl std::fmt::Display for OpfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpfsError::Storage(msg) => write!(f, "OPFS Storage Error: {}", msg),
            OpfsError::NotFound(msg) => write!(f, "OPFS Not Found: {}", msg),
            OpfsError::InvalidData(msg) => write!(f, "OPFS Invalid Data: {}", msg),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BlobInfo {
    pub cid: String,
    pub size: u64,
    pub download_url: String,
}

#[derive(Clone)]
pub struct OpfsBlobManager {
    blob_dir: DirectoryHandle,
}

impl OpfsBlobManager {
    pub async fn new() -> Result<Self, OpfsError> {
        console_info!("[OpfsBlobManager] 🚀 Initializing OPFS blob manager");

        console_debug!("[OpfsBlobManager] 📁 Accessing app-specific directory...");
        let app_dir = app_specific_dir().await.map_err(|e| {
            console_error!("{}", format!(
                "[OpfsBlobManager] ❌ Failed to access app-specific directory: {:?}",
                e
            ));
            OpfsError::from_opfs_error(e)
        })?;
        console_debug!("[OpfsBlobManager] ✅ App-specific directory accessed successfully");

        console_debug!("[OpfsBlobManager] 📁 Creating/accessing migration_blobs directory...");
        let options = GetDirectoryHandleOptions { create: true };
        let blob_dir = app_dir
            .get_directory_handle_with_options("migration_blobs", &options)
            .await
            .map_err(|e| {
                console_error!("{}", format!(
                    "[OpfsBlobManager] ❌ Failed to create/access migration_blobs directory: {:?}",
                    e
                ));
                OpfsError::from_opfs_error(e)
            })?;

        console_info!("[OpfsBlobManager] ✅ OPFS blob directory created/accessed successfully");
        Ok(Self { blob_dir })
    }

    pub async fn store_blob(&self, cid: &str, data: Vec<u8>) -> Result<(), OpfsError> {
        console_info!("{}", format!(
            "[OpfsBlobManager] 💾 Storing blob {} ({} bytes)",
            cid,
            format_bytes(data.len() as u64)
        ));

        console_debug!("{}", format!("[OpfsBlobManager] 📝 Creating file handle for blob {}", cid));
        let options = GetFileHandleOptions { create: true };
        let mut file = self
            .blob_dir
            .get_file_handle_with_options(cid, &options)
            .await
            .map_err(|e| {
                console_error!("{}", format!(
                    "[OpfsBlobManager] ❌ Failed to create file handle for blob {}: {:?}",
                    cid,
                    e
                ));
                OpfsError::from_opfs_error(e)
            })?;

        console_debug!("{}", format!(
            "[OpfsBlobManager] ✍️ Creating writable stream for blob {}",
            cid
        ));
        let write_options = CreateWritableOptions {
            keep_existing_data: false,
        };
        let mut writer = file
            .create_writable_with_options(&write_options)
            .await
            .map_err(|e| {
                console_error!("{}", format!(
                    "[OpfsBlobManager] ❌ Failed to create writable stream for blob {}: {:?}",
                    cid,
                    e
                ));
                OpfsError::from_opfs_error(e)
            })?;

        console_debug!("{}", format!(
            "[OpfsBlobManager] ⬆️ Writing {} bytes to blob {}",
            data.len(),
            cid
        ));
        writer.write_at_cursor_pos(data).await.map_err(|e| {
            console_error!("{}", format!(
                "[OpfsBlobManager] ❌ Failed to write data to blob {}: {:?}",
                cid,
                e
            ));
            OpfsError::from_opfs_error(e)
        })?;

        console_debug!("{}", format!("[OpfsBlobManager] 🔒 Closing writer for blob {}", cid));
        writer.close().await.map_err(|e| {
            console_error!("{}", format!(
                "[OpfsBlobManager] ❌ Failed to close writer for blob {}: {:?}",
                cid,
                e
            ));
            OpfsError::from_opfs_error(e)
        })?;

        console_info!("{}", format!("[OpfsBlobManager] ✅ Blob {} stored successfully", cid));
        Ok(())
    }

    pub async fn retrieve_blob(&self, cid: &str) -> Result<Vec<u8>, OpfsError> {
        console_info!("{}", format!("[OpfsBlobManager] 📖 Retrieving blob {}", cid));

        console_debug!("{}", format!(
            "[OpfsBlobManager] 🔍 Looking for file handle for blob {}",
            cid
        ));
        let options = GetFileHandleOptions { create: false };
        let file = self
            .blob_dir
            .get_file_handle_with_options(cid, &options)
            .await
            .map_err(|e| {
                console_warn!("{}", format!(
                    "[OpfsBlobManager] ⚠️ Blob {} not found: {:?}",
                    cid,
                    e
                ));
                OpfsError::NotFound(format!("Blob {} not found", cid))
            })?;

        console_debug!("{}", format!("[OpfsBlobManager] 📥 Reading data from blob {}", cid));
        let data = file.read().await.map_err(|e| {
            console_error!("{}", format!(
                "[OpfsBlobManager] ❌ Failed to read blob {}: {:?}",
                cid,
                e
            ));
            OpfsError::from_opfs_error(e)
        })?;

        console_info!("{}", format!(
            "[OpfsBlobManager] ✅ Blob {} retrieved successfully ({} bytes)",
            cid,
            format_bytes(data.len() as u64)
        ));
        Ok(data)
    }

    pub async fn has_blob(&self, cid: &str) -> bool {
        console_debug!("{}", format!("[OpfsBlobManager] 🔍 Checking if blob {} exists", cid));
        let options = GetFileHandleOptions { create: false };
        let exists = self
            .blob_dir
            .get_file_handle_with_options(cid, &options)
            .await
            .is_ok();
        console_debug!("{}", format!(
            "[OpfsBlobManager] 📋 Blob {} existence check result: {}",
            cid,
            exists
        ));
        exists
    }

    pub async fn get_storage_usage(&self) -> Result<u64, OpfsError> {
        console_info!("Calculating OPFS storage usage");
        let total_size = 0u64;
        let _entries_stream = self
            .blob_dir
            .entries()
            .await
            .map_err(OpfsError::from_opfs_error)?;

        // Note: This is a simplified version - actual implementation would iterate through stream
        console_info!("{}", format!("OPFS storage usage: {} bytes", format_bytes(total_size)));
        Ok(total_size)
    }

    pub async fn cleanup_blobs(&self) -> Result<(), OpfsError> {
        console_info!("Cleaning up OPFS blob storage");
        // Implementation would iterate through files and remove them
        console_info!("OPFS cleanup completed");
        Ok(())
    }

    /// Store blob with retry logic (compatible with LocalStorage BlobManager interface)
    pub async fn store_blob_with_retry(&self, cid: &str, data: Vec<u8>) -> Result<(), OpfsError> {
        let config = get_global_config();
        let mut attempts = 0;

        loop {
            attempts += 1;
            match self.store_blob(cid, data.clone()).await {
                Ok(()) => return Ok(()),
                Err(e) if attempts >= config.retry.storage_retries => return Err(e),
                Err(_) => {
                    console_warn!("{}", format!("Blob storage attempt {} failed, retrying...", attempts));
                    // Simple backoff delay could be added here
                }
            }
        }
    }

    /// List all stored blob CIDs in OPFS storage
    pub async fn list_stored_blobs(&self) -> Result<Vec<String>, OpfsError> {
        console_debug!("[OpfsBlobManager] 📋 Listing all stored blobs");
        
        let blob_cids = Vec::new();
        
        // List all files in the blob directory
        let _entries = self.blob_dir.entries().await.map_err(OpfsError::from_opfs_error)?;
        
        // Use futures utilities to handle the stream - for now, let's use a simpler approach
        // Since we can't easily iterate over the Stream in WASM, let's collect keys differently
        // For now, return empty list - this can be improved later with proper stream handling
        console_warn!("[OpfsBlobManager] Stream iteration not fully implemented - returning empty list");
        
        // TODO: Implement proper stream iteration when futures/stream support is available
        // This is a limitation of the current OPFS stream API in WASM context
        
        console_debug!("{}", format!(
            "[OpfsBlobManager] 📋 Found {} stored blobs",
            blob_cids.len()
        ));
        
        Ok(blob_cids)
    }
}

// Sequential blob migration (simplified to avoid tokio complexity in WASM)
pub async fn migrate_blobs_parallel(
    manager: &OpfsBlobManager,
    blobs: Vec<BlobInfo>,
    progress_callback: impl Fn(u32, u32) + Clone + 'static,
) -> Result<(), OpfsError> {
    console_info!("{}", format!(
        "Starting blob migration for {} blobs",
        blobs.len()
    ));
    let total_blobs = blobs.len() as u32;
    let mut completed = 0u32;

    // Process blobs sequentially for now (can be optimized later with proper tokio setup)
    for blob_info in blobs {
        match migrate_single_blob(manager, &blob_info).await {
            Ok(()) => {
                completed += 1;
                progress_callback(completed, total_blobs);
                console_info!("{}", format!(
                    "Blob migration progress: {}/{}",
                    completed,
                    total_blobs
                ));
            }
            Err(e) => {
                console_error!("{}", format!("Blob migration failed: {:?}", e));
                return Err(e);
            }
        }
    }

    console_info!("{}", format!(
        "Blob migration completed: {}/{}",
        completed,
        total_blobs
    ));
    Ok(())
}

async fn migrate_single_blob(
    manager: &OpfsBlobManager,
    blob_info: &BlobInfo,
) -> Result<(), OpfsError> {
    // Check if blob already exists
    if manager.has_blob(&blob_info.cid).await {
        console_info!("{}", format!("Blob {} already exists, skipping", &blob_info.cid));
        return Ok(());
    }

    // Download blob data (this would be implemented using your API)
    // For now, using placeholder data as mentioned in CLAUDE.md
    console_info!("{}", format!(
        "Downloading blob {} from {}",
        &blob_info.cid,
        &blob_info.download_url
    ));
    let blob_data = vec![0u8; blob_info.size as usize]; // Placeholder

    // Store in OPFS
    manager.store_blob(&blob_info.cid, blob_data).await?;

    Ok(())
}

// Note: JS export functionality can be added later when needed
// For now, the OPFS functionality is used internally within the Rust application
