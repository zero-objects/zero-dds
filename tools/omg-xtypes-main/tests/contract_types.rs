// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-omg-xtypes-main` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`: the 25 generated shape types of
//! `src/shape_type.rs` (`include!` of the `zerodds-build` output) each carry
//! a `DdsType::TYPE_IDENTIFIER` constant
//! `TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([..]))`, a
//! `type_object()` that is `type_object::TypeObject::from_bytes_le` over the
//! octets the generator wrote, and for a derived type a
//! `type_object_dependencies()` over the same call; `shape_type.rs` returns
//! them as `[Option<TypeObject>; 25]` and `[TypeIdentifier; 25]`
//! (`variant_type_objects`, `variant_type_identifiers`). The client registers
//! the object and announces the identifier; a peer resolves the one through
//! the other.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5 and Annex B (the COMPLETE object of
//! `@final struct Shape1Final { @key string<32> color; long x; long y; long
//! shapesize; };`, written out by hand below with the same layout rules that
//! reproduce CycloneDDS 11.0.1's octets for `to::Plain`; "First 14 bytes of
//! MD5 of the serialized TypeObject", the MD5 computed outside this tree),
//! RFC 1321 A.5 (MD5 of "abc"), §7.2.2.4.4.4.8 (a key member is
//! must-understand), §7.3.4.6.3 and §7.6.3.3.4 (a base type is named by its
//! hash and served beside the derived type).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_dcps::DdsType;
use zerodds_omg_xtypes_main::shape_type::generated::{Shape1Final, Shape5Final};
use zerodds_omg_xtypes_main::shape_type::{
    VARIANT_NAMES, variant_type_identifiers, variant_type_objects,
};
use zerodds_types::hash::hash_bytes;
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};
use zerodds_types::{EquivalenceHash, TypeIdentifier};

/// `Shape1Final`, COMPLETE, XCDR2 little endian:
///
/// ```text
/// 80000000 f2 51 0100               DHEADER 128, EK_COMPLETE, TK_STRUCTURE, IS_FINAL
/// 14000000 00 00 00 00              header: DHEADER 20, TK_NONE base, no annotations, pad
///   0c000000 "Shape1Final\0"
/// 60000000 04000000                 members: DHEADER 96, four
///   14000000 00000000 3100 70 20    DHEADER 20, id 0, T1|M|K, TI_STRING8_SMALL bound 32
///     06000000 "color\0" 00 00
///   10000000 01000000 0100 04 00    DHEADER 16, id 1, T1, TK_INT32, pad
///     02000000 "x\0" 00 00
///   10000000 02000000 0100 04 00 02000000 "y\0" 00 00
///   18000000 03000000 0100 04 00    DHEADER 24, id 3
///     0a000000 "shapesize\0" 00 00
/// ```
const SHAPE1_FINAL_COMPLETE: &str = "80000000f251010014000000000000000c00000053686170653146696e616c00600000000400000014000000000000003100702006000000636f6c6f72000000100000000100000001000400020000007800000010000000020000000100040002000000790000001800000003000000010004000a000000736861706573697a65000000";
/// MD5 of those 132 octets, first 14.
const SHAPE1_FINAL_HASH: [u8; 14] = [
    0xab, 0xd9, 0x16, 0xdd, 0x91, 0xbe, 0x09, 0x40, 0x3d, 0x41, 0x82, 0x9a, 0x40, 0x15,
];

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn annex_b_shape1final_announces_the_hash_of_its_annex_b_object() {
    let object = <Shape1Final as DdsType>::type_object().unwrap();
    assert_eq!(object.to_bytes_le().unwrap(), unhex(SHAPE1_FINAL_COMPLETE));
    assert_eq!(
        <Shape1Final as DdsType>::TYPE_IDENTIFIER,
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(SHAPE1_FINAL_HASH))
    );
}

#[test]
fn annex_b_every_variant_announces_the_hash_of_the_object_it_registers() {
    // The MD5 behind `hash_bytes`, against RFC 1321 A.5.
    assert_eq!(
        hash_bytes(b"abc").0,
        [
            0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1
        ]
    );
    for ((name, object), identifier) in VARIANT_NAMES
        .iter()
        .zip(variant_type_objects())
        .zip(variant_type_identifiers())
    {
        let object = object.unwrap_or_else(|| panic!("{name}: no TypeObject"));
        assert!(matches!(object, TypeObject::Complete(_)), "{name}");
        let TypeIdentifier::EquivalenceHashComplete(hash) = identifier else {
            panic!("{name}: {identifier:?} is not a complete hash");
        };
        assert_eq!(
            hash,
            hash_bytes(&object.to_bytes_le().unwrap()),
            "{name}: the identifier is not the hash of the object"
        );
    }
}

#[test]
fn xtypes_7_3_4_6_3_a_derived_variant_names_its_base_by_hash_and_registers_it() {
    let Some(TypeObject::Complete(CompleteTypeObject::Struct(derived))) =
        <Shape5Final as DdsType>::type_object()
    else {
        panic!("Shape5Final has a complete structure object");
    };
    assert_eq!(
        derived.header.base_type,
        <Shape1Final as DdsType>::TYPE_IDENTIFIER
    );
    let base = unhex(SHAPE1_FINAL_COMPLETE);
    assert!(
        <Shape5Final as DdsType>::type_object_dependencies()
            .iter()
            .any(|object| object.to_bytes_le().unwrap() == base),
        "the base's object is registered beside the derived one"
    );
}
