use crossbeam_channel::unbounded;
use rust_assignment::common::data_types::{
    ActuatorCommand, ActuatorFeedback, PerformanceMetrics, SensorData,
};
use rust_assignment::config::SensorConfig;
use rust_assignment::sensor::generator::run_sensor_array;
use rust_assignment::sensor::transmitter::run_transmitter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (command_tx, command_rx) = unbounded::<ActuatorCommand>();
    let (feedback_tx, feedback_rx) = unbounded::<ActuatorFeedback>();

    let (sensor_tx, sensor_rx) = unbounded::<SensorData>();
    let (metrics_tx, _metrics_rx) = unbounded::<PerformanceMetrics>();

    let config = SensorConfig {
        sample_rate_ms: 100,
        num_sensors: 3,
        enable_anomalies: true,
        anomaly_rate: 0.01,
    };

    // Start the sensor
    let config_clone = config.clone();
    tokio::spawn(async move {
        run_sensor_array(&config_clone, sensor_tx.clone(), metrics_tx.clone()).await;
    });

    // Convert SensorData into ActuatorCommand
    tokio::spawn({
        let command_tx = command_tx.clone();
        async move {
            while let Ok(data) = sensor_rx.recv() {
                let cmd = ActuatorCommand::from_sensor_data(&data);
                let _ = command_tx.send(cmd);
            }
        }
    });

    println!("SENSOR started");

    tokio::spawn(run_transmitter(command_rx, feedback_tx.clone()));

    // Listen for feedback
    while let Ok(feedback) = feedback_rx.recv() {
        println!(
            "[Feedback: {:<25}] | Status: {:<8} | Message: {}",
            feedback.actuator_id,
            format!("{:?}", feedback.status),
            feedback
                .message
                .clone()
                .unwrap_or_else(|| "None".to_string())
        );
    }

    Ok(())
}
