use crate::actuator::controller::PIDController;
use crate::actuator::executor::Executor;
use crate::actuator::receiver::ReceiverTask;
use crate::actuator::scheduler::Scheduler;
use crate::common::data_types::SensorData;
use crate::common::metrics::MetricsCollector;
use crate::config::MetricsConfig;
use crossbeam_channel::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn run_actuator_system(rx: Receiver<SensorData>) {
    // Create a MetricsConfig instance — fill these fields according to your config struct definition
    let metrics_config = MetricsConfig {
        report_interval_ms: 60_000, // 60 seconds in milliseconds
        log_to_file: false,
        log_file: String::new(),
    };

    // Create MetricsCollector with config, wrapped in Arc for sharing
    let metrics: Arc<MetricsCollector> = Arc::new(MetricsCollector::new(&metrics_config));
    let mut receiver_task = ReceiverTask::new(rx, Arc::clone(&metrics));

    // Create shared controller and executor
    let controller: Arc<Mutex<PIDController>> =
        Arc::new(Mutex::new(PIDController::new(1.0, 0.1, 0.05))); // You must implement PIDController::new
    let executor: Arc<Executor> = Arc::new(Executor::new());

    // Start receiver thread
    std::thread::spawn(move || {
        receiver_task.run();
    });

    // Scheduler to run control loop every 5 ms
    let scheduler = Scheduler::new(5);

    // Clone Arcs to move into closure
    let controller_clone = Arc::clone(&controller);
    let executor_clone = Arc::clone(&executor);

    // Start scheduled control loop
    scheduler.start(move || {
        // TODO: Replace with real sensor data from shared state/channel
        let sensor_value = 42.0; // Placeholder
        let setpoint = 50.0;
        let dt = 0.005; // 5 ms in seconds

        let mut ctrl = controller_clone.lock().unwrap();
        let command = ctrl.compute(setpoint, sensor_value, dt);

        executor_clone.execute(command);
    });
}
