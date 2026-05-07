//! Criterion-Benches fuer XCDR-Encode/Decode-Hot-Paths.
//! Regression-Detection. Kein Cross-Vendor-Vergleich.

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use zerodds_cdr::Endianness;
use zerodds_cdr::buffer::{BufferReader, BufferWriter};
use zerodds_cdr::encode::{CdrDecode, CdrEncode};

fn bench_u32_encode(c: &mut Criterion) {
    c.bench_function("cdr_u32_encode_le", |b| {
        b.iter(|| {
            let mut w = BufferWriter::new(Endianness::Little);
            for i in 0u32..100 {
                black_box(i).encode(&mut w).unwrap();
            }
            w.into_bytes()
        });
    });
}

fn bench_u32_decode(c: &mut Criterion) {
    let mut w = BufferWriter::new(Endianness::Little);
    for i in 0u32..100 {
        i.encode(&mut w).unwrap();
    }
    let bytes = w.into_bytes();
    c.bench_function("cdr_u32_decode_le", |b| {
        b.iter(|| {
            let mut r = BufferReader::new(black_box(&bytes), Endianness::Little);
            for _ in 0..100 {
                let _: u32 = u32::decode(&mut r).unwrap();
            }
        });
    });
}

fn bench_u64_encode(c: &mut Criterion) {
    c.bench_function("cdr_u64_encode_le", |b| {
        b.iter(|| {
            let mut w = BufferWriter::new(Endianness::Little);
            for i in 0u64..100 {
                black_box(i).encode(&mut w).unwrap();
            }
            w.into_bytes()
        });
    });
}

fn bench_f64_decode(c: &mut Criterion) {
    let mut w = BufferWriter::new(Endianness::Little);
    for i in 0..100 {
        f64::from(i).encode(&mut w).unwrap();
    }
    let bytes = w.into_bytes();
    c.bench_function("cdr_f64_decode_le", |b| {
        b.iter(|| {
            let mut r = BufferReader::new(black_box(&bytes), Endianness::Little);
            for _ in 0..100 {
                let _: f64 = f64::decode(&mut r).unwrap();
            }
        });
    });
}

criterion_group!(
    benches,
    bench_u32_encode,
    bench_u32_decode,
    bench_u64_encode,
    bench_f64_decode,
);
criterion_main!(benches);
