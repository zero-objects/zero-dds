// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-rtps` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`, from its non-test sources
//! (`src/publication_data.rs`, `src/subscription_data.rs`,
//! `src/topic_data.rs`, `src/type_v1.rs`, `src/qos_bridge.rs`):
//! - `TypeIdentifier` as a public field of the publication and
//!   subscription data, `TypeIdentifier::None` as "no identity", its
//!   `Default` after a parameter that does not decode, and `encode_into` /
//!   `decode_from` through `framing::xcdr2_writer` / `xcdr2_reader` for the
//!   vendor parameter `PID_ZERODDS_TYPE_ID`.
//! - `type_information::TypeInformation::from_bytes(value, endianness)` on
//!   the `PID_TYPE_INFORMATION` of a foreign announcement, in the byte order
//!   of the enclosing parameter list, and `preferred_type_identifier`.
//! - `qos::TypeConsistencyEnforcement` (`to_bytes(true)`, `from_bytes`,
//!   `not_announced`, `default`) for `PID_TYPE_CONSISTENCY_ENFORCEMENT`.
//! - `type_object_v1::{TypeId, TypeObjectV1}` (`to_bytes_le`,
//!   `from_bytes(value, big_endian)` called with `!little_endian`) and
//!   `type_object_v1::convert::to_complete(..).roots`, whose first entry
//!   identifies an endpoint that announces only a `TypeObjectV1`.
//!
//! Oracles: DDS-XTypes 1.3 §7.3.4.2 and Annex B (the `TypeIdentifier`
//! union and its discriminators), §7.4.3.4 (DHEADER, EMHEADER and the
//! length codes), §7.6.3.2.2 (`TypeInformation`), §7.6.3.4.1 and §7.6.3.4.3
//! (`TypeConsistencyEnforcementQosPolicy`); DDS-XTypes 1.1 Annex B (the
//! `_TypeId` union over the 16-bit `TypeKind`; the 1.1 text is not in this
//! repository, its IDL is quoted from memory and cross-read against
//! `zerodds_types::type_object_v1`'s module documentation). Vendor
//! recordings from `crates/discovery/tests/fixtures/vendor-discovery/`
//! (Cyclone DDS 11.0.1, RTI Connext 7.7.0, OpenDDS 3.34.0), copied here by
//! hand with each field annotated.
#![allow(clippy::unwrap_used)]

use zerodds_types::TypeIdentifier;
use zerodds_types::framing::{xcdr2_reader, xcdr2_writer};
use zerodds_types::qos::{TypeConsistencyEnforcement, TypeConsistencyKind};
use zerodds_types::type_identifier::{EquivalenceHash, PrimitiveKind};
use zerodds_types::type_information::TypeInformation;
use zerodds_types::type_object_v1::convert::to_complete;
use zerodds_types::type_object_v1::{TypeId, TypeLibrary, TypeObjectV1, kind};
use zerodds_wire::Endianness;

// ---------------------------------------------------------------------
// TypeIdentifier in PID_ZERODDS_TYPE_ID
// ---------------------------------------------------------------------

fn encode(id: &TypeIdentifier) -> Vec<u8> {
    let mut w = xcdr2_writer();
    id.encode_into(&mut w).unwrap();
    w.finish()
}

fn decode(bytes: &[u8]) -> Result<TypeIdentifier, zerodds_wire::WireError> {
    let mut r = xcdr2_reader(bytes);
    TypeIdentifier::decode_from(&mut r)
}

#[test]
fn xtypes_7_3_4_2_a_primitive_identifier_is_its_discriminator_alone() {
    // Annex B: TK_INT64 = 0x05, TK_UINT32 = 0x07, TK_BOOLEAN = 0x01; the
    // primitive cases of the union have no member.
    for (kind, octet) in [
        (PrimitiveKind::Int64, 0x05),
        (PrimitiveKind::UInt32, 0x07),
        (PrimitiveKind::Boolean, 0x01),
    ] {
        let id = TypeIdentifier::Primitive(kind);
        assert_eq!(encode(&id), [octet], "{kind:?}");
        assert_eq!(decode(&[octet]).unwrap(), id);
    }
    // TK_NONE = 0x00.
    assert_eq!(decode(&[0x00]).unwrap(), TypeIdentifier::None);
}

