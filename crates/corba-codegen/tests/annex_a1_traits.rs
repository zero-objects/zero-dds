// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Annex A.1.1's tables against a model the real front end built.
//!
//! The unit tests in `src/traits.rs` construct a [`TypeTraits`] by hand
//! and prove the renderers. These start from IDL text, run it through
//! `zerodds-idl-parse` and `zerodds-idl-lower`, and check that the
//! member set a type gets is the one its table names — which is the half
//! a hand-built value cannot show.
//!
//! [`TypeTraits`]: zerodds_corba_codegen::TypeTraits

// Tests assert on Results directly; a failed expect IS the test failure.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;

use zerodds_ast::Ast;
use zerodds_ast_core::Id;
use zerodds_corba_codegen::{TargetLanguage, TraitDefinition, render_type_traits, traits_of};

const IDL: &str = r#"
module probe {
    struct Point {
        long x;
        long y;
    };

    struct WithString {
        string s;
    };

    typedef string<32> Bounded;
    typedef string Unbounded;
    typedef sequence<long, 8> BoundedLongs;
    typedef sequence<Point> Points;
    typedef long Grid[3][4];
    typedef map<string, long> Lookup;

    enum Colour { RED, GREEN };

    @bit_bound(9)
    bitmask Flags { FIRST, SECOND };

    valuetype Value {
        public long x;
    };

    valuetype Truncated : truncatable Value {
        public long y;
    };

    abstract valuetype AbstractValue {
    };

    valuetype BoxedLong long;

    local interface LocalEcho {
        void ping();
    };

    abstract interface AbstractEcho {
    };

    interface Echo {
        void ping();
    };

    struct Holder {
        Echo target;
        LocalEcho local_target;
        AbstractEcho abstract_target;
    };
};
"#;

fn model() -> Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "the traits IDL must parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "annex-a1-traits.idl");
    assert!(
        lowered.diagnostics.iter().all(|d| !d.is_error()),
        "the traits IDL must lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.broken.is_empty(),
        "every name must resolve: {:?}",
        report.broken
    );
    ast
}

fn types_by_name(ast: &Ast) -> BTreeMap<String, Id<zerodds_ast::TypeNode>> {
    ast.types()
        .filter(|(_, node)| !node.name.is_empty())
        .filter_map(|(id, node)| {
            let scoped = ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))?;
            Some((scoped.segments().join("::"), id))
        })
        .collect()
}

fn named(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    *types_by_name(ast)
        .get(name)
        .unwrap_or_else(|| panic!("no type `{name}`"))
}

/// The target of a `typedef`, which is where Tables A.3 to A.11 live.
fn aliased(ast: &Ast, alias: &str) -> Id<zerodds_ast::TypeNode> {
    let id = named(ast, alias);
    let node = ast.type_(id).expect("the alias");
    match &node.kind {
        zerodds_ast::TypeKind::Alias { aliased } => aliased.resolved().expect("a resolved alias"),
        other => panic!("`{alias}` is not a typedef: {other:?}"),
    }
}

fn member_names(ast: &Ast, id: Id<zerodds_ast::TypeNode>) -> Vec<&'static str> {
    traits_of(ast, id).expect("a traited type").member_names()
}

#[test]
fn table_a1_is_the_whole_set_for_a_structure_a_union_and_an_enumeration() {
    let ast = model();
    let four = vec!["value_type", "in_type", "out_type", "inout_type"];
    for name in ["probe::Point", "probe::WithString", "probe::Colour"] {
        assert_eq!(member_names(&ast, named(&ast, name)), four, "{name}");
    }
}

#[test]
fn a_typedef_has_no_traits_of_its_own() {
    let ast = model();
    // C++ has typedefs, so a specialisation on the alias would be a
    // redefinition of the aliased type's.
    for alias in ["probe::Bounded", "probe::Points", "probe::Lookup"] {
        assert!(
            traits_of(&ast, named(&ast, alias)).is_none(),
            "{alias} should answer None"
        );
    }
}

