use crate::actuator::controller::PIDController;
use crate::actuator::executor::Executor;
use crate::actuator::receiver::ReceiverTask;
use crate::actuator::scheduler::Scheduler;
use crate::common::data_types::SensorData;
use crate::common::metrics::MetricsTracker;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

pub fn run_actuator_system(rx: Receiver<SensorData>) {
    let metrics = MetricsTracker::new();
    let mut receiver_task = ReceiverTask::new(rx, metrics.clone());

    // Shared controller and executor for the scheduled loop
    let controller = Arc::new(Mutex::new(PIDController::new(1.0, 0.1, 0.05)));
    let executor = Arc::new(Executor::new());

    // Start receiver thread to listen to sensor data
    std::thread::spawn(move || {
        receiver_task.run();
    });

    // Start control loop with scheduler
    let scheduler = Scheduler::new(5); // 5ms interval

    let controller_clone = Arc::clone(&controller);
    let executor_clone = Arc::clone(&executor);

    scheduler.start(move || {
        // Here you'd get latest sensor data from shared state or channel
        // For example, let's fake sensor data for demonstration:
        let sensor_value = 42.0; // You'd update this with real data

        let setpoint = 50.0; // Target value you want to maintain
        let dt = 0.005; // 5ms interval in seconds

        let mut ctrl = controller_clone.lock().unwrap();
        let command = ctrl.compute(setpoint, sensor_value, dt);

        executor_clone.execute(command);
    });
}
