use crate::common::constants::{ACTUATOR_COMMAND_QUEUE, ACTUATOR_FEEDBACK_QUEUE};
use crate::common::data_types::{
    ActuatorCommand, ActuatorFeedback, PerformanceMetrics, SensorData,
};
use crossbeam_channel::{Receiver, Sender};
use lapin::{options::*, types::FieldTable, BasicProperties, Connection, ConnectionProperties};
use serde_json;
use std::error::Error;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

// Transmitter for sending data to the actuator system
pub struct DataTransmitter {
    // Connection options
    connection_type: ConnectionType,
    // Connection details (for TCP/UDP)
    endpoint: Option<String>,
    // Shared memory name (for shared memory)
    shared_mem_name: Option<String>,
    // Connected status
    connected: bool,
    // TCP connection if using TCP
    tcp_connection: Option<Arc<Mutex<TcpStream>>>,
}

// Communication methods supported
pub enum ConnectionType {
    SharedMemory,
    TcpSocket,
    CrossbeamChannel,
}

impl DataTransmitter {
    pub fn new(connection_type: ConnectionType) -> Self {
        Self {
            connection_type,
            endpoint: None,
            shared_mem_name: None,
            connected: false,
            tcp_connection: None,
        }
    }

    // Configure TCP connection
    pub fn with_tcp_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = Some(endpoint.to_string());
        self
    }

    // Configure shared memory connection
    pub fn with_shared_memory(mut self, name: &str) -> Self {
        self.shared_mem_name = Some(name.to_string());
        self
    }

    // Connect to the actuator system
    pub async fn connect(&mut self) -> Result<(), Box<dyn Error>> {
        match self.connection_type {
            ConnectionType::TcpSocket => {
                if let Some(endpoint) = &self.endpoint {
                    let stream = TcpStream::connect(endpoint).await?;
                    self.tcp_connection = Some(Arc::new(Mutex::new(stream)));
                    self.connected = true;
                } else {
                    return Err("TCP endpoint not configured".into());
                }
            }
            ConnectionType::SharedMemory => {
                // This would use a shared memory crate in a real implementation
                // For simulation purposes, we'll just mark as connected
                if self.shared_mem_name.is_some() {
                    self.connected = true;
                } else {
                    return Err("Shared memory name not configured".into());
                }
            }
            ConnectionType::CrossbeamChannel => {
                // For testing with crossbeam channels, always consider connected
                self.connected = true;
            }
        }

        Ok(())
    }

    // Send data to the actuator system
    pub async fn send_data(
        &self,
        data: &SensorData,
    ) -> Result<PerformanceMetrics, Box<dyn Error + Send + Sync + 'static>> {
        let mut metrics = PerformanceMetrics::new("data_transmission");

        if !self.connected {
            metrics.complete(false);
            return Err("Not connected to actuator system".into());
        }

        // Serialize the data
        let serialized = serde_json::to_string(data)?;

        match self.connection_type {
            ConnectionType::TcpSocket => {
                if let Some(conn) = &self.tcp_connection {
                    let mut stream = conn.lock().await;
                    stream.write_all(serialized.as_bytes()).await?;
                    // Add newline as delimiter
                    stream.write_all(b"\n").await?;
                }
            }
            ConnectionType::SharedMemory => {
                // In a real implementation, this would write to shared memory
                // For simulation, we'll just simulate the time it takes
                tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
            }
            ConnectionType::CrossbeamChannel => {
                // If we're using a crossbeam channel for direct in-process communication
                // This would send through the channel (implementation in run_transmitter)
            }
        }

        metrics.complete(true);
        Ok(metrics)
    }

    // Receive feedback from the actuator system
    pub async fn receive_feedback(&self) -> Result<ActuatorFeedback, Box<dyn Error>> {
        if !self.connected {
            return Err("Not connected to actuator system".into());
        }

        match self.connection_type {
            ConnectionType::TcpSocket => {
                if let Some(conn) = &self.tcp_connection {
                    let mut stream = conn.lock().await;
                    let mut buffer = Vec::new();
                    let mut temp_buf = [0u8; 1024];

                    // Read until newline
                    let mut found_newline = false;
                    while !found_newline {
                        let n = stream.read(&mut temp_buf).await?;
                        if n == 0 {
                            break;
                        }

                        for i in 0..n {
                            if temp_buf[i] == b'\n' {
                                buffer.extend_from_slice(&temp_buf[0..i]);
                                found_newline = true;
                                break;
                            }
                        }

                        if !found_newline {
                            buffer.extend_from_slice(&temp_buf[0..n]);
                        }
                    }

                    // Deserialize the feedback
                    let feedback: ActuatorFeedback = serde_json::from_slice(&buffer)?;
                    return Ok(feedback);
                }
                Err("TCP connection not available".into())
            }
            ConnectionType::SharedMemory => {
                // In a real implementation, this would read from shared memory
                // For simulation, just return a dummy feedback
                let feedback = ActuatorFeedback {
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis(),
                    actuator_id: "sim_actuator".to_string(),
                    status: crate::common::data_types::ActuatorStatus::Normal,
                    message: Some("Simulation feedback".to_string()),
                };
                Ok(feedback)
            }
            ConnectionType::CrossbeamChannel => {
                // This would be handled in run_transmitter
                Err("Feedback not implemented for CrossbeamChannel".into())
            }
        }
    }
}

