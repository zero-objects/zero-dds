// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The builtin member annotations, through the wire form and back.
//!
//! DDS-XTypes 1.3, Annex B, gives `AppliedBuiltinMemberAnnotations` four
//! members: `@unit`, `@min`, `@max` and the `@hashid` source name. None of
//! them moves a data octet — they are constraints and documentation, not
//! framing — so `type_object_bridge.rs`'s byte comparison cannot see
//! them. This measures them directly: a declaration lowers to a model,
//! the model writes its `CompleteTypeObject`, the object goes through
//! its serialized form, and the model rebuilt from what came back has
//! to say what the declaration said.
//!
//! `@default` has no slot there. The object the model writes carries it
//! in memory; the serialized form does not, as Cyclone DDS's and Fast
//! DDS's do not (`typeobject_member_annotations.rs` has the vendor
//! octets).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, ConstValue, MemberIdOrigin, MemberNode, XTypesFacet};
use zerodds_ast_core::Id;
use zerodds_types::from_ast::{complete_type_object, register_all};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_object::common::{AnnotationParameterValue, NameHash};
use zerodds_types::type_object::{CompleteTypeObject, TypeObject};

const IDL: &str = r#"
@appendable
struct Sample {
    @unit("celsius") @min(-40) @max(125) short temperature;
    @default(2.5) double factor;
    @default("none") string label;
    @hashid("alpha") long renamed;
    @default(TRUE) boolean armed;
    @min(0.5) float ratio;
    long plain;
};

union Choice switch (long) {
    case 1: @unit("m") @min(0) long distance;
    default: octet raw;
};
"#;

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "type_object_annotations.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("the resolve pass");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn type_id(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    let segments: Vec<String> = name.split("::").map(str::to_string).collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("`{name}` is not in the model"))
}

/// The member of that name, with its facet.
fn member<'a>(ast: &'a Ast, type_: Id<zerodds_ast::TypeNode>, name: &str) -> &'a MemberNode {
    ast.members_of(type_)
        .iter()
        .map(|id| ast.member(*id).expect("a member the type listed"))
        .find(|member| member.name == name)
        .unwrap_or_else(|| panic!("no member `{name}`"))
}

fn facet<'a>(ast: &'a Ast, type_: Id<zerodds_ast::TypeNode>, name: &str) -> &'a XTypesFacet {
    member(ast, type_, name)
        .facets
        .xtypes
        .as_ref()
        .unwrap_or_else(|| panic!("`{name}` carries no XTypes facet"))
}

/// Serialize and decode again: what the peer receives is what the test
/// rebuilds from.
fn through_the_wire(object: &CompleteTypeObject) -> CompleteTypeObject {
    let bytes = TypeObject::Complete(object.clone())
        .to_bytes_le()
        .expect("the object encodes");
    match TypeObject::from_bytes_le(&bytes).expect("the octets decode") {
        TypeObject::Complete(decoded) => decoded,
        other => panic!("a complete object came back as {other:?}"),
    }
}

#[test]
fn the_builtin_member_annotations_reach_the_object() {
    let ast = model(IDL);
    let object = complete_type_object(&ast, type_id(&ast, "Sample")).expect("the object builds");
    let CompleteTypeObject::Struct(st) = &object else {
        panic!("not a struct object");
    };
    let detail = |name: &str| {
        &st.member_seq
            .iter()
            .find(|member| member.detail.name == name)
            .unwrap_or_else(|| panic!("no member `{name}`"))
            .detail
            .ann_builtin
    };

    let temperature = detail("temperature");
    assert_eq!(temperature.unit.as_deref(), Some("celsius"));
    assert_eq!(temperature.min, Some(AnnotationParameterValue::Int16(-40)));
    assert_eq!(temperature.max, Some(AnnotationParameterValue::Int16(125)));

    assert_eq!(detail("factor").default_value.as_deref(), Some("2.5"));
    assert_eq!(detail("label").default_value.as_deref(), Some("none"));
    assert_eq!(detail("renamed").hash_id.as_deref(), Some("alpha"));
    assert_eq!(detail("armed").default_value.as_deref(), Some("TRUE"));
    assert_eq!(
        detail("ratio").min,
        Some(AnnotationParameterValue::Float32Bits(0.5f32.to_bits()))
    );
    assert!(detail("plain").is_empty());

    // The wire pass loses none of it but `@default`.
    let mut without_defaults = object.clone();
    if let CompleteTypeObject::Struct(st) = &mut without_defaults {
        for member in &mut st.member_seq {
            member.detail.ann_builtin.default_value = None;
        }
    }
    assert_eq!(through_the_wire(&object), without_defaults);
}

