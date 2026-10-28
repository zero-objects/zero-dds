// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What every walk over the type graph does when the graph has a cycle.
//!
//! Two cycles, and the probe has to answer both by name rather than by
//! running out of stack. A cell that aborted measured nothing, and an
//! empty cell in `vendor_bytes.md` reads as a vendor difference.
//!
//! ## The aggregate cycle
//!
//! `recursive-mutual` in the corpus: `A → sequence<B> → B → sequence<A>`.
//! `dump.rs`, `dump_cpp.rs` and `corba_cpp.rs` emit one function per
//! aggregate on the cycle and call it where the expansion would have
//! unrolled — `crate::recursion::recursive_aggregates` names them. Two
//! walks ran past that answer and followed the cycle instead:
//! `corba_java::dump_at` and `dump_ros::emit`, both measured
//! `SIGABRT`/"has overflowed its stack" on 2026-08-31. Neither has a
//! shape to emit a function into, and neither column takes a recursive
//! type at all — `corba_idl` and `ros_idl` refuse it — so both refuse
//! it here too, in the same words.
//!
//! ## The typedef cycle
//!
//! `typedef Tee Tee;`, which IDL 4.2 §7.4.1.4.4.4 has no declaration
//! for and the frontend admits anyway. Every walk in both crates follows
//! a typedef by recursing on its target with nothing else changing, so
//! the chain has no end: thirteen entry points overflowed the stack,
//! including `recursion::recursive_aggregates`, which is the function
//! the other walks ask before they can refuse anything.
//! `contract::reject_alias_cycle` is the gate each entry point now
//! passes first.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::{Value, json};
use zerodds_ast::Ast;
use zerodds_ast_core::{Id, ScopedName};
use zerodds_wire_probe::encode::{ProbeError, model_from_idl};
use zerodds_wire_probe::{Vendor, corba_cpp, corba_java, dump, dump_cpp, dump_ros};
use zerodds_wire_probe::{canonical, fill, fill_cpp, fill_ros, fill_rust, recursion, ros_idl};

/// The corpus's `recursive-mutual`, verbatim.
const MUTUAL: &str = "module m {\n  struct B;\n  @appendable struct A { long value; sequence<B> bs; };\n  @appendable struct B { long weight; sequence<A> as; };\n};";

/// A typedef whose target is itself. Not IDL, and accepted anyway.
const ALIAS_CYCLE: &str = "typedef Tee Tee;\n@final struct Ess { Tee ex; };";

fn model(source: &str) -> Ast {
    model_from_idl(source, "recursion.idl").expect("the fixture parses")
}

fn type_of(ast: &Ast, name: &str) -> Id<zerodds_ast::TypeNode> {
    ast.find_by_scoped_name(&ScopedName::absolute(
        name.split("::")
            .filter(|segment| !segment.is_empty())
            .map(std::string::ToString::to_string)
            .collect(),
    ))
    .ok()
    .and_then(zerodds_ast::AnyId::as_type)
    .expect("the type is in the model")
}

fn mutual_sample() -> Value {
    json!({ "value": 1, "bs": [{ "weight": 2, "as": [] }] })
}

/// The message a refusal carries, whichever variant it is.
fn refusal(error: &ProbeError) -> String {
    format!("{error}")
}

#[test]
fn the_ros_dump_refuses_an_aggregate_on_a_cycle_by_name() {
    let ast = model(MUTUAL);
    let root = type_of(&ast, "::m::A");
    let error = dump_ros::dump_ros(&ast, root).expect_err("a cycle has no finite expansion");
    let text = refusal(&error);
    assert!(
        text.contains("a recursive type"),
        "the refusal has to name the construct: {text}"
    );
    assert!(
        text.contains("§7.4.1.1"),
        "and the clause the ROS column refuses it under: {text}"
    );
}

#[test]
fn the_corba_java_dump_refuses_an_aggregate_on_a_cycle_by_name() {
    let ast = model(MUTUAL);
    let root = type_of(&ast, "::m::A");
    let error =
        corba_java::dump_corba_java(&ast, root).expect_err("a cycle has no finite expansion");
    let text = refusal(&error);
    assert!(
        text.contains("a recursive type"),
        "the refusal has to name the construct: {text}"
    );
    assert!(
        text.contains("CORBA CDR"),
        "and say whose limit it is: {text}"
    );
}

#[test]
fn the_three_emitters_that_carry_a_function_per_cycle_still_emit_one() {
    let ast = model(MUTUAL);
    let root = type_of(&ast, "::m::A");
    // Not a refusal: these three call `zd_rec_dump_<n>` where the
    // expansion would have unrolled, which is the answer
    // `recursion::recursive_aggregates` exists for.
    for (column, emitted) in [
        ("dump", dump::dump(&ast, root, Vendor::Cyclone)),
        ("dump_cpp", dump_cpp::dump_fastdds(&ast, root)),
        ("corba_cpp", corba_cpp::dump_corba_cpp(&ast, root)),
    ] {
        let source = emitted.unwrap_or_else(|error| panic!("{column}: {error}"));
        assert!(
            source.contains("zd_rec"),
            "{column} has to call the per-cycle function rather than unroll"
        );
    }
    assert_eq!(
        recursion::recursive_aggregates(&ast, root).len(),
        2,
        "both A and B sit on the cycle"
    );
}

