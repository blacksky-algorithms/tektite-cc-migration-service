//! Real-time Storage Monitoring and Capacity Prediction
//!
//! This module provides comprehensive storage monitoring capabilities including
//! real-time usage tracking, capacity prediction, performance metrics, and
//! intelligent recommendations for blob migration optimization.

use crate::services::blob::blob_fallback_manager::FallbackBlobManager;
use crate::services::blob::blob_manager_trait::{BlobManagerError, BlobManagerTrait};
use crate::{console_debug, console_info, console_warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Real-time storage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetrics {
    pub backend_name: String,
    pub current_usage_bytes: u64,
    pub total_capacity_bytes: u64,
    pub available_bytes: u64,
    pub usage_percentage: f64,
    pub blob_count: u32,
    pub average_blob_size: u64,
    pub last_updated: u64,
    pub trend_data: Vec<UsageDataPoint>,
}

/// Single data point for usage trending
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageDataPoint {
    pub timestamp: u64,
    pub usage_bytes: u64,
    pub blob_count: u32,
    pub operation_type: String, // "store", "retrieve", "cleanup"
}

/// Performance metrics for storage operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub backend_name: String,
    pub average_store_time_ms: f64,
    pub average_retrieve_time_ms: f64,
    pub store_success_rate: f64,
    pub retrieve_success_rate: f64,
    pub throughput_mbps: f64,
    pub total_operations: u64,
    pub failed_operations: u64,
    pub last_updated: u64,
}

/// Capacity prediction based on current trends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapacityPrediction {
    pub backend_name: String,
    pub current_usage_bytes: u64,
    pub predicted_full_date: Option<u64>, // Timestamp when storage might be full
    pub days_until_full: Option<f64>,
    pub confidence_level: f64, // 0.0 to 1.0
    pub recommended_action: String,
    pub trend_direction: TrendDirection,
    pub growth_rate_bytes_per_day: f64,
}

/// Direction of storage usage trend
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrendDirection {
    Growing,
    Stable,
    Decreasing,
    InsufficientData,
}

impl std::fmt::Display for TrendDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrendDirection::Growing => write!(f, "Growing"),
            TrendDirection::Stable => write!(f, "Stable"),
            TrendDirection::Decreasing => write!(f, "Decreasing"),
            TrendDirection::InsufficientData => write!(f, "Insufficient Data"),
        }
    }
}

/// Real-time storage monitor with predictive capabilities
pub struct StorageMonitor {
    metrics_history: HashMap<String, Vec<StorageMetrics>>,
    performance_history: HashMap<String, Vec<PerformanceMetrics>>,
    max_history_points: usize,
}

impl Default for StorageMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageMonitor {
    /// Create a new storage monitor
    pub fn new() -> Self {
        Self {
            metrics_history: HashMap::new(),
            performance_history: HashMap::new(),
            max_history_points: 100, // Keep last 100 data points
        }
    }

    /// Record current storage metrics for a backend
    pub async fn record_metrics(
        &mut self,
        blob_manager: &FallbackBlobManager,
    ) -> Result<StorageMetrics, BlobManagerError> {
        let (backend_name, _) = blob_manager.get_active_backend_info();
        console_debug!("{}", format!(
            "📊 [StorageMonitor] Recording metrics for {} backend",
            backend_name
        ));

        let current_usage = blob_manager.get_storage_usage().await.unwrap_or(0);
        let total_capacity = blob_manager.estimate_storage_capacity().await.unwrap_or(0);
        let available_bytes = if total_capacity == u64::MAX {
            u64::MAX
        } else {
            total_capacity.saturating_sub(current_usage)
        };

        let usage_percentage = if total_capacity == 0 || total_capacity == u64::MAX {
            0.0
        } else {
            (current_usage as f64 / total_capacity as f64) * 100.0
        };

        let timestamp = js_sys::Date::now() as u64;

        // Get existing trend data for this backend
        let existing_metrics = self
            .metrics_history
            .get(backend_name)
            .and_then(|history| history.last());
        let mut trend_data = existing_metrics
            .map(|m| m.trend_data.clone())
            .unwrap_or_default();

        // Add new data point
        trend_data.push(UsageDataPoint {
            timestamp,
            usage_bytes: current_usage,
            blob_count: 0, // Would need to track this separately
            operation_type: "measurement".to_string(),
        });

        // Keep only recent data points
        if trend_data.len() > self.max_history_points {
            trend_data = trend_data
                .into_iter()
                .rev()
                .take(self.max_history_points)
                .rev()
                .collect();
        }

        let metrics = StorageMetrics {
            backend_name: backend_name.to_string(),
            current_usage_bytes: current_usage,
            total_capacity_bytes: total_capacity,
            available_bytes,
            usage_percentage,
            blob_count: 0,        // Would need to track this
            average_blob_size: 0, // Would need to calculate this
            last_updated: timestamp,
            trend_data,
        };

        // Store in history
        let history = self
            .metrics_history
            .entry(backend_name.to_string())
            .or_default();
        history.push(metrics.clone());

        // Keep history size manageable
        if history.len() > self.max_history_points {
            history.remove(0);
        }

        console_info!("{}", format!(
            "📊 [StorageMonitor] Recorded metrics: {:.1}% used ({:.1} MB / {})",
            usage_percentage,
            current_usage as f64 / 1_048_576.0,
            if total_capacity == u64::MAX {
                "Unlimited".to_string()
            } else {
                format!("{:.1} MB", total_capacity as f64 / 1_048_576.0)
            }
        ));

        Ok(metrics)
    }

