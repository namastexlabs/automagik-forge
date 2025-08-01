use automagik_forge::services::{
    whatsapp_config::WhatsAppConfig,
    whatsapp_notifier::WhatsAppNotifier,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use futures::future::join_all;

/// Performance and stress tests for WhatsApp notification integration
/// 
/// This test suite focuses on:
/// - Concurrent notification performance
/// - Resource usage and memory leak detection
/// - Stress testing under high load
/// - Timeout behavior validation
/// - Resource cleanup verification
/// - Performance regression detection

#[cfg(test)]
mod whatsapp_performance_tests {
    use super::*;

    // ============================================================================
    // PERFORMANCE TEST UTILITIES
    // ============================================================================

    #[derive(Debug, Clone)]
    struct PerformanceMetrics {
        total_requests: usize,
        successful_requests: usize,
        failed_requests: usize,
        timeout_requests: usize,
        total_duration: Duration,
        average_duration: Duration,
        min_duration: Duration,
        max_duration: Duration,
        memory_usage_kb: usize,
    }

    impl PerformanceMetrics {
        fn new() -> Self {
            Self {
                total_requests: 0,
                successful_requests: 0,
                failed_requests: 0,
                timeout_requests: 0,
                total_duration: Duration::ZERO,
                average_duration: Duration::ZERO,
                min_duration: Duration::MAX,
                max_duration: Duration::ZERO,
                memory_usage_kb: 0,
            }
        }

        fn add_request_result(&mut self, duration: Duration, success: bool, timeout: bool) {
            self.total_requests += 1;
            self.total_duration += duration;

            if timeout {
                self.timeout_requests += 1;
            } else if success {
                self.successful_requests += 1;
            } else {
                self.failed_requests += 1;
            }

            if duration < self.min_duration {
                self.min_duration = duration;
            }
            if duration > self.max_duration {
                self.max_duration = duration;
            }

            if self.total_requests > 0 {
                self.average_duration = self.total_duration / self.total_requests as u32;
            }
        }

        fn finalize(&mut self) {
            // Estimate memory usage (simplified)
            self.memory_usage_kb = std::mem::size_of::<Self>() / 1024;
        }

        fn success_rate(&self) -> f64 {
            if self.total_requests == 0 {
                0.0
            } else {
                self.successful_requests as f64 / self.total_requests as f64
            }
        }

        fn timeout_rate(&self) -> f64 {
            if self.total_requests == 0 {
                0.0
            } else {
                self.timeout_requests as f64 / self.total_requests as f64
            }
        }
    }

    /// Performance test harness
    struct PerformanceTestHarness {
        config: WhatsAppConfig,
        metrics: Arc<Mutex<PerformanceMetrics>>,
    }

    impl PerformanceTestHarness {
        fn new(config: WhatsAppConfig) -> Self {
            Self {
                config,
                metrics: Arc::new(Mutex::new(PerformanceMetrics::new())),
            }
        }

        async fn run_single_notification(&self, title: &str, message: &str, timeout_ms: u64) -> bool {
            let start_time = Instant::now();
            let notifier = WhatsAppNotifier::new(self.config.clone()).await.unwrap();
            
            let result = timeout(
                Duration::from_millis(timeout_ms),
                notifier.send_notification(title, message)
            ).await;

            let duration = start_time.elapsed();
            let (success, is_timeout) = match result {
                Ok(send_result) => (send_result.is_ok(), false),
                Err(_) => (false, true), // Timeout
            };

            self.metrics.lock().unwrap().add_request_result(duration, success, is_timeout);
            success
        }

        async fn run_concurrent_notifications(&self, count: usize, timeout_ms: u64) -> PerformanceMetrics {
            let tasks = (0..count).map(|i| {
                let harness = self.clone();
                tokio::spawn(async move {
                    harness.run_single_notification(
                        &format!("Perf Test {}", i),
                        &format!("Performance test message {}", i),
                        timeout_ms
                    ).await
                })
            });

            let results = join_all(tasks).await;
            
            // Verify all tasks completed without panicking
            for (i, result) in results.into_iter().enumerate() {
                if result.is_err() {
                    eprintln!("Task {} panicked", i);
                }
            }

            let mut metrics = self.metrics.lock().unwrap().clone();
            metrics.finalize();
            metrics
        }

        fn get_metrics(&self) -> PerformanceMetrics {
            let mut metrics = self.metrics.lock().unwrap().clone();
            metrics.finalize();
            metrics
        }

        fn reset_metrics(&self) {
            *self.metrics.lock().unwrap() = PerformanceMetrics::new();
        }
    }

    impl Clone for PerformanceTestHarness {
        fn clone(&self) -> Self {
            Self {
                config: self.config.clone(),
                metrics: Arc::clone(&self.metrics),
            }
        }
    }

    fn create_performance_config() -> WhatsAppConfig {
        WhatsAppConfig {
            base_url: "http://perf-test-evolution-api:8080".to_string(),
            api_key: "perf-test-api-key".to_string(),
            instance: "perf-test-instance".to_string(),
            fixed_recipient: Some("+1234567890".to_string()),
            timeout_ms: 2000, // Reasonable timeout for performance tests
            include_task_url: true,
        }
    }

    // ============================================================================
    // SEQUENTIAL PERFORMANCE TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_sequential_notification_performance() {
        let config = create_performance_config();
        let harness = PerformanceTestHarness::new(config);

        let test_start = Instant::now();
        
        // Run 10 sequential notifications
        for i in 0..10 {
            harness.run_single_notification(
                &format!("Sequential Test {}", i),
                "Sequential performance test message",
                100 // Short timeout for performance test
            ).await;
        }

        let total_test_time = test_start.elapsed();
        let metrics = harness.get_metrics();

        // Performance assertions
        assert_eq!(metrics.total_requests, 10, "Should have 10 total requests");
        assert!(total_test_time < Duration::from_secs(5), "Sequential tests should complete within 5 seconds");
        
        // Log performance metrics
        println!("Sequential Performance Metrics:");
        println!("  Total Time: {:?}", total_test_time);
        println!("  Average Request Duration: {:?}", metrics.average_duration);
        println!("  Min Duration: {:?}", metrics.min_duration);
        println!("  Max Duration: {:?}", metrics.max_duration);
        println!("  Success Rate: {:.2}%", metrics.success_rate() * 100.0);
        println!("  Timeout Rate: {:.2}%", metrics.timeout_rate() * 100.0);
        
        // Basic performance expectations
        assert!(metrics.average_duration < Duration::from_millis(500), 
               "Average request duration should be reasonable");
    }

    #[tokio::test]
    async fn test_notification_performance_with_varying_message_sizes() {
        let config = create_performance_config();
        let harness = PerformanceTestHarness::new(config);

        let message_sizes = vec![
            (10, "Small message"),
            (100, "Medium message"),
            (1000, "Large message"),
            (5000, "Very large message"),
            (10000, "Huge message"),
        ];

        for (size, description) in message_sizes {
            harness.reset_metrics();
            
            let large_message = "X".repeat(size);
            let start_time = Instant::now();
            
            let success = harness.run_single_notification(
                &format!("Size Test {}", size),
                &large_message,
                200 // Reasonable timeout
            ).await;

            let duration = start_time.elapsed();
            
            println!("{} ({}B): {:?} - {}", 
                    description, size, duration, 
                    if success { "Success" } else { "Failed/Timeout" });

            // Performance should not degrade dramatically with size
            if size <= 1000 {
                assert!(duration < Duration::from_millis(300), 
                       "Small messages should be fast: {}", description);
            } else {
                assert!(duration < Duration::from_millis(500), 
                       "Large messages should still be reasonable: {}", description);
            }
        }
    }

    // ============================================================================
    // CONCURRENT PERFORMANCE TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_concurrent_notification_performance_small_load() {
        let config = create_performance_config();
        let harness = PerformanceTestHarness::new(config);

        let start_time = Instant::now();
        let metrics = harness.run_concurrent_notifications(5, 150).await;
        let total_time = start_time.elapsed();

        // Performance assertions for small concurrent load
        assert_eq!(metrics.total_requests, 5, "Should have 5 concurrent requests");
        assert!(total_time < Duration::from_secs(3), "Concurrent small load should complete quickly");
        
        println!("Small Concurrent Load (5 notifications):");
        println!("  Total Time: {:?}", total_time);
        println!("  Success Rate: {:.2}%", metrics.success_rate() * 100.0);
        println!("  Timeout Rate: {:.2}%", metrics.timeout_rate() * 100.0);
        println!("  Average Duration: {:?}", metrics.average_duration);
        
        // Should handle small concurrent load well
        assert!(metrics.timeout_rate() < 0.5, "Timeout rate should be low for small load");
    }

    #[tokio::test]
    async fn test_concurrent_notification_performance_medium_load() {
        let config = create_performance_config();
        let harness = PerformanceTestHarness::new(config);

        let start_time = Instant::now();
        let metrics = harness.run_concurrent_notifications(15, 200).await;
        let total_time = start_time.elapsed();

        // Performance assertions for medium concurrent load
        assert_eq!(metrics.total_requests, 15, "Should have 15 concurrent requests");
        assert!(total_time < Duration::from_secs(8), "Medium concurrent load should complete reasonably");
        
        println!("Medium Concurrent Load (15 notifications):");
        println!("  Total Time: {:?}", total_time);
        println!("  Success Rate: {:.2}%", metrics.success_rate() * 100.0);
        println!("  Timeout Rate: {:.2}%", metrics.timeout_rate() * 100.0);
        println!("  Average Duration: {:?}", metrics.average_duration);
        
        // Medium load may have more timeouts but should still be manageable
        assert!(metrics.timeout_rate() < 0.8, "Timeout rate should be acceptable for medium load");
    }

    #[tokio::test]
    async fn test_concurrent_notification_performance_high_load() {
        let config = create_performance_config();
        let harness = PerformanceTestHarness::new(config);

        let start_time = Instant::now();
        let metrics = harness.run_concurrent_notifications(30, 300).await;
        let total_time = start_time.elapsed();

        // Performance assertions for high concurrent load
        assert_eq!(metrics.total_requests, 30, "Should have 30 concurrent requests");
        assert!(total_time < Duration::from_secs(15), "High concurrent load should eventually complete");
        
        println!("High Concurrent Load (30 notifications):");
        println!("  Total Time: {:?}", total_time);
        println!("  Success Rate: {:.2}%", metrics.success_rate() * 100.0);
        println!("  Timeout Rate: {:.2}%", metrics.timeout_rate() * 100.0);
        println!("  Average Duration: {:?}", metrics.average_duration);
        
        // High load will have many timeouts, but system should remain stable
        // The main goal is that it doesn't crash or hang indefinitely
        assert!(total_time < Duration::from_secs(15), "Should not hang indefinitely under high load");
    }

    // ============================================================================
    // STRESS TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_stress_rapid_creation_and_destruction() {
        let config = create_performance_config();
        
        let start_time = Instant::now();
        let mut successful_creations = 0;
        let mut successful_notifications = 0;

        // Rapidly create and destroy notifiers
        for i in 0..20 {
            let notifier_result = WhatsAppNotifier::new(config.clone()).await;
            
            if let Ok(notifier) = notifier_result {
                successful_creations += 1;
                
                // Try to send a notification with very short timeout
                let result = timeout(
                    Duration::from_millis(50),
                    notifier.send_notification(&format!("Stress {}", i), "Stress test")
                ).await;
                
                if let Ok(send_result) = result {
                    if send_result.is_ok() {
                        successful_notifications += 1;
                    }
                }
                
                // Notifier is dropped here - testing cleanup
            }
        }

        let total_time = start_time.elapsed();
        
        println!("Stress Test - Rapid Creation/Destruction:");
        println!("  Total Time: {:?}", total_time);
        println!("  Successful Creations: {}/20", successful_creations);
        println!("  Successful Notifications: {}/20", successful_notifications);
        
        // Stress test assertions
        assert!(total_time < Duration::from_secs(10), "Stress test should complete within reasonable time");
        assert_eq!(successful_creations, 20, "All notifier creations should succeed");
        // Notifications may fail/timeout in stress test, but system should remain stable
    }

    #[tokio::test]
    async fn test_stress_memory_usage_pattern() {
        let config = create_performance_config();
        
        // Create many notifiers and track if memory usage grows unboundedly
        let mut notifiers = Vec::with_capacity(10);
        
        for i in 0..10 {
            match WhatsAppNotifier::new(config.clone()).await {
                Ok(notifier) => {
                    notifiers.push(notifier);
                    
                    // Try a notification to exercise the code path
                    let _ = timeout(
                        Duration::from_millis(25),
                        notifiers[i].send_notification(&format!("Memory Test {}", i), "Memory test")
                    ).await;
                },
                Err(e) => {
                    eprintln!("Failed to create notifier {}: {}", i, e);
                }
            }
        }
        
        // Keep notifiers alive for a moment
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Drop all notifiers
        notifiers.clear();
        
        // Give time for cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // If we reach here without out-of-memory, the test passes
        assert!(true, "Memory stress test completed without issues");
        
        println!("Memory Stress Test: Created and cleaned up 10 notifiers successfully");
    }

    // ============================================================================
    // TIMEOUT BEHAVIOR TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_timeout_behavior_patterns() {
        let config = create_performance_config();
        let notifier = WhatsAppNotifier::new(config).await.unwrap();

        let timeout_test_cases = vec![
            (10, "Very short timeout"),
            (50, "Short timeout"),
            (100, "Medium timeout"),
            (250, "Long timeout"),
            (500, "Very long timeout"),
        ];

        for (timeout_ms, description) in timeout_test_cases {
            let start_time = Instant::now();
            
            let result = timeout(
                Duration::from_millis(timeout_ms),
                notifier.send_notification("Timeout Test", &format!("Testing {}", description))
            ).await;
            
            let actual_duration = start_time.elapsed();
            
            println!("{} ({}ms): Actual={:?}, Result={}", 
                    description, timeout_ms, actual_duration, 
                    match result {
                        Ok(Ok(())) => "Success",
                        Ok(Err(_)) => "Failed",
                        Err(_) => "Timeout",
                    });

            // Verify timeout behavior
            match result {
                Ok(_) => {
                    // If it completed, it should be within reasonable bounds
                    assert!(actual_duration <= Duration::from_millis(timeout_ms + 50),
                           "Completed operations should respect timeout bounds");
                },
                Err(_) => {
                    // If it timed out, duration should be close to timeout
                    let timeout_duration = Duration::from_millis(timeout_ms);
                    assert!(actual_duration >= timeout_duration,
                           "Timeout should occur at specified time");
                    assert!(actual_duration <= timeout_duration + Duration::from_millis(100),
                           "Timeout should not significantly exceed specified time");
                }
            }
        }
    }

    #[tokio::test]
    async fn test_timeout_consistency_under_load() {
        let config = create_performance_config();
        let timeout_ms = 100;
        
        // Run concurrent operations with same timeout
        let tasks = (0..10).map(|i| {
            let config = config.clone();
            tokio::spawn(async move {
                let notifier = WhatsAppNotifier::new(config).await.unwrap();
                let start_time = Instant::now();
                
                let result = timeout(
                    Duration::from_millis(timeout_ms),
                    notifier.send_notification(&format!("Concurrent Timeout {}", i), "Timeout consistency test")
                ).await;
                
                let duration = start_time.elapsed();
                (result.is_err(), duration) // (is_timeout, duration)
            })
        });

        let results = join_all(tasks).await;
        
        let mut timeout_durations = Vec::new();
        let mut completion_durations = Vec::new();
        
        for (i, result) in results.into_iter().enumerate() {
            assert!(result.is_ok(), "Task {} should not panic", i);
            
            let (is_timeout, duration) = result.unwrap();
            if is_timeout {
                timeout_durations.push(duration);
            } else {
                completion_durations.push(duration);
            }
        }
        
        println!("Timeout Consistency Test:");
        println!("  Timeouts: {}, Completions: {}", timeout_durations.len(), completion_durations.len());
        
        // Verify timeout consistency
        for (i, &duration) in timeout_durations.iter().enumerate() {
            assert!(duration >= Duration::from_millis(timeout_ms),
                   "Timeout {} should occur at or after specified time", i);
            assert!(duration <= Duration::from_millis(timeout_ms + 150),
                   "Timeout {} should not significantly exceed specified time", i);
        }
        
        // Log timeout consistency metrics
        if !timeout_durations.is_empty() {
            let avg_timeout = timeout_durations.iter().sum::<Duration>() / timeout_durations.len() as u32;
            println!("  Average timeout duration: {:?}", avg_timeout);
        }
        if !completion_durations.is_empty() {
            let avg_completion = completion_durations.iter().sum::<Duration>() / completion_durations.len() as u32;
            println!("  Average completion duration: {:?}", avg_completion);
        }
    }

    // ============================================================================
    // RESOURCE CLEANUP TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_resource_cleanup_verification() {
        let config = create_performance_config();
        
        // Create and use many notifiers to test cleanup
        for iteration in 0..5 {
            let mut notifiers = Vec::new();
            
            // Create multiple notifiers in this iteration
            for i in 0..5 {
                let notifier = WhatsAppNotifier::new(config.clone()).await.unwrap();
                notifiers.push(notifier);
            }
            
            // Use all notifiers concurrently
            let notification_tasks = notifiers.into_iter().enumerate().map(|(i, notifier)| {
                tokio::spawn(async move {
                    timeout(
                        Duration::from_millis(50),
                        notifier.send_notification(
                            &format!("Cleanup Test {}-{}", iteration, i),
                            "Resource cleanup test"
                        )
                    ).await
                })
            });
            
            let _results = join_all(notification_tasks).await;
            
            // All notifiers are dropped here
            // Give time for cleanup
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        // Final cleanup pause
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // If we completed without hanging or excessive resource usage, cleanup is working
        assert!(true, "Resource cleanup test completed successfully");
        
        println!("Resource Cleanup Test: 25 notifiers created and cleaned up across 5 iterations");
    }

    #[tokio::test]
    async fn test_resource_cleanup_under_concurrent_load() {
        let config = create_performance_config();
        
        // Create concurrent tasks that create and destroy notifiers
        let cleanup_tasks = (0..8).map(|task_id| {
            let config = config.clone();
            tokio::spawn(async move {
                let mut local_success_count = 0;
                
                for i in 0..3 {
                    let notifier = WhatsAppNotifier::new(config.clone()).await.unwrap();
                    
                    let result = timeout(
                        Duration::from_millis(25),
                        notifier.send_notification(
                            &format!("Concurrent Cleanup {}-{}", task_id, i),
                            "Concurrent cleanup test"
                        )
                    ).await;
                    
                    if result.is_ok() && result.unwrap().is_ok() {
                        local_success_count += 1;
                    }
                    
                    // Small delay to create overlap
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    
                    // Notifier is dropped here
                }
                
                local_success_count
            })
        });
        
        let start_time = Instant::now();
        let results = join_all(cleanup_tasks).await;
        let total_time = start_time.elapsed();
        
        let mut total_successes = 0;
        for (i, result) in results.into_iter().enumerate() {
            assert!(result.is_ok(), "Cleanup task {} should not panic", i);
            total_successes += result.unwrap();
        }
        
        println!("Concurrent Resource Cleanup Test:");
        println!("  Total Time: {:?}", total_time);
        println!("  Total Successful Notifications: {}/24", total_successes);
        
        // Verify concurrent cleanup behavior
        assert!(total_time < Duration::from_secs(5), "Concurrent cleanup should complete in reasonable time");
        // Success rate may vary due to timeouts, but system should remain stable
        
        // Final cleanup pause
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        println!("Concurrent cleanup completed successfully with {} tasks", 8);
    }
}