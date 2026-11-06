// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-foreign-typeobject-probe` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-types` (`src/main.rs`; a binary without a
//! library, so the contract calls `zerodds-types` directly):
//! - `TypeInformation::from_bytes_le` on the `PID_TYPE_INFORMATION` blob a
//!   peer's writer announced, then `complete.typeid_with_size.type_id` and the
//!   `type_id` of every `complete.dependent_typeids` entry, which it asks the
//!   peer's TypeLookup service for.
//! - `TypeIdentifier::EquivalenceHashComplete`: any other kind of announced
//!   identifier ends the run.
//! - `dynamic::to_ast::dynamic_type` over the complete object the registry
//!   files under that hash, resolving the member types against the same
//!   registry; an error is retried until the deadline, because the objects
//!   of the nested types may arrive later.
//! - `DynamicType::qualified_name`, `DynamicType::extensibility` and
//!   `dynamic::descriptor::ExtensibilityKind::{Appendable, Mutable}`, which
//!   choose the encoding the samples are decoded with.
//!
//! Oracle: DDS-XTypes 1.3 §7.6.3.2.1 and Annex B (`@mutable struct
//! TypeInformation` with members `0x1001` minimal and `0x1002` complete, each
//! an `@appendable TypeIdentifierWithDependencies`, under XCDR2 little endian;
//! the EMHEADER length code 4 is the one CycloneDDS, Fast DDS and OpenDDS
//! write, `crates/types/tests/type_information_length_codes.rs`), §7.4.3.4.2
//! (EMHEADER), the COMPLETE objects and hashes CycloneDDS 11.0.1 gives for
//! `to::Inner`, `to::Appendable` and `to::Mutable`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`), and the COMPLETE
//! object of `@final struct to::Outer { to::Inner i; }` written out by hand
//! from Annex B, its MD5 computed outside this tree.
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::dynamic::descriptor::ExtensibilityKind;
use zerodds_types::dynamic::to_ast::dynamic_type;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, TypeIdentifier, TypeInformation};

const INNER: &str = "4c000000f251010012000000000000000a000000746f3a3a496e6e65720000002c0000000200000010000000000000000100040002000000780000001000000001000000010004000200000079000000";
const INNER_HASH: [u8; 14] = [
    0xf0, 0x03, 0x88, 0xe0, 0xe0, 0xc6, 0x61, 0x9e, 0x41, 0x75, 0xc6, 0x8e, 0x8a, 0xf8,
];
const OUTER: &str = "48000000f251010012000000000000000a000000746f3a3a4f75746572000000280000000100000020000000000000000100f2f00388e0e0c6619e4175c68e8af80000000200000069000000";
const OUTER_HASH: [u8; 14] = [
    0x91, 0x23, 0x71, 0x8c, 0xb7, 0x2c, 0x3e, 0x15, 0xbd, 0xec, 0x18, 0x80, 0x34, 0xf7,
];
const APPENDABLE: &str = "50000000f251020017000000000000000f000000746f3a3a417070656e6461626c6500002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
const MUTABLE: &str = "4c000000f251040014000000000000000c000000746f3a3a4d757461626c65002c00000002000000100000000a0000000100040002000000610000001000000014000000010003000200000062000000";

