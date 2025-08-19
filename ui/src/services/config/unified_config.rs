//! Unified Configuration System for Migration Service
//!
//! This module consolidates all configuration from across the codebase,
//! providing platform-specific optimizations and centralized settings.

use serde::{Serialize, Deserialize};

/// Unified configuration for the entire migration system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedMigrationConfig {
    /// Streaming configuration for data transfer operations
    pub streaming: StreamingConfig,
    
    /// Performance configuration for optimization
    pub performance: PerformanceConfig,
    
    /// Platform-specific configuration
    pub platform: PlatformConfig,
    
    /// Network configuration for transfers
    pub network: NetworkConfig,
    
    /// Memory management configuration
    pub memory: MemoryConfig,
    
    /// Security and validation configuration
    pub security: SecurityConfig,
}

/// Streaming-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// Default chunk size for streaming operations
    pub chunk_size: usize,
    
    /// Maximum number of concurrent streams
    pub max_concurrent: u32,
    
    /// Maximum memory threshold (as ratio of available memory)
    pub memory_threshold: f64,
    
    /// Whether to enable compression for streaming
    pub enable_compression: bool,
    
    /// Compression algorithm to use
    pub compression_algorithm: CompressionAlgorithm,
    
    /// Buffer size for streaming operations
    pub buffer_size: usize,
    
    /// Enable streaming metrics collection
    pub enable_metrics: bool,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    
    /// Base retry delay in milliseconds
    pub retry_delay_ms: u64,
    
    /// Retry backoff multiplier
    pub retry_backoff_multiplier: f64,
    
    /// Timeout for individual operations in milliseconds
    pub operation_timeout_ms: u64,
    
    /// Chunk processing timeout in milliseconds
    pub chunk_timeout_ms: u64,
    
    /// Enable adaptive performance tuning
    pub enable_adaptive_tuning: bool,
    
    /// Performance monitoring interval in milliseconds
    pub monitoring_interval_ms: u64,
}

/// Platform-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    /// Target platform
    pub platform_type: PlatformType,
    
    /// Browser-specific settings (when platform is Browser)
    pub browser_config: Option<BrowserConfig>,
    
    /// Desktop-specific settings (when platform is Desktop)
    pub desktop_config: Option<DesktopConfig>,
    
    /// Mobile-specific settings (when platform is Mobile)
    pub mobile_config: Option<MobileConfig>,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
    
    /// Request timeout in milliseconds
    pub request_timeout_ms: u64,
    
    /// Maximum number of redirects to follow
    pub max_redirects: u32,
    
    /// Enable HTTP/2 if available
    pub enable_http2: bool,
    
    /// User agent string
    pub user_agent: String,
    
    /// Network quality adaptation settings
    pub quality_adaptation: QualityAdaptationConfig,
}

/// Memory management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum memory usage in bytes
    pub max_memory_bytes: u64,
    
    /// Memory pressure threshold (0.0 to 1.0)
    pub pressure_threshold: f64,
    
    /// Enable memory monitoring
    pub enable_monitoring: bool,
    
    /// Memory cleanup interval in milliseconds
    pub cleanup_interval_ms: u64,
    
    /// Enable aggressive garbage collection
    pub aggressive_gc: bool,
}

/// Security and validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable data integrity checking
    pub enable_integrity_checks: bool,
    
    /// Hash algorithm for integrity checks
    pub hash_algorithm: HashAlgorithm,
    
    /// Enable TLS verification
    pub verify_tls: bool,
    
    /// Maximum allowed redirect chains
    pub max_redirect_depth: u32,
    
    /// Allowed domains for migration (empty = allow all)
    pub allowed_domains: Vec<String>,
}

/// Platform types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformType {
    Browser,
    Desktop,
    Mobile,
    Server,
}

/// Browser-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    /// Use OPFS for storage when available
    pub prefer_opfs: bool,
    
    /// Use IndexedDB for large data
    pub use_indexeddb: bool,
    
    /// Use localStorage for small data
    pub use_localstorage: bool,
    
    /// Enable Service Worker if available
    pub enable_service_worker: bool,
    
    /// Web Worker configuration
    pub web_workers: WebWorkerConfig,
}

/// Desktop-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopConfig {
    /// Use native file system APIs
    pub use_native_fs: bool,
    
    /// Maximum concurrent file operations
    pub max_file_operations: u32,
    
    /// Enable memory mapping for large files
    pub enable_mmap: bool,
}

/// Mobile-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileConfig {
    /// Reduce memory usage on mobile
    pub low_memory_mode: bool,
    
    /// Reduce network usage on mobile data
    pub conservative_network: bool,
    
    /// Battery optimization settings
    pub battery_optimization: BatteryOptimizationConfig,
}

/// Web Worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebWorkerConfig {
    /// Enable Web Workers for compression
    pub enable_for_compression: bool,
    
    /// Enable Web Workers for hashing
    pub enable_for_hashing: bool,
    
    /// Maximum number of Web Workers
    pub max_workers: u32,
    
    /// Worker idle timeout in milliseconds
    pub idle_timeout_ms: u64,
}

