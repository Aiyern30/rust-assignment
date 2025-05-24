use criterion::Criterion;

pub fn benchmark_sensor(c: &mut Criterion) {
    c.bench_function("dummy_sensor_benchmark", |b| {
        b.iter(|| {
            let _ = 2 + 2;
        })
    });
}
