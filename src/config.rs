use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sensor: SensorConfig,
    pub processor: ProcessorConfig,

    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    pub sample_rate_ms: u64,
    pub num_sensors: usize,
    pub enable_anomalies: bool,
    pub anomaly_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessorConfig {
    pub window_size: usize,
    pub anomaly_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub log_to_file: bool,
    pub log_file: String,
    pub report_interval_ms: u64,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let config: Config = serde_json::from_str(&contents)?;
        Ok(config)
    }

    pub fn default() -> Self {
        Self {
            sensor: SensorConfig {
                sample_rate_ms: 5,
                num_sensors: 3,
                enable_anomalies: true,
                anomaly_rate: 0.01,
            },
            processor: ProcessorConfig {
                window_size: 20,
                anomaly_threshold: 3.0,
            },

            metrics: MetricsConfig {
                log_to_file: true,
                log_file: "metrics.log".to_string(),
                report_interval_ms: 1000,
            },
        }
    }
}
