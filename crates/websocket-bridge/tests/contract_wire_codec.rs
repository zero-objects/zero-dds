// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-websocket-bridge` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses (`src/daemon/json_frames.rs`, feature `daemon`):
//! `ProbeRequest::new` with an encoding and a byte order, `emit =
//! Emit::KeyHash`, and `encode`/`decode` between a `serde_json::Value` and
//! a body. It relies on the octets of DDS-XTypes 1.3 §7.4: under XCDR2 an
//! `@appendable` struct is a DHEADER, then the members (§7.4.2), a string
//! its length with the NUL, the characters and the NUL (§7.4.1.1.2); and on
//! the KeyHash of DDSI-RTPS 2.5 §9.6.4.8 (a key holder of at most 16 octets,
//! padded with zeros).
#![allow(clippy::unwrap_used, clippy::expect_used, missing_docs)]

use serde_json::{Value, json};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_codec::{Emit, ProbeRequest};

#[test]
fn xtypes_7_4_2_a_json_value_is_the_octets_of_its_type_and_back() {
    let composed = zerodds_idl_compose::compose_source_with(
        "<contract>",
        "@appendable struct R { @key long id; string text; };",
        &zerodds_idl_compose::PathLoader::bare(),
        zerodds_idl_compose::ComposeOptions::default(),
    );
    let ast = composed.ast;
    let value = json!({ "id": 7, "text": "hi" });
    let request = ProbeRequest::new("R", Encoding::DelimitedCdr, Endianness::Little);
    let body = zerodds_wire_codec::encode(&ast, &request, &value).unwrap();
    assert_eq!(
        body,
        [0x0b, 0, 0, 0, 0x07, 0, 0, 0, 0x03, 0, 0, 0, b'h', b'i', 0]
    );
    assert_eq!(
        zerodds_wire_codec::decode::<Value>(&ast, &request, &body).unwrap(),
        value
    );
    let mut request = ProbeRequest::new("R", Encoding::PlainCdr2, Endianness::Big);
    request.emit = Emit::KeyHash;
    assert_eq!(
        zerodds_wire_codec::encode(&ast, &request, &value).unwrap(),
        [0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );
}
