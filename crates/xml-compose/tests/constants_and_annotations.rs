// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XTypes 1.3 §7.3.2 for the XML loader: constants, `<const_exp>`
//! bounds, file inclusion, bitsets, the order of the collection
//! attributes, verbatim text, annotation declarations and applications.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use zerodds_ast::{AnyId, Bound, ConstExpr, TypeKind, TypeNode, VerbatimPlacement};
use zerodds_ast_core::{Id, Ref, ScopedName};
use zerodds_xml_compose::{Composed, compose, compose_source};

fn load(body: &str) -> Composed {
    compose_source(
        "t.xml",
        &format!("<dds><types><module name=\"T\">{body}</module></types></dds>"),
    )
}

fn type_in(composed: &Composed, path: &[&str]) -> Id<TypeNode> {
    composed
        .ast
        .find_by_scoped_name(&ScopedName::absolute(
            path.iter().map(|s| (*s).to_owned()).collect(),
        ))
        .ok()
        .and_then(AnyId::as_type)
        .unwrap_or_else(|| panic!("{path:?} is declared"))
}

fn member_type(composed: &Composed, struct_: &str, member: &str) -> Id<TypeNode> {
    let id = type_in(composed, &["T", struct_]);
    let member = composed
        .ast
        .members_of(id)
        .iter()
        .filter_map(|m| composed.ast.member(*m).ok())
        .find(|m| m.name == member)
        .expect("the member");
    member.type_.resolved().expect("a resolved member type")
}

fn bound_value(composed: &Composed, bound: &Bound) -> i128 {
    match bound {
        Bound::Const(expr) => composed.ast.eval_integral(*expr).expect("evaluable"),
        other => panic!("not a bound: {other:?}"),
    }
}

