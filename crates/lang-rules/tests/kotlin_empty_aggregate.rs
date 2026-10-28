// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Empty IDL aggregates stay empty, but cannot be Kotlin data classes.
//!
//! These checks inspect the typed CST and independently stated fixture
//! shapes. They do not replace compilation by kotlinc, nor constitute a
//! SAT/Lean proof. The negative control mutates a valid empty holder into
//! the exact invalid data-class state that the real compiler rejected.
#![allow(clippy::expect_used)]

use zerodds_lang_cst::kotlin::{Class, Decl, File};
use zerodds_lang_rules::adapter::kotlin::kotlin_file;
use zerodds_lang_rules::{ModelGraph, apply, rule};

fn declarations(source: &str) -> File {
    let composed = zerodds_idl_compose::compose_source(
        "empty-aggregate.idl",
        source,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed(), "{:?}", composed.diagnostics);
    let model = ModelGraph::of(&composed.ast);
    let applied = apply(&model, &rule::kotlin::catalogue()).expect("Kotlin rules apply");
    kotlin_file(&composed.ast, &applied).expect("every admitted field materializes")
}

/// Kotlin's primary-constructor restriction, checked on the CST rather
/// than inferred from the presence or absence of words in rendered text.
fn data_classes_have_a_constructor_property(file: &File) -> bool {
    file.decls.iter().all(|decl| match decl {
        Decl::Class(class) => !class.is_data || !class.properties.is_empty(),
        _ => true,
    })
}

fn class<'a>(file: &'a File, name: &str) -> &'a Class {
    file.decls
        .iter()
        .find_map(|decl| match decl {
            Decl::Class(class) if class.name.as_str() == name => Some(class),
            _ => None,
        })
        .expect("the declared aggregate remains present")
}

fn assert_empty_class(file: &File, name: &str) {
    let empty = class(file, name);
    assert!(!empty.is_data, "an empty holder cannot be a data class");
    assert!(empty.properties.is_empty(), "no invented marker property");
    assert!(data_classes_have_a_constructor_property(file));
    let rendered = zerodds_lang_render::kotlin::file(file);
    assert!(
        rendered.contains(&format!("class {name}(\n)")),
        "the empty holder remains constructible: {rendered}"
    );
}

#[test]
fn the_empty_struct_corpus_case_keeps_a_zero_property_class() {
    let file = declarations(include_str!(
        "../../../tests/interop-corpus/empty-struct/type.idl"
    ));
    assert_empty_class(&file, "m_S");
}

#[test]
fn an_empty_base_does_not_remove_the_derived_members() {
    let file = declarations(include_str!(
        "../../../tests/interop-corpus/empty-struct-as-base/type.idl"
    ));
    assert_empty_class(&file, "m_Base");
    let derived = class(&file, "m_S");
    assert!(derived.is_data);
    let names: Vec<_> = derived.properties.iter().map(|p| p.name.as_str()).collect();
    // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    assert_eq!(names, ["parent", "a", "b"]);
}

#[test]
fn empty_collection_elements_do_not_remove_the_containing_fields() {
    let file = declarations(include_str!(
        "../../../tests/interop-corpus/empty-struct-in-sequence/type.idl"
    ));
    assert_empty_class(&file, "m_E");
    let holder = class(&file, "m_S");
    assert!(holder.is_data);
    let names: Vec<_> = holder.properties.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["items", "pair", "tail"]);
}

#[test]
fn the_base_counts_as_a_property_even_without_locally_declared_members() {
    let file = declarations("module m { struct Base { long value; }; struct Child : Base {}; };");
    let derived = class(&file, "m_Child");
    assert!(derived.is_data);
    // Its one property is the base, the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    assert_eq!(derived.properties.len(), 1);
    assert_eq!(derived.properties[0].name.as_str(), "parent");
    assert!(data_classes_have_a_constructor_property(&file));
}

#[test]
fn an_empty_exception_uses_the_same_no_dummy_property_rule() {
    let file = declarations("exception Empty {};");
    assert_empty_class(&file, "Empty");
}

#[test]
fn the_invariant_checker_rejects_the_original_invalid_cst() {
    let mut file = declarations("struct Empty {};");
    assert_empty_class(&file, "Empty");
    let empty = file
        .decls
        .iter_mut()
        .find_map(|decl| match decl {
            Decl::Class(class) if class.name.as_str() == "Empty" => Some(class),
            _ => None,
        })
        .expect("the empty class exists before mutation");
    empty.is_data = true;
    assert!(empty.properties.is_empty());
    assert!(!data_classes_have_a_constructor_property(&file));
}
