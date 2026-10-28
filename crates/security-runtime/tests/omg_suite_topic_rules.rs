// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The 27 `<topic_rule>` elements of the OMG DDS-Security interop suite,
//! each read back through the gate and compared against what its own topic
//! NAME says it should be.
//!
//! `TESTONLY_governance.xml` carries the naming convention in a comment:
//!
//! ```text
//! OD / PD             : Open / Protected Discovery
//! OA / RA / WA / RWA  : Open / Read / Write / ReadWrite Access
//! OM / SM / EM        : Open / Signed / Encrypted  Meta-data
//! OD / SD / ED        : Open / Signed / Encrypted  Data
//! ```
//!
//! That comment is the oracle: the expectations below are derived from the
//! topic name alone, not copied out of the rule bodies. All 27 rules sit in
//! ONE `<domain_rule>` for domain 0, and `OD_OA_OM_OD` — every protection
//! off — is the FIRST of them. A `topic_rules.first()` lookup therefore
//! reports "open" for all 27 topics, including `PD_RWA_EM_ED`.
//!
//! The suite is not vendored into this repo. Point
//! `ZERODDS_OMG_GOVERNANCE` at a checkout's `TESTONLY_governance.xml`; the
//! test skips loudly without it.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    // The loud skip when the interop suite is not checked out.
    clippy::print_stderr
)]

use std::sync::Arc;

use zerodds_security_crypto::AesGcmCryptoPlugin;
use zerodds_security_runtime::{ProtectionLevel, SharedSecurityGate};

/// What one topic name promises, per the suite's own naming comment.
#[derive(Debug, PartialEq, Eq)]
struct Expected {
    discovery: bool,
    read_ac: bool,
    write_ac: bool,
    metadata: ProtectionLevel,
    data: ProtectionLevel,
}

/// Splits a suite topic name into its four tokens. `SET_<name>*` is the
/// topic-EXPRESSION spelling of the same rule, and `PDRWAOMOD` is the one
/// name in the suite written without separators.
fn tokens(name: &str) -> Vec<String> {
    let core = name.trim_start_matches("SET_").trim_end_matches('*');
    if core == "PDRWAOMOD" {
        return ["PD", "RWA", "OM", "OD"]
            .iter()
            .map(|s| (*s).into())
            .collect();
    }
    core.split('_').map(str::to_string).collect()
}

fn expected_from_name(name: &str) -> Expected {
    let t = tokens(name);
    assert_eq!(t.len(), 4, "unparsable suite topic name: {name}");
    let (read_ac, write_ac) = match t[1].as_str() {
        "OA" => (false, false),
        "RA" => (true, false),
        "WA" => (false, true),
        "RWA" => (true, true),
        other => panic!("{name}: unknown access token {other}"),
    };
    let level = |tok: &str, open: &str, sign: &str, enc: &str| -> ProtectionLevel {
        if tok == open {
            ProtectionLevel::None
        } else if tok == sign {
            ProtectionLevel::Sign
        } else if tok == enc {
            ProtectionLevel::Encrypt
        } else {
            panic!("{name}: unknown token {tok}")
        }
    };
    Expected {
        discovery: match t[0].as_str() {
            "OD" => false,
            "PD" => true,
            other => panic!("{name}: unknown discovery token {other}"),
        },
        read_ac,
        write_ac,
        metadata: level(&t[2], "OM", "SM", "EM"),
        data: level(&t[3], "OD", "SD", "ED"),
    }
}

fn gate_for(gov_xml: &str) -> Arc<SharedSecurityGate> {
    Arc::new(SharedSecurityGate::new(
        0,
        zerodds_security_permissions::parse_governance_xml(gov_xml).expect("parse governance"),
        Box::new(AesGcmCryptoPlugin::new()),
    ))
}

fn actual(gate: &SharedSecurityGate, lookup: &str) -> Expected {
    Expected {
        discovery: gate.topic_discovery_protected(lookup).unwrap(),
        read_ac: gate.topic_read_protected(lookup).unwrap(),
        write_ac: gate.topic_write_protected(lookup).unwrap(),
        metadata: gate.metadata_protection(lookup).unwrap(),
        data: gate.data_protection(lookup).unwrap(),
    }
}

/// The name a reader/writer actually carries. For a `SET_<x>*` rule that is
/// any topic under the prefix, so the lookup also exercises the wildcard
/// path of `topic_match`.
fn lookup_name(expression: &str) -> String {
    if let Some(prefix) = expression.strip_suffix('*') {
        format!("{prefix}Square")
    } else {
        expression.to_string()
    }
}

fn suite_governance() -> Option<String> {
    let path = std::env::var("ZERODDS_OMG_GOVERNANCE").ok()?;
    Some(std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")))
}

fn topic_expressions(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<topic_expression>") {
        rest = &rest[i + "<topic_expression>".len()..];
        let end = rest.find("</topic_expression>").expect("closing tag");
        out.push(rest[..end].trim().to_string());
        rest = &rest[end..];
    }
    out
}

#[test]
fn every_suite_topic_resolves_to_what_its_name_says() {
    let Some(xml) = suite_governance() else {
        eprintln!(
            "SKIP omg_suite_topic_rules: set ZERODDS_OMG_GOVERNANCE to the interop suite's \
             TESTONLY_governance.xml (this test is the only check of the 27-rule table)"
        );
        return;
    };
    let expressions = topic_expressions(&xml);
    assert_eq!(
        expressions.len(),
        27,
        "the suite governance carries 27 <topic_rule> elements"
    );
    assert_eq!(
        expressions.first().map(String::as_str),
        Some("OD_OA_OM_OD"),
        "the all-open rule is FIRST — that is what makes topic_rules.first() wrong"
    );

    let gate = gate_for(&xml);
    let mut mismatches = Vec::new();
    for expr in &expressions {
        let lookup = lookup_name(expr);
        let want = expected_from_name(expr);
        let got = actual(&gate, &lookup);
        if want != got {
            mismatches.push(format!(
                "{expr} (looked up as {lookup}): want {want:?}, got {got:?}"
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} suite topics do not resolve to their own name:\n{}",
        mismatches.len(),
        expressions.len(),
        mismatches.join("\n")
    );

    // The counter-proof: the first rule is all-open, so a first()-lookup
    // would report exactly that for every one of the 27.
    let protected: Vec<&String> = expressions
        .iter()
        .filter(|e| expected_from_name(e).data != ProtectionLevel::None)
        .collect();
    assert_eq!(
        protected.len(),
        12,
        "12 of the 27 suite topics ask for SIGN or ENCRYPT on user data: {protected:?}"
    );
}
