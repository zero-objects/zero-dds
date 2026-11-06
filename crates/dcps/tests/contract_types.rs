// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-dcps` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`, from its non-test sources:
//! - `from_ast::{complete_type_object, type_identifier (Form::Complete),
//!   dependency_type_objects}` and `compute_hash` for the identity of a type
//!   (`builtin_types.rs`, `serialized_type.rs`, `participant.rs`,
//!   `runtime.rs::register_type_object`); `TypeObject`,
//!   `CompleteTypeObject::Struct` with `type_object::flags::StructTypeFlag`
//!   for the extensibility of a described type (`participant.rs`);
//!   `TypeIdentifier`, `EquivalenceHash`, `type_identifier::kinds::
//!   EK_COMPLETE`.
//! - `resolve::TypeRegistry` (`insert_complete`, `insert_minimal`,
//!   `insert_object`, `project_completes_to_minimal`, `get_complete`,
//!   `minimal_hash_of_complete`, `transitive_dependencies`,
//!   `first_unresolved_dependency`, `generation`) and
//!   `TypeInformation::{from_registry, to_bytes_le, from_bytes_le}` for
//!   `PID_TYPE_INFORMATION` (`runtime.rs`, `participant.rs`).
//! - `type_lookup::{TypeLookupRequest, RequestHeader, TypeLookupCall,
//!   GetTypesRequest, TypeLookupReplyMessage, ReplyHeader, TypeLookupReturn,
//!   GetTypesResult, TypeLookupEncoding, TypeLookupEncapsulation}` for the
//!   TypeLookup service samples (`runtime.rs`).
//! - `type_matcher::{TypeMatcher, TypeMatchResult}`, `qos::
//!   {TypeConsistencyEnforcement, TypeConsistencyKind}` and the
//!   `assignability::{KeyMemberBounds, StrongAssignability,
//!   KeyMustUnderstand}` axes for type consistency
//!   (`runtime.rs::decide_type_consistency`, `qos.rs`).
//! - The settings `wire.type_lookup_encoding`, `xtypes.key_member_bounds`,
//!   `xtypes.strong_assignability`, `xtypes.key_must_understand`,
//!   `xtypes.annotation_parameter_order`:
//!   `parse`, `as_str`, `set_annotation_parameter_order` (`runtime.rs`).
//! - `framing::xcdr2_reader`, `TypeObject::decode_from`,
//!   `dynamic::{ModelFromTypeObjects, DynamicType, DynamicSample,
//!   ExtensibilityKind, TypeKind}` for a type known only by its TypeObjects
//!   (`serialized_type.rs`, `multitopic.rs`, `xml_data.rs`).
//! - `type_object_v1::convert::{from_complete, to_complete}` for
//!   `type_object_v1` (`runtime.rs`).
//!
//! Oracles: DDS-XTypes 1.3 Annex B (TypeObject layout, `StructTypeFlag`
//! bits, `EK_MINIMAL`/`EK_COMPLETE`), §7.3.4 (the hash is the first 14
//! octets of the MD5 of the XCDR2 little-endian TypeObject), §7.4.3.4.2
//! (EMHEADER length codes 4 and 5), §7.6.3.2.1 (TypeInformation and its
//! dependency count), §7.6.3.3.3 (the TypeLookup envelopes), §7.6.3.4.1
//! and §7.6.3.4.2 (the default TypeConsistencyEnforcement, coercion),
//! §7.2.4 (assignability of appendable and final structures), §7.6.3.2.2
//! (an XTYPES 1.1 endpoint identified by the Complete TypeObject it
//! describes); vendor output: Cyclone DDS 11.0.1's hashes of `@final struct
//! Robot { uint32 id; uint32 label; }` and the sizes 55 and 84 Cyclone,
//! Fast DDS 3.6.1 and OpenDDS 3.34.0 announce for it
//! (`crates/types/tests/typeobject_serialized_size.rs`), the Cyclone 0.11.0
//! serializer's TypeLookup envelopes
//! (`crates/types/tests/fixtures/cyclone-typelookup-11.log`), RTI Connext
//! 7.7.0's length code 5 in `PID_TYPE_INFORMATION`
//! (`crates/types/tests/type_information_length_codes.rs`); the
//! settings' spellings as the runtime's help text names them.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use zerodds_ast::{AnyId, Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_types::assignability::{KeyMemberBounds, KeyMustUnderstand, StrongAssignability};
use zerodds_types::from_ast::{Form, complete_type_object, dependency_type_objects};
use zerodds_types::qos::{TypeConsistencyEnforcement, TypeConsistencyKind};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::kinds::{EK_COMPLETE, EK_MINIMAL};
use zerodds_types::type_information::TypeInformation;
use zerodds_types::type_lookup::{
    GetTypesRequest, GetTypesResult, ReplyHeader, RequestHeader, TypeLookupCall,
    TypeLookupEncapsulation, TypeLookupEncoding, TypeLookupReplyMessage, TypeLookupRequest,
    TypeLookupReturn,
};
use zerodds_types::type_matcher::{TypeMatchResult, TypeMatcher};
use zerodds_types::type_object::complete::{
    AnnotationParameterOrder, annotation_parameter_order, set_annotation_parameter_order,
};
use zerodds_types::type_object::flags::StructTypeFlag;
use zerodds_types::{
    CompleteTypeObject, EquivalenceHash, TypeIdentifier, TypeObject, compute_hash,
};

