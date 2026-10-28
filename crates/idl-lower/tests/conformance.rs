// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The conformance gate. Spec §8.
//!
//! The fixtures are the gate because nothing in this rebuild produced
//! them: the XTypes specification's own IDL, the DCPS and Security IDL,
//! and hand-curated files from four vendor dialects.
//!
//! Three parts, because parsing alone proves little. A parser that
//! accepts everything and understands nothing also parses every fixture.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

fn fixtures() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../idl-parse/tests/fixtures");
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read the fixture directory") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "idl") {
                out.push(path);
            }
        }
    }
    out.sort();
    assert!(!out.is_empty(), "no fixtures found");
    out
}

fn name(path: &Path) -> String {
    path.file_name()
        .expect("a file name")
        .to_string_lossy()
        .to_string()
}

// -- part one: every fixture parses with no diagnostics ------------------

/// How many `keylist` directives each fixture writes.
///
/// `keylist` is RTI Connext's and is in no building block of IDL 4.2, so
/// the default parse accepts it and says so, once per directive. The
/// count is here rather than a blanket "warnings are fine" because a
/// fixture that grew a second unportable construct would otherwise pass
/// unnoticed — which is the whole reason the parse says anything at all.
const EXPECTED_VENDOR_WARNINGS: &[(&str, usize)] =
    &[("sensor_topic.idl", 2), ("with_preprocessor.idl", 2)];

