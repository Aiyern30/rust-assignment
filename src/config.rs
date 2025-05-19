use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sensor: SensorConfig,
    pub processor: ProcessorConfig,
    pub transmitter: TransmitterConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    pub sample_rate_ms: u64,    // How often to generate sensor readings
    pub num_sensors: usize,     // Number of sensors to simulate
    pub enable_anomalies: bool, // Whether to intentionally generate anomalies
    pub anomaly_rate: f64,      // Rate of anomaly generation (0.0-1.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorConfig {
    pub window_size: usize,     // Size of moving average window
    pub anomaly_threshold: f64, // Base threshold for anomaly detection
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransmitterConfig {
    pub connection_type: String, // "tcp", "shared_memory", or "channel"
    pub endpoint: String,        // For TCP: address:port
    pub shared_mem_name: String, // For shared memory: name
    pub buffer_size: usize,      // Buffer size for communication
    pub retry_attempts: usize,   // How many times to retry failed transmissions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub log_to_file: bool,       // Whether to log metrics to file
    pub log_file: String,        // Path to log file
    pub report_interval_ms: u64, // How often to report metrics
}

impl Config {
    // Load configuration from file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let config: Config = serde_json::from_str(&contents)?;
        Ok(config)
    }

    // Get default configuration
    pub fn default() -> Self {
        Self {
            sensor: SensorConfig {
                sample_rate_ms: 5,      // 5ms sample rate
                num_sensors: 3,         // 3 sensors
                enable_anomalies: true, // Enable anomaly generation
                anomaly_rate: 0.01,     // 1% anomaly rate
            },
            processor: ProcessorConfig {
                window_size: 20,        // 20 samples window
                anomaly_threshold: 3.0, // 3 standard deviations
            },
            transmitter: TransmitterConfig {
                connection_type: "channel".to_string(), // Default to in-process channel
                endpoint: "127.0.0.1:8080".to_string(), // Default TCP endpoint
                shared_mem_name: "sensor_data".to_string(), // Default shared memory name
                buffer_size: 1024,                      // 1KB buffer
                retry_attempts: 3,                      // 3 retry attempts
            },
            metrics: MetricsConfig {
                log_to_file: true,                   // Log metrics to file
                log_file: "metrics.log".to_string(), // Default log file
                report_interval_ms: 1000,            // Report every second
            },
        }
    }

    // Save configuration to file
    pub fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = serde_json::to_string_pretty(self)?;
        std::fs::write(path, serialized)?;
        Ok(())
    }
}
