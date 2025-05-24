// #[tokio::main]
// async fn main() -> anyhow::Result<()> {
//     use crossbeam_channel::{unbounded, Receiver, Sender};
//     use rust_assignment::actuator::system::run_actuator_system;
//     use rust_assignment::common::data_types::{ActuatorCommand, ActuatorFeedback, SensorData};

//     let (sensor_data_tx, sensor_data_rx): (Sender<SensorData>, Receiver<SensorData>) = unbounded();
//     let (feedback_tx, _feedback_rx): (Sender<ActuatorFeedback>, Receiver<ActuatorFeedback>) =
//         unbounded();
//     let (command_tx, _command_rx): (Sender<ActuatorCommand>, Receiver<ActuatorCommand>) =
//         unbounded();

//     println!("Starting ACTUATOR system with RabbitMQ...");
//     run_actuator_system(sensor_data_rx, feedback_tx, command_tx).await
// }

use futures::StreamExt;
use lapin::{
    options::*, types::FieldTable, BasicProperties, Channel, Connection, ConnectionProperties,
};
use rust_assignment::common::data_types::{ActuatorCommand, ActuatorFeedback, ActuatorStatus};
use serde_json;
use tokio;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ACTUATOR started. Connecting to RabbitMQ...");

    // 1. Connect to RabbitMQ
    let conn =
        Connection::connect("amqp://127.0.0.1:5672/%2f", ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;

    // 2. Declare queues (ensure they exist)
    channel
        .queue_declare(
            "actuator_command_queue",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_declare(
            "actuator_feedback_queue",
            QueueDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // 3. Start consuming command messages
    println!("Waiting for actuator commands...");
    let mut consumer = channel
        .basic_consume(
            "actuator_command_queue",
            "actuator_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // 4. Process each command
    while let Some(delivery) = consumer.next().await {
        if let Ok(delivery) = delivery {
            let data = &delivery.data;

            // Parse the command
            let command: ActuatorCommand = match serde_json::from_slice(data) {
                Ok(cmd) => cmd,
                Err(err) => {
                    eprintln!("Failed to parse ActuatorCommand: {}", err);
                    continue;
                }
            };

            println!("ACTUATOR received command:");
            println!("  actuator_id: {}", command.actuator_id);
            println!("  value: {}", command.control_command.value);
            println!("  priority: {}", command.priority);
            println!("  deadline: {}", command.deadline);

            // Construct feedback
            let feedback = ActuatorFeedback {
                timestamp: chrono::Utc::now().timestamp_millis() as u128,
                actuator_id: command.actuator_id.clone(),
                status: ActuatorStatus::Normal,
                message: None,
            };

            let feedback_bytes = serde_json::to_vec(&feedback)?;

            // Send feedback
            channel
                .basic_publish(
                    "",
                    "actuator_feedback_queue",
                    BasicPublishOptions::default(),
                    &feedback_bytes,
                    BasicProperties::default(),
                )
                .await?
                .await?; // Wait for confirmation

            delivery.ack(BasicAckOptions::default()).await?;
        }
    }

    Ok(())
}
