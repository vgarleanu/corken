use corken::*;
use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;

fn single_engine(mut state: State, txs: Vec<Transaction>) {
    for tx in txs {
        let _ = state.execute(tx);
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut rng = rand::thread_rng();
    println!("Generating {} txs", 1_000_000);

    let txs: Vec<Transaction> = (0..1_000_000).map(|_| rng.gen()).collect();

    c.bench_function("bench_single_engine", move |b| {
        b.iter_batched(
            || (txs.clone(), State::default()),
            |(txs, state)| single_engine(state, txs),
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
