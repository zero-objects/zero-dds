// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-opcua-uacp` with `zerodds-opcua-pubsub`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-opcua-pubsub`: the binary cursor pair
//! `UaWriter`/`UaReader` for every UACP and SecureChannel field
//! (`connection.rs`, `securechannel.rs`, `crypto.rs`), the error types
//! `DecodeError`/`EncodeError`, the `UaEncode`/`UaDecode` traits, and
//! `uadp::datatypes::MessageSecurityMode` for the `securityMode` of an
//! OpenSecureChannel request.
//!
//! So the crate relies on: integers and floats written and read
//! little-endian in their IEEE 754 form; a read past the end being an error,
//! not a short value; the security modes being the numbers of the service
//! definition and an unknown number being refused.
//!
//! Oracles: OPC 10000-6 v1.05.07 §5.2.2.2 Figure 2 (1 000 000 000 as
//! `00 CA 9A 3B`), §5.2.2.3 Figure 3 (-6.5 as the Float `00 00 D0 C0`);
//! OPC 10000-4 v1.05.07 §7.20 Table 139 (MessageSecurityMode values).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_opcua_pubsub::uadp::datatypes::MessageSecurityMode;
use zerodds_opcua_pubsub::{UaReader, UaWriter};

#[test]
fn uacs_5_2_2_2_the_integer_of_figure_2_is_little_endian() {
    let mut w = UaWriter::new();
    w.write_u32(1_000_000_000);
    w.write_i32(1_000_000_000);
    assert_eq!(
        w.as_slice(),
        [0x00, 0xca, 0x9a, 0x3b, 0x00, 0xca, 0x9a, 0x3b]
    );
    let mut r = UaReader::new(w.as_slice());
    assert_eq!(r.read_u32().unwrap(), 1_000_000_000);
    assert_eq!(r.read_i32().unwrap(), 1_000_000_000);
    assert!(r.is_empty());
}

#[test]
fn uacs_5_2_2_3_the_float_of_figure_3_is_little_endian_ieee_754() {
    let mut w = UaWriter::new();
    w.write_f32(-6.5);
    assert_eq!(w.as_slice(), [0x00, 0x00, 0xd0, 0xc0]);
    let mut r = UaReader::new(&[0x00, 0x00, 0xd0, 0xc0]);
    assert_eq!(r.read_f32().unwrap().to_bits(), (-6.5f32).to_bits());
}

#[test]
fn uacs_5_2_2_2_a_read_past_the_end_is_an_error() {
    let mut r = UaReader::new(&[0x01, 0x02, 0x03]);
    assert!(r.read_u32().is_err());
}

#[test]
fn uacs_7_20_the_message_security_modes_are_the_values_of_table_139() {
    for (mode, value) in [
        (MessageSecurityMode::Invalid, 0),
        (MessageSecurityMode::None, 1),
        (MessageSecurityMode::Sign, 2),
        (MessageSecurityMode::SignAndEncrypt, 3),
    ] {
        assert_eq!(mode.to_i32(), value);
        assert_eq!(MessageSecurityMode::from_i32(value).unwrap(), mode);
    }
    assert!(MessageSecurityMode::from_i32(4).is_err());
}