#[test]
fn every_fixture_parses_with_no_diagnostics_beyond_the_extensions_it_declares() {
    let mut failures = Vec::new();
    for path in fixtures() {
        let file = name(&path);
        let source = std::fs::read_to_string(&path).expect("read the fixture");
        let parsed = zerodds_idl_parse::parse(&source);
        let expected = EXPECTED_VENDOR_WARNINGS
            .iter()
            .find(|(fixture, _)| *fixture == file)
            .map_or(0, |(_, count)| *count);
        let vendor = parsed
            .diagnostics()
            .iter()
            .filter(|d| {
                d.code == zerodds_idl_parse::DiagnosticCode::VendorExtension
                    && d.severity == zerodds_idl_parse::Severity::Warning
            })
            .count();
        let other = parsed.diagnostics().len() - vendor;
        if other != 0 || vendor != expected {
            failures.push(format!(
                "{file}: expected {expected} vendor warning(s) and nothing else, got {:?}",
                parsed.diagnostics().iter().take(3).collect::<Vec<_>>()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_fixtures_listed_as_unportable_are_the_ones_that_actually_are() {
    // A fixture listed here that stopped using the extension would
    // otherwise sit in the table forever, claiming to cover a gate it no
    // longer trips.
    for (file, count) in EXPECTED_VENDOR_WARNINGS {
        let path = fixtures()
            .into_iter()
            .find(|path| name(path) == *file)
            .unwrap_or_else(|| panic!("{file} is listed but not present"));
        let source = std::fs::read_to_string(&path).expect("read the fixture");
        let parsed = zerodds_idl_parse::parse(&source);
        let vendor = parsed
            .diagnostics()
            .iter()
            .filter(|d| d.code == zerodds_idl_parse::DiagnosticCode::VendorExtension)
            .count();
        assert_eq!(vendor, *count, "{file}");
    }
}

// -- part two: every fixture round-trips byte for byte -------------------

#[test]
fn every_fixture_round_trips_byte_for_byte() {
    // The property the whole tree exists for. A formatter that cannot
    // reproduce its input cannot be trusted to change one line of it.
    for path in fixtures() {
        let source = std::fs::read_to_string(&path).expect("read the fixture");
        let text = zerodds_idl_parse::parse(&source).syntax().text();
        assert_eq!(text, source, "{} did not round-trip", name(&path));
    }
}

// -- part three: the lowered model is coherent ---------------------------

/// What a fixture is expected to report, and why.
///
/// Two of the fixtures contain constructs that are legal to write and
/// wrong to mean; they were curated for grammar coverage, not for
/// semantic correctness. Listing them here is the difference between a
/// gate that knows what it allows and one that allows whatever it finds.
const EXPECTED: &[(&str, &[&str])] = &[
    // `abstract eventtype CmdBase;` and `component CompForward;` are
    // forward declarations the file never defines. IDL 4.2 §7.4.1.4.5
    // requires a definition in the same translation unit.
    ("ccm_components.idl", &["IDL0015", "IDL0015"]),
    // Two members share `@hashid("position")`, so both hash to one id
    // and neither is distinguishable on the wire.
    ("fastdds_pragmas.idl", &["IDL0016"]),
    // `typeprefix Bar` and `typeid Bar` name a scope the file never
    // declares; the fixture demonstrates the directives, not a module
    // for them to apply to. CORBA 3.3 §10.7.
    ("corba_value_repository.idl", &["IDL0010", "IDL0010"]),
];

#[test]
fn every_fixture_lowers_to_exactly_the_diagnostics_it_should() {
    let mut failures = Vec::new();
    for path in fixtures() {
        let file = name(&path);
        let source = std::fs::read_to_string(&path).expect("read the fixture");
        let parsed = zerodds_idl_parse::parse(&source);
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), &file);

        let expected: Vec<&str> = EXPECTED
            .iter()
            .find(|(fixture, _)| *fixture == file)
            .map(|(_, codes)| codes.to_vec())
            .unwrap_or_default();
        let actual: Vec<&str> = lowered
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect();

        if actual != expected {
            failures.push(format!(
                "{file}: expected {expected:?}, got {actual:?} — {:?}",
                lowered
                    .diagnostics
                    .iter()
                    .map(|d| &d.message)
                    .collect::<Vec<_>>()
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_expected_diagnostics_are_the_ones_the_fixtures_actually_earn() {
    // A fixture listed here that stopped reporting would otherwise sit
    // in the table forever, claiming to cover a rule it no longer trips.
    for (file, codes) in EXPECTED {
        let path = fixtures()
            .into_iter()
            .find(|path| name(path) == *file)
            .unwrap_or_else(|| panic!("{file} is listed but not present"));
        let source = std::fs::read_to_string(&path).expect("read the fixture");
        let parsed = zerodds_idl_parse::parse(&source);
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), file);
        assert_eq!(
            lowered.diagnostics.len(),
            codes.len(),
            "{file} no longer reports what the table claims"
        );
    }
}

#[test]
fn every_fixture_produces_a_model_with_declarations_in_it() {
    // Guards against the empty pass: a lowering that produced nothing
    // would satisfy every other check here.
    for path in fixtures() {
        let source = std::fs::read_to_string(&path).expect("read the fixture");
        let parsed = zerodds_idl_parse::parse(&source);
        let ast = zerodds_idl_lower::lower(&parsed.syntax(), &name(&path)).ast;
        assert!(
            ast.types().count() > 0 || ast.interfaces().count() > 0,
            "{} lowered to nothing",
            name(&path)
        );
    }
}

#[test]
fn every_node_of_every_fixture_names_the_file_it_came_from() {
    for path in fixtures() {
        let source = std::fs::read_to_string(&path).expect("read the fixture");
        let file = name(&path);
        let parsed = zerodds_idl_parse::parse(&source);
        let ast = zerodds_idl_lower::lower(&parsed.syntax(), &file).ast;

        for (id, node) in ast.types() {
            // The built-in primitives are the model's, not the file's.
            if node.name.is_empty() || node.facets.source.span.is_none() {
                continue;
            }
            let any = ast.any_of_type(id).expect("any");
            let facets = ast.facets(any).expect("facets");
            match &facets.source.origin {
                zerodds_ast::SourceOrigin::File { path } => {
                    assert_eq!(path, &file, "{} names the wrong file", node.name);
                }
                other => panic!("{} in {file} has origin {other:?}", node.name),
            }
        }
    }
}

// -- the strong one ------------------------------------------------------

#[test]
fn the_xtypes_specifications_own_idl_lowers_completely() {
    // dds_xtypes.idl is a hand-curated selection of the constructs XTypes
    // 1.3 defines (its header says which), and it uses parts of the
    // language a fixture written for one rule would not think to cover.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../idl-parse/tests/fixtures/omg/dds_xtypes.idl");
    let source = std::fs::read_to_string(&path).expect("read dds_xtypes.idl");
    let parsed = zerodds_idl_parse::parse(&source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );

    let mut lowered = zerodds_idl_lower::lower(&parsed.syntax(), "dds_xtypes.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);

    let report = lowered.ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.is_clean(),
        "names left unresolved: {:?}",
        report.broken.iter().take(5).collect::<Vec<_>>()
    );

    let verdict = lowered.ast.validate_complete();
    assert!(verdict.is_ok(), "{verdict:?}");
}

// -- the model survives a snapshot ---------------------------------------

#[test]
fn every_fixture_survives_a_snapshot_round_trip() {
    for path in fixtures() {
        let source = std::fs::read_to_string(&path).expect("read the fixture");
        let file = name(&path);
        let parsed = zerodds_idl_parse::parse(&source);
        let ast = zerodds_idl_lower::lower(&parsed.syntax(), &file).ast;

        let bytes = ast.to_snapshot().expect("snapshot");
        let loaded = zerodds_ast::Ast::from_snapshot(&bytes).expect("load");
        assert_eq!(loaded.node_count(), ast.node_count(), "{file}: node count");
        assert_eq!(
            loaded.to_snapshot().expect("re-snapshot"),
            bytes,
            "{file}: not byte-identical"
        );
    }
}

// -- through the preprocessor --------------------------------------------

/// A loader over the fixture tree, so an `#include` finds its file.
struct FixtureLoader {
    root: PathBuf,
}

impl zerodds_idl_parse::FileLoader for FixtureLoader {
    fn load(&self, path: &str, _from: Option<&str>) -> Option<(String, String)> {
        // An include is relative to the including file's directory; the
        // fixtures that use one sit beside what they include.
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).ok()? {
                let candidate = entry.ok()?.path();
                if candidate.is_dir() {
                    stack.push(candidate);
                } else if candidate.file_name().is_some_and(|name| name == path) {
                    let text = std::fs::read_to_string(&candidate).ok()?;
                    return Some((path.to_string(), text));
                }
            }
        }
        None
    }
}

#[test]
fn a_fixture_with_an_include_gains_the_declarations_it_includes() {
    // with_preprocessor.idl includes sensor_common.idl. Parsed on its
    // own, `Header` is a name from nowhere; through the preprocessor it
    // is a declaration.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../idl-parse/tests/fixtures");
    let path = root.join("rti/with_preprocessor.idl");
    let source = std::fs::read_to_string(&path).expect("read the fixture");

    let loader = FixtureLoader { root };
    let pre = zerodds_idl_parse::preprocess("with_preprocessor.idl", &source, &loader);
    assert!(pre.diagnostics.is_empty(), "{:?}", pre.diagnostics);

    let parsed = zerodds_idl_parse::parse_preprocessed(&pre);
    // The file's `keylist` directives are RTI's and are reported as
    // such; nothing else is wrong with it.
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());

    let ast = zerodds_idl_lower::lower(&parsed.syntax(), "with_preprocessor.idl").ast;
    let names: Vec<String> = ast
        .types()
        .map(|(_, node)| node.name.clone())
        .filter(|name| !name.is_empty())
        .collect();
    assert!(
        names.iter().any(|name| name == "ConfiguredTopic"),
        "the file's own declaration is missing: {names:?}"
    );
    for included in ["Header", "ChannelId"] {
        assert!(
            names.iter().any(|name| name == included),
            "the included declaration `{included}` did not arrive: {names:?}"
        );
    }
}

#[test]
fn a_conditional_in_a_fixture_keeps_the_branch_out_of_the_model() {
    // `#ifdef ENABLE_DEBUG` is never defined, so DebugChannel is not a
    // declaration of this translation unit.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../idl-parse/tests/fixtures");
    let path = root.join("rti/with_preprocessor.idl");
    let source = std::fs::read_to_string(&path).expect("read the fixture");
    let pre =
        zerodds_idl_parse::preprocess("with_preprocessor.idl", &source, &FixtureLoader { root });
    let parsed = zerodds_idl_parse::parse_preprocessed(&pre);
    let ast = zerodds_idl_lower::lower(&parsed.syntax(), "with_preprocessor.idl").ast;
    assert!(
        ast.types().all(|(_, node)| node.name != "DebugChannel"),
        "a branch that is off reached the model"
    );
}

/// An annotation whose parameter is an enumerated kind names a value the
/// file never declares: `AutoidKind`, `ExtensibilityKind`,
/// `TryConstructFailAction` and the data-representation constants come
/// with the annotation, not with the translation unit. Lowering the
/// argument as an expression as well left a pending `NameRef` on that
/// name, which the resolve pass then broke — and a broken reference
/// fails the whole file, so `@autoid(HASH)`, `@extensibility(MUTABLE)`
/// and `@try_construct(TRIM)` could not be planned at all.
#[test]
fn an_enumerated_annotation_argument_leaves_nothing_to_resolve() {
    for source in [
        "@autoid(HASH) @mutable struct S { long a; };",
        "@autoid(SEQUENTIAL) @mutable struct S { long a; };",
        "@extensibility(MUTABLE) struct S { long a; };",
        "@mutable struct S { @try_construct(TRIM) string a; };",
        "@data_representation(XCDR1 | XCDR2) @mutable struct S { long a; };",
    ] {
        let parsed = zerodds_idl_parse::parse(source);
        assert!(parsed.diagnostics().is_empty(), "{source}");
        let mut lowered = zerodds_idl_lower::lower(&parsed.syntax(), "test.idl");
        assert!(
            lowered.diagnostics.is_empty(),
            "{source}: {:?}",
            lowered.diagnostics
        );
        let report = lowered.ast.resolve_all_by_name().expect("a resolve pass");
        assert!(report.is_clean(), "{source}: {:?}", report.broken);
    }
}