/// §7.3.2.1.3 and §7.3.2.4: the bound is a `<const_exp>` that may name a
/// constant; §7.3.2.4.1: so is every array dimension.
#[test]
fn bounds_are_const_expressions_over_constants() {
    let composed = load(
        "<const name=\"N\" type=\"int32\" value=\"2 + 3\"/>\
         <struct name=\"S\">\
         <member name=\"text\" type=\"string\" stringMaxLength=\"2 + N\"/>\
         <member name=\"q\" type=\"int32\" sequenceMaxLength=\"6 * 3\"/>\
         <member name=\"a\" type=\"int32\" arrayDimensions=\" 2, N + 3 \"/>\
         <member name=\"u\" type=\"int32\" sequenceMaxLength=\"-1\"/>\
         </struct>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let string = composed
        .ast
        .type_(member_type(&composed, "S", "text"))
        .unwrap();
    let TypeKind::String { bound, .. } = &string.kind else {
        panic!("not a string")
    };
    assert_eq!(bound_value(&composed, bound), 7);
    let seq = composed
        .ast
        .type_(member_type(&composed, "S", "q"))
        .unwrap();
    let TypeKind::Sequence { bound, .. } = &seq.kind else {
        panic!("not a sequence")
    };
    assert_eq!(bound_value(&composed, bound), 18);
    let array = composed
        .ast
        .type_(member_type(&composed, "S", "a"))
        .unwrap();
    let TypeKind::Array { dims, .. } = &array.kind else {
        panic!("not an array")
    };
    let values: Vec<i128> = dims
        .iter()
        .map(|d| composed.ast.eval_integral(*d).unwrap())
        .collect();
    assert_eq!(values, [2, 8]);
    let unbounded = composed
        .ast
        .type_(member_type(&composed, "S", "u"))
        .unwrap();
    let TypeKind::Sequence { bound, .. } = &unbounded.kind else {
        panic!("not a sequence")
    };
    assert!(matches!(bound, Bound::Unbounded));
}

/// A constant may be declared after the bound that names it, and the
/// constant's own value may name another constant.
#[test]
fn a_constant_is_found_whichever_side_of_its_use_it_is_declared() {
    let composed = load(
        "<struct name=\"S\"><member name=\"q\" type=\"int32\" sequenceMaxLength=\"B\"/></struct>\
         <const name=\"B\" type=\"int32\" value=\"A * 2\"/>\
         <const name=\"A\" type=\"int32\" value=\"4\"/>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let seq = composed
        .ast
        .type_(member_type(&composed, "S", "q"))
        .unwrap();
    let TypeKind::Sequence { bound, .. } = &seq.kind else {
        panic!("not a sequence")
    };
    assert_eq!(bound_value(&composed, bound), 8);
}

/// §7.3.2.1.3: constants appear "within a structure declaration", and a
/// name inside the structure starts its search there.
#[test]
fn a_constant_inside_a_structure_is_in_its_scope() {
    let composed = load(
        "<struct name=\"S\">\
         <member name=\"q\" type=\"int32\" sequenceMaxLength=\"C\"/>\
         <const name=\"C\" type=\"int32\" value=\"5\"/>\
         </struct>\
         <struct name=\"U\"><member name=\"q\" type=\"int32\" sequenceMaxLength=\"S::C + 1\"/></struct>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    for (struct_, expected) in [("S", 5), ("U", 6)] {
        let seq = composed
            .ast
            .type_(member_type(&composed, struct_, "q"))
            .unwrap();
        let TypeKind::Sequence { bound, .. } = &seq.kind else {
            panic!("not a sequence")
        };
        assert_eq!(bound_value(&composed, bound), expected, "{struct_}");
    }
}

/// An expression that names nothing is an error, not an unbounded
/// collection.
#[test]
fn a_bound_naming_no_constant_is_reported() {
    let composed = load(
        "<struct name=\"S\"><member name=\"q\" type=\"int32\" sequenceMaxLength=\"MISSING\"/></struct>",
    );
    assert!(composed.failed());
    let composed = load(
        "<struct name=\"S\"><member name=\"q\" type=\"int32\" sequenceMaxLength=\"2 +\"/></struct>",
    );
    assert!(composed.failed());
    assert!(
        composed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("<const_exp>")),
        "{:#?}",
        composed.diagnostics
    );
}

/// §7.3.2.4.4: sequence binds tightest, then array, then map — "a map
/// whose values are arrays of sequences".
#[test]
fn the_collection_attributes_nest_sequence_then_array_then_map() {
    let composed = load(
        "<struct name=\"S\"><member name=\"m\" type=\"int32\" sequenceMaxLength=\"4\" \
         arrayDimensions=\"2\" mapKeyType=\"string\" mapMaxLength=\"-1\"/></struct>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let map = composed
        .ast
        .type_(member_type(&composed, "S", "m"))
        .unwrap();
    let TypeKind::Map { value, bound, .. } = &map.kind else {
        panic!("outermost is not a map: {:?}", map.kind)
    };
    assert!(matches!(bound, Bound::Unbounded));
    let array = composed.ast.type_(value.resolved().unwrap()).unwrap();
    let TypeKind::Array { element, .. } = &array.kind else {
        panic!("the map value is not an array: {:?}", array.kind)
    };
    let seq = composed.ast.type_(element.resolved().unwrap()).unwrap();
    assert!(
        matches!(seq.kind, TypeKind::Sequence { .. }),
        "{:?}",
        seq.kind
    );
}

/// §7.3.2.4.3: `mapKeyNonBasicTypeName` is not allowed for a basic key,
/// `mapKeyStringMaxLength` only for a string key.
#[test]
fn map_key_attributes_are_checked_against_the_key_type() {
    let composed = load(
        "<struct name=\"S\"><member name=\"m\" type=\"int32\" mapKeyType=\"int32\" \
         mapKeyNonBasicTypeName=\"X\" mapMaxLength=\"-1\"/></struct>",
    );
    assert!(composed.failed());
    let composed = load(
        "<struct name=\"S\"><member name=\"m\" type=\"int32\" mapKeyType=\"int32\" \
         mapKeyStringMaxLength=\"4\" mapMaxLength=\"-1\"/></struct>",
    );
    assert!(composed.failed());
    let composed = load(
        "<struct name=\"S\"><member name=\"m\" type=\"int32\" mapKeyType=\"string\" \
         mapKeyStringMaxLength=\"4\" mapMaxLength=\"8\"/></struct>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let map = composed
        .ast
        .type_(member_type(&composed, "S", "m"))
        .unwrap();
    let TypeKind::Map { key, bound, .. } = &map.kind else {
        panic!("not a map")
    };
    assert_eq!(bound_value(&composed, bound), 8);
    let key = composed.ast.type_(key.resolved().unwrap()).unwrap();
    let TypeKind::String { bound, .. } = &key.kind else {
        panic!("the key is not a string")
    };
    assert_eq!(bound_value(&composed, bound), 4);
}

/// §7.3.2.4.3: "This attribute is required for all map types."
#[test]
fn a_map_without_map_max_length_is_reported() {
    let composed = load(
        "<struct name=\"S\"><member name=\"m\" type=\"int32\" mapKeyType=\"string\"/></struct>",
    );
    assert!(composed.failed());
}

/// §7.3.2.1.1: an `<include>` is "equivalent to an IDL #include": the
/// included document's declarations are in the model once, beside the
/// including one's, however many documents include it.
#[test]
fn an_include_brings_the_named_document_into_the_model_once() {
    let dir = std::env::temp_dir().join(format!("xc_include_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let common = dir.join("common.xml");
    std::fs::write(
        &common,
        "<dds><types><module name=\"C\">\
         <const name=\"N\" type=\"int32\" value=\"3\"/>\
         <struct name=\"Base\"><member name=\"a\" type=\"int32\"/></struct>\
         </module></types></dds>",
    )
    .unwrap();
    let main = dir.join("main.xml");
    std::fs::write(
        &main,
        "<dds><types><include file=\"common.xml\"/><module name=\"T\">\
         <include file=\"common.xml\"/>\
         <struct name=\"S\" baseType=\"C::Base\">\
         <member name=\"q\" type=\"int32\" sequenceMaxLength=\"C::N\"/></struct>\
         </module></types></dds>",
    )
    .unwrap();
    let composed = compose(main.to_str().unwrap()).expect("the document");
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let base = type_in(&composed, &["C", "Base"]);
    let TypeKind::Structure {
        base: Some(found), ..
    } = &composed
        .ast
        .type_(type_in(&composed, &["T", "S"]))
        .unwrap()
        .kind
    else {
        panic!("S has no base")
    };
    assert_eq!(found.resolved(), Some(base));
    let bases = composed
        .ast
        .types()
        .filter(|(_, t)| t.name == "Base")
        .count();
    assert_eq!(bases, 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A file that cannot be read, and one of another Type Representation,
/// are reported.
#[test]
fn an_include_that_cannot_be_followed_is_reported() {
    let composed = compose_source(
        "t.xml",
        "<dds><types><include file=\"missing.xml\"/><include file=\"x.idl\"/></types></dds>",
    );
    assert!(composed.failed());
    let messages: Vec<&str> = composed
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("cannot be read")),
        "{messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("another Type")),
        "{messages:?}"
    );
}

/// A bitset is lowered: fields by width (`bits`) or by the width of their
/// mask, padding without a name, a base bitset.
#[test]
fn a_bitset_is_lowered_with_its_fields() {
    let composed = load(
        "<bitset name=\"Base\"><bitfield name=\"a\" type=\"uint8\" mask=\"0x03\"/></bitset>\
         <bitset name=\"Bits\" baseType=\"Base\">\
         <bitfield name=\"b\" bits=\"5\"/><bitfield bits=\"3\"/>\
         </bitset>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let base = composed
        .ast
        .type_(type_in(&composed, &["T", "Base"]))
        .unwrap();
    let TypeKind::Bitset { fields, .. } = &base.kind else {
        panic!("not a bitset")
    };
    let field = composed.ast.bitfield(fields[0]).unwrap();
    assert_eq!((field.name.as_deref(), field.bits), (Some("a"), 2));
    assert!(field.holder_type.is_some());
    let bits = composed
        .ast
        .type_(type_in(&composed, &["T", "Bits"]))
        .unwrap();
    let TypeKind::Bitset {
        base: Some(parent),
        fields,
    } = &bits.kind
    else {
        panic!("not a derived bitset")
    };
    assert_eq!(parent.resolved(), Some(type_in(&composed, &["T", "Base"])));
    let widths: Vec<(Option<&str>, u16)> = fields
        .iter()
        .map(|f| {
            let f = composed.ast.bitfield(*f).unwrap();
            (f.name.as_deref(), f.bits)
        })
        .collect();
    assert_eq!(widths, [(Some("b"), 5), (None, 3)]);
}

/// A bitfield with neither `bits` nor `mask` has no width.
#[test]
fn a_bitfield_without_a_width_is_reported() {
    let composed = load("<bitset name=\"Bits\"><bitfield name=\"b\"/></bitset>");
    assert!(composed.failed());
}

/// §7.3.2.5.1.1: verbatim text on a structure, in either of the two
/// spellings (the clause's example and Annex A's `<text>` child).
#[test]
fn verbatim_text_reaches_the_type() {
    let composed = load(
        "<struct name=\"S\">\
         <verbatim language=\"Java\" placement=\"before-declaration\">/** doc */</verbatim>\
         <verbatim language=\"c\" placement=\"end-declaration\"><text>tail</text></verbatim>\
         <member name=\"a\" type=\"int32\"/></struct>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let node = composed.ast.type_(type_in(&composed, &["T", "S"])).unwrap();
    let verbatim = &node.facets.xtypes.as_ref().unwrap().verbatim;
    assert_eq!(verbatim.len(), 2);
    assert_eq!(verbatim[0].language, "Java");
    assert_eq!(verbatim[0].placement, VerbatimPlacement::BeforeDeclaration);
    assert_eq!(verbatim[0].text, "/** doc */");
    assert_eq!(verbatim[1].placement, VerbatimPlacement::EndDeclaration);
    assert_eq!(verbatim[1].text, "tail");
}

#[test]
fn a_verbatim_placement_that_is_none_of_the_six_is_reported() {
    let composed = load(
        "<struct name=\"S\"><verbatim placement=\"sideways\">x</verbatim>\
         <member name=\"a\" type=\"int32\"/></struct>",
    );
    assert!(composed.failed());
}

/// §7.3.2.9: an annotation type, applied to a structure, a member, a
/// union and its discriminator, an enumeration and an enumerator.
#[test]
fn an_annotation_is_declared_and_applied() {
    let composed = load(
        "<annotation name=\"MyAnnotation\"><member name=\"widgets\" type=\"int32\"/></annotation>\
         <struct name=\"S\">\
         <annotate name=\"MyAnnotation\"><member name=\"widgets\" value=\"5 + 1\"/></annotate>\
         <member name=\"a\" type=\"int32\">\
         <annotate name=\"MyAnnotation\"><member name=\"widgets\" value=\"7\"/></annotate></member>\
         </struct>\
         <union name=\"U\">\
         <annotate name=\"MyAnnotation\"><member name=\"widgets\" value=\"1\"/></annotate>\
         <discriminator type=\"int32\">\
         <annotate name=\"MyAnnotation\"><member name=\"widgets\" value=\"2\"/></annotate>\
         </discriminator>\
         <case><caseDiscriminator value=\"1\"/><member name=\"x\" type=\"int32\"/></case></union>\
         <enum name=\"E\">\
         <annotate name=\"MyAnnotation\"><member name=\"widgets\" value=\"3\"/></annotate>\
         <enumerator name=\"A\"><annotate name=\"MyAnnotation\">\
         <member name=\"widgets\" value=\"4\"/></annotate></enumerator>\
         <enumerator name=\"B\"/></enum>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let (def, _) = composed
        .ast
        .annotation_defs()
        .find(|(_, d)| d.name == "MyAnnotation")
        .expect("the annotation type");
    assert_eq!(composed.ast.annotation_def(def).unwrap().members.len(), 1);
    let check = |facets: &zerodds_ast::Facets, expected: i128, what: &str| {
        assert_eq!(facets.annotations.len(), 1, "{what}");
        let applied = &facets.annotations[0];
        assert_eq!(applied.definition.resolved(), Some(def), "{what}");
        assert_eq!(applied.arguments[0].name.as_deref(), Some("widgets"));
        assert_eq!(
            composed
                .ast
                .eval_integral(applied.arguments[0].value)
                .unwrap(),
            expected,
            "{what}"
        );
    };
    let s = composed.ast.type_(type_in(&composed, &["T", "S"])).unwrap();
    check(&s.facets, 6, "struct");
    let member = composed.ast.members_of(type_in(&composed, &["T", "S"]))[0];
    check(&composed.ast.member(member).unwrap().facets, 7, "member");
    let u = composed.ast.type_(type_in(&composed, &["T", "U"])).unwrap();
    check(&u.facets, 1, "union");
    let on_discriminator = &u.facets.xtypes.as_ref().unwrap().discriminator_annotations;
    assert_eq!(on_discriminator.len(), 1);
    assert_eq!(
        composed
            .ast
            .eval_integral(on_discriminator[0].arguments[0].value)
            .unwrap(),
        2
    );
    let e = composed.ast.type_(type_in(&composed, &["T", "E"])).unwrap();
    check(&e.facets, 3, "enumeration");
    let TypeKind::Enumeration { enumerators } = &e.kind else {
        panic!("not an enumeration")
    };
    check(
        &composed.ast.enumerator(enumerators[0]).unwrap().facets,
        4,
        "enumerator",
    );
    assert!(
        composed
            .ast
            .enumerator(enumerators[1])
            .unwrap()
            .facets
            .annotations
            .is_empty()
    );
}

/// An annotation no declaration is found for is reported by the resolve
/// pass, and a built-in annotation written as `<annotate>` is reported
/// with the attribute to use.
#[test]
fn an_annotate_of_an_unknown_or_a_built_in_annotation_is_reported() {
    let composed = load(
        "<struct name=\"S\"><annotate name=\"Nowhere\"/><member name=\"a\" type=\"int32\"/></struct>",
    );
    assert!(composed.failed());
    let composed = load(
        "<struct name=\"S\"><member name=\"a\" type=\"int32\"><annotate name=\"key\"/></member></struct>",
    );
    assert!(composed.failed());
    assert!(
        composed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("attribute")),
        "{:#?}",
        composed.diagnostics
    );
}

/// §7.3.2.7.1: an enumerator's value is a `<const_exp>`: the clause's own
/// example writes `value="0+1"`.
#[test]
fn an_enumerator_value_is_a_const_expression() {
    let composed = load(
        "<const name=\"K\" type=\"int32\" value=\"5\"/>\
         <enum name=\"E\"><enumerator name=\"A\" value=\"0\"/>\
         <enumerator name=\"B\" value=\"0+1\"/><enumerator name=\"C\" value=\"K * 2\"/>\
         <enumerator name=\"D\"/></enum>",
    );
    assert!(!composed.failed(), "{:#?}", composed.diagnostics);
    let e = composed.ast.type_(type_in(&composed, &["T", "E"])).unwrap();
    let TypeKind::Enumeration { enumerators } = &e.kind else {
        panic!("not an enumeration")
    };
    let values: Vec<i128> = enumerators
        .iter()
        .map(|id| composed.ast.effective_enumerator_value(*id).unwrap())
        .collect();
    assert_eq!(values, [0, 1, 10, 11]);
}

/// The expression tree keeps the name, not only the number.
#[test]
fn a_bound_keeps_the_constant_it_names() {
    let composed = load(
        "<const name=\"N\" type=\"int32\" value=\"4\"/>\
         <struct name=\"S\"><member name=\"q\" type=\"int32\" sequenceMaxLength=\"N\"/></struct>",
    );
    let seq = composed
        .ast
        .type_(member_type(&composed, "S", "q"))
        .unwrap();
    let TypeKind::Sequence {
        bound: Bound::Const(expr),
        ..
    } = &seq.kind
    else {
        panic!("not a bounded sequence")
    };
    let node = composed.ast.expr(*expr).unwrap();
    assert!(
        matches!(&node.expr, ConstExpr::NameRef(Ref::Resolved(_))),
        "{:?}",
        node.expr
    );
}
