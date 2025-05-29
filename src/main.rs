mod actuator;
mod common;
mod config;
mod sensor;

use crate::actuator::system::run_actuator_system;
use clap::{Parser, Subcommand};
use crossbeam_channel::{bounded, unbounded};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sensor_system")]
#[command(about = "Real-time sensor system for manufacturing automation")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { config, .. } => {
            let config = match config {
                Some(path) => config::Config::from_file(path.to_str().unwrap())?,
                None => config::Config::default(),
            };

            let (sensor_tx, sensor_rx_main) = bounded::<common::data_types::SensorData>(100);

            let (sensor_tx_actuator, sensor_rx_actuator) =
                bounded::<common::data_types::SensorData>(100);
            let (sensor_tx_processor, sensor_rx_processor) =
                bounded::<common::data_types::SensorData>(100);
            let (actuator_command_tx, actuator_command_rx) = crossbeam_channel::unbounded();

            let (processed_tx, _processed_rx) = bounded::<common::data_types::SensorData>(100);
            let (metrics_tx, metrics_rx) = unbounded::<common::data_types::PerformanceMetrics>();
            let (actuator_tx, actuator_rx) = bounded::<common::data_types::ActuatorCommand>(100);
            let (feedback_tx, feedback_rx) = unbounded::<common::data_types::ActuatorFeedback>();
            let (feedback_to_processor_tx, feedback_to_processor_rx) =
                unbounded::<common::data_types::ActuatorFeedback>();

            tokio::spawn(async move {
                while let Ok(cmd) = actuator_rx.recv() {
                    println!(
                        "Received actuator command for actuator id: {}",
                        cmd.actuator_id
                    );
                    println!("Command details: {:?}", cmd.control_command);
                    println!("Priority: {}", cmd.priority);
                    println!("Deadline: {:?}", cmd.deadline);
                }
            });

            tokio::spawn(async move {
                loop {
                    match sensor_rx_main.recv() {
                        Ok(data) => {
                            let _ = sensor_tx_actuator.send(data.clone());
                            let _ = sensor_tx_processor.send(data);
                        }
                        Err(err) => {
                            eprintln!("Sensor dispatcher channel closed: {:?}", err);
                            break;
                        }
                    }
                }
            });

            tokio::spawn(async move {
                while let Ok(feedback) = feedback_rx.recv() {
                    println!("Received actuator feedback: {:?}", feedback);
                    let _ = feedback_to_processor_tx.send(feedback.clone());
                }
            });

            tokio::spawn(async move {
                let _ =
                    run_actuator_system(sensor_rx_actuator, feedback_tx, actuator_command_tx).await;
            });

            let metrics_config = config.metrics.clone();
            tokio::spawn(async move {
                common::metrics::run_metrics_collector(&metrics_config, metrics_rx).await;
            });

            let sensor_config = config.sensor.clone();
            let sensor_metrics_tx = metrics_tx.clone();
            tokio::spawn(async move {
                sensor::generator::run_sensor_array(&sensor_config, sensor_tx, sensor_metrics_tx)
                    .await;
            });

            let actuator_tx_for_processor = actuator_tx.clone();
            let _actuator_tx_for_transmitter = actuator_tx.clone();

            let processor_config = config.processor.clone();
            let processor_metrics_tx = metrics_tx.clone();
            tokio::spawn(async move {
                sensor::processor::run_processor(
                    &processor_config,
                    sensor_rx_processor,
                    processed_tx,
                    processor_metrics_tx,
                    actuator_tx_for_processor,
                    feedback_to_processor_rx,
                )
                .await;
            });
            tokio::spawn(async move {
                while let Ok(command) = actuator_command_rx.recv() {
                    println!("Received external actuator command: {:?}", command);

                    if let Err(e) = actuator_tx.send(command) {
                        eprintln!("Failed to forward actuator command: {:?}", e);
                    }
                }
            });
            println!("System running. Press Ctrl+C to stop.");
            tokio::signal::ctrl_c().await?;
            println!("Shutting down...");
        }
    }

    Ok(())
}
