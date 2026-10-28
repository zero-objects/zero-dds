// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RTPS writer fragmented bench — measures the DATA_FRAG path for
//! Samples > MTU.
//!
//! # Scope
//!
//! Complements `transports_e2e.rs`: the transport bench is DGRAM-
//! capped at 60 KiB. Real DDS samples, however, can be arbitrarily
//! large — the writer splits them into MTU-sized DATA_FRAG
//! Submessages. This bench measures `ReliableWriter::write(payload)`
//! for payloads that are **far** above MTU, so that the
//! fragmentation cost becomes visible.
//!
//! Payload-Achse: 32 B (below-MTU, sanity) → 4 MiB (kamerabild-
//! size, ~3100 fragments @ 1344 B).
//!
//! # What is measured
//!
//! - Per-sample time for `write()` including:
//!   - Arc payload build (zero-copy path),
//!   - HistoryCache-Insert,
//!   - Fragmentation in N DATA_FRAG-Submessages (N = ceil(size / 1344)),
//!   - Datagram build per fragment.
//! - **No network send**: the bench only calls `writer.write()` and
//!   discards the datagram vec. Transport time is separate in
//!   `transports_e2e.rs`.
//!
//! # Why not transport E2E?
//!
//! A complete "Writer → Transport → Reader → Assembler" path
//! is the concern of a live interop harness (two real hosts with
//! timing). This is the *protocol* bench for fragmentation
//! isolated.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    clippy::print_stderr,
    clippy::print_stdout
)]

use core::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_bench_suite::{PAYLOAD_SIZES, make_payload, size_label};
use zerodds_rtps::history_cache::HistoryKind;
use zerodds_rtps::message_builder::DEFAULT_MTU;
use zerodds_rtps::reader_proxy::ReaderProxy;
use zerodds_rtps::reliable_writer::{
    DEFAULT_FRAGMENT_SIZE, DEFAULT_HEARTBEAT_PERIOD, ReliableWriter, ReliableWriterConfig,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, VendorId};

fn make_writer(max_samples: usize) -> ReliableWriter {
    let writer_guid = Guid::new(
        GuidPrefix::from_bytes([1; 12]),
        EntityId::user_writer_with_key([0x10, 0x20, 0x30]),
    );
    let reader_proxy = ReaderProxy::new(
        Guid::new(
            GuidPrefix::from_bytes([2; 12]),
            EntityId::user_reader_with_key([0x40, 0x50, 0x60]),
        ),
        vec![Locator::udp_v4([127, 0, 0, 1], 7410)],
        vec![],
        true,
    );
    ReliableWriter::new(ReliableWriterConfig {
        guid: writer_guid,
        vendor_id: VendorId::ZERODDS,
        reader_proxies: vec![reader_proxy],
        max_samples,
        history_kind: HistoryKind::KeepAll,
        heartbeat_period: DEFAULT_HEARTBEAT_PERIOD,
        fragment_size: DEFAULT_FRAGMENT_SIZE,
        mtu: DEFAULT_MTU,
    })
}

fn bench_writer_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("writer_write");
    group.measurement_time(Duration::from_secs(5));
    for &size in PAYLOAD_SIZES {
        let payload = make_payload(size);
        let n_fragments = size.div_ceil(DEFAULT_FRAGMENT_SIZE as usize);
        // Criterion throughput counts the sample bytes, not the
        // Total sent frame bytes. Both informational.
        group.throughput(Throughput::Bytes(size as u64));

        // Label includes the fragment count, so that the report
        // is directly readable.
        let label = if n_fragments <= 1 {
            format!("{} (1 frag)", size_label(size))
        } else {
            format!("{} ({} frags)", size_label(size), n_fragments)
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &payload,
            |b, payload| {
                // Set up the writer fresh per iter, so that the history cache
                // does not fill up and the bench is deterministic.
                b.iter_batched(
                    || make_writer(1),
                    |mut w| {
                        let dgs = w.write(black_box(payload.as_slice())).expect("write ok");
                        black_box(dgs);
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_writer_write);
criterion_main!(benches);
