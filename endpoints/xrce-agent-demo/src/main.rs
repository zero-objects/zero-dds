// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Example XRCE hub side for the native endpoint SDKs (ADR 0013).
//!
//! ```text
//! recv <port> [count]   -- bind UDP, receive WRITE_DATA from endpoints,
//!                          decode the SensorReading, print (the agent side).
//! send <host> <port>    -- send a DATA message (a sample the hub pushes)
//!                          to a native receiver endpoint.
//! ```
//!
//! Both use the real zerodds-xrce, so a native endpoint's frames are proven
//! against real Rust XRCE over a socket, in both directions.
#![allow(clippy::print_stdout, clippy::expect_used)]

use std::net::UdpSocket;

use zerodds_wire::{Endianness, Reader, Writer};
use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
use zerodds_xrce::object_kind::ObjectKind;
use zerodds_xrce::submessages::{DataPayload, Message, SubmessageId, WriteDataPayload};
use zerodds_xrce::{BaseObjectRequest, ObjectId, SerialNumber16};

/// The DataReader the hub reads from / pushes DATA for (§7.2.1 ObjectId).
fn reader_object_id() -> ObjectId {
    ObjectId::new(0x001, ObjectKind::DataReader).expect("reader id")
}

/// The fixed SensorReading sample body (XCDR2, LE).
///
/// XCDR2 caps alignment at four, so the writer is built with that bound
/// rather than with `Writer::new`, which caps at eight: the `uint64`
/// member lands on a four-boundary here and on an eight-boundary there.
fn sample_body() -> Vec<u8> {
    let mut w = Writer::with_max_align(Endianness::Little, 4);
    w.write_u32(0xA1B2_C3D4);
    w.write_u16(0x1234);
    w.write_u8(0x5A);
    w.write_u32(3.5f32.to_bits());
    w.write_u64(0x0102_0304_0506_0708);
    w.write_string("bay-12");
    w.write_u32(4);
    w.write_bytes(&[0xDE, 0xAD, 0xBE, 0xEF]);
    w.finish()
}

fn recv(port: u16, count: usize) {
    let sock = UdpSocket::bind(("0.0.0.0", port)).expect("bind");
    sock.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("read timeout");
    println!("hub: listening on udp/{port} for {count} sample(s)");
    let mut buf = [0u8; 2048];
    for _ in 0..count {
        let (n, from) = sock.recv_from(&mut buf).expect("recv");
        let msg = Message::decode(&buf[..n]).expect("decode");
        let sm = msg
            .submessages
            .iter()
            .find(|s| s.header.submessage_id == SubmessageId::WriteData)
            .expect("WRITE_DATA");
        // Conformant DDS-XRCE WRITE_DATA: the BaseObjectRequest (request_id +
        // object_id) precedes the sample, so the hub can map the frame to the
        // target DataWriter before touching the payload (§8.3.5.8 / §7.7.8).
        let wd = WriteDataPayload::try_from_submessage(sm).expect("wd");
        let target = wd.base.object_id;
        let mut r = Reader::with_max_align(&wd.serialized_data, Endianness::Little, 4);
        let id = r.read_u32().expect("id");
        assert_eq!(r.read_u16().expect("kind"), 0x1234);
        assert_eq!(r.read_u8().expect("flags"), 0x5A);
        assert_eq!(r.read_u32().expect("value"), 3.5f32.to_bits());
        assert_eq!(r.read_u64().expect("stamp"), 0x0102_0304_0506_0708);
        let label = r.read_string().expect("label");
        assert_eq!(id, 0xA1B2_C3D4);
        assert_eq!(label, "bay-12");
        assert_eq!(target.raw(), 0x0015);
        assert_eq!(r.read_u32().expect("raw length"), 4);
        assert_eq!(r.read_bytes(4).expect("raw"), &[0xDE, 0xAD, 0xBE, 0xEF]);
        println!(
            "AGENT OK: WRITE_DATA from {from} for object 0x{:04X} decoded (id=0x{id:08X} label={label})",
            target.raw()
        );
    }
    println!("hub: {count} sample(s) received");
}

fn send(host: &str, port: u16) {
    let header = MessageHeader::without_client_key(
        SessionId(0x80),
        StreamId::BUILTIN_BEST_EFFORT,
        SerialNumber16(1),
    )
    .expect("header");
    // Conformant DDS-XRCE DATA (agent -> client, §8.3.5.10): request_id echoes
    // the client's READ_DATA request, object_id identifies the source
    // DataReader — both precede the sample.
    let sm = DataPayload {
        base: BaseObjectRequest {
            request_id: [0x00, 0x01],
            object_id: reader_object_id(),
        },
        serialized_data: sample_body(),
    }
    .into_submessage()
    .expect("submessage");
    let frame = Message::new(header, std::vec![sm])
        .expect("msg")
        .encode()
        .expect("encode");
    let sock = UdpSocket::bind(("0.0.0.0", 0)).expect("bind");
    sock.send_to(&frame, (host, port)).expect("send");
    println!(
        "hub: sent {}-byte DATA message to {host}:{port}",
        frame.len()
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "recv".to_string());
    match mode.as_str() {
        "send" => {
            let host = args.next().unwrap_or_else(|| "127.0.0.1".to_string());
            let port = args.next().and_then(|s| s.parse().ok()).unwrap_or(7447);
            send(&host, port);
        }
        _ => {
            // `recv` -- accept "recv <port> [count]" or a bare "<port> [count]".
            let a = if mode == "recv" {
                args.next()
            } else {
                Some(mode)
            };
            let port = a.and_then(|s| s.parse().ok()).unwrap_or(7447);
            let count = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
            recv(port, count);
        }
    }
}