const IDL: &str = r"
@final struct Robot { uint32 id; uint32 label; };
@final struct Point { long x; long y; };
@final struct Path { Point head; Point tail; };
@appendable struct Grow { long a; };
@mutable struct Shift { long a; };
module W {
  @appendable struct S { long a; };
  @final struct F { long a; };
};
module R {
  @appendable struct S { long a; long b; };
  @final struct F { long a; long b; };
};
";

/// Cyclone DDS 11.0.1's MINIMAL hash of `Robot`.
const ROBOT_MINIMAL: [u8; 14] = [
    0x62, 0x54, 0x36, 0xec, 0xee, 0x22, 0xcc, 0x29, 0x43, 0xeb, 0xc5, 0x98, 0xc4, 0x3e,
];
/// Cyclone DDS 11.0.1's COMPLETE hash of `Robot`, also RTI's.
const ROBOT_COMPLETE: [u8; 14] = [
    0xaa, 0xd3, 0x35, 0x12, 0xa5, 0xe4, 0x28, 0x68, 0xff, 0x3e, 0x9e, 0x01, 0x68, 0x9a,
];

fn model() -> Ast {
    zerodds_wire_probe::encode::model_from_idl(IDL, "contract_types.idl").expect("the model")
}

fn type_id(ast: &Ast, name: &str) -> Id<TypeNode> {
    let segments = name.split("::").map(String::from).collect();
    ast.find_by_scoped_name(&ScopedName::absolute(segments))
        .ok()
        .and_then(AnyId::as_type)
        .expect(name)
}

fn complete(ast: &Ast, name: &str) -> CompleteTypeObject {
    complete_type_object(ast, type_id(ast, name)).unwrap()
}

fn hash_of(object: &CompleteTypeObject) -> EquivalenceHash {
    compute_hash(&TypeObject::Complete(object.clone())).unwrap()
}

/// `DcpsRuntime::register_type_object`: the complete object in, the
/// minimal one projected beside it.
fn register(registry: &mut TypeRegistry, object: CompleteTypeObject) -> EquivalenceHash {
    let hash = hash_of(&object);
    registry.insert_complete(hash, object);
    registry.project_completes_to_minimal();
    hash
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

// ---------------------------------------------------------------------
// The identity of a type
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_3_4_robot_from_idl_has_the_hashes_cyclone_announces() {
    let ast = model();
    let robot = complete(&ast, "Robot");
    let hash = hash_of(&robot);
    assert_eq!(hash, EquivalenceHash(ROBOT_COMPLETE));
    assert_eq!(
        zerodds_types::from_ast::type_identifier(&ast, type_id(&ast, "Robot"), Form::Complete)
            .unwrap(),
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(ROBOT_COMPLETE))
    );
    assert!(
        dependency_type_objects(&ast, type_id(&ast, "Robot"))
            .unwrap()
            .is_empty()
    );
    // The minimal form the runtime registers beside it.
    let mut registry = TypeRegistry::new();
    let before = registry.generation();
    register(&mut registry, robot.clone());
    assert_ne!(
        registry.generation(),
        before,
        "a reader retries on a change"
    );
    assert_eq!(registry.get_complete(&hash), Some(&robot));
    assert_eq!(
        registry.minimal_hash_of_complete(&hash),
        Some(EquivalenceHash(ROBOT_MINIMAL))
    );
    assert!(
        registry
            .get_minimal(&EquivalenceHash(ROBOT_MINIMAL))
            .is_some()
    );
    // Annex B: the equivalence kinds.
    assert_eq!((EK_MINIMAL, EK_COMPLETE), (0xF1, 0xF2));
}