#[test]
fn xtypes_7_3_4_2_a_hashed_identifier_is_the_kind_octet_and_fourteen_hash_octets() {
    let hash = [
        0x34, 0xea, 0xc3, 0x10, 0xf6, 0x74, 0x97, 0x6a, 0x57, 0x55, 0xaf, 0x4d, 0xb9, 0xad,
    ];
    let minimal = TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(hash));
    let mut expected = vec![0xf1]; // EK_MINIMAL
    expected.extend_from_slice(&hash);
    assert_eq!(encode(&minimal), expected);
    assert_eq!(decode(&expected).unwrap(), minimal);

    let complete = TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(hash));
    expected[0] = 0xf2; // EK_COMPLETE
    assert_eq!(encode(&complete), expected);
    assert_eq!(decode(&expected).unwrap(), complete);
}

#[test]
fn a_type_identifier_that_does_not_decode_leaves_the_default_none() {
    // publication_data.rs and subscription_data.rs read the parameter with
    // `decode_from(..).ok()` and fall back to `unwrap_or_default()`.
    assert_eq!(TypeIdentifier::default(), TypeIdentifier::None);
    // EK_MINIMAL with three of its fourteen hash octets.
    assert!(decode(&[0xf1, 0x01, 0x02, 0x03]).is_err());
    assert!(decode(&[]).is_err());
}

// ---------------------------------------------------------------------
// TypeInformation in PID_TYPE_INFORMATION
// ---------------------------------------------------------------------

const RTI_MINIMAL_HASH: [u8; 14] = [
    0x34, 0xea, 0xc3, 0x10, 0xf6, 0x74, 0x97, 0x6a, 0x57, 0x55, 0xaf, 0x4d, 0xb9, 0xad,
];
const RTI_COMPLETE_HASH: [u8; 14] = [
    0x30, 0x4b, 0x20, 0x44, 0x52, 0xf9, 0x2b, 0x64, 0x5f, 0x9c, 0xc2, 0x08, 0x03, 0x00,
];

