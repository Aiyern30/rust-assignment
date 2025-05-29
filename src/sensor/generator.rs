use crate::common::data_types::{PerformanceMetrics, SensorData, SensorType};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use rand_distr::{Distribution, Normal};
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
            last_value: initial_value,
            last_emit_time: None,
        }
    }

    pub fn generate_reading(&mut self) -> (SensorData, PerformanceMetrics) {
        let mut metrics = PerformanceMetrics::new("sensor_reading_generation");

        let noise = self.normal_dist.sample(&mut self.rng);

        let drift = (self.rng.gen_range(0.0..1.0) - 0.5) * self.drift_factor;
        self.last_value += drift;

        let value = self.last_value + noise;

        let is_anomaly = self.rng.gen_range(0.0..1.0) < 0.01;
        let anomaly_factor = if is_anomaly {
            self.rng.gen_range(3.0..5.0)
        } else {
            1.0
        };

        let final_value = value * anomaly_factor;

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
            confidence: 1.0,
            forwarded_at: 0,
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

            let now = Instant::now();
            let duration_since_last = self.last_emit_time.map(|last| now.duration_since(last));
            self.last_emit_time = Some(now);

            let (data, mut metrics) = self.generate_reading();

            if let Some(dur) = duration_since_last {
                metrics.duration_ms = Some(dur.as_secs_f64() * 1000.0);
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

pub async fn run_sensor_array(
    config: &crate::config::SensorConfig,
    tx: crossbeam_channel::Sender<SensorData>,
    metrics_tx: crossbeam_channel::Sender<PerformanceMetrics>,
) {
    let mut handles = vec![];

    let mut force_sensor = SensorGenerator::new(
        "force_sensor_1",
        SensorType::Force,
        config.sample_rate_ms,
        10.0,
        0.2,
        0.01,
    );

    let mut position_sensor = SensorGenerator::new(
        "position_sensor_1",
        SensorType::Position,
        config.sample_rate_ms,
        100.0,
        0.5,
        0.005,
    );

    let mut temp_sensor = SensorGenerator::new(
        "temp_sensor_1",
        SensorType::Temperature,
        config.sample_rate_ms * 2,
        25.0,
        0.1,
        0.002,
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

    for handle in handles {
        let _ = handle.await;
    }
}
