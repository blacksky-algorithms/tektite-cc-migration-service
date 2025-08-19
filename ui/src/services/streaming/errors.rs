//! Enhanced Error Types for Streaming Operations
//!
//! This module provides comprehensive error handling for streaming operations,
//! with detailed error information to help with diagnostics and recovery.

use thiserror::Error;
use serde::{Serialize, Deserialize};

/// Comprehensive streaming errors with detailed context
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum StreamingError {
    #[error("Chunk transfer failed: {chunk_id} after {retry_count} retries (last error: {last_error})")]
    ChunkTransferFailed {
        chunk_id: String,
        retry_count: u32,
        last_error: String,
        total_size: Option<u64>,
        bytes_transferred: u64,
    },

    #[error("Memory pressure detected: {used_mb}MB used, {available_mb}MB available (pressure: {pressure_ratio:.2})")]
    MemoryPressure {
        used_mb: u64,
        available_mb: u64,
        pressure_ratio: f64,
        peak_usage_mb: u64,
    },

    #[error("Storage quota exceeded: requested {requested_mb}MB, available {available_mb}MB")]
    StorageQuotaExceeded {
        requested_mb: u64,
        available_mb: u64,
        current_usage_mb: u64,
    },

    #[error("Network timeout: operation took {duration_ms}ms, timeout was {timeout_ms}ms")]
    NetworkTimeout {
        duration_ms: u64,
        timeout_ms: u64,
        operation: String,
        retry_attempt: u32,
    },

    #[error("Compression failed: {reason} (input size: {input_size}, algorithm: {algorithm})")]
    CompressionFailed {
        reason: String,
        input_size: u64,
        algorithm: String,
    },

    #[error("Data integrity check failed: expected {expected_hash}, got {actual_hash}")]
    DataIntegrityFailed {
        chunk_id: String,
        expected_hash: String,
        actual_hash: String,
        chunk_size: u64,
    },

    #[error("Stream interrupted: {reason} at offset {offset}/{total_size}")]
    StreamInterrupted {
        reason: String,
        offset: u64,
        total_size: u64,
        resumable: bool,
    },

    #[error("Concurrent limit exceeded: {active_streams} active streams, limit is {max_concurrent}")]
    ConcurrentLimitExceeded {
        active_streams: u32,
        max_concurrent: u32,
        operation: String,
    },

    #[error("Browser API unavailable: {api_name} is not supported in this environment")]
    BrowserApiUnavailable {
        api_name: String,
        fallback_available: bool,
        required_features: Vec<String>,
    },

    #[error("Configuration error: {parameter} = {value} is invalid ({reason})")]
    ConfigurationError {
        parameter: String,
        value: String,
        reason: String,
        valid_range: Option<String>,
    },
}

/// Streaming error with recovery context
#[derive(Debug, Clone)]
pub struct RecoverableStreamingError {
    pub error: StreamingError,
    pub recovery_suggestions: Vec<RecoveryStrategy>,
    pub error_context: ErrorContext,
}

/// Recovery strategies for different types of streaming errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    RetryWithBackoff {
        max_retries: u32,
        initial_delay_ms: u64,
        backoff_multiplier: f64,
    },
    ReduceChunkSize {
        current_size: u64,
        suggested_size: u64,
    },
    EnableCompression {
        algorithm: String,
        expected_ratio: f64,
    },
    SwitchToFallbackStorage,
    ReduceConcurrency {
        current: u32,
        suggested: u32,
    },
    ClearCache {
        cache_type: String,
        estimated_freed_mb: u64,
    },
    RequestUserAction {
        action: String,
        reason: String,
    },
}

/// Context information for error analysis and recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub timestamp: u64, // Unix timestamp
    pub operation: String,
    pub user_agent: Option<String>,
    pub available_memory_mb: Option<u64>,
    pub network_conditions: NetworkConditions,
    pub browser_info: BrowserInfo,
    pub previous_errors: Vec<String>, // Recent error history
}

/// Network condition information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConditions {
    pub effective_type: Option<String>, // "slow-2g", "2g", "3g", "4g"
    pub downlink_mbps: Option<f64>,
    pub rtt_ms: Option<f64>,
    pub save_data: bool,
}

/// Browser environment information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserInfo {
    pub is_mobile: bool,
    pub supports_opfs: bool,
    pub supports_compression: bool,
    pub supports_streaming: bool,
    pub max_concurrent_requests: u32,
}

impl RecoverableStreamingError {
    /// Create a new recoverable error with suggested recovery strategies
    pub fn new(error: StreamingError, context: ErrorContext) -> Self {
        let recovery_suggestions = suggest_recovery_strategies(&error, &context);
        Self {
            error,
            recovery_suggestions,
            error_context: context,
        }
    }

    /// Get the most appropriate recovery strategy
    pub fn best_recovery_strategy(&self) -> Option<&RecoveryStrategy> {
        // Return the first (most appropriate) recovery strategy
        self.recovery_suggestions.first()
    }

    /// Check if the error is likely to be transient
    pub fn is_transient(&self) -> bool {
        matches!(
            self.error,
            StreamingError::NetworkTimeout { .. }
            | StreamingError::StreamInterrupted { resumable: true, .. }
            | StreamingError::ChunkTransferFailed { .. }
        )
    }

