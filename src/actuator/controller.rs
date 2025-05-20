use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use crate::common::data_types::ControlCommand;

pub struct PIDController {
    kp: f64,
    ki: f64,
    kd: f64,
    prev_error: f64,
    integral: f64,
}

impl PIDController {
    /// Constructor to create a new PIDController with given gains
    pub fn new(kp: f64, ki: f64, kd: f64) -> Self {
        Self {
            kp,
            ki,
            kd,
            prev_error: 0.0,
            integral: 0.0,
        }
    }

    /// Compute the PID control command based on setpoint, current measurement, and elapsed time dt
    pub fn compute(&mut self, setpoint: f64, measurement: f64, dt: f64) -> ControlCommand {
        let error = setpoint - measurement;
        self.integral += error * dt;
        let derivative = (error - self.prev_error) / dt;
        self.prev_error = error;

        let output = self.kp * error + self.ki * self.integral + self.kd * derivative;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        ControlCommand {
            command_type: "PID_OUTPUT".to_string(),
            payload: None, // Optional additional info, can be Some(String)
            timestamp,
            value: output,
        }
    }
}
