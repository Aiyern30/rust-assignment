use crate::actuator::controller::PIDController;
use crate::actuator::executor::Executor;
use crate::actuator::receiver::ReceiverTask;
use crate::actuator::scheduler::Scheduler;
use crate::common::data_types::{ActuatorFeedback, ActuatorStatus, SensorData};
use crate::common::metrics::MetricsCollector;
use crate::config::MetricsConfig;
use crossbeam_channel::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub async fn run_actuator_system(rx: Receiver<SensorData>, feedback_tx: Sender<ActuatorFeedback>) {
    let metrics_config = MetricsConfig {
        report_interval_ms: 60_000,
        log_to_file: false,
        log_file: String::new(),
    };

    let metrics: Arc<MetricsCollector> = Arc::new(MetricsCollector::new(&metrics_config));
    let mut receiver_task = ReceiverTask::new(rx, Arc::clone(&metrics));

    let controller: Arc<Mutex<PIDController>> =
        Arc::new(Mutex::new(PIDController::new(1.0, 0.1, 0.05)));
    let executor: Arc<Executor> = Arc::new(Executor::new());

    // Spawn thread for receiving sensor data
    std::thread::spawn(move || {
        receiver_task.run();
    });

    let scheduler = Scheduler::new(5);

    let controller_clone = Arc::clone(&controller);
    let executor_clone = Arc::clone(&executor);
    let feedback_tx_clone = feedback_tx.clone();

    scheduler.start(move || {
        let sensor_value = 42.0; // Placeholder, replace with real data
        let setpoint = 50.0;
        let dt = 0.005;

        let mut ctrl = controller_clone.lock().unwrap();
        let command = ctrl.compute(setpoint, sensor_value, dt);

        executor_clone.execute(command);

        // Get current timestamp in milliseconds since epoch
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        let feedback = ActuatorFeedback {
            timestamp,
            actuator_id: "actuator_1".to_string(),
            status: ActuatorStatus::Normal,
            message: Some("Command executed successfully".to_string()),
        };

        // Send feedback, ignoring errors
        let _ = feedback_tx_clone.send(feedback);
    });
}
