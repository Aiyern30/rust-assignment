use crate::common::data_types::PerformanceMetrics;
use chrono::Local;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time;

// Metrics collector for benchmarking performance
pub struct MetricsCollector {
    metrics: Arc<Mutex<HashMap<String, Vec<PerformanceMetrics>>>>,
    last_report_time: Instant,
    report_interval: Duration,
    log_to_file: bool,
    log_file: String,
}

impl MetricsCollector {
    pub fn new(config: &crate::config::MetricsConfig) -> Self {
        Self {
            metrics: Arc::new(Mutex::new(HashMap::new())),
            last_report_time: Instant::now(),
            report_interval: Duration::from_millis(config.report_interval_ms),
            log_to_file: config.log_to_file,
            log_file: config.log_file.clone(),
        }
    }
    
    // Add a new metrics record
    pub fn add_metrics(&self, metrics: PerformanceMetrics) {
        let mut metrics_lock = self.metrics.lock().unwrap();
        let entry = metrics_lock.entry(metrics.operation.clone()).or_default();
        entry.push(metrics);
    }
    
    // Generate a report of current metrics
    pub fn generate_report(&self) -> HashMap<String, OperationStats> {
        let metrics_lock = self.metrics.lock().unwrap();
        let mut report = HashMap::new();
        
        for (operation, metrics) in metrics_lock.iter() {
            if metrics.is_empty() {
                continue;
            }
            
            // Calculate statistics
            let total = metrics.len();
            let success_count = metrics.iter().filter(|m| m.success).count();
            let success_rate = success_count as f64 / total as f64 * 100.0;
            
            // Calculate average duration
            let durations: Vec<f64> = metrics
                .iter()
                .filter_map(|m| m.duration_ms)
                .collect();
            
            let avg_duration = if !durations.is_empty() {
                durations.iter().sum::<f64>() / durations.len() as f64
            } else {
                0.0
            };
            
            // Calculate min and max durations
            let min_duration = durations.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max_duration = durations.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            
            // Calculate jitter (standard deviation of durations)
            let jitter = if durations.len() > 1 {
                let mean = avg_duration;
                let variance = durations.iter()
                    .map(|&d| (d - mean).powi(2))
                    .sum::<f64>() / durations.len() as f64;
                variance.sqrt()
            } else {
                0.0
            };
            
            // Calculate missed deadlines
            let mut missed_deadlines = 0;
            for m in metrics {
                if let Some(duration) = m.duration_ms {
                    match m.operation.as_str() {
                        "data_processing" => {
                            if duration > 2.0 {
                                missed_deadlines += 1;
                            }
                        },
                        "data_transmission" => {
                            if duration > 1.0 {
                                missed_deadlines += 1;
                            }
                        },
                        _ => {}
                    }
                }
            }
            
            let stats = OperationStats {
                operation: operation.clone(),
                total_operations: total,
                success_rate,
                avg_duration,
                min_duration: if min_duration.is_finite() { min_duration } else { 0.0 },
                max_duration: if max_duration.is_finite() { max_duration } else { 0.0 },
                jitter,
                missed_deadlines,
            };
            
            report.insert(operation.clone(), stats);
        }
        
        report
    }
    
    // Log report to console and file
    pub fn log_report(&self, report: &HashMap<String, OperationStats>) {
        // Print to console
        println!("--- Performance Report ---");
        println!("Time: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
        println!("{:<20} | {:<10} | {:<10} | {:<15} | {:<15} | {:<15} | {:<10} | {:<15}", 
                 "Operation", "Total", "Success%", "Avg Duration(ms)", "Min Duration(ms)", 
                 "Max Duration(ms)", "Jitter(ms)", "Missed Deadlines");
        println!("{:-<130}", "");
        
        for stats in report.values() {
            println!("{:<20} | {:<10} | {:<10.2} | {:<15.3} | {:<15.3} | {:<15.3} | {:<10.3} | {:<15}", 
                     stats.operation, stats.total_operations, stats.success_rate, 
                     stats.avg_duration, stats.min_duration, stats.max_duration, 
                     stats.jitter, stats.missed_deadlines);
        }
        println!("{:-<130}", "");
        
        // Log to file if enabled
        if self.log_to_file {
            let log = format!(
                "Time: {}\n{:<20} | {:<10} | {:<10} | {:<15} | {:<15} | {:<15} | {:<10} | {:<15}\n{:-<130}\n", 
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                "Operation", "Total", "Success%", "Avg Duration(ms)", "Min Duration(ms)", 
                "Max Duration(ms)", "Jitter(ms)", "Missed Deadlines",
                ""
            );
            
            // Open the file in append mode
            let mut file = match OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.log_file) {
                Ok(file) => file,
                Err(e) => {
                    println!("Failed to open log file: {}", e);
                    return;
                }
            };
            
            // Write header
            if let Err(e) = file.write_all(log.as_bytes()) {
                println!("Failed to write to log file: {}", e);
                return;
            }
            
            // Write data
            for stats in report.values() {
                let line = format!(
                    "{:<20} | {:<10} | {:<10.2} | {:<15.3} | {:<15.3} | {:<15.3} | {:<10.3} | {:<15}\n", 
                    stats.operation, stats.total_operations, stats.success_rate, 
                    stats.avg_duration, stats.min_duration, stats.max_duration, 
                    stats.jitter, stats.missed_deadlines
                );
                
                if let Err(e) = file.write_all(line.as_bytes()) {
                    println!("Failed to write to log file: {}", e);
                    return;
                }
            }
            
            // Write footer
            if let Err(e) = file.write_all(format!("{:-<130}\n\n", "").as_bytes()) {
                println!("Failed to write to log file: {}", e);
            }
        }
    }
    
    // Check if it's time to report metrics
    pub fn should_report(&self) -> bool {
        self.last_report_time.elapsed() >= self.report_interval
    }
    
    // Reset the report timer
    pub fn reset_report_timer(&mut self) {
        self.last_report_time = Instant::now();
    }
    
    // Clear metrics after reporting
    pub fn clear_metrics(&self) {
        let mut metrics_lock = self.metrics.lock().unwrap();
        for (_, metrics) in metrics_lock.iter_mut() {
            metrics.clear();
        }
    }
}

// Statistics for an operation
#[derive(Debug, Clone)]
pub struct OperationStats {
    pub operation: String,
    pub total_operations: usize,
    pub success_rate: f64,
    pub avg_duration: f64,
    pub min_duration: f64,
    pub max_duration: f64,
    pub jitter: f64,
    pub missed_deadlines: usize,
}

// Function to run the metrics collector in real-time
pub async fn run_metrics_collector(
    config: &crate::config::MetricsConfig,
    rx: crossbeam_channel::Receiver<PerformanceMetrics>,
) {
    let mut collector = MetricsCollector::new(config);
    let mut interval = time::interval(Duration::from_millis(100)); // Check every 100ms
    
    loop {
        // Wait for the next check
        interval.tick().await;
        
        // Try to receive metrics (non-blocking)
        loop {
            match rx.try_recv() {
                Ok(metrics) => {
                    collector.add_metrics(metrics);
                },
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // No more metrics in queue
                    break;
                },
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    println!("Metrics channel closed, stopping collector.");
                    return;
                }
            }
        }
        
        // Report metrics if it's time
        if collector.should_report() {
            let report = collector.generate_report();
            collector.log_report(&report);
            collector.reset_report_timer();
            collector.clear_metrics();
        }
    }
}