// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-build` with `zerodds-types`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-types`, at build time, to write Rust
//! text the consumer compiles:
//! - `src/lib.rs`, `type_identifiers_module`:
//!   `from_ast::type_identifier(.., Form::Complete)`, written as a constant
//!   `TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([..]))` or
//!   `TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId
//!   { equivalence_kind, hash, scc_length, scc_index })`; `type_objects_module`:
//!   `from_ast::minimal_type_object` and `TypeObject::Minimal(..).to_bytes_le`.
//! - `src/dds_type.rs`, `identity_items`: the same identifier as
//!   `TYPE_IDENTIFIER` (a component identifier only when its
//!   `equivalence_kind` is `type_identifier::kinds::EK_COMPLETE`);
//!   `from_ast::complete_type_object` and `from_ast::dependency_type_objects`
//!   written as octets of `TypeObject::Complete(..).to_bytes_le`, kept only if
//!   `TypeObject::from_bytes_le` reads them back, and read at run time with
//!   `TypeObject::from_bytes_le(&[..]).ok()`; `component_items`:
//!   `from_ast::type_component`, its `complete`, `minimal`, `complete_hash`
//!   and `minimal_hash`, written back as a `resolve::TypeComponent` literal.
//!
//! The model is composed with `zerodds-idl-compose`, as `Config::compile`
//! composes a unit.
//!
//! Oracle: DDS-XTypes 1.3 §7.3.4.5 and Annex B, the MINIMAL and COMPLETE
//! octets CycloneDDS 11.0.1 writes for `to::Plain` and `to::Inner` and the
//! COMPLETE hashes CycloneDDS and RTI Connext 7.7.0 report
//! (`crates/types/tests/typeobject_vendor_dheader.rs`); the hand-written
//! COMPLETE object of `@final struct to::Outer { to::Inner i; }`; §7.3.4.9.2
//! and §7.3.4.9.3 for the parts of a component identifier the specification
//! fixes (one hash per component, members numbered from one in the order of
//! their fully qualified names, `TI_STRONGLY_CONNECTED_COMPONENT = 0xB0`).
//! No other implementation computes a component hash, so its value is not
//! asserted.
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_types::from_ast::{
    Form, complete_type_object, dependency_type_objects, minimal_type_object, type_component,
    type_identifier,
};
use zerodds_types::resolve::TypeComponent;
use zerodds_types::type_identifier::kinds::{EK_COMPLETE, TI_STRONGLY_CONNECTED_COMPONENT};
use zerodds_types::type_object::TypeObject;
use zerodds_types::{EquivalenceHash, StronglyConnectedComponentId, TypeIdentifier};

const CORPUS: &str = "\
module to {
  @final struct Plain { long a; short b; };
  @final struct Inner { long x; long y; };
  @final struct Outer { Inner i; };
};";

const PLAIN_MINIMAL: &str = "33000000f1510100010000000000000023000000020000000b000000000000000100040cc175b9000b0000000100000001000392eb5ffe";
const PLAIN_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a506c61696e0000002c0000000200000010000000000000000100040002000000610000001000000001000000010003000200000062000000";
const INNER_COMPLETE: &str = "4c000000f251010012000000000000000a000000746f3a3a496e6e65720000002c0000000200000010000000000000000100040002000000780000001000000001000000010004000200000079000000";
const OUTER_COMPLETE: &str = "48000000f251010012000000000000000a000000746f3a3a4f75746572000000280000000100000020000000000000000100f2f00388e0e0c6619e4175c68e8af80000000200000069000000";

/// The generated form, spelled as `type_identifiers_module` and
/// `identity_items` spell it: CycloneDDS's and RTI's COMPLETE hash of
/// `to::Plain`.
const PLAIN_TYPE_IDENTIFIER: TypeIdentifier =
    TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([
        0x7c, 0x0c, 0x6f, 0xab, 0x7d, 0x06, 0x6a, 0x0c, 0x56, 0xb8, 0xe9, 0x3c, 0x0e, 0xa7,
    ]));

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn compose(idl: &str) -> zerodds_ast::Ast {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        idl,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed(), "the unit composes without an error");
    composed.ast
}

fn id_of(ast: &zerodds_ast::Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    ast.types()
        .find(|(_, node)| node.name == name)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no type `{name}`"))
}

#[test]
fn annex_b_the_identifier_constant_is_the_vendors_complete_hash() {
    let ast = compose(CORPUS);
    assert_eq!(
        type_identifier(&ast, id_of(&ast, "Plain"), Form::Complete).unwrap(),
        PLAIN_TYPE_IDENTIFIER
    );
    assert_eq!(
        type_identifier(&ast, id_of(&ast, "Inner"), Form::Complete).unwrap(),
        TypeIdentifier::EquivalenceHashComplete(EquivalenceHash([
            0xf0, 0x03, 0x88, 0xe0, 0xe0, 0xc6, 0x61, 0x9e, 0x41, 0x75, 0xc6, 0x8e, 0x8a, 0xf8,
        ]))
    );
}

