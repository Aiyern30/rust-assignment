use crate::actuator::{controller::PIDController, executor::Executor, scheduler::Scheduler};
use crate::common::constants::*;
use crate::common::data_types::{ActuatorCommand, ActuatorFeedback, ActuatorStatus, SensorData};
use crossbeam_channel::{Receiver, Sender};
use futures::StreamExt;
use lapin::BasicProperties;
use lapin::{options::*, types::FieldTable, Connection, ConnectionProperties};
use rand::Rng;
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

pub async fn run_actuator_system(
    _sensor_data_rx: Receiver<SensorData>,
    feedback_tx: Sender<ActuatorFeedback>,
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

    let publish_channel = channel.clone();

    // ✅ Create async feedback channel using tokio
    let (publish_tx, mut publish_rx) = mpsc::unbounded_channel::<ActuatorFeedback>();

    tokio::spawn(async move {
        while let Some(feedback) = publish_rx.recv().await {
            if let Ok(data) = serde_json::to_vec(&feedback) {
                match publish_channel
                    .basic_publish(
                        "",
                        ACTUATOR_FEEDBACK_QUEUE,
                        BasicPublishOptions::default(),
                        &data,
                        BasicProperties::default(),
                    )
                    .await
                {
                    Ok(confirm) => match confirm.await {
                        Ok(_) => {
                            println!("✅ Published ActuatorFeedback to RabbitMQ: {:?}", feedback)
                        }
                        Err(e) => eprintln!("❌ Failed to confirm publication: {:?}", e),
                    },
                    Err(e) => eprintln!("❌ Failed to publish feedback to RabbitMQ: {:?}", e),
                }
            }
        }
    });

    let mut consumer = channel
        .basic_consume(
            ACTUATOR_COMMAND_QUEUE,
            "actuator_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    println!("📡 Waiting for ACTUATOR_COMMANDS...");

    let tx_for_feedback = feedback_tx.clone();
    let publish_tx_clone = publish_tx.clone();

    while let Some(delivery) = consumer.next().await {
        if let Ok(delivery) = delivery {
            let command: ActuatorCommand = match serde_json::from_slice(&delivery.data) {
                Ok(cmd) => cmd,
                Err(e) => {
                    eprintln!("⚠️ Failed to deserialize actuator command: {}", e);
                    delivery.nack(BasicNackOptions::default()).await?;
                    continue;
                }
            };

            println!("📥 Received ActuatorCommand: {:?}", command);
            let _ = command_tx.send(command.clone());

            let feedback = ActuatorFeedback {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                actuator_id: command.actuator_id.clone(),
                status: ActuatorStatus::InProgress,
                message: Some("Command received and forwarded.".to_string()),
            };

            let _ = tx_for_feedback.send(feedback.clone());
            let _ = publish_tx_clone.send(feedback);

            delivery.ack(BasicAckOptions::default()).await?;
        }
    }

    Ok(())
}

#[allow(dead_code)]
pub fn initialize_actuator_control_system(
    shared_sensor_data: Arc<Mutex<Option<SensorData>>>,
    feedback_tx: Sender<ActuatorFeedback>,
    command_rx: Receiver<ActuatorCommand>,
) {
    let targets: Arc<Mutex<HashMap<String, f64>>> = Arc::new(Mutex::new(HashMap::new()));
    let tgt = Arc::clone(&targets);

    let controller = Arc::new(Mutex::new(PIDController::new(1.0, 0.1, 0.05)));
    let executor = Arc::new(Executor::new());

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
    let scheduler = Scheduler::new(5);
    let ctrl = Arc::clone(&controller);
    let exec = Arc::clone(&executor);
    let shared_map = Arc::clone(&command_map);
    let shared_sensor = Arc::clone(&shared_sensor_data);
    let overheat_flag = Arc::new(Mutex::new(false)); // true = overheat active
    let overheat_flag_clone = Arc::clone(&overheat_flag);

    scheduler.start(shared_map, move |actuator_id, command| {
        if let Some(fwd_at) = command.forwarded_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();
            println!(
                "⏱️ Scheduler delay for [{}]: {} ms",
                actuator_id,
                now - fwd_at
            );
        }

        let target = {
            let mut map = tgt.lock().unwrap();
            map.entry(actuator_id.clone())
                .or_insert_with(|| {
                    let value = rand::thread_rng().gen_range(40.0..80.0);
                    println!("🎯 New target for [{}]: {:.2}", actuator_id, value);
                    value
                })
                .clone()
        };

        let maybe_data = shared_sensor.lock().unwrap().clone();
        if let Some(sensor_data) = maybe_data {
            let is_overheating = sensor_data.value > 90.0;
            {
                let mut overheat = overheat_flag_clone.lock().unwrap();
                if is_overheating && !*overheat {
                    println!(
                        "🔥 OVERHEAT WARNING for [{}]: temp = {:.2}°C",
                        actuator_id, sensor_data.value
                    );
                    *overheat = true;
                } else if !is_overheating && *overheat {
                    println!(
                        "❄️ Temperature normalized for [{}]: temp = {:.2}°C",
                        actuator_id, sensor_data.value
                    );
                    *overheat = false;
                }
            }

            let is_currently_overheating = *overheat_flag_clone.lock().unwrap();
            if is_currently_overheating {
                println!(
                    "⚠️ Skipping control for [{}] due to overheating",
                    actuator_id
                );
                let feedback = ActuatorFeedback {
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis(),
                    actuator_id: format!("actuator_for_{}", actuator_id.clone()),
                    status: ActuatorStatus::Warning,
                    message: Some(format!(
                        "OVERHEATING: temp = {:.2}°C - control disabled",
                        sensor_data.value
                    )),
                };
                let _ = tx.send(feedback);
                return;
            }
            let mut pid = ctrl.lock().unwrap();

            let control_start = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let command_out = pid.compute(target, sensor_data.value, 0.005);

            exec.execute(command_out.clone());

            let execution_end_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let control_duration = execution_end_time - control_start;
            println!(
                "⏱️ Control execution time for [{}]: {} ms",
                actuator_id, control_duration
            );

            let diff = target - sensor_data.value;
            let adjustment = diff.clamp(-10.0, 10.0);
            let new_value = sensor_data.value + adjustment;
            let status = if (new_value - target).abs() <= 0.5 {
                ActuatorStatus::Success
            } else {
                ActuatorStatus::Adjusting
            };

            let feedback = ActuatorFeedback {
                timestamp: execution_end_time,
                actuator_id: format!("actuator_for_{}", actuator_id.clone()),
                status,
                message: Some(format!(
                    "Target: {:.2}, New: {:.2}, Adjustment: {:.2}, Status: {:?}",
                    target, new_value, adjustment, status
                )),
            };

            let feedback_gen_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();

            println!(
                "⏱️ Time from ACTUATION to FEEDBACK generation [{}]: {} ms",
                actuator_id,
                feedback_gen_time - execution_end_time
            );

            let _ = tx.send(feedback);
        }
    });
}
