// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-xml` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/typeobject_bridge.rs`, the
//! XML type representation of XTypes 1.3 §7.3.2 turned into TypeObjects):
//! - `builder::TypeObjectBuilder` for structures (`struct_type`,
//!   `extensibility`, `nested`, `autoid_hash`, `base`, and per member `key`,
//!   `optional`, `must_understand`, `external`, `try_construct`, `id`), for
//!   enumerations (`enum_type`, `bit_bound`, `literal`, `default_literal`),
//!   unions (`union_type`, `case`, `default_case`), aliases, bitmasks
//!   (`bitmask`, `bit_bound`, `flag`) and bitsets (`bitset`, `field`), with
//!   `build_minimal` and `build_complete`.
//! - Hand-built minimal enumeration and union objects
//!   (`MinimalEnumeratedType`, `MinimalUnionType`, `CommonUnionMember`,
//!   `type_object::common::NameHash::from_name`, the flag types).
//! - Member identifiers: `TypeIdentifier::Primitive`, `String8Small`/`Large`
//!   and `String16Small`/`Large` split at a bound of 255, the plain
//!   sequence, array and map identifiers split the same way, with
//!   `PlainCollectionHeader` and `CollectionElementFlag`, and
//!   `EquivalenceHashMinimal`/`EquivalenceHashComplete` of a named type from
//!   `hash::{compute_minimal_hash, compute_complete_hash}`.
//!
//! Oracle: DDS-XTypes 1.3 Annex B (the TypeIdentifier union, `SBound` an
//! octet and `LBound` an unsigned long, `PlainCollectionHeader`, `NameHash`
//! with its example "color" = {0x70, 0xDD, 0xA5, 0xDF}, `MemberFlag` and
//! `TypeFlag` bit positions), §7.3.1.2.1.1 (sequential member ids start at
//! zero), §7.2.2.4.4.4.6 and §7.2.2.4.4.4.8 (a union discriminator and a key
//! member must be understood), §7.3.4.6.1 (a plain collection of a fully
//! descriptive element carries `EK_BOTH`), and the MINIMAL and COMPLETE octets
//! and hashes CycloneDDS 11.0.1, Fast DDS 3.6.1 and RTI Connext 7.7.0 give for
//! `module to { @final struct Plain { long a; short b; }; }`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`).
#![allow(clippy::unwrap_used)]

use zerodds_types::builder::{Extensibility, TryConstruct, TypeObjectBuilder};
use zerodds_types::hash::{compute_complete_hash, compute_minimal_hash};
use zerodds_types::type_identifier::CollectionElementFlag;
use zerodds_types::type_identifier::kinds::{EK_BOTH, TK_INT32};
use zerodds_types::type_object::common::NameHash;
use zerodds_types::type_object::flags::{
    EnumLiteralFlag, StructMemberFlag, StructTypeFlag, UnionDiscriminatorFlag, UnionMemberFlag,
};
use zerodds_types::type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};
use zerodds_types::{EquivalenceHash, PlainCollectionHeader, PrimitiveKind, TypeIdentifier};

const PLAIN_MINIMAL: &str = "33000000f1510100010000000000000023000000020000000b000000000000000100040cc175b9000b0000000100000001000392eb5ffe";
const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn long() -> TypeIdentifier {
    TypeIdentifier::Primitive(PrimitiveKind::Int32)
}

#[test]
fn annex_b_the_struct_builder_writes_the_vendors_octets_in_both_forms() {
    let plain = TypeObjectBuilder::struct_type("to::Plain")
        .extensibility(Extensibility::Final)
        .member("a", long(), |m| m)
        .member("b", TypeIdentifier::Primitive(PrimitiveKind::Int16), |m| m);
    let minimal = plain.build_minimal();
    let complete = plain.build_complete();
    assert_eq!(
        TypeObject::Minimal(MinimalTypeObject::Struct(minimal.clone()))
            .to_bytes_le()
            .unwrap(),
        unhex(PLAIN_MINIMAL)
    );
    assert_eq!(
        TypeObject::Complete(CompleteTypeObject::Struct(complete.clone()))
            .to_bytes_le()
            .unwrap(),
        unhex(PLAIN_COMPLETE)
    );
    // The identifiers a reference to the type gets in the library pass.
    assert_eq!(
        compute_minimal_hash(&MinimalTypeObject::Struct(minimal)).unwrap(),
        EquivalenceHash([
            0xde, 0xb9, 0x22, 0xf0, 0x40, 0xf7, 0xc2, 0x28, 0xb2, 0x57, 0x28, 0xbe, 0x51, 0xf6
        ])
    );
    assert_eq!(
        compute_complete_hash(&CompleteTypeObject::Struct(complete)).unwrap(),
        EquivalenceHash([
            0x7c, 0x0c, 0x6f, 0xab, 0x7d, 0x06, 0x6a, 0x0c, 0x56, 0xb8, 0xe9, 0x3c, 0x0e, 0xa7
        ])
    );
}