#[test]
fn xtypes_7_3_4_5_the_object_octets_are_the_vendors_and_read_back() {
    let ast = compose(CORPUS);
    let plain = id_of(&ast, "Plain");
    // `type_objects_module`.
    let minimal = TypeObject::Minimal(minimal_type_object(&ast, plain).unwrap())
        .to_bytes_le()
        .unwrap();
    assert_eq!(minimal, unhex(PLAIN_MINIMAL));
    // `identity_items`: written, then filtered by its own round trip, then
    // read at run time.
    let complete = TypeObject::Complete(complete_type_object(&ast, plain).unwrap())
        .to_bytes_le()
        .unwrap();
    assert_eq!(complete, unhex(PLAIN_COMPLETE));
    let read = TypeObject::from_bytes_le(&complete).ok().unwrap();
    assert_eq!(read.to_bytes_le().unwrap(), complete);
    assert!(TypeObject::from_bytes_le(&minimal).is_ok());
}

#[test]
fn xtypes_7_3_4_6_3_the_dependency_constants_are_the_closure_by_hash() {
    let ast = compose(CORPUS);
    let outer = id_of(&ast, "Outer");
    assert_eq!(
        TypeObject::Complete(complete_type_object(&ast, outer).unwrap())
            .to_bytes_le()
            .unwrap(),
        unhex(OUTER_COMPLETE)
    );
    let dependencies: Vec<Vec<u8>> = dependency_type_objects(&ast, outer)
        .unwrap()
        .into_iter()
        .map(|object| TypeObject::Complete(object).to_bytes_le().unwrap())
        .collect();
    assert_eq!(dependencies, [unhex(INNER_COMPLETE)]);
    // A type that names nothing has no dependency constants.
    assert!(
        dependency_type_objects(&ast, id_of(&ast, "Plain"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn xtypes_7_3_4_9_3_a_recursive_type_gets_a_component_identifier_and_its_component() {
    let ast = compose(
        "module m {
           struct B;
           @appendable struct A { long value; sequence<B> bs; };
           @appendable struct B { long weight; sequence<A> items; };
         };",
    );
    let identifier =
        |name: &str| match type_identifier(&ast, id_of(&ast, name), Form::Complete).unwrap() {
            TypeIdentifier::StronglyConnectedComponent(scc) => scc,
            other => panic!("`{name}`: {other:?} is not a component identifier"),
        };
    let a = identifier("A");
    let b = identifier("B");
    assert_eq!(TI_STRONGLY_CONNECTED_COMPONENT, 0xB0);
    assert_eq!(
        TypeIdentifier::StronglyConnectedComponent(a).discriminator(),
        0xB0
    );
    // The form `identity_items` writes as `TYPE_IDENTIFIER`.
    assert_eq!(a.equivalence_kind, EK_COMPLETE);
    assert_eq!(a.hash, b.hash, "one hash for the component");
    assert_eq!((a.scc_length, b.scc_length), (2, 2));
    assert_eq!((a.scc_index, b.scc_index), (1, 2), "`m::A` before `m::B`");

    // `component_items`: the component, whole, in both forms.
    let component = type_component(&ast, id_of(&ast, "A")).unwrap().unwrap();
    assert_eq!(component.complete_hash, a.hash);
    assert_eq!(component.complete.len(), 2);
    assert_eq!(component.minimal.len(), 2);
    for object in &component.complete {
        let bytes = TypeObject::Complete(object.clone()).to_bytes_le().unwrap();
        assert!(TypeObject::from_bytes_le(&bytes).is_ok());
    }
    // The literals the generated code builds compile against these fields.
    let rebuilt = TypeComponent {
        complete_hash: component.complete_hash,
        minimal_hash: component.minimal_hash,
        complete: component.complete.clone(),
        minimal: component.minimal.clone(),
    };
    assert_eq!(rebuilt.complete, component.complete);
    let spelled = TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
        equivalence_kind: a.equivalence_kind,
        hash: a.hash,
        scc_length: a.scc_length,
        scc_index: a.scc_index,
    });
    assert_eq!(spelled, TypeIdentifier::StronglyConnectedComponent(a));
    // A type outside any cycle has no component.
    let plain = compose(CORPUS);
    assert!(
        type_component(&plain, id_of(&plain, "Plain"))
            .unwrap()
            .is_none()
    );
}
