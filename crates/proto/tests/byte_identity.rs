// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! CP-A5 end-to-end byte-identity.
//!
//! Proves the full chain: real protobuf `FileDescriptorSet` bytes ->
//! [`FileDescriptorSet::decode`] -> [`Registry`] -> XTypes `DynamicType` ->
//! DDS-native XCDR2 wire. A proto-derived type is **wire-indistinguishable**
//! from the equivalent hand-written IDL type: the same values encode to the
//! same bytes, and each type decodes the other's wire.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use zerodds_proto::descriptor::FileDescriptorSet;
use zerodds_proto::registry::Registry;
use zerodds_types::dynamic::codec::{decode_dynamic, encode_dynamic};
use zerodds_types::dynamic::{
    DynamicData, DynamicDataFactory, DynamicType, DynamicTypeBuilderFactory, DynamicValue,
    ExtensibilityKind, TypeDescriptor, TypeKind,
};

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
fn native_sample_type(ext: ExtensibilityKind) -> DynamicType {
    let mut td = TypeDescriptor::structure("Sample");
    td.extensibility_kind = ext;
    let mut b = DynamicTypeBuilderFactory::create_type(td).unwrap();
    b.add_struct_member("id", 1, TypeDescriptor::primitive(TypeKind::Int32, "int32"))
        .unwrap();
    b.add_struct_member("name", 2, TypeDescriptor::string8(0))
        .unwrap();
    b.add_struct_member(
        "score",
        3,
        TypeDescriptor::primitive(TypeKind::Float64, "double"),
    )
    .unwrap();
    b.add_struct_member(
        "flags",
        4,
        TypeDescriptor::sequence(
            "sequence<int32>",
            TypeDescriptor::primitive(TypeKind::Int32, "int32"),
            0,
        ),
    )
    .unwrap();
    b.build().unwrap()
}

/// Identical values on any `Sample`-shaped type (member ids 1..=4).
fn populate(ty: &DynamicType) -> DynamicData {
    let int32_ty = DynamicTypeBuilderFactory::get_primitive_type(TypeKind::Int32).unwrap();
    let mut d = DynamicDataFactory::create_data(ty).unwrap();
    d.set_int32_value(1, 42).unwrap();
    d.set_string_value(2, "zerodds").unwrap();
    d.set_float64_value(3, 2.5).unwrap();
    let flags: Vec<DynamicData> = [1_i32, 2, 3]
        .into_iter()
        .map(|n| {
            let mut e = DynamicDataFactory::create_data(&int32_ty).unwrap();
            e.set_value_raw(0, DynamicValue::Int32(n));
            e
        })
        .collect();
    d.set_sequence_value(4, flags).unwrap();
    d
}

fn proto_sample_type(ext: ExtensibilityKind) -> DynamicType {
    let set = FileDescriptorSet::decode(&sample_descriptor_set()).expect("descriptor decodes");
    let mut reg = Registry::from_set(&set, ext);
    reg.build("demo.Sample").expect("Sample maps")
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
        proto.member_by_id(4).unwrap().dynamic_type().kind(),
        TypeKind::Sequence
    );
}

fn assert_byte_identity(ext: ExtensibilityKind, xcdr2: bool) {
    let proto = proto_sample_type(ext);
    let native = native_sample_type(ext);

    let proto_bytes = encode_dynamic(&populate(&proto), xcdr2, false).unwrap();
    let native_bytes = encode_dynamic(&populate(&native), xcdr2, false).unwrap();

    assert_eq!(
        proto_bytes, native_bytes,
        "proto-derived and native IDL types must encode identically ({ext:?}, xcdr2={xcdr2})"
    );
}

#[test]
fn byte_identical_appendable_xcdr2() {
    assert_byte_identity(ExtensibilityKind::Appendable, true);
}

#[test]
fn byte_identical_final_xcdr2() {
    assert_byte_identity(ExtensibilityKind::Final, true);
}

#[test]
fn byte_identical_mutable_xcdr2() {
    assert_byte_identity(ExtensibilityKind::Mutable, true);
}

#[test]
fn byte_identical_appendable_xcdr1() {
    assert_byte_identity(ExtensibilityKind::Appendable, false);
}

#[test]
fn proto_and_native_decode_each_others_wire() {
    let ext = ExtensibilityKind::Appendable;
    let proto = proto_sample_type(ext);
    let native = native_sample_type(ext);

    // native encodes -> proto-derived type decodes it losslessly
    let native_bytes = encode_dynamic(&populate(&native), true, false).unwrap();
    let via_proto = decode_dynamic(&proto, &native_bytes, true, false).unwrap();
    assert_eq!(via_proto.get_int32_value(1).unwrap(), 42);
    assert_eq!(via_proto.get_string_value(2).unwrap(), "zerodds");
    assert!((via_proto.get_float64_value(3).unwrap() - 2.5).abs() < f64::EPSILON);
    assert_eq!(
        via_proto
            .get_sequence_element(4, 2)
            .unwrap()
            .get_int32_value(0)
            .unwrap(),
        3
    );

    // and the reverse: proto encodes -> native IDL type decodes it
    let proto_bytes = encode_dynamic(&populate(&proto), true, false).unwrap();
    let via_native = decode_dynamic(&native, &proto_bytes, true, false).unwrap();
    assert_eq!(via_native.get_string_value(2).unwrap(), "zerodds");

    // re-encode the decoded value reproduces the wire byte-for-byte
    assert_eq!(
        encode_dynamic(&via_proto, true, false).unwrap(),
        native_bytes
    );
}