#[test]
fn annex_b_the_member_and_type_attributes_the_xml_carries_become_flags() {
    let base = TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([7; 14]));
    let s = TypeObjectBuilder::struct_type("m::S")
        .extensibility(Extensibility::Mutable)
        .nested()
        .autoid_hash()
        .base(base.clone())
        .member("k", long(), |m| m.key())
        .member("o", long(), |m| m.optional())
        .member("u", long(), |m| m.must_understand())
        .member("x", long(), |m| m.external())
        .member("d", long(), |m| m.try_construct(TryConstruct::UseDefault))
        .member("t", long(), |m| m.try_construct(TryConstruct::Trim))
        .member("i", long(), |m| m.id(42))
        .build_complete();
    assert_eq!(
        s.struct_flags.0,
        StructTypeFlag::IS_MUTABLE | StructTypeFlag::IS_NESTED | StructTypeFlag::IS_AUTOID_HASH
    );
    assert_eq!(s.header.base_type, base);
    let flags: Vec<u16> = s
        .member_seq
        .iter()
        .map(|m| m.common.member_flags.0)
        .collect();
    let t1 = StructMemberFlag::TRY_CONSTRUCT1;
    assert_eq!(
        flags,
        [
            t1 | StructMemberFlag::IS_MUST_UNDERSTAND | StructMemberFlag::IS_KEY,
            t1 | StructMemberFlag::IS_OPTIONAL,
            t1 | StructMemberFlag::IS_MUST_UNDERSTAND,
            t1 | StructMemberFlag::IS_EXTERNAL,
            StructMemberFlag::TRY_CONSTRUCT2,
            t1 | StructMemberFlag::TRY_CONSTRUCT2,
            t1,
        ]
    );
    assert_eq!(s.member_seq[6].common.member_id, 42);
}

#[test]
fn xtypes_7_3_1_2_1_1_sequential_member_ids_start_at_zero() {
    let s = TypeObjectBuilder::struct_type("m::S")
        .member("a", long(), |m| m)
        .member("b", long(), |m| m)
        .build_minimal();
    let ids: Vec<u32> = s.member_seq.iter().map(|m| m.common.member_id).collect();
    assert_eq!(ids, [0, 1]);
    let u = TypeObjectBuilder::union_type("m::U", long())
        .case("a", long(), vec![1])
        .default_case("z", TypeIdentifier::Primitive(PrimitiveKind::Byte))
        .build_minimal();
    let ids: Vec<u32> = u.member_seq.iter().map(|m| m.common.member_id).collect();
    assert_eq!(ids, [0, 1]);
    // §7.2.2.4.4.4.6: the discriminator is always must-understand.
    assert!(
        u.discriminator
            .common
            .member_flags
            .has(UnionDiscriminatorFlag::IS_MUST_UNDERSTAND)
    );
    // The default case is marked; its member flags keep TRY_CONSTRUCT1.
    assert_eq!(
        u.member_seq[1].common.member_flags.0,
        UnionMemberFlag::TRY_CONSTRUCT1 | UnionMemberFlag::IS_DEFAULT
    );
    assert!(u.member_seq[1].common.label_seq.is_empty());
}

#[test]
fn annex_b_the_name_hash_is_the_first_four_md5_octets() {
    assert_eq!(
        NameHash::from_name("color"),
        NameHash([0x70, 0xdd, 0xa5, 0xdf])
    );
}