/// Network quality adaptation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityAdaptationConfig {
    /// Enable automatic quality adaptation
    pub enabled: bool,
    
    /// Minimum chunk size for slow connections
    pub min_chunk_size: usize,
    
    /// Maximum chunk size for fast connections
    pub max_chunk_size: usize,
    
    /// Adaptation response time in milliseconds
    pub adaptation_interval_ms: u64,
}

/// Battery optimization configuration for mobile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryOptimizationConfig {
    /// Enable battery optimizations
    pub enabled: bool,
    
    /// Reduce CPU usage when on battery
    pub reduce_cpu_usage: bool,
    
    /// Reduce network activity when on battery
    pub reduce_network_activity: bool,
    
    /// Battery level threshold for optimizations (0.0 to 1.0)
    pub low_battery_threshold: f64,
}

/// Compression algorithms supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    Gzip,
    Deflate,
    Brotli,
    Lz4,
}

/// Hash algorithms for integrity checking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashAlgorithm {
    Sha256,
    Sha1,
    Md5,
    Blake3,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self::for_platform(PlatformType::Browser)
    }
}

impl StreamingConfig {
    /// Create platform-optimized streaming configuration
    pub fn for_platform(platform: PlatformType) -> Self {
        match platform {
            PlatformType::Browser => Self {
                chunk_size: 256 * 1024, // 256KB for browsers
                max_concurrent: 4,       // Conservative for browser
                memory_threshold: 0.8,   // 80% memory threshold
                enable_compression: true,
                compression_algorithm: CompressionAlgorithm::Gzip,
                buffer_size: 64 * 1024, // 64KB buffer
                enable_metrics: true,
            },
            PlatformType::Mobile => Self {
                chunk_size: 128 * 1024, // Smaller chunks for mobile
                max_concurrent: 2,       // Very conservative
                memory_threshold: 0.7,   // Lower threshold
                enable_compression: true,
                compression_algorithm: CompressionAlgorithm::Gzip,
                buffer_size: 32 * 1024, // Smaller buffer
                enable_metrics: true,
            },
            PlatformType::Desktop => Self {
                chunk_size: 1024 * 1024, // 1MB for desktop
                max_concurrent: 8,        // Higher concurrency
                memory_threshold: 0.9,    // Higher threshold
                enable_compression: true,
                compression_algorithm: CompressionAlgorithm::Lz4,
                buffer_size: 256 * 1024, // Larger buffer
                enable_metrics: true,
            },
            PlatformType::Server => Self {
                chunk_size: 4 * 1024 * 1024, // 4MB for server
                max_concurrent: 16,           // Much higher concurrency
                memory_threshold: 0.95,       // Very high threshold
                enable_compression: true,
                compression_algorithm: CompressionAlgorithm::Brotli,
                buffer_size: 1024 * 1024, // Large buffer
                enable_metrics: true,
            },
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            retry_delay_ms: 1000,
            retry_backoff_multiplier: 2.0,
            operation_timeout_ms: 30_000,
            chunk_timeout_ms: 10_000,
            enable_adaptive_tuning: true,
            monitoring_interval_ms: 5_000,
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            connect_timeout_ms: 10_000,
            request_timeout_ms: 30_000,
            max_redirects: 10,
            enable_http2: true,
            user_agent: "ATProto-Migration-Service/1.0".to_string(),
            quality_adaptation: QualityAdaptationConfig::default(),
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024, // 512MB default
            pressure_threshold: 0.8,
            enable_monitoring: true,
            cleanup_interval_ms: 30_000,
            aggressive_gc: false,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enable_integrity_checks: true,
            hash_algorithm: HashAlgorithm::Sha256,
            verify_tls: true,
            max_redirect_depth: 5,
            allowed_domains: Vec::new(),
        }
    }
}

impl Default for QualityAdaptationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_chunk_size: 32 * 1024,  // 32KB minimum
            max_chunk_size: 2 * 1024 * 1024, // 2MB maximum
            adaptation_interval_ms: 10_000,
        }
    }
}

impl Default for WebWorkerConfig {
    fn default() -> Self {
        Self {
            enable_for_compression: true,
            enable_for_hashing: true,
            max_workers: 4,
            idle_timeout_ms: 60_000,
        }
    }
}

impl Default for BatteryOptimizationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            reduce_cpu_usage: true,
            reduce_network_activity: true,
            low_battery_threshold: 0.2, // 20%
        }
    }
}

impl Default for UnifiedMigrationConfig {
    fn default() -> Self {
        Self::for_browser()
    }
}

impl UnifiedMigrationConfig {
    /// Create configuration optimized for browser environment
    pub fn for_browser() -> Self {
        Self {
            streaming: StreamingConfig::for_platform(PlatformType::Browser),
            performance: PerformanceConfig::default(),
            platform: PlatformConfig {
                platform_type: PlatformType::Browser,
                browser_config: Some(BrowserConfig {
                    prefer_opfs: true,
                    use_indexeddb: true,
                    use_localstorage: false, // Avoid localStorage for large data
                    enable_service_worker: false, // Disabled by default for simplicity
                    web_workers: WebWorkerConfig::default(),
                }),
                desktop_config: None,
                mobile_config: None,
            },
            network: NetworkConfig::default(),
            memory: MemoryConfig {
                max_memory_bytes: 256 * 1024 * 1024, // 256MB for browser
                pressure_threshold: 0.75, // Lower threshold for browser
                enable_monitoring: true,
                cleanup_interval_ms: 15_000, // More frequent cleanup
                aggressive_gc: true, // Enable for browser
            },
            security: SecurityConfig::default(),
        }
    }

