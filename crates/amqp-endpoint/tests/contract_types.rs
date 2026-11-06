// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-amqp-endpoint` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types` (`src/bridge_daemon.rs`,
//! `key_type_support`, feature `std`): `type_object::TypeObject::from_bytes_le`
//! on the octets of a topic's `type_file`, an error mapped to a
//! configuration error, and the variant: `TypeObject::Complete` is handed to
//! `zerodds-dcps`, `TypeObject::Minimal` is refused.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5 and Annex B (`@appendable union
//! TypeObject` under XCDR2 little endian: a `DHEADER`, the `EquivalenceKind`
//! octet `EK_MINIMAL = 0xF1` or `EK_COMPLETE = 0xF2`, then the object), and
//! the octets CycloneDDS 11.0.1 `idlc` writes for
//! `module to { @final struct Plain { long a; short b; }; }`, recorded in
//! `crates/types/tests/typeobject_vendor_dheader.rs` (RTI Connext 7.7.0 hashes
//! the same 80 COMPLETE octets).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::type_identifier::{PrimitiveKind, TypeIdentifier};
use zerodds_types::type_object::flags::{StructMemberFlag, StructTypeFlag};
use zerodds_types::type_object::{CompleteTypeObject, MinimalTypeObject, TypeObject};

/// CycloneDDS's COMPLETE `TypeObject` of `to::Plain`.
const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
/// CycloneDDS's MINIMAL `TypeObject` of `to::Plain`.
const PLAIN_MINIMAL: &str = "33000000f1510100010000000000000023000000020000000b000000000000000100040cc175b9000b0000000100000001000392eb5ffe";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn xtypes_7_3_4_5_a_complete_type_file_decodes_to_the_complete_object() {
    let object = TypeObject::from_bytes_le(&unhex(PLAIN_COMPLETE)).unwrap();
    let TypeObject::Complete(CompleteTypeObject::Struct(plain)) = &object else {
        panic!("EK_COMPLETE with TK_STRUCTURE is a complete structure: {object:?}");
    };
    assert_eq!(plain.struct_flags, StructTypeFlag(StructTypeFlag::IS_FINAL));
    assert_eq!(plain.header.base_type, TypeIdentifier::None);
    assert_eq!(plain.header.detail.type_name, "to::Plain");
    let members: Vec<_> = plain
        .member_seq
        .iter()
        .map(|m| {
            (
                m.common.member_id,
                m.common.member_type_id.clone(),
                m.detail.name.as_str(),
            )
        })
        .collect();
    assert_eq!(
        members,
        [
            (0, TypeIdentifier::Primitive(PrimitiveKind::Int32), "a"),
            (1, TypeIdentifier::Primitive(PrimitiveKind::Int16), "b"),
        ]
    );
    for member in &plain.member_seq {
        assert_eq!(
            member.common.member_flags,
            StructMemberFlag(StructMemberFlag::TRY_CONSTRUCT1),
            "Annex B MemberFlag: T1 set, T2 clear is DISCARD"
        );
    }
}

#[test]
fn xtypes_7_3_4_5_a_minimal_type_file_decodes_to_the_minimal_variant() {
    // The daemon refuses this variant by name; it must not arrive as
    // `Complete`.
    let object = TypeObject::from_bytes_le(&unhex(PLAIN_MINIMAL)).unwrap();
    assert!(
        matches!(object, TypeObject::Minimal(MinimalTypeObject::Struct(_))),
        "{object:?}"
    );
}

#[test]
fn a_type_file_that_holds_no_type_object_is_an_error() {
    // Empty file: no DHEADER.
    assert!(TypeObject::from_bytes_le(&[]).is_err());
    // A text file: its first four octets read as a DHEADER far past the end.
    assert!(TypeObject::from_bytes_le(b"{\"type\": \"to::Plain\"}").is_err());
    // A truncated object: the DHEADER announces 76 octets, 75 follow.
    let mut truncated = unhex(PLAIN_COMPLETE);
    truncated.pop();
    assert!(TypeObject::from_bytes_le(&truncated).is_err());
    // An EquivalenceKind Annex B does not define for a TypeObject
    // (EK_BOTH = 0xF3 names no object form).
    let mut both = unhex(PLAIN_COMPLETE);
    both[4] = 0xF3;
    assert!(TypeObject::from_bytes_le(&both).is_err());
}
