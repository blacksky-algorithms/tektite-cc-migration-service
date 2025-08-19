# Streaming Services

This module provides the WASM-first streaming infrastructure for the ATProto migration service, implementing the channel-tee pattern described in CLAUDE.md for efficient data migration between Personal Data Servers (PDS).

## Architecture Overview

The streaming architecture enables simultaneous operations:
- **Source PDS** streams data → **Channel-Tee** → **Storage (OPFS/IndexedDB)** + **Target PDS**

This approach minimizes memory usage while maximizing throughput by avoiding the traditional download → store → upload pipeline.

## Core Components

### `traits.rs` - Core Abstractions
- **`DataSource`** - Trait for fetching data streams (repos, blobs)
- **`DataTarget`** - Trait for uploading data to target PDS
- **`StorageBackend`** - Trait for browser storage operations
- **`BrowserStream`** - WASM-compatible stream wrapper using ReadableStreamDefaultReader
- **`ChannelTee`** - Channel-based data duplication for simultaneous operations
- **`DataChunk`** - Generic data container with metadata

### `orchestrator.rs` - Coordination Logic
- **`SyncOrchestrator`** - Main coordination class implementing channel-tee pattern
- **`sync_with_tee()`** - Generic method for streaming data with concurrent storage + upload
- Uses `futures::join!` for WASM-compatible concurrency (no tokio::spawn)

### `wasm_http_client.rs` - Browser HTTP
- **`WasmHttpClient`** - Browser fetch API wrapper
- Streaming GET requests with `BrowserStream` responses
- POST requests for uploads with proper headers
- JSON response handling with serde integration

### `browser_storage.rs` - Hybrid Storage
- **`BrowserStorage`** - OPFS + IndexedDB hybrid storage backend
- Automatic fallback: OPFS (preferred) → IndexedDB (compatibility)
- Chunked writing for large data streams
- Uses `opfs` crate for type-safe OPFS operations

### `implementations.rs` - Concrete Types
- **Repository Migration**:
  - `RepoSource` - Fetches CAR data from source PDS via `com.atproto.sync.getRepo`
  - `RepoTarget` - Uploads CAR data to target PDS via `com.atproto.repo.importRepo`
- **Blob Migration**:
  - `BlobSource` - Fetches blob data from source PDS via `com.atproto.sync.getBlob`
  - `BlobTarget` - Uploads blob data to target PDS via `com.atproto.repo.uploadBlob`
- **Storage**:
  - `BufferedStorage` - Wraps BrowserStorage for the streaming traits

### `metrics.rs` - Performance Monitoring
- **`StreamingMetrics`** - Comprehensive performance tracking
- **`MetricsCollector`** - Real-time metrics collection during streaming
- **`MemoryStats`** - Memory usage monitoring and pressure detection
- **`NetworkStats`** - Network performance and retry tracking
- **`ErrorStats`** - Error classification and recovery rate monitoring

### `errors.rs` - Enhanced Error Handling
- **`StreamingError`** - Detailed error types with context information
- **`RecoverableStreamingError`** - Errors with automatic recovery suggestions
- **`RecoveryStrategy`** - Smart recovery recommendations based on error type
- **`ErrorContext`** - Comprehensive context for error analysis and debugging

## WASM-First Design

### Key WASM Compatibility Features:
- **No Send/Sync bounds** - All traits use `#[async_trait(?Send)]`
- **Browser-native APIs** - Uses fetch, OPFS, IndexedDB instead of native HTTP/filesystem
- **futures::join! instead of tokio::spawn** - WASM-compatible concurrency
- **ReadableStream integration** - Direct browser stream handling
- **Error handling** - String-based errors for WASM compatibility

## Usage Example

```rust
use crate::services::streaming::*;

// Create orchestrator
let orchestrator = SyncOrchestrator::new();

// Set up source, target, and storage
let source = RepoSource::new(&old_session);
let target = RepoTarget::new(&new_session);  
let storage = BufferedStorage::new("repos/did:example:123".to_string()).await?;

// Execute streaming migration with channel-tee
let result = orchestrator.sync_with_tee(source, target, storage).await?;

println!("Migrated {} bytes successfully", result.total_bytes_processed);
```

## Data Flow

```
┌─────────────┐    ┌──────────────┐    ┌─────────────┐
│ Source PDS  │───►│ BrowserStream│───►│ ChannelTee  │
└─────────────┘    └──────────────┘    └─────┬───────┘
                                             │
                        ┌────────────────────┼────────────────────┐
                        ▼                    ▼                    ▼
                ┌───────────────┐    ┌──────────────┐    ┌──────────────┐
                │ StorageBackend│    │              │    │ DataTarget   │
                │ (OPFS/IndexDB)│    │ futures::    │    │ (Target PDS) │
                └───────────────┘    │ join!        │    └──────────────┘
                                     │              │
                                     └──────────────┘
```

## Performance Characteristics

- **Memory Efficient**: Streams data in configurable chunks (default 256KB for browser)
- **Concurrent Operations**: Storage and upload happen simultaneously
- **Browser Optimized**: Uses OPFS for fast storage when available
- **Fallback Support**: Graceful degradation to IndexedDB when OPFS unavailable
- **Progress Tracking**: Built-in progress reporting for UI integration
- **Performance Monitoring**: Real-time metrics collection for transfer rates, memory usage, and error tracking
- **Smart Error Recovery**: Automatic recovery strategies based on error type and network conditions
- **Platform Optimization**: Adaptive configuration for browser, mobile, and desktop environments