    /// Create configuration optimized for mobile browser
    pub fn for_mobile() -> Self {
        let mut config = Self::for_browser();
        config.streaming = StreamingConfig::for_platform(PlatformType::Mobile);
        config.platform = PlatformConfig {
            platform_type: PlatformType::Mobile,
            browser_config: Some(BrowserConfig {
                prefer_opfs: false, // OPFS may not be available on mobile
                use_indexeddb: true,
                use_localstorage: false,
                enable_service_worker: false,
                web_workers: WebWorkerConfig {
                    max_workers: 2, // Fewer workers on mobile
                    ..WebWorkerConfig::default()
                },
            }),
            desktop_config: None,
            mobile_config: Some(MobileConfig {
                low_memory_mode: true,
                conservative_network: true,
                battery_optimization: BatteryOptimizationConfig::default(),
            }),
        };
        config.memory.max_memory_bytes = 128 * 1024 * 1024; // 128MB for mobile
        config.memory.pressure_threshold = 0.6; // Even lower threshold
        config
    }

    /// Create configuration optimized for desktop
    pub fn for_desktop() -> Self {
        Self {
            streaming: StreamingConfig::for_platform(PlatformType::Desktop),
            performance: PerformanceConfig::default(),
            platform: PlatformConfig {
                platform_type: PlatformType::Desktop,
                browser_config: None,
                desktop_config: Some(DesktopConfig {
                    use_native_fs: true,
                    max_file_operations: 10,
                    enable_mmap: true,
                }),
                mobile_config: None,
            },
            network: NetworkConfig::default(),
            memory: MemoryConfig {
                max_memory_bytes: 2 * 1024 * 1024 * 1024, // 2GB for desktop
                pressure_threshold: 0.9,
                enable_monitoring: true,
                cleanup_interval_ms: 60_000, // Less frequent cleanup
                aggressive_gc: false,
            },
            security: SecurityConfig::default(),
        }
    }

    /// Detect platform and create appropriate configuration
    pub fn auto_detect() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            // In WASM, check if we're on mobile
            if is_mobile_browser() {
                Self::for_mobile()
            } else {
                Self::for_browser()
            }
        }
        
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::for_desktop()
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.streaming.chunk_size == 0 {
            errors.push("Streaming chunk_size must be greater than 0".to_string());
        }

        if self.streaming.max_concurrent == 0 {
            errors.push("Streaming max_concurrent must be greater than 0".to_string());
        }

        if self.streaming.memory_threshold <= 0.0 || self.streaming.memory_threshold > 1.0 {
            errors.push("Streaming memory_threshold must be between 0.0 and 1.0".to_string());
        }

        if self.performance.max_retries == 0 {
            errors.push("Performance max_retries must be greater than 0".to_string());
        }

        if self.memory.max_memory_bytes == 0 {
            errors.push("Memory max_memory_bytes must be greater than 0".to_string());
        }

        if self.memory.pressure_threshold <= 0.0 || self.memory.pressure_threshold > 1.0 {
            errors.push("Memory pressure_threshold must be between 0.0 and 1.0".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Detect if running in a mobile browser environment
#[cfg(target_arch = "wasm32")]
fn is_mobile_browser() -> bool {
    if let Some(window) = web_sys::window() {
        let navigator = window.navigator();
        if let Ok(user_agent) = navigator.user_agent() {
            let user_agent = user_agent.to_lowercase();
            return user_agent.contains("mobile") 
                || user_agent.contains("android")
                || user_agent.contains("iphone")
                || user_agent.contains("ipad");
        }
    }
    false
}

#[cfg(not(target_arch = "wasm32"))]
fn is_mobile_browser() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = UnifiedMigrationConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_config() {
        let mut config = UnifiedMigrationConfig::default();
        config.streaming.chunk_size = 0;
        config.streaming.memory_threshold = 1.5;
        
        let errors = config.validate().unwrap_err();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_platform_specific_configs() {
        let browser_config = UnifiedMigrationConfig::for_browser();
        let mobile_config = UnifiedMigrationConfig::for_mobile();
        let desktop_config = UnifiedMigrationConfig::for_desktop();

        assert_eq!(browser_config.platform.platform_type, PlatformType::Browser);
        assert_eq!(mobile_config.platform.platform_type, PlatformType::Mobile);
        assert_eq!(desktop_config.platform.platform_type, PlatformType::Desktop);

        // Mobile should have smaller chunk size than browser
        assert!(mobile_config.streaming.chunk_size < browser_config.streaming.chunk_size);
        
        // Desktop should have larger chunk size than browser
        assert!(desktop_config.streaming.chunk_size > browser_config.streaming.chunk_size);
    }
}