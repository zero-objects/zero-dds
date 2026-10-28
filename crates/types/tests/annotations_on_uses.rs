// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A declared annotation written on the element of a collection, on a
//! map's key and on a union's discriminator: DDS-XTypes 1.3 §7.2.2.6 and
//! §7.3.1.2.5.
//!
//! §7.2.2.6: an annotation is attached to "an element/member/literal of
//! an aggregated/collection/enumerated type". §7.3.1.2.5: "Annotations
//! can be applied to the implicit discriminator member of a union type by
//! applying them to the discriminator type declaration in the header of
//! the union type's definition: `union MyUnion switch (@MyAnnotation
//! long) {`". Annex B gives `CompleteCollectionElement` and
//! `CompleteDiscriminatorMember` an `ann_custom` for them, and §7.3.4.1
//! makes a collection with such an annotation a non-plain type: it has a
//! `TypeObject` beside its identifier.
//!
//! These tests check the part the specification fixes: where the
//! application lands, that the COMPLETE form has an object for the
//! collection and the MINIMAL form is the plain collection it is without
//! the annotation, and that the object comes back into the model. They
//! do not compare against another implementation: no peer in the matrix
//! writes an annotation on a collection element.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::string::ToString;

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_types::dynamic::dynamic_type;
use zerodds_types::from_ast::{complete_type_object, minimal_type_object, register_all};
use zerodds_types::hash::compute_minimal_hash;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::{EquivalenceHash, PrimitiveKind, TypeIdentifier};
use zerodds_types::type_object::common::{AnnotationParameterValue, OptionalAppliedAnnotationSeq};
use zerodds_types::type_object::{CompleteTypeObject, MinimalTypeObject};

/// `Holder` annotates the element of a sequence, of an array, the key and
/// the value of a map; `Choice` annotates its discriminator.
const ANNOTATED: &str = "
@annotation Mark { long n default 0; };

@final struct Holder {
    sequence<@Mark(5) long, 4> seq;
    long arr @Mark(6) [3];
    map<@Mark(7) long, @Mark(8) long, 4> pairs;
    sequence<long, 4> plain;
};

@final union Choice switch (@Mark(9) long) {
    case 0: long a;
    default: long b;
};
";

/// The same declarations with no annotation on any use.
const PLAIN: &str = "
@annotation Mark { long n default 0; };

@final struct Holder {
    sequence<long, 4> seq;
    long arr [3];
    map<long, long, 4> pairs;
    sequence<long, 4> plain;
};

@final union Choice switch (long) {
    case 0: long a;
    default: long b;
};
";

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "uses.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn type_of(ast: &Ast, name: &str) -> Id<TypeNode> {
    let segments: Vec<String> = name.split("::").map(ToString::to_string).collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("`{name}` is not in the model"))
}

fn registry_of(ast: &Ast) -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    register_all(ast, &mut registry).expect("the registry fills");
    registry
}

/// The COMPLETE identifier of the member `name` of `Holder`, which must be
/// a hash.
fn member_hash(object: &CompleteTypeObject, name: &str) -> EquivalenceHash {
    let CompleteTypeObject::Struct(node) = object else {
        panic!("Holder is not a struct object");
    };
    let member = node
        .member_seq
        .iter()
        .find(|member| member.detail.name == name)
        .unwrap_or_else(|| panic!("no member `{name}`"));
    match &member.common.member_type_id {
        TypeIdentifier::EquivalenceHashComplete(hash) => *hash,
        other => panic!("`{name}` is not hashed in the complete form: {other:?}"),
    }
}

/// The first parameter of the only application in `sequence`.
fn mark(sequence: &OptionalAppliedAnnotationSeq) -> AnnotationParameterValue {
    let applications = sequence
        .0
        .as_ref()
        .unwrap_or_else(|| panic!("`ann_custom` is absent"));
    assert_eq!(applications.len(), 1, "one application");
    applications[0].param_seq[0].value.clone()
}

