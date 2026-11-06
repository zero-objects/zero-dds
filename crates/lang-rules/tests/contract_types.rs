// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-lang-rules` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`:
//! - `src/adapter/interface.rs` (`operation_hash`, `exception_hash`):
//!   `hash::hash_bytes` of the name, its first four octets read little
//!   endian as the `long` case label of a DDS-RPC `Call`/`Return` union.
//! - `src/emit/interface_rust/type_information.rs` (the TypeObjects and the
//!   `PID_TYPE_INFORMATION` of a service's request and reply topics):
//!   `builder::TypeObjectBuilder::struct_type` with `Extensibility::Final`
//!   or `Mutable`, `member(.., |m| m.id(n))` and `.optional()`,
//!   `TypeObjectBuilder::enum_type(..).literal(..)`, `build_complete`;
//!   `from_ast::{type_identifier(.., Form::Complete), complete_type_object,
//!   dependency_type_objects}`; `compute_hash`, `TypeObject::to_bytes_le`,
//!   `resolve::{TypeRegistry::insert_complete, collect_referenced_hashes,
//!   DEFAULT_MAX_RESOLVE_DEPTH}`, `TypeInformation::from_registry` and
//!   `CompleteTypeObject::to_minimal`.
//!
//! The model of the IDL parameter types is composed with
//! `zerodds-idl-compose`, a dependency of this crate.
//!
//! Oracle: DDS-RPC 1.0 §7.5.1.1.2 (the HASH is MD5 of the name, octets 0 to
//! 3 little endian) with RFC 1321 MD5, computed outside this tree;
//! DDS-XTypes 1.3 §7.3.4.4 (the minimal object is the complete one without
//! names, its member names replaced by their four-octet MD5) and §7.3.4.5
//! with the MINIMAL and COMPLETE octets and hashes CycloneDDS 11.0.1, Fast DDS
//! 3.6.1 and RTI Connext 7.7.0 give for `to::Plain` and `to::Inner`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`); the COMPLETE and
//! MINIMAL objects of `@final struct to::Outer { to::Inner i; }` written out
//! by hand with the same layout (MD5 computed outside this tree);
//! §7.6.3.2.1 (dependencies listed with the sizes of their streams);
//! Annex B `MemberFlag` (`IS_OPTIONAL` = bit 3) and `EnumeratedLiteralFlag`
//! (`IS_DEFAULT` = bit 6, on the first literal when none is annotated,
//! §7.3.1.2.1.10).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::from_ast::{
    Form, complete_type_object, dependency_type_objects, type_identifier,
};
use zerodds_types::hash::hash_bytes;
use zerodds_types::resolve::{DEFAULT_MAX_RESOLVE_DEPTH, TypeRegistry, collect_referenced_hashes};
use zerodds_types::type_information::TypeInformation;
use zerodds_types::type_object::flags::{EnumLiteralFlag, StructMemberFlag, StructTypeFlag};
use zerodds_types::{
    CompleteTypeObject, EquivalenceHash, PrimitiveKind, TypeIdentifier, TypeObject, compute_hash,
};

const PLAIN_MINIMAL: &str = "33000000f1510100010000000000000023000000020000000b000000000000000100040cc175b9000b0000000100000001000392eb5ffe";
const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
const INNER_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a496e6e65720000002c0000000200000010000000000000000100040002000000780000001000000001000000010004000200000079000000";
const INNER_COMPLETE_HASH: &str = "f00388e0e0c6619e4175c68e8af8";
const INNER_MINIMAL_HASH: &str = "0c380bda28dab0250db24ae23c16";
const OUTER_COMPLETE: &str = "48000000f251010012000000000000000a000000746f3a3a4f75746572000000280000000100000020000000000000000100f2f00388e0e0c6619e4175c68e8af80000000200000069000000";
const OUTER_COMPLETE_HASH: &str = "9123718cb72c3e15bdec188034f7";
/// `to::Outer`, MINIMAL: DHEADER 49, `EK_MINIMAL`, `TK_STRUCTURE`,
/// `IS_FINAL`; the header (DHEADER 1, `TK_NONE`); pad; the member sequence
/// (DHEADER 33, count 1) with one member (DHEADER 25: id 0, flags
/// `TRY_CONSTRUCT1`, `EK_MINIMAL` + the minimal hash of `to::Inner`,
/// `NameHash("i") = 865c0c0b`).
const OUTER_MINIMAL: &str = "31000000f15101000100000000000000210000000100000019000000000000000100f10c380bda28dab0250db24ae23c16865c0c0b";
const OUTER_MINIMAL_HASH: &str = "e25add4218729f55a07622ee38c4";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn hash(hex: &str) -> EquivalenceHash {
    let mut out = [0u8; 14];
    out.copy_from_slice(&unhex(hex));
    EquivalenceHash(out)
}

