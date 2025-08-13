//! Test utilities for concurrent strategy progress tracking
//! This is not a full unit test but provides helper functions to validate progress behavior

#[cfg(test)]
mod tests {
    use super::super::concurrent_strategy::*;
    use std::time::{Duration, SystemTime};
    
    #[tokio::test]
    async fn test_progress_tracker_basic() {
        let tracker = ProgressTracker::new();
        
        // Test initial state
        let progress = tracker.get_current_progress(100).await;
        assert_eq!(progress.total_blobs, 100);
        assert_eq!(progress.processed_blobs, 0);
        assert_eq!(progress.processed_bytes, 0);
        assert_eq!(progress.current_blob_progress, Some(0.0));
        
        // Test recording completions
        tracker.record_blob_completion("blob1".to_string(), 1024);
        let progress = tracker.get_current_progress(100).await;
        assert_eq!(progress.processed_blobs, 1);
        assert_eq!(progress.processed_bytes, 1024);
        assert_eq!(progress.current_blob_progress, Some(1.0));
        
        // Test progress calculation
        for i in 2..=50 {
            tracker.record_blob_completion(format!("blob{}", i), 1024);
        }
        let progress = tracker.get_current_progress(100).await;
        assert_eq!(progress.processed_blobs, 50);
        assert_eq!(progress.processed_bytes, 50 * 1024);
        assert_eq!(progress.current_blob_progress, Some(50.0));
        
        // Test completion
        for i in 51..=100 {
            tracker.record_blob_completion(format!("blob{}", i), 1024);
        }
        let progress = tracker.get_current_progress(100).await;
        assert_eq!(progress.processed_blobs, 100);
        assert_eq!(progress.processed_bytes, 100 * 1024);
        assert_eq!(progress.current_blob_progress, Some(100.0));
    }
    
    #[tokio::test]
    async fn test_progress_tracker_throughput() {
        let tracker = ProgressTracker::new();
        
        // Wait a bit to ensure some time passes
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Record some completions
        for i in 1..=10 {
            tracker.record_blob_completion(format!("blob{}", i), 1024 * i as u64);
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        
        let (blobs_per_sec, bytes_per_sec) = tracker.get_throughput_info();
        
        // Should have some meaningful throughput
        assert!(blobs_per_sec > 0.0);
        assert!(bytes_per_sec.is_some());
        assert!(bytes_per_sec.unwrap() > 0);
    }
    
    #[tokio::test]
    async fn test_progress_update_throttling() {
        let tracker = ProgressTracker::new();
        
        // First completion should always trigger update
        tracker.record_blob_completion("blob1".to_string(), 1024);
        assert!(tracker.should_update_progress().await);
        
        // Test that time-based throttling works
        // Complete more blobs to get past the "processed <= 5" rule
        for i in 2..=6 {
            tracker.record_blob_completion(format!("blob{}", i), 1024);
        }
        
        // Blob 7 - should not trigger immediately (no special rules apply)
        tracker.record_blob_completion("blob7".to_string(), 1024);
        // First call should return false (not enough time passed)
        assert!(!tracker.should_update_progress().await);
        
        // Wait enough time for throttling to reset
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(tracker.should_update_progress().await);
        
        // Test the 10th blob rule - complete blobs to get to a multiple of 10
        for i in 8..10 {
            tracker.record_blob_completion(format!("blob{}", i), 1024);
        }
        
        // The 10th blob should trigger
        tracker.record_blob_completion("blob10".to_string(), 1024);
        assert!(tracker.should_update_progress().await);
        
        // The 20th blob should also trigger
        for i in 11..20 {
            tracker.record_blob_completion(format!("blob{}", i), 1024);
        }
        tracker.record_blob_completion("blob20".to_string(), 1024);
        assert!(tracker.should_update_progress().await);
    }
    
    #[tokio::test]
    async fn test_progress_edge_cases() {
        let tracker = ProgressTracker::new();
        
        // Test zero blobs
        let progress = tracker.get_current_progress(0).await;
        assert_eq!(progress.current_blob_progress, Some(0.0));
        
        // Test single blob
        let progress = tracker.get_current_progress(1).await;
        assert_eq!(progress.current_blob_progress, Some(0.0));
        
        tracker.record_blob_completion("single".to_string(), 1024);
        let progress = tracker.get_current_progress(1).await;
        assert_eq!(progress.current_blob_progress, Some(100.0));
        
        // Test large numbers
        let progress = tracker.get_current_progress(10000).await;
        assert_eq!(progress.processed_blobs, 1);
        assert_eq!(progress.current_blob_progress, Some(0.01)); // 1/10000 * 100
    }
}

// Helper function for manual testing in WASM context
#[cfg(feature = "web")]
pub fn create_test_progress_tracker() -> std::sync::Arc<super::ProgressTracker> {
    std::sync::Arc::new(super::ProgressTracker::new())
}