use crate::common::data_types::{PerformanceMetrics, SensorData, SensorType};
use rolling_stats::Stats;
use std::collections::HashMap;
use std::time::Instant;

pub struct DataProcessor {
    moving_averages: HashMap<String, Stats<f64>>,
    _window_size: usize,
    anomaly_thresholds: HashMap<SensorType, f64>,
}

impl DataProcessor {
    pub fn new(_window_size: usize) -> Self {
        let mut anomaly_thresholds = HashMap::new();

        anomaly_thresholds.insert(SensorType::Force, 2.5);
        anomaly_thresholds.insert(SensorType::Position, 3.0);
        anomaly_thresholds.insert(SensorType::Velocity, 2.8);
        anomaly_thresholds.insert(SensorType::Temperature, 3.5);

        Self {
            moving_averages: HashMap::new(),
            _window_size,
            anomaly_thresholds,
        }
    }

    pub fn process(&mut self, mut raw_data: SensorData) -> (SensorData, PerformanceMetrics) {
        let mut metrics = PerformanceMetrics::new("data_processing");

        let moving_avg = self
            .moving_averages
            .entry(raw_data.sensor_id.clone())
            .or_default();

        moving_avg.update(raw_data.value);
        let filtered_value = moving_avg.mean;

        let threshold = self
            .anomaly_thresholds
            .get(&raw_data.reading_type)
            .cloned()
            .unwrap_or(3.0);

        // Update value with filtered (smoothed) value
        raw_data.value = filtered_value;

        // Call the unified anomaly detection method on SensorData
        raw_data.detect_anomaly(filtered_value, moving_avg.std_dev, threshold);

        metrics.complete(true);
        (raw_data, metrics)
    }

    #[allow(dead_code)]
    pub fn adjust_threshold(&mut self, sensor_type: SensorType, new_threshold: f64) {
        self.anomaly_thresholds.insert(sensor_type, new_threshold);
    }
}

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

                let (processed_data, metrics) = processor.process(raw_data);

                let _ = metrics_tx.send(metrics);

                let processing_time = start.elapsed();
                if processing_time.as_millis() > 2 {
                    println!(
                        "⚠️  Warning: Processing took too long: {:?}",
                        processing_time
                    );
                }

                if tx.send(processed_data).is_err() {
                    println!("❌ Transmitter has been dropped, stopping processor.");
                    break;
                }
            }
            Err(_) => {
                println!("❌ Sensor channel closed, stopping processor.");
                break;
            }
        }
    }
}
