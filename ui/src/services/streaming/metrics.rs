//! Streaming Performance Metrics
//!
//! This module provides comprehensive metrics tracking for the streaming migration system,
//! helping optimize performance and monitor the health of data transfers.

use std::time::{Duration, Instant};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Comprehensive streaming performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingMetrics {
    /// Data transfer rate in bytes per second
    pub transfer_rate: f64,
    
    /// Chunk processing efficiency (successful_chunks / total_chunks)
    pub chunk_efficiency: f64,
    
    /// Memory usage statistics
    pub memory_stats: MemoryStats,
    
    /// Compression ratio if compression is used (compressed_size / original_size)
    pub compression_ratio: Option<f64>,
    
    /// Network-related metrics
    pub network_stats: NetworkStats,
    
    /// Error tracking
    pub error_stats: ErrorStats,
}

/// Memory usage statistics for streaming operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Peak memory usage in bytes during streaming
    pub peak_usage_bytes: u64,
    
    /// Current memory usage in bytes
    pub current_usage_bytes: u64,
    
    /// Available memory in bytes (estimated)
    pub available_bytes: u64,
    
    /// Memory pressure indicator (0.0 = no pressure, 1.0 = critical)
    pub pressure_ratio: f64,
}

/// Network performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    /// Average latency per request in milliseconds
    pub avg_latency_ms: f64,
    
    /// Number of retries performed
    pub retry_count: u32,
    
    /// Number of failed requests
    pub failed_requests: u32,
    
    /// Total requests made
    pub total_requests: u32,
    
    /// Network success rate (successful_requests / total_requests)
    pub success_rate: f64,
}

/// Error tracking statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStats {
    /// Total errors encountered
    pub total_errors: u32,
    
    /// Errors by category
    pub errors_by_type: HashMap<String, u32>,
    
    /// Recovery success rate
    pub recovery_rate: f64,
}

