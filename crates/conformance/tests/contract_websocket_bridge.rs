// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-conformance` with `zerodds-websocket-bridge`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-websocket-bridge` (`src/autobahn_ws.rs`,
//! the RFC 6455 and RFC 7692 vectors): `compute_accept` and
//! `handshake::WEBSOCKET_GUID`, `ClosePayload`, `CloseCode`,
//! `encode_close_payload` and `decode_close_payload`, the permessage-deflate
//! negotiation (`parse_offer`, `render_accept`, `PermessageDeflateParams`,
//! `NegotiationError`) and the tail helpers (`append_tail`, `strip_tail`).
//!
//! Oracles: RFC 6455 §1.3 (the GUID and the accept value of the sample
//! nonce), §5.5.1 (a close reason of at most 123 octets), §7.4.1 and §7.4.2
//! (1000 is a normal closure; 1005, 1006 and 1015 never appear on the wire);
//! RFC 7692 §7.1 (window bits 8 to 15, a boolean parameter without a value).
#![allow(clippy::unwrap_used)]

use zerodds_websocket_bridge::handshake::WEBSOCKET_GUID;
use zerodds_websocket_bridge::{
    CloseCode, ClosePayload, NegotiationError, compute_accept, decode_close_payload,
    encode_close_payload, parse_offer,
};

#[test]
fn ws_1_3_the_accept_value_of_the_sample_nonce() {
    assert_eq!(WEBSOCKET_GUID, "258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    assert_eq!(
        compute_accept("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

#[test]
fn ws_7_4_1_a_normal_closure_is_1000_big_endian_and_reserved_codes_are_refused() {
    let normal = ClosePayload {
        code: CloseCode::Normal,
        reason: "bye".into(),
    };
    let bytes = encode_close_payload(&normal);
    assert_eq!(bytes, [0x03, 0xe8, b'b', b'y', b'e']);
    assert_eq!(decode_close_payload(&bytes), Ok(normal));
    for code in [1005u16, 1006, 1015] {
        assert!(decode_close_payload(&code.to_be_bytes()).is_err(), "{code}");
    }
    let mut long = vec![0x03, 0xe8];
    long.extend(std::iter::repeat_n(b'a', 124));
    assert!(
        decode_close_payload(&long).is_err(),
        "a reason over 123 octets"
    );
}

#[test]
fn rfc7692_7_1_window_bits_are_8_to_15_and_a_boolean_has_no_value() {
    for bits in [7, 16, 0] {
        assert!(
            parse_offer(&format!(
                "permessage-deflate; server_max_window_bits={bits}"
            ))
            .is_err(),
            "{bits}"
        );
    }
    assert!(parse_offer("permessage-deflate; server_max_window_bits=15").is_ok());
    assert!(matches!(
        parse_offer("permessage-deflate; server_no_context_takeover=yes"),
        Err(NegotiationError::BooleanWithValue(_))
    ));
}
