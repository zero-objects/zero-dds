// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-idlc` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this tool uses of `zerodds-types` to write the type metadata of the
//! generated bindings:
//! - `src/type_metadata.rs` (`standard_metadata`, the TypeInformation and the
//!   closure of TypeObjects the Java, C#, TypeScript and Python support
//!   embed): `resolve::TypeRegistry` filled by `from_ast::register_all`,
//!   `from_ast::{minimal_type_object, complete_type_object}`,
//!   `TypeInformation::from_minimal_and_complete`, `hash::compute_hash`,
//!   `resolve::collect_referenced_hashes` with `DEFAULT_MAX_RESOLVE_DEPTH`,
//!   `TypeRegistry::{get_minimal, get_complete}`,
//!   `hash::{minimal_serialized_size, complete_serialized_size}`,
//!   `TypeIdentifierWithSize`, `TypeInformation::{to_bytes_le,
//!   from_bytes_le}` and a round trip that has to give the same value back.
//! - `src/java_support.rs`: `hash::{compute_complete_hash,
//!   compute_minimal_hash}`, `TypeIdentifier::to_bytes_le` for the identifier
//!   pairs of the Java TypeObject registry.
//! - `src/c_support.rs`: `hash::compute_hash` of the complete object, the hash
//!   in the C type support.
//! - `src/generate.rs`, `src/csharp_support.rs`, `src/typescript_support.rs`,
//!   `src/python_support.rs`: `TypeObject::Complete(..).to_bytes_le`, kept
//!   only if `TypeObject::from_bytes_le` reads it back.
//!
//! The tool is a binary without a library, so the contract calls
//! `zerodds-types` directly with a model composed by `zerodds-idl-compose`,
//! as the tool composes a file.
//!
//! Oracle: DDS-XTypes 1.3 Annex B and §7.3.4.5, the MINIMAL and COMPLETE
//! octets and hashes CycloneDDS 11.0.1, Fast DDS 3.6.1 and RTI Connext 7.7.0
//! give for `to::Plain` and `to::Inner`
//! (`crates/types/tests/typeobject_vendor_dheader.rs`); §7.6.3.2.1 (the
//! `typeobject_serialized_size` is the length of the hashed stream; the
//! `@mutable TypeInformation` with members 0x1001 and 0x1002, here in the
//! EMHEADER length code 4 CycloneDDS writes, §7.4.3.4.2); §7.3.4.6.3 and
//! §7.3.4.6.5 (a member type is named by the hash of the form being
//! hashed).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::from_ast::{complete_type_object, minimal_type_object, register_all};
use zerodds_types::hash::{
    complete_serialized_size, compute_complete_hash, compute_hash, compute_minimal_hash,
    minimal_serialized_size,
};
use zerodds_types::resolve::{DEFAULT_MAX_RESOLVE_DEPTH, TypeRegistry, collect_referenced_hashes};
use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, TypeIdentifier, TypeInformation};

const CORPUS: &str = "\
module to {
  @final struct Plain { long a; short b; };
  @final struct Inner { long x; long y; };
  @final struct Outer { Inner i; };
};";

const PLAIN_MINIMAL_HASH: &str = "deb922f040f7c228b25728be51f6";
const PLAIN_COMPLETE_HASH: &str = "7c0c6fab7d066a0c56b8e93c0ea7";
const INNER_MINIMAL_HASH: &str = "0c380bda28dab0250db24ae23c16";
const INNER_COMPLETE_HASH: &str = "f00388e0e0c6619e4175c68e8af8";
const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";

/// The `TypeInformation` of `to::Plain`: a DHEADER of 96, then the two
/// members, each an EMHEADER with length code 4, a NEXTINT of 40 and the
/// `TypeIdentifierWithDependencies` (DHEADER 36; the identifier with size
/// behind a DHEADER of 20; a dependency count of 0; the empty sequence).
const PLAIN_TYPE_INFORMATION: &str = "6000000001100040280000002400000014000000f1deb922f040f7c228b25728be51f6003700000000000000040000000000000002100040280000002400000014000000f27c0c6fab7d066a0c56b8e93c0ea70050000000000000000400000000000000";

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn hash(hex: &str) -> EquivalenceHash {
    let bytes = unhex(hex);
    let mut out = [0u8; 14];
    out.copy_from_slice(&bytes);
    EquivalenceHash(out)
}

fn compose() -> zerodds_ast::Ast {
    let composed = zerodds_idl_compose::compose_source(
        "corpus.idl",
        CORPUS,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed(), "the corpus composes without an error");
    composed.ast
}

fn id_of(ast: &zerodds_ast::Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no type `{name}`"))
}

