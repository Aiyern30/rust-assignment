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
