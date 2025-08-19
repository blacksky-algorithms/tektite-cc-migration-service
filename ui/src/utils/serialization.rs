//! Serialization utilities for WASM/JavaScript compatibility

use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Import console macros from our crate
use crate::console_warn;

/// Maximum safe integer value in JavaScript (2^53 - 1)
const MAX_SAFE_INTEGER: u64 = (1u64 << 53) - 1;

/// Serialize u64 values safely for JavaScript/WASM
/// Large numbers (> 2^53-1) are serialized as strings to avoid precision loss
pub fn serialize_u64<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    // For WASM/JS, serialize large numbers as strings to avoid BigInt issues
    if *value > MAX_SAFE_INTEGER {
        console_warn!(
            "[Serialization] Serializing large u64 {} as string to avoid BigInt issues",
            value
        );
        serializer.serialize_str(&value.to_string())
    } else {
        serializer.serialize_u64(*value)
    }
}

/// Deserialize u64 values that might be strings or numbers
pub fn deserialize_u64_flexible<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum U64OrString {
        Number(u64),
        String(String),
    }

    match U64OrString::deserialize(deserializer)? {
        U64OrString::Number(n) => Ok(n),
        U64OrString::String(s) => s.parse().map_err(serde::de::Error::custom),
    }
}

/// Helper function to safely format u64 values for logging/display
/// Avoids BigInt serialization issues in JavaScript contexts
pub fn format_bytes(bytes: u64) -> String {
    bytes.to_string()
}

/// Helper function to safely format numeric values for logging
pub fn format_number<T: std::fmt::Display>(value: T) -> String {
    value.to_string()
}

/// Format bytes with human-readable units
pub fn format_bytes_human(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: u64 = 1024;

    if bytes < THRESHOLD {
        return format!("{} B", bytes);
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD as f64 && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD as f64;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Safe JSON serialization for JavaScript-compatible types
pub fn serialize_js_safe<T>(value: &T) -> Result<String, serde_json::Error>
where
    T: Serialize,
{
    serde_json::to_string(value)
}

/// Storage information with proper serialization for WASM/JS
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StorageInfo {
    #[serde(
        serialize_with = "serialize_u64",
        deserialize_with = "deserialize_u64_flexible"
    )]
    pub current_usage_bytes: u64,

    #[serde(
        serialize_with = "serialize_u64",
        deserialize_with = "deserialize_u64_flexible"
    )]
    pub available_bytes: u64,

    #[serde(
        serialize_with = "serialize_u64",
        deserialize_with = "deserialize_u64_flexible"
    )]
    pub total_capacity_bytes: u64,

    pub backend_name: String,
    pub is_available: bool,
}

impl StorageInfo {
    pub fn new(
        current_usage: u64,
        available: u64,
        total_capacity: u64,
        backend: &str,
        available_status: bool,
    ) -> Self {
        Self {
            current_usage_bytes: current_usage,
            available_bytes: available,
            total_capacity_bytes: total_capacity,
            backend_name: backend.to_string(),
            is_available: available_status,
        }
    }

    pub fn usage_percentage(&self) -> f64 {
        if self.total_capacity_bytes == 0 {
            0.0
        } else {
            (self.current_usage_bytes as f64 / self.total_capacity_bytes as f64) * 100.0
        }
    }

    pub fn format_usage(&self) -> String {
        format!(
            "{} / {} ({}%)",
            format_bytes_human(self.current_usage_bytes),
            format_bytes_human(self.total_capacity_bytes),
            self.usage_percentage() as u32
        )
    }
}

/// Blob statistics with safe serialization
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlobStats {
    #[serde(
        serialize_with = "serialize_u64",
        deserialize_with = "deserialize_u64_flexible"
    )]
    pub total_blobs: u64,

    #[serde(
        serialize_with = "serialize_u64",
        deserialize_with = "deserialize_u64_flexible"
    )]
    pub total_bytes: u64,

    #[serde(
        serialize_with = "serialize_u64",
        deserialize_with = "deserialize_u64_flexible"
    )]
    pub processed_blobs: u64,

    #[serde(
        serialize_with = "serialize_u64",
        deserialize_with = "deserialize_u64_flexible"
    )]
    pub processed_bytes: u64,

    pub start_time: String,
    pub status: String,
}

impl BlobStats {
    pub fn new() -> Self {
        Self {
            total_blobs: 0,
            total_bytes: 0,
            processed_blobs: 0,
            processed_bytes: 0,
            start_time: chrono::Utc::now().to_rfc3339(),
            status: "initialized".to_string(),
        }
    }

    pub fn progress_percentage(&self) -> f64 {
        if self.total_blobs == 0 {
            0.0
        } else {
            (self.processed_blobs as f64 / self.total_blobs as f64) * 100.0
        }
    }

    pub fn throughput_blobs_per_sec(&self, elapsed_seconds: f64) -> f64 {
        if elapsed_seconds > 0.0 {
            self.processed_blobs as f64 / elapsed_seconds
        } else {
            0.0
        }
    }

    pub fn throughput_bytes_per_sec(&self, elapsed_seconds: f64) -> f64 {
        if elapsed_seconds > 0.0 {
            self.processed_bytes as f64 / elapsed_seconds
        } else {
            0.0
        }
    }
}

impl Default for BlobStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_format_bytes_human() {
        assert_eq!(format_bytes_human(512), "512 B");
        assert_eq!(format_bytes_human(1024), "1.00 KB");
        assert_eq!(format_bytes_human(1536), "1.50 KB");
        assert_eq!(format_bytes_human(1048576), "1.00 MB");
        assert_eq!(format_bytes_human(1073741824), "1.00 GB");
    }

    #[test]
    fn test_storage_info_serialization() {
        let storage = StorageInfo::new(512 * 1024, 1024 * 1024, 2048 * 1024, "opfs", true);

        // Should serialize without errors
        let json = serde_json::to_string(&storage).unwrap();
        assert!(json.contains("opfs"));

        // Should deserialize back correctly
        let deserialized: StorageInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.current_usage_bytes, 512 * 1024);
        assert_eq!(deserialized.backend_name, "opfs");
    }

    #[test]
    fn test_blob_stats_calculations() {
        let mut stats = BlobStats::new();
        stats.total_blobs = 100;
        stats.processed_blobs = 25;

        assert_eq!(stats.progress_percentage(), 25.0);

        let throughput = stats.throughput_blobs_per_sec(10.0); // 10 seconds
        assert_eq!(throughput, 2.5); // 25 blobs / 10 seconds
    }

    #[cfg(target_arch = "wasm32")]
    #[test]
    fn test_large_number_serialization() {
        // Test serialization of large numbers on WASM
        let large_number = MAX_SAFE_INTEGER + 1;

        #[derive(Serialize)]
        struct TestStruct {
            #[serde(serialize_with = "serialize_u64")]
            value: u64,
        }

        let test = TestStruct {
            value: large_number,
        };
        let json = serde_json::to_string(&test).unwrap();

        // Should be serialized as a string for large numbers
        assert!(json.contains(&format!("\"{}\"", large_number)));
    }
}

// Add chrono dependency for timestamps - we'll need to add this to Cargo.toml
// For now, using a simple timestamp approach
mod chrono {
    pub struct Utc;

    impl Utc {
        pub fn now() -> DateTime {
            DateTime
        }
    }

    pub struct DateTime;

    impl DateTime {
        pub fn to_rfc3339(&self) -> String {
            // Use js_sys::Date for WASM32 target
            format!("{}", js_sys::Date::new_0().to_iso_string())
        }
    }
}
