// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CP-A5 end-to-end byte-identity.
//!
//! Proves the full chain: real protobuf `FileDescriptorSet` bytes ->
//! [`FileDescriptorSet::decode`] -> [`Registry`] -> an XTypes type ->
//! DDS-native XCDR wire. A proto-derived type is **wire-indistinguishable**
//! from the equivalent hand-written IDL type: the same values encode to the
//! same bytes, and each type decodes the other's wire.
//!
//! ## The other side is the IDL front end, not this crate
//!
//! The type the proto-derived one is held against is lowered from IDL text
//! by `zerodds-idl-parse` and `zerodds-idl-lower` — the same path a
//! hand-written `.idl` takes — and not built with the same builder the
//! registry uses. A comparison between two types one builder produced
//! would agree with itself whatever the builder decided; this one has to
//! agree with the front end.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::sync::Arc;

use zerodds_ast::{Ast, PrimitiveKind};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_proto::descriptor::FileDescriptorSet;
use zerodds_proto::registry::Registry;
use zerodds_types::dynamic::{DynamicData, DynamicType, ExtensibilityKind, TypeKind};
use zerodds_wire_codec::value::ProbeRequest;
use zerodds_wire_codec::{decode, encode};

// --- a minimal protobuf wire encoder, just enough for a FileDescriptorSet ---

fn varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    }
}

fn tag(out: &mut Vec<u8>, field: u32, wire: u32) {
    varint(out, u64::from((field << 3) | wire));
}

fn len_field(out: &mut Vec<u8>, field: u32, payload: &[u8]) {
    tag(out, field, 2);
    varint(out, payload.len() as u64);
    out.extend_from_slice(payload);
}

fn varint_field(out: &mut Vec<u8>, field: u32, v: u64) {
    tag(out, field, 0);
    varint(out, v);
}

/// Encode a `FieldDescriptorProto`: name(1), number(3), label(4), type(5).
fn field_proto(name: &str, number: u64, label: u64, ty: u64) -> Vec<u8> {
    let mut f = Vec::new();
    len_field(&mut f, 1, name.as_bytes());
    varint_field(&mut f, 3, number);
    varint_field(&mut f, 4, label);
    varint_field(&mut f, 5, ty);
    f
}

/// The `FileDescriptorSet` for:
/// ```proto
/// package demo;
/// message Sample {
///   int32 id = 1; string name = 2; double score = 3; repeated int32 flags = 4;
/// }
/// ```
fn sample_descriptor_set() -> Vec<u8> {
    // DescriptorProto Sample
    let mut msg = Vec::new();
    len_field(&mut msg, 1, b"Sample");
    len_field(&mut msg, 2, &field_proto("id", 1, 1, 5)); // int32
    len_field(&mut msg, 2, &field_proto("name", 2, 1, 9)); // string
    len_field(&mut msg, 2, &field_proto("score", 3, 1, 1)); // double
    len_field(&mut msg, 2, &field_proto("flags", 4, 3, 5)); // repeated int32

    // FileDescriptorProto: package(2), message_type(4)
    let mut file = Vec::new();
    len_field(&mut file, 2, b"demo");
    len_field(&mut file, 4, &msg);

    // FileDescriptorSet: file(1)
    let mut set = Vec::new();
    len_field(&mut set, 1, &file);
    set
}

/// The IDL a user would otherwise hand-write for the same data model.
///
/// The extensibility is written out rather than left to the default,
/// because that is the axis the comparison runs over.
///
/// The `@id(n)` are written out for the same reason, and they are the
/// **claim** being tested rather than a convenience: a proto field number
/// becomes an XTypes member id, so the IDL a user would write for the
/// same `.proto` states those numbers. Under `@mutable` the id reaches
/// the wire in every EMHEADER, and a comparison against IDL that left
/// the ids implicit would compare a type numbered 1..4 against one
/// numbered 0..3 — which is what this test found when the ids were left
/// out.
fn native_idl(ext: ExtensibilityKind) -> String {
    format!(
        "@{}\nstruct Sample {{\n  @id(1) long id;\n  @id(2) string name;\n  \
         @id(3) double score;\n  @id(4) sequence<long> flags;\n}};\n",
        ext.as_str()
    )
}

/// The model that IDL lowers to, through the front end.
fn native_sample_type(ext: ExtensibilityKind) -> Arc<Ast> {
    let source = native_idl(ext);
    let parsed = zerodds_idl_parse::parse(&source);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "native/Sample.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the model resolves");
    assert!(report.is_clean(), "{:?}", report.broken);
    Arc::new(ast)
}

/// The proto-derived type, and the model it lives in.
fn proto_sample_type(ext: ExtensibilityKind) -> DynamicType {
    let set = FileDescriptorSet::decode(&sample_descriptor_set()).expect("descriptor decodes");
    let reg = Registry::from_set(&set, ext);
    reg.build_type("demo.Sample").expect("Sample maps")
}

