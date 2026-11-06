// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-recorder-decode` with `zerodds-idl-compose`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-idl-compose` (`src/type_source.rs`,
//! `TypeSource::from_idl`): `compose_source_with` with `PathLoader::bare()`
//! and `ComposeOptions::default()` over a `--type-file` someone else wrote.
//! It relies on three things: a scoped name across modules arrives resolved
//! (the model it decodes with has no open reference), a vendor extension is
//! admitted with at most a warning, and every error of every stage is an
//! error-severity diagnostic, which refuses the file.
//!
//! Oracle: IDL 4.2 §7.5.2 (a scoped name `telemetry::Unit` names the
//! declaration in that module), zerodds-idl-frontend-1.0 §3.5 (the default
//! of `ComposeOptions` admits every vendor's extensions; OpenDDS's
//! `#pragma keylist <type> <member>`), IDL 4.2 §7.4.1.4.4.4.4 (a member
//! whose type names nothing is not a valid declaration).
#![allow(clippy::unwrap_used, clippy::panic)]

use zerodds_idl_compose::{ComposeOptions, PathLoader, compose_source_with};
use zerodds_idl_parse::Severity;

fn errors(source: &str) -> (zerodds_idl_compose::Composed, usize) {
    let composed = compose_source_with(
        "<type-file>",
        source,
        &PathLoader::bare(),
        ComposeOptions::default(),
    );
    let count = composed
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    (composed, count)
}

#[test]
fn idl_7_5_2_a_scoped_name_across_modules_arrives_resolved() {
    let (composed, count) = errors(
        "module telemetry { enum Unit { METRE, SECOND }; };
         module app { struct Reading { telemetry::Unit unit; double value; }; };",
    );
    assert_eq!(count, 0, "{:?}", composed.diagnostics);
    let (reading, _) = composed
        .ast
        .types()
        .find(|(_, n)| n.name == "Reading")
        .unwrap();
    for member in composed.ast.members_of(reading) {
        let member = composed.ast.member(*member).unwrap();
        assert!(
            member.type_.resolved().is_some(),
            "{} is left unresolved",
            member.name
        );
    }
}

#[test]
fn a_vendor_pragma_in_a_type_file_is_no_error() {
    let (composed, count) = errors("struct Track { long id; };\n#pragma keylist Track id\n");
    assert_eq!(count, 0, "{:?}", composed.diagnostics);
}

#[test]
fn idl_7_4_1_4_4_4_4_a_member_typed_with_a_name_that_names_nothing_is_an_error() {
    let (_, count) = errors("module app { struct Reading { telemetry::Missing unit; }; };");
    assert!(count > 0);
}
