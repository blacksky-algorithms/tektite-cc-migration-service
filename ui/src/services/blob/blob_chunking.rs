//! Intelligent Blob Chunking System
//!
//! This module provides adaptive blob chunking capabilities for large blob handling
//! across different storage backends. It intelligently divides large blobs into
//! optimally-sized chunks based on backend capabilities and WASM memory constraints.

use crate::{console_debug, console_error, console_info};
use serde::{Deserialize, Serialize};

// Conservative chunk size constants based on Safari's proven limits
// Applied universally across all browsers for consistent behavior

// OPFS unified limits (Safari conservative)
const OPFS_MAX: usize = 50 * 1024 * 1024; // 50MB
const OPFS_OPTIMAL: usize = 25 * 1024 * 1024; // 25MB
const OPFS_MIN: usize = 5 * 1024 * 1024; // 5MB

// IndexedDB unified limits (Safari conservative)
const IDB_MAX: usize = 25 * 1024 * 1024; // 25MB
const IDB_OPTIMAL: usize = 10 * 1024 * 1024; // 10MB
const IDB_MIN: usize = 2 * 1024 * 1024; // 2MB

// LocalStorage limits (already very conservative)
const LS_MAX: usize = 1024 * 1024; // 1MB
const LS_OPTIMAL: usize = 512 * 1024; // 512KB
const LS_MIN: usize = 256 * 1024; // 256KB

/// Chunk metadata for tracking blob pieces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobChunk {
    pub parent_cid: String,
    pub chunk_id: String,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub chunk_size: u64,
    pub data: Vec<u8>,
}

/// Information about a chunked blob
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkedBlobInfo {
    pub original_cid: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub chunk_size: u64,
    pub chunk_cids: Vec<String>,
    pub created_at: u64,
}

/// Chunking strategy configuration with const generic chunk sizes
#[derive(Debug, Clone)]
pub struct ChunkingConfig<const OPTIMAL_SIZE: usize, const MAX_SIZE: usize, const MIN_SIZE: usize> {
    pub backend_name: String,
}

// Type aliases for specific backend configurations
pub type OpfsChunkingConfig = ChunkingConfig<OPFS_OPTIMAL, OPFS_MAX, OPFS_MIN>;
pub type IndexedDbChunkingConfig = ChunkingConfig<IDB_OPTIMAL, IDB_MAX, IDB_MIN>;
pub type LocalStorageChunkingConfig = ChunkingConfig<LS_OPTIMAL, LS_MAX, LS_MIN>;

impl<const OPTIMAL_SIZE: usize, const MAX_SIZE: usize, const MIN_SIZE: usize>
    ChunkingConfig<OPTIMAL_SIZE, MAX_SIZE, MIN_SIZE>
{
    /// Const generic accessors for chunk sizes
    pub const fn max_chunk_size(&self) -> u64 {
        MAX_SIZE as u64
    }

    pub const fn optimal_chunk_size(&self) -> u64 {
        OPTIMAL_SIZE as u64
    }

    pub const fn min_chunk_size(&self) -> u64 {
        MIN_SIZE as u64
    }
}

// Factory methods for specific backend configurations
impl Default for OpfsChunkingConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl OpfsChunkingConfig {
    pub fn new() -> Self {
        Self {
            backend_name: "OPFS".to_string(),
        }
    }
}

impl Default for IndexedDbChunkingConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexedDbChunkingConfig {
    pub fn new() -> Self {
        Self {
            backend_name: "IndexedDB".to_string(),
        }
    }
}

impl Default for LocalStorageChunkingConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalStorageChunkingConfig {
    pub fn new() -> Self {
        Self {
            backend_name: "LocalStorage".to_string(),
        }
    }
}

/// Helper function to create appropriate config for backend
pub fn create_chunking_config(backend_name: &str) -> Box<dyn ChunkingConfigTrait> {
    match backend_name {
        "OPFS" => Box::new(OpfsChunkingConfig::new()),
        "IndexedDB" => Box::new(IndexedDbChunkingConfig::new()),
        "LocalStorage" => Box::new(LocalStorageChunkingConfig::new()),
        _ => Box::new(OpfsChunkingConfig::new()), // Default to OPFS
    }
}

/// Trait for unified access to chunking config methods
pub trait ChunkingConfigTrait {
    fn max_chunk_size(&self) -> u64;
    fn optimal_chunk_size(&self) -> u64;
    fn min_chunk_size(&self) -> u64;
    fn backend_name(&self) -> &str;
    fn should_chunk_blob(&self, blob_size: u64) -> bool;
    fn calculate_optimal_chunks(&self, blob_size: u64) -> u32;
}