#[test]
fn the_annotations_come_back_into_the_rebuilt_model() {
    let ast = model(IDL);
    let object = complete_type_object(&ast, type_id(&ast, "Sample")).expect("the object builds");
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("the registry fills");

    let decoded = through_the_wire(&object);
    let rebuilt = zerodds_types::dynamic::dynamic_type(&decoded, &registry).expect("it rebuilds");
    let rebuilt_ast = rebuilt.ast().clone();
    let sample = type_id(&rebuilt_ast, &rebuilt.qualified_name());

    let temperature = facet(&rebuilt_ast, sample, "temperature");
    assert_eq!(temperature.unit.as_deref(), Some("celsius"));
    let range = temperature.range.as_ref().expect("`@min`/`@max` survive");
    assert_eq!(
        rebuilt_ast.eval_const(range.min.expect("a lower bound")),
        Ok(ConstValue::Integer(-40))
    );
    assert_eq!(
        rebuilt_ast.eval_const(range.max.expect("an upper bound")),
        Ok(ConstValue::Integer(125))
    );

    // `@default` has no slot on the wire; the rebuilt model has none.
    for name in ["factor", "label", "armed"] {
        assert!(
            facet(&rebuilt_ast, sample, name).default_value.is_none(),
            "`{name}`: a default that did not travel"
        );
    }

    let renamed = facet(&rebuilt_ast, sample, "renamed");
    let assignment = renamed.member_id.as_ref().expect("an id came over");
    assert_eq!(
        assignment.origin,
        MemberIdOrigin::Hashed {
            source_name: "alpha".to_string()
        }
    );
    assert_eq!(assignment.effective, NameHash::member_id_from_name("alpha"));

    let ratio = facet(&rebuilt_ast, sample, "ratio");
    let ratio_range = ratio.range.as_ref().expect("`@min` survives");
    assert_eq!(
        rebuilt_ast.eval_const(ratio_range.min.expect("a lower bound")),
        Ok(ConstValue::Float {
            mantissa: "0.5".to_string(),
            exponent: None
        })
    );
    assert!(ratio_range.max.is_none());

    let plain = member(&rebuilt_ast, sample, "plain");
    if let Some(facet) = plain.facets.xtypes.as_ref() {
        assert_eq!(facet.unit, None);
        assert!(facet.range.is_none());
        assert!(facet.default_value.is_none());
        if let Some(assignment) = facet.member_id.as_ref() {
            assert!(!matches!(assignment.origin, MemberIdOrigin::Hashed { .. }));
        }
    }
}

#[test]
fn a_union_member_carries_them_too() {
    let ast = model(IDL);
    let object = complete_type_object(&ast, type_id(&ast, "Choice")).expect("the object builds");
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("the registry fills");

    let decoded = through_the_wire(&object);
    let CompleteTypeObject::Union(u) = &decoded else {
        panic!("not a union object");
    };
    let distance = &u
        .member_seq
        .iter()
        .find(|member| member.detail.name == "distance")
        .expect("the annotated branch")
        .detail
        .ann_builtin;
    assert_eq!(distance.unit.as_deref(), Some("m"));
    assert_eq!(distance.min, Some(AnnotationParameterValue::Int32(0)));

    let rebuilt = zerodds_types::dynamic::dynamic_type(&decoded, &registry).expect("it rebuilds");
    let rebuilt_ast = rebuilt.ast().clone();
    let choice = type_id(&rebuilt_ast, &rebuilt.qualified_name());
    let zerodds_ast::TypeKind::Union { cases, .. } =
        &rebuilt_ast.type_(choice).expect("the union node").kind
    else {
        panic!("not a union in the rebuilt model");
    };
    let distance = cases
        .iter()
        .map(|id| {
            let case = rebuilt_ast.case(*id).expect("a case the union listed");
            rebuilt_ast.member(case.member).expect("the case's member")
        })
        .find(|member| member.name == "distance")
        .expect("the annotated branch came back");
    let distance = distance
        .facets
        .xtypes
        .as_ref()
        .expect("`distance` carries a facet");
    assert_eq!(distance.unit.as_deref(), Some("m"));
    let range = distance.range.as_ref().expect("`@min` survives");
    assert_eq!(
        rebuilt_ast.eval_const(range.min.expect("a lower bound")),
        Ok(ConstValue::Integer(0))
    );
}

// ---------------------------------------------------------------------
// TK_ANNOTATION — Annex B's other TypeObject
// ---------------------------------------------------------------------