    /// Record performance metrics for storage operations
    pub fn record_performance(
        &mut self,
        backend_name: &str,
        operation: &str,
        duration_ms: f64,
        success: bool,
        bytes_transferred: u64,
    ) {
        console_debug!("{}", format!(
            "⚡ [StorageMonitor] Recording performance: {} {} in {:.2}ms ({})",
            backend_name,
            operation,
            duration_ms,
            if success { "success" } else { "failed" }
        ));

        let timestamp = js_sys::Date::now() as u64;
        let throughput_mbps = if duration_ms > 0.0 {
            (bytes_transferred as f64 / 1_048_576.0) / (duration_ms / 1000.0)
        } else {
            0.0
        };

        // Get or create performance metrics for this backend
        let history = self
            .performance_history
            .entry(backend_name.to_string())
            .or_default();

        // Create new metrics or update existing
        let new_metrics = if let Some(last_metrics) = history.last() {
            let total_ops = last_metrics.total_operations + 1;
            let failed_ops = if success {
                last_metrics.failed_operations
            } else {
                last_metrics.failed_operations + 1
            };

            PerformanceMetrics {
                backend_name: backend_name.to_string(),
                average_store_time_ms: if operation == "store" {
                    (last_metrics.average_store_time_ms + duration_ms) / 2.0
                } else {
                    last_metrics.average_store_time_ms
                },
                average_retrieve_time_ms: if operation == "retrieve" {
                    (last_metrics.average_retrieve_time_ms + duration_ms) / 2.0
                } else {
                    last_metrics.average_retrieve_time_ms
                },
                store_success_rate: if operation == "store" {
                    ((last_metrics.store_success_rate * last_metrics.total_operations as f64)
                        + if success { 1.0 } else { 0.0 })
                        / total_ops as f64
                        * 100.0
                } else {
                    last_metrics.store_success_rate
                },
                retrieve_success_rate: if operation == "retrieve" {
                    ((last_metrics.retrieve_success_rate * last_metrics.total_operations as f64)
                        + if success { 1.0 } else { 0.0 })
                        / total_ops as f64
                        * 100.0
                } else {
                    last_metrics.retrieve_success_rate
                },
                throughput_mbps: (last_metrics.throughput_mbps + throughput_mbps) / 2.0,
                total_operations: total_ops,
                failed_operations: failed_ops,
                last_updated: timestamp,
            }
        } else {
            PerformanceMetrics {
                backend_name: backend_name.to_string(),
                average_store_time_ms: if operation == "store" {
                    duration_ms
                } else {
                    0.0
                },
                average_retrieve_time_ms: if operation == "retrieve" {
                    duration_ms
                } else {
                    0.0
                },
                store_success_rate: if operation == "store" && success {
                    100.0
                } else {
                    0.0
                },
                retrieve_success_rate: if operation == "retrieve" && success {
                    100.0
                } else {
                    0.0
                },
                throughput_mbps,
                total_operations: 1,
                failed_operations: if success { 0 } else { 1 },
                last_updated: timestamp,
            }
        };

        history.push(new_metrics);

        // Keep history size manageable
        if history.len() > self.max_history_points {
            history.remove(0);
        }
    }

