// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-endpoint-e2e` with `zerodds-wire`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire` (`src/lib.rs`): its public
//! `xcdr2_writer()` and `xcdr2_reader()` return a `Writer` and a `Reader`
//! built with `with_max_align(.., Endianness::Little, 4)`; `ping_pong` and
//! `serve_ping_pong` read the `@final struct Ping { long seq; string msg; }`
//! with `read_u32` and `read_string`, write the `Pong` with `write_u32`,
//! `write_string` and `finish`, and put a `WireError` into their error
//! text with `Display`. No socket is opened here.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.1.1 Table 31 (Int32), §7.4.1.1.2 (a string
//! is a UInt32 length that counts the NUL, the octets, the NUL), §7.4.2
//! (XCDR2 MAXALIGN 4).
#![allow(clippy::unwrap_used)]

use zerodds_endpoint_e2e::{PING_MSG, PING_SEQ, xcdr2_reader, xcdr2_writer};
use zerodds_wire::{Endianness, WireError};

/// `Ping { seq: 1, msg: "hello from app" }`: 14 characters, length 15.
const PING: [u8; 23] = [
    0x01, 0x00, 0x00, 0x00, // seq
    0x0f, 0x00, 0x00, 0x00, // length, NUL counted
    b'h', b'e', b'l', b'l', b'o', b' ', b'f', b'r', b'o', b'm', b' ', b'a', b'p', b'p', 0x00,
];

#[test]
fn xtypes_7_4_1_1_2_ping_body_octets() {
    let mut w = xcdr2_writer();
    assert_eq!(w.max_align(), 4);
    assert_eq!(w.endianness(), Endianness::Little);
    w.write_u32(PING_SEQ);
    w.write_string(PING_MSG);
    assert_eq!(w.finish(), PING);
}

#[test]
fn xtypes_7_4_1_1_2_ping_body_reads_back_from_the_clause_octets() {
    let mut r = xcdr2_reader(&PING);
    assert_eq!(r.max_align(), 4);
    assert_eq!(r.endianness(), Endianness::Little);
    assert_eq!(r.read_u32().unwrap(), PING_SEQ);
    assert_eq!(r.read_string().unwrap(), PING_MSG);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn a_cut_ping_is_an_error_with_a_reason_not_a_value() {
    // `serve_ping_pong` returns `format!("ping msg: {e}")`.
    let mut r = xcdr2_reader(&PING[..12]);
    assert_eq!(r.read_u32().unwrap(), PING_SEQ);
    let error = r.read_string().unwrap_err();
    assert!(matches!(error, WireError::ImpossibleLength { at: 4, .. }));
    assert!(!error.to_string().is_empty());
}
