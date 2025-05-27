use crate::actuator::{controller::PIDController, executor::Executor, scheduler::Scheduler};
use crate::common::constants::*;
use crate::common::data_types::{ActuatorCommand, ActuatorFeedback, ActuatorStatus, SensorData};
use crossbeam_channel::{Receiver, Sender};
use futures::StreamExt;
use lapin::{options::*, types::FieldTable, Connection, ConnectionProperties};
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use super::receiver::ReceiverTask;

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

    let latest_sensor_data: Arc<Mutex<Option<SensorData>>> = Arc::new(Mutex::new(None));

    let sensor_data_clone = Arc::clone(&latest_sensor_data);
    let metrics_clone = Arc::clone(&metrics);

    let mut receiver_task = ReceiverTask::new(rx, metrics_clone, sensor_data_clone);

    std::thread::spawn(move || {
        receiver_task.run();
    });

    // === Scheduler to process control loop ===
    let scheduler = Scheduler::new(5);
    let controller_clone = Arc::clone(&controller);
    let executor_clone = Arc::clone(&executor);
    let feedback_tx_clone = feedback_tx.clone();
    let data_for_scheduler = Arc::clone(&latest_sensor_data);

    scheduler.start(shared_map, move |actuator_id, command| {
        let maybe_data = shared_sensor.lock().unwrap().clone();
        if let Some(sensor_data) = maybe_data {
            let mut pid = ctrl.lock().unwrap();
            let command = pid.compute(50.0, sensor_data.value, 0.005);

            exec.execute(command.clone());

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let missed_deadline = now > command.deadline;

            let feedback = ActuatorFeedback {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                actuator_id: format!("actuator_for_{}", actuator_id.clone()),
                status: ActuatorStatus::Normal,
                message: Some(format!(
                    "Executed {} with {:.2}",
                    command.command_type, command.value
                )),
            };

            let _ = tx.send(feedback);
        }
    });
}