#[test]
fn xtypes_annex_b_struct_type_flags_carry_the_extensibility() {
    assert_eq!(
        (
            StructTypeFlag::IS_FINAL,
            StructTypeFlag::IS_APPENDABLE,
            StructTypeFlag::IS_MUTABLE
        ),
        (0x0001, 0x0002, 0x0004)
    );
    let ast = model();
    for (name, bit) in [
        ("Robot", StructTypeFlag::IS_FINAL),
        ("Grow", StructTypeFlag::IS_APPENDABLE),
        ("Shift", StructTypeFlag::IS_MUTABLE),
    ] {
        let CompleteTypeObject::Struct(s) = complete(&ast, name) else {
            panic!("{name} is a structure");
        };
        assert!(s.struct_flags.has(bit), "{name}: {:#06x}", s.struct_flags.0);
        let others =
            (StructTypeFlag::IS_FINAL | StructTypeFlag::IS_APPENDABLE | StructTypeFlag::IS_MUTABLE)
                & !bit;
        assert!(!s.struct_flags.has(others), "{name}: one extensibility");
    }
}

// ---------------------------------------------------------------------
// PID_TYPE_INFORMATION
// ---------------------------------------------------------------------

/// The TypeInformation of `Robot`, by hand from §7.6.3.2.1 and Annex B:
/// each member behind an EMHEADER of length code `lc` (4 as Cyclone, Fast
/// DDS and OpenDDS write it, 5 as RTI does).
fn robot_type_information(lc: u32) -> Vec<u8> {
    let with_dependencies = |kind: u8, hash: [u8; 14], size: u32| {
        let mut body = 20_u32.to_le_bytes().to_vec(); // DHEADER, TypeIdentifierWithSize
        body.push(kind);
        body.extend_from_slice(&hash);
        body.push(0);
        body.extend_from_slice(&size.to_le_bytes());
        body.extend_from_slice(&0_i32.to_le_bytes()); // dependent_typeid_count
        body.extend_from_slice(&[4, 0, 0, 0, 0, 0, 0, 0]); // empty sequence
        body
    };
    let mut members = Vec::new();
    for (id, body) in [
        (0x1001_u32, with_dependencies(0xf1, ROBOT_MINIMAL, 55)),
        (0x1002, with_dependencies(0xf2, ROBOT_COMPLETE, 84)),
    ] {
        members.extend_from_slice(&((lc << 28) | id).to_le_bytes());
        if lc == 4 {
            members.extend_from_slice(&u32::try_from(body.len() + 4).unwrap().to_le_bytes());
        }
        members.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
        members.extend_from_slice(&body);
    }
    let mut out = u32::try_from(members.len()).unwrap().to_le_bytes().to_vec();
    out.extend_from_slice(&members);
    out
}

#[test]
fn xtypes_7_6_3_2_1_robot_type_information_is_what_the_vendors_announce() {
    let ast = model();
    let mut registry = TypeRegistry::new();
    let robot = complete(&ast, "Robot");
    register(&mut registry, robot.clone());
    let ti = TypeInformation::from_registry(&robot, &registry).unwrap();
    assert_eq!(
        ti.minimal.typeid_with_size.type_id,
        TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(ROBOT_MINIMAL))
    );
    assert_eq!(
        ti.complete.typeid_with_size.type_id,
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(ROBOT_COMPLETE))
    );
    assert_eq!(ti.minimal.typeid_with_size.typeobject_serialized_size, 55);
    assert_eq!(ti.complete.typeid_with_size.typeobject_serialized_size, 84);
    assert_eq!(
        (
            ti.minimal.dependent_typeid_count,
            ti.complete.dependent_typeid_count
        ),
        (0, 0)
    );

    let lc4 = robot_type_information(4);
    assert_eq!(lc4.len(), 100);
    assert_eq!(ti.to_bytes_le().unwrap(), lc4);
    // What a peer announces is read whichever length code it chose.
    assert_eq!(TypeInformation::from_bytes_le(&lc4).unwrap(), ti);
    assert_eq!(
        TypeInformation::from_bytes_le(&robot_type_information(5)).unwrap(),
        ti
    );
}