impl<const OPTIMAL_SIZE: usize, const MAX_SIZE: usize, const MIN_SIZE: usize> ChunkingConfigTrait
    for ChunkingConfig<OPTIMAL_SIZE, MAX_SIZE, MIN_SIZE>
{
    fn max_chunk_size(&self) -> u64 {
        MAX_SIZE as u64
    }

    fn optimal_chunk_size(&self) -> u64 {
        OPTIMAL_SIZE as u64
    }

    fn min_chunk_size(&self) -> u64 {
        MIN_SIZE as u64
    }

    fn backend_name(&self) -> &str {
        &self.backend_name
    }

    fn should_chunk_blob(&self, blob_size: u64) -> bool {
        blob_size > self.optimal_chunk_size()
    }

    fn calculate_optimal_chunks(&self, blob_size: u64) -> u32 {
        if blob_size <= self.optimal_chunk_size() {
            return 1;
        }

        // Calculate chunks needed, ensuring each chunk is within limits
        let chunks_needed = blob_size.div_ceil(self.optimal_chunk_size());
        chunks_needed.max(1) as u32
    }
}

/// Intelligent blob chunker with adaptive sizing
pub struct BlobChunker {
    config: Box<dyn ChunkingConfigTrait>,
}

impl BlobChunker {
    /// Create a new blob chunker for a specific backend
    pub fn new(backend_name: &str) -> Self {
        let config = create_chunking_config(backend_name);
        console_info!(
            "{}",
            format!("🧩 [BlobChunker] Initialized for {} backend", backend_name)
        );
        console_debug!(
            "{}",
            format!(
                "📊 [BlobChunker] Config: max={:.1}MB, optimal={:.1}MB, min={:.1}MB",
                config.max_chunk_size() as f64 / 1_048_576.0,
                config.optimal_chunk_size() as f64 / 1_048_576.0,
                config.min_chunk_size() as f64 / 1_048_576.0
            )
        );

        Self { config }
    }

    /// Analyze a blob and determine if chunking would be beneficial
    pub fn analyze_blob(&self, blob_size: u64) -> BlobAnalysis {
        console_debug!(
            "{}",
            format!(
                "🔍 [BlobChunker] Analyzing blob of {:.2} MB",
                blob_size as f64 / 1_048_576.0
            )
        );

        let should_chunk = self.config.should_chunk_blob(blob_size);
        let recommended_chunks = if should_chunk {
            self.config.calculate_optimal_chunks(blob_size)
        } else {
            1
        };

        let chunk_size = if recommended_chunks > 1 {
            blob_size.div_ceil(recommended_chunks as u64)
        } else {
            blob_size
        };

        let analysis = BlobAnalysis {
            blob_size,
            should_chunk,
            recommended_chunks,
            estimated_chunk_size: chunk_size,
            memory_efficiency_gain: if should_chunk {
                ((blob_size as f64 - chunk_size as f64) / blob_size as f64 * 100.0) as u32
            } else {
                0
            },
            backend_compatibility: match self.config.backend_name() {
                "OPFS" => {
                    if blob_size <= self.config.max_chunk_size() {
                        "Optimal"
                    } else {
                        "Chunking Required"
                    }
                }
                "IndexedDB" => {
                    if blob_size <= self.config.max_chunk_size() {
                        "Good"
                    } else {
                        "Chunking Required"
                    }
                }
                "LocalStorage" => {
                    if blob_size <= self.config.optimal_chunk_size() {
                        "Acceptable"
                    } else {
                        "Chunking Essential"
                    }
                }
                _ => "Unknown",
            }
            .to_string(),
        };

        console_info!("{}", format!("📋 [BlobChunker] Analysis: {} chunks recommended, {:.2} MB per chunk, {}% memory efficiency gain",
                       recommended_chunks, chunk_size as f64 / 1_048_576.0, analysis.memory_efficiency_gain));

        analysis
    }

