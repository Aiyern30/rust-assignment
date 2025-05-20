use crate::actuator::controller::PIDController;
use crate::actuator::executor::Executor;
use crate::actuator::receiver::ReceiverTask;
use crate::actuator::scheduler::Scheduler;
use crate::common::data_types::{ActuatorFeedback, ActuatorStatus, SensorData};
use crate::common::metrics::MetricsCollector;
use crate::config::MetricsConfig;
use crossbeam_channel::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn run_actuator_system(rx: Receiver<SensorData>, feedback_tx: Sender<ActuatorFeedback>) {
    let metrics_config = MetricsConfig {
        report_interval_ms: 60_000,
        log_to_file: false,
        log_file: String::new(),
    };

    let metrics: Arc<MetricsCollector> = Arc::new(MetricsCollector::new(&metrics_config));

    let controller: Arc<Mutex<PIDController>> =
        Arc::new(Mutex::new(PIDController::new(1.0, 0.1, 0.05)));
    let executor: Arc<Executor> = Arc::new(Executor::new());

    // === NEW: Shared sensor data storage ===
    let latest_sensor_data: Arc<Mutex<Option<SensorData>>> = Arc::new(Mutex::new(None));

    // === Modified ReceiverTask to update shared data ===
    let sensor_data_clone = Arc::clone(&latest_sensor_data);
    let metrics_clone = Arc::clone(&metrics);

    std::thread::spawn(move || {
        loop {
            if let Ok(data) = rx.recv() {
                *sensor_data_clone.lock().unwrap() = Some(data);
                // metrics_clone.record_sensor_data(&data); // Optional metric collection
            }
        }
    });

    // === Scheduler to process control loop ===
    let scheduler = Scheduler::new(5);
    let controller_clone = Arc::clone(&controller);
    let executor_clone = Arc::clone(&executor);
    let feedback_tx_clone = feedback_tx.clone();
    let data_for_scheduler = Arc::clone(&latest_sensor_data);

    scheduler.start(move || {
        let maybe_data = data_for_scheduler.lock().unwrap().clone();

        if let Some(data) = maybe_data {
            let sensor_value = data.value;
            let setpoint = 50.0;
            let dt = 0.005;

            let mut ctrl = controller_clone.lock().unwrap();
            let command = ctrl.compute(setpoint, sensor_value, dt);

            let command_clone = command.clone();
            executor_clone.execute(command_clone);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis();

            let feedback = ActuatorFeedback {
                timestamp,
                actuator_id: "actuator_1".to_string(),
                status: ActuatorStatus::Normal,
                message: Some(format!(
                    "Executed command {:?} for sensor {:.2}",
                    command, sensor_value
                )),
            };
            let _ = feedback_tx_clone.send(feedback);
        }
    });
}
