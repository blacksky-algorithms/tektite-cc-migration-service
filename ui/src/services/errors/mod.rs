use thiserror::Error;

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("Storage error: {source}")]
    Storage {
        source: StorageError,
        context: String,
    },

    #[error("Network error: {message}")]
    Network { message: String, retry_count: u32 },

    #[error("Configuration error: {field} = {value}")]
    Configuration { field: String, value: String },

    #[error("Authentication error: {message}")]
    Authentication { message: String },

    #[error("Migration step error: {step} - {reason}")]
    MigrationStep { step: String, reason: String },

    #[error("Blob processing error: {cid} - {error}")]
    BlobProcessing { cid: String, error: String },

    #[error("PDS client error: {message}")]
    PdsClient { message: String },

    #[error("Validation error: {field} - {message}")]
    Validation { field: String, message: String },

    #[error("Resume error: {reason}")]
    Resume { reason: String },

    #[error("Circuit breaker open: {reason} - retry after {retry_after_ms}ms")]
    CircuitBreakerOpen { reason: String, retry_after_ms: u64 },

    #[error("Deduplication error: {operation} - {message}")]
    Deduplication { operation: String, message: String },

    #[error("Integrity verification failed: {cid} - {reason}")]
    IntegrityCheckFailed { cid: String, reason: String },

    #[error("Progress tracking error: {component} - {error}")]
    ProgressTracking { component: String, error: String },

    #[error("Unknown error: {message}")]
    Unknown { message: String },
}

/// Enhanced storage error with more specific failure types
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Storage backend unavailable: {backend}")]
    BackendUnavailable { backend: String },

    #[error("Storage quota exceeded: {used}/{limit} bytes")]
    QuotaExceeded { used: u64, limit: u64 },

    #[error("Storage operation failed: {operation} - {reason}")]
    OperationFailed { operation: String, reason: String },

    #[error("Storage initialization failed: {backend} - {error}")]
    InitializationFailed { backend: String, error: String },

    #[error("Blob not found: {cid} in {backend}")]
    BlobNotFound { cid: String, backend: String },

    #[error("Storage operation failed after {attempts} attempts")]
    RetryExhausted {
        attempts: u32,
        #[source]
        cause: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl From<StorageError> for MigrationError {
    fn from(error: StorageError) -> Self {
        MigrationError::Storage {
            source: error,
            context: "Storage operation failed".to_string(),
        }
    }
}

impl From<String> for MigrationError {
    fn from(message: String) -> Self {
        MigrationError::Unknown { message }
    }
}

impl From<&str> for MigrationError {
    fn from(message: &str) -> Self {
        MigrationError::Unknown {
            message: message.to_string(),
        }
    }
}

pub type MigrationResult<T> = Result<T, MigrationError>;

impl MigrationError {
    pub fn with_context(self, context: &str) -> Self {
        match self {
            MigrationError::Storage { source, .. } => MigrationError::Storage {
                source,
                context: context.to_string(),
            },
            other => other,
        }
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            MigrationError::Network { .. } => true,
            MigrationError::Storage { source, .. } => {
                matches!(source, StorageError::OperationFailed { .. })
            }
            MigrationError::BlobProcessing { .. } => true,
            _ => false,
        }
    }

    pub fn retry_count(&self) -> u32 {
        match self {
            MigrationError::Network { retry_count, .. } => *retry_count,
            _ => 0,
        }
    }

    /// Get error severity for logging/alerting purposes
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            MigrationError::IntegrityCheckFailed { .. } => ErrorSeverity::Critical,
            MigrationError::Storage { source, .. } => match source {
                StorageError::QuotaExceeded { .. } => ErrorSeverity::High,
                StorageError::InitializationFailed { .. } => ErrorSeverity::High,
                _ => ErrorSeverity::Medium,
            },
            MigrationError::CircuitBreakerOpen { .. } => ErrorSeverity::Medium,
            MigrationError::Network { .. } => ErrorSeverity::Low,
            MigrationError::Configuration { .. } => ErrorSeverity::High,
            MigrationError::Authentication { .. } => ErrorSeverity::High,
            _ => ErrorSeverity::Medium,
        }
    }

    /// Check if error indicates a temporary condition
    pub fn is_temporary(&self) -> bool {
        match self {
            MigrationError::Network { .. } => true,
            MigrationError::CircuitBreakerOpen { .. } => true,
            MigrationError::Storage { source, .. } => {
                matches!(source, StorageError::BackendUnavailable { .. })
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}
