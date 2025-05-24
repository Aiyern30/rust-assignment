use criterion::{criterion_group, criterion_main};

// Declare the modules (assuming they exist in benches/)
mod processor_benchmark;
mod sensor_benchmark;

criterion_group!(
    benches,
    processor_benchmark::benchmark_processor,
    sensor_benchmark::benchmark_sensor,
);
criterion_main!(benches);
