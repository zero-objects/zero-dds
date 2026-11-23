// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-mqtt-bridge` with `zerodds-websocket-bridge`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-websocket-bridge`:
//!
//! * the client (`src/daemon/ws_transport.rs`, feature `daemon`, the
//!   `ws://` and `wss://` broker schemes of MQTT 5.0 §6):
//!   `handshake::{client_key, render_client_request,
//!   validate_server_response}` for the opening handshake with the
//!   subprotocol `mqtt`, `codec::{encode, decode}` and
//!   `frame::{Frame, Opcode}` for the binary frames that carry the MQTT
//!   packets, every frame it sends masked;
//! * the server (`src/server/websocket.rs`, feature `websocket`, the
//!   WebSocket listener of the broker server): `handshake::{
//!   parse_client_request, build_server_response, render_server_response,
//!   HandshakeError}` for the server's side of the opening handshake,
//!   `codec::peek_header` to read a frame's header before its payload,
//!   `close::{CloseCode, parse_close_body}` for the closing handshake.
//!
//! So the crate relies on: the request carrying the key and the offered
//! subprotocol; a response being accepted only with status 101, the accept
//! value of the key and the subprotocol selected; a client's request being
//! read with its offered subprotocols and refused with the HTTP status of
//! its fault; the server's response carrying the accept value and the
//! subprotocol it selects and nothing it did not select; a frame header
//! being read with its mask bit and its length in each of the three forms
//! before the payload is there; a masked frame being the octets RFC 6455
//! gives, and a decoded frame being unmasked.
//!
//! Oracles: RFC 6455 §1.2 and §1.3 (the sample handshake and the accept
//! value of the sample nonce), §4.1 and §4.2 (the handshake), §5.2, §5.3
//! and §5.7 (the example frames "Hello", unmasked and masked with
//! `37 fa 21 3d`, and the 64-bit length of a 64 KiB binary message),
//! §7.4.1 (the status codes).
#![allow(clippy::unwrap_used)]

use zerodds_websocket_bridge::close::{CloseCode, parse_close_body};
use zerodds_websocket_bridge::codec::{decode, encode, peek_header};
use zerodds_websocket_bridge::frame::{Frame, Opcode};
use zerodds_websocket_bridge::handshake::{
    build_server_response, client_key, parse_client_request, render_client_request,
    render_server_response, validate_server_response,
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

/// RFC 6455 §1.2: the client's handshake of the sample, with the
/// subprotocols `chat` and `superchat`.
const SAMPLE_REQUEST: &str = "GET /chat HTTP/1.1\r\nHost: server.example.com\r\n\
     Upgrade: websocket\r\nConnection: Upgrade\r\n\
     Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nOrigin: http://example.com\r\n\
     Sec-WebSocket-Protocol: chat, superchat\r\nSec-WebSocket-Version: 13\r\n\r\n";

#[test]
fn ws_4_2_1_a_request_is_read_with_its_offered_subprotocols_and_refused_with_its_status() {
    let req = parse_client_request(SAMPLE_REQUEST).unwrap();
    assert_eq!(req.key, NONCE_KEY);
    assert_eq!(req.path, "/chat");
    assert_eq!(req.protocols, ["chat", "superchat"]);
    for (request, status) in [
        (SAMPLE_REQUEST.replacen("GET", "POST", 1), 405),
        (SAMPLE_REQUEST.replace("Version: 13", "Version: 8"), 426),
        (SAMPLE_REQUEST.replace("Upgrade: websocket\r\n", ""), 400),
        (SAMPLE_REQUEST.replace(NONCE_KEY, "c2hvcnQ="), 400),
    ] {
        let e = parse_client_request(&request).unwrap_err();
        assert_eq!(e.http_status(), status, "{e}");
        assert!(
            e.render_error_response()
                .starts_with(&format!("HTTP/1.1 {status} ")),
            "{e}"
        );
    }
}

#[test]
fn ws_4_2_2_the_response_carries_the_accept_value_and_only_the_selected_subprotocol() {
    let req = parse_client_request(&SAMPLE_REQUEST.replace("chat, superchat", "mqtt")).unwrap();
    let mut resp = build_server_response(&req);
    assert_eq!(resp.status, 101);
    assert_eq!(resp.accept, ACCEPT);
    // Nothing is selected until the caller selects it.
    assert_eq!(resp.protocol, None);
    assert!(resp.extensions.is_empty());
    assert!(!render_server_response(&resp).contains("Sec-WebSocket-Protocol"));
    resp.protocol = Some("mqtt".to_string());
    let text = render_server_response(&resp);
    assert!(text.starts_with("HTTP/1.1 101 "), "{text}");
    for header in [
        "Upgrade: websocket\r\n",
        "Connection: Upgrade\r\n",
        "Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n",
        "Sec-WebSocket-Protocol: mqtt\r\n",
    ] {
        assert!(text.contains(header), "{header:?} in {text}");
    }
    assert!(!text.contains("Sec-WebSocket-Extensions"), "{text}");
    assert!(text.ends_with("\r\n\r\n"));
    // What the client of this crate accepts.
    assert_eq!(
        validate_server_response(&text, NONCE_KEY, &["mqtt"], true).unwrap(),
        Some("mqtt".to_string())
    );
}

#[test]
fn ws_5_2_the_header_of_a_frame_is_read_before_its_payload() {
    // A masked binary frame of 65 536 octets: the 64-bit length (§5.7).
    let head = [0x82, 0xFF, 0, 0, 0, 0, 0, 1, 0, 0, 0x37, 0xfa, 0x21, 0x3d];
    let h = peek_header(&head).unwrap();
    assert_eq!(
        (h.opcode, h.fin, h.masked, h.payload_len, h.header_len),
        (Opcode::Binary, true, true, 65_536, 14)
    );
    // The 16-bit form of 256 octets, unmasked (§5.7).
    let h = peek_header(&[0x82, 0x7E, 0x01, 0x00]).unwrap();
    assert_eq!((h.masked, h.payload_len, h.header_len), (false, 256, 4));
    // A continuation frame without FIN, the length in seven bits.
    let h = peek_header(&[0x00, 0x83]).unwrap();
    assert_eq!(
        (h.opcode, h.fin, h.masked, h.payload_len, h.header_len),
        (Opcode::Continuation, false, true, 3, 6)
    );
    assert!(peek_header(&head[..5]).unwrap_err().is_incomplete());
    // A control frame must not be fragmented (§5.5).
    assert!(!peek_header(&[0x09, 0x80]).unwrap_err().is_incomplete());
}

#[test]
fn ws_7_4_1_the_status_codes_and_the_close_body() {
    assert_eq!(CloseCode::Normal.to_u16(), 1000);
    assert_eq!(CloseCode::ProtocolError.to_u16(), 1002);
    assert_eq!(CloseCode::UnsupportedData.to_u16(), 1003);
    assert_eq!(
        parse_close_body(&[0x03, 0xe9, b'b', b'y', b'e']).unwrap(),
        Some((1001, "bye".to_string()))
    );
    assert_eq!(parse_close_body(&[]).unwrap(), None);
    assert_eq!(
        parse_close_body(&[0x03]).unwrap_err().close_code(),
        CloseCode::ProtocolError
    );
    assert_eq!(
        parse_close_body(&1005u16.to_be_bytes())
            .unwrap_err()
            .close_code(),
        CloseCode::ProtocolError
    );
    assert_eq!(
        parse_close_body(&[0x03, 0xe8, 0xff])
            .unwrap_err()
            .close_code(),
        CloseCode::InvalidPayloadData
    );
}
