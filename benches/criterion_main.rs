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

// Include all benchmarks in one group
criterion_group!(benches, benchmark_processor);
criterion_main!(benches);
