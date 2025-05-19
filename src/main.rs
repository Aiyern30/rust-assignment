use crossbeam::channel::{bounded, Receiver, RecvTimeoutError, Sender};
use rand::Rng;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

// Structure to represent sensor data
#[derive(Debug, Clone, Copy)]
struct SensorData {
    force: f64,
    position: [f64; 3],
    temperature: f64,
    timestamp: u128,
}

// Structure to represent actuator commands
#[derive(Debug, Clone, Copy)]
struct ActuatorCommand {
    target_position: [f64; 3],
    grip_force: f64,
    priority: u8,
    timestamp: u128,
}

// Structure for feedback data sent from actuator to sensor
#[derive(Debug, Clone, Copy)]
struct ActuatorFeedback {
    status: ActuatorStatus,
    current_position: [f64; 3],
    current_force: f64,
    timestamp: u128,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ActuatorStatus {
    Normal,
    Overloaded,
    Misaligned,
    CalibrationNeeded,
}

// Moving Average Filter for smoothing sensor data
struct MovingAverageFilter {
    window_size: usize,
    force_values: VecDeque<f64>,
    position_values: [VecDeque<f64>; 3],
    temperature_values: VecDeque<f64>,
}

impl MovingAverageFilter {
    fn new(window_size: usize) -> Self {
        Self {
            window_size,
            force_values: VecDeque::with_capacity(window_size),
            position_values: [
                VecDeque::with_capacity(window_size),
                VecDeque::with_capacity(window_size),
                VecDeque::with_capacity(window_size),
            ],
            temperature_values: VecDeque::with_capacity(window_size),
        }
    }

    fn add_sample(&mut self, data: &SensorData) -> SensorData {
        // Process force data
        if self.force_values.len() == self.window_size {
            self.force_values.pop_front();
        }
        self.force_values.push_back(data.force);
        let avg_force = self.force_values.iter().sum::<f64>() / self.force_values.len() as f64;

        // Process position data
        let mut avg_position = [0.0; 3];
        for i in 0..3 {
            if self.position_values[i].len() == self.window_size {
                self.position_values[i].pop_front();
            }
            self.position_values[i].push_back(data.position[i]);
            avg_position[i] =
                self.position_values[i].iter().sum::<f64>() / self.position_values[i].len() as f64;
        }

        // Process temperature data
        if self.temperature_values.len() == self.window_size {
            self.temperature_values.pop_front();
        }
        self.temperature_values.push_back(data.temperature);
        let avg_temp =
            self.temperature_values.iter().sum::<f64>() / self.temperature_values.len() as f64;

        SensorData {
            force: avg_force,
            position: avg_position,
            temperature: avg_temp,
            timestamp: data.timestamp,
        }
    }
}

// Anomaly detector for sensor data
struct AnomalyDetector {
    force_threshold: f64,
    position_threshold: f64,
    temperature_threshold: f64,
    force_baseline: f64,
    position_baseline: [f64; 3],
    temperature_baseline: f64,
}

impl AnomalyDetector {
    fn new(
        force_threshold: f64,
        position_threshold: f64,
        temperature_threshold: f64,
        force_baseline: f64,
        position_baseline: [f64; 3],
        temperature_baseline: f64,
    ) -> Self {
        Self {
            force_threshold,
            position_threshold,
            temperature_threshold,
            force_baseline,
            position_baseline,
            temperature_baseline,
        }
    }

    fn detect_anomalies(&self, data: &SensorData) -> Vec<String> {
        let mut anomalies = Vec::new();

        // Check force anomalies
        if (data.force - self.force_baseline).abs() > self.force_threshold {
            anomalies.push(format!("Force anomaly: {:.2}", data.force));
        }

        // Check position anomalies
        for i in 0..3 {
            if (data.position[i] - self.position_baseline[i]).abs() > self.position_threshold {
                anomalies.push(format!(
                    "Position anomaly at axis {}: {:.2}",
                    i, data.position[i]
                ));
            }
        }

        // Check temperature anomalies
        if (data.temperature - self.temperature_baseline).abs() > self.temperature_threshold {
            anomalies.push(format!("Temperature anomaly: {:.2}", data.temperature));
        }

        anomalies
    }