#[test]
fn xtypes_7_6_3_2_1_a_dependency_is_announced_once_it_is_registered() {
    let ast = model();
    let point = complete(&ast, "Point");
    let path = complete(&ast, "Path");
    assert_eq!(
        dependency_type_objects(&ast, type_id(&ast, "Path")).unwrap(),
        vec![point.clone()]
    );
    let mut registry = TypeRegistry::new();
    let path_hash = register(&mut registry, path.clone());
    let point_hash = hash_of(&point);
    // Point unknown: the runtime announces no TypeInformation and asks
    // for the missing type.
    assert!(TypeInformation::from_registry(&path, &registry).is_err());
    assert_eq!(
        registry.first_unresolved_dependency(
            &TypeIdentifier::EquivalenceHashComplete(path_hash),
            zerodds_types::resolve::DEFAULT_MAX_RESOLVE_NODES
        ),
        Some(TypeIdentifier::EquivalenceHashComplete(point_hash))
    );

    register(&mut registry, point);
    assert_eq!(
        registry.first_unresolved_dependency(
            &TypeIdentifier::EquivalenceHashComplete(path_hash),
            zerodds_types::resolve::DEFAULT_MAX_RESOLVE_NODES
        ),
        None
    );
    assert_eq!(
        registry.transitive_dependencies(
            &path_hash,
            zerodds_types::resolve::DEFAULT_MAX_RESOLVE_NODES
        ),
        vec![point_hash]
    );
    let ti = TypeInformation::from_registry(&path, &registry).unwrap();
    assert_eq!(ti.minimal.dependent_typeid_count, 1);
    assert_eq!(ti.complete.dependent_typeid_count, 1);
    let point_minimal = registry.minimal_hash_of_complete(&point_hash).unwrap();
    assert_eq!(
        ti.minimal.dependent_typeids[0].type_id,
        TypeIdentifier::EquivalenceHashMinimal(point_minimal)
    );
    assert_eq!(
        ti.complete.dependent_typeids[0].type_id,
        TypeIdentifier::EquivalenceHashComplete(point_hash)
    );
}

// ---------------------------------------------------------------------
// The TypeLookup service samples
// ---------------------------------------------------------------------

/// Cyclone 0.11.0's serializer, `DDS_Builtin_TypeLookup_Request` with no
/// identifiers (`cyclone-typelookup-11.log`, `GET_TYPES_REQUEST`).
const CYCLONE_GET_TYPES_REQUEST: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a0403020108070605290000006464732e6275696c74696e2e544f532e3030303030303030303030303030303030303030303030300000000014000000d35282010c0000006560535c0400000000000000";
/// The same, `DDS_Builtin_TypeLookup_Reply` with an empty `getTypes`
/// result (`GET_TYPES_REPLY`).
const CYCLONE_GET_TYPES_REPLY: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a04030201080706050000000028000000d3528201200000000000000018000000d14a8052040000000000000077658e5b0400000000000000";