    /// Get severity level of the error
    pub fn severity(&self) -> ErrorSeverity {
        match &self.error {
            StreamingError::DataIntegrityFailed { .. } => ErrorSeverity::Critical,
            StreamingError::StorageQuotaExceeded { .. } => ErrorSeverity::Critical,
            StreamingError::BrowserApiUnavailable { fallback_available: false, .. } => ErrorSeverity::Critical,
            StreamingError::MemoryPressure { pressure_ratio, .. } => {
                if *pressure_ratio > 0.9 {
                    ErrorSeverity::Critical
                } else {
                    ErrorSeverity::Warning
                }
            }
            StreamingError::NetworkTimeout { .. } => ErrorSeverity::Warning,
            StreamingError::ChunkTransferFailed { retry_count, .. } => {
                if *retry_count > 5 {
                    ErrorSeverity::High
                } else {
                    ErrorSeverity::Warning
                }
            }
            _ => ErrorSeverity::Medium,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Low,
    Medium,
    Warning,
    High,
    Critical,
}

/// Suggest appropriate recovery strategies based on error type and context
fn suggest_recovery_strategies(error: &StreamingError, context: &ErrorContext) -> Vec<RecoveryStrategy> {
    let mut strategies = Vec::new();

    match error {
        StreamingError::ChunkTransferFailed { retry_count, .. } => {
            if *retry_count < 3 {
                strategies.push(RecoveryStrategy::RetryWithBackoff {
                    max_retries: 5,
                    initial_delay_ms: 1000,
                    backoff_multiplier: 2.0,
                });
            }
            
            if context.network_conditions.effective_type.as_deref() == Some("slow-2g") {
                strategies.push(RecoveryStrategy::ReduceChunkSize {
                    current_size: 256 * 1024, // Assume 256KB default
                    suggested_size: 64 * 1024, // Reduce to 64KB
                });
            }
        }

        StreamingError::MemoryPressure { .. } => {
            strategies.push(RecoveryStrategy::ReduceChunkSize {
                current_size: 256 * 1024,
                suggested_size: 64 * 1024,
            });
            
            strategies.push(RecoveryStrategy::ReduceConcurrency {
                current: 4,
                suggested: 1,
            });

            strategies.push(RecoveryStrategy::ClearCache {
                cache_type: "StreamingBuffers".to_string(),
                estimated_freed_mb: 10,
            });
        }

        StreamingError::StorageQuotaExceeded { .. } => {
            strategies.push(RecoveryStrategy::EnableCompression {
                algorithm: "lz4".to_string(),
                expected_ratio: 0.6,
            });
            
            strategies.push(RecoveryStrategy::RequestUserAction {
                action: "Clear browser storage".to_string(),
                reason: "Storage quota exceeded".to_string(),
            });
        }

        StreamingError::NetworkTimeout { .. } => {
            strategies.push(RecoveryStrategy::RetryWithBackoff {
                max_retries: 3,
                initial_delay_ms: 2000,
                backoff_multiplier: 1.5,
            });
        }

        StreamingError::ConcurrentLimitExceeded { .. } => {
            strategies.push(RecoveryStrategy::ReduceConcurrency {
                current: 8,
                suggested: 2,
            });
        }

        StreamingError::BrowserApiUnavailable { fallback_available: true, .. } => {
            strategies.push(RecoveryStrategy::SwitchToFallbackStorage);
        }

        _ => {
            // Generic retry strategy for unknown errors
            strategies.push(RecoveryStrategy::RetryWithBackoff {
                max_retries: 3,
                initial_delay_ms: 1000,
                backoff_multiplier: 2.0,
            });
        }
    }

    strategies
}

/// Result type that must be used for streaming operations to prevent data loss
pub type StreamingResult<T> = Result<T, RecoverableStreamingError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_severity_ordering() {
        assert!(ErrorSeverity::Critical > ErrorSeverity::High);
        assert!(ErrorSeverity::High > ErrorSeverity::Warning);
        assert!(ErrorSeverity::Warning > ErrorSeverity::Medium);
        assert!(ErrorSeverity::Medium > ErrorSeverity::Low);
    }

    #[test]
    fn test_recovery_strategy_generation() {
        let error = StreamingError::ChunkTransferFailed {
            chunk_id: "test".to_string(),
            retry_count: 1,
            last_error: "timeout".to_string(),
            total_size: Some(1024),
            bytes_transferred: 512,
        };

        let context = ErrorContext {
            timestamp: 0,
            operation: "test".to_string(),
            user_agent: None,
            available_memory_mb: Some(1024),
            network_conditions: NetworkConditions {
                effective_type: Some("3g".to_string()),
                downlink_mbps: Some(1.5),
                rtt_ms: Some(300.0),
                save_data: false,
            },
            browser_info: BrowserInfo {
                is_mobile: false,
                supports_opfs: true,
                supports_compression: true,
                supports_streaming: true,
                max_concurrent_requests: 4,
            },
            previous_errors: Vec::new(),
        };

        let recoverable_error = RecoverableStreamingError::new(error, context);
        assert!(!recoverable_error.recovery_suggestions.is_empty());
        assert!(recoverable_error.is_transient());
    }
}