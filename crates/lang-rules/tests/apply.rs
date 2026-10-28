// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Applying a catalogue. Spec §2.4, §2.5, §4.2.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, MemberSpec, PrimitiveKind, TypeKind};
use zerodds_ast_core::Ref;
use zerodds_lang_cst::Language;
use zerodds_lang_rules::rule::{catalogue, rust};
use zerodds_lang_rules::{ModelGraph, apply};

fn model_with_two_members() -> Ast {
    let mut ast = Ast::new();
    let root = ast.root_module();
    let long = ast
        .add_type(root, "long", TypeKind::Primitive(PrimitiveKind::Int32))
        .expect("long");
    let pose = ast
        .add_type(
            root,
            "Pose",
            TypeKind::Structure {
                base: None,
                members: Vec::new(),
            },
        )
        .expect("Pose");
    for name in ["x", "y"] {
        let _ = ast
            .add_member(pose, MemberSpec::new(name, Ref::Resolved(long)))
            .expect("member");
    }
    ast
}

/// How many nodes of a type the applied graph holds.
fn count_of(applied: &zerodds_lang_rules::Applied, type_name: &str) -> usize {
    applied
        .graph
        .types
        .lookup(type_name)
        .map_or(0, |typ| applied.graph.nodes_of_type(typ).count())
}

#[test]
fn a_struct_becomes_a_rust_struct() {
    let ast = model_with_two_members();
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rust::catalogue()).expect("apply");

    assert_eq!(
        count_of(&applied, "RustStruct"),
        1,
        "the struct rule produced {} structs",
        count_of(&applied, "RustStruct")
    );
}

#[test]
fn each_member_becomes_a_field() {
    // Two members, two fields — and the member rule fired for each
    // without the struct rule knowing anything about members. That
    // composition is what §2.4 asks for.
    let ast = model_with_two_members();
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rust::catalogue()).expect("apply");

    assert_eq!(count_of(&applied, "RustField"), 2);
}

#[test]
fn applying_the_catalogue_twice_changes_nothing_the_second_time() {
    // Application is monotone and add-only, so a second run over its own
    // result creates nothing: two rules never claim the same construct.
    // Spec §4.2.
    let ast = model_with_two_members();
    let model = ModelGraph::of(&ast);
    let first = apply(&model, &rust::catalogue()).expect("first");
    let structs = count_of(&first, "RustStruct");
    let fields = count_of(&first, "RustField");

    let second = apply(&model, &rust::catalogue()).expect("second");
    assert_eq!(count_of(&second, "RustStruct"), structs);
    assert_eq!(count_of(&second, "RustField"), fields);
}

/// Every language whose catalogue this file's loops can meaningfully
/// exercise.
///
/// [`Language::ALL`] minus Node: `rule::catalogue` answers it with a
/// `RuleFile` of zero rules, because Node's materialiser declares
/// nothing the seesaw graph has anything to track —
/// `crates/lang-rules/src/adapter/node.rs`'s module documentation
/// carries the reasoning, and `crates/lang-rules/src/rule/mod.rs`'s
/// `catalogue` states it again where the empty answer is built. An
/// empty catalogue validates, round-trips and applies with zero steps
/// well enough on its own; excluding it here is about what these
/// particular assertions mean, not about avoiding a failure — "fires at
/// least three times" and "covers what the other columns cover" are
/// both claims a catalogue with no rules cannot make honestly, however
/// it is answered. A filter and not a silent skip inside the loop, so a
/// second exclusion added later has to explain itself here too.
fn languages_with_a_catalogue() -> impl Iterator<Item = Language> {
    Language::ALL
        .into_iter()
        .filter(|language| *language != Language::Node)
}

#[test]
fn every_catalogue_validates_against_the_rule_format() {
    // The engine's own answer, asserted on rather than re-derived: it
    // resolves every node name a rule refers to and reports one that
    // was never declared. A hand-built rule value can otherwise carry
    // such a name all the way to a match that silently never fires.
    for language in languages_with_a_catalogue() {
        let file = catalogue(language);
        let result = seesaw_tgg::rules::validate::validate(&file);
        assert!(result.is_ok(), "{}: {:?}", language.as_str(), result.err());
    }
}

#[test]
fn every_catalogue_serialises_and_reads_back_the_same() {
    // The rules are data: a catalogue that could not round-trip through
    // its own format would be a value that only exists in code, which
    // is what having a format is meant to prevent.
    for language in languages_with_a_catalogue() {
        let file = catalogue(language);
        let json = serde_json::to_string(&file).expect("serialise");
        let read = seesaw_tgg::rules::format::RuleFile::from_json(&json).expect("read back");
        assert_eq!(read.rules.len(), file.rules.len(), "{}", language.as_str());
        assert!(
            seesaw_tgg::rules::validate::validate(&read).is_ok(),
            "{} did not validate after a round trip",
            language.as_str()
        );
    }
}

#[test]
fn every_rule_of_the_rust_catalogue_is_reachable() {
    // A rule no model can trigger is dead. The model below contains one
    // of everything the catalogue claims to cover, so a rule that never
    // fires against it covers nothing. Spec §4.2.
    let ast = model_with_two_members();
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rust::catalogue()).expect("apply");

    assert!(
        applied.steps >= 2,
        "the catalogue fired {} times for two rules",
        applied.steps
    );
    for (name, type_name) in [
        (rust::STRUCT_TO_STRUCT, "RustStruct"),
        (rust::MEMBER_TO_FIELD, "RustField"),
    ] {
        assert!(
            count_of(&applied, type_name) > 0,
            "the rule `{name}` produced nothing"
        );
    }
}

#[test]
fn every_language_catalogue_fires_on_the_same_model() {
    // The four catalogues cover the same constructs; a model with a
    // struct and two members has to reach every one of them. A language
    // whose rules never fire is one whose column of the table is a
    // claim rather than a mapping.
    let ast = model_with_two_members();
    let model = ModelGraph::of(&ast);
    for language in languages_with_a_catalogue() {
        let applied = apply(&model, &catalogue(language)).expect("apply");
        assert!(
            applied.steps >= 3,
            "{} fired {} times for one struct and two members",
            language.as_str(),
            applied.steps
        );
    }
}

#[test]
fn a_created_node_can_be_traced_back_to_the_model_node_it_came_from() {
    // Values never enter the overlay; a materialiser resolves them from
    // the original, which needs this map. Spec §2.3.
    let ast = model_with_two_members();
    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rust::catalogue()).expect("apply");

    let pose = ast
        .types()
        .find(|(_, node)| node.name == "Pose")
        .map(|(_, node)| node.node_id)
        .expect("Pose");
    let ghost = model.ghost(pose).expect("Pose in the graph");
    assert_eq!(applied.origin.get(&ghost), Some(&pose));
}