/// The same sample value, in the one representation both types read.
///
/// A `DynamicData` and not two hand-built values: the value is the thing
/// held constant, so it is written once and encoded against each type.
fn sample_value() -> DynamicData {
    DynamicData::Aggregate(vec![
        (
            "id".to_string(),
            DynamicData::integer(PrimitiveKind::Int32, 42),
        ),
        ("name".to_string(), DynamicData::text("zerodds".to_string())),
        (
            "score".to_string(),
            DynamicData::float(PrimitiveKind::Float64, 2.5),
        ),
        (
            "flags".to_string(),
            DynamicData::Collection(
                [1_i128, 2, 3]
                    .into_iter()
                    .map(|n| DynamicData::integer(PrimitiveKind::Int32, n))
                    .collect(),
            ),
        ),
    ])
}

#[test]
fn proto_type_structure_matches_native() {
    let proto = proto_sample_type(ExtensibilityKind::Appendable);
    assert_eq!(proto.kind(), TypeKind::Structure);
    assert_eq!(proto.member_count(), 4);
    // proto field numbers are the XTypes member ids
    assert_eq!(proto.member_by_id(1).unwrap().name(), "id");
    assert_eq!(proto.member_by_id(2).unwrap().name(), "name");
    assert_eq!(proto.member_by_id(4).unwrap().name(), "flags");
    assert_eq!(
        proto
            .member_by_id(4)
            .unwrap()
            .dynamic_type()
            .unwrap()
            .kind(),
        TypeKind::Sequence
    );
}

/// The two types, the same value, the same octets.
fn assert_byte_identity(ext: ExtensibilityKind, encoding: Encoding, endianness: Endianness) {
    let proto = proto_sample_type(ext);
    let native = native_sample_type(ext);
    let value = sample_value();
    let request = ProbeRequest::new("Sample", encoding, endianness);

    let proto_bytes = encode::<DynamicData>(proto.ast(), &request, &value)
        .unwrap_or_else(|error| panic!("the proto-derived type does not encode: {error}"));
    let native_bytes = encode::<DynamicData>(&native, &request, &value)
        .unwrap_or_else(|error| panic!("the IDL type does not encode: {error}"));

    assert_eq!(
        proto_bytes, native_bytes,
        "a proto-derived and an IDL type must encode identically \
         ({ext:?}, {encoding:?}, {endianness:?})"
    );
}

#[test]
fn byte_identical_appendable_xcdr2() {
    assert_byte_identity(
        ExtensibilityKind::Appendable,
        Encoding::PlainCdr2,
        Endianness::Little,
    );
}

#[test]
fn byte_identical_final_xcdr2() {
    assert_byte_identity(
        ExtensibilityKind::Final,
        Encoding::PlainCdr2,
        Endianness::Little,
    );
}

#[test]
fn byte_identical_mutable_xcdr2() {
    assert_byte_identity(
        ExtensibilityKind::Mutable,
        Encoding::PlainCdr2,
        Endianness::Little,
    );
}

#[test]
fn byte_identical_appendable_xcdr1() {
    assert_byte_identity(
        ExtensibilityKind::Appendable,
        Encoding::PlainCdr1,
        Endianness::Little,
    );
}

/// Both byte orders, so that nothing here holds only on a little-endian
/// host.
#[test]
fn byte_identical_big_endian() {
    for ext in ExtensibilityKind::ALL {
        for encoding in [Encoding::PlainCdr1, Encoding::PlainCdr2] {
            assert_byte_identity(ext, encoding, Endianness::Big);
        }
    }
}

#[test]
fn proto_and_native_decode_each_others_wire() {
    let ext = ExtensibilityKind::Appendable;
    let proto = proto_sample_type(ext);
    let native = native_sample_type(ext);
    let request = ProbeRequest::new("Sample", Encoding::PlainCdr2, Endianness::Little);
    let value = sample_value();

    // The IDL type writes; the proto-derived type reads it back whole.
    let native_bytes = encode::<DynamicData>(&native, &request, &value).unwrap();
    let via_proto: DynamicData = decode(proto.ast(), &request, &native_bytes).unwrap();
    assert_eq!(
        via_proto.member("id"),
        Some(&DynamicData::integer(PrimitiveKind::Int32, 42))
    );
    assert_eq!(
        via_proto.member("name"),
        Some(&DynamicData::text("zerodds".to_string()))
    );
    assert_eq!(
        via_proto.member("score"),
        Some(&DynamicData::float(PrimitiveKind::Float64, 2.5))
    );
    assert_eq!(
        via_proto.member("flags").and_then(DynamicData::items),
        Some(
            [1_i128, 2, 3]
                .map(|n| DynamicData::integer(PrimitiveKind::Int32, n))
                .as_slice()
        )
    );

    // And the reverse: the proto-derived type writes, the IDL type reads.
    let proto_bytes = encode::<DynamicData>(proto.ast(), &request, &value).unwrap();
    let via_native: DynamicData = decode(&native, &request, &proto_bytes).unwrap();
    assert_eq!(
        via_native.member("name"),
        Some(&DynamicData::text("zerodds".to_string()))
    );

    // Re-encoding what came back reproduces the wire, octet for octet.
    assert_eq!(
        encode::<DynamicData>(proto.ast(), &request, &via_proto).unwrap(),
        native_bytes
    );
}
