// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Corpus compiler gates enforce production frontend admission in both directions.
#![allow(clippy::print_stderr, clippy::panic)]

#[path = "support/corpus_frontend.rs"]
mod corpus_frontend;

#[test]
fn a_by_value_array_cycle_is_refused_before_a_language_compiler() {
    assert!(!corpus_frontend::accepts(
        "struct Node { Node child[1]; };",
        "recursive-array.idl",
    ));
}

#[test]
fn sequence_recursion_still_reaches_a_language_compiler() {
    assert!(corpus_frontend::accepts(
        "struct Node { sequence<Node> children; };",
        "recursive-sequence.idl",
    ));
}

#[test]
#[should_panic(expected = "frontend admission changed")]
fn an_unlisted_refusal_is_not_silently_skipped() {
    corpus_frontend::accepts("struct Node { Node child[1]; };", "unexpected-cycle.idl");
}

#[test]
#[should_panic(expected = "frontend admission changed")]
fn a_previously_refused_case_that_becomes_valid_is_not_silently_skipped() {
    corpus_frontend::accepts("struct Node { long value; };", "recursive-array.idl");
}