/// An `@annotation` declaration has a `TypeObject`, and it can be built
/// from IDL.
///
/// Annex B gives `MinimalTypeObject` a
/// `case TK_ANNOTATION: MinimalAnnotationType annotation_type;` and
/// `CompleteTypeObject` its counterpart, and Table 29 lists
/// `TK_ANNOTATION` as a TypeIdentifier discriminator. The variants
/// existed in this crate and nothing could reach them from a
/// declaration: an `@annotation` is an `AnnotationDefNode` and not a
/// `TypeNode`, so `@annotation A { long v; };` left the model with no
/// type named `A` and `minimal_type_object` had nothing to be asked
/// about. `docs/spec-coverage/dds-xtypes-1.3.gaps.md` §A.1.
#[test]
fn an_annotation_declaration_has_a_type_object() {
    use zerodds_types::from_ast::{Form, annotation_type_object};
    use zerodds_types::type_object::MinimalTypeObject;

    let source = "\
@annotation Verbosity {
    long level default 3;
    string name default \"quiet\";
    boolean loud;
};";
    let parsed = zerodds_idl_parse::parse(source);
    assert!(
        !parsed.diagnostics().iter().any(|d| d.is_error()),
        "{:?}",
        parsed.diagnostics()
    );
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "annotation.idl").ast;
    let report = ast.resolve_all_by_name().expect("resolution");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);

    let (id, _) = ast
        .annotation_defs()
        .find(|(_, node)| node.name == "Verbosity")
        .expect("the declaration");

    let minimal = annotation_type_object(&ast, id, Form::Minimal).expect("minimal");
    let TypeObject::Minimal(MinimalTypeObject::Annotation(ref annotation)) = minimal else {
        panic!("expected TK_ANNOTATION: {minimal:?}");
    };
    // Three parameters. A `MinimalAnnotationParameter` names its parameter
    // by the `NameHash` Annex B gives it, and §7.3.4.5 orders the sequence
    // by that hash — which is not the declaration order.
    let name_of = |p: &zerodds_types::type_object::minimal::MinimalAnnotationParameter| {
        ["level", "name", "loud"]
            .iter()
            .find(|name| NameHash::from_name(name) == p.name_hash)
            .copied()
    };
    let names: Vec<&str> = annotation
        .member_seq
        .iter()
        .map(|p| name_of(p).expect("a parameter of the declaration"))
        .collect();
    // NameHash("loud") = 51 ed f3 fe, ("name") = b0 68 93 1c,
    // ("level") = c9 e9 a8 48.
    assert_eq!(
        names,
        vec!["loud", "name", "level"],
        "increasing name_hash: {:?}",
        annotation
            .member_seq
            .iter()
            .map(|p| p.name_hash.0)
            .collect::<Vec<_>>()
    );
    // A declared default is the value in its own kind. `loud` declares
    // none: `default_value` is not `@optional` in Annex B, so it takes the
    // default value of its type — `FALSE` for a `boolean`, per §7.3.1.2.4
    // and Table 9.
    let default_of = |name: &str| {
        annotation
            .member_seq
            .iter()
            .find(|p| name_of(p) == Some(name))
            .map(|p| p.default_value.clone())
            .unwrap_or_else(|| panic!("no parameter `{name}`"))
    };
    assert_eq!(default_of("level"), AnnotationParameterValue::Int32(3));
    assert_eq!(default_of("loud"), AnnotationParameterValue::Boolean(false));
    assert_eq!(
        default_of("name"),
        AnnotationParameterValue::String8("quiet".into())
    );

    // And it survives the wire form, `DHEADER` and TypeKind octet and
    // all.
    let bytes = minimal.to_bytes_le().expect("encode");
    assert_eq!(
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize,
        bytes.len() - 4
    );
    assert_eq!(bytes[5], 0x50, "TK_ANNOTATION");
    assert_eq!(TypeObject::from_bytes_le(&bytes).expect("decode"), minimal);

    // The complete form carries the qualified name.
    let complete = annotation_type_object(&ast, id, Form::Complete).expect("complete");
    let TypeObject::Complete(CompleteTypeObject::Annotation(ref c)) = complete else {
        panic!("expected TK_ANNOTATION: {complete:?}");
    };
    // The complete header is `CompleteAnnotationHeader`, whose one member
    // is the qualified name — not a `CompleteTypeDetail`.
    assert_eq!(c.header.annotation_name, "Verbosity");
    // §7.3.4.5 orders the complete sequence by `member_index`, the
    // declaration order (`xtypes.annotation_parameter_order` is the other
    // reading); the declaration here is level, name, loud.
    let complete_names: Vec<&str> = c.member_seq.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(complete_names, vec!["level", "name", "loud"]);
    let bytes = complete.to_bytes_le().expect("encode");
    assert_eq!(TypeObject::from_bytes_le(&bytes).expect("decode"), complete);
}