#[test]
fn xtypes_7_6_3_3_3_type_lookup_samples_are_cyclones_envelopes_behind_cdr2_le() {
    assert_eq!(TypeLookupEncoding::default(), TypeLookupEncoding::Xcdr2);
    let request = TypeLookupRequest {
        header: RequestHeader {
            writer_guid: [0x5a; 16],
            sequence_number: 0x0102_0304_0506_0708,
            instance_name: String::from("dds.builtin.TOS.000000000000000000000000"),
        },
        data: TypeLookupCall::GetTypes(GetTypesRequest::default()),
    };
    // CDR2_LE {0x00, 0x07}, no padding (96 octets), then Cyclone's body.
    let payload = request.to_payload(TypeLookupEncoding::Xcdr2).unwrap();
    let mut expected = vec![0x00, 0x07, 0x00, 0x00];
    expected.extend_from_slice(&hex(CYCLONE_GET_TYPES_REQUEST));
    assert_eq!(payload, expected);

    // The receive path: the header says how to read the body.
    let encapsulation = TypeLookupEncapsulation::read(&payload).unwrap();
    assert_eq!(encapsulation.endianness, zerodds_wire::Endianness::Little);
    assert_eq!(
        encapsulation.version,
        zerodds_types::framing::EncodingVersion::Xcdr2
    );
    assert_eq!(encapsulation.options, [0, 0]);
    let mut reader = encapsulation.reader(&payload[4..]);
    assert_eq!(
        TypeLookupRequest::decode_from(&mut reader).unwrap(),
        request
    );
    assert_eq!(reader.remaining(), 0);

    let reply = TypeLookupReplyMessage {
        header: ReplyHeader {
            writer_guid: [0x5a; 16],
            sequence_number: 0x0102_0304_0506_0708,
            remote_ex: 0,
        },
        ret: TypeLookupReturn::GetTypes(GetTypesResult::Ok(Default::default())),
    };
    let mut expected = vec![0x00, 0x07, 0x00, 0x00];
    expected.extend_from_slice(&hex(CYCLONE_GET_TYPES_REPLY));
    assert_eq!(
        reply.to_payload(TypeLookupEncoding::Xcdr2).unwrap(),
        expected
    );
    let mut reader = TypeLookupEncapsulation::read(&expected)
        .unwrap()
        .reader(&expected[4..]);
    assert_eq!(
        TypeLookupReplyMessage::decode_from(&mut reader).unwrap(),
        reply
    );

    // `wire.type_lookup_encoding: xcdr1` writes CDR_LE {0x00, 0x01}.
    let xcdr1 = request.to_payload(TypeLookupEncoding::Xcdr1).unwrap();
    assert_eq!(xcdr1[..2], [0x00, 0x01]);
    assert_eq!(
        TypeLookupEncapsulation::read(&xcdr1).unwrap().version,
        zerodds_types::framing::EncodingVersion::Xcdr1
    );
    // Neither CDR nor CDR2 (PL_CDR_LE here): named back to the caller.
    assert_eq!(
        TypeLookupEncapsulation::read(&[0x00, 0x03, 0x00, 0x00]),
        Err(Some([0x00, 0x03]))
    );
    // An operation §7.6.3.3.3 does not define is refused.
    let mut unknown = hex(CYCLONE_GET_TYPES_REQUEST);
    unknown[76..80].copy_from_slice(&[0, 0, 0, 0]);
    assert!(
        TypeLookupRequest::decode_from(&mut zerodds_types::framing::xcdr2_reader(&unknown))
            .is_err()
    );
}

// ---------------------------------------------------------------------
// Type consistency
// ---------------------------------------------------------------------

#[test]
fn xtypes_7_6_3_4_1_the_default_type_consistency_enforcement() {
    assert_eq!(
        TypeConsistencyEnforcement::default(),
        TypeConsistencyEnforcement {
            kind: TypeConsistencyKind::AllowTypeCoercion,
            ignore_sequence_bounds: true,
            ignore_string_bounds: true,
            ignore_member_names: false,
            prevent_type_widening: false,
            force_type_validation: false,
        }
    );
}

