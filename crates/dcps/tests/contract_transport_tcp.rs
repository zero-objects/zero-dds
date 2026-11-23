// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-transport-tcp`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-transport-tcp`, all in `runtime.rs`:
//! `TcpTransport::bind_v4`, `bind_v6` and `bind_v6_only` for the user
//! unicast leg (`select_user_transport`, `transport.user_transport =
//! TCPv4/TCPv6`, `transport.tcp_port`),
//! `TcpTransport::with_locator_kinds` with the `TcpKindSet` of the runtime
//! (`transport.tcp_locator_kinds`), `with_dialect`,
//! `with_accepted_dialects`, `with_accepted_vendors` and
//! `with_logical_port_claim` on the transport just bound,
//! `TcpDialect::from_name` for the names of `transport.tcp_dialect`, the
//! `Arc<TcpTransport>` coerced to `Arc<dyn Transport + Send + Sync>` and
//! moved into the `zdds-tcp-accept` thread that runs `serve`, and `close`
//! through the trait at shutdown. The receive path hands every `recv` to
//! `zerodds_rtps::datagram::decode_datagram`, so it relies on the stream
//! framing giving back exactly one whole RTPS message per frame: never a
//! part of one, never two run together, never a truncated one.
//!
//! Oracles: ZeroDDS TCP Transport 1.0 §2.1 (a four-octet big-endian length
//! in front of each message, an empty body allowed, frames back to back),
//! §2.2 (reassembly across reads), §2.3 (an announced length above 16 MiB
//! is refused before anything is allocated), §2.4 (EOF inside a frame is
//! refused), `docs/spec-coverage/zerodds-tcp-transport-1.0.md`; DDSI-RTPS
//! 2.5 §8.3.3 and §9.4.5.1 for the hand-built messages. The framing is
//! exercised on in-memory streams; no socket is opened, so `bind_v4`,
//! `bind_v6`, `serve` and the trait's `send`/`recv` are left to the
//! integration pillar. `with_locator_kinds` is held by its signature; what
//! it does to the announced locator is `zerodds-tcp-transport-1.0` §1.1,
//! tested in `runtime.rs`.
#![allow(clippy::unwrap_used)]

use std::io::{Cursor, Read};

use zerodds_rtps::wire_types::TcpKindSet;
use zerodds_transport::Transport;
use zerodds_transport_tcp::framing::{
    FRAME_HEADER_LEN, FramingError, MAX_FRAME_SIZE, read_frame, write_frame,
};
use zerodds_transport_tcp::{TcpTransport, TcpTransportError};

/// DDSI-RTPS 2.5 §8.3.3: `RTPS`, version 2.5, vendor 0x01F0, a prefix.
fn rtps_header() -> Vec<u8> {
    let mut out = b"RTPS".to_vec();
    out.extend_from_slice(&[2, 5, 0x01, 0xF0]);
    out.extend_from_slice(&[0x01, 0xF0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    out
}

/// The header and one PAD submessage (§9.4.5.1: id 0x01, E flag, no body).
fn rtps_message_with_pad() -> Vec<u8> {
    let mut out = rtps_header();
    out.extend_from_slice(&[0x01, 0x01, 0x00, 0x00]);
    out
}

/// Hands out at most three octets per `read`, as a TCP stream may.
struct Trickle<'a> {
    bytes: &'a [u8],
}

impl Read for Trickle<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(3).min(self.bytes.len());
        buf[..n].copy_from_slice(&self.bytes[..n]);
        self.bytes = &self.bytes[n..];
        Ok(n)
    }
}

#[test]
fn each_rtps_message_is_one_frame_behind_a_big_endian_length() {
    let first = rtps_header();
    let second = rtps_message_with_pad();
    let mut stream = Vec::new();
    write_frame(&mut stream, &first).unwrap();
    write_frame(&mut stream, &[]).unwrap();
    write_frame(&mut stream, &second).unwrap();

    // §2.1, written out: 20 = 0x14, 0, 24 = 0x18, each in four octets
    // big-endian, the bodies behind them unchanged.
    assert_eq!(FRAME_HEADER_LEN, 4);
    let mut expected = vec![0, 0, 0, 0x14];
    expected.extend_from_slice(&first);
    expected.extend_from_slice(&[0, 0, 0, 0]);
    expected.extend_from_slice(&[0, 0, 0, 0x18]);
    expected.extend_from_slice(&second);
    assert_eq!(stream, expected);

    // Read back one message per frame, in order.
    let mut cursor = Cursor::new(&stream);
    assert_eq!(read_frame(&mut cursor).unwrap(), first);
    assert!(read_frame(&mut cursor).unwrap().is_empty());
    assert_eq!(read_frame(&mut cursor).unwrap(), second);
    // EOF on the frame boundary: the end of the connection, not a message
    // and not a cut frame (§2.4).
    assert!(matches!(read_frame(&mut cursor), Err(FramingError::Closed)));
}

