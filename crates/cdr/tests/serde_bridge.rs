// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Smoke-Test fuer `serde-bridge` Feature.

#![cfg(feature = "serde-bridge")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use serde::{Deserialize, Serialize};
use zerodds_cdr::serde_bridge::{decode_via_serde, decoded_json_repr, encode_via_serde};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Telemetry {
    sensor_id: u32,
    name: String,
    samples: Vec<f64>,
}

#[test]
fn serde_roundtrip_struct() {
    let t = Telemetry {
        sensor_id: 42,
        name: "thermostat-living".to_string(),
        samples: vec![21.5, 21.7, 22.0],
    };
    let bytes = encode_via_serde(&t).expect("encode");
    let t2: Telemetry = decode_via_serde(&bytes).expect("decode");
    assert_eq!(t, t2);
}

#[test]
fn serde_roundtrip_primitives() {
    let bytes = encode_via_serde(&42i64).expect("encode i64");
    let v: i64 = decode_via_serde(&bytes).expect("decode i64");
    assert_eq!(v, 42);
}

#[test]
fn serde_decoded_json_repr_is_xcdr2_string_payload() {
    let t = Telemetry {
        sensor_id: 1,
        name: "x".to_string(),
        samples: vec![],
    };
    let bytes = encode_via_serde(&t).expect("encode");
    // First 4 bytes are length (LE uint32) of the JSON+NUL.
    let len = u32::from_le_bytes(bytes[0..4].try_into().expect("4 bytes"));
    assert!(len as usize <= bytes.len() - 4);
    // Decoded JSON-repr matches what we'd get from serde_json::to_string.
    let json = decoded_json_repr(&bytes).expect("read string");
    let expected = serde_json::to_string(&t).expect("serde_json");
    assert_eq!(json, expected);
}