#[test]
fn tables_a3_and_a4_split_a_string_on_whether_it_is_bounded() {
    let ast = model();
    assert_eq!(
        member_names(&ast, aliased(&ast, "probe::Bounded")),
        vec![
            "value_type",
            "in_type",
            "out_type",
            "inout_type",
            "element_traits",
            "is_bounded",
            "bound",
        ]
    );
    assert_eq!(
        member_names(&ast, aliased(&ast, "probe::Unbounded")),
        vec![
            "value_type",
            "in_type",
            "out_type",
            "inout_type",
            "element_traits",
            "is_bounded",
        ]
    );
    let bounded = traits_of(&ast, aliased(&ast, "probe::Bounded")).expect("bounded");
    assert_eq!(
        bounded.member("bound").map(|m| m.definition.clone()),
        Some(TraitDefinition::Constant(32))
    );
    assert_eq!(
        bounded
            .member("element_traits")
            .map(|m| m.definition.clone()),
        Some(TraitDefinition::TraitsOf("char".to_string()))
    );
}

#[test]
fn tables_a5_and_a6_give_a_sequence_its_element_and_its_bound() {
    let ast = model();
    let bounded = traits_of(&ast, aliased(&ast, "probe::BoundedLongs")).expect("bounded");
    assert_eq!(
        bounded.member("bound").map(|m| m.definition.clone()),
        Some(TraitDefinition::Constant(8))
    );
    assert_eq!(
        bounded.member("is_bounded").map(|m| m.definition.clone()),
        Some(TraitDefinition::Flag(true))
    );
    let unbounded = traits_of(&ast, aliased(&ast, "probe::Points")).expect("points");
    assert_eq!(
        unbounded.member("is_bounded").map(|m| m.definition.clone()),
        Some(TraitDefinition::Flag(false))
    );
    assert!(unbounded.member("bound").is_none());
    assert_eq!(
        unbounded
            .member("element_traits")
            .map(|m| m.definition.clone()),
        Some(TraitDefinition::TraitsOf("probe::Point".to_string()))
    );
}

#[test]
fn table_a7_counts_an_arrays_dimensions_and_takes_its_core_element() {
    let ast = model();
    let grid = traits_of(&ast, aliased(&ast, "probe::Grid")).expect("grid");
    assert_eq!(
        grid.member("dimensions").map(|m| m.definition.clone()),
        Some(TraitDefinition::Constant(2)),
        "`long Grid[3][4]` has two dimensions"
    );
    // Table A.7: the core type's trait, not the row type's.
    assert_eq!(
        grid.member("element_traits").map(|m| m.definition.clone()),
        Some(TraitDefinition::TraitsOf("long".to_string()))
    );
}

#[test]
fn tables_a10_and_a11_give_a_map_a_key_and_a_value_trait() {
    let ast = model();
    let lookup = traits_of(&ast, aliased(&ast, "probe::Lookup")).expect("lookup");
    assert_eq!(
        lookup.member_names(),
        vec![
            "value_type",
            "in_type",
            "out_type",
            "inout_type",
            "key_traits",
            "value_traits",
            "is_bounded",
        ]
    );
}

#[test]
fn table_a8_reads_abstract_and_truncatable_off_the_declaration() {
    let ast = model();
    let plain = traits_of(&ast, named(&ast, "probe::Value")).expect("Value");
    assert_eq!(
        plain.member("is_abstract").map(|m| m.definition.clone()),
        Some(TraitDefinition::Flag(false))
    );
    assert_eq!(
        plain.member("is_truncatable").map(|m| m.definition.clone()),
        Some(TraitDefinition::Flag(false))
    );
    let abstract_ = traits_of(&ast, named(&ast, "probe::AbstractValue")).expect("AbstractValue");
    assert_eq!(
        abstract_
            .member("is_abstract")
            .map(|m| m.definition.clone()),
        Some(TraitDefinition::Flag(true))
    );
    assert!(plain.member("factory_type").is_some());
    assert!(plain.member("obv_type").is_some());
}

