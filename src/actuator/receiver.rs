use crossbeam_channel::Receiver;

use crate::common::{
    data_types::{PerformanceMetrics, SensorData},
    metrics::MetricsCollector,
};
use std::sync::{Arc, Mutex};

pub struct ReceiverTask {
    rx: Receiver<SensorData>,
    metrics_collector: Arc<MetricsCollector>, // Use Arc for shared ownership
    shared_sensor_data: Arc<Mutex<Option<SensorData>>>,
}

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

            // Update the shared sensor data
            {
                let mut data_lock = self.shared_sensor_data.lock().unwrap();
                *data_lock = Some(sensor_data.clone());
            }

            // Process sensor_data here
            println!("Received sensor data: {:?}", sensor_data);

            // Calculate end_time and duration
            let end_time = std::time::Instant::now();
            let duration = end_time.duration_since(start_time).as_secs_f64() * 1000.0; // in ms

            let perf_metrics = PerformanceMetrics {
                operation: "sensor_receive".to_string(),
                start_time,
                end_time: Some(end_time),
                duration_ms: Some(duration),
                success: true,
            };

            // Add metrics to collector
            self.metrics_collector.add_metrics(perf_metrics);
        }
        println!("Receiver channel closed, stopping receiver.");
    }
}
