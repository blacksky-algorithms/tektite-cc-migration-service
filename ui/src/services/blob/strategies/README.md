# Blob Migration Strategy Pattern

This module implements a strategy pattern for blob migration, allowing the system to automatically select the optimal migration approach based on context.

## Strategies

### ConcurrentStrategy
- **Best for**: Many blobs (10+) with sufficient system resources
- **Approach**: Direct PDS-to-PDS transfers with high concurrency
- **Memory usage**: Minimal (streaming approach)
- **Concurrency**: Configurable, typically 5-10 concurrent transfers

### StorageStrategy
- **Best for**: Moderate number of blobs (5-50) with retry requirements
- **Approach**: Download → Cache → Upload with retry logic
- **Memory usage**: Higher (stores blobs temporarily)
- **Reliability**: High (retry capability, caching for resume)

### StreamingStrategy
- **Best for**: Memory-constrained environments or very large blobs
- **Approach**: Direct streaming with minimal memory footprint
- **Memory usage**: Minimal (chunk-based processing)
- **Scalability**: Excellent for large blobs

## Strategy Selection

The `StrategySelector` automatically chooses the optimal strategy based on:

- Blob count and estimated size
- Available storage backend (OPFS, IndexedDB, LocalStorage)
- Available system memory
- Storage backend capabilities

## Usage

```rust
use crate::services::blob::strategies::StrategySelector;

let strategy = StrategySelector::select_strategy(&missing_blobs, &blob_manager, available_memory);
let result = strategy.migrate(blobs, old_session, new_session, &mut blob_manager, &dispatch).await?;
```

## Adding New Strategies

1. Implement the `MigrationStrategy` trait
2. Add to `StrategySelector::get_candidate_strategies()`
3. Update scoring logic in `StrategySelector::score_strategy()`