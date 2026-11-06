// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-discovery` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`, from its non-test sources
//! (`type_lookup/{mod,client,server,endpoints}.rs`; the `sedp/` modules name
//! `zerodds_types` only in their tests):
//! - `type_lookup::{GetTypesRequest, GetTypesReply,
//!   GetTypeDependenciesRequest, GetTypeDependenciesReply,
//!   TypeIdentifierTypeObjectPair, TypeIdentifierPair, ContinuationPoint}`,
//!   written with `encode_into` into `framing::xcdr2_writer()` and read with
//!   `decode_from` from `framing::xcdr2_reader(..)`;
//!   `OPERATION_GET_TYPES`, `OPERATION_GET_DEPENDENCIES` and
//!   `service_instance_name` for the endpoint name.
//! - `resolve::TypeRegistry` (`new`, `insert_object`, `get_minimal`,
//!   `get_complete`, `minimal_hash_of_complete`, `transitive_dependencies`,
//!   the component lookups for strongly connected components) and
//!   `hash::{minimal_serialized_size, complete_serialized_size}` for the
//!   `typeobject_serialized_size` of a dependency.
//! - `TypeIdentifier` (`EquivalenceHashMinimal`, `EquivalenceHashComplete`,
//!   `StronglyConnectedComponent`), `EquivalenceHash`,
//!   `type_information::TypeIdentifierWithSize`, `type_object::TypeObject`,
//!   `error::TypeCodecError`.
//!
//! Oracles: DDS-XTypes 1.3 §7.3.1.2.1.1 (a `@hashid` member id is the first
//! four octets of MD5 of the name, little-endian, masked with 0x0FFFFFFF;
//! the values below were computed with Python's `hashlib`, outside this
//! tree), §7.3.4.5 and Annex B (TypeObject layout), §7.4.3.4 (DHEADER,
//! EMHEADER, LC 5), §7.6.3.3.3 (the TypeLookup types and the published
//! `TypeLookup_getTypes_HashId` = 0x018252d3 and
//! `TypeLookup_getDependencies_HashId` = 0x05aafb31), DDS-XTypes 1.4 Beta 1
//! §7.6.3.3.3 (encoding version 2 for the TypeLookup endpoints). Vendor
//! output: the Cyclone DDS 11.0.1 serializer
//! (`crates/types/tests/fixtures/cyclone-typelookup-11.log`) for the empty
//! request and reply bodies, and Cyclone's `TYPE_INFO_CDR_Robot` hashes for
//! `@final struct Robot { uint32 id; uint32 label; }`
//! (`crates/types/tests/typeobject_serialized_size.rs`): the two TypeObjects
//! below were written by hand from Annex B and their MD5, computed with
//! Python, gives Cyclone's hashes; Cyclone, Fast DDS 3.6.1 and OpenDDS
//! 3.34.0 announce the sizes 55 and 84.
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::framing::{EncodingVersion, xcdr2_reader, xcdr2_writer};
use zerodds_types::hash::{complete_serialized_size, minimal_serialized_size};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_information::TypeIdentifierWithSize;
use zerodds_types::type_lookup::{
    ContinuationPoint, GetTypeDependenciesReply, GetTypeDependenciesRequest, GetTypesReply,
    GetTypesRequest, OPERATION_GET_DEPENDENCIES, OPERATION_GET_TYPES, TypeIdentifierTypeObjectPair,
    service_instance_name,
};
use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, MinimalTypeObject, TypeIdentifier};
use zerodds_wire::{Endianness, Writer};

/// Cyclone's minimal hash of `Robot`.
const ROBOT_MINIMAL_HASH: [u8; 14] = [
    0x62, 0x54, 0x36, 0xec, 0xee, 0x22, 0xcc, 0x29, 0x43, 0xeb, 0xc5, 0x98, 0xc4, 0x3e,
];
/// Cyclone's complete hash of `Robot`.
const ROBOT_COMPLETE_HASH: [u8; 14] = [
    0xaa, 0xd3, 0x35, 0x12, 0xa5, 0xe4, 0x28, 0x68, 0xff, 0x3e, 0x9e, 0x01, 0x68, 0x9a,
];

