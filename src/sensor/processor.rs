use crate::common::data_types::{PerformanceMetrics, SensorData, SensorType};
use rolling_stats::Stats;
use std::collections::HashMap;
use std::time::Instant;

pub struct DataProcessor {
    // Moving average filters for each sensor
    moving_averages: HashMap<String, Stats<f64>>,
    // Window size for moving average calculation (not used directly by Stats)
    _window_size: usize,
    // Anomaly detection thresholds for each sensor type
    anomaly_thresholds: HashMap<SensorType, f64>,
}

impl DataProcessor {
    pub fn new(_window_size: usize) -> Self {
        let mut anomaly_thresholds = HashMap::new();

        // Set default thresholds for each sensor type
        anomaly_thresholds.insert(SensorType::Force, 2.5); // 2.5 standard deviations
        anomaly_thresholds.insert(SensorType::Position, 3.0); // 3.0 standard deviations
        anomaly_thresholds.insert(SensorType::Velocity, 2.8); // 2.8 standard deviations
        anomaly_thresholds.insert(SensorType::Temperature, 3.5); // 3.5 standard deviations

        Self {
            moving_averages: HashMap::new(),
            _window_size,
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
            .or_insert_with(Stats::new);

        // Add current value to moving average
        // FIXED: Changed from push() to update()
        moving_avg.update(raw_data.value);

        // Compute filtered (smoothed) value
        // FIXED: mean is a field of type f64, not Option<f64>
        let filtered_value = moving_avg.mean;

        // Anomaly detection using Z-score
        let mut is_anomaly = raw_data.is_anomaly; // Preserve original flag
        let mut confidence = 0.95; // Default confidence

        // FIXED: Using count instead of len
        if moving_avg.count >= 10 {
            // Get threshold for this sensor type
            let threshold = self
                .anomaly_thresholds
                .get(&raw_data.reading_type)
                .cloned()
                .unwrap_or(3.0);

            // Approximate standard deviation
            let std_dev = 0.1 * filtered_value.abs();

            if std_dev > 0.0 {
                // Z-score = (raw - mean) / std_dev
                let z_score = (raw_data.value - filtered_value).abs() / std_dev;

                is_anomaly = z_score > threshold;
                confidence = 1.0 - (z_score / (threshold * 2.0)).min(0.9);
                confidence = confidence.max(0.1); // Ensure minimum confidence
            }
        }

        let processed_data = SensorData {
            timestamp: raw_data.timestamp,
            sensor_id: raw_data.sensor_id,
            reading_type: raw_data.reading_type,
            value: filtered_value,
            is_anomaly,
            confidence,
        };

        metrics.complete(true);
        (processed_data, metrics)
    }

    #[allow(dead_code)]
    pub fn adjust_threshold(&mut self, sensor_type: SensorType, new_threshold: f64) {
        self.anomaly_thresholds.insert(sensor_type, new_threshold);
    }
}

// Real-time processor runner
pub async fn run_processor(
    config: &crate::config::ProcessorConfig,
    rx: crossbeam_channel::Receiver<SensorData>,
    tx: crossbeam_channel::Sender<SensorData>,
    metrics_tx: crossbeam_channel::Sender<PerformanceMetrics>,
) {
    let mut processor = DataProcessor::new(config.window_size);

    loop {
        match rx.recv() {
            Ok(raw_data) => {
                let start = Instant::now();

                // Process the data
                let (processed_data, metrics) = processor.process(raw_data);

                // Send metrics
                let _ = metrics_tx.send(metrics);

                // Log if processing is slow
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
