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

    #[error("Unknown error: {message}")]
    Unknown { message: String },
}

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
}
