// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A place that a statement has to create, and the values that used to
//! leave it uncreated.
//!
//! `std::map::operator[]` inserts the entry, and the only thing that
//! calls it is a statement naming the subscript. So a map value whose
//! filled form is **empty** — an empty sequence, an empty inner map, a
//! structure with no member to write — emitted no statement, the entry
//! was never inserted, and Fast DDS serialised a map one entry short of
//! the sample. Sixteen rows of `tests/interop-results/` recorded that as
//! a Fast DDS divergence; `docs/open-rows-breakdown.md` cause **E**.
//!
//! The corpus reaches two of these shapes. The rest are here because
//! the defect is a property of the *emitter*, not of the two cases that
//! happened to hold it: any value whose C++ form is empty has it, and a
//! corpus that grows one is a wrong measurement of a vendor, not a red
//! test. Every assertion below fails against the emitter as it was.
//!
//! The two shapes the corpus does not have and which were checked and
//! are **not** defects are recorded here too, as tests: an empty
//! `string` map value writes `""` and an absent `@optional` *member*
//! outside a map has nothing to create. A checked non-existence is a
//! result.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use serde_json::json;
use zerodds_ast_core::ScopedName;
use zerodds_wire_probe::encode::model_from_idl;
use zerodds_wire_probe::fill_cpp::fill_fastdds;

