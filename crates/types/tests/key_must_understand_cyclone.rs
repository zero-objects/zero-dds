// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A key member's must-understand bit in the `TypeObject` of Cyclone DDS
//! against the same IDL lowered by this crate.
//!
//! DDS-XTypes 1.3 §7.2.2.4.4.4.8 (1.4 Beta 1 §7.2.3.2.4.4.8): key members
//! "shall always have their 'must understand' attribute set to true"; 1.4
//! Beta 1 §7.3.3.4, the resolution of DDSXTY14-64, encodes the attribute
//! in `IS_MUST_UNDERSTAND`, and `MemberFlagMinimalMask` (`0x003f`) keeps it
//! in the minimal object. A reader that announces no
//! `TypeConsistencyEnforcement` is held to DISALLOW_TYPE_COERCION
//! (§7.6.3.4.1), which asks for MINIMAL equivalence (§7.6.3.4.2 Step 1,
//! §7.3.4.7), and ALLOW_TYPE_COERCION asks for the rules of Table 19,
//! which compare no member flag for equality.
//!
//! The oracle is Cyclone's own announcement of the suite's `ShapeType`:
//! its `PID_TYPE_INFORMATION` and its getTypes reply, captured from Cyclone
//! master (`fixtures/cyclone-master-shapetype.txt` holds the provenance).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_types::assignability::KeyMustUnderstand;
use zerodds_types::framing::xcdr2_reader;
use zerodds_types::from_ast::{minimal_type_object, register_all};
use zerodds_types::hash::compute_minimal_hash;
use zerodds_types::qos::TypeConsistencyEnforcement;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_lookup::{GetTypesResult, TypeLookupReplyMessage, TypeLookupReturn};
use zerodds_types::type_matcher::{TypeMatchResult, TypeMatcher};
use zerodds_types::type_object::flags::StructMemberFlag;
use zerodds_types::{MinimalTypeObject, TypeIdentifier, TypeInformation, TypeObject};

const FIXTURE: &str = include_str!("fixtures/cyclone-master-shapetype.txt");
const SHAPE_IDL: &str = include_str!("../../../interop/shapes/omg-dds-rtps.idl");

fn fixture(name: &str) -> Vec<u8> {
    let hex = FIXTURE
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
        .unwrap_or_else(|| panic!("{name} is not in the fixture"));
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect()
}

/// Cyclone's minimal identifier of `ShapeType`, as its DATA(r) announced
/// it, and the minimal object its getTypes reply sent for it.
fn cyclone() -> (TypeIdentifier, MinimalTypeObject) {
    let information =
        TypeInformation::from_bytes_le(&fixture("TYPE_INFORMATION")).expect("TypeInformation");
    let announced = information.minimal.typeid_with_size.type_id;
    let reply = fixture("GET_TYPES_REPLY");
    // CDR2_LE, options 0.
    assert_eq!(&reply[..4], &[0x00, 0x07, 0x00, 0x00]);
    let message =
        TypeLookupReplyMessage::decode_from(&mut xcdr2_reader(&reply[4..])).expect("the reply");
    let TypeLookupReturn::GetTypes(GetTypesResult::Ok(result)) = message.ret else {
        panic!(
            "not a getTypes reply with DDS_RETCODE_OK: {:?}",
            message.ret
        );
    };
    assert_eq!(result.types.len(), 1);
    let pair = &result.types[0];
    assert_eq!(pair.type_identifier, announced);
    let TypeObject::Minimal(object) = &pair.type_object else {
        panic!("not a minimal object");
    };
    (announced, object.clone())
}

/// This crate's minimal object of the same IDL, and a registry holding
/// every object of the model.
fn zerodds(registry: &mut TypeRegistry) -> (TypeIdentifier, MinimalTypeObject) {
    let parsed = zerodds_idl_parse::parse(SHAPE_IDL);
    assert!(!parsed.has_errors());
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "omg-dds-rtps.idl").ast;
    ast.resolve_all_by_name().expect("resolves");
    let id = ast
        .types()
        .find(|(_, node)| node.name == "ShapeType")
        .map(|(id, _)| id)
        .expect("ShapeType");
    register_all(&ast, registry).expect("registers");
    let object = minimal_type_object(&ast, id).expect("the minimal object");
    let hash = compute_minimal_hash(&object).expect("hash");
    (TypeIdentifier::EquivalenceHashMinimal(hash), object)
}