/// The `PID_TYPE_INFORMATION` value RTI Connext 7.7.0 (`rtiddsping`) sent,
/// `rti.frames`. Each member's EMHEADER has length code 5: NEXTINT is the
/// member's own DHEADER.
#[rustfmt::skip]
const RTI_TYPE_INFORMATION: [u8; 92] = [
    0x58, 0x00, 0x00, 0x00, // DHEADER of TypeInformation: 88
    0x01, 0x10, 0x00, 0x50, // EMHEADER: M = 0, LC = 5, member id 0x1001 (minimal)
    0x24, 0x00, 0x00, 0x00, // NEXTINT = DHEADER of TypeIdentifierWithDependencies: 36
    0x14, 0x00, 0x00, 0x00, // DHEADER of TypeIdentifierWithSize: 20
    0xf1, // EK_MINIMAL
    0x34, 0xea, 0xc3, 0x10, 0xf6, 0x74, 0x97, 0x6a, 0x57, 0x55, 0xaf, 0x4d, 0xb9, 0xad,
    0x00, // padding to four
    0x28, 0x00, 0x00, 0x00, // typeobject_serialized_size: 40
    0x00, 0x00, 0x00, 0x00, // dependent_typeid_count: 0
    0x04, 0x00, 0x00, 0x00, // DHEADER of dependent_typeids: 4
    0x00, 0x00, 0x00, 0x00, // sequence length: 0
    0x02, 0x10, 0x00, 0x50, // EMHEADER: M = 0, LC = 5, member id 0x1002 (complete)
    0x24, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00,
    0xf2, // EK_COMPLETE
    0x30, 0x4b, 0x20, 0x44, 0x52, 0xf9, 0x2b, 0x64, 0x5f, 0x9c, 0xc2, 0x08, 0x03, 0x00,
    0x00,
    0x45, 0x00, 0x00, 0x00, // typeobject_serialized_size: 69
    0x00, 0x00, 0x00, 0x00,
    0x04, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

/// `RTI_TYPE_INFORMATION` in big-endian order, converted by hand: every
/// DHEADER, EMHEADER and `unsigned long` reversed, the octets of the
/// discriminators and hashes as they are.
#[rustfmt::skip]
const RTI_TYPE_INFORMATION_BIG_ENDIAN: [u8; 92] = [
    0x00, 0x00, 0x00, 0x58,
    0x50, 0x00, 0x10, 0x01,
    0x00, 0x00, 0x00, 0x24,
    0x00, 0x00, 0x00, 0x14,
    0xf1,
    0x34, 0xea, 0xc3, 0x10, 0xf6, 0x74, 0x97, 0x6a, 0x57, 0x55, 0xaf, 0x4d, 0xb9, 0xad,
    0x00,
    0x00, 0x00, 0x00, 0x28,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x04,
    0x00, 0x00, 0x00, 0x00,
    0x50, 0x00, 0x10, 0x02,
    0x00, 0x00, 0x00, 0x24,
    0x00, 0x00, 0x00, 0x14,
    0xf2,
    0x30, 0x4b, 0x20, 0x44, 0x52, 0xf9, 0x2b, 0x64, 0x5f, 0x9c, 0xc2, 0x08, 0x03, 0x00,
    0x00,
    0x00, 0x00, 0x00, 0x45,
    0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x04,
    0x00, 0x00, 0x00, 0x00,
];

/// The `PID_TYPE_INFORMATION` value Cyclone DDS 11.0.1 (`ddsperf`) sent for
/// a type with one dependency, `cyclone.frames`. Length code 4: NEXTINT is
/// the member length and the member's DHEADER follows it.
#[rustfmt::skip]
const CYCLONE_TYPE_INFORMATION: [u8; 148] = [
    0x90, 0x00, 0x00, 0x00, // DHEADER of TypeInformation: 144
    0x01, 0x10, 0x00, 0x40, // EMHEADER: M = 0, LC = 4, member id 0x1001
    0x40, 0x00, 0x00, 0x00, // NEXTINT: 64
    0x3c, 0x00, 0x00, 0x00, // DHEADER of TypeIdentifierWithDependencies: 60
    0x14, 0x00, 0x00, 0x00, // DHEADER of TypeIdentifierWithSize: 20
    0xf1,
    0xbc, 0xbd, 0x72, 0x87, 0xb6, 0xcd, 0xa0, 0x71, 0x6b, 0xfb, 0xe0, 0x54, 0xe1, 0xd3,
    0x00,
    0x00, 0x00, 0x00, 0x00, // typeobject_serialized_size: 0
    0x01, 0x00, 0x00, 0x00, // dependent_typeid_count: 1
    0x1c, 0x00, 0x00, 0x00, // DHEADER of dependent_typeids: 28
    0x01, 0x00, 0x00, 0x00, // sequence length: 1
    0x14, 0x00, 0x00, 0x00, // DHEADER of the dependency's TypeIdentifierWithSize
    0xf1,
    0x91, 0xf3, 0x54, 0xb8, 0xe1, 0x34, 0xe4, 0x2f, 0x8d, 0x51, 0x3e, 0x31, 0x6e, 0x88,
    0x00,
    0x47, 0x00, 0x00, 0x00, // its size: 71
    0x02, 0x10, 0x00, 0x40, // EMHEADER: member id 0x1002
    0x40, 0x00, 0x00, 0x00,
    0x3c, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00,
    0xf2,
    0x7c, 0x5d, 0x32, 0x86, 0xad, 0x2c, 0xf5, 0x8e, 0x2a, 0x6f, 0x07, 0xe3, 0xf8, 0x9e,
    0x00,
    0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00,
    0x1c, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00,
    0xf2,
    0x83, 0x83, 0xdd, 0xd1, 0x57, 0x23, 0xcf, 0x04, 0xf5, 0x87, 0x96, 0xfd, 0x2f, 0xb9,
    0x00,
    0x74, 0x00, 0x00, 0x00,
];

/// The `PID_TYPE_INFORMATION` value OpenDDS 3.34.0 sent, `opendds.frames`:
/// a minimal hash and a complete side that is `TK_NONE`.
#[rustfmt::skip]
const OPENDDS_TYPE_INFORMATION: [u8; 88] = [
    0x54, 0x00, 0x00, 0x00, // DHEADER: 84
    0x01, 0x10, 0x00, 0x40, // EMHEADER: LC = 4, member id 0x1001
    0x28, 0x00, 0x00, 0x00, // NEXTINT: 40
    0x24, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00,
    0xf1,
    0x3a, 0x25, 0xb0, 0x69, 0x69, 0xdd, 0x08, 0xcd, 0xeb, 0x62, 0x11, 0x98, 0x93, 0xaf,
    0x00,
    0x67, 0x00, 0x00, 0x00, // typeobject_serialized_size: 103
    0xff, 0xff, 0xff, 0xff, // dependent_typeid_count: -1 (unknown)
    0x04, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
    0x02, 0x10, 0x00, 0x40, // EMHEADER: LC = 4, member id 0x1002
    0x1c, 0x00, 0x00, 0x00, // NEXTINT: 28
    0x18, 0x00, 0x00, 0x00, // DHEADER of TypeIdentifierWithDependencies: 24
    0x08, 0x00, 0x00, 0x00, // DHEADER of TypeIdentifierWithSize: 8
    0x00, 0x00, 0x00, 0x00, // TK_NONE and three octets of padding
    0x00, 0x00, 0x00, 0x00, // typeobject_serialized_size: 0
    0x00, 0x00, 0x00, 0x00,
    0x04, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

/// A member whose `TypeIdentifier` is `TK_NONE`, written by hand after the
/// OpenDDS complete member (§7.4.3.4, LC = 4), with the member id given.
fn none_member(id: u8) -> [u8; 36] {
    #[rustfmt::skip]
    let m = [
        id, 0x10, 0x00, 0x40,
        0x1c, 0x00, 0x00, 0x00,
        0x18, 0x00, 0x00, 0x00,
        0x08, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
        0x04, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    m
}

#[test]
fn xtypes_7_6_3_2_2_rti_type_information_prefers_the_minimal_hash() {
    let info = TypeInformation::from_bytes(&RTI_TYPE_INFORMATION, Endianness::Little).unwrap();
    assert_eq!(
        info.preferred_type_identifier(),
        Some(TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
            RTI_MINIMAL_HASH
        )))
    );
    assert_eq!(
        info.complete.typeid_with_size.type_id,
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(RTI_COMPLETE_HASH))
    );
}

