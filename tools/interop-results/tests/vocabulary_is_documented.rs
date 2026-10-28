// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every verdict this tree can print is in the document that defines
//! them, and every verdict survives a round trip through the ledger.
//!
//! # Why a verdict needs a document
//!
//! Because the vocabulary is the whole of the instrument. The numbers
//! this tree produced about nine toolchains were wrong three times in
//! a row, and every one of those was a verdict doing two jobs:
//! "refused" meant a specification without the construct *and* a
//! generator that emits code it cannot build *and* a value outside the
//! type *and* a gap in a harness of ours. A word that means four
//! things reports three of them wrongly, and no test failed while it
//! did.
//!
//! `docs/specs/zerodds-verdicts-1.0.md` is where each word gets one
//! meaning and the evidence that produces it. A verdict added to the
//! enum and not to that file is a fifth job waiting to happen.
#![allow(clippy::expect_used, clippy::panic)]

use zerodds_interop_results::ledger::Outcome;

/// One instance of every verdict.
///
/// Written out rather than derived: a list that built itself from the
/// enum would pass for a variant nobody thought about, which is the
/// thing this file is for.
fn every_verdict() -> Vec<Outcome> {
    vec![
        Outcome::Bytes("00".to_string()),
        Outcome::Same,
        Outcome::Different("a value".to_string()),
        Outcome::Refused("in its own words".to_string()),
        Outcome::Unwritten,
        Outcome::VersionAlias("plain-cdr1"),
        Outcome::NoVersion2,
        Outcome::OneOrderOnly,
        Outcome::NoForm("a bitmask, which CORBA IDL has not".to_string()),
        Outcome::OutOfProfile("not inside CORBA 3.3".to_string()),
        Outcome::GeneratorDefect("its own C++ does not build".to_string()),
        Outcome::MappingDivergence("x87 extended, not binary128".to_string()),
        Outcome::HarnessGap("this probe has not written it".to_string()),
        Outcome::NoProbe,
        Outcome::NotMeasured("the command that would".to_string()),
    ]
}

fn specification() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/specs/zerodds-verdicts-1.0.md");
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn every_verdict_is_in_the_document_that_defines_them() {
    let text = specification();
    let mut missing = Vec::new();
    for verdict in every_verdict() {
        let token = verdict.token();
        if !text.contains(token) {
            missing.push(token);
        }
    }
    assert!(
        missing.is_empty(),
        "these verdicts can be printed and are not in \
         docs/specs/zerodds-verdicts-1.0.md, so nothing says what they \
         mean or what produces them: {missing:?}"
    );
}

#[test]
fn every_verdict_survives_its_own_two_columns() {
    // The ledger is two columns, a token and a detail. A verdict that
    // does not come back from them is one the tree can write and not
    // read, and a re-record would then differ from itself.
    for verdict in every_verdict() {
        let parsed = Outcome::parse(verdict.token(), &verdict.detail());
        assert_eq!(
            parsed.as_ref().map(Outcome::token),
            Some(verdict.token()),
            "`{}` does not come back from its own columns",
            verdict.token()
        );
    }
}

#[test]
fn an_unknown_token_is_refused_rather_than_guessed() {
    assert!(
        Outcome::parse("no-such-verdict", "").is_none(),
        "an unrecognised verdict has to be refused; guessing one is how \
         `_ => Unwritten` turned an unknown answer into a sentence about \
         a direction that was never run"
    );
}

/// The four verdicts that are **not** a measurement of the toolchain.
///
/// `is_measurement` decides what a diff between two runs counts. A
/// verdict that is a property of the case (`out-of-profile`), of this
/// tree (`harness-gap`, `not-measured`, `no-probe`) or of the slot
/// (`no-version-2`, `one-order-only`, `version-alias`) is not a thing
/// the toolchain did, and counting it would report a corpus edit as a
/// vendor change.
#[test]
fn what_counts_as_a_measurement_is_what_the_toolchain_did() {
    for verdict in [
        Outcome::OutOfProfile(String::new()),
        Outcome::HarnessGap(String::new()),
        Outcome::NotMeasured(String::new()),
        Outcome::NoProbe,
        Outcome::NoVersion2,
        Outcome::OneOrderOnly,
        Outcome::Unwritten,
    ] {
        assert!(
            !verdict.is_measurement(),
            "`{}` is not something a toolchain did",
            verdict.token()
        );
    }
    for verdict in [
        Outcome::Bytes(String::new()),
        Outcome::Same,
        Outcome::Different(String::new()),
        Outcome::Refused(String::new()),
        Outcome::GeneratorDefect(String::new()),
        Outcome::MappingDivergence(String::new()),
    ] {
        assert!(
            verdict.is_measurement(),
            "`{}` is something a toolchain did and a diff has to see it",
            verdict.token()
        );
    }
}
