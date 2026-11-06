// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-shape-main` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-types`:
//! - The `DdsType` impl `zerodds-build` generates for `ShapeType` from
//!   `interop/shapes/omg-dds-rtps.idl` (`src/shape_type.rs`, `include!`): a
//!   constant `TYPE_IDENTIFIER: TypeIdentifier =
//!   TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([..]))` and a
//!   `type_object()` that is `type_object::TypeObject::from_bytes_le(&[..])
//!   .ok()` over the octets of the COMPLETE object.
//! - `assignability::KeyMemberBounds::Ignored` (`src/main.rs`,
//!   `suite_settings`): a reader of `@key string<128> color` takes a writer
//!   whose key is an unbounded string (dust_dds 0.16.1).
//!
//! The tool is a binary without a library, so the contract builds what the
//! generated code holds from the specification and calls `zerodds-types`
//! with it.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5 and Annex B (the COMPLETE object of
//! `@appendable struct ShapeType { @key string<128> color; long x; long y;
//! long shapesize; sequence<uint8> additional_payload_size; }` written out by
//! hand with the layout rules that reproduce CycloneDDS 11.0.1's octets for
//! `to::Plain`; its MD5 computed outside this tree), §7.3.4.6.1 (`EK_BOTH`
//! in the header of a plain collection of a fully descriptive element),
//! §7.2.2.4.4.4.8 (key members must be understood), §7.2.4.4.4.4 Table 19
//! (a reader's key member shorter than the writer's is not assignable).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::assignability::{AssignabilityConfig, KeyMemberBounds, is_assignable};
use zerodds_types::hash::compute_complete_hash;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::PrimitiveKind;
use zerodds_types::type_object::common::{CommonStructMember, NameHash};
use zerodds_types::type_object::flags::{StructMemberFlag, StructTypeFlag};
use zerodds_types::type_object::minimal::{
    MinimalStructHeader, MinimalStructMember, MinimalStructType,
};
use zerodds_types::type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};
use zerodds_types::{EquivalenceHash, TypeIdentifier};

/// `ShapeType` of `omg-dds-rtps.idl`, COMPLETE, XCDR2 little endian. The
/// members after `color` are those of `Shape1Final` in
/// `tools/omg-xtypes-main`; the last one's identifier is
/// `80 f3 0100 00 0d`: `TI_PLAIN_SEQUENCE_SMALL`, `EK_BOTH`, element flags
/// `TRY_CONSTRUCT1`, bound 0 (unbounded), `TK_UINT8`.
const SHAPE_TYPE_COMPLETE: &str = "ae000000f251020012000000000000000a0000005368617065547970650000008e0000000500000014000000000000003100708006000000636f6c6f72000000100000000100000001000400020000007800000010000000020000000100040002000000790000001800000003000000010004000a000000736861706573697a650000002a00000004000000010080f30100000d180000006164646974696f6e616c5f7061796c6f61645f73697a65000000";
/// MD5 of those 178 octets, first 14.
const SHAPE_TYPE_HASH: [u8; 14] = [
    0x4d, 0x9a, 0x6d, 0x25, 0xf1, 0xeb, 0x43, 0x4d, 0xbb, 0x3a, 0x60, 0x1f, 0xb1, 0x8a,
];