#[test]
fn xtypes_7_6_3_2_2_type_information_follows_the_byte_order_of_the_parameter_list() {
    let big =
        TypeInformation::from_bytes(&RTI_TYPE_INFORMATION_BIG_ENDIAN, Endianness::Big).unwrap();
    let little = TypeInformation::from_bytes(&RTI_TYPE_INFORMATION, Endianness::Little).unwrap();
    assert_eq!(big, little);
    assert_eq!(big.minimal.typeid_with_size.typeobject_serialized_size, 40);
    assert_eq!(big.complete.typeid_with_size.typeobject_serialized_size, 69);
}

#[test]
fn xtypes_7_4_3_4_cyclone_type_information_with_length_code_4_and_a_dependency() {
    let info = TypeInformation::from_bytes(&CYCLONE_TYPE_INFORMATION, Endianness::Little).unwrap();
    let own = [
        0xbc, 0xbd, 0x72, 0x87, 0xb6, 0xcd, 0xa0, 0x71, 0x6b, 0xfb, 0xe0, 0x54, 0xe1, 0xd3,
    ];
    let dependency = [
        0x91, 0xf3, 0x54, 0xb8, 0xe1, 0x34, 0xe4, 0x2f, 0x8d, 0x51, 0x3e, 0x31, 0x6e, 0x88,
    ];
    // The type's own minimal hash, not the one of its dependency.
    assert_eq!(
        info.preferred_type_identifier(),
        Some(TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(own)))
    );
    assert_eq!(info.minimal.dependent_typeid_count, 1);
    assert_eq!(info.minimal.dependent_typeids.len(), 1);
    assert_eq!(
        info.minimal.dependent_typeids[0].type_id,
        TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(dependency))
    );
    assert_eq!(
        info.minimal.dependent_typeids[0].typeobject_serialized_size,
        71
    );
}

