// The bench setup may use expect() (no no-panic contract) and does not
// require missing_docs on helper items.
#![allow(clippy::expect_used, missing_docs)]

//! Bench: Same-host loopback latency (Spec §11.1).
//!
//! Measures against the InMemorySlotAllocator (no POSIX mmap).
//! The lower bound is therefore "API overhead per write+read" — the
//! real POSIX-mmap variant is in the same range or faster.
//!
//! Run: `cargo bench -p zerodds-flatdata --bench loopback`

use std::sync::Arc;

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};

use zerodds_flatdata::{FlatReader, FlatStruct, FlatWriter, InMemorySlotAllocator};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
struct Pose1k {
    /// 1024 byte payload.
    bytes: [u8; 1024],
}

// SAFETY: repr(C) + Copy + 'static, the only field is [u8; 1024].
unsafe impl FlatStruct for Pose1k {
    const TYPE_HASH: [u8; 16] = [0x42; 16];
}

fn bench_write_read_loopback(c: &mut Criterion) {
    let alloc = Arc::new(InMemorySlotAllocator::new(0, 16, Pose1k::WIRE_SIZE));
    let writer = FlatWriter::<Pose1k>::new(Arc::clone(&alloc), 0b1);
    let reader = FlatReader::<Pose1k>::new(Arc::clone(&alloc), 0);

    c.bench_function("flat_write_read_1kb", |b| {
        let payload = Pose1k {
            bytes: [0xAA; 1024],
        };
        b.iter(|| {
            let _ = writer.write(black_box(&payload)).expect("write");
            let _ = reader.read().expect("read");
        });
    });
}

/// Spec §11.2 — memcpy-bound write throughput. Reports both elements/s
/// (samples) and bytes/s so the ~1 GB/s target is directly readable. The
/// reader drains so slots recycle (steady state, no NoFreeSlot).
fn bench_write_throughput(c: &mut Criterion) {
    let alloc = Arc::new(InMemorySlotAllocator::new(0, 64, Pose1k::WIRE_SIZE));
    let writer = FlatWriter::<Pose1k>::new(Arc::clone(&alloc), 0b1);
    let reader = FlatReader::<Pose1k>::new(Arc::clone(&alloc), 0);
    let payload = Pose1k {
        bytes: [0xAA; 1024],
    };

    let mut group = c.benchmark_group("flat_throughput_1kb");
    group.throughput(Throughput::Bytes(Pose1k::WIRE_SIZE as u64));
    group.bench_function("write", |b| {
        b.iter(|| {
            let _ = writer.write(black_box(&payload)).expect("write");
            let _ = reader.read().expect("read"); // drain → slot recycles
        });
    });
    group.throughput(Throughput::Elements(1));
    group.bench_function("write_samples", |b| {
        b.iter(|| {
            let _ = writer.write(black_box(&payload)).expect("write");
            let _ = reader.read().expect("read");
        });
    });
    group.finish();
}

criterion_group!(benches, bench_write_read_loopback, bench_write_throughput);
criterion_main!(benches);
