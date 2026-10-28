// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The identifier of a type that reaches itself. XTypes 1.3 §7.3.4.9.2.
//!
//! A recursive type has no `EquivalenceHash` of its own: the hash of an
//! aggregate is taken over its members' identifiers, and one of those
//! members is the aggregate. §7.3.4.9.2 answers the whole cycle at once
//! with `TI_STRONGLY_CONNECTED_COMPONENT` — one hash over the component,
//! and each type numbered within it.
//!
//! ## What these tests can and cannot establish
//!
//! They establish the parts the specification fixes: that a recursive
//! type gets a component identifier rather than a refusal, that the
//! component is ordered by fully qualified name, that numbering starts
//! at one, that every member of a component carries the same hash and a
//! distinct index, and that the computation is deterministic.
//!
//! They do **not** establish that the hash matches another
//! implementation's, and no test here can. Nothing in the interop matrix
//! computes an SCC identifier: Fast DDS declares
//! `build_strongly_connected_component_id` and `fastddsgen` never calls
//! it. The placeholder §7.3.4.9.2 needs for a component-internal
//! reference is an open OMG RTF question, so the hash is a decision this
//! project made and labelled, not a measurement.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_ast::Ast;
use zerodds_types::from_ast::{Form, complete_type_object, type_identifier};
use zerodds_types::type_identifier::TypeIdentifier;

/// `A` holds a `sequence<B>` and `B` a `sequence<A>` — the two-type
/// component. Kept beside the self-referential case because the walker
/// enters them at different points.
const MUTUAL: &str = "
module m {
  struct B;
  @appendable struct A { long value; sequence<B> bs; };
  @appendable struct B { long weight; sequence<A> items; };
};
";

/// One type reaching itself through a sequence: a component of one.
const SELF: &str = "
module m {
  @appendable struct Node {
    long value;
    sequence<Node> children;
  };
};
";

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "scc.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn type_of(ast: &Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    let segments: Vec<String> = name.split("::").map(ToString::to_string).collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("{name} is not in the model"))
}

fn component(
    ast: &Ast,
    name: &str,
) -> zerodds_types::type_identifier::StronglyConnectedComponentId {
    match type_identifier(ast, type_of(ast, name), Form::Complete)
        .unwrap_or_else(|error| panic!("{name}: {error}"))
    {
        TypeIdentifier::StronglyConnectedComponent(scc) => scc,
        other => panic!("{name} is recursive and got {other:?}"),
    }
}

/// A type reaching itself is identified, not refused.
///
/// This is the whole point: `from_ast` used to answer
/// `FromAstError::Recursive`, so a recursive type went on the wire
/// correctly and could not be described in discovery at all.
#[test]
fn a_self_referential_type_gets_a_component_identifier() {
    let ast = model(SELF);
    let scc = component(&ast, "m::Node");
    assert_eq!(scc.scc_length, 1, "a component of one type");
    assert_eq!(scc.scc_index, 1, "§7.3.4.9.2 step 4b numbers from one");
}

/// Both halves of a mutually recursive pair are in one component, carry
/// the same hash, and are numbered 1 and 2 in name order.
#[test]
fn a_mutually_recursive_pair_is_one_component_of_two() {
    let ast = model(MUTUAL);
    let a = component(&ast, "m::A");
    let b = component(&ast, "m::B");

    assert_eq!(a.scc_length, 2);
    assert_eq!(b.scc_length, 2);
    assert_eq!(
        a.hash, b.hash,
        "one component, one hash — the members are numbered within it"
    );
    // Sorted by fully qualified name: `m::A` before `m::B`.
    assert_eq!(a.scc_index, 1, "A sorts first");
    assert_eq!(b.scc_index, 2, "B sorts second");
}

/// The same model asked twice answers the same, which is the minimum a
/// hash used for discovery matching has to do.
#[test]
fn the_component_hash_is_deterministic() {
    let first = component(&model(MUTUAL), "m::A");
    let second = component(&model(MUTUAL), "m::A");
    assert_eq!(first.hash, second.hash);
    assert_eq!(first.scc_index, second.scc_index);
}

