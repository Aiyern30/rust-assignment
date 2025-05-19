use crate::common::data_types::SensorData; // Assuming your sensor data type
use crate::common::metrics::MetricsTracker;
use std::sync::mpsc::Receiver; // Assuming metrics tracking

pub struct ReceiverTask {
    rx: Receiver<SensorData>,
    metrics: MetricsTracker,
}

impl ReceiverTask {
    pub fn new(rx: Receiver<SensorData>, metrics: MetricsTracker) -> Self {
        Self { rx, metrics }
    }

    pub fn run(&mut self) {
        println!("Actuator receiver started.");
        while let Ok(sensor_data) = self.rx.recv() {
            // Update metrics
            self.metrics.record_received();

            // Forward sensor_data to controller or actuator logic
            // For now just print or log
            println!("Received sensor data: {:?}", sensor_data);

            // TODO: forward to actuator controller / processing pipeline
        }
        println!("Receiver channel closed, stopping receiver.");
    }
}