/// The element of an annotated sequence, array or map carries the
/// application in the COMPLETE object the member's identifier names. The
/// array header names the collection by the containing type and the member
/// (the name §7.3.4.9.2 step 4b gives an anonymous type); the header of a
/// sequence and a map has no detail, which Annex B says is "not present for
/// anonymous" (and Cyclone DDS 11.0.1 does not fill).
#[test]
fn an_annotation_on_an_element_is_in_the_complete_collection_object() {
    let ast = model(ANNOTATED);
    let registry = registry_of(&ast);
    let holder = complete_type_object(&ast, type_of(&ast, "Holder")).expect("Holder builds");

    let seq = registry
        .get_complete(&member_hash(&holder, "seq"))
        .expect("the annotated sequence has an object");
    let CompleteTypeObject::Sequence(seq) = seq else {
        panic!("not a sequence object: {seq:?}");
    };
    assert_eq!(seq.bound, 4);
    assert_eq!(seq.detail, None, "an anonymous sequence has no detail");
    assert_eq!(
        mark(&seq.element.ann_custom),
        AnnotationParameterValue::Int32(5)
    );

    let arr = registry
        .get_complete(&member_hash(&holder, "arr"))
        .expect("the annotated array has an object");
    let CompleteTypeObject::Array(arr) = arr else {
        panic!("not an array object: {arr:?}");
    };
    assert_eq!(arr.bound_seq, [3]);
    assert_eq!(
        mark(&arr.element.ann_custom),
        AnnotationParameterValue::Int32(6)
    );

    let pairs = registry
        .get_complete(&member_hash(&holder, "pairs"))
        .expect("the annotated map has an object");
    let CompleteTypeObject::Map(pairs) = pairs else {
        panic!("not a map object: {pairs:?}");
    };
    assert_eq!(pairs.detail, None, "an anonymous map has no detail");
    assert_eq!(
        mark(&pairs.key.ann_custom),
        AnnotationParameterValue::Int32(7)
    );
    assert_eq!(
        mark(&pairs.element.ann_custom),
        AnnotationParameterValue::Int32(8)
    );
}

/// A collection with no annotation on its element stays a plain
/// collection: no object, and the identifier says what it is (§7.3.4.1).
#[test]
fn a_collection_without_an_annotation_stays_plain() {
    let ast = model(ANNOTATED);
    let holder = complete_type_object(&ast, type_of(&ast, "Holder")).expect("Holder builds");
    let CompleteTypeObject::Struct(node) = &holder else {
        panic!("not a struct object");
    };
    let plain = node
        .member_seq
        .iter()
        .find(|member| member.detail.name == "plain")
        .expect("the member");
    assert!(
        matches!(
            plain.common.member_type_id,
            TypeIdentifier::PlainSequenceSmall { .. }
        ),
        "{:?}",
        plain.common.member_type_id
    );
}

/// The MINIMAL form holds "only the information relevant to
/// assignability" (§7.3.4.4), so the annotation changes nothing in it:
/// the object, and therefore the hash, is that of the unannotated twin.
#[test]
fn the_minimal_form_is_the_one_the_unannotated_declaration_has() {
    let annotated = model(ANNOTATED);
    let plain = model(PLAIN);
    for name in ["Holder", "Choice"] {
        let with = minimal_type_object(&annotated, type_of(&annotated, name)).expect("minimal");
        let without = minimal_type_object(&plain, type_of(&plain, name)).expect("minimal");
        assert_eq!(with, without, "{name}: the minimal objects");
        assert_eq!(
            compute_minimal_hash(&with).expect("hash"),
            compute_minimal_hash(&without).expect("hash"),
            "{name}: the minimal hashes"
        );
    }
    let MinimalTypeObject::Struct(holder) =
        minimal_type_object(&annotated, type_of(&annotated, "Holder")).expect("minimal")
    else {
        panic!("not a struct");
    };
    assert!(
        holder.member_seq.iter().all(|member| matches!(
            member.common.member_type_id,
            TypeIdentifier::PlainSequenceSmall { .. }
                | TypeIdentifier::PlainArraySmall { .. }
                | TypeIdentifier::PlainMapSmall { .. }
        )),
        "every collection member is plain in the minimal form"
    );
}

/// Projecting the COMPLETE object onto its MINIMAL form gives the object
/// `minimal_type_object` builds, annotated collection included: the hash
/// of the collection object does not leak into the minimal identifier.
#[test]
fn the_complete_object_projects_onto_the_same_minimal_object() {
    let ast = model(ANNOTATED);
    let registry = registry_of(&ast);
    let id = type_of(&ast, "Holder");
    let complete = complete_type_object(&ast, id).expect("complete");
    let expected = minimal_type_object(&ast, id).expect("minimal");
    assert_eq!(
        complete.to_minimal(&registry).expect("it projects"),
        expected
    );
}