// Function to run the transmitter in real-time
// pub async fn run_transmitter(
//     config: &crate::config::TransmitterConfig,
//     rx: crossbeam_channel::Receiver<SensorData>,
//     actuator_tx: Option<crossbeam_channel::Sender<ActuatorCommand>>,
//     metrics_tx: crossbeam_channel::Sender<PerformanceMetrics>,
//     feedback_tx: Option<crossbeam_channel::Sender<ActuatorFeedback>>,
// ) {
//     // Create and configure transmitter
//     let transmitter = match config.connection_type.as_str() {
//         "tcp" => {
//             let mut tx =
//                 DataTransmitter::new(ConnectionType::TcpSocket).with_tcp_endpoint(&config.endpoint);

//             // Try to connect
//             if let Err(e) = tx.connect().await {
//                 println!("Failed to connect transmitter: {}", e);
//                 return;
//             }
//             tx
//         }
//         "shared_memory" => {
//             let mut tx = DataTransmitter::new(ConnectionType::SharedMemory)
//                 .with_shared_memory(&config.shared_mem_name);

//             // Try to connect
//             if let Err(e) = tx.connect().await {
//                 println!("Failed to connect transmitter: {}", e);
//                 return;
//             }
//             tx
//         }
//         "channel" => DataTransmitter::new(ConnectionType::CrossbeamChannel),
//         _ => {
//             println!("Unknown connection type: {}", config.connection_type);
//             return;
//         }
//     };

pub async fn run_transmitter(
    command_rx: Receiver<ActuatorCommand>,
    feedback_tx: Sender<ActuatorFeedback>,
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

    // Listen for feedback
    let feedback_channel = channel.clone();
    let tx_clone = feedback_tx.clone();
    tokio::spawn(async move {
        use futures::StreamExt; // ⬅ Add this line to fix `.next()`
        let mut consumer = feedback_channel
            .basic_consume(
                ACTUATOR_FEEDBACK_QUEUE,
                "sensor_consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await
            .unwrap();

        while let Some(delivery) = consumer.next().await {
            if let Ok(delivery) = delivery {
                if let Ok(feedback) = serde_json::from_slice::<ActuatorFeedback>(&delivery.data) {
                    tx_clone.send(feedback).ok();
                }
                delivery.ack(BasicAckOptions::default()).await.unwrap();
            }
        }
    });

    // Send commands
    while let Ok(command) = command_rx.recv() {
        let data = serde_json::to_vec(&command)?; // ⬅ command must derive Serialize
        channel
            .basic_publish(
                "",
                ACTUATOR_COMMAND_QUEUE,
                BasicPublishOptions::default(),
                &data,
                BasicProperties::default(),
            )
            .await?
            .await?;
    }

    Ok(())
}

// Process and transmit data in real time
// loop {
//     // Try to receive processed data
//     match rx.recv() {
//         Ok(data) => {
//             let start = std::time::Instant::now();

//             if let ConnectionType::CrossbeamChannel = transmitter.connection_type {
//                 if let Some(tx) = &actuator_tx {
//                     let command = ActuatorCommand::from_sensor_data(&data); // You need to implement this conversion
//                     if tx.send(command).is_err() {
//                         println!("Actuator channel closed, stopping transmitter.");
//                         break;
//                     }
//                 }

//                 // Record metrics
//                 let mut metrics = PerformanceMetrics::new("data_transmission");
//                 metrics.complete(true);
//                 let _ = metrics_tx.send(metrics);
//             } else {
//                 // For other connection types, use the transmitter
//                 let mut attempts = 0;
//                 let max_attempts = 3;
//                 let mut success = false;
//                 let mut final_metrics = PerformanceMetrics::new("data_transmission");

//                 while attempts < max_attempts {
//                     match transmitter.send_data(&data).await {
//                         Ok(metrics) => {
//                             final_metrics = metrics;
//                             final_metrics.complete(true);
//                             success = true;
//                             break;
//                         }
//                         Err(e) => {
//                             // Convert error to String immediately for Send safety
//                             let err_msg = e.to_string();
//                             attempts += 1;
//                             println!(
//                                 "Attempt {}/{}: Failed to send data: {}",
//                                 attempts, max_attempts, err_msg
//                             );
//                             tokio::time::sleep(std::time::Duration::from_millis(100)).await;
//                         }
//                     }
//                 }

//                 if !success {
//                     final_metrics.complete(false);
//                 }
//                 let _ = metrics_tx.send(final_metrics);
//             }

//             // Check if transmission took too long
//             let transmission_time = start.elapsed();
//             if transmission_time.as_millis() > 1 {
//                 println!(
//                     "Warning: Transmission took too long: {:?}",
//                     transmission_time
//                 );
//             }

//             // Try to receive feedback
//             if let ConnectionType::CrossbeamChannel = transmitter.connection_type {
//                 // Feedback would come through a separate channel
//                 // No implementation for now
//             } else if let Some(tx) = &feedback_tx {
//                 match transmitter.receive_feedback().await {
//                     Ok(feedback) => {
//                         if tx.send(feedback).is_err() {
//                             println!("Feedback channel closed.");
//                         }
//                     }
//                     Err(_) => {
//                         // No feedback available or error
//                     }
//                 }
//             }
//         }
//         Err(_) => {
//             println!("Processor channel closed, stopping transmitter.");
//             break;
//         }
//     }
// }
// }
