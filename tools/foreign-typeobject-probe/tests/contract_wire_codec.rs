// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-foreign-typeobject-probe` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-wire-codec` (`src/main.rs`; a binary
//! without a library, so the contract calls `zerodds-wire-codec` directly):
//! `ProbeRequest::in_dialect` with the type's qualified name, the encoding
//! the type's extensibility and the sample's XCDR version choose
//! (`PlainCdr1`, `PlCdr1`, `PlainCdr2`, `DelimitedCdr`, `PlCdr2`), the
//! sample's byte order and the dialect of `--dialect` (default
//! `Dialect::Cyclone`); then `decode::<serde_json::Value>` over the model of
//! the peer's type, printed one JSON object per sample. An `Err` ends the
//! run.
//!
//! The types are a peer's: CycloneDDS 11.0.1's COMPLETE TypeObjects of
//! `to::Inner`, `to::Appendable` and `to::Mutable`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`) and the hand-written
//! COMPLETE object of `@final struct to::Outer { to::Inner i; }`, turned into
//! a model by `zerodds_types::dynamic::to_ast::dynamic_type` as the tool does.
//!
//! Oracle for the sample octets, written out by hand: DDS-XTypes 1.3 §7.4.2
//! (PLAIN_CDR2 for FINAL, a nested FINAL struct inline; DELIMITED_CDR =
//! DHEADER + PLAIN_CDR2; PL_CDR2 = DHEADER + an EMHEADER per member),
//! §7.4.1.1.1 (byte order of the stream), §7.4.3.4.2 (EMHEADER layout).
//! JSON form: an aggregate is an object by member name and a 32- or 16-bit
//! integer a JSON number (`zerodds_wire_codec::json`, the documented value
//! form the tool prints).
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value, json};
use zerodds_ast_layout::Dialect;
use zerodds_ast_layout::key::{Encoding, Endianness};
use zerodds_types::dynamic::to_ast::dynamic_type;
use zerodds_types::dynamic::type_::DynamicType;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_object::TypeObject;
use zerodds_wire_codec::{ProbeRequest, decode};

const INNER: &str = "4c000000f251010012000000000000000a000000746f3a3a496e6e65720000002c0000000200000010000000000000000100040002000000780000001000000001000000010004000200000079000000";
const OUTER: &str = "48000000f251010012000000000000000a000000746f3a3a4f75746572000000280000000100000020000000000000000100f2f00388e0e0c6619e4175c68e8af80000000200000069000000";
const APPENDABLE: &str = "50000000f251020017000000000000000f000000746f3a3a417070656e6461626c6500002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
const MUTABLE: &str = "4c000000f251040014000000000000000c000000746f3a3a4d757461626c65002c00000002000000100000000a0000000100040002000000610000001000000014000000010003000200000062000000";

fn object(hex: &str) -> TypeObject {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    TypeObject::from_bytes_le(&bytes).unwrap()
}

fn peer_type(hex: &str, registry: &TypeRegistry) -> DynamicType {
    let TypeObject::Complete(complete) = object(hex) else {
        panic!("a COMPLETE object");
    };
    dynamic_type(&complete, registry).unwrap()
}

fn read(
    ty: &DynamicType,
    encoding: Encoding,
    endianness: Endianness,
    payload: &[u8],
) -> Result<Value, zerodds_wire_codec::ProbeError> {
    let request =
        ProbeRequest::in_dialect(&ty.qualified_name(), encoding, endianness, Dialect::Cyclone);
    decode(ty.ast(), &request, payload)
}

#[test]
fn xtypes_7_4_2_a_final_sample_with_a_nested_peer_type() {
    let mut registry = TypeRegistry::new();
    registry.insert_object(object(INNER)).unwrap();
    let outer = peer_type(OUTER, &registry);
    let value = read(
        &outer,
        Encoding::PlainCdr2,
        Endianness::Little,
        &[0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00],
    )
    .unwrap();
    assert_eq!(value, json!({ "i": { "x": 1, "y": 2 } }));
}

#[test]
fn xtypes_7_4_2_an_appendable_sample_big_endian() {
    let ty = peer_type(APPENDABLE, &TypeRegistry::new());
    let value = read(
        &ty,
        Encoding::DelimitedCdr,
        Endianness::Big,
        &[
            0x00, 0x00, 0x00, 0x06, // DHEADER: 6
            0x00, 0x00, 0x00, 0x07, 0xff, 0xfe,
        ],
    )
    .unwrap();
    assert_eq!(value, json!({ "a": 7, "b": -2 }));
}

#[test]
fn xtypes_7_4_3_4_2_a_mutable_sample_little_endian() {
    let ty = peer_type(MUTABLE, &TypeRegistry::new());
    let value = read(
        &ty,
        Encoding::PlCdr2,
        Endianness::Little,
        &[
            0x10, 0x00, 0x00, 0x00, // DHEADER: 16
            0x14, 0x00, 0x00, 0x10, // EMHEADER: LC 1, id 20 (`b`) first
            0xfe, 0xff, 0x00, 0x00, // b, pad
            0x0a, 0x00, 0x00, 0x20, // EMHEADER: LC 2, id 10 (`a`)
            0x07, 0x00, 0x00, 0x00,
        ],
    );
    // §7.4.1.2.3 lets a writer reorder the members of a mutable type; the
    // reader finds each by its id.
    assert_eq!(value.unwrap(), json!({ "a": 7, "b": -2 }));
}

#[test]
fn a_sample_the_type_cannot_read_ends_with_an_error() {
    let ty = peer_type(APPENDABLE, &TypeRegistry::new());
    // The DHEADER announces 6 octets, 3 follow.
    assert!(
        read(
            &ty,
            Encoding::DelimitedCdr,
            Endianness::Little,
            &[0x06, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00]
        )
        .is_err()
    );
}
