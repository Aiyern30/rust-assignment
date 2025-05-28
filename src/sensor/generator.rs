use crate::common::data_types::{PerformanceMetrics, SensorData, SensorType};
use rand::rngs::SmallRng; // This now works with the `small_rng` feature
use rand::{Rng, SeedableRng}; // Rng is needed for gen_range, SeedableRng for from_entropy
use rand_distr::{Distribution, Normal}; // For noise generation
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::{self, Instant};

pub struct SensorGenerator {
    sensor_id: String,
    sensor_type: SensorType,
    sample_rate_ms: u64,
    drift_factor: f64,
    rng: SmallRng,
    normal_dist: Normal<f64>,
    last_value: f64,
    last_emit_time: Option<Instant>,
}

impl SensorGenerator {
    pub fn new(
        sensor_id: &str,
        sensor_type: SensorType,
        sample_rate_ms: u64,
        _base_value: f64,
        noise_level: f64,
        drift_factor: f64,
    ) -> Self {
        let normal_dist = Normal::new(0.0, noise_level).unwrap();
        let mut rng = SmallRng::from_entropy();
        let initial_value = rng.gen_range(0.0..100.0);

        Self {
            sensor_id: sensor_id.to_string(),
            sensor_type,
            sample_rate_ms,

            drift_factor,
            rng: SmallRng::from_entropy(),

            normal_dist,
            last_value: initial_value, // Start with a random base value
            last_emit_time: None,
        }
    }

    // Generate a single sensor reading
    pub fn generate_reading(&mut self) -> (SensorData, PerformanceMetrics) {
        let mut metrics = PerformanceMetrics::new("sensor_reading_generation");

        // Add some random noise
        let noise = self.normal_dist.sample(&mut self.rng);

        // Add some drift to simulate real sensor behavior
        let drift = (self.rng.gen_range(0.0..1.0) - 0.5) * self.drift_factor;
        self.last_value += drift;

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
            forwarded_at: 0, // Will be set when sent
        };

        metrics.complete(true);
        (sensor_data, metrics)
    }

    pub async fn run(
        &mut self,
        tx: crossbeam_channel::Sender<SensorData>,
        metrics_tx: crossbeam_channel::Sender<PerformanceMetrics>,
    ) {
        let mut interval = time::interval(Duration::from_millis(self.sample_rate_ms));

        loop {
            interval.tick().await;

            // Measure time since last emit
            let now = Instant::now();
            let duration_since_last = self.last_emit_time.map(|last| now.duration_since(last));
            self.last_emit_time = Some(now);

            let (data, mut metrics) = self.generate_reading();

            // If we have a previous timestamp, add jitter duration metric
            if let Some(dur) = duration_since_last {
                metrics.duration_ms = Some(dur.as_secs_f64() * 1000.0); // in milliseconds
            }

            println!(
    "[Sensor: {:<17}] | Type: {:<11} | Value: {:>7.3} | Anomaly: {:<5} | Interval: {:>4} ms | Perf: {:<25} | Duration: {:>7.2} ms | Success: {}",
    data.sensor_id,
    format!("{:?}", data.reading_type),
    data.value,
    data.is_anomaly,
    duration_since_last.map(|d| d.as_millis()).unwrap_or_default(),
    metrics.operation,
    metrics.duration_ms.unwrap_or_default(),
    metrics.success
);

            let _ = metrics_tx.send(metrics);

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
        0.002,
        // Drift factor
    );

    handles.push(tokio::spawn({
        let tx = tx.clone();
        let metrics_tx = metrics_tx.clone();
        async move {
            force_sensor.run(tx, metrics_tx).await;
        }
    }));

    handles.push(tokio::spawn({
        let tx = tx.clone();
        let metrics_tx = metrics_tx.clone();
        async move {
            position_sensor.run(tx, metrics_tx).await;
        }
    }));

    handles.push(tokio::spawn({
        let tx = tx.clone();
        let metrics_tx = metrics_tx.clone();
        async move {
            temp_sensor.run(tx, metrics_tx).await;
        }
    }));

    // Wait for all sensors to complete (they run indefinitely in this case)
    for handle in handles {
        let _ = handle.await;
    }
}
