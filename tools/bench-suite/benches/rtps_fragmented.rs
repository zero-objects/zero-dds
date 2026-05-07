// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! RTPS-Writer-Fragmented Bench — misst den DATA_FRAG-Pfad fuer
//! Samples > MTU.
//!
//! # Scope
//!
//! Ergaenzt `transports_e2e.rs`: der Transport-Bench ist DGRAM-
//! gekappt bei 60 KiB. Echte DDS-Samples koennen aber beliebig
//! gross sein — der Writer zerlegt sie in MTU-sized DATA_FRAG-
//! Submessages. Dieser Bench misst `ReliableWriter::write(payload)`
//! fuer Payloads die **weit** ueber MTU liegen, damit die
//! Fragmentation-Kosten sichtbar werden.
//!
//! Payload-Achse: 32 B (below-MTU, sanity) → 4 MiB (kamerabild-
//! groesse, ~3100 Fragments @ 1344 B).
//!
//! # Was gemessen wird
//!
//! - Per-Sample-Zeit fuer `write()` inklusive:
//!   - Arc-Payload-Build (Zero-Copy-Pfad),
//!   - HistoryCache-Insert,
//!   - Fragmentation in N DATA_FRAG-Submessages (N = ceil(size / 1344)),
//!   - Datagramm-Build je Fragment.
//! - **Kein Netzwerk-send**: der Bench ruft nur `writer.write()` und
//!   verwirft den Datagramm-Vec. Transport-Time ist separat in
//!   `transports_e2e.rs`.
//!
//! # Warum nicht Transport-E2E?
//!
//! Ein kompletter "Writer → Transport → Reader → Assembler"-Pfad
//! ist Sache eines Live-Interop-Harness (echte zwei Hosts mit
//! Timing). Dies ist der *Protokoll*-Bench fuer Fragmentation
//! isoliert.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

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
        // Criterion-throughput zaehlt die Sample-Bytes, nicht die
        // Gesamtzahl versendeter Frame-Bytes. Beides informativ.
        group.throughput(Throughput::Bytes(size as u64));

        // Label inkludiert die Fragment-Anzahl, damit der Report
        // direkt lesbar ist.
        let label = if n_fragments <= 1 {
            format!("{} (1 frag)", size_label(size))
        } else {
            format!("{} ({} frags)", size_label(size), n_fragments)
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &payload,
            |b, payload| {
                // Writer pro-Iter neu aufsetzen, damit der history-cache
                // nicht voll laeuft und der Bench deterministisch ist.
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
