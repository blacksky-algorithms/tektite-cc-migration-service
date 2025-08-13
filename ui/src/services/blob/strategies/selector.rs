//! Strategy selector for choosing the optimal blob migration approach

use gloo_console as console;
use crate::services::{
    client::ClientMissingBlob,
    blob::{blob_fallback_manager::FallbackBlobManager, blob_manager_trait::BlobManagerTrait},
};

use super::{
    MigrationStrategy, 
    ConcurrentStrategy, 
    StorageStrategy, 
    StreamingStrategy
};

/// Selector for choosing the optimal migration strategy
pub struct StrategySelector;

impl StrategySelector {
    /// Select the optimal migration strategy based on context
    pub fn select_strategy(
        blobs: &[ClientMissingBlob],
        blob_manager: &FallbackBlobManager,
        available_memory: Option<u64>,
    ) -> Box<dyn MigrationStrategy> {
        let blob_count = blobs.len() as u32;
        let backend_name = blob_manager.storage_name();
        
        console::info!("[StrategySelector] Selecting optimal strategy for {} blobs using {} backend", 
                      blob_count, backend_name);
        
        // Get candidate strategies
        let strategies = Self::get_candidate_strategies(blob_count, backend_name);
        
        // Score each strategy based on context
        let mut best_strategy: Option<Box<dyn MigrationStrategy>> = None;
        let mut best_score = 0u32;
        
        for strategy in strategies {
            let score = Self::score_strategy(&strategy, blob_count, backend_name, available_memory);
            console::debug!("[StrategySelector] Strategy '{}' scored: {}", strategy.name(), score);
            
            if score > best_score {
                best_score = score;
                best_strategy = Some(strategy);
            }
        }
        
        let selected = best_strategy.unwrap_or_else(|| Box::new(StreamingStrategy::new()));
        console::info!("[StrategySelector] Selected strategy: '{}' (score: {})", selected.name(), best_score);
        
        selected
    }
    
    /// Get candidate strategies that can handle the given context
    fn get_candidate_strategies(blob_count: u32, backend_name: &str) -> Vec<Box<dyn MigrationStrategy>> {
        let mut candidates: Vec<Box<dyn MigrationStrategy>> = Vec::new();
        
        // Concurrent strategy
        let concurrent = Box::new(ConcurrentStrategy::new());
        if concurrent.supports_blob_count(blob_count) && concurrent.supports_storage_backend(backend_name) {
            candidates.push(concurrent);
        }
        
        // Storage strategy (with cache)
        let storage_cached = Box::new(StorageStrategy::new());
        if storage_cached.supports_blob_count(blob_count) && storage_cached.supports_storage_backend(backend_name) {
            candidates.push(storage_cached);
        }
        
        // Storage strategy (without cache)
        let storage_direct = Box::new(StorageStrategy::with_cache(false));
        if storage_direct.supports_blob_count(blob_count) && storage_direct.supports_storage_backend(backend_name) {
            candidates.push(storage_direct);
        }
        
        // Streaming strategy (always available as fallback)
        candidates.push(Box::new(StreamingStrategy::new()));
        
        candidates
    }
    
    /// Score a strategy based on the migration context
    fn score_strategy(
        strategy: &Box<dyn MigrationStrategy>,
        blob_count: u32,
        backend_name: &str,
        available_memory: Option<u64>,
    ) -> u32 {
        let mut score = strategy.priority();
        
        // Bonus for blob count suitability
        if strategy.supports_blob_count(blob_count) {
            score += 20;
        }
        
        // Bonus for backend compatibility
        if strategy.supports_storage_backend(backend_name) {
            score += 15;
        }
        
        // Memory usage considerations
        if let Some(available) = available_memory {
            let estimated_usage = strategy.estimate_memory_usage(blob_count);
            if estimated_usage <= available {
                score += 10;
            } else {
                // Penalty for exceeding available memory
                score = score.saturating_sub(30);
            }
        }
        
        // Specific strategy bonuses based on context
        match strategy.name() {
            "concurrent" => {
                // Bonus for many blobs
                if blob_count >= 20 {
                    score += 15;
                }
            }
            "storage" => {
                // Bonus for good storage backends
                match backend_name {
                    "OPFS" => score += 10,
                    "IndexedDB" => score += 8,
                    "LocalStorage" => score += 5,
                    _ => {}
                }
            }
            "streaming" => {
                // Bonus for limited memory scenarios
                if available_memory.is_none_or(|mem| mem < 100 * 1024 * 1024) {
                    score += 12;
                }
            }
            _ => {}
        }
        
        score
    }
    
    /// Get a fallback strategy that should always work
    pub fn get_fallback_strategy() -> Box<dyn MigrationStrategy> {
        Box::new(StreamingStrategy::new())
    }
}