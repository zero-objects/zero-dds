// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Smoke test for `#[derive(DdsType)]`.
//!
//! Verifies that a plain `struct` with `#[derive(DdsType)]` gets a
//! valid `impl DdsType` that encodes/decodes byte-exact — the same
//! wire format that the idl-rust codegen produces.

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
    clippy::approx_constant,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_cdr_derive::DdsType;
use zerodds_dcps::DdsType as DdsTypeTrait;

#[derive(DdsType, Debug, Clone, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(DdsType, Debug, Clone, PartialEq)]
pub struct Sensor {
    #[dds(key)]
    pub id: i32,
    pub value: f64,
}

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(type_name = "namespaced::Custom")]
pub struct Custom {
    pub n: u32,
}

#[test]
fn type_name_default_is_struct_ident() {
    assert_eq!(<Point as DdsTypeTrait>::TYPE_NAME, "Point");
}

#[test]
fn type_name_attribute_overrides_default() {
    assert_eq!(<Custom as DdsTypeTrait>::TYPE_NAME, "namespaced::Custom");
}

#[test]
fn keyed_struct_sets_has_key() {
    fn has_key<T: DdsTypeTrait>() -> bool {
        T::HAS_KEY
    }
    assert!(has_key::<Sensor>());
    assert!(!has_key::<Point>());
}

#[test]
fn point_encode_matches_v2_wire() {
    let p = Point { x: 1, y: -2 };
    let mut buf = Vec::new();
    p.encode(&mut buf).expect("encode");
    // V-2 wire: 01 00 00 00 fe ff ff ff (LE i32 pair)
    let expected: [u8; 8] = [0x01, 0, 0, 0, 0xfe, 0xff, 0xff, 0xff];
    assert_eq!(buf.as_slice(), &expected);
}

#[test]
fn point_roundtrip() {
    let p = Point { x: 42, y: -7 };
    let mut buf = Vec::new();
    p.encode(&mut buf).expect("encode");
    let q = Point::decode(&buf).expect("decode");
    assert_eq!(p, q);
}

#[test]
fn sensor_roundtrip() {
    let s = Sensor {
        id: 42,
        value: 3.14,
    };
    let mut buf = Vec::new();
    s.encode(&mut buf).expect("encode");
    let s2 = Sensor::decode(&buf).expect("decode");
    assert_eq!(s, s2);
}
