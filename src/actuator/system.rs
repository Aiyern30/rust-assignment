use crate::actuator::{controller::PIDController, executor::Executor, scheduler::Scheduler};
use crate::common::constants::*;
use crate::common::data_types::{ActuatorCommand, ActuatorFeedback, ActuatorStatus, SensorData};
use crossbeam_channel::{Receiver, Sender};
use futures::StreamExt;
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

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

    println!("📡 Waiting for ACTUATOR_COMMANDS...");

    while let Some(delivery) = consumer.next().await {
        if let Ok(delivery) = delivery {
            let command: ActuatorCommand = serde_json::from_slice(&delivery.data)?;

            // Process command (e.g., run controller logic)
            command_tx.send(command.clone()).ok();

            // let fb_data = serde_json::to_vec(&feedback)?;
            // channel
            //     .basic_publish(
            //         "",
            //         ACTUATOR_FEEDBACK_QUEUE,
            //         BasicPublishOptions::default(),
            //         &fb_data,
            //         BasicProperties::default(),
            //     )
            //     .await?
            //     .await?;

            delivery.ack(BasicAckOptions::default()).await?;
        }
    }

    Ok(())
}

pub fn initialize_actuator_control_system(
    shared_sensor_data: Arc<Mutex<Option<SensorData>>>,
    feedback_tx: Sender<ActuatorFeedback>,
    command_rx: Receiver<ActuatorCommand>,
) {
    let controller = Arc::new(Mutex::new(PIDController::new(1.0, 0.1, 0.05)));
    let executor = Arc::new(Executor::new());
    // let shared = Arc::clone(&shared_sensor_data);

    let tx = feedback_tx.clone();
    let command_map: Arc<Mutex<HashMap<String, Vec<ActuatorCommand>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    {
        let command_map = Arc::clone(&command_map);
        std::thread::spawn(move || {
            while let Ok(command) = command_rx.recv() {
                command_map
                    .lock()
                    .unwrap()
                    .entry(command.actuator_id.clone())
                    .or_default()
                    .push(command);
            }
        });
    }
    let scheduler = Scheduler::new(5); // 5ms
    let ctrl = Arc::clone(&controller);
    let exec = Arc::clone(&executor);
    let shared_map = Arc::clone(&command_map);
    let shared_sensor = Arc::clone(&shared_sensor_data);

    scheduler.start(shared_map, move |actuator_id, command| {
        let maybe_data = shared_sensor.lock().unwrap().clone();
        if let Some(sensor_data) = maybe_data {
            let mut pid = ctrl.lock().unwrap();
            let command = pid.compute(50.0, sensor_data.value, 0.005);

            exec.execute(command.clone());

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
