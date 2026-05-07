// Bench-Setup darf expect() nutzen (kein no-panic-Kontrakt) und
// erfordert keine missing_docs auf Helper-Items.
#![allow(clippy::expect_used, missing_docs)]

//! Bench: Same-Host loopback latency (Spec §11.1).
//!
//! Misst gegen den InMemorySlotAllocator (kein POSIX-mmap).
//! Damit ist die Untergrenze "API-Overhead pro write+read" — die
//! reale POSIX-mmap-Variante ist im selben Bereich oder schneller.
//!
//! Run: `cargo bench -p zerodds-flatdata --bench loopback`

use std::sync::Arc;

use criterion::{Criterion, black_box, criterion_group, criterion_main};

use zerodds_flatdata::{FlatReader, FlatStruct, FlatWriter, InMemorySlotAllocator};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(C)]
struct Pose1k {
    /// 1024 byte payload.
    bytes: [u8; 1024],
}

// SAFETY: repr(C) + Copy + 'static, einziges Feld ist [u8; 1024].
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

criterion_group!(benches, bench_write_read_loopback);
criterion_main!(benches);