    /// Predict storage capacity based on usage trends
    pub fn predict_capacity(&self, backend_name: &str) -> Option<CapacityPrediction> {
        console_debug!("{}", format!(
            "🔮 [StorageMonitor] Predicting capacity for {} backend",
            backend_name
        ));

        let metrics_history = self.metrics_history.get(backend_name)?;
        if metrics_history.len() < 2 {
            console_warn!("⚠️ [StorageMonitor] Insufficient data for capacity prediction (need at least 2 data points)");
            return Some(CapacityPrediction {
                backend_name: backend_name.to_string(),
                current_usage_bytes: metrics_history.last()?.current_usage_bytes,
                predicted_full_date: None,
                days_until_full: None,
                confidence_level: 0.0,
                recommended_action: "Continue monitoring - insufficient data for prediction"
                    .to_string(),
                trend_direction: TrendDirection::InsufficientData,
                growth_rate_bytes_per_day: 0.0,
            });
        }

        let latest = metrics_history.last()?;
        let oldest = &metrics_history[metrics_history.len().saturating_sub(10).min(0)]; // Use last 10 points max

        let time_diff_days =
            (latest.last_updated - oldest.last_updated) as f64 / (1000.0 * 60.0 * 60.0 * 24.0);
        let usage_diff_bytes =
            latest.current_usage_bytes as i64 - oldest.current_usage_bytes as i64;

        if time_diff_days <= 0.0 {
            console_warn!("⚠️ [StorageMonitor] Invalid time difference for prediction");
            return None;
        }

        let growth_rate_bytes_per_day = usage_diff_bytes as f64 / time_diff_days;

        let trend_direction = if growth_rate_bytes_per_day > 1024.0 * 1024.0 {
            // > 1MB/day
            TrendDirection::Growing
        } else if growth_rate_bytes_per_day < -1024.0 * 1024.0 {
            // < -1MB/day
            TrendDirection::Decreasing
        } else {
            TrendDirection::Stable
        };

        let (predicted_full_date, days_until_full) =
            if growth_rate_bytes_per_day > 0.0 && latest.total_capacity_bytes != u64::MAX {
                let remaining_bytes = latest
                    .total_capacity_bytes
                    .saturating_sub(latest.current_usage_bytes)
                    as f64;
                let days_until_full = remaining_bytes / growth_rate_bytes_per_day;
                let predicted_full_timestamp =
                    latest.last_updated + (days_until_full * 24.0 * 60.0 * 60.0 * 1000.0) as u64;
                (Some(predicted_full_timestamp), Some(days_until_full))
            } else {
                (None, None)
            };

        let confidence_level = (metrics_history.len().min(10) as f64 / 10.0).min(1.0);

        let recommended_action = match trend_direction {
            TrendDirection::Growing => {
                if let Some(days) = days_until_full {
                    if days < 7.0 {
                        "URGENT: Storage will be full within a week - clean up or upgrade storage immediately".to_string()
                    } else if days < 30.0 {
                        "WARNING: Storage will be full within a month - plan cleanup or storage expansion".to_string()
                    } else {
                        "Monitor storage usage - growth detected but not immediate concern"
                            .to_string()
                    }
                } else {
                    "Monitor storage usage - unlimited storage with growing usage detected"
                        .to_string()
                }
            }
            TrendDirection::Decreasing => {
                "Storage usage is decreasing - continue current practices".to_string()
            }
            TrendDirection::Stable => {
                "Storage usage is stable - no immediate action needed".to_string()
            }
            TrendDirection::InsufficientData => {
                "Continue monitoring to gather more data for accurate predictions".to_string()
            }
        };

        console_info!("{}", format!(
            "🔮 [StorageMonitor] Capacity prediction: {} trend, {:.1} MB/day growth, {} confidence",
            trend_direction.to_string(),
            growth_rate_bytes_per_day / 1_048_576.0,
            (confidence_level * 100.0) as u32
        ));

        Some(CapacityPrediction {
            backend_name: backend_name.to_string(),
            current_usage_bytes: latest.current_usage_bytes,
            predicted_full_date,
            days_until_full,
            confidence_level,
            recommended_action,
            trend_direction,
            growth_rate_bytes_per_day,
        })
    }

    /// Get comprehensive storage status report
    pub fn get_status_report(&self, backend_name: &str) -> Option<StorageStatusReport> {
        let latest_metrics = self.metrics_history.get(backend_name)?.last()?.clone();
        let latest_performance = self
            .performance_history
            .get(backend_name)
            .and_then(|h| h.last())
            .cloned();
        let capacity_prediction = self.predict_capacity(backend_name);

        Some(StorageStatusReport {
            metrics: latest_metrics,
            performance: latest_performance,
            prediction: capacity_prediction,
            health_score: self.calculate_health_score(backend_name),
            recommendations: self.generate_recommendations(backend_name),
        })
    }

