use criterion::{criterion_group, criterion_main, Criterion};
use rust_assignment::common::data_types::{SensorData, SensorType};
use rust_assignment::sensor::processor::DataProcessor;
use std::hint::black_box;

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
            });
            let _ = processor.process(data);
        });
    });
}

pub fn benchmark_serialization(c: &mut Criterion) {
    // Benchmark JSON serialization (what transmitter does)
    c.bench_function("json_serialization", |b| {
        let data = SensorData {
            sensor_id: "S1".to_string(),
            reading_type: SensorType::Force,
            value: 10.0,
            timestamp: 0,
            is_anomaly: false,
            confidence: 1.0,
        };
        
        b.iter(|| {
            let serialized = black_box(serde_json::to_string(&data).unwrap());
            black_box(serialized);
        });
    });

    // Benchmark JSON deserialization 
    c.bench_function("json_deserialization", |b| {
        let json_str = r#"{"sensor_id":"S1","reading_type":"Force","value":10.0,"timestamp":0,"is_anomaly":false,"confidence":1.0}"#;
        
        b.iter(|| {
            let data: SensorData = black_box(serde_json::from_str(json_str).unwrap());
            black_box(data);
        });
    });
}

criterion_group!(benches, benchmark_processor, benchmark_serialization);
criterion_main!(benches);