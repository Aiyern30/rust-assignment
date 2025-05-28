use criterion::{criterion_group, criterion_main, Criterion};
use rust_assignment::actuator::controller::PIDController;
use rust_assignment::common::data_types::{
    ActuatorCommand, ActuatorFeedback, ActuatorStatus, ControlCommand,
};
use rust_assignment::common::data_types::{SensorData, SensorType};
use rust_assignment::sensor::generator::SensorGenerator;
use rust_assignment::sensor::processor::DataProcessor;
use std::hint::black_box;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn benchmark_generate_reading(c: &mut Criterion) {
    let mut generator =
        SensorGenerator::new("test_sensor", SensorType::Force, 100, 10.0, 0.2, 0.01);

    c.bench_function("sensor_generate_reading", |b| {
        b.iter(|| {
            let (reading, _metrics) = generator.generate_reading();
            black_box(reading);
        });
    });
}

pub fn benchmark_processor(c: &mut Criterion) {
    let mut processor = DataProcessor::new(10);
    c.bench_function("sensor_processor_process", |b| {
        b.iter(|| {
            let data = black_box(SensorData {
                sensor_id: "S1".to_string(),
                reading_type: SensorType::Force,
                value: 10.0,
                timestamp: 0,
                is_anomaly: false,
                confidence: 1.0,
                forwarded_at: 0,
            });
            let _ = processor.process(data);
        });
    });
}

pub fn benchmark_actuator_processing(c: &mut Criterion) {
    let command = ActuatorCommand {
        command_id: "cmd1".to_string(),
        actuator_id: "A1".to_string(),
        control_command: ControlCommand {
            command_type: "RegulateTemperature".to_string(),
            payload: Some("cool".to_string()),
            timestamp: current_time(),
            value: 85.0,
            deadline: current_time() + 2000,
        },
        priority: 10,
        deadline: current_time() + 2000,
        forwarded_at: Some(current_time()),
    };
    let mut pid = PIDController::new(1.0, 0.1, 0.05);

    c.bench_function("actuator_processing", |b| {
        b.iter(|| {
            let control = pid.compute(75.0, command.control_command.value, 0.005);
            let status = if (75.0 - command.control_command.value).abs() <= 0.5 {
                ActuatorStatus::Success
            } else {
                ActuatorStatus::Adjusting
            };
            let _feedback = ActuatorFeedback {
                timestamp: current_time(),
                actuator_id: command.actuator_id.clone(),
                status,
                message: Some("Benchmark feedback".to_string()),
            };
            black_box(_feedback);
        });
    });
}

fn current_time() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

pub fn benchmark_serialization(c: &mut Criterion) {
    let data = SensorData {
        sensor_id: "S1".to_string(),
        reading_type: SensorType::Force,
        value: 10.0,
        timestamp: 0,
        is_anomaly: false,
        confidence: 1.0,
        forwarded_at: 0,
    };

    c.bench_function("json_serialization", |b| {
        b.iter(|| {
            let serialized = black_box(serde_json::to_string(&data).unwrap());
            black_box(serialized);
        });
    });

    let json_str = r#"{
        "sensor_id":"S1",
        "reading_type":"Force",
        "value":10.0,
        "timestamp":0,
        "is_anomaly":false,
        "confidence":1.0,
        "forwarded_at":0
    }"#;

    c.bench_function("json_deserialization", |b| {
        b.iter(|| {
            let data: SensorData = black_box(serde_json::from_str(json_str).unwrap());
            black_box(data);
        });
    });
}

/// Benchmark ActuatorFeedback deserialization
pub fn benchmark_actuator_feedback_deserialization(c: &mut Criterion) {
    let json = br#"{
        "actuator_id": "A1",
        "status": "Success",
        "message": "Command executed successfully",
        "timestamp": 1234567890
    }"#;

    c.bench_function("actuator_feedback_deserialization", |b| {
        b.iter(|| {
            let feedback: ActuatorFeedback = black_box(serde_json::from_slice(json).unwrap());
            black_box(feedback);
        });
    });
}

/// Simulate just the encoding step in transmitter
pub fn benchmark_transmitter_encode_step(c: &mut Criterion) {
    let command = ActuatorCommand {
        command_id: "CMD999".to_string(),
        actuator_id: "A2".to_string(),
        control_command: ControlCommand {
            command_type: "SetPosition".to_string(),
            payload: Some("Target=42".to_string()),
            timestamp: 1234567890,
            value: 42.0,
            deadline: todo!(),
        },
        priority: 2,
        deadline: 9876543210,
        forwarded_at: todo!(),
    };

    c.bench_function("transmitter_encode_sim", |b| {
        b.iter(|| {
            let json = black_box(serde_json::to_vec(&command).unwrap());
            black_box(json);
        });
    });
}

criterion_group!(
    benches,
    benchmark_generate_reading,
    benchmark_processor,
    benchmark_serialization
);
criterion_main!(benches);
