// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-mqtt-bridge` with `zerodds-websocket-bridge`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-websocket-bridge` (`src/daemon/
//! ws_transport.rs`, feature `daemon`, the `ws://` and `wss://` broker
//! schemes of MQTT 5.0 §6): `handshake::{client_key, render_client_request,
//! validate_server_response}` for the opening handshake with the subprotocol
//! `mqtt`, `codec::{encode, decode}` and `frame::{Frame, Opcode}` for the
//! binary frames that carry the MQTT packets, every frame it sends masked.
//!
//! So the crate relies on: the request carrying the key and the offered
//! subprotocol; a response being accepted only with status 101, the accept
//! value of the key and the subprotocol selected; a masked frame being the
//! octets RFC 6455 gives, and a decoded frame being unmasked.
//!
//! Oracles: RFC 6455 §1.3 (the accept value of the sample nonce), §4.1 and
//! §4.2.2 (the handshake), §5.2, §5.3 and §5.7 (the example frames "Hello",
//! unmasked and masked with `37 fa 21 3d`).
#![allow(clippy::unwrap_used)]

use zerodds_websocket_bridge::codec::{decode, encode};
use zerodds_websocket_bridge::frame::{Frame, Opcode};
use zerodds_websocket_bridge::handshake::{
    client_key, render_client_request, validate_server_response,
};

/// RFC 6455 §1.3: the sample nonce and the accept value the server answers.
const NONCE_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";
const ACCEPT: &str = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";

#[test]
fn ws_4_1_the_request_carries_the_key_and_the_offered_subprotocol() {
    // "the sample nonce" is these sixteen octets.
    let key = client_key(b"the sample nonce");
    assert_eq!(key, NONCE_KEY);
    let request = render_client_request("broker:8080", "/mqtt", &key, &["mqtt"]);
    assert!(request.starts_with("GET /mqtt HTTP/1.1\r\n"), "{request}");
    for header in [
        "Host: broker:8080\r\n",
        "Upgrade: websocket\r\n",
        "Connection: Upgrade\r\n",
        "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n",
        "Sec-WebSocket-Version: 13\r\n",
        "Sec-WebSocket-Protocol: mqtt\r\n",
    ] {
        assert!(request.contains(header), "{header:?} in {request}");
    }
    assert!(request.ends_with("\r\n\r\n"));
}

#[test]
fn ws_4_1_a_response_is_accepted_with_101_the_accept_value_and_the_subprotocol() {
    let ok = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Accept: {ACCEPT}\r\nSec-WebSocket-Protocol: mqtt\r\n\r\n"
    );
    assert_eq!(
        validate_server_response(&ok, NONCE_KEY, &["mqtt"], true).unwrap(),
        Some("mqtt".to_string())
    );
    let wrong_accept = ok.replace(ACCEPT, "AAAAAAAAAAAAAAAAAAAAAAAAAAA=");
    assert!(validate_server_response(&wrong_accept, NONCE_KEY, &["mqtt"], true).is_err());
    let no_protocol = ok.replace("Sec-WebSocket-Protocol: mqtt\r\n", "");
    assert!(validate_server_response(&no_protocol, NONCE_KEY, &["mqtt"], true).is_err());
    let not_101 = ok.replace("101 Switching Protocols", "200 OK");
    assert!(validate_server_response(&not_101, NONCE_KEY, &["mqtt"], true).is_err());
}

#[test]
fn ws_5_7_a_masked_frame_is_the_octets_of_the_example_and_decodes_unmasked() {
    let mut hello = Frame::text("Hello");
    assert_eq!(
        encode(&hello).unwrap(),
        [0x81, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f]
    );
    hello.masking_key = Some([0x37, 0xfa, 0x21, 0x3d]);
    let masked = encode(&hello).unwrap();
    assert_eq!(
        masked,
        [
            0x81, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58
        ]
    );
    let (frame, used) = decode(&masked).unwrap();
    assert_eq!(used, masked.len());
    assert_eq!(frame.opcode, Opcode::Text);
    assert_eq!(frame.payload, b"Hello");
    let binary = Frame::binary(vec![0x10, 0x00]);
    assert_eq!(encode(&binary).unwrap(), [0x82, 0x02, 0x10, 0x00]);
}
