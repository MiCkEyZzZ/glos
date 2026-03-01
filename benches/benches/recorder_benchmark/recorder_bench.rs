use criterion::{criterion_group, criterion_main, Criterion};

fn bench_or_recorder(_c: &mut Criterion) {
    unimplemented!();
}

criterion_group!(benches, bench_or_recorder);
criterion_main!(benches);
