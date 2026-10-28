// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The recursion OMG IDL 4.2 §7.4.1.4.4.4.4 does not admit.
//!
//! A type may contain itself only across something at which a *value*
//! can stop: a sequence or a map, whose length may be zero, or an
//! `@optional` member, which may be absent. A cycle with none of those
//! on it describes a value that never terminates.
//!
//! The rule existed and cited the clause — `zerodds-ast-layout` raises
//! `LayoutError::RecursionOutsideCollection` — and it sat one stage past
//! the front end. `zerodds-idlc check` reported nothing for
//! `struct Sa { Sa m; };`; only `generate` refused it, and the person
//! using `check` to ask whether a file is valid IDL was not told.
//!
//! It runs in `compose` rather than in the CLI so that every caller of
//! the front end gets the same answer: `check`, a `build.rs` through
//! `zerodds-build`, and a wire probe all compose the same way.
//!
//! What it is **not** is a second implementation. `compose` calls
//! `zerodds_ast_layout::types_that_close_an_inadmissible_cycle`, which
//! runs the planner's own walk and keeps that one error. The barrier
//! bookkeeping lives in the descent, and a rule written twice drifts.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

/// The error messages `compose` reports for `source`.
fn errors(source: &str) -> Vec<String> {
    zerodds_idl_compose::compose_source(
        "test.idl",
        source,
        &zerodds_idl_compose::PathLoader::bare(),
    )
    .diagnostics
    .iter()
    .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
    .map(|d| d.message.clone())
    .collect()
}

fn refuses_the_cycle(source: &str) -> bool {
    errors(source)
        .iter()
        .any(|m| m.contains("closes a type cycle"))
}

#[test]
fn a_cycle_with_no_terminator_on_it_is_refused() {
    for source in [
        "struct Sa { Sa m; };",
        // An array has no length on the wire, so a cycle through one
        // never terminates. `tests/interop-corpus/recursive-array` is
        // the measured form of this case.
        "struct Sa { Sa m[3]; };",
        // Mutual recursion is the same cycle with two names on it.
        "struct Sb;\nstruct Sa { Sb m; };\nstruct Sb { Sa m; };",
        "union Ua switch (long) { case 1: Ua m; };",
    ] {
        assert!(
            refuses_the_cycle(source),
            "{source:?} should be refused: {:?}",
            errors(source)
        );
    }
}

#[test]
fn a_cycle_with_a_terminator_on_it_is_admitted() {
    // The half that matters most: refusing these would break every
    // recursive type in the tree. §7.4.1.4.4.4.4 admits the collection
    // forms, and XTypes 1.3 §7.3.1.2.1.3 gives `@optional` the same
    // property — Fast DDS carries it, measured in
    // `tests/interop-corpus/recursive-optional-external`.
    for source in [
        "struct Sa { sequence<Sa> m; };",
        "struct Sa { sequence<Sa, 4> m; };",
        "struct Sa { @optional Sa m; };",
        "struct Sa { map<long, Sa> m; };",
        // Mutual recursion across sequences, the corpus's
        // `recursive-mutual`.
        "struct Sb;\nstruct Sa { sequence<Sb> bs; };\nstruct Sb { sequence<Sa> as; };",
        "struct Sb { long m; };\nstruct Sa { Sb m; };",
        // A name that merely contains the type's own is not a cycle.
        "struct Sa { long Sa_field; };",
    ] {
        assert!(
            !refuses_the_cycle(source),
            "{source:?} is IDL 4.2 and was refused: {:?}",
            errors(source)
        );
    }
}

#[test]
fn the_diagnostic_names_the_type_and_the_clause() {
    // A refusal that does not say which type closed the cycle sends the
    // author looking through the file. The layout stage's own message
    // named a `NodeId`, which is worse than nothing outside a debugger.
    let found = errors("module Ma { struct Sa { Sa m; }; };");
    let message = found
        .iter()
        .find(|m| m.contains("closes a type cycle"))
        .expect("the refusal");
    assert!(message.contains("Ma::Sa"), "{message}");
    assert!(message.contains("7.4.1.4.4.4.4"), "{message}");
}