/// A type that reaches no cycle keeps its ordinary hashed identifier.
///
/// The guard against the component path swallowing everything: an
/// aggregate that merely holds a sequence is not recursive.
#[test]
fn an_ordinary_type_is_not_given_a_component_identifier() {
    let ast = model("module m { @appendable struct Plain { long v; sequence<long> xs; }; };");
    let identifier = type_identifier(&ast, type_of(&ast, "m::Plain"), Form::Complete).expect("id");
    assert!(
        !matches!(
            identifier,
            TypeIdentifier::StronglyConnectedComponent { .. }
        ),
        "{identifier:?}"
    );
}

/// The `CompleteTypeObject` of a component member builds, and its
/// recursive member carries the component identifier rather than
/// sending the walker round again.
#[test]
fn a_component_members_type_object_builds() {
    let ast = model(SELF);
    let object = complete_type_object(&ast, type_of(&ast, "m::Node")).expect("a type object");
    let rendered = alloc_debug(&object);
    assert!(
        rendered.contains("StronglyConnectedComponent"),
        "the recursive member should carry the component identifier: {rendered}"
    );
}

/// A type outside every component that holds a member of one — the
/// corpus case `recursive-mutual-optional`, `Holder { B root; }` with `A`
/// and `B` reaching each other through an `@optional` member. The walk
/// starts at `Holder`, which belongs to no component, and used to go
/// down into `B`, round through `A` and back into `B`, and answer
/// `FromAstError::Recursive`. The member now carries `B`'s component
/// identifier, the same one `B` gets on its own.
#[test]
fn a_type_that_holds_a_component_member_from_outside_builds() {
    let ast = model(
        "
module m {
  struct B;
  @mutable struct A { long x; @optional B tail; };
  @mutable struct B { long y; A head; };
  @appendable struct Holder { long z; B root; };
};
",
    );
    let object = complete_type_object(&ast, type_of(&ast, "m::Holder")).expect("a type object");
    let rendered = alloc_debug(&object);
    let b = component(&ast, "m::B");
    assert!(
        rendered.contains("StronglyConnectedComponent"),
        "the member should carry the component identifier: {rendered}"
    );
    assert!(
        rendered.contains(&format!("scc_index: {}", b.scc_index)),
        "the member should carry B's index in the component: {rendered}"
    );
    assert!(
        matches!(
            type_identifier(&ast, type_of(&ast, "m::Holder"), Form::Complete),
            Ok(TypeIdentifier::EquivalenceHashComplete(_))
        ),
        "Holder itself is in no component"
    );
}

fn alloc_debug(object: &zerodds_types::type_object::CompleteTypeObject) -> String {
    format!("{object:?}")
}

/// A component member's object rebuilds into a model that holds it under
/// its own name, with the recursive member pointing back at it.
///
/// The round trip discovery depends on: a peer is sent the object and
/// has to arrive at the same type.
#[test]
fn a_component_rebuilds_into_a_model() {
    use zerodds_types::dynamic::to_ast::ModelFromTypeObjects;
    use zerodds_types::from_ast::register_all;
    use zerodds_types::resolve::TypeRegistry;

    let ast = model(SELF);
    let mut registry = TypeRegistry::new();
    let skipped = register_all(&ast, &mut registry).expect("register");
    assert!(skipped.is_empty(), "{skipped:?}");

    let object = complete_type_object(&ast, type_of(&ast, "m::Node")).expect("object");
    let mut builder = ModelFromTypeObjects::new(&registry);
    let id = builder
        .add_type_object(&zerodds_types::type_object::TypeObject::Complete(object))
        .expect("rebuild");
    let rebuilt = builder.finish();
    let node = rebuilt.type_(id).expect("the rebuilt node");
    assert_eq!(node.name, "Node", "{:?}", node.name);
    // Found by the name a peer would ask for, which is what discovery
    // does with it.
    let found = rebuilt.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
        String::from("m"),
        String::from("Node"),
    ]));
    assert!(
        found.is_ok(),
        "m::Node is not in the rebuilt model: {found:?}"
    );
}

