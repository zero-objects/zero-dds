// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Example XRCE hub side for the native endpoint SDKs (ADR 0013).
//!
//!   recv <port> [count]   -- bind UDP, receive WRITE_DATA from endpoints,
//!                            decode the SensorReading, print (the agent side).
//!   send <host> <port>    -- send a DATA message (a sample the hub pushes)
//!                            to a native receiver endpoint.
//!
//! Both use the real zerodds-xrce, so a native endpoint's frames are proven
//! against real Rust XRCE over a socket, in both directions.
#![allow(clippy::print_stdout, clippy::expect_used)]

use std::net::UdpSocket;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_xrce::SerialNumber16;
use zerodds_xrce::header::{MessageHeader, SessionId, StreamId};
use zerodds_xrce::submessages::{DataFormat, DataPayload, Message, SubmessageId, WriteDataPayload};

/// The fixed SensorReading sample body (XCDR2, LE).
fn sample_body() -> Vec<u8> {
    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    w.write_u32(0xA1B2_C3D4).expect("write id");
    w.write_u16(0x1234).expect("write seq");
    w.write_u8(0x5A).expect("write flags");
    w.write_u32(3.5f32.to_bits()).expect("write value");
    w.write_u64(0x0102_0304_0506_0708).expect("write ts");
    w.write_string("bay-12").expect("write label");
    w.write_u32(4).expect("write blob len");
    w.write_bytes(&[0xDE, 0xAD, 0xBE, 0xEF])
        .expect("write blob");
    w.into_bytes()
}

fn recv(port: u16, count: usize) {
    let sock = UdpSocket::bind(("0.0.0.0", port)).expect("bind");
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
        let wd = WriteDataPayload::try_from_submessage(sm).expect("wd");
        let mut r = BufferReader::new(&wd.representation, Endianness::Little).xcdr2();
        let id = r.read_u32().expect("id");
        let _ = (r.read_u16(), r.read_u8(), r.read_u32(), r.read_u64());
        let label = r.read_string().expect("label");
        assert_eq!(id, 0xA1B2_C3D4);
        assert_eq!(label, "bay-12");
        println!("AGENT OK: WRITE_DATA from {from} decoded (id=0x{id:08X} label={label})");
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
    let sm = DataPayload {
        representation: sample_body(),
        data_format: DataFormat::Sample,
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
