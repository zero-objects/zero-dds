//! Criterion benches for the XCDR writer and reader hot paths.
//!
//! Successor of `crates/cdr/benches/encode_decode_hotpaths.rs`, which was
//! removed together with `zerodds-cdr` in d045e43ea86. The runtime the
//! generated codecs call now lives in `zerodds-wire`, so the hot paths
//! moved here with it.
//!
//! Regression detection for this repository only. No cross-vendor
//! comparison — that is `tests/perf/dds-roundtrip-bench`.
//!
//! Measured:
//! * scalar encode/decode (`u32`, `u64`, `f64`) — the loop every plain
//!   member goes through
//! * `string` encode/decode — length word, bytes, NUL, and the
//!   allocation on the read side
//! * XCDR2 appendable framing (DHEADER write + patch, read)
//! * XCDR2 mutable framing (EMHEADER per member, both length codes that
//!   need a NEXTINT and those that do not)
//! * big-endian scalars — the byte-swapping path a BE peer takes

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_wire::{Endianness, Reader, Writer};

/// How many members a scalar loop writes. Same count the removed
/// `zerodds-cdr` bench used, so the shape of the numbers is comparable.
const N: usize = 100;

fn bench_scalar_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("wire_scalar_encode");
    group.throughput(Throughput::Bytes((N * 4) as u64));
    group.bench_function("u32_le", |b| {
        b.iter(|| {
            let mut w = Writer::new(Endianness::Little);
            for i in 0..N as u32 {
                w.write_u32(black_box(i));
            }
            w.finish()
        });
    });
    group.throughput(Throughput::Bytes((N * 8) as u64));
    group.bench_function("u64_le", |b| {
        b.iter(|| {
            let mut w = Writer::new(Endianness::Little);
            for i in 0..N as u64 {
                w.write_u64(black_box(i));
            }
            w.finish()
        });
    });
    group.bench_function("f64_le", |b| {
        b.iter(|| {
            let mut w = Writer::new(Endianness::Little);
            for i in 0..N {
                w.write_f64(black_box(i as f64));
            }
            w.finish()
        });
    });
    // The BE writer swaps every word; a BE peer pays this on every member.
    group.throughput(Throughput::Bytes((N * 4) as u64));
    group.bench_function("u32_be", |b| {
        b.iter(|| {
            let mut w = Writer::new(Endianness::Big);
            for i in 0..N as u32 {
                w.write_u32(black_box(i));
            }
            w.finish()
        });
    });
    group.finish();
}

fn scalars(endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    for i in 0..N as u32 {
        w.write_u32(i);
    }
    w.finish()
}

fn bench_scalar_decode(c: &mut Criterion) {
    let le = scalars(Endianness::Little);
    let be = scalars(Endianness::Big);

    let mut doubles = Writer::new(Endianness::Little);
    for i in 0..N {
        doubles.write_f64(i as f64);
    }
    let doubles = doubles.finish();

    let mut group = c.benchmark_group("wire_scalar_decode");
    group.throughput(Throughput::Bytes(le.len() as u64));
    group.bench_function("u32_le", |b| {
        b.iter(|| {
            let mut r = Reader::new(black_box(&le), Endianness::Little);
            for _ in 0..N {
                black_box(r.read_u32().unwrap());
            }
        });
    });
    group.bench_function("u32_be", |b| {
        b.iter(|| {
            let mut r = Reader::new(black_box(&be), Endianness::Big);
            for _ in 0..N {
                black_box(r.read_u32().unwrap());
            }
        });
    });
    group.throughput(Throughput::Bytes(doubles.len() as u64));
    group.bench_function("f64_le", |b| {
        b.iter(|| {
            let mut r = Reader::new(black_box(&doubles), Endianness::Little);
            for _ in 0..N {
                black_box(r.read_f64().unwrap());
            }
        });
    });
    group.finish();
}

