use crate::common::data_types::{
    ActuatorCommand, ActuatorFeedback, ActuatorStatus, ControlCommand, PerformanceMetrics,
    SensorData, SensorType,
};
use rolling_stats::Stats;
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub struct DataProcessor {
    moving_averages: HashMap<String, Stats<f64>>,
    _window_size: usize,
    anomaly_thresholds: HashMap<SensorType, f64>,
}
fn current_timestamp_ms() -> u64 {
    let now = SystemTime::now();
    now.duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u64
}

impl DataProcessor {
    pub fn handle_feedback(&mut self, feedback: &ActuatorFeedback) {
        match feedback.status {
            ActuatorStatus::Warning => {
                if let Some(msg) = &feedback.message {
                    if msg.contains("Deadline missed") {
                        // Example: increase thresholds for the related sensor type to reduce sensitivity
                        println!(
                        "Warning received: Deadline missed, increasing threshold for actuator {}",
                        feedback.actuator_id
                    );

                        // You need a way to map actuator_id back to SensorType, assuming a naming convention:
                        if let Some(sensor_type) =
                            self.actuator_id_to_sensor_type(&feedback.actuator_id)
                        {
                            let current_threshold = self
                                .anomaly_thresholds
                                .get(&sensor_type)
                                .cloned()
                                .unwrap_or(3.0);
                            let new_threshold = current_threshold + 0.5; // increase by 0.5 or a suitable value
                            self.adjust_threshold(sensor_type, new_threshold);
                            println!(
                                "Threshold for {:?} adjusted to {}",
                                sensor_type, new_threshold
                            );
                        }
                    }
                }
            }
            ActuatorStatus::Adjusting => {
                if let Some(msg) = &feedback.message {
                    if msg == "increase" {
                        if let Some(sensor_type) =
                            self.actuator_id_to_sensor_type(&feedback.actuator_id)
                        {
                            let current_threshold = self
                                .anomaly_thresholds
                                .get(&sensor_type)
                                .cloned()
                                .unwrap_or(3.0);
                            let new_threshold = (current_threshold + 0.2).min(10.0); // upper cap
                            self.adjust_threshold(sensor_type, new_threshold);
                            println!(
                                "Adjusting: Increasing threshold for {:?} to {}",
                                sensor_type, new_threshold
                            );
                        }
                    } else if msg == "decrease" {
                        if let Some(sensor_type) =
                            self.actuator_id_to_sensor_type(&feedback.actuator_id)
                        {
                            let current_threshold = self
                                .anomaly_thresholds
                                .get(&sensor_type)
                                .cloned()
                                .unwrap_or(3.0);
                            let new_threshold = (current_threshold - 0.2).max(0.1); // lower cap
                            self.adjust_threshold(sensor_type, new_threshold);
                            println!(
                                "Adjusting: Decreasing threshold for {:?} to {}",
                                sensor_type, new_threshold
                            );
                        }
                    }
                }
            }
            ActuatorStatus::Normal
            | ActuatorStatus::Success
            | ActuatorStatus::InProgress
            | ActuatorStatus::Failure
            | ActuatorStatus::Error => {
                // Optionally handle other statuses if needed
            }
        }
    }

    /// Helper method to map actuator_id to SensorType
    fn actuator_id_to_sensor_type(&self, actuator_id: &str) -> Option<SensorType> {
        if actuator_id.contains("force_sensor") {
            Some(SensorType::Force)
        } else if actuator_id.contains("temp_sensor") {
            Some(SensorType::Temperature)
        } else if actuator_id.contains("position_sensor") {
            Some(SensorType::Position)
        } else if actuator_id.contains("velocity_sensor") {
            Some(SensorType::Velocity)
        } else {
            None
        }
    }

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
    pub fn generate_actuator_command(&self, sensor_data: &SensorData) -> Option<ActuatorCommand> {
        if sensor_data.is_anomaly {
            Some(ActuatorCommand {
                command_id: format!("cmd_{}", sensor_data.sensor_id),
                actuator_id: sensor_data.sensor_id.clone(),
                control_command: ControlCommand {
                    command_type: "adjust_position".to_string(),
                    payload: Some("new_target_position".to_string()),
                    timestamp: current_timestamp_ms() as u128,
                    value: sensor_data.value,
                    deadline: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                        + 2,
                },
                priority: 1,
                // deadline: Instant::now() + Duration::from_millis(2),
                deadline: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    + 2000, // 2 seconds from now
                forwarded_at: Some(current_timestamp_ms().into()),
            })
        } else {
            None
        }
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
    actuator_tx: crossbeam_channel::Sender<ActuatorCommand>,
    feedback_rx: crossbeam_channel::Receiver<ActuatorFeedback>,
) {
    let mut processor = DataProcessor::new(config.window_size);

    let mut prev_duration = None;
    let mut durations = vec![];
    let max_samples = 1000;

    loop {
        crossbeam_channel::select! {
                    recv(rx) -> sensor_res => {
                        match sensor_res {
                            Ok(raw_data) => {
                                let start = Instant::now();

                                let (processed_data, metrics) = processor.process(raw_data);

                                if let Some(act_cmd) = processor.generate_actuator_command(&processed_data) {
                                    if actuator_tx.send(act_cmd).is_err() {
                                        println!("❌ Actuator command channel closed, stopping processor.");
                                        break;
                                    }
                                }

                                let elapsed = start.elapsed();
                                let elapsed_ns = elapsed.as_nanos();

                                if let Some(prev) = prev_duration {
                                    let jitter = if elapsed_ns > prev {
                                        elapsed_ns - prev
                                    } else {
                                        prev - elapsed_ns
                                    };
                                    println!(
                                        "[Processor Timing] Processing time: {} ns, Jitter: {} ns",
                                        elapsed_ns, jitter
                                    );
                                } else {
                                    println!("[Processor Timing] Processing time: {} ns", elapsed_ns);
                                }

                                prev_duration = Some(elapsed_ns);

                                durations.push(elapsed_ns);
                                if durations.len() > max_samples {
                                    durations.remove(0);
                                }

                                if durations.len() % 100 == 0 {
                                    let min = durations.iter().min().unwrap();
                                    let max = durations.iter().max().unwrap();
                                    let avg = durations.iter().sum::<u128>() / durations.len() as u128;
                                    println!(
                                        "[Processor Stats] Min: {} ns, Max: {} ns, Avg: {} ns, Samples: {}",
                                        min,
                                        max,
                                        avg,
                                        durations.len()
                                    );
                                }

                                let _ = metrics_tx.send(metrics);

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
                    },
        recv(feedback_rx) -> feedback_res => {
            match feedback_res {
                Ok(feedback) => {
                    println!("Processor received actuator feedback: {:?}", feedback);
                    processor.handle_feedback(&feedback);
                }
                Err(_) => {
                    println!("❌ Feedback channel closed, stopping processor.");
                    break;
                }
            }
        }

                }
    }
}
