//! Criterion-Benches für RTPS-Decoder-Hot-Paths.
//!
//! Regression-Detection. Misst:
//! * `decode_datagram` mit DATA / HEARTBEAT / mehrere Submessages
//! * `SequenceNumberSet::read_from` (bei DATA-fragments häufig)
//! * Submessage-Decoding einzeln (DataSubmessage::read_body etc.)
//!
//! Baseline:
//!   `cargo bench -p zerodds-rtps --bench decode_hotpaths -- --save-baseline pre`

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use core::time::Duration;

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_rtps::datagram::decode_datagram;
use zerodds_rtps::submessages::{
    AckNackSubmessage, DataSubmessage, FragmentNumberSet, HeartbeatSubmessage, SequenceNumberSet,
};
use zerodds_rtps::wire_types::{FragmentNumber, SequenceNumber};

/// Minimaler RTPS-DATA-Datagramm (32 byte Header + 24 byte DATA-body
/// mit kleiner Payload). Wir bauen die Bytes zur Compile-Zeit nicht
/// (zu komplex), sondern dekodieren ein bekannt-gutes Cyclone-Fixture.
/// Fixtures committet unter `benches/fixtures/` (statt unter
/// `fuzz/corpus/`, das per crate-`.gitignore` exkludiert ist).
const CYCLONE_DATA_FIXTURE: &[u8] = include_bytes!("fixtures/cyclone_data_with_cdr2_payload");

const CYCLONE_HEARTBEAT_FIXTURE: &[u8] = include_bytes!("fixtures/cyclone_heartbeat");

fn bench_decode_datagram_data(c: &mut Criterion) {
    let mut group = c.benchmark_group("rtps_decode_datagram_data");
    group.throughput(Throughput::Bytes(CYCLONE_DATA_FIXTURE.len() as u64));
    group.bench_function("cyclone_data_with_cdr2", |b| {
        b.iter(|| {
            let _ = decode_datagram(black_box(CYCLONE_DATA_FIXTURE));
        });
    });
    group.finish();
}

fn bench_decode_datagram_heartbeat(c: &mut Criterion) {
    let mut group = c.benchmark_group("rtps_decode_datagram_heartbeat");
    group.throughput(Throughput::Bytes(CYCLONE_HEARTBEAT_FIXTURE.len() as u64));
    group.bench_function("cyclone_heartbeat", |b| {
        b.iter(|| {
            let _ = decode_datagram(black_box(CYCLONE_HEARTBEAT_FIXTURE));
        });
    });
    group.finish();
}

fn bench_seqnum_set_roundtrip(c: &mut Criterion) {
    // Realistic SeqnumSet: base=1000, missing 5 SNs across 32-bit window.
    let base = SequenceNumber(1000);
    let missing = vec![
        SequenceNumber(1003),
        SequenceNumber(1010),
        SequenceNumber(1025),
        SequenceNumber(1031),
    ];
    let set = SequenceNumberSet::from_missing(base, &missing);
    let mut bytes = Vec::new();
    set.write_to(&mut bytes, true);

    let mut group = c.benchmark_group("rtps_seqnum_set");
    group.bench_function("decode_4_missing_sns", |b| {
        b.iter(|| {
            let _ = SequenceNumberSet::read_from(black_box(&bytes), 0, true);
        });
    });
    group.finish();
}

fn bench_fragment_set_roundtrip(c: &mut Criterion) {
    let missing: Vec<FragmentNumber> = (1..=8).map(FragmentNumber).collect();
    let set = FragmentNumberSet::from_missing(FragmentNumber(1), &missing);
    let mut bytes = Vec::new();
    set.write_to(&mut bytes, true);

    c.bench_function("rtps_fragnum_set_decode_8_missing", |b| {
        b.iter(|| {
            let _ = FragmentNumberSet::read_from(black_box(&bytes), 0, true);
        });
    });
}

fn bench_submessage_read_data_minimal(c: &mut Criterion) {
    // Minimal DATA-Submessage body (without RTPS header, without
    // SubmessageHeader). 24-Byte: Reader/Writer EID (8B), SeqNum (8B),
    // 4B inline_qos placeholder, no payload.
    // We build a real one through encoded round-trip first.
    let body = [
        0u8, 0, 16, 0, // extra flags, octets to inline_qos
        0xC1, 0x02, 0x03, 0x04, // reader EID
        0xC2, 0x02, 0x03, 0x04, // writer EID
        0x00, 0x00, 0x00, 0x00, // seqnum hi
        0x01, 0x00, 0x00, 0x00, // seqnum lo (= 1)
    ];
    c.bench_function("rtps_data_submessage_read_body", |b| {
        b.iter(|| {
            let _ = DataSubmessage::read_body(black_box(&body), true);
        });
    });
}

fn bench_submessage_read_heartbeat_minimal(c: &mut Criterion) {
    // 28-byte HEARTBEAT body: reader EID, writer EID, first SN (8B),
    // last SN (8B), count (4B).
    let body = [
        0xC1, 0x02, 0x03, 0x04, // reader EID
        0xC2, 0x02, 0x03, 0x04, // writer EID
        0x00, 0x00, 0x00, 0x00, // first SN hi
        0x01, 0x00, 0x00, 0x00, // first SN lo
        0x00, 0x00, 0x00, 0x00, // last SN hi
        0x10, 0x00, 0x00, 0x00, // last SN lo
        0x05, 0x00, 0x00, 0x00, // count
    ];
    c.bench_function("rtps_heartbeat_submessage_read_body", |b| {
        b.iter(|| {
            let _ = HeartbeatSubmessage::read_body(black_box(&body), true, false, false, false);
        });
    });
}

fn bench_submessage_read_acknack_minimal(c: &mut Criterion) {
    // ACKNACK body: reader EID, writer EID, SeqnumSet (variable),
    // count (4B).
    let mut body = vec![
        0xC1, 0x02, 0x03, 0x04, // reader EID
        0xC2, 0x02, 0x03, 0x04, // writer EID
    ];
    let set = SequenceNumberSet::from_missing(
        SequenceNumber(100),
        &[SequenceNumber(102), SequenceNumber(105)],
    );
    set.write_to(&mut body, true);
    body.extend_from_slice(&3u32.to_le_bytes()); // count

    c.bench_function("rtps_acknack_submessage_read_body", |b| {
        b.iter(|| {
            let _ = AckNackSubmessage::read_body(black_box(&body), true, false);
        });
    });
}

fn config() -> Criterion {
    Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(50)
}

criterion_group! {
    name = benches;
    config = config();
    targets =
        bench_decode_datagram_data,
        bench_decode_datagram_heartbeat,
        bench_seqnum_set_roundtrip,
        bench_fragment_set_roundtrip,
        bench_submessage_read_data_minimal,
        bench_submessage_read_heartbeat_minimal,
        bench_submessage_read_acknack_minimal,
}
criterion_main!(benches);
