// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-test-vendor-reader` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/lib.rs`,
//! `keyed_point_a_type_object`): `dynamic::DynamicType::find` to locate
//! `KeyedPointA` in the model, `DynamicType::id`,
//! `from_ast::complete_type_object`, and `TypeObject::Complete(..).to_bytes_le`
//! for the octets a ZeroDDS writer registers and a bridge's `type_file`
//! holds.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5 and Annex B for the COMPLETE object of
//! `@final struct KeyedPointA { @key long id; long value; };`, written out by
//! hand below; §7.2.2.4.4.4.8 ("Key members ... shall always have their
//! 'must understand' attribute set to true"); Annex B `MemberFlag`
//! (`TRY_CONSTRUCT1` = bit 0 for DISCARD, `IS_MUST_UNDERSTAND` = bit 4,
//! `IS_KEY` = bit 5).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::TypeObject;
use zerodds_types::type_object::CompleteTypeObject;
use zerodds_types::type_object::flags::StructMemberFlag;

/// The COMPLETE `TypeObject` of `KeyedPointA`, XCDR2 little endian:
///
/// ```text
/// 54000000              DHEADER of the TypeObject: 84
/// f2 51 0100            EK_COMPLETE, TK_STRUCTURE, struct_flags IS_FINAL
/// 14000000              DHEADER of CompleteStructHeader: 20
///   00 00 00 00         base_type TK_NONE, ann_builtin and ann_custom absent, pad
///   0c000000 "KeyedPointA\0"
/// 34000000 02000000     DHEADER of the member sequence: 52; two members
///   11000000            DHEADER of member 0: 17
///     00000000 3100 04  id 0, flags T1|M|K, TK_INT32
///     00 03000000 "id\0" 00 00
///   000000              pad
///   14000000            DHEADER of member 1: 20
///     01000000 0100 04  id 1, flags T1, TK_INT32
///     00 06000000 "value\0" 00 00
/// ```
const KEYED_POINT_A_COMPLETE: &str = "54000000f251010014000000000000000c0000004b65796564506f696e74410034000000020000001100000000000000310004000300000069640000000000001400000001000000010004000600000076616c7565000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn xtypes_7_3_4_5_the_keyed_point_type_object_is_the_annex_b_octets() {
    assert_eq!(
        zerodds_test_vendor_reader::keyed_point_a_type_object(),
        unhex(KEYED_POINT_A_COMPLETE)
    );
}

#[test]
fn xtypes_7_2_2_4_4_4_8_the_key_member_is_marked_key_and_must_understand() {
    let bytes = zerodds_test_vendor_reader::keyed_point_a_type_object();
    let TypeObject::Complete(CompleteTypeObject::Struct(object)) =
        TypeObject::from_bytes_le(&bytes).unwrap()
    else {
        panic!("a complete structure");
    };
    assert_eq!(object.header.detail.type_name, "KeyedPointA");
    let id = object.member_seq[0].common.member_flags;
    assert_eq!(object.member_seq[0].detail.name, "id");
    assert!(id.has(StructMemberFlag::IS_KEY));
    assert!(id.has(StructMemberFlag::IS_MUST_UNDERSTAND));
    let value = object.member_seq[1].common.member_flags;
    assert_eq!(object.member_seq[1].detail.name, "value");
    assert!(!value.has(StructMemberFlag::IS_KEY));
    assert!(!value.has(StructMemberFlag::IS_MUST_UNDERSTAND));
}