/// The annotations written on a union's discriminator land in the
/// discriminator's `ann_custom`, and the union's own stays empty.
#[test]
fn an_annotation_on_the_discriminator_is_in_the_discriminator_member() {
    let ast = model(ANNOTATED);
    let choice = complete_type_object(&ast, type_of(&ast, "Choice")).expect("Choice builds");
    let CompleteTypeObject::Union(choice) = choice else {
        panic!("not a union object");
    };
    assert_eq!(
        mark(&choice.discriminator.ann_custom),
        AnnotationParameterValue::Int32(9)
    );
    assert!(
        choice.header.detail.ann_custom.0.is_none(),
        "the union's own applications are not the discriminator's"
    );
    assert_eq!(
        choice.discriminator.common.type_id,
        TypeIdentifier::Primitive(PrimitiveKind::Int32)
    );
}

/// The declaration an annotation on a use names is a dependency of the
/// type, as it is when the annotation sits on a member.
#[test]
fn the_declaration_is_in_the_dependency_closure() {
    let ast = model(
        "
@annotation Mark { long n default 0; };
@final struct OnlyHere { sequence<@Mark(1) long, 4> xs; };
",
    );
    let objects = zerodds_types::from_ast::dependency_type_objects(&ast, type_of(&ast, "OnlyHere"))
        .expect("the closure builds");
    assert!(
        objects
            .iter()
            .any(|object| matches!(object, CompleteTypeObject::Annotation(_))),
        "the declaration of `@Mark` is missing: {objects:?}"
    );
    assert!(
        objects
            .iter()
            .any(|object| matches!(object, CompleteTypeObject::Sequence(_))),
        "the annotated sequence is missing: {objects:?}"
    );
}

/// An annotation the file does not declare is ignored (IDL 4.2
/// §7.4.15.4.2), so the collection it sits on stays plain.
#[test]
fn an_undeclared_annotation_leaves_the_collection_plain() {
    let ast = model("@final struct S { sequence<@Unknown long, 4> xs; };");
    let object = complete_type_object(&ast, type_of(&ast, "S")).expect("S builds");
    let CompleteTypeObject::Struct(node) = &object else {
        panic!("not a struct object");
    };
    assert!(
        matches!(
            node.member_seq[0].common.member_type_id,
            TypeIdentifier::PlainSequenceSmall { .. }
        ),
        "{:?}",
        node.member_seq[0].common.member_type_id
    );
}

/// The objects rebuild into a model that has the applications where they
/// were written: on the element, on the key, on the discriminator.
#[test]
fn the_applications_come_back_into_the_model() {
    let ast = model(ANNOTATED);
    let registry = registry_of(&ast);

    let holder = complete_type_object(&ast, type_of(&ast, "Holder")).expect("Holder builds");
    let rebuilt = dynamic_type(&holder, &registry).expect("Holder rebuilds");
    let rebuilt_ast = rebuilt.ast().clone();
    let holder_id = type_of(&rebuilt_ast, &rebuilt.qualified_name());
    let member_type = |name: &str| {
        let member = rebuilt_ast
            .members_of(holder_id)
            .iter()
            .map(|id| rebuilt_ast.member(*id).expect("member"))
            .find(|member| member.name == name)
            .unwrap_or_else(|| panic!("no member `{name}`"));
        member.type_.resolved().expect("resolved")
    };
    let seq = member_type("seq");
    assert_eq!(rebuilt_ast.element_annotations(seq).len(), 1);
    assert_eq!(rebuilt_ast.element_annotations(seq)[0].name, "Mark");
    let pairs = member_type("pairs");
    assert_eq!(rebuilt_ast.key_annotations(pairs).len(), 1);
    assert_eq!(rebuilt_ast.element_annotations(pairs).len(), 1);

    let choice = complete_type_object(&ast, type_of(&ast, "Choice")).expect("Choice builds");
    let rebuilt = dynamic_type(&choice, &registry).expect("Choice rebuilds");
    let rebuilt_ast = rebuilt.ast().clone();
    let choice_id = type_of(&rebuilt_ast, &rebuilt.qualified_name());
    assert_eq!(rebuilt_ast.discriminator_annotations(choice_id).len(), 1);
    assert!(
        rebuilt_ast
            .type_(choice_id)
            .expect("the union")
            .facets
            .annotations
            .is_empty(),
        "and the union's own list is empty"
    );
}