/// Names of the size a DDS topic actually carries — short enough that the
/// allocation, not the copy, dominates the read side.
const NAMES: [&str; 8] = [
    "Square",
    "Circle",
    "Triangle",
    "sensor/front/left",
    "rt/tf_static",
    "DCPSPublication",
    "DCPSSubscription",
    "DCPSParticipant",
];

fn bench_string(c: &mut Criterion) {
    let mut w = Writer::new(Endianness::Little);
    for name in NAMES {
        w.write_string(name);
    }
    let encoded = w.finish();

    let mut group = c.benchmark_group("wire_string");
    group.throughput(Throughput::Bytes(encoded.len() as u64));
    group.bench_function("encode", |b| {
        b.iter(|| {
            let mut w = Writer::new(Endianness::Little);
            for name in NAMES {
                w.write_string(black_box(name));
            }
            w.finish()
        });
    });
    group.bench_function("decode", |b| {
        b.iter(|| {
            let mut r = Reader::new(black_box(&encoded), Endianness::Little);
            for _ in 0..NAMES.len() {
                black_box(r.read_string().unwrap());
            }
        });
    });
    group.finish();
}

/// One `@appendable` aggregate: DHEADER, then eight plain members.
///
/// `write_dheader` reserves a placeholder and `finish_dheader` patches it,
/// which is the extra work an appendable type pays over a final one.
fn bench_appendable_framing(c: &mut Criterion) {
    let mut w = Writer::new(Endianness::Little);
    let h = w.write_dheader();
    for i in 0..8u32 {
        w.write_u32(i);
    }
    w.finish_dheader(h);
    let encoded = w.finish();

    let mut group = c.benchmark_group("wire_appendable");
    group.throughput(Throughput::Bytes(encoded.len() as u64));
    group.bench_function("encode", |b| {
        b.iter(|| {
            let mut w = Writer::new(Endianness::Little);
            let h = w.write_dheader();
            for i in 0..8u32 {
                w.write_u32(black_box(i));
            }
            w.finish_dheader(h);
            w.finish()
        });
    });
    group.bench_function("decode", |b| {
        b.iter(|| {
            let mut r = Reader::new(black_box(&encoded), Endianness::Little);
            black_box(r.read_dheader().unwrap());
            for _ in 0..8 {
                black_box(r.read_u32().unwrap());
            }
        });
    });
    group.finish();
}

/// One `@mutable` aggregate: an EMHEADER per member.
///
/// Length code 2 carries the length in the code, so no NEXTINT is written;
/// code 4 reserves one and patches it in `finish_emheader`. Both are
/// exercised, because a mutable type mixes them.
fn bench_mutable_framing(c: &mut Criterion) {
    fn encode() -> Vec<u8> {
        let mut w = Writer::new(Endianness::Little);
        let h = w.write_dheader();
        for id in 0..8u32 {
            // LC2: a four-byte member, length implied by the code.
            let em = w.write_emheader(id, 2, false).unwrap();
            w.write_u32(id);
            w.finish_emheader(em).unwrap();
        }
        for id in 8..12u32 {
            // LC4: a NEXTINT the writer has to reserve and patch.
            let em = w.write_emheader(id, 4, false).unwrap();
            w.write_string("member");
            w.finish_emheader(em).unwrap();
        }
        w.finish_dheader(h);
        w.finish()
    }

    let encoded = encode();

    let mut group = c.benchmark_group("wire_mutable");
    group.throughput(Throughput::Bytes(encoded.len() as u64));
    group.bench_function("encode", |b| b.iter(|| black_box(encode())));
    group.bench_function("decode", |b| {
        b.iter(|| {
            let mut r = Reader::new(black_box(&encoded), Endianness::Little);
            let end = r.read_dheader().unwrap();
            let _ = black_box(end);
            for _ in 0..12 {
                let head = r.read_emheader().unwrap();
                black_box(head.member_id);
                let past = r.position() + head.extent;
                r.skip_to(past).unwrap();
            }
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_scalar_encode,
    bench_scalar_decode,
    bench_string,
    bench_appendable_framing,
    bench_mutable_framing,
);
criterion_main!(benches);
