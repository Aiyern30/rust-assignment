mod actuator;
mod common;
mod config;
mod sensor;

use actuator::system::run_actuator_system;
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
    /// Run the sensor system
    Run {
        /// Path to configuration file
        #[arg(short, long, value_name = "FILE")]
        config: Option<PathBuf>,

        /// Connection mode (tcp, shared_memory, channel)
        #[arg(short, long, default_value = "channel")]
        mode: String,

        /// Endpoint for connection (IP:PORT for TCP)
        #[arg(short, long)]
        endpoint: Option<String>,

        /// Sample rate in milliseconds
        #[arg(short, long)]
        sample_rate: Option<u64>,
    },

    /// Generate default configuration file
    GenConfig {
        /// Path to output configuration file
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
    },

    /// Run benchmarks
    Benchmark {
        /// Number of iterations for benchmarking
        #[arg(short, long, default_value = "1000")]
        iterations: usize,

        /// Path to output benchmark results
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            config,
            mode,
            endpoint,
            sample_rate,
        } => {
            // Load configuration
            let mut config = match config {
                Some(path) => config::Config::from_file(path.to_str().unwrap())?,
                None => config::Config::default(),
            };

            // Override config with CLI args
            config.transmitter.connection_type = mode;
            if let Some(ep) = endpoint {
                config.transmitter.endpoint = ep;
            }
            if let Some(rate) = sample_rate {
                config.sensor.sample_rate_ms = rate;
            }

            // Display current config
            println!("Starting sensor system with configuration:");
            println!("  Sample rate: {}ms", config.sensor.sample_rate_ms);
            println!("  Connection type: {}", config.transmitter.connection_type);
            if config.transmitter.connection_type == "tcp" {
                println!("  Endpoint: {}", config.transmitter.endpoint);
            } else if config.transmitter.connection_type == "shared_memory" {
                println!(
                    "  Shared memory name: {}",
                    config.transmitter.shared_mem_name
                );
            }

            // Create main sensor channel
            let (sensor_tx, sensor_rx_main) = bounded::<common::data_types::SensorData>(100);

            // Create fan-out channels for actuator system and processor
            let (sensor_tx_actuator, sensor_rx_actuator) =
                bounded::<common::data_types::SensorData>(100);
            let (sensor_tx_processor, sensor_rx_processor) =
                bounded::<common::data_types::SensorData>(100);

            // Other channels
            let (processed_tx, processed_rx) = bounded::<common::data_types::SensorData>(100);
            let (metrics_tx, metrics_rx) = unbounded::<common::data_types::PerformanceMetrics>();
            let (actuator_tx, actuator_rx) = bounded::<common::data_types::ActuatorCommand>(100);
            let (feedback_tx, feedback_rx) = unbounded::<common::data_types::ActuatorFeedback>();
            let feedback_tx_clone = feedback_tx.clone();
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

            // Spawn a dispatcher task that reads from sensor_rx_main and forwards to actuator and processor channels
            tokio::spawn(async move {
                loop {
                    match sensor_rx_main.recv() {
                        Ok(data) => {
                            // Clone data to send to both consumers
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

            // Spawn feedback listener task
            tokio::spawn(async move {
                while let Ok(feedback) = feedback_rx.recv() {
                    println!("Received actuator feedback: {:?}", feedback);
                    // Handle the feedback (e.g., log it, update UI, etc.)
                }
            });

            // Spawn actuator system task with actuator's sensor receiver
            tokio::spawn(async move {
                run_actuator_system(sensor_rx_actuator, feedback_tx).await;
            });

            // Spawn metrics collector task
            let metrics_config = config.metrics.clone();
            tokio::spawn(async move {
                common::metrics::run_metrics_collector(&metrics_config, metrics_rx).await;
            });

            // Spawn sensor generator task
            let sensor_config = config.sensor.clone();
            let sensor_metrics_tx = metrics_tx.clone();
            tokio::spawn(async move {
                sensor::generator::run_sensor_array(&sensor_config, sensor_tx, sensor_metrics_tx)
                    .await;
            });

            // Clone actuator_tx for processor and transmitter
            let actuator_tx_for_processor = actuator_tx.clone();
            let actuator_tx_for_transmitter = actuator_tx.clone();

            // Spawn processor task with processor's sensor receiver
            let processor_config = config.processor.clone();
            let processor_metrics_tx = metrics_tx.clone();
            tokio::spawn(async move {
                sensor::processor::run_processor(
                    &processor_config,
                    sensor_rx_processor,
                    processed_tx,
                    processor_metrics_tx,
                    actuator_tx_for_processor,
                )
                .await;
            });

            // Spawn transmitter task
            let transmitter_config = config.transmitter.clone();
            let transmitter_metrics_tx = metrics_tx.clone();
            let feedback_tx_for_transmitter = feedback_tx_clone;
            tokio::spawn(async move {
                sensor::transmitter::run_transmitter(
                    &transmitter_config,
                    processed_rx,
                    Some(actuator_tx_for_transmitter),
                    transmitter_metrics_tx,
                    Some(feedback_tx_for_transmitter),
                )
                .await;
            });

            // Keep running
            println!("System running. Press Ctrl+C to stop.");
            tokio::signal::ctrl_c().await?;
            println!("Shutting down...");
        }

        Commands::GenConfig { output } => {
            let config = config::Config::default();
            config.save_to_file(output.to_str().unwrap())?;
            println!("Default configuration saved to {:?}", output);
        }

        Commands::Benchmark { iterations, output } => {
            println!("Running benchmarks with {} iterations", iterations);

            let _config = config::Config::default();
            let (_sensor_tx, _sensor_rx) = bounded::<common::data_types::SensorData>(100);
            let (_processed_tx, _processed_rx) = bounded::<common::data_types::SensorData>(100);
            let (_metrics_tx, _metrics_rx) = unbounded::<common::data_types::PerformanceMetrics>();

            // Setup benchmarking sensor generator
            let mut generator = sensor::generator::SensorGenerator::new(
                "bench_sensor",
                common::data_types::SensorType::Force,
                1,
                10.0,
                0.2,
                0.01,
            );

            // Setup benchmarking processor
            let mut processor = sensor::processor::DataProcessor::new(20);

            // Benchmark sensor data generation
            println!("Benchmarking sensor data generation...");
            let start = std::time::Instant::now();
            for _ in 0..iterations {
                let (_data, _timestamp) = generator.generate_reading();
            }
            let generation_time = start.elapsed();

            // Generate test data for processing
            let mut test_data = Vec::with_capacity(iterations as usize);
            for _ in 0..iterations {
                let (data, _timestamp) = generator.generate_reading();
                test_data.push(data);
            }

            // Benchmark data processing
            println!("Benchmarking data processing...");
            let start = std::time::Instant::now();
            for data in &test_data {
                let (_processed_data, _metrics) = processor.process(data.clone());
            }
            let processing_time = start.elapsed();

            // Show results
            println!("Benchmark Results:");
            println!(
                "  Sensor data generation: {:?} for {} iterations ({:?} per iteration)",
                generation_time,
                iterations,
                generation_time / iterations as u32
            );
            println!(
                "  Data processing: {:?} for {} iterations ({:?} per iteration)",
                processing_time,
                iterations,
                processing_time / iterations as u32
            );

            // Save results to file
            let results = format!(
                "Benchmark Results:\n\
         Iterations: {}\n\
         Sensor data generation: {:?} ({:?} per iteration)\n\
         Data processing: {:?} ({:?} per iteration)\n",
                iterations,
                generation_time,
                generation_time / iterations as u32,
                processing_time,
                processing_time / iterations as u32
            );

            std::fs::write(&output, results)?;
            println!("Benchmark results saved to {:?}", output);
        }
    }

    Ok(())
}
