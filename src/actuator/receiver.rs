use crate::common::data_types::SensorData;
use crate::common::metrics::MetricsTracker;
use std::sync::mpsc::Receiver;

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
            self.metrics.record_received();

            println!("Received sensor data: {:?}", sensor_data);

            // TODO: forward data to controller (not implemented here)
        }
        println!("Receiver channel closed, stopping receiver.");
    }
}