#[test]
fn xtypes_7_6_3_4_2_coercion_decides_whether_a_longer_appendable_reader_matches() {
    let ast = model();
    let mut registry = TypeRegistry::new();
    let hashes: Vec<EquivalenceHash> = ["W::S", "R::S", "W::F", "R::F"]
        .into_iter()
        .map(|name| register(&mut registry, complete(&ast, name)))
        .collect();
    let id = |complete_hash: &EquivalenceHash| {
        TypeIdentifier::EquivalenceHashMinimal(
            registry.minimal_hash_of_complete(complete_hash).unwrap(),
        )
    };
    let (w_s, r_s, w_f, r_f) = (
        id(&hashes[0]),
        id(&hashes[1]),
        id(&hashes[2]),
        id(&hashes[3]),
    );
    let check = |tce: &TypeConsistencyEnforcement,
                 writer: &TypeIdentifier,
                 reader: &TypeIdentifier|
     -> TypeMatchResult {
        TypeMatcher::new(tce)
            .with_key_member_bounds(KeyMemberBounds::default())
            .with_strong_assignability(StrongAssignability::default())
            .with_key_must_understand(KeyMustUnderstand::default())
            .match_types(writer, reader, &registry)
    };
    let allow = TypeConsistencyEnforcement::default();
    let disallow = TypeConsistencyEnforcement {
        kind: TypeConsistencyKind::DisallowTypeCoercion,
        ..TypeConsistencyEnforcement::default()
    };
    // The same type matches either way.
    assert_eq!(check(&allow, &w_s, &w_s), TypeMatchResult::Matches);
    assert_eq!(check(&disallow, &w_s, &w_s), TypeMatchResult::Matches);
    // An appendable reader with a member more is assignable from the
    // writer, and not equivalent to it.
    assert_eq!(check(&allow, &w_s, &r_s), TypeMatchResult::Matches);
    assert!(matches!(
        check(&disallow, &w_s, &r_s),
        TypeMatchResult::Incompatible { .. }
    ));
    // A final one is not even assignable.
    assert!(matches!(
        check(&allow, &w_f, &r_f),
        TypeMatchResult::Incompatible { .. }
    ));
}

#[test]
fn the_settings_parse_the_spellings_the_runtime_documents() {
    // `wire.type_lookup_encoding`: "xcdr1 or xcdr2".
    for (text, value) in [
        ("xcdr1", TypeLookupEncoding::Xcdr1),
        ("xcdr2", TypeLookupEncoding::Xcdr2),
    ] {
        assert_eq!(TypeLookupEncoding::parse(text), Some(value));
        assert_eq!(value.as_str(), text);
    }
    assert_eq!(TypeLookupEncoding::parse("cdr2"), None);
    // `xtypes.key_member_bounds`: "enforced or ignored".
    for (text, value) in [
        ("enforced", KeyMemberBounds::Enforced),
        ("ignored", KeyMemberBounds::Ignored),
    ] {
        assert_eq!(KeyMemberBounds::parse(text), Some(value));
        assert_eq!(value.as_str(), text);
    }
    assert_eq!(KeyMemberBounds::default(), KeyMemberBounds::Enforced);
    assert_eq!(KeyMemberBounds::parse("on"), None);
    // `xtypes.strong_assignability`: "equivalence or names_ignored".
    for (text, value) in [
        ("equivalence", StrongAssignability::Equivalence),
        ("names_ignored", StrongAssignability::NamesIgnored),
    ] {
        assert_eq!(StrongAssignability::parse(text), Some(value));
        assert_eq!(value.as_str(), text);
    }
    assert_eq!(
        StrongAssignability::default(),
        StrongAssignability::Equivalence
    );
    // `xtypes.key_must_understand`: "flagged or implied"; the default is
    // the bit as the TypeObject carries it (DDS-XTypes 1.4 Beta 1 §7.3.3.4).
    for (text, value) in [
        ("flagged", KeyMustUnderstand::Flagged),
        ("implied", KeyMustUnderstand::ImpliedByKey),
    ] {
        assert_eq!(KeyMustUnderstand::parse(text), Some(value));
        assert_eq!(value.as_str(), text);
    }
    assert_eq!(KeyMustUnderstand::default(), KeyMustUnderstand::Flagged);
    assert_eq!(KeyMustUnderstand::parse("key"), None);
    // `xtypes.annotation_parameter_order`: "member-index or name", the
    // order every annotation type built afterwards takes.
    for (text, value) in [
        ("member-index", AnnotationParameterOrder::MemberIndex),
        ("name", AnnotationParameterOrder::Name),
    ] {
        assert_eq!(AnnotationParameterOrder::parse(text), Some(value));
        assert_eq!(value.as_str(), text);
    }
    assert_eq!(AnnotationParameterOrder::parse("index"), None);
    assert_eq!(
        annotation_parameter_order(),
        AnnotationParameterOrder::MemberIndex
    );
    set_annotation_parameter_order(AnnotationParameterOrder::Name);
    let set = annotation_parameter_order();
    set_annotation_parameter_order(AnnotationParameterOrder::MemberIndex);
    assert_eq!(set, AnnotationParameterOrder::Name);
}