fn complete(hex: &str) -> CompleteTypeObject {
    let TypeObject::Complete(object) = TypeObject::from_bytes_le(&unhex(hex)).unwrap() else {
        panic!("a COMPLETE object");
    };
    object
}

#[test]
fn rpc_7_5_1_1_2_the_case_label_of_an_operation_is_its_hash() {
    // MD5("getName") opens 32 13 2c 89, MD5("add") 34 ec 78 fc.
    let label = |name: &str| {
        let digest = hash_bytes(name.as_bytes());
        i32::from_le_bytes([digest.0[0], digest.0[1], digest.0[2], digest.0[3]])
    };
    assert_eq!(label("getName"), -1_993_600_206);
    assert_eq!(label("add"), -59_184_076);
}

#[test]
fn xtypes_7_3_4_4_the_minimal_projection_is_the_vendors_minimal_object() {
    // A type of primitives projects without the registry.
    let plain = complete(PLAIN_COMPLETE);
    let projected = plain.to_minimal(&TypeRegistry::new()).unwrap();
    assert_eq!(
        TypeObject::Minimal(projected).to_bytes_le().unwrap(),
        unhex(PLAIN_MINIMAL)
    );

    // A member of an aggregated type is named by that type's minimal hash,
    // which the registry's complete object of it gives.
    let outer = complete(OUTER_COMPLETE);
    assert!(outer.to_minimal(&TypeRegistry::new()).is_err());
    let mut registry = TypeRegistry::new();
    let inner = complete(INNER_COMPLETE);
    registry.insert_complete(
        compute_hash(&TypeObject::Complete(inner.clone())).unwrap(),
        inner,
    );
    let projected = TypeObject::Minimal(outer.to_minimal(&registry).unwrap());
    assert_eq!(projected.to_bytes_le().unwrap(), unhex(OUTER_MINIMAL));
    assert_eq!(compute_hash(&projected).unwrap(), hash(OUTER_MINIMAL_HASH));
    assert_eq!(
        collect_referenced_hashes(&projected, DEFAULT_MAX_RESOLVE_DEPTH).unwrap(),
        [hash(INNER_MINIMAL_HASH)]
    );
}

#[test]
fn xtypes_7_6_3_2_1_the_type_information_lists_the_dependencies_with_their_sizes() {
    let mut registry = TypeRegistry::new();
    for hex in [INNER_COMPLETE, OUTER_COMPLETE] {
        let object = complete(hex);
        let key = compute_hash(&TypeObject::Complete(object.clone())).unwrap();
        registry.insert_complete(key, object);
    }
    let info = TypeInformation::from_registry(&complete(OUTER_COMPLETE), &registry).unwrap();
    assert_eq!(
        info.complete.typeid_with_size.type_id,
        TypeIdentifier::EquivalenceHashComplete(hash(OUTER_COMPLETE_HASH))
    );
    assert_eq!(
        info.complete.typeid_with_size.typeobject_serialized_size,
        76
    );
    assert_eq!(
        info.minimal.typeid_with_size.type_id,
        TypeIdentifier::EquivalenceHashMinimal(hash(OUTER_MINIMAL_HASH))
    );
    assert_eq!(info.minimal.typeid_with_size.typeobject_serialized_size, 53);
    assert_eq!(info.complete.dependent_typeid_count, 1);
    assert_eq!(
        info.complete.dependent_typeids[0].type_id,
        TypeIdentifier::EquivalenceHashComplete(hash(INNER_COMPLETE_HASH))
    );
    assert_eq!(
        info.complete.dependent_typeids[0].typeobject_serialized_size,
        80
    );
    assert_eq!(info.minimal.dependent_typeid_count, 1);
    assert_eq!(
        info.minimal.dependent_typeids[0].type_id,
        TypeIdentifier::EquivalenceHashMinimal(hash(INNER_MINIMAL_HASH))
    );
    assert_eq!(
        info.minimal.dependent_typeids[0].typeobject_serialized_size,
        55
    );
    let bytes = info.to_bytes_le().unwrap();
    assert_eq!(TypeInformation::from_bytes_le(&bytes).unwrap(), info);
}