## Integration Points

This module integrates with:
- **Migration Steps** (`ui/src/migration/steps/`) - Repository and blob migration
- **Progress Tracking** (`ui/src/migration/progress/`) - Real-time progress updates  
- **Client Services** (`ui/src/services/client/`) - Session management and API calls
- **Configuration** (`ui/src/services/config/`) - Unified configuration with platform optimization
- **UI Components** - Progress bars and migration status display

```                                                                                                           
                                                                                  ┌──────────────────────────┐ 
  ┌──────────────────────┐                                                        │                          │ 
  │                      │                                                        │                          │ 
  │                      │                                                        │                          │ 
  │                      │                                                        │                          │ 
  │                      │                                                        │                          │ 
  │   Source PDS         │                                                        │     Target PDS           │ 
  │                      │                                                        │                          │ 
  │                      │                                                        │                          │ 
  │                      │                                                        │                          │ 
  │                      │                                                        │                          │ 
  └──────────┬───────────┘                                                        └──────────────────────────┘ 
             │                                                                                  ▲              
             │                                                                                  │              
             │                                                                                  │              
             ▼                                                                                  │              
 ┌────────────────────────┐            ┌─────────────────────────┐                ┌─────────────┼────────────┐ 
 │                        │            │                         │                │                          │ 
 │                        │            │                         │                │                          │ 
 │                        │            │                         │                │                          │ 
 │                        │ channel    │                         │  channel       │                          │ 
 │   sync.getRepo       tx├───────────►│rx     storage rx      tx├───────────────►│rx   repo.importRepo      │ 
 │                        │            │                         │                │                          │ 
 │                        │            │                         │                │                          │ 
 │                        │            │                         │                │                          │ 
 │                        │            │                         │                │                          │ 
 │                        │            │                         │                │                          │ 
 └────────────────────────┘            └────────────┬────────────┘                └──────────────────────────┘ 
                                                    │                                                          
                                                    │                                                          
                                                    │                                                          
                                                    │                                                          
                                                    │                                                          
                                                    ▼                                                          
                                        ┌────────────────────────┐                                             
                                        │                        │                                             
                                        │                        │                                             
                                        │     Write to either    │                                             
                                        │                        │                                             
                                        │      OPFS or IndexDB   │                                             
                                        │                        │                                             
                                        │                        │                                             
                                        │                        │                                             
                                        │                        │                                             
                                        │                        │                                             
                                        └────────────────────────┘                                             
```
and 
```                                                                                                           
                                                                                                                                        ┌──────────────────────────┐ 
 ┌──────────────────────┐                                                                                                               │                          │ 
 │                      │                                                                                                               │                          │ 
 │                      │                                                                                                               │                          │ 
 │                      │                                                                                                               │                          │ 
 │                      │                                                                                                               │                          │ 
 │   Source PDS         ├─────────────────────────┐                                                                                     │     Target PDS           │ 
 │                      │                         │                                                                                     │                          │ 
 │                      │                         │                                                                                     │                          │ 
 │                      │                         │                                                                                     │                          │ 
 │                      │                         │                                                                                     │                          │ 
 └──────────┬───────────┘                         │                                                                                     └──────────────────────────┘ 
            │                                     │                                                                                                   ▲              
            │                                     │                                                                                                   │              
            │                                     │                                                                                                   │              
            ▼                                     │                                                                                                   │              
┌────────────────────────┐            ┌───────────▼───────────────┐                          ┌─────────────────────────┐                ┌─────────────┼────────────┐ 
│                        │            │                           │                          │                         │                │                          │ 
│                        │            │                           │                          │                         │                │                          │ 
│                        │            │                           │                          │                         │                │                          │ 
│                        │ channel    │                           │       channel            │                         │  channel       │                          │ 
│repo.listMissingBlobs tx├───────────►│rx  sync.listBlobs    ┌─►tx┼─────────────────────────►│rx     storage rx      tx├───────────────►│rx   repo.uploadBlob      │ 
│                        │            │            │         │    │                          │                         │                │                          │ 
│                        │            │            │         │    │                          │                         │                │                          │ 
│                        │            │            ▼         │    │                          │                         │                │                          │ 
│                        │            │    sync.getBlob──────┘    │                          │                         │                │                          │ 
│                        │            │                           │                          │                         │                │                          │ 
└────────────────────────┘            └───────────────────────────┘                          └────────────┬────────────┘                └──────────────────────────┘ 
                                                                                                          │                                                          
                                                                                                          │                                                          
                                                                                                          │                                                          
                                                                                                          │                                                          
                                                                                                          │                                                          
                                                                                                          ▼                                                          
                                                                                              ┌────────────────────────┐                                             
                                                                                              │                        │                                             
                                                                                              │                        │                                             
                                                                                              │     Write to either    │                                             
                                                                                              │                        │                                             
                                                                                              │      OPFS or IndexDB   │                                             
                                                                                              │                        │                                             
                                                                                              │                        │                                             
                                                                                              │                        │                                             
                                                                                              │                        │                                             
                                                                                              │                        │                                             
                                                                                              └────────────────────────┘
```