// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `rmw-zerodds-shim` with `zerodds-wire-codec`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-codec` (`src/dynamic.rs`, the
//! ROS 2 dynamic type support; the dependency has `serde-json` off):
//! - `zerodds_dyn_data_serialize`: `ProbeRequest::in_dialect(qualified
//!   name, Encoding::PlainCdr1, Endianness::Little, dialect)` and
//!   `encode(ast, &request, &DynamicData)`, the body behind a CDR_LE
//!   encapsulation header;
//! - `zerodds_dyn_data_deserialize`: the encoding and byte order read from
//!   the encapsulation header, then `decode::<DynamicData>(ast, &request,
//!   &bytes[4..])`;
//! - `default_of_type::<DynamicData>(ast, id, "")` for a fresh value and
//!   for a member or element that is cleared;
//! - `Source::as_text` to read a string member.
//!
//! Oracle: DDS-XTypes 1.3 §7.4.1.1.1 Table 31, §7.4.1.1.2, §7.4.3.2.2
//! Table 37 (MAXALIGN 8 for version 1, 4 for version 2), §7.4.2 (a
//! `@final` struct in version 2 is PLAIN_CDR2), §7.4.3.5.3 (an array is
//! its elements without a count, a sequence of a primitive is a count and
//! the elements), §7.2.2.4.4.4.7 Table 9 and §7.2.2.4.4.4.9 (defaults).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_ast::PrimitiveKind;
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_types::dynamic::{
    DynamicData, DynamicType, DynamicTypeBuilder, ExtensibilityKind, NewMember, TypeKind,
};
use zerodds_wire_codec::{ProbeRequest, Source};

/// `@final struct pkg::msg::Sample { int32 a; float64 b; string s;
/// int32 arr[2]; sequence<int16> seq; int32 d = 42; }`, built the way
/// `src/dynamic.rs` builds a ROS message.
fn sample_type() -> DynamicType {
    let mut model = DynamicTypeBuilder::new();
    let module = model.module(&["pkg", "msg"]).unwrap();
    let root = model
        .structure_in(module, "Sample", ExtensibilityKind::Final)
        .unwrap();
    let int32 = model.primitive(TypeKind::Int32).unwrap();
    let float64 = model.primitive(TypeKind::Float64).unwrap();
    let string = model.string(false, None).unwrap();
    let array = model.array(int32, &[2]).unwrap();
    let int16 = model.primitive(TypeKind::Int16).unwrap();
    let sequence = model.sequence(int16, None).unwrap();
    for (name, type_) in [
        ("a", int32),
        ("b", float64),
        ("s", string),
        ("arr", array),
        ("seq", sequence),
    ] {
        model
            .add_member(root, &NewMember::new(name, type_))
            .unwrap();
    }
    model
        .add_member(root, &NewMember::new("d", int32).with_default("42"))
        .unwrap();
    model.finish_as(root).unwrap()
}

fn value() -> DynamicData {
    DynamicData::Aggregate(vec![
        (
            "a".to_string(),
            DynamicData::integer(PrimitiveKind::Int32, -2),
        ),
        (
            "b".to_string(),
            DynamicData::float(PrimitiveKind::Float64, 0.5),
        ),
        ("s".to_string(), DynamicData::text("x".to_string())),
        (
            "arr".to_string(),
            DynamicData::Collection(vec![
                DynamicData::integer(PrimitiveKind::Int32, 3),
                DynamicData::integer(PrimitiveKind::Int32, 4),
            ]),
        ),
        (
            "seq".to_string(),
            DynamicData::Collection(vec![DynamicData::integer(PrimitiveKind::Int16, 5)]),
        ),
        (
            "d".to_string(),
            DynamicData::integer(PrimitiveKind::Int32, 6),
        ),
    ])
}