#[test]
fn annex_b_strings_switch_from_the_small_to_the_large_form_above_255() {
    assert_eq!(
        TypeIdentifier::String8Small { bound: 255 }
            .to_bytes_le()
            .unwrap(),
        [0x70, 0xff]
    );
    // `LBound` is an unsigned long, aligned to four from the identifier's
    // first octet.
    assert_eq!(
        TypeIdentifier::String8Large { bound: 256 }
            .to_bytes_le()
            .unwrap(),
        [0x71, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00]
    );
    assert_eq!(
        TypeIdentifier::String16Small { bound: 0 }
            .to_bytes_le()
            .unwrap(),
        [0x72, 0x00]
    );
    assert_eq!(
        TypeIdentifier::String16Large { bound: 300 }
            .to_bytes_le()
            .unwrap(),
        [0x73, 0x00, 0x00, 0x00, 0x2c, 0x01, 0x00, 0x00]
    );
}

#[test]
fn xtypes_7_3_4_6_1_a_plain_collection_of_a_fully_descriptive_element_carries_ek_both() {
    // The header §7.3.4.6.1 asks for, with the element flags the vendors
    // write (TryConstruct DISCARD).
    let header = PlainCollectionHeader::for_element(EK_BOTH);
    assert_eq!(header.equiv_kind, 0xF3);
    assert_eq!(header.element_flags, CollectionElementFlag::discard());
    let sequence = TypeIdentifier::PlainSequenceSmall {
        header,
        bound: 0,
        element: Box::new(long()),
    };
    assert_eq!(
        sequence.to_bytes_le().unwrap(),
        [0x80, 0xF3, 0x01, 0x00, 0x00, TK_INT32]
    );
    // `Default` is not that header: an EquivalenceKind of zero is none of
    // EK_MINIMAL, EK_COMPLETE and EK_BOTH.
    let default = PlainCollectionHeader::default();
    assert_eq!(default.equiv_kind, 0x00);
    assert_ne!(default, header);
    // An array of more than 255 elements in one dimension is the large form,
    // its dimensions `LBound`s.
    let array = TypeIdentifier::PlainArrayLarge {
        header,
        array_bounds: vec![256],
        element: Box::new(long()),
    };
    assert_eq!(
        array.to_bytes_le().unwrap(),
        [
            0x91, 0xF3, 0x01, 0x00, // discriminator, header
            0x01, 0x00, 0x00, 0x00, // one dimension
            0x00, 0x01, 0x00, 0x00, // 256
            TK_INT32,
        ]
    );
}

#[test]
fn annex_b_enumerations_bitmasks_and_bitsets_keep_what_the_xml_declares() {
    let e = TypeObjectBuilder::enum_type("m::E")
        .bit_bound(16)
        .literal("A", 0)
        .default_literal("B", 5)
        .build_complete();
    assert_eq!(e.header.common.bit_bound, 16);
    let literals: Vec<(i32, bool)> = e
        .literal_seq
        .iter()
        .map(|l| {
            (
                l.common.value,
                l.common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL != 0,
            )
        })
        .collect();
    assert_eq!(literals, [(0, false), (5, true)]);

    let flags = TypeObjectBuilder::bitmask("m::F")
        .bit_bound(8)
        .flag("R", 0)
        .flag("X", 5)
        .build_minimal();
    assert_eq!(flags.bit_bound, 8);
    let positions: Vec<u16> = flags.flag_seq.iter().map(|f| f.common.position).collect();
    assert_eq!(positions, [0, 5]);

    let bits = TypeObjectBuilder::bitset("m::B")
        .field("low", 0, 3, zerodds_types::type_identifier::kinds::TK_UINT8)
        .field(
            "high",
            3,
            5,
            zerodds_types::type_identifier::kinds::TK_UINT8,
        )
        .build_minimal();
    let fields: Vec<(u16, u8)> = bits
        .field_seq
        .iter()
        .map(|f| (f.common.position, f.common.bitcount))
        .collect();
    assert_eq!(fields, [(0, 3), (3, 5)]);

    let alias = TypeObjectBuilder::alias("m::Meters", long()).build_minimal();
    assert_eq!(alias.body.common.related_type, long());
}