/// What the generated impl writes, spelled the way it spells it.
const TYPE_IDENTIFIER: TypeIdentifier =
    TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(SHAPE_TYPE_HASH));

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn annex_b_the_generated_object_parses_and_its_hash_is_the_announced_identifier() {
    let bytes = unhex(SHAPE_TYPE_COMPLETE);
    let object = TypeObject::from_bytes_le(&bytes).ok();
    let Some(TypeObject::Complete(CompleteTypeObject::Struct(shape))) = &object else {
        panic!("the generated `type_object()` would answer None: {object:?}");
    };
    assert_eq!(shape.header.detail.type_name, "ShapeType");
    assert_eq!(
        shape.struct_flags,
        StructTypeFlag(StructTypeFlag::IS_APPENDABLE)
    );
    let color = &shape.member_seq[0];
    assert_eq!(color.detail.name, "color");
    assert!(color.common.member_flags.has(StructMemberFlag::IS_KEY));
    assert!(
        color
            .common
            .member_flags
            .has(StructMemberFlag::IS_MUST_UNDERSTAND)
    );
    assert_eq!(
        color.common.member_type_id,
        TypeIdentifier::String8Small { bound: 128 }
    );
    let payload = &shape.member_seq[4];
    assert_eq!(payload.detail.name, "additional_payload_size");
    let TypeIdentifier::PlainSequenceSmall {
        header,
        bound,
        element,
    } = &payload.common.member_type_id
    else {
        panic!("{:?}", payload.common.member_type_id);
    };
    assert_eq!(header.equiv_kind, 0xF3, "EK_BOTH");
    assert_eq!(*bound, 0, "unbounded");
    assert_eq!(**element, TypeIdentifier::Primitive(PrimitiveKind::UInt8));

    // The identifier a peer resolves this object by.
    assert_eq!(
        TypeIdentifier::EquivalenceHashComplete(
            compute_complete_hash(&shape_object(&bytes)).unwrap()
        ),
        TYPE_IDENTIFIER
    );
}

fn shape_object(bytes: &[u8]) -> zerodds_types::CompleteTypeObject {
    let TypeObject::Complete(object) = TypeObject::from_bytes_le(bytes).unwrap() else {
        panic!("complete");
    };
    object
}

fn keyed_shape(color: TypeIdentifier) -> MinimalTypeObject {
    let member = |id: u32, flags: u16, type_id: TypeIdentifier, name: &str| MinimalStructMember {
        common: CommonStructMember {
            member_id: id,
            member_flags: StructMemberFlag(flags),
            member_type_id: type_id,
        },
        detail: NameHash::from_name(name),
    };
    let key = StructMemberFlag::TRY_CONSTRUCT1
        | StructMemberFlag::IS_MUST_UNDERSTAND
        | StructMemberFlag::IS_KEY;
    let long = || TypeIdentifier::Primitive(PrimitiveKind::Int32);
    MinimalTypeObject::Struct(MinimalStructType {
        struct_flags: StructTypeFlag(StructTypeFlag::IS_APPENDABLE),
        header: MinimalStructHeader {
            base_type: TypeIdentifier::None,
        },
        member_seq: vec![
            member(0, key, color, "color"),
            member(1, StructMemberFlag::TRY_CONSTRUCT1, long(), "x"),
            member(2, StructMemberFlag::TRY_CONSTRUCT1, long(), "y"),
            member(3, StructMemberFlag::TRY_CONSTRUCT1, long(), "shapesize"),
        ],
    })
}

#[test]
fn xtypes_7_2_4_4_4_4_ignored_key_member_bounds_admit_an_unbounded_writer_key() {
    let mut registry = TypeRegistry::new();
    let reader = registry
        .insert_object(TypeObject::Minimal(keyed_shape(
            TypeIdentifier::String8Small { bound: 128 },
        )))
        .unwrap();
    // dust_dds 0.16.1's writer: the key `color` as an unbounded string.
    let writer = registry
        .insert_object(TypeObject::Minimal(keyed_shape(
            TypeIdentifier::String8Small { bound: 0 },
        )))
        .unwrap();
    let reader = TypeIdentifier::EquivalenceHashMinimal(reader);
    let writer = TypeIdentifier::EquivalenceHashMinimal(writer);

    // The clause: the reader's key cannot hold every key the writer sends.
    let enforced = AssignabilityConfig::default();
    assert_eq!(enforced.key_member_bounds, KeyMemberBounds::Enforced);
    assert!(!is_assignable(&writer, &reader, &registry, &enforced).is_yes());

    // The suite setting: the bullets are skipped and the member rule,
    // with `ignore_string_bounds`, admits the pair.
    let ignored = AssignabilityConfig {
        key_member_bounds: KeyMemberBounds::Ignored,
        ..AssignabilityConfig::default()
    };
    assert!(is_assignable(&writer, &reader, &registry, &ignored).is_yes());
}