    /// Split a large blob into optimally-sized chunks
    pub async fn chunk_blob(&self, cid: &str, data: Vec<u8>) -> Result<Vec<BlobChunk>, String> {
        let blob_size = data.len() as u64;
        console_info!(
            "{}",
            format!(
                "✂️ [BlobChunker] Chunking blob {} ({:.2} MB)",
                cid,
                blob_size as f64 / 1_048_576.0
            )
        );

        let analysis = self.analyze_blob(blob_size);

        if !analysis.should_chunk {
            console_debug!(
                "{}",
                format!(
                    "📦 [BlobChunker] Blob {} doesn't need chunking, returning as single chunk",
                    cid
                )
            );
            return Ok(vec![BlobChunk {
                parent_cid: cid.to_string(),
                chunk_id: format!("{}_chunk_0", cid),
                chunk_index: 0,
                total_chunks: 1,
                chunk_size: blob_size,
                data,
            }]);
        }

        let chunk_size = analysis.estimated_chunk_size as usize;
        let total_chunks = analysis.recommended_chunks;
        let mut chunks = Vec::new();

        console_info!(
            "{}",
            format!(
                "🔧 [BlobChunker] Creating {} chunks of ~{:.2} MB each",
                total_chunks,
                chunk_size as f64 / 1_048_576.0
            )
        );

        for (chunk_index, data_chunk) in data.chunks(chunk_size).enumerate() {
            let chunk_id = format!("{}_chunk_{}", cid, chunk_index);
            let chunk = BlobChunk {
                parent_cid: cid.to_string(),
                chunk_id: chunk_id.clone(),
                chunk_index: chunk_index as u32,
                total_chunks,
                chunk_size: data_chunk.len() as u64,
                data: data_chunk.to_vec(),
            };

            console_debug!(
                "{}",
                format!(
                    "📦 [BlobChunker] Created chunk {} ({:.2} MB)",
                    chunk_id,
                    chunk.chunk_size as f64 / 1_048_576.0
                )
            );
            chunks.push(chunk);
        }

        console_info!(
            "{}",
            format!(
                "✅ [BlobChunker] Successfully chunked blob {} into {} pieces",
                cid,
                chunks.len()
            )
        );
        Ok(chunks)
    }

    /// Reassemble chunks back into the original blob
    pub async fn reassemble_chunks(&self, chunks: Vec<BlobChunk>) -> Result<Vec<u8>, String> {
        if chunks.is_empty() {
            return Err("No chunks provided for reassembly".to_string());
        }

        let parent_cid = chunks[0].parent_cid.clone();
        let expected_total = chunks[0].total_chunks;

        console_info!(
            "{}",
            format!(
                "🔧 [BlobChunker] Reassembling {} chunks for blob {}",
                chunks.len(),
                &parent_cid
            )
        );

        if chunks.len() != expected_total as usize {
            console_error!(
                "{}",
                format!(
                    "❌ [BlobChunker] Chunk count mismatch: expected {}, got {}",
                    expected_total,
                    chunks.len()
                )
            );
            return Err(format!(
                "Chunk count mismatch: expected {}, got {}",
                expected_total,
                chunks.len()
            ));
        }

        // Sort chunks by index to ensure correct order
        let mut sorted_chunks = chunks;
        sorted_chunks.sort_by_key(|chunk| chunk.chunk_index);

        // Validate chunk sequence
        for (i, chunk) in sorted_chunks.iter().enumerate() {
            if chunk.chunk_index != i as u32 {
                console_error!(
                    "{}",
                    format!(
                        "❌ [BlobChunker] Chunk sequence error: expected index {}, got {}",
                        i, chunk.chunk_index
                    )
                );
                return Err(format!(
                    "Chunk sequence error: expected index {}, got {}",
                    i, chunk.chunk_index
                ));
            }

            if chunk.parent_cid != parent_cid {
                console_error!(
                    "{}",
                    format!(
                        "❌ [BlobChunker] Parent CID mismatch: expected {}, got {}",
                        &parent_cid, &chunk.parent_cid
                    )
                );
                return Err(format!(
                    "Parent CID mismatch: expected {}, got {}",
                    parent_cid, chunk.parent_cid
                ));
            }
        }

        // Reassemble data
        let mut reassembled_data = Vec::new();
        let mut total_size = 0u64;

        for chunk in sorted_chunks {
            console_debug!(
                "{}",
                format!(
                    "🔧 [BlobChunker] Adding chunk {} ({:.2} MB)",
                    chunk.chunk_index,
                    chunk.chunk_size as f64 / 1_048_576.0
                )
            );
            reassembled_data.extend(chunk.data);
            total_size += chunk.chunk_size;
        }

        console_info!(
            "{}",
            format!(
                "✅ [BlobChunker] Successfully reassembled blob {} ({:.2} MB total)",
                &parent_cid,
                total_size as f64 / 1_048_576.0
            )
        );
        Ok(reassembled_data)
    }

    /// Get chunking configuration for this backend
    pub fn get_config(&self) -> &dyn ChunkingConfigTrait {
        &*self.config
    }
}

/// Results of blob analysis for chunking decisions
#[derive(Debug, Clone)]
pub struct BlobAnalysis {
    pub blob_size: u64,
    pub should_chunk: bool,
    pub recommended_chunks: u32,
    pub estimated_chunk_size: u64,
    pub memory_efficiency_gain: u32, // Percentage
    pub backend_compatibility: String,
}