    /// Calculate a health score (0-100) for the storage backend
    fn calculate_health_score(&self, backend_name: &str) -> u32 {
        let mut score = 100u32;

        // Check usage percentage
        if let Some(metrics) = self
            .metrics_history
            .get(backend_name)
            .and_then(|h| h.last())
        {
            if metrics.usage_percentage > 90.0 {
                score = score.saturating_sub(30);
            } else if metrics.usage_percentage > 80.0 {
                score = score.saturating_sub(15);
            } else if metrics.usage_percentage > 70.0 {
                score = score.saturating_sub(5);
            }
        }

        // Check performance
        if let Some(perf) = self
            .performance_history
            .get(backend_name)
            .and_then(|h| h.last())
        {
            if perf.store_success_rate < 90.0 {
                score = score.saturating_sub(20);
            } else if perf.store_success_rate < 95.0 {
                score = score.saturating_sub(10);
            }

            if perf.retrieve_success_rate < 95.0 {
                score = score.saturating_sub(15);
            } else if perf.retrieve_success_rate < 98.0 {
                score = score.saturating_sub(5);
            }
        }

        // Check capacity prediction
        if let Some(prediction) = self.predict_capacity(backend_name) {
            if let Some(days) = prediction.days_until_full {
                if days < 7.0 {
                    score = score.saturating_sub(25);
                } else if days < 30.0 {
                    score = score.saturating_sub(10);
                }
            }
        }

        score
    }

    /// Generate actionable recommendations for the backend
    fn generate_recommendations(&self, backend_name: &str) -> Vec<String> {
        let mut recommendations = Vec::new();

        if let Some(metrics) = self
            .metrics_history
            .get(backend_name)
            .and_then(|h| h.last())
        {
            if metrics.usage_percentage > 85.0 {
                recommendations.push("Consider cleaning up old or unnecessary blobs".to_string());
            }
            if metrics.usage_percentage > 95.0 {
                recommendations.push("URGENT: Free up storage space immediately".to_string());
            }
        }

        if let Some(perf) = self
            .performance_history
            .get(backend_name)
            .and_then(|h| h.last())
        {
            if perf.store_success_rate < 95.0 {
                recommendations
                    .push("Storage operations are failing - check backend health".to_string());
            }
            if perf.average_store_time_ms > 5000.0 {
                recommendations.push(
                    "Storage operations are slow - consider backend optimization".to_string(),
                );
            }
        }

        if let Some(prediction) = self.predict_capacity(backend_name) {
            if let Some(days) = prediction.days_until_full {
                if days < 30.0 {
                    recommendations.push(format!(
                        "Plan storage expansion - only {:.1} days until full",
                        days
                    ));
                }
            }
        }

        if recommendations.is_empty() {
            recommendations
                .push("Storage backend is healthy - continue current practices".to_string());
        }

        recommendations
    }

    /// Log comprehensive monitoring summary
    pub fn log_monitoring_summary(&self, backend_name: &str) {
        console_info!("{}", format!(
            "📊 [StorageMonitor] === MONITORING SUMMARY FOR {} ===",
            backend_name
        ));

        if let Some(report) = self.get_status_report(backend_name) {
            console_info!("{}", format!(
                "💾 [StorageMonitor] Storage Usage: {:.1}% ({:.1} MB used)",
                report.metrics.usage_percentage,
                report.metrics.current_usage_bytes as f64 / 1_048_576.0
            ));

            if let Some(perf) = &report.performance {
                console_info!("{}", format!(
                    "⚡ [StorageMonitor] Performance: {:.1}% success rate, {:.2} MB/s throughput",
                    (perf.store_success_rate + perf.retrieve_success_rate) / 2.0,
                    perf.throughput_mbps
                ));
            }

            if let Some(pred) = &report.prediction {
                if let Some(days) = pred.days_until_full {
                    console_info!("{}", format!("🔮 [StorageMonitor] Capacity Prediction: {:.1} days until full ({}% confidence)", 
                                   days, (pred.confidence_level * 100.0) as u32));
                }
                console_info!("{}", format!(
                    "📈 [StorageMonitor] Trend: {} ({:.1} MB/day growth)",
                    pred.trend_direction.to_string(),
                    pred.growth_rate_bytes_per_day / 1_048_576.0
                ));
            }

            console_info!("{}", format!(
                "🏥 [StorageMonitor] Health Score: {}/100",
                report.health_score
            ));

            console_info!("💡 [StorageMonitor] Recommendations:");
            for (i, rec) in report.recommendations.iter().enumerate() {
                console_info!("{}", format!("   {}. {}", i + 1, rec));
            }
        } else {
            console_warn!("{}", format!(
                "⚠️ [StorageMonitor] No monitoring data available for {}",
                backend_name
            ));
        }

        console_info!("📊 [StorageMonitor] === END MONITORING SUMMARY ===");
    }
}

/// Comprehensive storage status report
#[derive(Debug, Clone)]
pub struct StorageStatusReport {
    pub metrics: StorageMetrics,
    pub performance: Option<PerformanceMetrics>,
    pub prediction: Option<CapacityPrediction>,
    pub health_score: u32,
    pub recommendations: Vec<String>,
}