#[test]
fn xtypes_7_6_3_2_2_a_tk_none_side_is_absent_and_the_other_side_is_preferred() {
    // OpenDDS: minimal hash, complete TK_NONE.
    let info = TypeInformation::from_bytes(&OPENDDS_TYPE_INFORMATION, Endianness::Little).unwrap();
    assert_eq!(
        info.preferred_type_identifier(),
        Some(TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([
            0x3a, 0x25, 0xb0, 0x69, 0x69, 0xdd, 0x08, 0xcd, 0xeb, 0x62, 0x11, 0x98, 0x93, 0xaf,
        ])))
    );
    assert_eq!(info.minimal.dependent_typeid_count, -1);
    assert_eq!(info.complete.typeid_with_size.type_id, TypeIdentifier::None);

    // Minimal TK_NONE, complete hash: the complete identifier is taken.
    let mut swapped = vec![0x54, 0x00, 0x00, 0x00];
    swapped.extend_from_slice(&none_member(0x01));
    swapped.extend_from_slice(&OPENDDS_TYPE_INFORMATION[4..52]);
    swapped[40] = 0x02; // member id 0x1002
    swapped[56] = 0xf2; // EK_COMPLETE
    let info = TypeInformation::from_bytes(&swapped, Endianness::Little).unwrap();
    assert_eq!(info.minimal.typeid_with_size.type_id, TypeIdentifier::None);
    assert_eq!(
        info.preferred_type_identifier(),
        Some(TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([
            0x3a, 0x25, 0xb0, 0x69, 0x69, 0xdd, 0x08, 0xcd, 0xeb, 0x62, 0x11, 0x98, 0x93, 0xaf,
        ])))
    );

    // Both TK_NONE: no identity at all.
    let mut neither = vec![0x48, 0x00, 0x00, 0x00];
    neither.extend_from_slice(&none_member(0x01));
    neither.extend_from_slice(&none_member(0x02));
    let info = TypeInformation::from_bytes(&neither, Endianness::Little).unwrap();
    assert_eq!(info.preferred_type_identifier(), None);
}

#[test]
fn a_truncated_type_information_is_an_error() {
    // The caller keeps the raw blob and the identifier stays `None`.
    assert!(TypeInformation::from_bytes(&RTI_TYPE_INFORMATION[..60], Endianness::Little).is_err());
    assert!(TypeInformation::from_bytes(&[], Endianness::Little).is_err());
}

// ---------------------------------------------------------------------
// TypeConsistencyEnforcement in PID_TYPE_CONSISTENCY_ENFORCEMENT
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_6_3_4_3_type_consistency_body_is_a_short_and_five_booleans() {
    // RTI Connext 7.7.0 and Fast DDS both write this body for the policy at
    // its defaults (`rti.frames`, `fastdds.frames`): ALLOW_TYPE_COERCION as
    // an unsigned short, then the five booleans, then one octet of padding.
    let recorded = [0x01, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00];
    assert_eq!(
        TypeConsistencyEnforcement::default().to_bytes(true),
        recorded
    );
    assert_eq!(
        TypeConsistencyEnforcement::from_bytes(&recorded, true),
        TypeConsistencyEnforcement::default()
    );
    let big = [0x00, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00];
    assert_eq!(
        TypeConsistencyEnforcement::from_bytes(&big, false),
        TypeConsistencyEnforcement::default()
    );
}

#[test]
fn xtypes_7_6_3_4_1_type_consistency_defaults_present_and_absent() {
    // Present: ALLOW_TYPE_COERCION, ignore_sequence_bounds and
    // ignore_string_bounds true, the others false.
    let present = TypeConsistencyEnforcement::default();
    assert_eq!(present.kind, TypeConsistencyKind::AllowTypeCoercion);
    assert!(present.ignore_sequence_bounds);
    assert!(present.ignore_string_bounds);
    assert!(!present.ignore_member_names);
    assert!(!present.prevent_type_widening);
    assert!(!present.force_type_validation);
    // Absent: "assume a kind of DISALLOW_TYPE_COERCION", with
    // prevent_type_widening true and the others false.
    let absent = TypeConsistencyEnforcement::not_announced();
    assert_eq!(absent.kind, TypeConsistencyKind::DisallowTypeCoercion);
    assert!(!absent.ignore_sequence_bounds);
    assert!(!absent.ignore_string_bounds);
    assert!(!absent.ignore_member_names);
    assert!(absent.prevent_type_widening);
    assert!(!absent.force_type_validation);
}