    // Update baseline values based on feedback
    fn update_baselines(&mut self, feedback: &ActuatorFeedback) {
        // Adjust force baseline slightly based on feedback
        self.force_baseline = self.force_baseline * 0.95 + feedback.current_force * 0.05;

        // Adjust position baseline
        for i in 0..3 {
            self.position_baseline[i] =
                self.position_baseline[i] * 0.95 + feedback.current_position[i] * 0.05;
        }

        // Additional calibration if needed
        if feedback.status == ActuatorStatus::CalibrationNeeded {
            self.force_threshold *= 1.1; // Increase threshold temporarily
        }
    }
}

// PID Controller for actuator control
struct PIDController {
    kp: f64, // Proportional gain
    ki: f64, // Integral gain
    kd: f64, // Derivative gain
    previous_error: f64,
    integral: f64,
    dt: f64, // Time step in seconds
}

impl PIDController {
    fn new(kp: f64, ki: f64, kd: f64, dt: f64) -> Self {
        Self {
            kp,
            ki,
            kd,
            previous_error: 0.0,
            integral: 0.0,
            dt,
        }
    }

    fn compute(&mut self, setpoint: f64, measurement: f64) -> f64 {
        let error = setpoint - measurement;

        // Proportional term
        let p_term = self.kp * error;

        // Integral term
        self.integral += error * self.dt;
        let i_term = self.ki * self.integral;

        // Derivative term
        let derivative = (error - self.previous_error) / self.dt;
        let d_term = self.kd * derivative;

        // Store error for next iteration
        self.previous_error = error;

        // Compute control output
        p_term + i_term + d_term
    }

    fn reset(&mut self) {
        self.previous_error = 0.0;
        self.integral = 0.0;
    }
}

// Virtual Robotic Arm that simulates actuation
struct RoboticArm {
    current_position: [f64; 3],
    target_position: [f64; 3],
    current_force: f64,
    pid_controllers: [PIDController; 3],
    max_speed: f64,
    max_force: f64,
}

impl RoboticArm {
    fn new(initial_position: [f64; 3], max_speed: f64, max_force: f64) -> Self {
        // Create PID controllers for each axis
        let pid_controllers = [
            PIDController::new(0.5, 0.1, 0.01, 0.005), // X-axis
            PIDController::new(0.5, 0.1, 0.01, 0.005), // Y-axis
            PIDController::new(0.5, 0.1, 0.01, 0.005), // Z-axis
        ];

        Self {
            current_position: initial_position,
            target_position: initial_position,
            current_force: 0.0,
            pid_controllers,
            max_speed,
            max_force,
        }
    }

    fn set_target(&mut self, target: [f64; 3]) {
        self.target_position = target;
    }

    fn apply_force(&mut self, force: f64) -> ActuatorStatus {
        if force > self.max_force {
            self.current_force = self.max_force;
            return ActuatorStatus::Overloaded;
        } else {
            self.current_force = force;
            return ActuatorStatus::Normal;
        }
    }

    fn update(&mut self) -> ActuatorStatus {
        let mut status = ActuatorStatus::Normal;
        let mut max_position_error: f64 = 0.0;

        // Update position using PID controllers
        for i in 0..3 {
            let control_output =
                self.pid_controllers[i].compute(self.target_position[i], self.current_position[i]);

            // Apply control output (limited by max speed)
            let movement = f64::min(control_output, self.max_speed);
            let movement = f64::max(movement, -self.max_speed);
            self.current_position[i] += movement;

            // Calculate position error
            let position_error = (self.target_position[i] - self.current_position[i]).abs();
            max_position_error = f64::max(max_position_error, position_error);
        }

        // Check if arm is misaligned
        if max_position_error > 5.0 {
            status = ActuatorStatus::Misaligned;
        }

        // Simulate random need for calibration (1% chance)
        if rand::thread_rng().gen_range(0..100) == 0 {
            status = ActuatorStatus::CalibrationNeeded;
        }

        status
    }