/// The value as PLAIN_CDR1 little-endian: `b` at 8, `arr` at 24, `seq`
/// at 32, `d` at 40.
const SAMPLE_CDR1_LE: [u8; 44] = [
    0xfe, 0xff, 0xff, 0xff, // a
    0x00, 0x00, 0x00, 0x00, // padding
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe0, 0x3f, // b at 8
    0x02, 0x00, 0x00, 0x00, b'x', 0x00, // s at 16
    0x00, 0x00, // padding
    0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, // arr at 24
    0x01, 0x00, 0x00, 0x00, 0x05, 0x00, // seq at 32
    0x00, 0x00, // padding
    0x06, 0x00, 0x00, 0x00, // d at 40
];

/// The same value as PLAIN_CDR2 little-endian (an incoming CDR2_LE
/// sample): `b` at 4.
const SAMPLE_CDR2_LE: [u8; 40] = [
    0xfe, 0xff, 0xff, 0xff, // a
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe0, 0x3f, // b at 4
    0x02, 0x00, 0x00, 0x00, b'x', 0x00, // s at 12
    0x00, 0x00, // padding
    0x03, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, // arr at 20
    0x01, 0x00, 0x00, 0x00, 0x05, 0x00, // seq at 28
    0x00, 0x00, // padding
    0x06, 0x00, 0x00, 0x00, // d at 36
];

fn request(ty: &DynamicType, encoding: Encoding) -> ProbeRequest {
    ProbeRequest::in_dialect(
        &ty.qualified_name(),
        encoding,
        Endianness::Little,
        Dialect::default(),
    )
}

fn check_members(decoded: &DynamicData) {
    assert!(matches!(
        decoded.member("a"),
        Some(DynamicData::Integer { value: -2, .. })
    ));
    let Some(DynamicData::Float { value, .. }) = decoded.member("b") else {
        panic!("b is a float: {decoded:?}");
    };
    assert_eq!(value.to_bits(), 0.5_f64.to_bits());
    assert_eq!(decoded.member("s").unwrap().as_text(), Some("x"));
    assert_eq!(decoded.member("arr").unwrap().items().unwrap().len(), 2);
    assert_eq!(decoded.member("seq").unwrap().items().unwrap().len(), 1);
}

#[test]
fn xtypes_7_4_3_2_2_dynamic_data_serializes_as_plain_cdr1_little_endian() {
    let ty = sample_type();
    let request = request(&ty, Encoding::PlainCdr1);
    let bytes = zerodds_wire_codec::encode(ty.ast(), &request, &value()).unwrap();
    assert_eq!(bytes, SAMPLE_CDR1_LE);

    let decoded: DynamicData =
        zerodds_wire_codec::decode(ty.ast(), &request, &SAMPLE_CDR1_LE).unwrap();
    check_members(&decoded);
    assert_eq!(
        zerodds_wire_codec::encode(ty.ast(), &request, &decoded).unwrap(),
        SAMPLE_CDR1_LE
    );
}

#[test]
fn xtypes_7_4_2_a_plain_cdr2_sample_decodes_with_the_version_2_alignment() {
    let ty = sample_type();
    let request = request(&ty, Encoding::PlainCdr2);
    let decoded: DynamicData =
        zerodds_wire_codec::decode(ty.ast(), &request, &SAMPLE_CDR2_LE).unwrap();
    check_members(&decoded);
    assert!(
        zerodds_wire_codec::decode::<DynamicData>(ty.ast(), &request, &SAMPLE_CDR2_LE[..30])
            .is_err()
    );
}

#[test]
fn xtypes_7_2_2_4_4_4_9_a_fresh_value_takes_the_explicit_default_then_table_9() {
    let ty = sample_type();
    let fresh: DynamicData = zerodds_wire_codec::default_of_type(ty.ast(), ty.id(), "").unwrap();
    assert!(matches!(
        fresh.member("a"),
        Some(DynamicData::Integer { value: 0, .. })
    ));
    assert!(matches!(
        fresh.member("d"),
        Some(DynamicData::Integer { value: 42, .. })
    ));
    assert_eq!(fresh.member("s").unwrap().as_text(), Some(""));
    assert_eq!(fresh.member("arr").unwrap().items().unwrap().len(), 2);
    assert!(fresh.member("seq").unwrap().items().unwrap().is_empty());
}
