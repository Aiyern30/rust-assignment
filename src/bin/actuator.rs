use rust_assignment::actuator::controller::PIDController;
use rust_assignment::actuator::executor::Executor;
use rust_assignment::actuator::receiver::ReceiverTask;
use rust_assignment::actuator::system::{initialize_actuator_control_system, run_actuator_system};
use rust_assignment::common::data_types::{
    ActuatorCommand, ActuatorFeedback, ActuatorStatus, SensorData,
};
use rust_assignment::common::metrics::MetricsCollector;
use rust_assignment::config::MetricsConfig;

use chrono::Utc;
use crossbeam_channel::{unbounded, Receiver, Sender};
use futures::StreamExt;
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use serde_json;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("ACTUATOR system started.");

    // Channels
    let (_sensor_data_tx, sensor_data_rx): (Sender<SensorData>, Receiver<SensorData>) = unbounded();
    let (command_tx, command_rx): (Sender<ActuatorCommand>, Receiver<ActuatorCommand>) =
        unbounded();
    let (feedback_tx, feedback_rx): (Sender<ActuatorFeedback>, Receiver<ActuatorFeedback>) =
        unbounded();

    // Shared state
    let metrics = Arc::new(MetricsCollector::new(&MetricsConfig {
        log_to_file: false,
        log_file: "".into(),
        report_interval_ms: 1000,
    }));
    // let metrics = Arc::new(MetricsCollector::new(&metrics_config));
    let shared_sensor_data = Arc::new(Mutex::new(None));
    // let last_data = Arc::new(Mutex::new(None));
    {
        let mut receiver = ReceiverTask::new(
            sensor_data_rx.clone(),
            Arc::clone(&metrics),
            Arc::clone(&shared_sensor_data),
        );
        std::thread::spawn(move || receiver.run());
    }

    initialize_actuator_control_system(shared_sensor_data, feedback_tx.clone(), command_rx);

    tokio::spawn(async move {
        if let Err(err) = run_actuator_system(sensor_data_rx, feedback_tx, command_tx).await {
            eprintln!("❌ Error in run_actuator_system: {}", err);
        }
    });

    // 🟢 Start RabbitMQ connection
    let conn =
        Connection::connect("amqp://127.0.0.1:5672/%2f", ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;

    std::thread::spawn(move || {
        while let Ok(feedback) = feedback_rx.recv() {
            println!("📨 FEEDBACK SENT TO SENSOR: {:?}", feedback);
        }
    });

    // Declare queues
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

    // Start command consumer
    let mut consumer = channel
        .basic_consume(
            "actuator_command_queue",
            "actuator_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // Executor and PID controller
    let executor = Executor::new();
    let mut pid = PIDController::new(1.0, 0.1, 0.05);

    // Listen for commands from RabbitMQ
    println!("Waiting for actuator commands...");
    while let Some(delivery) = consumer.next().await {
        if let Ok(delivery) = delivery {
            let command: ActuatorCommand = serde_json::from_slice(&delivery.data)?;

            println!("ACTUATOR received command for [{}]:", command.actuator_id);
            println!("  → type: {}", command.control_command.command_type);
            println!("  → value: {}", command.control_command.value);
            println!("  → priority: {}", command.priority);
            println!("  → deadline: {}", command.deadline);

            // Simulate sensor feedback (example)
            let measurement = command.control_command.value * 0.95; // pretend current state
            let dt = 0.01;
            let control = pid.compute(command.control_command.value, measurement, dt);

            // Execute the control
            executor.execute(control);

            // Send feedback
            let feedback = ActuatorFeedback {
                timestamp: Utc::now().timestamp_millis() as u128,
                actuator_id: command.actuator_id.clone(),
                status: ActuatorStatus::Success,
                message: Some(format!(
                    "Executed command_type: {}",
                    command.control_command.command_type
                )),
            };

            let feedback_bytes = serde_json::to_vec(&feedback)?;
            channel
                .basic_publish(
                    "",
                    "actuator_feedback_queue",
                    BasicPublishOptions::default(),
                    &feedback_bytes,
                    BasicProperties::default(),
                )
                .await?
                .await?;

            delivery.ack(BasicAckOptions::default()).await?;
        }
    }

    Ok(())
}