// ---------------------------------------------------------------------
// A type known only by its TypeObjects
// ---------------------------------------------------------------------

/// The COMPLETE TypeObject of `Robot` written out from Annex B; MD5 of
/// these 84 octets begins with Cyclone's complete hash.
#[rustfmt::skip]
const ROBOT_COMPLETE_OBJECT: [u8; 84] = [
    0x50, 0x00, 0x00, 0x00, 0xf2, 0x51, 0x01, 0x00,
    0x0e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x06, 0x00, 0x00, 0x00, b'R', b'o', b'b', b'o', b't', 0x00,
    0x00, 0x00,
    0x34, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
    0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07, 0x00,
    0x03, 0x00, 0x00, 0x00, b'i', b'd', 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00,
    0x14, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07, 0x00,
    0x06, 0x00, 0x00, 0x00, b'l', b'a', b'b', b'e', b'l', 0x00,
    0x00, 0x00,
];

#[test]
fn xtypes_7_5_a_model_rebuilt_from_type_objects_names_the_type_and_its_members() {
    // `SerializedTypeSupport::from_type_objects`.
    let mut reader = zerodds_types::framing::xcdr2_reader(&ROBOT_COMPLETE_OBJECT);
    let object = TypeObject::decode_from(&mut reader).unwrap();
    assert_eq!(reader.remaining(), 0);
    assert!(matches!(object, TypeObject::Complete(_)));
    let mut registry = TypeRegistry::new();
    assert_eq!(
        registry.insert_object(object.clone()).unwrap(),
        EquivalenceHash(ROBOT_COMPLETE)
    );
    let mut rebuilt = zerodds_types::dynamic::ModelFromTypeObjects::new(&registry);
    let root = rebuilt.add_type_object(&object).unwrap();
    let ast: Arc<Ast> = rebuilt.finish();

    let robot = zerodds_types::dynamic::DynamicType::find(&ast, "Robot").unwrap();
    assert_eq!(robot.id(), root);
    assert_eq!(robot.kind(), zerodds_types::dynamic::TypeKind::Structure);
    assert_eq!(
        robot.extensibility(),
        Some(zerodds_types::dynamic::ExtensibilityKind::Final)
    );
    let members: Vec<(String, u32)> = robot
        .members()
        .iter()
        .map(|m| (m.name().to_string(), m.id()))
        .collect();
    assert_eq!(
        members,
        vec![(String::from("id"), 0), (String::from("label"), 1)]
    );
    assert!(zerodds_types::dynamic::DynamicType::find(&ast, "Robot2").is_err());

    // A sample of it hands the codec back the value it holds.
    let value = zerodds_types::dynamic::DynamicData::Aggregate(vec![
        (
            String::from("id"),
            zerodds_types::dynamic::DynamicData::integer(zerodds_ast::PrimitiveKind::UInt32, 1),
        ),
        (
            String::from("label"),
            zerodds_types::dynamic::DynamicData::integer(zerodds_ast::PrimitiveKind::UInt32, 2),
        ),
    ]);
    let sample = zerodds_types::dynamic::DynamicSample::with_value(robot.clone(), value.clone());
    assert_eq!(sample.snapshot().unwrap(), value);
    assert!(sample.type_().equals_ignoring_annotations(&robot));
}

#[test]
fn xtypes_7_6_3_2_2_a_type_object_v1_names_the_same_complete_type() {
    use zerodds_types::type_object_v1::convert::{from_complete, to_complete};
    let ast = model();
    let mut registry = TypeRegistry::new();
    let robot = complete(&ast, "Robot");
    let hash = register(&mut registry, robot.clone());
    let root = TypeIdentifier::EquivalenceHashComplete(hash);
    // What the runtime announces with `type_object_v1`.
    let v1 = from_complete(&root, &registry).unwrap();
    assert!(!v1.the_type.is_empty());
    // What it makes of a peer's: the same Complete TypeObject, under
    // Cyclone's hash.
    let back = to_complete(&v1).unwrap();
    assert_eq!(back.roots, vec![root]);
    assert!(
        back.types
            .iter()
            .any(|(h, o)| *h == EquivalenceHash(ROBOT_COMPLETE) && *o == robot)
    );
}