/// The `TypeInformation` a writer of `to::Outer` announces:
///
/// ```text
/// 78000000                         DHEADER of the @mutable TypeInformation: 120
/// 01100040 28000000                EMHEADER LC 4, id 0x1001; NEXTINT 40
///   24000000                       DHEADER of TypeIdentifierWithDependencies: 36
///     14000000 f1 <14 octets> 00 37000000   minimal TypeIdentifierWithSize, size 55
///     00000000                     dependent_typeid_count 0
///     04000000 00000000            the sequence: DHEADER 4, length 0
/// 02100040 40000000                EMHEADER LC 4, id 0x1002; NEXTINT 64
///   3c000000                       DHEADER: 60
///     14000000 f2 <OUTER_HASH> 00 4c000000  complete, size 76
///     01000000                     dependent_typeid_count 1
///     1c000000 01000000            the sequence: DHEADER 28, length 1
///       14000000 f2 <INNER_HASH> 00 50000000  size 80
/// ```
const OUTER_TYPE_INFORMATION: &str = "7800000001100040280000002400000014000000f10102030405060708090a0b0c0d0e003700000000000000040000000000000002100040400000003c00000014000000f29123718cb72c3e15bdec188034f7004c000000010000001c0000000100000014000000f2f00388e0e0c6619e4175c68e8af80050000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn complete(hex: &str) -> zerodds_types::CompleteTypeObject {
    let TypeObject::Complete(object) = TypeObject::from_bytes_le(&unhex(hex)).unwrap() else {
        panic!("a COMPLETE object");
    };
    object
}

#[test]
fn xtypes_7_6_3_2_1_the_announcement_names_the_complete_type_and_its_dependencies() {
    let information = TypeInformation::from_bytes_le(&unhex(OUTER_TYPE_INFORMATION)).unwrap();
    let root = &information.complete.typeid_with_size;
    assert_eq!(
        root.type_id,
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(OUTER_HASH))
    );
    assert_eq!(root.typeobject_serialized_size, 76);
    let dependents: Vec<&TypeIdentifier> = information
        .complete
        .dependent_typeids
        .iter()
        .map(|with_size| &with_size.type_id)
        .collect();
    assert_eq!(
        dependents,
        [&TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(
            INNER_HASH
        ))]
    );
    assert_eq!(
        information.complete.dependent_typeids[0].typeobject_serialized_size,
        80
    );
    // The minimal side is read too, and is not mistaken for the complete one.
    assert!(matches!(
        information.minimal.typeid_with_size.type_id,
        TypeIdentifier::EquivalenceHashMinimal(_)
    ));
}

#[test]
fn xtypes_7_6_3_2_1_a_blob_cut_short_is_an_error() {
    let bytes = unhex(OUTER_TYPE_INFORMATION);
    assert!(TypeInformation::from_bytes_le(&bytes[..bytes.len() - 1]).is_err());
    assert!(TypeInformation::from_bytes_le(&[]).is_err());
}

#[test]
fn xtypes_7_3_4_3_a_peers_complete_object_becomes_a_type_once_its_dependencies_are_held() {
    let outer = complete(OUTER);
    // The nested type's object has not arrived: no type yet, and the probe
    // asks again.
    assert!(dynamic_type(&outer, &TypeRegistry::new()).is_err());

    let mut registry = TypeRegistry::new();
    let inner = registry
        .insert_object(TypeObject::Complete(complete(INNER)))
        .unwrap();
    assert_eq!(inner, EquivalenceHash(INNER_HASH));
    let ty = dynamic_type(&outer, &registry).unwrap();
    assert_eq!(ty.qualified_name(), "to::Outer");
    assert_eq!(ty.extensibility(), Some(ExtensibilityKind::Final));
    assert_eq!(ty.member_count(), 1);
    assert!(ty.member_by_name("i").is_some());
}

#[test]
fn xtypes_7_3_1_2_1_8_the_extensibility_of_a_peers_type_chooses_the_encoding() {
    let registry = TypeRegistry::new();
    let appendable = dynamic_type(&complete(APPENDABLE), &registry).unwrap();
    assert_eq!(appendable.qualified_name(), "to::Appendable");
    assert_eq!(
        appendable.extensibility(),
        Some(ExtensibilityKind::Appendable)
    );
    let mutable = dynamic_type(&complete(MUTABLE), &registry).unwrap();
    assert_eq!(mutable.qualified_name(), "to::Mutable");
    assert_eq!(mutable.extensibility(), Some(ExtensibilityKind::Mutable));
}
