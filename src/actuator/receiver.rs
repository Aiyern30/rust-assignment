use crossbeam_channel::Receiver;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::common::{
    data_types::{PerformanceMetrics, SensorData},
    metrics::MetricsCollector,
};
use std::sync::{Arc, Mutex};

#[allow(dead_code)]
pub struct ReceiverTask {
    rx: Receiver<SensorData>,
    metrics_collector: Arc<MetricsCollector>,
    shared_sensor_data: Arc<Mutex<Option<SensorData>>>,
}
#[allow(dead_code)]
impl ReceiverTask {
    pub fn new(
        rx: Receiver<SensorData>,
        metrics_collector: Arc<MetricsCollector>,
        shared_sensor_data: Arc<Mutex<Option<SensorData>>>,
    ) -> Self {
        Self {
            rx,
            metrics_collector,
            shared_sensor_data,
        }
    }

    pub fn run(&mut self) {
        println!("Actuator receiver started.");
        while let Ok(sensor_data) = self.rx.recv() {
            let start_time = std::time::Instant::now();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let time_from_sensor = now - sensor_data.timestamp;
            println!("⏱️ Time from SENSOR to RECEIVER: {} ms", time_from_sensor);
            let forward_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let mut sensor_data = sensor_data.clone();
            sensor_data.forwarded_at = forward_time;

            self.metrics_collector._record_sensor_data(&sensor_data);

            {
                let mut data_lock = self.shared_sensor_data.lock().unwrap();
                *data_lock = Some(sensor_data.clone());
            }

            println!("Received sensor data: {:?}", sensor_data);

            let end_time = std::time::Instant::now();
            let duration = end_time.duration_since(start_time).as_secs_f64() * 1000.0;

            let perf_metrics = PerformanceMetrics {
                operation: "sensor_receive".to_string(),
                start_time,
                end_time: Some(end_time),
                duration_ms: Some(duration),
                success: true,
            };

            self.metrics_collector.add_metrics(perf_metrics);
        }
        println!("Receiver channel closed, stopping receiver.");
    }
}
