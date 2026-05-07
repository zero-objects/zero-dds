//! WP 2.0a Zero-Copy-Spike — Baseline-Bench.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]
//!
//! Misst den Writer-Hot-Path `write()` + `tick()-with-resends`. Zahlen
//! sind Ausgangspunkt fuer den Refactor `DataSubmessage::
//! serialized_payload: Vec<u8>` → `Arc<[u8]>`.
//!
//! Run:
//!   cargo bench -p zerodds-rtps --bench writer_dispatch -- --save-baseline pre

use core::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_rtps::history_cache::HistoryKind;
use zerodds_rtps::message_builder::DEFAULT_MTU;
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::reliable_writer::{
    DEFAULT_FRAGMENT_SIZE, DEFAULT_HEARTBEAT_PERIOD, ReliableWriter, ReliableWriterConfig,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, SequenceNumber, VendorId};

fn make_writer() -> ReliableWriter {
    let writer_guid = Guid::new(
        GuidPrefix::from_bytes([1; 12]),
        EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
    );
    let reader_guid = Guid::new(
        GuidPrefix::from_bytes([2; 12]),
        EntityId::user_reader_with_key([0x40, 0x50, 0x60]),
    );
    let reader_proxy = ReaderProxy::new(
        reader_guid,
        vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
        vec![],
        true,
    );
    ReliableWriter::new(ReliableWriterConfig {
        guid: writer_guid,
        vendor_id: VendorId::ZERODDS,
        reader_proxies: vec![reader_proxy],
        max_samples: 4096,
        history_kind: HistoryKind::KeepAll,
        heartbeat_period: DEFAULT_HEARTBEAT_PERIOD,
        fragment_size: DEFAULT_FRAGMENT_SIZE,
        mtu: DEFAULT_MTU,
    })
}

fn reader_guid() -> Guid {
    Guid::new(
        GuidPrefix::from_bytes([2; 12]),
        EntityId::user_reader_with_key([0x40, 0x50, 0x60]),
    )
}

fn bench_write_sized(c: &mut Criterion) {
    let mut group = c.benchmark_group("writer_write");
    for size in [64usize, 1024, 4096, 16_384, 65_536] {
        let payload = vec![0xABu8; size];
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &payload, |b, payload| {
            b.iter_batched(
                make_writer,
                |mut w| {
                    let dgs = w.write(black_box(&payload.clone())).expect("write ok");
                    black_box(dgs);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_tick_resend(c: &mut Criterion) {
    let mut group = c.benchmark_group("writer_tick_resend");
    for backlog in [1usize, 10, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(backlog),
            &backlog,
            |b, &backlog| {
                let payload = vec![0xABu8; 1024];
                b.iter_batched(
                    || {
                        let mut w = make_writer();
                        for _ in 0..backlog {
                            w.write(&payload.clone()).unwrap();
                        }
                        let requested: Vec<SequenceNumber> =
                            (1..=(backlog as i64)).map(SequenceNumber).collect();
                        (w, requested)
                    },
                    |(mut w, requested)| {
                        // Simulate an incoming ACKNACK that re-requests the
                        // full backlog — triggers the resend hot-path.
                        w.handle_acknack(reader_guid(), SequenceNumber(1), requested);
                        let out = w.tick(Duration::from_millis(0)).expect("tick ok");
                        black_box(out);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_write_sized, bench_tick_resend);
criterion_main!(benches);