    fn get_status(&self) -> ActuatorFeedback {
        ActuatorFeedback {
            status: ActuatorStatus::Normal, // Will be updated by the update() method
            current_position: self.current_position,
            current_force: self.current_force,
            timestamp: Instant::now().elapsed().as_micros(),
        }
    }
}

// Performance metrics for benchmarking
#[derive(Debug, Default)]
struct PerformanceMetrics {
    processing_times: Vec<Duration>,
    transmission_times: Vec<Duration>,
    response_times: Vec<Duration>,
    missed_deadlines: usize,
    send_errors: usize,
    receive_errors: usize,
}

impl PerformanceMetrics {
    fn new() -> Self {
        Self {
            processing_times: Vec::new(),
            transmission_times: Vec::new(),
            response_times: Vec::new(),
            missed_deadlines: 0,
            send_errors: 0,
            receive_errors: 0,
        }
    }

    fn add_processing_time(&mut self, time: Duration) {
        self.processing_times.push(time);
        if time > Duration::from_micros(2000) {
            self.missed_deadlines += 1;
        }
    }

    fn add_transmission_time(&mut self, time: Duration) {
        self.transmission_times.push(time);
        if time > Duration::from_micros(1000) {
            self.missed_deadlines += 1;
        }
    }

    fn add_response_time(&mut self, time: Duration) {
        self.response_times.push(time);
        if time > Duration::from_micros(2000) {
            self.missed_deadlines += 1;
        }
    }

    fn increment_send_errors(&mut self) {
        self.send_errors += 1;
    }

    fn increment_receive_errors(&mut self) {
        self.receive_errors += 1;
    }

    fn report(&self) -> String {
        let avg_processing = if self.processing_times.is_empty() {
            Duration::from_secs(0)
        } else {
            let total: Duration = self.processing_times.iter().sum();
            total / self.processing_times.len() as u32
        };

        let avg_transmission = if self.transmission_times.is_empty() {
            Duration::from_secs(0)
        } else {
            let total: Duration = self.transmission_times.iter().sum();
            total / self.transmission_times.len() as u32
        };

        let avg_response = if self.response_times.is_empty() {
            Duration::from_secs(0)
        } else {
            let total: Duration = self.response_times.iter().sum();
            total / self.response_times.len() as u32
        };

        format!(
            "Performance Metrics:\n\
            - Average Processing Time: {:?}\n\
            - Average Transmission Time: {:?}\n\
            - Average Response Time: {:?}\n\
            - Missed Deadlines: {}\n\
            - Communication Errors - Send: {}, Receive: {}\n\
            - Total Samples: {}",
            avg_processing,
            avg_transmission,
            avg_response,
            self.missed_deadlines,
            self.send_errors,
            self.receive_errors,
            self.processing_times.len()
        )
    }
}

fn main() {
    println!("Starting Real-time Sensor-Actuator System");

    // Create communication channels with larger buffer
    let (sensor_tx, sensor_rx) = bounded::<SensorData>(100); // Increased buffer size from 10 to 100
    let (feedback_tx, feedback_rx) = bounded::<ActuatorFeedback>(100); // Increased buffer size

    // Shared performance metrics
    let sensor_metrics = Arc::new(Mutex::new(PerformanceMetrics::new()));
    let actuator_metrics = Arc::new(Mutex::new(PerformanceMetrics::new()));

    // Clone references for threads
    let sensor_metrics_clone = Arc::clone(&sensor_metrics);
    let actuator_metrics_clone = Arc::clone(&actuator_metrics);

    // Spawn Student A: Sensor Data Specialist thread
    let sensor_thread = thread::spawn(move || {
        student_a_sensor_module(sensor_tx, feedback_rx, sensor_metrics_clone);
    });

    // Small delay to ensure sensor starts first
    thread::sleep(Duration::from_millis(50));

    // Spawn Student B: Actuator Commander thread
    let actuator_thread = thread::spawn(move || {
        student_b_actuator_module(sensor_rx, feedback_tx, actuator_metrics_clone);
    });

    // Spawn a reporting thread to periodically print metrics
    let sensor_metrics_report = Arc::clone(&sensor_metrics);
    let actuator_metrics_report = Arc::clone(&actuator_metrics);
    let reporting_thread = thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(5));