/// The MINIMAL TypeObject of `Robot`, XCDR2 little-endian, by hand from
/// Annex B; MD5 of these 55 octets begins with `ROBOT_MINIMAL_HASH`.
#[rustfmt::skip]
const ROBOT_MINIMAL: [u8; 55] = [
    0x33, 0x00, 0x00, 0x00, // DHEADER of the appendable union TypeObject: 51
    0xf1,                   // EK_MINIMAL
    0x51,                   // TK_STRUCTURE
    0x01, 0x00,             // StructTypeFlag: IS_FINAL
    0x01, 0x00, 0x00, 0x00, // DHEADER of MinimalStructHeader: 1
    0x00,                   // base_type: TK_NONE (MinimalTypeDetail is empty)
    0x00, 0x00, 0x00,       // padding
    0x23, 0x00, 0x00, 0x00, // DHEADER of member_seq: 35
    0x02, 0x00, 0x00, 0x00, // two members
    0x0b, 0x00, 0x00, 0x00, // DHEADER of MinimalStructMember: 11
    0x00, 0x00, 0x00, 0x00, // member_id 0
    0x01, 0x00,             // StructMemberFlag: TRY_CONSTRUCT1 (DISCARD)
    0x07,                   // TK_UINT32
    0xb8, 0x0b, 0xb7, 0x74, // NameHash: MD5("id")[0..4]
    0x00,                   // padding
    0x0b, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, // member_id 1
    0x01, 0x00,
    0x07,
    0xd3, 0x04, 0xba, 0x20, // MD5("label")[0..4]
];

/// The COMPLETE TypeObject of `Robot`, by hand from Annex B; MD5 of these
/// 84 octets begins with `ROBOT_COMPLETE_HASH`.
#[rustfmt::skip]
const ROBOT_COMPLETE: [u8; 84] = [
    0x50, 0x00, 0x00, 0x00, // DHEADER: 80
    0xf2,                   // EK_COMPLETE
    0x51,                   // TK_STRUCTURE
    0x01, 0x00,             // IS_FINAL
    0x0e, 0x00, 0x00, 0x00, // DHEADER of CompleteStructHeader: 14
    0x00,                   // base_type: TK_NONE
    0x00, 0x00,             // ann_builtin, ann_custom: absent
    0x00,                   // padding
    0x06, 0x00, 0x00, 0x00, b'R', b'o', b'b', b'o', b't', 0x00, // type_name
    0x00, 0x00,             // padding
    0x34, 0x00, 0x00, 0x00, // DHEADER of member_seq: 52
    0x02, 0x00, 0x00, 0x00, // two members
    0x11, 0x00, 0x00, 0x00, // DHEADER of CompleteStructMember: 17
    0x00, 0x00, 0x00, 0x00, // member_id 0
    0x01, 0x00,             // TRY_CONSTRUCT1
    0x07,                   // TK_UINT32
    0x00,                   // padding
    0x03, 0x00, 0x00, 0x00, b'i', b'd', 0x00, // name
    0x00, 0x00,             // ann_builtin, ann_custom: absent
    0x00, 0x00, 0x00,       // padding
    0x14, 0x00, 0x00, 0x00, // DHEADER: 20
    0x01, 0x00, 0x00, 0x00, // member_id 1
    0x01, 0x00,
    0x07,
    0x00,
    0x06, 0x00, 0x00, 0x00, b'l', b'a', b'b', b'e', b'l', 0x00,
    0x00, 0x00,
];

fn object(bytes: &[u8]) -> TypeObject {
    TypeObject::decode_from(&mut xcdr2_reader(bytes)).unwrap()
}