#[test]
fn the_interop_corpus_case_for_this_is_refused_and_its_neighbours_are_not() {
    // The corpus curates seven recursion cases and exactly one of them
    // is the inadmissible kind. Reading them off disk rather than
    // restating them keeps this test measuring the same files the wire
    // gate measures.
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let case = |name: &str| {
        let path = repo
            .join("tests/interop-corpus")
            .join(name)
            .join("type.idl");
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    };
    assert!(
        refuses_the_cycle(&case("recursive-array")),
        "recursive-array is the case IDL 4.2 §7.4.1.4.4.4.4 forbids"
    );
    for admitted in [
        "recursive-self-sequence",
        "recursive-mutual",
        "recursive-through-map",
        "recursive-optional-external",
        "recursive-typedef-sequence",
        "recursive-union-branch",
    ] {
        assert!(
            !refuses_the_cycle(&case(admitted)),
            "{admitted} is admitted recursion and was refused"
        );
    }
}

/// One cycle is one error, whether or not the type was forward declared.
///
/// `types_that_close_an_inadmissible_cycle` deduplicates on the node at
/// which the cycle closes. A forward declaration and its definition are
/// two type entries that close it at two different nodes, so the walk
/// returned both and `compose` turned both into diagnostics — the second
/// with no name and no position, because a forward declaration carries
/// neither. The corpus case `recursive-array` is the shape, and its
/// refusal reason is recorded in `tests/interop-results/ledger.tsv`,
/// where the second sentence read `` `::` closes a type cycle `` and
/// named nothing.
#[test]
fn a_forward_declared_cycle_is_reported_once() {
    let reported: Vec<String> =
        errors("module m { struct Node; @appendable struct Node { long value; Node pair[2]; }; };")
            .into_iter()
            .filter(|m| m.contains("closes a type cycle"))
            .collect();
    assert_eq!(
        reported.len(),
        1,
        "one cycle, one error — got {reported:#?}"
    );
    assert!(
        reported[0].contains("::m::Node"),
        "the error names the type: {}",
        reported[0]
    );
}

#[test]
fn external_indirection_requires_a_finite_value_not_just_a_pointer() {
    // Reduced from CycloneDDS 11.0.1 ddsi_xt_typeinfo.idl:
    // TypeIdentifier -> PlainSequenceSElemDefn -> @external TypeIdentifier.
    // Neutral names ensure this is a semantic rule, not a vendor allowlist.
    let legal = [
        "union Tag; struct Item { @external Tag element; }; union Tag switch (long) { case 0: long leaf; case 1: Item nested; };",
        "union Tag; typedef Tag Ref; struct Item { @external Ref element; }; union Tag switch (long) { case 0: long leaf; case 1: Item nested; };",
        "union Tag switch (long) { case 0: long leaf; case 1: @external Tag nested; };",
    ];
    for source in legal {
        let composed = zerodds_idl_compose::compose_source(
            "external.idl",
            source,
            &zerodds_idl_compose::PathLoader::bare(),
        );
        assert!(errors(source).is_empty(), "{source}: {:?}", errors(source));
        // compose intentionally drops other planner errors. Demand actual
        // plans too: replacing IDL0014 with TooDeep would not fix generation.
        for encoding in [
            zerodds_ast_layout::Encoding::PlainCdr1,
            zerodds_ast_layout::Encoding::PlainCdr2,
        ] {
            let key = zerodds_ast_layout::LayoutKey::data(
                encoding,
                zerodds_ast_layout::Endianness::Little,
            );
            for (id, _) in composed.ast.types() {
                let result = zerodds_ast_layout::xcdr::plan(
                    &composed.ast,
                    id,
                    key,
                    &mut zerodds_ast_layout::xcdr::DepCollector::new(),
                );
                assert!(result.is_ok(), "{source}: {encoding:?}: {result:?}");
            }
        }
    }
    for source in [
        "struct Node { @external Node next; };",
        "struct A; struct B { @external A a; }; struct A { B b; };",
        "union Tag; struct Item { @external Tag element; }; union Tag switch (long) { case 0: Item nested; default: Item other; };",
        // Even a terminating alternative does not legalize inline storage.
        "union Tag switch (long) { case 0: long leaf; case 1: Tag nested; };",
        "union Tag; struct Item { Tag element; }; union Tag switch (long) { case 0: long leaf; case 1: Item nested; };",
    ] {
        assert!(refuses_the_cycle(source), "{source}: {:?}", errors(source));
    }
}