#[test]
fn xtypes_7_6_3_2_1_the_type_information_names_both_hashes_and_the_hashed_lengths() {
    let ast = compose();
    let plain = id_of(&ast, "Plain");
    let minimal = minimal_type_object(&ast, plain).unwrap();
    let complete = complete_type_object(&ast, plain).unwrap();
    let info = TypeInformation::from_minimal_and_complete(&minimal, &complete).unwrap();
    assert_eq!(
        info.minimal.typeid_with_size.type_id,
        TypeIdentifier::EquivalenceHashMinimal(hash(PLAIN_MINIMAL_HASH))
    );
    assert_eq!(info.minimal.typeid_with_size.typeobject_serialized_size, 55);
    assert_eq!(
        info.complete.typeid_with_size.type_id,
        TypeIdentifier::EquivalenceHashComplete(hash(PLAIN_COMPLETE_HASH))
    );
    assert_eq!(
        info.complete.typeid_with_size.typeobject_serialized_size,
        80
    );
    assert_eq!(info.minimal.dependent_typeid_count, 0);
    assert_eq!(info.complete.dependent_typeid_count, 0);

    let bytes = info.to_bytes_le().unwrap();
    assert_eq!(bytes, unhex(PLAIN_TYPE_INFORMATION));
    assert_eq!(TypeInformation::from_bytes_le(&bytes).unwrap(), info);
}

#[test]
fn xtypes_7_3_4_6_5_the_closure_is_followed_by_the_hash_of_each_form() {
    let ast = compose();
    let mut registry = TypeRegistry::new();
    assert!(register_all(&ast, &mut registry).unwrap().is_empty());
    let outer = id_of(&ast, "Outer");

    let minimal_root = TypeObject::Minimal(minimal_type_object(&ast, outer).unwrap());
    let minimal_dependencies =
        collect_referenced_hashes(&minimal_root, DEFAULT_MAX_RESOLVE_DEPTH).unwrap();
    assert_eq!(minimal_dependencies, [hash(INNER_MINIMAL_HASH)]);
    let inner_minimal = registry.get_minimal(&minimal_dependencies[0]).unwrap();
    assert_eq!(minimal_serialized_size(inner_minimal).unwrap(), 55);

    let complete_root = TypeObject::Complete(complete_type_object(&ast, outer).unwrap());
    let complete_dependencies =
        collect_referenced_hashes(&complete_root, DEFAULT_MAX_RESOLVE_DEPTH).unwrap();
    assert_eq!(complete_dependencies, [hash(INNER_COMPLETE_HASH)]);
    let inner_complete = registry.get_complete(&complete_dependencies[0]).unwrap();
    assert_eq!(complete_serialized_size(inner_complete).unwrap(), 80);

    // `compute_hash` of each object is the key it is filed and named under.
    assert_eq!(
        compute_hash(&TypeObject::Minimal(inner_minimal.clone())).unwrap(),
        hash(INNER_MINIMAL_HASH)
    );
    assert_eq!(
        compute_hash(&TypeObject::Complete(inner_complete.clone())).unwrap(),
        hash(INNER_COMPLETE_HASH)
    );
    // A leaf names nothing.
    assert!(
        collect_referenced_hashes(
            &TypeObject::Complete(inner_complete.clone()),
            DEFAULT_MAX_RESOLVE_DEPTH
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn annex_b_the_identifier_pairs_and_the_c_hash_are_the_vendors() {
    let ast = compose();
    let plain = id_of(&ast, "Plain");
    let complete = complete_type_object(&ast, plain).unwrap();
    let minimal = minimal_type_object(&ast, plain).unwrap();
    // `java_support.rs`.
    assert_eq!(
        compute_complete_hash(&complete).unwrap(),
        hash(PLAIN_COMPLETE_HASH)
    );
    assert_eq!(
        compute_minimal_hash(&minimal).unwrap(),
        hash(PLAIN_MINIMAL_HASH)
    );
    let mut identifier = vec![0xF2];
    identifier.extend_from_slice(&unhex(PLAIN_COMPLETE_HASH));
    assert_eq!(
        TypeIdentifier::EquivalenceHashComplete(hash(PLAIN_COMPLETE_HASH))
            .to_bytes_le()
            .unwrap(),
        identifier
    );
    identifier[0] = 0xF1;
    identifier[1..].copy_from_slice(&unhex(PLAIN_MINIMAL_HASH));
    assert_eq!(
        TypeIdentifier::EquivalenceHashMinimal(hash(PLAIN_MINIMAL_HASH))
            .to_bytes_le()
            .unwrap(),
        identifier
    );
    // `c_support.rs` and the embedded objects of the other supports.
    let object = TypeObject::Complete(complete);
    assert_eq!(compute_hash(&object).unwrap(), hash(PLAIN_COMPLETE_HASH));
    let bytes = object.to_bytes_le().unwrap();
    assert_eq!(bytes, unhex(PLAIN_COMPLETE));
    assert_eq!(TypeObject::from_bytes_le(&bytes).unwrap(), object);
}