#[test]
fn xtypes_7_3_4_5_the_builder_writes_the_service_structures_as_asked() {
    // A `Result` structure of the enhanced mapping: mutable, optional
    // members with explicit ids.
    let result = TypeObjectBuilder::struct_type("calc::Calculator_add_Result")
        .extensibility(Extensibility::Mutable)
        .member("ok", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| {
            m.id(7).optional()
        })
        .member(
            "unknown",
            TypeIdentifier::Primitive(PrimitiveKind::Byte),
            |m| m.id(9).optional(),
        )
        .build_complete();
    assert_eq!(
        result.struct_flags,
        StructTypeFlag(StructTypeFlag::IS_MUTABLE)
    );
    let members: Vec<(u32, bool)> = result
        .member_seq
        .iter()
        .map(|m| {
            (
                m.common.member_id,
                m.common.member_flags.has(StructMemberFlag::IS_OPTIONAL),
            )
        })
        .collect();
    assert_eq!(members, [(7, true), (9, true)]);

    // An `In` structure: final, members numbered from 0.
    let in_struct = TypeObjectBuilder::struct_type("calc::Calculator_add_In")
        .extensibility(Extensibility::Final)
        .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
        .build_complete();
    assert_eq!(
        in_struct.struct_flags,
        StructTypeFlag(StructTypeFlag::IS_FINAL)
    );
    assert_eq!(in_struct.member_seq[0].common.member_id, 0);
    assert!(
        !in_struct.member_seq[0]
            .common
            .member_flags
            .has(StructMemberFlag::IS_OPTIONAL)
    );

    // `RemoteExceptionCode_t`: the six literals 0..5, the first the default.
    let mut builder = TypeObjectBuilder::enum_type("dds::rpc::RemoteExceptionCode_t");
    for (value, literal) in ["REMOTE_EX_OK", "REMOTE_EX_UNSUPPORTED"].iter().enumerate() {
        builder = builder.literal(*literal, i32::try_from(value).unwrap());
    }
    let remote = builder.build_complete();
    let literals: Vec<(i32, bool)> = remote
        .literal_seq
        .iter()
        .map(|l| {
            (
                l.common.value,
                l.common.flags.0 & EnumLiteralFlag::IS_DEFAULT_LITERAL != 0,
            )
        })
        .collect();
    assert_eq!(literals, [(0, true), (1, false)]);
    let object = TypeObject::Complete(CompleteTypeObject::Enumerated(remote));
    assert!(TypeObject::from_bytes_le(&object.to_bytes_le().unwrap()).is_ok());
}

#[test]
fn xtypes_7_3_4_6_1_a_parameter_type_gives_its_identifier_and_its_objects() {
    let composed = zerodds_idl_compose::compose_source(
        "service.idl",
        "module to {
           @final struct Inner { long x; long y; };
           @final struct Outer { Inner i; };
         };",
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed(), "the IDL composes without an error");
    let ast = &composed.ast;
    let id = |name: &str| {
        ast.types()
            .find(|(_, node)| node.name == name)
            .map(|(id, _)| id)
            .unwrap()
    };
    let outer = id("Outer");
    assert_eq!(
        type_identifier(ast, outer, Form::Complete).unwrap(),
        TypeIdentifier::EquivalenceHashComplete(hash(OUTER_COMPLETE_HASH))
    );
    assert_eq!(
        complete_type_object(ast, outer).unwrap(),
        complete(OUTER_COMPLETE)
    );
    assert_eq!(
        dependency_type_objects(ast, outer).unwrap(),
        [complete(INNER_COMPLETE)]
    );
}
