// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-conformance` with `zerodds-grpc-bridge`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-grpc-bridge` (`src/h2spec_grpc.rs`, the
//! gRPC cases of the h2spec suite): `encode_message` and `decode_message`
//! (the Length-Prefixed-Message) and `parse_path` (`/<service>/<method>`).
//! It relies on the message layout and the path grammar being those of gRPC
//! over HTTP/2.
//!
//! Oracle: gRPC over HTTP/2 (`PROTOCOL-HTTP2.md`), §"Requests":
//! `Length-Prefixed-Message → Compressed-Flag Message-Length Message`, the
//! flag one octet, the length four octets big-endian; `Path → ":path" "/"
//! Service-Name "/" {method name}`.
#![allow(clippy::unwrap_used)]

use zerodds_grpc_bridge::{decode_message, encode_message, parse_path};

#[test]
fn grpc_requests_a_length_prefixed_message_is_flag_length_message() {
    let octets = [
        0x00, 0x00, 0x00, 0x00, 0x0b, b'h', b'e', b'l', b'l', b'o', b' ', b'w', b'o', b'r', b'l',
        b'd',
    ];
    assert_eq!(encode_message(b"hello world", false).unwrap(), octets);
    assert_eq!(
        decode_message(&octets).unwrap(),
        (0, b"hello world".to_vec(), octets.len())
    );
    assert_eq!(
        encode_message(b"x", true).unwrap(),
        [0x01, 0, 0, 0, 1, b'x']
    );
}

#[test]
fn grpc_requests_the_path_is_slash_service_slash_method() {
    assert_eq!(
        parse_path("/dds.demo.Trader/PlaceOrder").unwrap(),
        ("dds.demo.Trader".to_string(), "PlaceOrder".to_string())
    );
    assert!(
        parse_path("dds.demo.Trader/PlaceOrder").is_err(),
        "not absolute"
    );
    assert!(parse_path("/dds.demo.Trader").is_err(), "no method");
    assert!(
        parse_path("/dds.demo.Trader/a/b").is_err(),
        "a method is one segment"
    );
}
