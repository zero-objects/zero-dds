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

use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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

/// Spec §11.4 — "**Same-Host-Pub-Latenz < UDP-Pub-Latenz / 5**": the
/// same 1 kB payload published once over the slot path and once as a
/// bare `send_to` on a bound loopback socket, as two independently
/// reported criterion benches.
///
/// This is the live form of the comparison. It does NOT belong in a unit
/// test: a ratio of two latencies measured on one machine also measures
/// the machine, so the gate moves with the host's `sendto` cost while the
/// code under test stands still (five runs on one dev machine spread the
/// UDP median over 3.375-5.791 µs at a constant 83 ns publish median; the
/// CI container measured 1.704 µs for the same `sendto`). Under criterion
/// each side gets warm-up, batched timing and an outlier report, and
/// `bench-compare` in CI checks each side against the previous baseline
/// of the SAME host — which is the comparison that carries information.
/// The absolute §11.4 budget stays asserted in
/// `tests/perf_targets.rs::same_host_publish_beats_a_fifth_of_udp_publish`.
fn bench_publish_flat_vs_udp(c: &mut Criterion) {
    let mut group = c.benchmark_group("flat_vs_udp_publish_1kb");
    group.throughput(Throughput::Bytes(Pose1k::WIRE_SIZE as u64));

    // Reader mask 0: no reader has to acknowledge, so a committed slot is
    // free again at once and the timed closure is the bare publish path —
    // the quantity §11.4 names. The reader-side drain of the round-trip
    // benches above would otherwise land inside the measurement.
    let alloc = Arc::new(InMemorySlotAllocator::new(0, 16, Pose1k::WIRE_SIZE));
    let writer = FlatWriter::<Pose1k>::new(alloc, 0);
    let payload = Pose1k {
        bytes: [0xAA; 1024],
    };
    group.bench_function("flat_publish", |b| {
        b.iter(|| writer.write(black_box(&payload)).expect("write"));
    });

    // UDP reference: `send_to` of the same 1 kB, no RTPS header, no
    // encapsulation, no serialisation — the lower bound of a UDP publish.
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let peer = UdpSocket::bind("127.0.0.1:0").expect("bind peer");
    let peer_addr = peer.local_addr().expect("addr");
    peer.set_read_timeout(Some(Duration::from_millis(50)))
        .expect("read timeout");
    // A drain thread keeps the peer's receive queue from filling up; a
    // full queue would make `send_to` block or fail and measure the
    // queue, not the send.
    let stop = Arc::new(AtomicBool::new(false));
    let drain_stop = Arc::clone(&stop);
    let drain = std::thread::spawn(move || {
        let mut buf = [0u8; 2048];
        while !drain_stop.load(Ordering::Relaxed) {
            let _ = peer.recv_from(&mut buf);
        }
    });
    let udp_payload = [0xAAu8; 1024];
    group.bench_function("udp_publish", |b| {
        b.iter(|| {
            sock.send_to(black_box(&udp_payload), peer_addr)
                .expect("send")
        });
    });
    stop.store(true, Ordering::Relaxed);
    drain.join().expect("drain thread");

    group.finish();
}

criterion_group!(
    benches,
    bench_write_read_loopback,
    bench_write_throughput,
    bench_publish_flat_vs_udp
);
criterion_main!(benches);