#[test]
fn a_type_consistency_kind_annex_d_does_not_define_reads_as_disallow() {
    // subscription_data.rs uses the lenient `from_bytes`.
    let unknown = [0x07, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00];
    assert_eq!(
        TypeConsistencyEnforcement::from_bytes(&unknown, true).kind,
        TypeConsistencyKind::DisallowTypeCoercion
    );
}

// ---------------------------------------------------------------------
// TypeIdV1 and TypeObjectV1 (PID 0x0069, 0x0072)
// ---------------------------------------------------------------------

#[test]
fn xtypes_1_1_type_id_v1_is_a_16_bit_kind_and_its_member_in_the_given_order() {
    // `long`: INT_32_TYPE = 5, the discriminator and the PrimitiveTypeId.
    let id = TypeId::Primitive(kind::INT_32_TYPE);
    let little = [0x05, 0x00, 0x05, 0x00];
    let big = [0x00, 0x05, 0x00, 0x05];
    assert_eq!(id.to_bytes_le().unwrap(), little);
    // type_v1.rs passes `!little_endian`: the flag means big-endian.
    assert_eq!(TypeId::from_bytes(&little, false).unwrap(), id);
    assert_eq!(TypeId::from_bytes(&big, true).unwrap(), id);

    // A constructed id: the kind, padding to eight (XCDR1), the 64 bits.
    let id = TypeId::Constructed {
        kind: kind::STRUCTURE_TYPE,
        id: 0x0102_0304_0506_0708,
    };
    #[rustfmt::skip]
    let little = [
        0x16, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01,
    ];
    #[rustfmt::skip]
    let big = [
        0x00, 0x16, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    ];
    assert_eq!(id.to_bytes_le().unwrap(), little);
    assert_eq!(TypeId::from_bytes(&little, false).unwrap(), id);
    assert_eq!(TypeId::from_bytes(&big, true).unwrap(), id);
}

#[test]
fn a_type_object_v1_written_little_endian_reads_back_with_big_endian_false() {
    // type_v1.rs writes `to_bytes_le` and reads with `from_bytes(.., false)`
    // for a little-endian list.
    let object = TypeObjectV1 {
        library: TypeLibrary {
            element: Vec::new(),
        },
        the_type: vec![TypeId::Primitive(kind::UINT_32_TYPE)],
    };
    let little = object.to_bytes_le().unwrap();
    assert_eq!(TypeObjectV1::from_bytes(&little, false).unwrap(), object);
    let big = object.to_bytes_be().unwrap();
    assert_eq!(TypeObjectV1::from_bytes(&big, true).unwrap(), object);
    // A value that is no TypeObjectV1 is refused; the member is left out.
    assert!(TypeObjectV1::from_bytes(&[0x01, 0x02], false).is_err());
}

#[test]
fn xtypes_7_3_4_2_a_v1_primitive_root_converts_to_the_primitive_identifier() {
    // type_v1::identifier_of takes `roots.first()`.
    let object = TypeObjectV1 {
        library: TypeLibrary {
            element: Vec::new(),
        },
        the_type: vec![TypeId::Primitive(kind::INT_64_TYPE)],
    };
    let converted = to_complete(&object).unwrap();
    assert_eq!(
        converted.roots,
        vec![TypeIdentifier::Primitive(PrimitiveKind::Int64)]
    );
    // No type: no root, and the endpoint stays without identity.
    let empty = TypeObjectV1 {
        library: TypeLibrary {
            element: Vec::new(),
        },
        the_type: Vec::new(),
    };
    assert!(to_complete(&empty).unwrap().roots.is_empty());
}
