# Configuration System

This module provides a comprehensive, platform-aware configuration system for the ATProto migration service, consolidating all configuration from across the codebase.

## Overview

The configuration system provides two complementary approaches:

1. **Legacy Configuration** (`mod.rs`) - Original configuration focused on migration-specific settings
2. **Unified Configuration** (`unified_config.rs`) - New comprehensive system with platform-specific optimizations

## Unified Configuration System

### Core Features

- **Platform Detection**: Automatic detection of browser, mobile, desktop, and server environments
- **Performance Optimization**: Platform-specific tuning for memory, network, and streaming operations
- **Comprehensive Coverage**: All aspects of the migration system in a single configuration
- **Validation**: Built-in validation with detailed error reporting

### Configuration Structure

```rust
use crate::services::config::UnifiedMigrationConfig;

// Auto-detect platform and create optimized configuration
let config = UnifiedMigrationConfig::auto_detect();

// Or create platform-specific configurations
let browser_config = UnifiedMigrationConfig::for_browser();
let mobile_config = UnifiedMigrationConfig::for_mobile();
let desktop_config = UnifiedMigrationConfig::for_desktop();
```

### Configuration Sections

#### 1. Streaming Configuration
- **Chunk sizes**: Platform-optimized (128KB mobile → 4MB server)
- **Concurrency**: Adaptive limits based on platform capabilities
- **Memory thresholds**: Platform-appropriate memory pressure handling
- **Compression**: Algorithm selection based on platform performance

#### 2. Performance Configuration
- **Retry logic**: Configurable retry attempts with exponential backoff
- **Timeouts**: Operation and chunk-level timeout configuration
- **Adaptive tuning**: Real-time performance optimization
- **Monitoring**: Performance metrics collection intervals

#### 3. Platform Configuration
- **Browser-specific**: OPFS preference, IndexedDB usage, Web Workers
- **Mobile-specific**: Low memory mode, conservative network usage, battery optimization
- **Desktop-specific**: Native file system APIs, memory mapping, higher concurrency

#### 4. Network Configuration
- **Connection management**: Timeouts, redirects, HTTP/2 support
- **Quality adaptation**: Dynamic chunk size adjustment based on network conditions
- **User agent**: Configurable identification string

#### 5. Memory Configuration
- **Memory limits**: Platform-appropriate maximum memory usage
- **Pressure monitoring**: Automatic memory pressure detection and cleanup
- **Garbage collection**: Platform-specific GC strategies

#### 6. Security Configuration
- **Integrity checking**: Configurable hash algorithms for data verification
- **TLS verification**: Certificate validation controls
- **Domain restrictions**: Allowlist for migration targets

### Platform Optimizations

#### Browser Environment
```rust
let config = UnifiedMigrationConfig::for_browser();
// - 256KB chunks for optimal browser performance
// - 4 concurrent streams (conservative for browser)
// - OPFS preferred, IndexedDB fallback
// - 256MB memory limit with aggressive GC
// - Web Workers for compression and hashing
```

#### Mobile Environment
```rust
let config = UnifiedMigrationConfig::for_mobile();
// - 128KB chunks (smaller for mobile networks)
// - 2 concurrent streams (very conservative)
// - IndexedDB preferred (OPFS may not be available)
// - 128MB memory limit with low memory mode
// - Battery optimization features enabled
```

#### Desktop Environment
```rust
let config = UnifiedMigrationConfig::for_desktop();
// - 1MB chunks for higher throughput
// - 8 concurrent streams
// - Native file system APIs
// - 2GB memory limit
// - Memory mapping for large files
```

### Advanced Features

#### Quality Adaptation
```rust
config.network.quality_adaptation = QualityAdaptationConfig {
    enabled: true,
    min_chunk_size: 32 * 1024,      // 32KB minimum
    max_chunk_size: 2 * 1024 * 1024, // 2MB maximum
    adaptation_interval_ms: 10_000,   // Adjust every 10 seconds
};
```

#### Battery Optimization (Mobile)
```rust
config.platform.mobile_config.battery_optimization = BatteryOptimizationConfig {
    enabled: true,
    reduce_cpu_usage: true,
    reduce_network_activity: true,
    low_battery_threshold: 0.2, // Activate at 20% battery
};
```

#### Web Workers Configuration
```rust
config.platform.browser_config.web_workers = WebWorkerConfig {
    enable_for_compression: true,
    enable_for_hashing: true,
    max_workers: 4,
    idle_timeout_ms: 60_000,
};
```

## Legacy Configuration System

### Migration Configuration
- **Storage backends**: OPFS, IndexedDB, LocalStorage with intelligent selection
- **Concurrency control**: Platform-appropriate concurrent operation limits
- **Retry strategies**: Configurable retry attempts with backoff
- **Blob enumeration**: Migration-optimized vs. full enumeration methods

### Storage Integration
```rust
use crate::services::config::get_config_with_browser_storage;

// Get configuration with browser storage integration
let config = get_config_with_browser_storage().await;
```

### Storage Estimation
```rust
use crate::services::config::{get_storage_estimate, StorageEstimate};

let estimate = get_storage_estimate().await?;
println!("Available storage: {} MB", estimate.available_bytes() / 1_048_576);
```

## Migration from Legacy to Unified

For new features, prefer the unified configuration system:

```rust
// Old approach
use crate::services::config::{MigrationConfig, get_global_config};
let config = get_global_config();

// New approach
use crate::services::config::UnifiedMigrationConfig;
let config = UnifiedMigrationConfig::auto_detect();
```

## Integration Points

This configuration system integrates with:

- **Streaming Services** (`ui/src/services/streaming/`) - Performance and memory settings
- **Blob Management** (`ui/src/services/blob/`) - Chunking and storage configuration
- **Client Services** (`ui/src/services/client/`) - Network and timeout settings
- **Migration Orchestration** (`ui/src/migration/`) - Retry and concurrency settings
- **UI Components** - Platform-appropriate user experience settings

## Best Practices

1. **Use Auto-Detection**: Let the system detect the optimal configuration
   ```rust
   let config = UnifiedMigrationConfig::auto_detect();
   ```

2. **Validate Configuration**: Always validate before use
   ```rust
   config.validate().expect("Invalid configuration");
   ```

3. **Platform-Specific Overrides**: Customize as needed for specific use cases
   ```rust
   let mut config = UnifiedMigrationConfig::for_browser();
   config.streaming.chunk_size = 512 * 1024; // Custom 512KB chunks
   ```

4. **Environment-Specific Settings**: Use environment variables or build flags for deployment-specific configuration