#[test]
fn a_message_spread_over_many_reads_comes_up_whole() {
    let message = rtps_message_with_pad();
    let mut stream = Vec::new();
    write_frame(&mut stream, &message).unwrap();
    write_frame(&mut stream, &message).unwrap();
    // The length field itself arrives split (3 + 1 octets).
    let mut trickle = Trickle { bytes: &stream };
    assert_eq!(read_frame(&mut trickle).unwrap(), message);
    assert_eq!(read_frame(&mut trickle).unwrap(), message);
}

#[test]
fn a_truncated_or_oversized_frame_never_surfaces_as_a_message() {
    // §2.4: the length announces 24 octets, ten arrive.
    let mut cut = vec![0, 0, 0, 0x18];
    cut.extend_from_slice(&rtps_message_with_pad()[..10]);
    assert!(matches!(
        read_frame(&mut Cursor::new(&cut)),
        Err(FramingError::UnexpectedEof)
    ));
    // EOF inside the length field.
    assert!(matches!(
        read_frame(&mut Cursor::new(&[0u8, 0][..])),
        Err(FramingError::UnexpectedEof)
    ));
    // §2.3: 16 MiB + 1 announced; refused on the length alone.
    assert_eq!(MAX_FRAME_SIZE, 16 * 1024 * 1024);
    let over = (MAX_FRAME_SIZE + 1).to_be_bytes();
    assert!(matches!(
        read_frame(&mut Cursor::new(&over[..])),
        Err(FramingError::FrameTooLarge {
            announced: 0x0100_0001
        })
    ));
}

#[test]
fn the_transport_goes_behind_the_shared_transport_trait() {
    // `Arc<TcpTransport>` is coerced to `Arc<dyn Transport + Send + Sync>`
    // and a clone of it is moved into the accept thread.
    fn behind_dyn<T: Transport + Send + Sync + 'static>() {}
    behind_dyn::<TcpTransport>();
}

#[test]
fn the_kind_set_is_given_to_the_bound_transport_by_value() {
    // `select_user_transport` chains it onto the result of `bind_v4` and
    // `bind_v6`, before the transport goes into its `Arc`: it takes the
    // transport and the kind set by value and gives the transport back.
    // A changed signature does not compile.
    let _: fn(TcpTransport, TcpKindSet) -> TcpTransport = TcpTransport::with_locator_kinds;
}

#[test]
fn the_tcp_settings_are_given_to_the_bound_transport_by_value() {
    // `RuntimeConfig::apply_tcp_settings` chains the vendor list
    // (`transport.tcp_accepted_vendors`) and the logical-port claim
    // (`transport.tcp_logical_port_claim`) onto the transport the same
    // way. A changed signature does not compile.
    let _: fn(TcpTransport, Vec<[u8; 2]>) -> TcpTransport = TcpTransport::with_accepted_vendors;
    let _: fn(TcpTransport, bool) -> TcpTransport = TcpTransport::with_logical_port_claim;
    // The dialect (`transport.tcp_dialect`) as well, and the dialects
    // the listener takes (`transport.tcp_accept_dialects`).
    let _: fn(TcpTransport, zerodds_transport_tcp::TcpDialect) -> TcpTransport =
        TcpTransport::with_dialect;
    let _: fn(TcpTransport, &[zerodds_transport_tcp::TcpDialect]) -> TcpTransport =
        TcpTransport::with_accepted_dialects;
    // `parse_tcp_dialect` reads the names of `transport.tcp_dialect` from
    // the transport.
    let _: fn(&str) -> Option<zerodds_transport_tcp::TcpDialect> =
        zerodds_transport_tcp::TcpDialect::from_name;
}

#[test]
fn a_tcp_listener_binds_the_port_of_the_setting() {
    // `select_user_transport` hands `transport.tcp_port` to `bind_v4` and
    // `bind_v6`, and to `bind_v6_only` when a TCPv4 listener of the same
    // runtime holds the port. A changed signature does not compile.
    let _: fn(std::net::Ipv4Addr, u16) -> Result<TcpTransport, TcpTransportError> =
        TcpTransport::bind_v4;
    let _: fn(std::net::Ipv6Addr, u16) -> Result<TcpTransport, TcpTransportError> =
        TcpTransport::bind_v6;
    let _: fn(std::net::Ipv6Addr, u16) -> Result<TcpTransport, TcpTransportError> =
        TcpTransport::bind_v6_only;
}