fn robot_minimal() -> MinimalTypeObject {
    match object(&ROBOT_MINIMAL) {
        TypeObject::Minimal(m) => m,
        other => panic!("expected a minimal TypeObject, got {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Constants and names
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_6_3_3_3_operation_ids_are_the_published_hash_ids() {
    assert_eq!(OPERATION_GET_TYPES, 0x0182_52d3);
    assert_eq!(OPERATION_GET_DEPENDENCIES, 0x05aa_fb31);
}

#[test]
fn xtypes_7_6_3_3_4_service_instance_name_is_the_prefix_and_the_participant_guid() {
    let prefix = [
        0x01, 0x0f, 0xab, 0xcd, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x2a,
    ];
    // The participant GUID in hexadecimal: the prefix, then
    // ENTITYID_PARTICIPANT {00,00,01},c1 (DDSI-RTPS 2.5 Table 9.2). Lower
    // case is the spelling endpoints.rs documents for Cyclone's client.
    assert_eq!(
        service_instance_name(&prefix),
        "dds.builtin.TOS.010fabcd000000010000002a000001c1"
    );
}

// ---------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_6_3_3_3_the_typelookup_writer_is_encoding_version_2() {
    // 1.4 Beta 1 §7.6.3.3.3: encoding version 2. The helpers of this crate
    // choose the version by the alignment cap of the writer; a writer of
    // `Writer::new` (cap eight) is version 1.
    assert_eq!(
        EncodingVersion::of_writer(&xcdr2_writer()),
        EncodingVersion::Xcdr2
    );
    assert_eq!(
        EncodingVersion::of_writer(&Writer::new(Endianness::Little)),
        EncodingVersion::Xcdr1
    );
}

#[test]
fn xtypes_7_6_3_3_3_get_types_request_is_a_dheader_and_one_lc5_member() {
    // Cyclone 11.0.1 for no identifiers: DHEADER 12, EMHEADER LC 5 with
    // the @hashid of `type_ids` (0x0c536065), the sequence's DHEADER 4, 0.
    let mut w = xcdr2_writer();
    GetTypesRequest::default().encode_into(&mut w).unwrap();
    assert_eq!(
        w.finish(),
        [
            0x0c, 0x00, 0x00, 0x00, 0x65, 0x60, 0x53, 0x5c, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ]
    );

    // One minimal hash, as TypeLookupStack::make_get_types_request writes.
    let request = GetTypesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
            ROBOT_MINIMAL_HASH,
        ))],
    };
    let mut expected = vec![
        0x1b, 0x00, 0x00, 0x00, // DHEADER: 27
        0x65, 0x60, 0x53, 0x5c, // EMHEADER: LC 5, type_ids
        0x13, 0x00, 0x00, 0x00, // DHEADER of the sequence: 19
        0x01, 0x00, 0x00, 0x00, // one identifier
        0xf1,
    ];
    expected.extend_from_slice(&ROBOT_MINIMAL_HASH);
    let mut w = xcdr2_writer();
    request.encode_into(&mut w).unwrap();
    let bytes = w.finish();
    assert_eq!(bytes, expected);
    assert_eq!(
        GetTypesRequest::decode_from(&mut xcdr2_reader(&bytes)).unwrap(),
        request
    );
}

