use crossbeam_channel::unbounded;
use rand::Rng;
use rust_assignment::common::data_types::ActuatorStatus;
use rust_assignment::common::data_types::{
    ActuatorCommand, ActuatorFeedback, PerformanceMetrics, SensorData,
};
use rust_assignment::config::SensorConfig;
use rust_assignment::sensor::generator::run_sensor_array;
use rust_assignment::sensor::transmitter::run_transmitter;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (command_tx, command_rx) = unbounded::<ActuatorCommand>();
    let (feedback_tx, feedback_rx) = unbounded::<ActuatorFeedback>();

    let (sensor_tx, sensor_rx) = unbounded::<SensorData>();
    let (metrics_tx, _metrics_rx) = unbounded::<PerformanceMetrics>();
    // let mut current_value = rand::thread_rng().gen_range(10.0..90.0);

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
    let completed: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let mut value_map: HashMap<String, f64> = HashMap::new();

    // Listen for feedback
    while let Ok(feedback) = feedback_rx.recv() {
        if let Some(value) = value_map.get_mut(&feedback.actuator_id) {
            match feedback.message.as_deref() {
                Some("increase") => *value += 1.0,
                Some("decrease") => *value -= 1.0,
                _ => {}
            }
        }

        if feedback.status == ActuatorStatus::Success {
            completed
                .lock()
                .unwrap()
                .insert(feedback.actuator_id.clone());
        }

        println!("SENSOR received feedback: {:?}", feedback);
    }

    // while let Ok(data) = sensor_rx.recv() {
    //     let actuator_id = format!("actuator_for_{}", data.sensor_id);
    //     if !completed.lock().unwrap().contains(&actuator_id) {
    //         let cmd = ActuatorCommand::from_sensor_data(&data);
    //         let _ = command_tx.send(cmd);
    //     }
    // }

    Ok(())
}