#[test]
fn the_fill_emitters_follow_the_sample_and_end_where_it_does() {
    let ast = model(MUTUAL);
    let root = type_of(&ast, "::m::A");
    let sample = mutual_sample();
    // No refusal and no cycle: a fill walk descends the value, and the
    // value is finite because `as` is an empty sequence.
    fill::fill(&ast, root, &sample, Vendor::Cyclone).expect("the sample terminates the walk");
    fill_cpp::fill_fastdds(&ast, root, &sample).expect("the sample terminates the walk");
    corba_cpp::fill_corba_cpp(&ast, root, &sample).expect("the sample terminates the walk");
    corba_java::fill_corba_java(&ast, root, &sample).expect("the sample terminates the walk");
    fill_ros::fill_ros(&ast, root, &sample).expect("the sample terminates the walk");
    fill_rust::fill_hdds(&ast, root, &sample).expect("the sample terminates the walk");
}

#[test]
fn every_walk_refuses_a_typedef_that_returns_to_itself() {
    let ast = model(ALIAS_CYCLE);
    let root = type_of(&ast, "::Ess");
    let sample = json!({ "ex": 1 });

    let mut refusals: Vec<(&str, String)> = Vec::new();
    let mut record = |what: &'static str, error: ProbeError| refusals.push((what, refusal(&error)));

    record(
        "canonicalise",
        canonical::canonicalise(&ast, root, &sample, "Ess").expect_err("no end"),
    );
    record(
        "fill",
        fill::fill(&ast, root, &sample, Vendor::Cyclone).expect_err("no end"),
    );
    record(
        "dump",
        dump::dump(&ast, root, Vendor::Cyclone).expect_err("no end"),
    );
    record(
        "fill_cpp",
        fill_cpp::fill_fastdds(&ast, root, &sample).expect_err("no end"),
    );
    record(
        "dump_cpp",
        dump_cpp::dump_fastdds(&ast, root).expect_err("no end"),
    );
    record(
        "fill_corba_cpp",
        corba_cpp::fill_corba_cpp(&ast, root, &sample).expect_err("no end"),
    );
    record(
        "dump_corba_cpp",
        corba_cpp::dump_corba_cpp(&ast, root).expect_err("no end"),
    );
    record(
        "fill_corba_java",
        corba_java::fill_corba_java(&ast, root, &sample).expect_err("no end"),
    );
    record(
        "dump_corba_java",
        corba_java::dump_corba_java(&ast, root).expect_err("no end"),
    );
    record(
        "fill_ros",
        fill_ros::fill_ros(&ast, root, &sample).expect_err("no end"),
    );
    record(
        "dump_ros",
        dump_ros::dump_ros(&ast, root).expect_err("no end"),
    );
    record(
        "fill_rust",
        fill_rust::fill_hdds(&ast, root, &sample).expect_err("no end"),
    );
    record(
        "ros_package",
        ros_idl::ros_package(&ast, root).expect_err("no end"),
    );

    for (what, text) in &refusals {
        assert!(
            text.contains("a typedef chain that returns to itself"),
            "{what} has to name the construct rather than run into the stack: {text}"
        );
        assert!(
            text.contains("Tee"),
            "{what} has to name the typedef: {text}"
        );
    }
    assert_eq!(refusals.len(), 13, "every entry point is covered");
}

#[test]
fn the_cycle_finder_answers_over_a_typedef_that_returns_to_itself() {
    // The function every dump emitter asks before it can refuse
    // anything. It has no `Result` to refuse with, so the bound is the
    // `open` set: no connector is entered twice on one descent.
    let ast = model(ALIAS_CYCLE);
    let root = type_of(&ast, "::Ess");
    assert!(
        recursion::recursive_aggregates(&ast, root).is_empty(),
        "a typedef cycle closes over no aggregate"
    );
}

#[test]
fn the_cycle_finder_still_names_both_aggregates_of_a_mutual_cycle() {
    let ast = model(MUTUAL);
    let a = type_of(&ast, "::m::A");
    let b = type_of(&ast, "::m::B");
    let found = recursion::recursive_aggregates(&ast, a);
    assert!(found.contains(&a) && found.contains(&b), "{found:?}");
}

#[test]
fn a_map_that_reaches_the_same_aggregate_twice_still_names_it() {
    // The `open` set above must not lose an aggregate a second branch
    // reaches: a map's key and value are two descents from one node.
    let ast = model(
        "module m {\n  struct N;\n  @appendable struct N { long v; map<long, N> kids; };\n};",
    );
    let root = type_of(&ast, "::m::N");
    assert_eq!(recursion::recursive_aggregates(&ast, root), vec![root]);
}
