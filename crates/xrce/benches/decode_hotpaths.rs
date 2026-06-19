//! Criterion benches for the DDS-XRCE wire decoder hot paths.
//!
//! Measures:
//! * `Message::decode` with single + multi-submessage frames
//! * `SubmessageHeader::from_bytes`
//! * Serial frame decoders (SLIP-encoded)

#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use zerodds_xrce::transport_serial::{decode_frame, decode_frame_inner};
use zerodds_xrce::{
    Message, MessageHeader, SerialNumber16, SessionId, StreamId, Submessage, SubmessageHeader,
    SubmessageId,
};

fn build_single_msg() -> Vec<u8> {
    let header =
        MessageHeader::without_client_key(SessionId(0xC8), StreamId(0x80), SerialNumber16::new(1))
            .unwrap();
    let sub = Submessage::new(SubmessageId::Heartbeat, 0x01, vec![0; 8]).unwrap();
    Message::new(header, vec![sub]).unwrap().encode().unwrap()
}

fn build_multi_msg() -> Vec<u8> {
    let header =
        MessageHeader::without_client_key(SessionId(0xC8), StreamId(0x80), SerialNumber16::new(42))
            .unwrap();
    let subs = vec![
        Submessage::new(SubmessageId::Heartbeat, 0x01, vec![0xAA; 8]).unwrap(),
        Submessage::new(SubmessageId::AckNack, 0x01, vec![0xBB; 12]).unwrap(),
        Submessage::new(SubmessageId::Timestamp, 0x01, vec![0xCC; 8]).unwrap(),
    ];
    Message::new(header, subs).unwrap().encode().unwrap()
}

fn bench_decode_single(c: &mut Criterion) {
    let bytes = build_single_msg();
    let mut group = c.benchmark_group("xrce_decode_single_msg");
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("heartbeat", |b| {
        b.iter(|| {
            let _ = Message::decode(black_box(&bytes));
        });
    });
    group.finish();
}

fn bench_decode_multi(c: &mut Criterion) {
    let bytes = build_multi_msg();
    let mut group = c.benchmark_group("xrce_decode_multi_msg");
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("hb_acknack_ts", |b| {
        b.iter(|| {
            let _ = Message::decode(black_box(&bytes));
        });
    });
    group.finish();
}

fn bench_submessage_header(c: &mut Criterion) {
    // 4-byte SubmessageHeader: [id, flags, length-le-2B]
    let bytes = [SubmessageId::Heartbeat as u8, 0x01, 0x10, 0x00];
    c.bench_function("xrce_submessage_header_from_bytes", |b| {
        b.iter(|| {
            let _ = SubmessageHeader::from_bytes(black_box(&bytes));
        });
    });
}

fn bench_serial_frame_inner(c: &mut Criterion) {
    // Synthetisches SLIP-encoded Frame: end + payload + end.
    // Payload = ein minimal-Message-Frame.
    let inner = build_single_msg();
    let mut framed = vec![0xC0]; // SLIP END
    framed.extend_from_slice(&inner);
    framed.push(0xC0);
    c.bench_function("xrce_serial_decode_frame_inner", |b| {
        b.iter(|| {
            let _ = decode_frame_inner(black_box(&framed));
        });
    });
}

fn bench_serial_decode_frame(c: &mut Criterion) {
    let inner = build_single_msg();
    let mut framed = vec![0xC0];
    framed.extend_from_slice(&inner);
    framed.push(0xC0);
    c.bench_function("xrce_serial_decode_frame", |b| {
        b.iter(|| {
            let _ = decode_frame(black_box(&framed));
        });
    });
}

criterion_group!(
    benches,
    bench_decode_single,
    bench_decode_multi,
    bench_submessage_header,
    bench_serial_frame_inner,
    bench_serial_decode_frame,
);
criterion_main!(benches);