/// The C++ statements for one type and one sample.
fn source(idl: &str, type_name: &str, values: &serde_json::Value) -> String {
    let ast = model_from_idl(idl, "fill_creates_the_place.idl").expect("the model");
    let type_ = ast
        .find_by_scoped_name(&ScopedName::absolute(vec![type_name.to_string()]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("the type is in the model");
    fill_fastdds(&ast, type_, values).expect("the emitter")
}

/// Every entry of the sample's map is named by some statement.
///
/// The count comes from the value document and not from the emitter, so
/// the assertion is not a restatement of the walk it checks.
fn asserts_every_key(source: &str, keys: &[&str]) {
    for key in keys {
        let subscript = format!("entries()[{key}]");
        assert!(
            source.contains(&subscript),
            "no statement names `{subscript}`, so the entry is never inserted:\n{source}"
        );
    }
}

// -- the shapes the corpus holds ---------------------------------------

/// `map-value-sequence`. The entry keyed 1 holds `[]`.
#[test]
fn a_map_entry_whose_value_is_an_empty_sequence_is_still_created() {
    let idl = "@final struct Sample { map<long, sequence<long> > entries; long tail; };";
    let source = source(
        idl,
        "Sample",
        &json!({"entries": [[1, []], [2, [-1234567]]], "tail": -1}),
    );
    asserts_every_key(&source, &["1", "2"]);
}

/// `map-nested`. The entry keyed 2 holds `{}`.
#[test]
fn a_map_entry_whose_value_is_an_empty_map_is_still_created() {
    let idl = "@final struct Sample { map<long, map<long, long> > entries; long tail; };";
    let source = source(
        idl,
        "Sample",
        &json!({"entries": [[1, [[10, -1234567]]], [2, []]], "tail": -1}),
    );
    asserts_every_key(&source, &["1", "2"]);
}

// -- the shapes the corpus does not hold yet ---------------------------

/// A structure all of whose members are absent `@optional`s fills
/// nothing, and the entry is still the sample's.
#[test]
fn a_map_entry_whose_value_is_a_structure_with_nothing_to_write_is_still_created() {
    let idl = "@mutable struct Inner { @optional long a; };\n\
               @final struct Sample { map<long, Inner> entries; long tail; };";
    let source = source(
        idl,
        "Sample",
        &json!({"entries": [[1, {}], [2, {"a": 7}]], "tail": -1}),
    );
    asserts_every_key(&source, &["1", "2"]);
}

/// A structure whose only member is an empty sequence is the same
/// shape one level down.
#[test]
fn a_map_entry_whose_value_is_a_structure_holding_only_an_empty_sequence_is_still_created() {
    let idl = "@final struct Inner { sequence<long> q; };\n\
               @final struct Sample { map<long, Inner> entries; long tail; };";
    let source = source(
        idl,
        "Sample",
        &json!({"entries": [[1, {"q": []}], [2, {"q": [3]}]], "tail": -1}),
    );
    asserts_every_key(&source, &["1", "2"]);
}

/// An array reaches its elements through the subscript, so an array of
/// empty sequences names the map entry only if one of its elements
/// writes something — and none of them does.
#[test]
fn a_map_entry_whose_value_is_an_array_of_empty_sequences_is_still_created() {
    let idl = "typedef sequence<long> Row;\n\
               typedef Row Pair[2];\n\
               @final struct Sample { map<long, Pair> entries; long tail; };";
    let source = source(
        idl,
        "Sample",
        &json!({"entries": [[1, [[], []]], [2, [[3], []]]], "tail": -1}),
    );
    asserts_every_key(&source, &["1", "2"]);
}

/// A sequence of empty sequences writes its own `resize`, so the entry
/// is created by the outer sequence and not by the guard. Here so that
/// a fix which created *every* entry twice would be visible.
#[test]
fn a_map_entry_whose_value_is_a_sequence_of_empty_sequences_is_created_once() {
    let idl = "typedef sequence<long> Row;\n\
               @final struct Sample { map<long, sequence<Row> > entries; long tail; };";
    let source = source(
        idl,
        "Sample",
        &json!({"entries": [[1, [[], []]]], "tail": -1}),
    );
    asserts_every_key(&source, &["1"]);
    assert!(
        !source.contains("(void)"),
        "the outer sequence's resize already names the place:\n{source}"
    );
}

/// A value that writes a statement of its own needs no guard, and
/// getting one would be a second statement nobody asked for.
#[test]
fn a_map_entry_that_writes_a_value_gets_no_discarded_subscript() {
    let idl = "@final struct Sample { map<long, long> entries; long tail; };";
    let source = source(idl, "Sample", &json!({"entries": [[1, 2]], "tail": -1}));
    assert!(source.contains("entries()[1] = 2;"), "{source}");
    assert!(!source.contains("(void)"), "{source}");
}

// -- checked, and not defects ------------------------------------------

/// `map-value-string`. An empty `string` assigns `""`, which names the
/// place, so this shape never had the defect.
#[test]
fn a_map_entry_whose_value_is_an_empty_string_was_already_created_by_its_assignment() {
    let idl = "@final struct Sample { map<long, string> entries; long tail; };";
    let source = source(idl, "Sample", &json!({"entries": [[1, ""]], "tail": -1}));
    assert!(source.contains("entries()[1] = \"\";"), "{source}");
    assert!(!source.contains("(void)"), "{source}");
}

/// An `@optional` member absent from the sample is absent from the
/// generated structure too, and a statement creating it would be the
/// defect. Outside a map there is nothing to create.
#[test]
fn an_absent_optional_member_outside_a_map_writes_nothing() {
    let idl = "@mutable struct Sample { @optional long a; long tail; };";
    let source = source(idl, "Sample", &json!({"tail": -1}));
    assert!(!source.contains("sample.a"), "{source}");
    assert!(!source.contains("(void)"), "{source}");
}

// -- the other place a statement has to reach before it can write -----

/// A union branch filled through its getter is selected first.
///
/// The same class one construct along: `fastddsgen`'s branch getter
/// throws `BadParamException("This member has not been selected")`
/// until the branch is the selected one, and only the setter selects
/// it. `sample.picked().ids().resize(2)` therefore threw before a value
/// was written, and `union-member-sequence`, `union-member-struct` and
/// their twins stood in `sources/vendor-corpus.tsv` as sixteen Fast DDS
/// refusals that were this emitter's.
#[test]
fn a_union_branch_reached_through_its_getter_is_selected_before_it_is_filled() {
    let idl = "union Choice switch (short) { case -1: sequence<long> ids; \
               default: string text; };\n\
               @final struct Sample { Choice picked; long tail; };";
    let source = source(
        idl,
        "Sample",
        &json!({"picked": {"_d": -1, "ids": [-1234567]}, "tail": -1}),
    );
    let select = source
        .find("sample.picked().ids({});")
        .unwrap_or_else(|| panic!("the branch is never selected:\n{source}"));
    let fill = source
        .find("sample.picked().ids().resize(1);")
        .unwrap_or_else(|| panic!("the branch is never filled:\n{source}"));
    assert!(select < fill, "the selection comes first:\n{source}");
}

/// A structure branch is the same shape.
#[test]
fn a_union_branch_holding_a_structure_is_selected_before_it_is_filled() {
    let idl = "@final struct Cell { long a; };\n\
               union Choice switch (long) { case 1: Cell slot; default: octet raw; };\n\
               @final struct Sample { Choice picked; long tail; };";
    let source = source(
        idl,
        "Sample",
        &json!({"picked": {"_d": 1, "slot": {"a": 7}}, "tail": -1}),
    );
    assert!(source.contains("sample.picked().slot({});"), "{source}");
    assert!(source.contains("sample.picked().slot().a(7);"), "{source}");
}

/// A scalar branch is written by its own setter, which selects the
/// branch as it writes. A selection in front of it would be a statement
/// with nothing to do.
#[test]
fn a_scalar_union_branch_gets_no_selection_of_its_own() {
    let idl = "union Choice switch (long) { case 1: long whole; default: double fraction; };\n\
               @final struct Sample { Choice picked; long tail; };";
    let source = source(
        idl,
        "Sample",
        &json!({"picked": {"_d": 1, "whole": 7}, "tail": -1}),
    );
    assert!(source.contains("sample.picked().whole(7);"), "{source}");
    assert!(!source.contains("whole({})"), "{source}");
}

/// An empty sequence at a member writes its `resize(0)`.
///
/// The omission this used to carry was reasoned at a member — "the
/// member starts empty" — and was false at a map subscript. The
/// statement is unconditional now so that the reason does not have to
/// be re-derived at each place a sequence can sit.
#[test]
fn an_empty_sequence_at_a_member_still_writes_its_resize() {
    let idl = "@final struct Sample { sequence<long> q; long tail; };";
    let source = source(idl, "Sample", &json!({"q": [], "tail": -1}));
    assert!(source.contains("sample.q().resize(0);"), "{source}");
}
