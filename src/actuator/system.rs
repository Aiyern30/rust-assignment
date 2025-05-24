// use crate::actuator::controller::PIDController;
// use crate::actuator::executor::Executor;
// use crate::actuator::scheduler::Scheduler;
use crate::common::constants::*;
use crate::common::data_types::{ActuatorCommand, ActuatorFeedback, ActuatorStatus};
// use crate::common::metrics::MetricsCollector;
// use crate::config::MetricsConfig;
use crossbeam_channel::{Receiver, Sender};
use futures::StreamExt;
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use serde_json;
// use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// use super::receiver::ReceiverTask;

// pub async fn run_actuator_system(rx: Receiver<SensorData>, feedback_tx: Sender<ActuatorFeedback>) {
//     let metrics_config = MetricsConfig {
//         report_interval_ms: 60_000,
//         log_to_file: false,
//         log_file: String::new(),
//     };

//     let metrics: Arc<MetricsCollector> = Arc::new(MetricsCollector::new(&metrics_config));

//     let controller: Arc<Mutex<PIDController>> =
//         Arc::new(Mutex::new(PIDController::new(1.0, 0.1, 0.05)));
//     let executor: Arc<Executor> = Arc::new(Executor::new());

//     let latest_sensor_data: Arc<Mutex<Option<SensorData>>> = Arc::new(Mutex::new(None));

//     let sensor_data_clone = Arc::clone(&latest_sensor_data);
//     let metrics_clone = Arc::clone(&metrics);

//     let mut receiver_task = ReceiverTask::new(rx, metrics_clone, sensor_data_clone);

//     std::thread::spawn(move || {
//         receiver_task.run();
//     });

//     // === Scheduler to process control loop ===
//     let scheduler = Scheduler::new(5);
//     let controller_clone = Arc::clone(&controller);
//     let executor_clone = Arc::clone(&executor);
//     let feedback_tx_clone = feedback_tx.clone();
//     let data_for_scheduler = Arc::clone(&latest_sensor_data);

//     scheduler.start(move || {
//         let maybe_data = data_for_scheduler.lock().unwrap().clone();

//         if let Some(data) = maybe_data {
//             let sensor_value = data.value;
//             let setpoint = 50.0;
//             let dt = 0.005;

//             let mut ctrl = controller_clone.lock().unwrap();
//             let command = ctrl.compute(setpoint, sensor_value, dt);

//             let command_clone = command.clone();
//             executor_clone.execute(command_clone);
//             let timestamp = SystemTime::now()
//                 .duration_since(UNIX_EPOCH)
//                 .expect("Time went backwards")
//                 .as_millis();

//             let feedback = ActuatorFeedback {
//                 timestamp,
//                 actuator_id: "actuator_1".to_string(),
//                 status: ActuatorStatus::Normal,
//                 message: Some(format!(
//                     "Executed command {:?} for sensor {:.2}",
//                     command, sensor_value
//                 )),
//             };
//             let _ = feedback_tx_clone.send(feedback);
//         }
//     });
// }

pub async fn run_actuator_system(
    _sensor_data_rx: Receiver<crate::common::data_types::SensorData>,
    _feedback_tx: Sender<ActuatorFeedback>,
    command_tx: Sender<ActuatorCommand>,
) -> anyhow::Result<()> {
    let conn =
        Connection::connect("amqp://127.0.0.1:5672/%2f", ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;

    channel
        .queue_declare(
            ACTUATOR_COMMAND_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_declare(
            ACTUATOR_FEEDBACK_QUEUE,
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // Consume actuator commands
    let mut consumer = channel
        .basic_consume(
            ACTUATOR_COMMAND_QUEUE,
            "actuator_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    while let Some(delivery) = consumer.next().await {
        if let Ok(delivery) = delivery {
            let command: ActuatorCommand = serde_json::from_slice(&delivery.data)?;

            // Process command (e.g., run controller logic)
            command_tx.send(command.clone()).ok();

            // Simulate feedback response
            let feedback = ActuatorFeedback {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                actuator_id: command.actuator_id.clone(), // or derive this from somewhere relevant
                status: ActuatorStatus::Normal,           // or Failure / InProgress based on logic
                message: None,                            // or Some("reason for failure")
            };

            let fb_data = serde_json::to_vec(&feedback)?;
            channel
                .basic_publish(
                    "",
                    ACTUATOR_FEEDBACK_QUEUE,
                    BasicPublishOptions::default(),
                    &fb_data,
                    BasicProperties::default(),
                )
                .await?
                .await?;

            delivery.ack(BasicAckOptions::default()).await?;
        }
    }

    Ok(())
}
