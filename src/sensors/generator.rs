use crate::common::data_types::{PerformanceMetrics, SensorData, SensorType};
use rand::distributions::{Distribution, Normal};
use rand::Rng;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time;

pub struct SensorGenerator {
    sensor_id: String,
    sensor_type: SensorType,
    sample_rate_ms: u64, // Time between samples in milliseconds
    base_value: f64,     // Base value for the sensor
    noise_level: f64,    // Standard deviation of noise
    drift_factor: f64,   // How quickly the base value drifts
    rng: rand::rngs::ThreadRng,
    normal_dist: Normal,
    last_value: f64,
}

impl SensorGenerator {
    pub fn new(
        sensor_id: &str,
        sensor_type: SensorType,
        sample_rate_ms: u64,
        base_value: f64,
        noise_level: f64,
        drift_factor: f64,
    ) -> Self {
        let normal_dist = Normal::new(0.0, noise_level).unwrap();

        Self {
            sensor_id: sensor_id.to_string(),
            sensor_type,
            sample_rate_ms,
            base_value,
            noise_level,
            drift_factor,
            rng: rand::thread_rng(),
            normal_dist,
            last_value: base_value,
        }
    }

    // Generate a single sensor reading
    pub fn generate_reading(&mut self) -> (SensorData, PerformanceMetrics) {
        let mut metrics = PerformanceMetrics::new("sensor_reading_generation");

        // Add some random noise
        let noise = self.normal_dist.sample(&mut self.rng);

        // Add some drift to simulate real sensor behavior
        let drift = (self.rng.gen_range(0.0..1.0) - 0.5) * self.drift_factor;
        self.last_value = self.last_value + drift;

        // Calculate the final value
        let value = self.last_value + noise;

        // Occasionally generate anomaly (1% chance)
        let is_anomaly = self.rng.gen_range(0.0..1.0) < 0.01;
        let anomaly_factor = if is_anomaly {
            self.rng.gen_range(3.0..5.0) // Significant spike
        } else {
            1.0
        };

        let final_value = value * anomaly_factor;

        // Get current timestamp in milliseconds
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let sensor_data = SensorData {
            timestamp,
            sensor_id: self.sensor_id.clone(),
            reading_type: self.sensor_type,
            value: final_value,
            is_anomaly,
            confidence: 1.0, // Will be adjusted by processor
        };

        metrics.complete(true);
        (sensor_data, metrics)
    }

    // Run the sensor in real-time
    pub async fn run(
        &mut self,
        tx: crossbeam_channel::Sender<SensorData>,
        metrics_tx: crossbeam_channel::Sender<PerformanceMetrics>,
    ) {
        let mut interval = time::interval(Duration::from_millis(self.sample_rate_ms));

        loop {
            // Wait until the next tick
            interval.tick().await;

            // Generate reading and send it
            let (data, metrics) = self.generate_reading();

            // Send the metrics
            let _ = metrics_tx.send(metrics);

            // Send the sensor data
            if tx.send(data).is_err() {
                println!("Receiver has been dropped, stopping sensor generation.");
                break;
            }
        }
    }
}

// Create multiple sensors and run them concurrently
pub async fn run_sensor_array(
    config: &crate::config::SensorConfig,
    tx: crossbeam_channel::Sender<SensorData>,
    metrics_tx: crossbeam_channel::Sender<PerformanceMetrics>,
) {
    let mut handles = vec![];

    // Create a force sensor
    let mut force_sensor = SensorGenerator::new(
        "force_sensor_1",
        SensorType::Force,
        config.sample_rate_ms,
        10.0, // Base value (10 Newtons)
        0.2,  // Noise level
        0.01, // Drift factor
    );

    // Create a position sensor
    let mut position_sensor = SensorGenerator::new(
        "position_sensor_1",
        SensorType::Position,
        config.sample_rate_ms,
        100.0, // Base value (100 mm)
        0.5,   // Noise level
        0.005, // Drift factor
    );

    // Create a temperature sensor (slower sample rate)
    let mut temp_sensor = SensorGenerator::new(
        "temp_sensor_1",
        SensorType::Temperature,
        config.sample_rate_ms * 2, // Slower sampling for temperature
        25.0,                      // Base value (25 degrees C)
        0.1,                       // Noise level
        0.002,                     // Drift factor
    );

    // Spawn tasks for each sensor
    handles.push(tokio::spawn(
        force_sensor.run(tx.clone(), metrics_tx.clone()),
    ));
    handles.push(tokio::spawn(
        position_sensor.run(tx.clone(), metrics_tx.clone()),
    ));
    handles.push(tokio::spawn(
        temp_sensor.run(tx.clone(), metrics_tx.clone()),
    ));

    // Wait for all sensors to complete (they run indefinitely in this case)
    for handle in handles {
        let _ = handle.await;
    }
}
