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
use rand::Rng;
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

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

    let target_map: Arc<Mutex<HashMap<String, f64>>> = Arc::new(Mutex::new(HashMap::new()));

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
            // let target_value = rand::thread_rng().gen_range(30.0..70.0);
            let command_receive_time = std::time::Instant::now();
            let command: ActuatorCommand = serde_json::from_slice(&delivery.data)?;
            let mut target_map = target_map.lock().unwrap();
            let target_value = *target_map
                .entry(command.actuator_id.clone())
                .or_insert_with(|| rand::thread_rng().gen_range(30.0..70.0));

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
            executor.execute(control.clone());

            let elapsed = command_receive_time.elapsed().as_millis();
            println!(
                "⏱️ Execution time for actuator {}: {} ms",
                command.actuator_id, elapsed
            );

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis();

            let missed_deadline = now > command.deadline;
            // let is_within_tolerance = (sensor_data.value - *target).abs() <= 0.1;
            // let tolerance = (command.control_command.value.abs()) * 0.1;
            // let is_within_tolerance =
            //     (command.control_command.value - control.value).abs() <= tolerance;

            let mut _is_within_tolerance = false;
            let tolerance = 0.5;
            let sensor_value = command.control_command.value;

            //if (sensor_value >= target_value - tolerance) && (sensor_value <= target_value + tolerance) {
            //     is_within_tolerance = true;
            // } else {
            //     is_within_tolerance = false;
            // }

            _is_within_tolerance = (sensor_value - target_value).abs() <= tolerance;

            let feedback_msg = if (sensor_value - target_value).abs() <= tolerance {
                "=".to_string()
            } else if sensor_value < target_value {
                "increase".to_string()
            } else {
                "decrease".to_string()
            };
            println!(
                "📣 FEEDBACK message for [{}]: {}",
                command.actuator_id, feedback_msg
            );

            // Send feedback
            let feedback = ActuatorFeedback {
                timestamp: Utc::now().timestamp_millis() as u128,
                actuator_id: command.actuator_id.clone(),
                // status: ActuatorStatus::Success,
                status: if missed_deadline {
                    ActuatorStatus::Warning
                } else if _is_within_tolerance {
                    ActuatorStatus::Success
                } else {
                    ActuatorStatus::Adjusting
                },
                // message: Some(format!(
                //     "Executed command_type: {}",
                //     command.control_command.command_type
                // )),
                message: if missed_deadline {
                    Some(format!(
                        "❌ Deadline missed: now = {}, deadline = {}",
                        now, command.deadline
                    ))
                } else {
                    Some(feedback_msg)
                },
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