impl BlobAnalysis {
    /// Get a human-readable summary of the analysis
    pub fn summary(&self) -> String {
        if self.should_chunk {
            format!(
                "{:.2} MB blob → {} chunks of {:.2} MB each ({}% memory efficiency gain, {})",
                self.blob_size as f64 / 1_048_576.0,
                self.recommended_chunks,
                self.estimated_chunk_size as f64 / 1_048_576.0,
                self.memory_efficiency_gain,
                self.backend_compatibility
            )
        } else {
            format!(
                "{:.2} MB blob → no chunking needed ({})",
                self.blob_size as f64 / 1_048_576.0,
                self.backend_compatibility
            )
        }
    }
}

/// Helper functions for chunk management
pub mod chunk_utils {
    use super::*;

    /// Generate a unique chunk ID
    pub fn generate_chunk_id(parent_cid: &str, chunk_index: u32) -> String {
        format!("{}_chunk_{:04}", parent_cid, chunk_index)
    }

    /// Parse chunk information from a chunk ID
    pub fn parse_chunk_id(chunk_id: &str) -> Option<(String, u32)> {
        if let Some(last_underscore) = chunk_id.rfind('_') {
            if let Some(second_last_underscore) = chunk_id[..last_underscore].rfind('_') {
                let parent_cid = &chunk_id[..second_last_underscore];
                let chunk_index_str = &chunk_id[last_underscore + 1..];
                if let Ok(chunk_index) = chunk_index_str.parse::<u32>() {
                    return Some((parent_cid.to_string(), chunk_index));
                }
            }
        }
        None
    }

    /// Calculate total size from chunk information
    pub fn calculate_total_size(chunks: &[BlobChunk]) -> u64 {
        chunks.iter().map(|chunk| chunk.chunk_size).sum()
    }

    /// Validate chunk integrity
    pub fn validate_chunks(chunks: &[BlobChunk]) -> Result<(), String> {
        if chunks.is_empty() {
            return Err("No chunks to validate".to_string());
        }

        let parent_cid = &chunks[0].parent_cid;
        let expected_total = chunks[0].total_chunks;

        // Check count
        if chunks.len() != expected_total as usize {
            return Err(format!(
                "Chunk count mismatch: expected {}, got {}",
                expected_total,
                chunks.len()
            ));
        }

        // Check sequence and consistency
        for (i, chunk) in chunks.iter().enumerate() {
            if chunk.chunk_index != i as u32 {
                return Err(format!(
                    "Chunk index mismatch at position {}: expected {}, got {}",
                    i, i, chunk.chunk_index
                ));
            }

            if chunk.parent_cid != *parent_cid {
                return Err(format!(
                    "Parent CID mismatch in chunk {}: expected {}, got {}",
                    i, parent_cid, chunk.parent_cid
                ));
            }

            if chunk.total_chunks != expected_total {
                return Err(format!(
                    "Total chunks mismatch in chunk {}: expected {}, got {}",
                    i, expected_total, chunk.total_chunks
                ));
            }
        }

        Ok(())
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_chunking_config_creation() {
//         let opfs_config = ChunkingConfig::for_backend("OPFS");
//         assert_eq!(opfs_config.backend_name, "OPFS");
//         assert!(opfs_config.max_chunk_size > opfs_config.optimal_chunk_size);

//         let idb_config = ChunkingConfig::for_backend("IndexedDB");
//         assert_eq!(idb_config.backend_name, "IndexedDB");
//         assert!(idb_config.max_chunk_size < opfs_config.max_chunk_size);

//         let ls_config = ChunkingConfig::for_backend("LocalStorage");
//         assert_eq!(ls_config.backend_name, "LocalStorage");
//         assert!(ls_config.max_chunk_size < idb_config.max_chunk_size);
//     }

//     #[test]
//     fn test_chunk_id_parsing() {
//         let chunk_id = "test_cid!23_chunk_0042";
//         let (parent_cid, chunk_index) = chunk_utils::parse_chunk_id(chunk_id).unwrap();
//         assert_eq!(parent_cid, "test_cid!23");
//         assert_eq!(chunk_index, 42);
//     }

//     #[test]
//     fn test_blob_analysis() {
//         let chunker = BlobChunker::new("OPFS");
//         let analysis = chunker.analyze_blob(100 * 1024 * 1024); // 100MB

//         assert!(analysis.should_chunk);
//         assert!(analysis.recommended_chunks > 1);
//         assert!(analysis.estimated_chunk_size <= chunker.config.optimal_chunk_size);
//     }

//     #[tokio::test]
//     async fn test_chunk_and_reassemble() {
//         let chunker = BlobChunker::new("IndexedDB");
//         let test_data = vec![0u8; 100_000]; // 100KB test data
//         let cid = "test_blob!23";

//         let chunks = chunker.chunk_blob(cid, test_data.clone()).await.unwrap();
//         let reassembled = chunker.reassemble_chunks(chunks).await.unwrap();

//         assert_eq!(test_data, reassembled);
//     }
// }
