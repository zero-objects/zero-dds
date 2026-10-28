// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `@verbatim` on a type in the Complete TypeObject and back, DDS-XTypes 1.3
//! §7.3.4.3 ("converted to the Complete TypeObject representation and back
//! to IDL with no information loss, other than formatting"), Annex B's
//! `AppliedBuiltinTypeAnnotations` and §7.2.2.4.8.2 (the placements).

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, VerbatimDirective, VerbatimPlacement};
use zerodds_ast_core::Id;
use zerodds_types::from_ast::{
    Form, complete_type_object, minimal_type_object, register_all, type_identifier,
};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_object::TypeObject;
use zerodds_types::type_object::common::{CompleteTypeDetail, VerbatimPlacement as Wire};
use zerodds_types::type_object::complete::CompleteTypeObject;

const IDL: &str = r#"
@verbatim(language="c++", placement=BEFORE_DECLARATION, text="// before S")
struct S {
    long a;
};

@verbatim(language="java", placement=END_FILE, text="// end of the file")
union U switch (long) {
    case 1: long one;
    default: octet raw;
};

@verbatim(language="*", placement=AFTER_DECLARATION, text="// after E")
enum E { E_FIRST, E_SECOND };

@verbatim(language="c", placement=BEGIN_DECLARATION, text="/* begin B */")
bitmask B { F0, F1 };

@verbatim(language="c++", placement=END_DECLARATION, text="/* end T */")
typedef long T;

struct Plain {
    long a;
};
"#;

/// Two `@verbatim` on one type: Annex B has room for one.
const TWICE: &str = r#"
@verbatim(language="c++", placement=BEFORE_DECLARATION, text="one")
@verbatim(language="c++", placement=AFTER_DECLARATION, text="two")
struct Twice {
    long a;
};
"#;

fn model() -> Ast {
    model_of(IDL)
}

fn model_of(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "verbatim.idl");
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

fn detail(object: &CompleteTypeObject) -> &CompleteTypeDetail {
    match object {
        CompleteTypeObject::Struct(t) => &t.header.detail,
        CompleteTypeObject::Union(t) => &t.header.detail,
        CompleteTypeObject::Enumerated(t) => &t.header.detail,
        CompleteTypeObject::Bitmask(t) => &t.detail,
        CompleteTypeObject::Alias(t) => &t.header.detail,
        other => panic!("no type detail on {other:?}"),
    }
}

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
fn a_types_verbatim_is_in_its_complete_object_after_the_wire() {
    let ast = model();
    let cases = [
        ("S", Wire::Before, "c++", "// before S"),
        ("U", Wire::EndFile, "java", "// end of the file"),
        ("E", Wire::After, "*", "// after E"),
        ("B", Wire::BeginDeclaration, "c", "/* begin B */"),
        ("T", Wire::EndDeclaration, "c++", "/* end T */"),
    ];
    for (name, placement, language, text) in cases {
        let object = complete_type_object(&ast, type_id(&ast, name)).expect("the object builds");
        for object in [object.clone(), through_the_wire(&object)] {
            let verbatim = detail(&object)
                .ann_builtin
                .verbatim
                .as_ref()
                .unwrap_or_else(|| panic!("`{name}` lost its @verbatim"));
            assert_eq!(verbatim.placement, placement, "{name}");
            assert_eq!(verbatim.language, language, "{name}");
            assert_eq!(verbatim.text, text, "{name}");
        }
    }
}

#[test]
fn a_type_without_verbatim_has_none() {
    let ast = model();
    let object = complete_type_object(&ast, type_id(&ast, "Plain")).expect("the object builds");
    assert!(detail(&object).ann_builtin.verbatim.is_none());
}

#[test]
fn the_verbatim_comes_back_into_the_rebuilt_model() {
    let ast = model();
    let mut registry = TypeRegistry::new();
    register_all(&ast, &mut registry).expect("the registry fills");
    for (name, expected) in [
        (
            "S",
            VerbatimDirective {
                language: "c++".into(),
                placement: VerbatimPlacement::BeforeDeclaration,
                text: "// before S".into(),
            },
        ),
        (
            "U",
            VerbatimDirective {
                language: "java".into(),
                placement: VerbatimPlacement::EndFile,
                text: "// end of the file".into(),
            },
        ),
        (
            "E",
            VerbatimDirective {
                language: "*".into(),
                placement: VerbatimPlacement::AfterDeclaration,
                text: "// after E".into(),
            },
        ),
    ] {
        let object = complete_type_object(&ast, type_id(&ast, name)).expect("the object builds");
        let decoded = through_the_wire(&object);
        let rebuilt = zerodds_types::dynamic::dynamic_type(&decoded, &registry).expect("rebuilds");
        let rebuilt_ast = rebuilt.ast().clone();
        let node = rebuilt_ast
            .type_(type_id(&rebuilt_ast, &rebuilt.qualified_name()))
            .expect("the rebuilt type");
        let directives = &node
            .facets
            .xtypes
            .as_ref()
            .unwrap_or_else(|| panic!("`{name}`: no XTypes facet came back"))
            .verbatim;
        assert_eq!(directives, &[expected], "{name}");
    }
}

#[test]
fn more_than_one_verbatim_on_a_type_is_refused_by_name() {
    let ast = model_of(TWICE);
    let error = complete_type_object(&ast, type_id(&ast, "Twice"))
        .expect_err("Annex B holds one @verbatim per type");
    let text = error.to_string();
    assert!(text.contains("Twice"), "{text}");
    assert!(text.contains("@verbatim"), "{text}");
}

#[test]
fn the_minimal_form_and_its_identifier_do_not_see_the_verbatim() {
    // §7.3.4.4: the Minimal TypeObject leaves out what does not impact
    // assignability, user-defined annotations among it. The same struct with
    // and without a `@verbatim` has one MINIMAL identifier.
    let with = {
        let ast = model();
        let id = type_id(&ast, "S");
        (
            minimal_type_object(&ast, id).expect("minimal"),
            type_identifier(&ast, id, Form::Minimal).expect("identifier"),
        )
    };
    let without_idl = "struct S { long a; };";
    let parsed = zerodds_idl_parse::parse(without_idl);
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "plain.idl").ast;
    ast.resolve_all_by_name().expect("resolution");
    let id = type_id(&ast, "S");
    let without = (
        minimal_type_object(&ast, id).expect("minimal"),
        type_identifier(&ast, id, Form::Minimal).expect("identifier"),
    );
    assert_eq!(with.0, without.0);
    assert_eq!(with.1, without.1);
}