/// Real-time metrics collector for streaming operations
pub struct MetricsCollector {
    start_time: Instant,
    bytes_transferred: u64,
    chunks_processed: u32,
    chunks_successful: u32,
    chunks_failed: u32,
    memory_samples: Vec<u64>,
    latency_samples: Vec<Duration>,
    errors: HashMap<String, u32>,
    retries: u32,
    peak_memory: u64,
    original_size: Option<u64>,
    compressed_size: Option<u64>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            bytes_transferred: 0,
            chunks_processed: 0,
            chunks_successful: 0,
            chunks_failed: 0,
            memory_samples: Vec::new(),
            latency_samples: Vec::new(),
            errors: HashMap::new(),
            retries: 0,
            peak_memory: 0,
            original_size: None,
            compressed_size: None,
        }
    }

    /// Record bytes transferred
    pub fn record_bytes_transferred(&mut self, bytes: u64) {
        self.bytes_transferred += bytes;
    }

    /// Record successful chunk processing
    pub fn record_chunk_success(&mut self) {
        self.chunks_processed += 1;
        self.chunks_successful += 1;
    }

    /// Record failed chunk processing
    pub fn record_chunk_failure(&mut self) {
        self.chunks_processed += 1;
        self.chunks_failed += 1;
    }

    /// Record memory usage sample
    pub fn record_memory_usage(&mut self, bytes: u64) {
        self.memory_samples.push(bytes);
        if bytes > self.peak_memory {
            self.peak_memory = bytes;
        }
    }

    /// Record network latency
    pub fn record_latency(&mut self, latency: Duration) {
        self.latency_samples.push(latency);
    }

    /// Record an error by type
    pub fn record_error(&mut self, error_type: &str) {
        *self.errors.entry(error_type.to_string()).or_insert(0) += 1;
    }

    /// Record a retry attempt
    pub fn record_retry(&mut self) {
        self.retries += 1;
    }

    /// Set compression information
    pub fn set_compression_info(&mut self, original_size: u64, compressed_size: u64) {
        self.original_size = Some(original_size);
        self.compressed_size = Some(compressed_size);
    }

    /// Calculate current transfer rate in bytes per second
    pub fn current_transfer_rate(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.bytes_transferred as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Calculate chunk efficiency
    pub fn chunk_efficiency(&self) -> f64 {
        if self.chunks_processed > 0 {
            self.chunks_successful as f64 / self.chunks_processed as f64
        } else {
            1.0
        }
    }

    /// Get current memory statistics
    pub fn memory_stats(&self) -> MemoryStats {
        let current_usage = self.memory_samples.last().copied().unwrap_or(0);
        
        // Estimate available memory based on browser constraints
        // This is a rough estimate as real memory info isn't available in WASM
        let estimated_available = estimate_available_memory();
        let pressure_ratio = if estimated_available > 0 {
            (current_usage as f64 / estimated_available as f64).min(1.0)
        } else {
            0.0
        };

        MemoryStats {
            peak_usage_bytes: self.peak_memory,
            current_usage_bytes: current_usage,
            available_bytes: estimated_available,
            pressure_ratio,
        }
    }

    /// Get network statistics
    pub fn network_stats(&self) -> NetworkStats {
        let avg_latency = if !self.latency_samples.is_empty() {
            let total: Duration = self.latency_samples.iter().sum();
            total.as_millis() as f64 / self.latency_samples.len() as f64
        } else {
            0.0
        };

        let total_requests = self.chunks_processed + self.retries;
        let success_rate = if total_requests > 0 {
            self.chunks_successful as f64 / total_requests as f64
        } else {
            1.0
        };

        NetworkStats {
            avg_latency_ms: avg_latency,
            retry_count: self.retries,
            failed_requests: self.chunks_failed,
            total_requests,
            success_rate,
        }
    }

    /// Get error statistics
    pub fn error_stats(&self) -> ErrorStats {
        let total_errors: u32 = self.errors.values().sum();
        
        // Recovery rate = (successful retries) / (total errors)
        // This is a simplified calculation
        let recovery_rate = if total_errors > 0 && self.chunks_successful > 0 {
            let recovered = self.retries.min(self.chunks_successful);
            recovered as f64 / total_errors as f64
        } else {
            0.0
        };

        ErrorStats {
            total_errors,
            errors_by_type: self.errors.clone(),
            recovery_rate,
        }
    }

    /// Generate complete metrics snapshot
    pub fn snapshot(&self) -> StreamingMetrics {
        let compression_ratio = if let (Some(original), Some(compressed)) = (self.original_size, self.compressed_size) {
            if original > 0 {
                Some(compressed as f64 / original as f64)
            } else {
                None
            }
        } else {
            None
        };

        StreamingMetrics {
            transfer_rate: self.current_transfer_rate(),
            chunk_efficiency: self.chunk_efficiency(),
            memory_stats: self.memory_stats(),
            compression_ratio,
            network_stats: self.network_stats(),
            error_stats: self.error_stats(),
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Estimate available memory in browser environment
/// This is a rough estimate as precise memory info isn't available in WASM
fn estimate_available_memory() -> u64 {
    // Conservative estimate based on typical browser memory limits
    // Most browsers allow 1-4GB for a single tab, we'll use 1GB as conservative
    1024 * 1024 * 1024 // 1GB
}

/// Enhanced streaming result that includes metrics
#[derive(Debug)]
pub struct MetricsStreamingResult<T> {
    pub data: T,
    pub metrics: StreamingMetrics,
    pub warnings: Vec<StreamingWarning>,
}

/// Warnings that can be generated during streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamingWarning {
    HighMemoryPressure { current: u64, threshold: u64 },
    LowTransferRate { current: f64, expected: f64 },
    HighErrorRate { rate: f64, threshold: f64 },
    NetworkLatencyHigh { avg_ms: f64, threshold_ms: f64 },
}

impl<T> MetricsStreamingResult<T> {
    pub fn new(data: T, metrics: StreamingMetrics) -> Self {
        let warnings = generate_warnings(&metrics);
        Self {
            data,
            metrics,
            warnings,
        }
    }
}

/// Generate performance warnings based on metrics
fn generate_warnings(metrics: &StreamingMetrics) -> Vec<StreamingWarning> {
    let mut warnings = Vec::new();

    // Memory pressure warning
    if metrics.memory_stats.pressure_ratio > 0.8 {
        warnings.push(StreamingWarning::HighMemoryPressure {
            current: metrics.memory_stats.current_usage_bytes,
            threshold: (metrics.memory_stats.available_bytes as f64 * 0.8) as u64,
        });
    }

    // Low transfer rate warning (less than 100KB/s)
    if metrics.transfer_rate < 100_000.0 {
        warnings.push(StreamingWarning::LowTransferRate {
            current: metrics.transfer_rate,
            expected: 500_000.0, // 500KB/s expected
        });
    }

    // High error rate warning
    if metrics.error_stats.total_errors > 0 && metrics.chunk_efficiency < 0.9 {
        warnings.push(StreamingWarning::HighErrorRate {
            rate: 1.0 - metrics.chunk_efficiency,
            threshold: 0.1, // 10% error rate threshold
        });
    }

    // High latency warning
    if metrics.network_stats.avg_latency_ms > 2000.0 {
        warnings.push(StreamingWarning::NetworkLatencyHigh {
            avg_ms: metrics.network_stats.avg_latency_ms,
            threshold_ms: 1000.0,
        });
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_basic() {
        let mut collector = MetricsCollector::new();
        
        collector.record_bytes_transferred(1024);
        collector.record_chunk_success();
        collector.record_memory_usage(512 * 1024);
        
        let metrics = collector.snapshot();
        assert!(metrics.transfer_rate > 0.0);
        assert_eq!(metrics.chunk_efficiency, 1.0);
        assert_eq!(metrics.memory_stats.current_usage_bytes, 512 * 1024);
    }

    #[test]
    fn test_chunk_efficiency_calculation() {
        let mut collector = MetricsCollector::new();
        
        collector.record_chunk_success();
        collector.record_chunk_success();
        collector.record_chunk_failure();
        
        let efficiency = collector.chunk_efficiency();
        assert!((efficiency - 0.6666666666666666).abs() < 0.0001);
    }
}