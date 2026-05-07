//! Criterion-Benches fuer AMQP-Decoder-Hot-Paths.
//!
//! Zweck: Regression-Detection. Baseline:
//!   `cargo bench -p zerodds-amqp-bridge --bench decode_hotpaths -- --save-baseline pre`
//! Nach Refactor:
//!   `cargo bench -p zerodds-amqp-bridge --bench decode_hotpaths -- --baseline pre`
//!
//! Kein Cross-Vendor-Vergleich, kein Marketing — nur Sanity-Check
//! dass keine Regression > 10% ungesehen durchrutscht.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_amqp_bridge::frame::decode_frame_header;
use zerodds_amqp_bridge::performatives::decode_performative;
use zerodds_amqp_bridge::types::{decode_value, encode_string, encode_ulong};

fn build_string_value(len: usize) -> Vec<u8> {
    let s: String = "x".repeat(len);
    encode_string(&s).unwrap()
}

fn build_minimal_frame() -> Vec<u8> {
    // SIZE=0x00000008 DOFF=0x02 TYPE=0x00 CHANNEL=0x0000
    vec![0x00, 0x00, 0x00, 0x08, 0x02, 0x00, 0x00, 0x00]
}

fn bench_decode_value_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("amqp_decode_value_string");
    for &len in &[8usize, 64, 256, 1024, 4096] {
        let bytes = build_string_value(len);
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_function(format!("len={len}"), |b| {
            b.iter(|| {
                let _ = decode_value(black_box(&bytes));
            });
        });
    }
    group.finish();
}

fn bench_decode_value_ulong(c: &mut Criterion) {
    let bytes = encode_ulong(0xDEAD_BEEF_DEAD_BEEF);
    c.bench_function("amqp_decode_value_ulong", |b| {
        b.iter(|| {
            let _ = decode_value(black_box(&bytes));
        });
    });
}

fn bench_decode_frame_header(c: &mut Criterion) {
    let bytes = build_minimal_frame();
    c.bench_function("amqp_decode_frame_header", |b| {
        b.iter(|| {
            let _ = decode_frame_header(black_box(&bytes));
        });
    });
}

fn bench_decode_performative_garbage(c: &mut Criterion) {
    // Random-ish bytes — Performative-Decoder muss schnell scheitern.
    let bytes = vec![0u8; 64];
    c.bench_function("amqp_decode_performative_invalid", |b| {
        b.iter(|| {
            let _ = decode_performative(black_box(&bytes));
        });
    });
}

criterion_group!(
    benches,
    bench_decode_value_string,
    bench_decode_value_ulong,
    bench_decode_frame_header,
    bench_decode_performative_garbage,
);
criterion_main!(benches);