/// Cyclone's object hashes to the identifier Cyclone announced, and the
/// two objects differ in one thing: the key member `color` carries
/// `IS_KEY | TRY_CONSTRUCT1` (`0x21`) in Cyclone's and `IS_MUST_UNDERSTAND`
/// besides in this crate's (`0x31`).
#[test]
fn xtypes_7_2_2_4_4_4_8_cyclone_writes_the_key_member_without_must_understand() {
    let (announced, theirs) = cyclone();
    assert_eq!(
        TypeIdentifier::EquivalenceHashMinimal(compute_minimal_hash(&theirs).expect("hash")),
        announced,
        "the minimal object hashes to the identifier Cyclone announced"
    );
    let mut registry = TypeRegistry::new();
    let (ours_id, ours) = zerodds(&mut registry);
    assert_ne!(ours_id, announced);
    let (MinimalTypeObject::Struct(theirs), MinimalTypeObject::Struct(ours)) = (theirs, ours)
    else {
        panic!("ShapeType is a structure");
    };
    assert_eq!(theirs.member_seq[0].common.member_flags.0, 0x0021);
    assert_eq!(ours.member_seq[0].common.member_flags.0, 0x0031);
    let mut with_bit = theirs.clone();
    with_bit.member_seq[0].common.member_flags =
        StructMemberFlag(0x0021 | StructMemberFlag::IS_MUST_UNDERSTAND);
    assert_eq!(with_bit, ours, "the objects differ in that bit alone");
}

/// The writer is this crate's `ShapeType`, the reader Cyclone's. Under the
/// policy a reader without PID 0x0074 is held to, the clause refuses the
/// pair; read with `IS_KEY` implying the bit, the two are one type. Under
/// ALLOW_TYPE_COERCION the bit decides nothing.
#[test]
fn xtypes_7_6_3_4_2_disallow_refuses_cyclone_unless_the_key_implies_must_understand() {
    let (reader, theirs) = cyclone();
    let mut registry = TypeRegistry::new();
    let (writer, _) = zerodds(&mut registry);
    let TypeIdentifier::EquivalenceHashMinimal(hash) = reader else {
        panic!("a minimal hash");
    };
    registry.insert_minimal(hash, theirs);

    let absent = TypeConsistencyEnforcement::not_announced();
    let matcher = TypeMatcher::new(&absent);
    assert!(
        matches!(
            matcher.match_types(&writer, &reader, &registry),
            TypeMatchResult::Incompatible { .. }
        ),
        "the clause: the minimal identifiers differ"
    );
    assert_eq!(
        matcher
            .with_key_must_understand(KeyMustUnderstand::ImpliedByKey)
            .match_types(&writer, &reader, &registry),
        TypeMatchResult::Matches,
        "IS_KEY read as implying IS_MUST_UNDERSTAND"
    );

    let allow = TypeConsistencyEnforcement::default();
    for reading in [KeyMustUnderstand::Flagged, KeyMustUnderstand::ImpliedByKey] {
        assert_eq!(
            TypeMatcher::new(&allow)
                .with_key_must_understand(reading)
                .match_types(&writer, &reader, &registry),
            TypeMatchResult::Matches,
            "ALLOW_TYPE_COERCION under {reading:?}"
        );
    }
}

/// The reading reaches a type through the types it names: a structure
/// whose member is Cyclone's `ShapeType` against the same structure over
/// this crate's.
#[test]
fn xtypes_7_3_4_7_the_reading_rewrites_the_types_a_type_names() {
    use zerodds_types::type_object::common::{CommonStructMember, NameHash};
    use zerodds_types::type_object::flags::StructTypeFlag;
    use zerodds_types::type_object::minimal::{
        MinimalStructHeader, MinimalStructMember, MinimalStructType,
    };
    let (reader_shape, theirs) = cyclone();
    let mut registry = TypeRegistry::new();
    let (writer_shape, _) = zerodds(&mut registry);
    let TypeIdentifier::EquivalenceHashMinimal(hash) = reader_shape else {
        panic!("a minimal hash");
    };
    registry.insert_minimal(hash, theirs);
    let holder = |shape: &TypeIdentifier| {
        MinimalTypeObject::Struct(MinimalStructType {
            struct_flags: StructTypeFlag(StructTypeFlag::IS_FINAL),
            header: MinimalStructHeader {
                base_type: TypeIdentifier::None,
            },
            member_seq: vec![MinimalStructMember {
                common: CommonStructMember {
                    member_id: 0,
                    member_flags: StructMemberFlag(0x0001),
                    member_type_id: shape.clone(),
                },
                detail: NameHash::from_name("shape"),
            }],
        })
    };
    let mut ids = Vec::new();
    for shape in [&writer_shape, &reader_shape] {
        let object = holder(shape);
        let hash = compute_minimal_hash(&object).expect("hash");
        registry.insert_minimal(hash, object);
        ids.push(TypeIdentifier::EquivalenceHashMinimal(hash));
    }
    let absent = TypeConsistencyEnforcement::not_announced();
    assert!(
        !TypeMatcher::new(&absent)
            .match_types(&ids[0], &ids[1], &registry)
            .is_match()
    );
    assert!(
        TypeMatcher::new(&absent)
            .with_key_must_understand(KeyMustUnderstand::ImpliedByKey)
            .match_types(&ids[0], &ids[1], &registry)
            .is_match()
    );
}
