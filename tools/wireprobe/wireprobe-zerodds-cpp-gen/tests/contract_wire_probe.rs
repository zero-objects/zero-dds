// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-wireprobe-cpp-gen` with `zerodds-wire-probe`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-wire-probe`:
//! - `src/dump.rs`: `recursion::recursive_aggregates` to refuse a recursive
//!   type by name;
//! - `src/bin/wireprobe-cpp-matrix.rs`: `matrix::{CONFIGURATIONS,
//!   Configuration, ENCODINGS, ORDERS}` for the slots it answers and the
//!   flags it hands each unit, `value::MapLengthPolicy` for the map length.
//!
//! So the crate relies on:
//! - the aggregates on a cycle are exactly the recursive ones;
//! - the slot coordinates (twelve configurations, five encodings, two byte
//!   orders) and their names, which the C++ column keys its cells by;
//! - a configuration being its dialect plus only the overrides it names.
//!
//! Oracles:
//! OMG IDL 4.2 §7.4.1.4.4.4.4 (recursion); DDS-XTypes 1.3 §7.6.3.1.2
//! Table 60 (the five encodings), §7.4.3.5.3 rule (15) (a delimited map's
//! entry count, the two readings); the slot names of the reference
//! columns' contract (`crates/wire-probe/tests/reference_columns.rs`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::{Id, ScopedName};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_wire_probe::encode::model_from_idl;
use zerodds_wire_probe::matrix::{CONFIGURATIONS, Configuration, ENCODINGS, ORDERS};
use zerodds_wire_probe::recursion::recursive_aggregates;
use zerodds_wire_probe::value::{MapLengthPolicy, ProbeRequest};

fn type_in(ast: &Ast, name: &[&str]) -> Id<TypeNode> {
    ast.find_by_scoped_name(&ScopedName::absolute(
        name.iter().map(|segment| (*segment).to_string()).collect(),
    ))
    .ok()
    .and_then(zerodds_ast::AnyId::as_type)
    .expect("the type is in the model")
}

#[test]
fn idl_7_4_1_4_4_4_4_the_aggregates_on_a_cycle_are_the_recursive_ones() {
    // A type that reaches itself through a sequence, two that reach each
    // other, and a holder of both that is on no cycle itself.
    let idl = "module m {\n\
               @appendable struct Node { long value; sequence<Node> children; };\n\
               struct B;\n\
               @appendable struct A { long value; sequence<B> bs; };\n\
               @appendable struct B { long weight; sequence<A> as; };\n\
               @final struct Leaf { long v; };\n\
               @final struct Holder { Node n; Leaf l; };\n\
               };";
    let ast = model_from_idl(idl, "recursion.idl").unwrap();
    let node = type_in(&ast, &["m", "Node"]);
    let a = type_in(&ast, &["m", "A"]);
    let b = type_in(&ast, &["m", "B"]);
    let leaf = type_in(&ast, &["m", "Leaf"]);
    let holder = type_in(&ast, &["m", "Holder"]);
    assert_eq!(recursive_aggregates(&ast, node), vec![node]);
    let pair = recursive_aggregates(&ast, a);
    assert!(
        pair.len() == 2 && pair.contains(&a) && pair.contains(&b),
        "both ends of a mutual recursion"
    );
    assert_eq!(
        recursive_aggregates(&ast, holder),
        vec![node],
        "the holder is on no cycle"
    );
    assert!(recursive_aggregates(&ast, leaf).is_empty());
}

#[test]
fn idl_7_4_1_4_4_4_4_a_recursion_without_a_collection_is_no_model() {
    // An array is no indirection: the front end refuses the file, so a
    // generator never sees such a type.
    let idl = "module m { struct Node; @appendable struct Node { long value; Node pair[2]; }; };";
    let error = format!("{}", model_from_idl(idl, "array.idl").err().unwrap());
    assert!(error.contains("7.4.1.4.4.4.4"), "{error}");
}

#[test]
fn xtypes_7_6_3_1_2_the_slot_coordinates_are_twelve_configurations_five_encodings_two_orders() {
    let names: Vec<&str> = CONFIGURATIONS.iter().map(|c| c.name).collect();
    assert_eq!(
        names,
        [
            "specification",
            "cyclone",
            "rti",
            "fastdds",
            "omniorb",
            "tao",
            "jacorb",
            "opendds",
            "ros-fastrtps",
            "ros-cyclone",
            "universal-nextint",
            "rule-15-omits",
        ]
    );
    // DDS-XTypes 1.3 Table 60 names the five encodings.
    assert_eq!(
        ENCODINGS.map(|(name, _)| name),
        [
            "plain-cdr1",
            "pl-cdr1",
            "plain-cdr2",
            "delimited-cdr",
            "pl-cdr2"
        ]
    );
    assert_eq!(
        ENCODINGS.map(|(_, encoding)| encoding),
        [
            Encoding::PlainCdr1,
            Encoding::PlCdr1,
            Encoding::PlainCdr2,
            Encoding::DelimitedCdr,
            Encoding::PlCdr2,
        ]
    );
    assert_eq!(ORDERS.map(|(name, _)| name), ["le", "be"]);
    assert_eq!(
        ORDERS.map(|(_, order)| order),
        [Endianness::Little, Endianness::Big]
    );
}

#[test]
fn a_configuration_is_its_dialect_and_only_the_overrides_it_names() {
    for settings in &CONFIGURATIONS[..10] {
        let settings: &Configuration = settings;
        assert_eq!(
            settings.flags(),
            [
                "--dialect".to_string(),
                settings.dialect.as_str().to_string()
            ],
            "{} carries no override",
            settings.name
        );
        let request = settings.request("T", Encoding::PlainCdr2, Endianness::Little);
        let plain = ProbeRequest::in_dialect(
            "T",
            Encoding::PlainCdr2,
            Endianness::Little,
            settings.dialect,
        );
        assert_eq!(request.length_codes, plain.length_codes);
        assert_eq!(request.map_length, plain.map_length);
    }
    let nextint = &CONFIGURATIONS[10];
    assert_eq!(
        nextint.flags()[..2],
        ["--dialect".to_string(), "specification".to_string()]
    );
    assert!(nextint.flags().contains(&"--length-codes".to_string()));
    let rule15 = &CONFIGURATIONS[11];
    assert_eq!(
        rule15.flags()[2..],
        ["--map-length".to_string(), "rule-15-omits".to_string()]
    );
    assert_eq!(
        rule15
            .request("T", Encoding::PlainCdr2, Endianness::Little)
            .map_length,
        MapLengthPolicy::Rule15Omits
    );
}

#[test]
fn xtypes_7_4_3_5_3_the_two_readings_of_a_delimited_maps_length_by_name() {
    // Rule (15) as written omits the entry count behind a DHEADER; the
    // default keeps it, the same shape as the sequence §7.4.1.1.4 makes a
    // map equivalent to.
    assert_eq!(
        MapLengthPolicy::default(),
        MapLengthPolicy::SequenceEquivalent
    );
    assert!(MapLengthPolicy::SequenceEquivalent.is_default());
    for policy in MapLengthPolicy::ALL {
        assert_eq!(MapLengthPolicy::parse(policy.as_str()), Some(policy));
    }
    assert_eq!(
        MapLengthPolicy::SequenceEquivalent.as_str(),
        "sequence-equivalent"
    );
    assert_eq!(MapLengthPolicy::Rule15Omits.as_str(), "rule-15-omits");
    assert_eq!(
        MapLengthPolicy::parse("rule-15-omit"),
        None,
        "a typo is no policy"
    );
}