        println!("\n--- SYSTEM PERFORMANCE REPORT ---");
        println!("SENSOR MODULE:");
        let metrics = sensor_metrics_report.lock().unwrap();
        println!("{}", metrics.report());

        println!("\nACTUATOR MODULE:");
        let metrics = actuator_metrics_report.lock().unwrap();
        println!("{}", metrics.report());
        println!("--------------------------------\n");
    });

    // Wait for threads to complete (they won't in this example, as they run indefinitely)
    let _ = sensor_thread.join();
    let _ = actuator_thread.join();
    let _ = reporting_thread.join();
}

// Student A: Sensor Data Specialist Implementation
fn student_a_sensor_module(
    sensor_tx: Sender<SensorData>,
    feedback_rx: Receiver<ActuatorFeedback>,
    metrics: Arc<Mutex<PerformanceMetrics>>,
) {
    println!("Student A: Starting Sensor Data Module");

    // Initialize sensor processing components
    let mut filter = MovingAverageFilter::new(5);
    let mut anomaly_detector = AnomalyDetector::new(
        10.0,            // force threshold
        2.0,             // position threshold
        5.0,             // temperature threshold
        50.0,            // force baseline
        [0.0, 0.0, 0.0], // position baseline
        25.0,            // temperature baseline
    );

    // Sensor sampling interval (5ms)
    let sampling_interval = Duration::from_millis(5);
    let mut last_sampling_time = Instant::now();

    // Track consecutive errors to detect disconnection
    let mut consecutive_errors = 0;
    let max_consecutive_errors = 10;

    loop {
        // Calculate time until next sample should be taken
        let elapsed = last_sampling_time.elapsed();
        if elapsed < sampling_interval {
            // Sleep for the remaining time until next sample
            thread::sleep(sampling_interval - elapsed);
        }
        last_sampling_time = Instant::now();

        // Check for feedback from actuator (non-blocking)
        match feedback_rx.try_recv() {
            Ok(feedback) => {
                // Update anomaly detector based on feedback
                anomaly_detector.update_baselines(&feedback);
                println!(
                    "Sensor received feedback - Status: {:?}, Position: {:?}, Force: {:.2}",
                    feedback.status, feedback.current_position, feedback.current_force
                );

                // Reset consecutive errors since we're receiving feedback
                consecutive_errors = 0;
            }
            Err(_) => {
                // Don't consider trying to receive an error when nothing is available
            }
        }

        // 1. Generate Sensor Data
        let raw_data = generate_sensor_data();

        // 2. Process Data (filtering and anomaly detection)
        let processing_start = Instant::now();

        // Apply moving average filter
        let filtered_data = filter.add_sample(&raw_data);

        // Detect anomalies
        let anomalies = anomaly_detector.detect_anomalies(&filtered_data);

        let processing_time = processing_start.elapsed();

        // Record processing metrics
        metrics.lock().unwrap().add_processing_time(processing_time);

        // Print anomaly information if any detected
        if !anomalies.is_empty() {
            println!("Sensor detected anomalies: {:?}", anomalies);
        }

        // 3. Transmit Data in Real Time (if no critical anomalies)
        if anomalies.len() <= 1 {
            // Allow minor anomalies, but skip if too many
            let transmission_start = Instant::now();

            // Send data to actuator
            match sensor_tx.send(filtered_data) {
                Ok(_) => {
                    let transmission_time = transmission_start.elapsed();
                    metrics
                        .lock()
                        .unwrap()
                        .add_transmission_time(transmission_time);

                    // Log successful transmission with timing
                    println!("Sensor transmitted data: Force={:.2}, Pos={:?}, Temp={:.2} (Process: {:?}, Transmit: {:?})",
                            filtered_data.force, filtered_data.position, filtered_data.temperature,
                            processing_time, transmission_time);

                    // Reset consecutive errors on successful send
                    consecutive_errors = 0;
                }
                Err(e) => {
                    println!("Error sending sensor data: {:?}", e);
                    metrics.lock().unwrap().increment_send_errors();

                    // Track consecutive errors
                    consecutive_errors += 1;

                    // If we've had too many consecutive errors, apply backoff
                    if consecutive_errors > max_consecutive_errors {
                        println!("Too many consecutive send errors, applying backoff...");
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        } else {
            println!("Skipping data transmission due to multiple anomalies");
        }
    }
}

// Function to generate simulated sensor data
fn generate_sensor_data() -> SensorData {
    let mut rng = rand::thread_rng();

    // Generate random force (40-60N with some noise)
    let force = 50.0 + rng.gen_range(-10.0..10.0);

    // Generate random position (around [0,0,0] with small variations)
    let position = [
        rng.gen_range(-1.0..1.0),
        rng.gen_range(-1.0..1.0),
        rng.gen_range(-1.0..1.0),
    ];

    // Generate random temperature (around 25°C with small variations)
    let temperature = 25.0 + rng.gen_range(-2.0..2.0);

    // Add timestamp (microseconds since program start)
    let timestamp = Instant::now().elapsed().as_micros();

    SensorData {
        force,
        position,
        temperature,
        timestamp,
    }
}

// Student B: Actuator Commander Implementation
fn student_b_actuator_module(
    sensor_rx: Receiver<SensorData>,
    feedback_tx: Sender<ActuatorFeedback>,
    metrics: Arc<Mutex<PerformanceMetrics>>,
) {
    println!("Student B: Starting Actuator Control Module");

    // Initialize robotic arm simulator
    let mut robotic_arm = RoboticArm::new(
        [0.0, 0.0, 0.0], // Initial position
        1.0,             // Max speed (units per update)
        100.0,           // Max force (N)
    );

    // Create a queue for actuator commands with different priorities
    let mut command_queue: Vec<ActuatorCommand> = Vec::new();

    // Track consecutive errors to detect disconnection
    let mut consecutive_errors = 0;
    let max_consecutive_errors = 10;

    loop {
        // 1. Receive Sensor Data
        let reception_start = Instant::now();
        let sensor_data = match sensor_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(data) => {
                let reception_time = reception_start.elapsed();

                // Record reception time
                if reception_time > Duration::from_micros(1000) {
                    println!(
                        "Warning: Sensor data reception exceeded 1ms deadline: {:?}",
                        reception_time
                    );
                }

                // Reset consecutive errors on successful receive
                consecutive_errors = 0;

                Some(data)
            }
            Err(RecvTimeoutError::Timeout) => {
                // This is normal, just no data within timeout period
                None
            }
            Err(RecvTimeoutError::Disconnected) => {
                println!("Error: Sensor channel disconnected");
                metrics.lock().unwrap().increment_receive_errors();

                // Track consecutive errors
                consecutive_errors += 1;

                // If we've had too many consecutive errors, apply backoff
                if consecutive_errors > max_consecutive_errors {
                    println!("Too many consecutive receive errors, applying backoff...");
                    thread::sleep(Duration::from_millis(100));
                }

                None
            }
        };

        // Process sensor data if available
        if let Some(data) = sensor_data {
            // Calculate time since data was generated (end-to-end latency)
            let data_age = Instant::now().elapsed().as_micros() - data.timestamp;
            println!(
                "Actuator received sensor data - Age: {}µs, Force: {:.2}, Position: {:?}",
                data_age, data.force, data.position
            );

            // 2. Control the Robotic Arm with Predictive Algorithms
            let control_start = Instant::now();

            // Calculate target position based on sensor data
            let target_position = calculate_target_position(&data);

            // Create actuator command with appropriate priority
            let command = ActuatorCommand {
                target_position,
                grip_force: data.force,
                priority: calculate_priority(&data),
                timestamp: Instant::now().elapsed().as_micros(),
            };

            // Add command to queue
            command_queue.push(command);

            // Sort command queue by priority (highest first)
            command_queue.sort_by(|a, b| b.priority.cmp(&a.priority));

            let control_time = control_start.elapsed();
            metrics.lock().unwrap().add_response_time(control_time);

            if control_time > Duration::from_micros(2000) {
                println!(
                    "Warning: Control algorithm exceeded 2ms deadline: {:?}",
                    control_time
                );
            }
        }

        // 3. Execute commands from queue (manage multiple actuators)
        if !command_queue.is_empty() {
            // Take highest priority command
            let command = command_queue.remove(0);

            // Apply command to robotic arm
            robotic_arm.set_target(command.target_position);
            let force_status = robotic_arm.apply_force(command.grip_force);

            // Update arm state
            let arm_status = robotic_arm.update();

            // Determine overall status (worst of the two)
            let overall_status = if force_status == ActuatorStatus::Overloaded {
                force_status
            } else {
                arm_status
            };

            // Get current arm state
            let mut feedback = robotic_arm.get_status();
            feedback.status = overall_status;
            feedback.timestamp = Instant::now().elapsed().as_micros();

            // 4. Close the Feedback Loop - Send feedback to sensor
            match feedback_tx.send(feedback) {
                Ok(_) => {
                    println!(
                        "Actuator executed command - Target: {:?}, Force: {:.2}, Priority: {}",
                        command.target_position, command.grip_force, command.priority
                    );
                    println!(
                        "Actuator sent feedback - Status: {:?}, Position: {:?}, Force: {:.2}",
                        feedback.status, feedback.current_position, feedback.current_force
                    );

                    // Reset consecutive errors on successful send
                    consecutive_errors = 0;
                }
                Err(e) => {
                    println!("Error sending feedback: {:?}", e);
                    metrics.lock().unwrap().increment_send_errors();

                    // Track consecutive errors
                    consecutive_errors += 1;

                    // If we've had too many consecutive errors, apply backoff
                    if consecutive_errors > max_consecutive_errors {
                        println!("Too many consecutive feedback send errors, applying backoff...");
                        thread::sleep(Duration::from_millis(100));
                    }
                }
            }

            // Clear old commands (older than 50ms)
            let current_time = Instant::now().elapsed().as_micros();
            command_queue.retain(|cmd| current_time - cmd.timestamp < 50000);
        }

        // Small sleep to prevent CPU hogging, adjusted to be shorter
        thread::sleep(Duration::from_micros(100));
    }
}

// Helper function to calculate target position based on sensor data
fn calculate_target_position(data: &SensorData) -> [f64; 3] {
    // Simple example: Move toward origin with some offset based on force
    let force_factor = data.force / 100.0;
    [
        data.position[0] * 0.9 + force_factor,
        data.position[1] * 0.9,
        data.position[2] * 0.9,
    ]
}

// Helper function to calculate command priority based on sensor data
fn calculate_priority(data: &SensorData) -> u8 {
    // Higher priority for extreme values (force or position)
    let force_priority = if data.force > 70.0 || data.force < 30.0 {
        3
    } else {
        1
    };

    let position_offset = data.position.iter().map(|p| p.abs()).sum::<f64>();
    let position_priority = if position_offset > 5.0 { 3 } else { 1 };

    let temp_priority = if data.temperature > 30.0 || data.temperature < 20.0 {
        2
    } else {
        1
    };

    // Return the highest priority
    force_priority.max(position_priority).max(temp_priority)
}