#[test]
fn xtypes_7_6_3_3_3_get_type_dependencies_request_carries_the_continuation_point() {
    // Cyclone 11.0.1 for no identifiers and no continuation point.
    let mut w = xcdr2_writer();
    GetTypeDependenciesRequest::default()
        .encode_into(&mut w)
        .unwrap();
    assert_eq!(
        w.finish(),
        [
            0x14, 0x00, 0x00, 0x00, 0x65, 0x60, 0x53, 0x5c, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xd2, 0xe3, 0x08, 0x55, 0x00, 0x00, 0x00, 0x00,
        ]
    );

    // One hash and the eight-octet cursor TypeLookupServer hands out.
    let request = GetTypeDependenciesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
            ROBOT_MINIMAL_HASH,
        ))],
        continuation_point: ContinuationPoint(vec![0x42, 0, 0, 0, 0, 0, 0, 0]),
    };
    let mut expected = vec![
        0x2c, 0x00, 0x00, 0x00, // DHEADER: 44
        0x65, 0x60, 0x53, 0x5c, // EMHEADER: LC 5, type_ids
        0x13, 0x00, 0x00, 0x00, // DHEADER of the sequence: 19
        0x01, 0x00, 0x00, 0x00, 0xf1,
    ];
    expected.extend_from_slice(&ROBOT_MINIMAL_HASH);
    expected.extend_from_slice(&[
        0x00, // padding to the next EMHEADER
        0xd2, 0xe3, 0x08, 0x55, // EMHEADER: LC 5, continuation_point (0x0508e3d2)
        0x08, 0x00, 0x00, 0x00, // sequence<octet, 32> length 8
        0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    let mut w = xcdr2_writer();
    request.encode_into(&mut w).unwrap();
    let bytes = w.finish();
    assert_eq!(bytes, expected);
    assert_eq!(
        GetTypeDependenciesRequest::decode_from(&mut xcdr2_reader(&bytes)).unwrap(),
        request
    );
    // §7.6.3.3.3: the cursor is a sequence<octet, 32>.
    assert_eq!(ContinuationPoint::MAX_LEN, 32);
}

// ---------------------------------------------------------------------
// Replies
// ---------------------------------------------------------------------

/// `TypeLookup_getTypes_Out` with the minimal `Robot`, by hand.
fn robot_reply_bytes() -> Vec<u8> {
    let mut b = vec![
        0x60, 0x00, 0x00, 0x00, // DHEADER: 96
        0xd1, 0x4a, 0x80, 0x52, // EMHEADER: LC 5, types (0x02804ad1)
        0x4b, 0x00, 0x00, 0x00, // DHEADER of the sequence: 75
        0x01, 0x00, 0x00, 0x00, // one TypeIdentifierTypeObjectPair (FINAL)
        0xf1,
    ];
    b.extend_from_slice(&ROBOT_MINIMAL_HASH);
    b.push(0x00); // padding: the TypeObject's DHEADER aligns to four
    b.extend_from_slice(&ROBOT_MINIMAL);
    b.extend_from_slice(&[
        0x00, // padding to the next EMHEADER
        0x77, 0x65, 0x8e, 0x5b, // EMHEADER: LC 5, complete_to_minimal (0x0b8e6577)
        0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    b
}

#[test]
fn xtypes_7_6_3_3_3_an_empty_get_types_reply_is_cyclones() {
    let cyclone = [
        0x18, 0x00, 0x00, 0x00, 0xd1, 0x4a, 0x80, 0x52, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x77, 0x65, 0x8e, 0x5b, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let reply = GetTypesReply::decode_from(&mut xcdr2_reader(&cyclone)).unwrap();
    assert!(reply.types.is_empty());
    assert!(reply.complete_to_minimal.is_empty());
    let mut w = xcdr2_writer();
    GetTypesReply::default().encode_into(&mut w).unwrap();
    assert_eq!(w.finish(), cyclone);
}

#[test]
fn xtypes_7_6_3_3_3_get_types_reply_carries_the_identifier_and_the_object() {
    let bytes = robot_reply_bytes();
    let reply = GetTypesReply::decode_from(&mut xcdr2_reader(&bytes)).unwrap();
    assert_eq!(reply.types.len(), 1);
    assert_eq!(
        reply.types[0].type_identifier,
        TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(ROBOT_MINIMAL_HASH))
    );
    assert_eq!(reply.types[0].type_object, object(&ROBOT_MINIMAL));

    // The responder side writes the same octets.
    let mut w = xcdr2_writer();
    GetTypesReply {
        types: vec![TypeIdentifierTypeObjectPair {
            type_identifier: TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
                ROBOT_MINIMAL_HASH,
            )),
            type_object: object(&ROBOT_MINIMAL),
        }],
        complete_to_minimal: Vec::new(),
    }
    .encode_into(&mut w)
    .unwrap();
    assert_eq!(w.finish(), bytes);
}

#[test]
fn a_truncated_get_types_reply_is_an_error() {
    let bytes = robot_reply_bytes();
    assert!(GetTypesReply::decode_from(&mut xcdr2_reader(&bytes[..50])).is_err());
}

#[test]
fn xtypes_7_6_3_3_3_get_type_dependencies_reply_lists_identifiers_with_size() {
    // Cyclone 11.0.1, empty.
    let cyclone = [
        0x14, 0x00, 0x00, 0x00, 0xc9, 0xdf, 0xa4, 0x5b, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xd2, 0xe3, 0x08, 0x55, 0x00, 0x00, 0x00, 0x00,
    ];
    let empty = GetTypeDependenciesReply::decode_from(&mut xcdr2_reader(&cyclone)).unwrap();
    assert!(empty.dependent_typeids.is_empty());
    assert!(empty.continuation_point.0.is_empty());

    // One dependency of 55 octets and a cursor, by hand.
    let mut b = vec![
        0x34, 0x00, 0x00, 0x00, // DHEADER: 52
        0xc9, 0xdf, 0xa4, 0x5b, // EMHEADER: LC 5, dependent_typeids (0x0ba4dfc9)
        0x1c, 0x00, 0x00, 0x00, // DHEADER of the sequence: 28
        0x01, 0x00, 0x00, 0x00, // one element
        0x14, 0x00, 0x00, 0x00, // DHEADER of TypeIdentifierWithSize: 20
        0xf1,
    ];
    b.extend_from_slice(&ROBOT_MINIMAL_HASH);
    b.extend_from_slice(&[
        0x00, // padding
        0x37, 0x00, 0x00, 0x00, // typeobject_serialized_size: 55
        0xd2, 0xe3, 0x08, 0x55, // EMHEADER: LC 5, continuation_point
        0x08, 0x00, 0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    let reply = GetTypeDependenciesReply::decode_from(&mut xcdr2_reader(&b)).unwrap();
    assert_eq!(
        reply.dependent_typeids,
        vec![TypeIdentifierWithSize {
            type_id: TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(ROBOT_MINIMAL_HASH)),
            typeobject_serialized_size: 55,
        }]
    );
    assert_eq!(
        reply.continuation_point,
        ContinuationPoint(vec![0x42, 0, 0, 0, 0, 0, 0, 0])
    );
    let mut w = xcdr2_writer();
    reply.encode_into(&mut w).unwrap();
    assert_eq!(w.finish(), b);
}

// ---------------------------------------------------------------------
// The registry behind the responder and the requester
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_3_4_5_the_registry_files_an_object_under_the_hash_of_its_serialization() {
    let mut registry = TypeRegistry::new();
    let minimal = registry.insert_object(object(&ROBOT_MINIMAL)).unwrap();
    let complete = registry.insert_object(object(&ROBOT_COMPLETE)).unwrap();
    assert_eq!(minimal, EquivalenceHash(ROBOT_MINIMAL_HASH));
    assert_eq!(complete, EquivalenceHash(ROBOT_COMPLETE_HASH));
    assert!(registry.get_minimal(&minimal).is_some());
    assert!(registry.get_complete(&complete).is_some());
    assert!(registry.get_minimal(&complete).is_none());
    // §7.6.3.3.4.2: the pair a complete answer carries.
    assert_eq!(registry.minimal_hash_of_complete(&complete), Some(minimal));
}

#[test]
fn xtypes_7_6_3_2_1_typeobject_serialized_size_counts_the_dheader() {
    let TypeObject::Complete(complete) = object(&ROBOT_COMPLETE) else {
        panic!("expected a complete TypeObject");
    };
    assert_eq!(minimal_serialized_size(&robot_minimal()).unwrap(), 55);
    assert_eq!(complete_serialized_size(&complete).unwrap(), 84);
}

#[test]
fn xtypes_7_6_3_3_4_1_a_type_depends_on_the_types_its_members_name_by_hash() {
    let mut registry = TypeRegistry::new();
    let robot = registry.insert_object(object(&ROBOT_MINIMAL)).unwrap();
    let outer = TypeObjectBuilder::struct_type("Outer")
        .extensibility(Extensibility::Final)
        .member("r", TypeIdentifier::EquivalenceHashMinimal(robot), |m| m)
        .build_minimal();
    let outer = registry
        .insert_object(TypeObject::Minimal(MinimalTypeObject::Struct(outer)))
        .unwrap();
    assert_eq!(registry.transitive_dependencies(&outer, 256), vec![robot]);
    assert!(registry.transitive_dependencies(&robot, 256).is_empty());
    // A hash the registry does not hold has no known dependencies.
    assert!(
        registry
            .transitive_dependencies(&EquivalenceHash([0xee; 14]), 256)
            .is_empty()
    );
}
