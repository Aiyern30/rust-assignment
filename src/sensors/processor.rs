use crate::common::data_types::{PerformanceMetrics, SensorData, SensorType};
use rolling_stats::Mean;
use std::collections::HashMap;
use std::time::Instant;

pub struct DataProcessor {
    // Moving average filters for each sensor
    moving_averages: HashMap<String, Mean>,
    // Window size for moving average calculation
    window_size: usize,
    // Anomaly detection thresholds for each sensor type
    anomaly_thresholds: HashMap<SensorType, f64>,
}

impl DataProcessor {
    pub fn new(window_size: usize) -> Self {
        let mut anomaly_thresholds = HashMap::new();

        // Set default thresholds for each sensor type
        anomaly_thresholds.insert(SensorType::Force, 2.5); // 2.5 standard deviations
        anomaly_thresholds.insert(SensorType::Position, 3.0); // 3.0 standard deviations
        anomaly_thresholds.insert(SensorType::Velocity, 2.8); // 2.8 standard deviations
        anomaly_thresholds.insert(SensorType::Temperature, 3.5); // 3.5 standard deviations

        Self {
            moving_averages: HashMap::new(),
            window_size,
            anomaly_thresholds,
        }
    }

    // Process a single sensor reading
    pub fn process(&mut self, raw_data: SensorData) -> (SensorData, PerformanceMetrics) {
        let mut metrics = PerformanceMetrics::new("data_processing");

        // Get or create moving average for this sensor
        let moving_avg = self
            .moving_averages
            .entry(raw_data.sensor_id.clone())
            .or_insert_with(|| Mean::with_capacity(self.window_size));

        // Add current value to moving average
        moving_avg.push(raw_data.value);

        // Calculate filtered value (smoothed)
        let filtered_value = if moving_avg.len() > 0 {
            moving_avg.mean()
        } else {
            raw_data.value
        };

        // Check for anomalies using Z-score if we have enough data
        let mut is_anomaly = raw_data.is_anomaly; // Keep original flag if it was set
        let mut confidence = 0.95; // Default confidence

        if moving_avg.len() >= 10 {
            // Get threshold for this sensor type
            let threshold = self
                .anomaly_thresholds
                .get(&raw_data.reading_type)
                .cloned()
                .unwrap_or(3.0);

            // Calculate standard deviation (approximation)
            let std_dev = 0.1 * filtered_value.abs(); // Simple approximation

            // Calculate Z-score
            let z_score = (raw_data.value - filtered_value).abs() / std_dev;

            // Determine if this is an anomaly based on Z-score
            is_anomaly = z_score > threshold;

            // Calculate confidence based on Z-score
            confidence = 1.0 - (z_score / (threshold * 2.0)).min(0.9);
            confidence = confidence.max(0.1); // Ensure minimum confidence
        }

        // Create processed data
        let processed_data = SensorData {
            timestamp: raw_data.timestamp,
            sensor_id: raw_data.sensor_id,
            reading_type: raw_data.reading_type,
            value: filtered_value, // Use filtered value
            is_anomaly,
            confidence,
        };

        metrics.complete(true);
        (processed_data, metrics)
    }

    // Adjust anomaly threshold for a specific sensor type
    pub fn adjust_threshold(&mut self, sensor_type: SensorType, new_threshold: f64) {
        self.anomaly_thresholds.insert(sensor_type, new_threshold);
    }
}

// Function to run the data processor in real-time
pub async fn run_processor(
    config: &crate::config::ProcessorConfig,
    rx: crossbeam_channel::Receiver<SensorData>,
    tx: crossbeam_channel::Sender<SensorData>,
    metrics_tx: crossbeam_channel::Sender<PerformanceMetrics>,
) {
    let mut processor = DataProcessor::new(config.window_size);

    // Process data in real time
    loop {
        // Try to receive data with timeout
        match rx.recv() {
            Ok(raw_data) => {
                let start = Instant::now();

                // Process the data
                let (processed_data, metrics) = processor.process(raw_data);

                // Send metrics
                let _ = metrics_tx.send(metrics);

                // Check if processing took too long
                let processing_time = start.elapsed();
                if processing_time.as_millis() > 2 {
                    println!("Warning: Processing took too long: {:?}", processing_time);
                }

                // Send processed data
                if tx.send(processed_data).is_err() {
                    println!("Transmitter has been dropped, stopping processor.");
                    break;
                }
            }
            Err(_) => {
                println!("Sensor channel closed, stopping processor.");
                break;
            }
        }
    }
}
