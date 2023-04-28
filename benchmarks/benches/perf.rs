use std::{hint::black_box, time::Duration};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode};
use flip_hash::{flip_hash_64, flip_hash_xxh3_64};
use flip_hash_benchmarks::jump_hash;
use rand::{thread_rng, RngCore};
use xxhash_rust::xxh3;

const RANGE_ENDS: [u64; 4] = [10, 1000, 100000, 10000000];

fn hash_u64(c: &mut Criterion) {
    let mut group = c.benchmark_group("HashU64");
    group.sampling_mode(SamplingMode::Flat);
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_millis(1000));
    group.sample_size(1000);

    let mut rng = thread_rng();

    for range_end in RANGE_ENDS {
        group.bench_with_input(
            BenchmarkId::new("Jump", format!("..={}", range_end)),
            &..=range_end,
            |b, &range| {
                let key = rng.next_u64();
                b.iter(|| jump_hash(black_box(key), black_box(..=range.end as u32)))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("Flip", format!("..={}", range_end)),
            &..=range_end,
            |b, &range| {
                let key = rng.next_u64();
                b.iter(|| flip_hash_64(black_box(key), black_box(range)))
            },
        );
    }
    group.finish();
}

fn hash_bytes_with_xxh3(c: &mut Criterion) {
    let mut group = c.benchmark_group("HashBytes");
    group.sampling_mode(SamplingMode::Flat);
    group.warm_up_time(Duration::from_millis(300));
    group.measurement_time(Duration::from_millis(1000));
    group.sample_size(1000);

    let mut rng = thread_rng();
    let mut bytes = [0_u8; 128];
    for range_end in RANGE_ENDS {
        group.bench_with_input(
            BenchmarkId::new("XXH3_then_Jump", format!("..={}", range_end)),
            &..=range_end as u32,
            |b, &range| {
                rng.fill_bytes(&mut bytes);
                b.iter(|| jump_hash(xxh3::xxh3_64(&black_box(bytes)), black_box(range)))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("XXH3_then_Flip", format!("..={}", range_end)),
            &..=range_end,
            |b, &range| {
                rng.fill_bytes(&mut bytes);
                b.iter(|| flip_hash_64(xxh3::xxh3_64(&black_box(bytes)), black_box(range)))
            },
        );
        group.bench_with_input(
            BenchmarkId::new("XXH3_based_Flip", format!("..={}", range_end)),
            &..=range_end,
            |b, &range| {
                rng.fill_bytes(&mut bytes);
                b.iter(|| flip_hash_xxh3_64(&black_box(bytes), black_box(range)))
            },
        );
    }
    group.finish();
}

criterion_group!(benches, hash_u64, hash_bytes_with_xxh3);
criterion_main!(benches);
