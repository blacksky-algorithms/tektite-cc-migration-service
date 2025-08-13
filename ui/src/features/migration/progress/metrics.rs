//! Migration metrics collection and analysis

use std::time::{Duration, Instant};

/// Collects and analyzes migration performance metrics
#[derive(Debug, Clone)]
pub struct MigrationMetrics {
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub repository_export_duration: Option<Duration>,
    pub repository_import_duration: Option<Duration>,
    pub blob_migration_duration: Option<Duration>,
    pub preferences_migration_duration: Option<Duration>,
    pub total_blobs: u32,
    pub migrated_blobs: u32,
    pub failed_blobs: u32,
    pub total_bytes: u64,
    pub strategy_used: Option<String>,
}

impl MigrationMetrics {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            end_time: None,
            repository_export_duration: None,
            repository_import_duration: None,
            blob_migration_duration: None,
            preferences_migration_duration: None,
            total_blobs: 0,
            migrated_blobs: 0,
            failed_blobs: 0,
            total_bytes: 0,
            strategy_used: None,
        }
    }
    
    pub fn complete(&mut self) {
        self.end_time = Some(Instant::now());
    }
    
    pub fn total_duration(&self) -> Option<Duration> {
        self.end_time.map(|end| end.duration_since(self.start_time))
    }
    
    pub fn blobs_per_second(&self) -> Option<f64> {
        self.total_duration().map(|duration| {
            if duration.as_secs() > 0 {
                self.migrated_blobs as f64 / duration.as_secs_f64()
            } else {
                0.0
            }
        })
    }
    
    pub fn bytes_per_second(&self) -> Option<f64> {
        self.total_duration().map(|duration| {
            if duration.as_secs() > 0 {
                self.total_bytes as f64 / duration.as_secs_f64()
            } else {
                0.0
            }
        })
    }
    
    pub fn success_rate(&self) -> f64 {
        if self.total_blobs > 0 {
            self.migrated_blobs as f64 / self.total_blobs as f64
        } else {
            0.0
        }
    }
}

impl Default for MigrationMetrics {
    fn default() -> Self {
        Self::new()
    }
}