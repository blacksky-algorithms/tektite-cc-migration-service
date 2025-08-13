# Migration Configuration Module

This module provides centralized configuration management for the Tektite CC Migration Service.

## Features

- **Centralized Constants**: All migration-related constants are defined in structured configuration types
- **Environment Variable Support**: Configuration can be overridden via environment variables
- **Validation**: Built-in validation ensures configuration values are sensible
- **Global Access**: Thread-safe global configuration access for the entire application

## Configuration Structure

```rust
pub struct MigrationConfig {
    pub storage: StorageConfig,        // Storage limits and settings
    pub concurrency: ConcurrencyConfig, // Concurrency limits by backend
    pub retry: RetryConfig,            // Retry strategies and limits
}
```

## Environment Variables

- `MAX_CONCURRENT_TRANSFERS`: Override concurrent transfer limit
- `LOCAL_STORAGE_LIMIT`: Override LocalStorage size limit (bytes)
- `INDEXEDDB_LIMIT`: Override IndexedDB size limit (bytes)
- `MAX_RETRY_ATTEMPTS`: Override maximum retry attempts

## Usage

```rust
use crate::services::config::get_global_config;

let config = get_global_config();
println!("Max concurrent transfers: {}", config.concurrency.max_concurrent_transfers);
```