// -- §7.3.4.9.2 steps 4b.i and 4c to 4e, §7.3.4.9.3 -----------------------

use zerodds_types::hash::hash_bytes;
use zerodds_types::type_identifier::EquivalenceHash;
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};

/// `TI_STRONGLY_CONNECTED_COMPONENT` identifiers with the hash of
/// step 4b.i: `sc_component_id = {discriminator=EK, hash= 0}`.
fn with_the_temporary_hash(ti: &TypeIdentifier) -> TypeIdentifier {
    match ti {
        TypeIdentifier::StronglyConnectedComponent(scc) => {
            let mut temporary = *scc;
            temporary.hash = EquivalenceHash::ZERO;
            TypeIdentifier::StronglyConnectedComponent(temporary)
        }
        TypeIdentifier::PlainSequenceSmall {
            header,
            bound,
            element,
        } => TypeIdentifier::PlainSequenceSmall {
            header: *header,
            bound: *bound,
            element: Box::new(with_the_temporary_hash(element)),
        },
        other => other.clone(),
    }
}

fn the_object_of_step_4b_ii(ast: &Ast, name: &str) -> Vec<u8> {
    let CompleteTypeObject::Struct(mut object) =
        complete_type_object(ast, type_of(ast, name)).expect("a type object")
    else {
        panic!("{name} is a structure");
    };
    for member in &mut object.member_seq {
        member.common.member_type_id = with_the_temporary_hash(&member.common.member_type_id);
    }
    TypeObject::Complete(CompleteTypeObject::Struct(object))
        .to_bytes_le()
        .expect("the object serialises")
}

/// Steps 4b.i to 4e, with the stream assembled here from the clause:
/// the objects are built with the temporary identifiers (hash zero), put
/// in a `TypeObjectSeq` in the order of their `scc_index`, serialised
/// "using the XCDR serialization for sequences with encoding version 2
/// and little endian" — a `DHEADER` over the `UInt32` count and the
/// elements, each `TypeObject` (itself `@appendable`, with its own
/// `DHEADER`) on a four-octet boundary — and the first 14 octets of the
/// MD5 are the hash.
#[test]
fn the_component_hash_is_the_md5_of_the_xcdr2_sequence_of_its_objects() {
    let ast = model(MUTUAL);
    let first = the_object_of_step_4b_ii(&ast, "m::A");
    let second = the_object_of_step_4b_ii(&ast, "m::B");

    let mut body = Vec::new();
    body.extend_from_slice(&2u32.to_le_bytes());
    body.extend_from_slice(&first);
    while body.len() % 4 != 0 {
        body.push(0);
    }
    body.extend_from_slice(&second);
    let mut stream = Vec::new();
    stream.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
    stream.extend_from_slice(&body);

    assert_eq!(
        component(&ast, "m::A").hash,
        hash_bytes(&stream),
        "the first 14 octets of the MD5 over the serialised TypeObjectSeq"
    );
}

/// §7.3.4.9.3: the SCCIdentifier, which names a component without naming
/// a type in it, has `scc_index = 0`.
#[test]
fn an_scc_identifier_with_index_zero_is_written_and_read() {
    let identifier = TypeIdentifier::StronglyConnectedComponent(
        zerodds_types::type_identifier::StronglyConnectedComponentId {
            equivalence_kind: zerodds_types::type_identifier::kinds::EK_COMPLETE,
            hash: EquivalenceHash([7; 14]),
            scc_length: 3,
            scc_index: 0,
        },
    );
    let bytes = identifier
        .to_bytes_le()
        .expect("scc_index 0 is the SCCIdentifier");
    assert_eq!(
        TypeIdentifier::from_bytes_le(&bytes).expect("reads back"),
        identifier
    );
}
