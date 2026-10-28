// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! DDS-DCPS 1.4 §2.3.3 — the DCPS PSM IDL, through the generator.
//!
//! §2.3.3, pp. 138–162, is a single normative IDL file: the whole DCPS
//! platform-specific model, from `DomainParticipantFactory` down to the
//! QoS structures and the status kinds.
//! `crates/idl-parse/tests/fixtures/omg/zerodds_dcps.idl` is that file,
//! and `crates/idl-lower/tests/conformance.rs` already gates that it
//! parses and lowers.
//!
//! What it does not gate is the other half of the clause: that a
//! declaration comes out the far end. A front end that reads the whole
//! DCPS PSM and a generator that produces nothing for it is a coverage
//! claim with no artifact behind it, which is why this walks every
//! language rather than one.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;

use zerodds_ast::Ast;
use zerodds_lang_cst::Language;
use zerodds_lang_rules::generate::declarations;

/// The declarations the fixture carries, which a generated file has to
/// name somewhere for the pass to mean anything.
///
/// The fixture is a content-equivalent selection and not a copy: the
/// OMG's file is under copyright and its own header says so. What is
/// gated here is that the clause's shapes — a QoS policy over an
/// enumeration, a built-in topic sample over nested policies, and
/// `SampleInfo` over three state kinds — reach a declaration in every
/// language.
const DCPS_NAMES: &[&str] = &[
    "Duration_t",
    "DurabilityQosPolicy",
    "ReliabilityQosPolicy",
    "HistoryQosPolicy",
    "ResourceLimitsQosPolicy",
    "PublicationBuiltinTopicData",
    "SubscriptionBuiltinTopicData",
    "SampleInfo",
];

fn model() -> Ast {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../idl-parse/tests/fixtures/omg/zerodds_dcps.idl");
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let parsed = zerodds_idl_parse::parse(&source);
    assert!(
        parsed.diagnostics().iter().all(|d| !d.is_error()),
        "the DCPS PSM IDL must parse: {:?}",
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "zerodds_dcps.idl");
    assert!(
        lowered.diagnostics.iter().all(|d| !d.is_error()),
        "the DCPS PSM IDL must lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    ast.resolve_all_by_name().expect("a resolve pass");
    ast
}

#[test]
fn the_dcps_psm_idl_declares_the_types_the_clause_names() {
    let ast = model();
    let names: Vec<String> = ast
        .types()
        .map(|(_, node)| node.name.clone())
        .filter(|name| !name.is_empty())
        .collect();
    for expected in DCPS_NAMES {
        assert!(
            names.iter().any(|name| name == expected),
            "`{expected}` is not in the lowered DCPS PSM: {} types",
            names.len()
        );
    }
}

#[test]
fn every_language_declares_the_dcps_psm() {
    let ast = model();
    let mut refused = Vec::new();
    for language in Language::ALL {
        // `Language::Node` declares nothing and says so: its
        // materialiser builds plain object literals wherever the codec
        // walk needs one, so there is no named declaration to look for.
        // `crates/lang-rules/src/rule/mod.rs` states the exception and
        // `crates/lang-rules/tests/apply.rs` excludes it for the same
        // reason.
        if language == Language::Node {
            continue;
        }
        match declarations(&ast, language) {
            Ok(text) => {
                assert!(
                    !text.trim().is_empty(),
                    "{language:?} produced nothing for the DCPS PSM"
                );
                // A generator that emitted a header and no declarations
                // would pass an emptiness check; the clause's own names
                // are what make the pass mean something.
                let missing: Vec<&&str> = DCPS_NAMES
                    .iter()
                    .filter(|name| !text.contains(**name))
                    .collect();
                assert!(
                    missing.is_empty(),
                    "{language:?} did not declare {missing:?}"
                );
            }
            Err(error) => refused.push(format!("{language:?}: {error}")),
        }
    }
    assert!(
        refused.is_empty(),
        "the DCPS PSM was refused by {} of the {} languages: {refused:?}",
        refused.len(),
        Language::ALL.len()
    );
}