#[test]
fn table_a9_gives_a_valuebox_its_boxed_trait() {
    let ast = model();
    let boxed = traits_of(&ast, named(&ast, "probe::BoxedLong")).expect("BoxedLong");
    assert_eq!(
        boxed.member("boxed_traits").map(|m| m.definition.clone()),
        Some(TraitDefinition::TraitsOf("long".to_string()))
    );
}

#[test]
fn table_a12_takes_the_models_effective_bit_bound() {
    let ast = model();
    let flags = traits_of(&ast, named(&ast, "probe::Flags")).expect("Flags");
    assert_eq!(
        flags.member("bit_bound").map(|m| m.definition.clone()),
        Some(TraitDefinition::Constant(9)),
        "the `@bit_bound(9)` annotation, not the default 32"
    );
    let text = render_type_traits(&flags, TargetLanguage::Cpp).expect("cpp");
    assert!(
        text.contains("using underlying_type = uint16_t;"),
        "nine bits need sixteen:\n{text}"
    );
}

#[test]
fn table_a2_reads_local_and_abstract_off_the_interface_declaration() {
    let ast = model();
    let holder = ast.type_(named(&ast, "probe::Holder")).expect("Holder");
    let zerodds_ast::TypeKind::Structure { members, .. } = &holder.kind else {
        panic!("Holder is not a structure");
    };
    let mut seen: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for member in members {
        let node = ast.member(*member).expect("a member");
        let target = node.type_.resolved().expect("a resolved member type");
        let traits = traits_of(&ast, target).expect("an object reference has a trait");
        let flag = |name: &str| {
            matches!(
                traits.member(name).map(|m| m.definition.clone()),
                Some(TraitDefinition::Flag(true))
            )
        };
        assert!(traits.member("ref_type").is_some(), "{}", node.name);
        assert!(traits.member("weak_ref_type").is_some(), "{}", node.name);
        seen.insert(node.name.clone(), (flag("is_local"), flag("is_abstract")));
    }
    assert_eq!(seen.get("target"), Some(&(false, false)));
    assert_eq!(seen.get("local_target"), Some(&(true, false)));
    assert_eq!(seen.get("abstract_target"), Some(&(false, true)));
}

#[test]
fn a_structure_has_no_is_local_because_table_a2_is_the_interface_table() {
    let ast = model();
    for name in ["probe::Point", "probe::Colour", "probe::Value"] {
        let traits = traits_of(&ast, named(&ast, name)).expect(name);
        assert!(
            traits.member("is_local").is_none(),
            "{name} must not declare is_local"
        );
    }
}

#[test]
fn every_traited_type_renders_in_every_column() {
    let ast = model();
    let mut rendered = 0_usize;
    for (_, id) in types_by_name(&ast) {
        let Some(traits) = traits_of(&ast, id) else {
            continue;
        };
        for language in TargetLanguage::ALL {
            let text = render_type_traits(&traits, language).expect("renderable");
            assert!(!text.is_empty(), "{language:?} rendered nothing");
        }
        rendered += 1;
    }
    assert!(rendered >= 10, "only {rendered} types were traited");
}

#[test]
fn the_cpp_specialisation_declares_ref_type_before_the_alias_that_names_it() {
    let ast = model();
    let holder = ast.type_(named(&ast, "probe::Holder")).expect("Holder");
    let zerodds_ast::TypeKind::Structure { members, .. } = &holder.kind else {
        panic!("Holder is not a structure");
    };
    let first = ast.member(members[0]).expect("a member");
    let target = first.type_.resolved().expect("a resolved member type");
    let traits = traits_of(&ast, target).expect("a trait");
    let text = render_type_traits(&traits, TargetLanguage::Cpp).expect("cpp");
    let ref_at = text.find("using ref_type").expect("a ref_type alias");
    let in_at = text.find("using in_type").expect("an in_type alias");
    assert!(
        ref_at < in_at,
        "a member alias must be declared before the alias that names it:\n{text}"
    );
}
