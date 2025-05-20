use serde::{Deserialize, Serialize};
use std::time::Instant;

// Main data structure for sensor readings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorData {
    pub timestamp: u128,          // Timestamp in milliseconds
    pub sensor_id: String,        // Unique identifier for the sensor
    pub reading_type: SensorType, // Type of sensor
    pub value: f64,               // Actual sensor reading
    pub is_anomaly: bool,         // Flag for anomalies
    pub confidence: f64,          // Confidence level (0.0-1.0)
}
#[derive(Debug, Clone)]
pub struct ControlCommand {
    pub command_type: String,
    pub payload: Option<String>,
    pub timestamp: u128,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct ActuatorCommand {
    pub actuator_id: String,
    pub control_command: ControlCommand,
    pub priority: u8,
    pub deadline: Instant,
}

// Types of sensors we might simulate
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SensorType {
    Force,       // Force sensor (Newtons)
    Position,    // Position sensor (mm)
    Velocity,    // Velocity sensor (mm/s)
    Temperature, // Temperature sensor (Celsius)
}

// Feedback from the actuator system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActuatorFeedback {
    pub timestamp: u128,
    pub actuator_id: String,
    pub status: ActuatorStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ActuatorStatus {
    Normal,
    Adjusting,
    Warning,
    Error,
}

// Metrics for performance benchmarking
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub operation: String,
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub duration_ms: Option<f64>,
    pub success: bool,
}

impl PerformanceMetrics {
    pub fn new(operation: &str) -> Self {
        Self {
            operation: operation.to_string(),
            start_time: Instant::now(),
            end_time: None,
            duration_ms: None,
            success: false,
        }
    }

    pub fn complete(&mut self, success: bool) {
        let end = Instant::now();
        self.end_time = Some(end);
        self.duration_ms = Some((end - self.start_time).as_secs_f64() * 1000.0);
        self.success = success;
    }
}

impl SensorData {
    /// Detects if the value is anomalous based on z-score and thresholds.
    /// Requires mean and std_dev to calculate z-score.
    pub fn detect_anomaly(&mut self, mean: f64, std_dev: f64, threshold: f64) {
        if std_dev > 0.0 {
            let z_score = (self.value - mean).abs() / std_dev;
            self.is_anomaly = z_score > threshold;

            let mut confidence = 1.0 - (z_score / (threshold * 2.0)).min(0.9);
            confidence = confidence.max(0.1);

            if self.is_anomaly {
                println!(
                    "[ANOMALY] Sensor: {}, Value: {:.2}, Mean: {:.2}, StdDev: {:.2}, Z-score: {:.2}, Confidence: {:.2}",
                    self.sensor_id, self.value, mean, std_dev, z_score, confidence
                );
            }

            self.confidence = confidence;
        } else {
            self.is_anomaly = false;
            self.confidence = 0.0;
        }
    }
}

impl ActuatorCommand {
    pub fn from_sensor_data(data: &SensorData) -> Self {
        // Determine actuator_id from sensor_id (example logic)
        let actuator_id = format!("actuator_for_{}", data.sensor_id);

        // Example: command_type depends on sensor reading type
        let command_type = match data.reading_type {
            SensorType::Force => "AdjustForce",
            SensorType::Position => "MovePosition",
            SensorType::Velocity => "SetVelocity",
            SensorType::Temperature => "RegulateTemperature",
        }
        .to_string();

        // Payload could be some JSON or string representing the command parameters,
        // here we just serialize the value as string for simplicity
        let payload = Some(format!("{{\"value\": {:.2}}}", data.value));

        // Set priority higher if anomaly detected, else default 5
        let priority = if data.is_anomaly { 10 } else { 5 };

        // Deadline example: 1 second from now
        let deadline = Instant::now() + std::time::Duration::from_secs(1);

        let control_command = ControlCommand {
            command_type,
            payload,
            timestamp: data.timestamp,
            value: data.value,
        };

        ActuatorCommand {
            actuator_id,
            control_command,
            priority,
            deadline,
        }
    }
}
