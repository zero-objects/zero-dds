// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The model as a seesaw graph. Spec §3.1.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_ast::{Ast, MemberSpec, PrimitiveKind, TypeKind};
use zerodds_ast_core::Ref;
use zerodds_lang_cst::Language;
use zerodds_lang_rules::coverage::{ConstructKind, cell_count, covered_count};
use zerodds_lang_rules::{ModelGraph, coverage_of};

fn keyed_struct() -> Ast {
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
    let member = ast
        .add_member(pose, MemberSpec::new("id", Ref::Resolved(long)))
        .expect("id");

    let node = ast.member(member).expect("member").node_id;
    let any = zerodds_ast::AnyId::Member(member, node);
    ast.facets_mut(any).expect("facets").xtypes_mut().is_key = true;
    ast
}

#[test]
fn every_model_node_reaches_the_graph_and_maps_back() {
    let ast = keyed_struct();
    let graph = ModelGraph::of(&ast);

    for (id, node) in ast.types() {
        let ghost = graph
            .ghost(node.node_id)
            .unwrap_or_else(|| panic!("{} did not reach the graph", node.name));
        assert_eq!(
            graph.model_node(&ghost),
            Some(node.node_id),
            "{} did not map back",
            node.name
        );
        let _ = id;
    }
}

#[test]
fn a_facet_is_a_node_rather_than_an_attribute() {
    // A rule matching a keyed member is then a two-node pattern, the
    // same shape as a rule matching a member at all — which is what
    // makes the rules comparable across languages.
    let ast = keyed_struct();
    let graph = ModelGraph::of(&ast);

    let key_type = graph
        .graph
        .types
        .lookup("Key")
        .expect("no `Key` type in the graph");
    let keys = graph.graph.nodes_of_type(key_type).count();
    assert_eq!(keys, 1, "the `@key` facet did not become a node");
}

#[test]
fn a_flag_that_is_false_has_no_node() {
    // The rules are positive: "not optional" is never a pattern, so an
    // absent flag has nothing in the graph to match on.
    let ast = keyed_struct();
    let graph = ModelGraph::of(&ast);
    assert!(
        graph.graph.types.lookup("Optional").is_none(),
        "an absent flag produced a node"
    );
}

#[test]
fn building_the_graph_twice_from_one_model_gives_the_same_graph() {
    // The node identities are the model's own, which are stable across
    // snapshots and processes; a graph that differed between two runs
    // would make every rule application unreproducible.
    let ast = keyed_struct();
    let first = ModelGraph::of(&ast);
    let second = ModelGraph::of(&ast);

    assert_eq!(first.node_count(), second.node_count());
    for (_, node) in ast.types() {
        assert_eq!(first.ghost(node.node_id), second.ghost(node.node_id));
    }
}

#[test]
fn a_member_links_to_its_aggregate_and_to_its_type() {
    let ast = keyed_struct();
    let graph = ModelGraph::of(&ast);

    let pose = ast
        .types()
        .find(|(_, node)| node.name == "Pose")
        .map(|(_, node)| node.node_id)
        .expect("Pose");
    let long = ast
        .types()
        .find(|(_, node)| node.name == "long")
        .map(|(_, node)| node.node_id)
        .expect("long");
    let member = ast
        .members()
        .next()
        .map(|(_, node)| node.node_id)
        .expect("the member");

    let pose_ghost = graph.ghost(pose).expect("Pose in the graph");
    let long_ghost = graph.ghost(long).expect("long in the graph");
    let member_ghost = graph.ghost(member).expect("the member in the graph");

    assert!(
        graph.graph.connected(&pose_ghost, &member_ghost),
        "the aggregate does not own its member"
    );
    assert!(
        graph.graph.connected(&member_ghost, &long_ghost),
        "the member does not point at its type"
    );
}

// -- the coverage table, spec §4.1 ---------------------------------------

#[test]
fn the_coverage_table_has_a_cell_for_every_construct_and_language() {
    // Total by construction: `coverage_of` is exhaustive over both, so
    // reaching every cell is what proves the table has no hole.
    let mut cells = 0;
    for construct in ConstructKind::ALL {
        for language in Language::ALL {
            let coverage = coverage_of(construct, language);
            assert!(
                !coverage.reason().is_empty(),
                "{} in {} says nothing",
                construct.as_str(),
                language.as_str()
            );
            cells += 1;
        }
    }
    assert_eq!(cells, cell_count());
    assert_eq!(cells, 23 * zerodds_lang_cst::Language::ALL.len());
}

#[test]
fn the_table_reports_what_is_covered_and_what_is_not() {
    // The number is not the point; that it is stated rather than
    // implied is. A table listing only what is done reads as done.
    let covered = covered_count();
    assert!(covered > 0, "nothing is covered");
    assert!(
        covered < cell_count(),
        "the table claims completeness the rules do not have"
    );
}

#[test]
fn an_uncovered_cell_says_why_rather_than_only_that() {
    for construct in ConstructKind::ALL {
        for language in Language::ALL {
            let coverage = coverage_of(construct, language);
            if coverage.is_covered() {
                continue;
            }
            let reason = coverage.reason();
            assert!(
                reason.len() > 8,
                "{} in {} gives no reason: {reason:?}",
                construct.as_str(),
                language.as_str()
            );
        }
    }
}
