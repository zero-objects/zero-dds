// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Does the reflective value change anything? The corpus answers.
//!
//! `zerodds_types::dynamic::DynamicData` is the second implementation of
//! `zerodds_wire_codec`'s two value traits. `serde_json::Value` is the
//! first, and the whole claim of the reflective half is that putting one
//! in place of the other changes no octet — the layout plan, the framing
//! and every dialect answer are the same code either way.
//!
//! So this measures it. For each of the 347 cases of
//! `tests/interop-corpus/`, in each of the four `{XCDR1, XCDR2} × {LE,
//! BE}` combinations:
//!
//! 1. the case's `values.json` is encoded through `serde_json::Value`;
//! 2. those bytes are decoded **twice**, once into a `serde_json::Value`
//!    and once into a `DynamicData`;
//! 3. both are encoded again;
//! 4. the two byte strings are compared, octet for octet — and where the
//!    JSON column reproduced the sample it started from, the
//!    `DynamicData` column has to reproduce it too;
//! 5. and where the type declares a `@key`, the sixteen-byte KeyHash is
//!    computed from both values and compared as well — the key holder is
//!    a second serialisation, under a second layout key, and a facade
//!    that got the payload right and the holder wrong would otherwise
//!    pass.
//!
//! ## Why both columns are decoded rather than only one
//!
//! Because one thing in the read direction is the representation's by
//! design, and it is not the octets. `zerodds_wire_codec::decode`
//! **sorts a map's entries** by `Source::render` — §7.4.1.1.4 says
//! nothing about entry order, `fastddsgen` maps a `map` onto a
//! `std::map` and so reorders one, and a canonical form that kept the
//! wire's order could not be compared across implementations. So a
//! decoded map is in the canonical order of whichever representation
//! decoded it, and re-encoding it need not reproduce the sample's own
//! entry order.
//!
//! Measured, and it is exactly the twenty-four slots it should be: the
//! six `map-key-*` cases in four modes each. Comparing a re-encode
//! against the *sample* would report that normalisation as a facade
//! defect; comparing the two re-encodes against each other reports what
//! this test is for.
//!
//! ## What this can and cannot catch
//!
//! It cannot catch a decoder that is wrong in the same way the encoder
//! is wrong: both sides are this tree's. What it does catch is the thing
//! it exists for — a value representation that loses, widens, reorders or
//! substitutes something on the way through. A `long long` that came back
//! as a double, a `char` that became a string, an `@optional` that came
//! back present, a `@mutable` member written in the order the reader
//! happened to meet it: each moves an octet, and each is named by case.
//!
//! The foreign half of the measurement is `zerodds_types::dynamic::to_ast`
//! and lives with the TypeLookup service; see that module's note.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use zerodds_ast::Ast;
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_types::dynamic::{DynamicData, DynamicType};
use zerodds_wire_codec::value::{Emit, ProbeRequest};
use zerodds_wire_codec::{ProbeError, decode, encode};

/// The cases this project refuses to encode at all, and why.
///
/// Named rather than skipped by a pattern, and asserted in both
/// directions below: a case that stops refusing has to show up as
/// loudly as one that starts.
///
/// Both are samples naming a character the type cannot hold.
/// §7.2.2.2.1.2.2 gives `char` eight bits and §7.2.2.2.1.2.5 holds
/// `wchar` to the Basic Multilingual Plane, so the value contract
/// refuses them before a byte is written. The cases exist to check that
/// it does.
///
/// # Two cases are not here that a neighbouring list still names
///
/// `crates/wire-probe/tests/decode_round_trip.rs::REFUSED` carries
/// `bitset-inheritance` and `bitset-inheritance-unannotated` as well, on
/// the reading that "the encoder will not pack a bitset that inherits
/// from another, because the planner sizes only the bitset's own
/// fields". Measured here, in all four modes: both encode, both decode,
/// and both re-encode to the byte string they started from —
/// `tests/interop-corpus/bitset-inheritance/README.md` records the same
/// `8dab` under each of the four runs. The gap is closed and that list
/// has not caught up; a case skipped by a list that no longer applies is
/// a case nobody measures.
// `recursive-optional-external` was here and left on 2026-08-26: the
// planner carries the case now, and the `DynamicData` walk carries it
// with it, writing the 28 octets Fast DDS records under XCDR1
// big-endian. What terminates a value of that type is the `@optional`
// member being absent, the same property a sequence length has.
// `recursive-array` stays: a cycle through an array has nothing to
// terminate it — the element count is fixed and non-zero — so no value
// of the type is finite. IDL 4.2 §7.4.1.4.4.4.4, and no measured
// implementation accepts the file either. The case exists so that the
// refusal stays a named one.
const REFUSED: [&str; 3] = [
    "primitive-char-above-latin1",
    "primitive-wchar-above-bmp",
    "recursive-array",
];

/// The cases whose re-encode is not the sample's own byte string.
///
/// A map, and only a map. `zerodds_wire_codec::decode` sorts a map's
/// entries into the canonical order named in the module note, so a
/// sample that declared its entries in some other order comes back in
/// that one. Both columns do it and both do it the same way — which is
/// what the comparison above measures — and neither reproduces the
/// sample.
///
/// The three `map-key-*` families and their `-unannotated` twins. The
/// other `map-*` cases in the corpus declare their entries in the
/// canonical order already, so their re-encode is the sample.
const NORMALISED_ENTRY_ORDER: [&str; 6] = [
    "map-key-boolean",
    "map-key-boolean-unannotated",
    "map-key-char",
    "map-key-char-unannotated",
    "map-key-long",
    "map-key-long-unannotated",
];

/// The four combinations the corpus runs in.
const MODES: [(Encoding, Endianness); 4] = [
    (Encoding::PlainCdr2, Endianness::Little),
    (Encoding::PlainCdr2, Endianness::Big),
    (Encoding::PlainCdr1, Endianness::Little),
    (Encoding::PlainCdr1, Endianness::Big),
];

struct Case {
    name: String,
    type_name: String,
    idl: String,
    values: Value,
}

fn repository_root() -> PathBuf {
    // `crates/types` → the tree root. Two levels and not a search for a
    // marker file: a search that walked past the root would find another
    // checkout's corpus and measure it instead.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the repository root, two levels above crates/types")
        .to_path_buf()
}

/// The corpus cases that never reach a model, because the IDL front end
/// refuses the file.
///
/// `resolve-module-named-as-type` is `module common { struct common
/// { … }; }`. OMG IDL 4.2 §7.5.2: "The name of a module, structure,
/// union, map, interface, value type, event type, exception or home may
/// not be redefined within the immediate scope of the module, …", with
/// `module M { typedef short M; }` printed as the clause's own
/// `// Error` example. The case is in the corpus deliberately — it
/// measures which vendors refuse the file and which resolve it anyway —
/// and `tools/idlc/tests/corpus.rs` carries it as an `Intended`
/// diagnostic for that reason.
///
/// It is not on `REFUSED` above: that list is about what the *encoder*
/// declines for a model it holds, and there is no model here to decline.
/// Removed before the walk starts, so the counts below are over the
/// cases that have one.
const NOT_LOWERED: [&str; 1] = ["resolve-module-named-as-type"];

fn corpus() -> Vec<Case> {
    let directory = repository_root().join("tests").join("interop-corpus");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .map(|entry| entry.expect("a corpus entry").path())
        .filter(|path| path.join("type").is_file())
        .collect();
    paths.sort();
    // Exact, so that a name that stops being in the corpus — or starts
    // lowering, and belongs back in the walk — is reported rather than
    // silently skipped.
    let mut dropped: Vec<String> = Vec::new();
    paths.retain(|path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        if NOT_LOWERED.contains(&name.as_str()) {
            dropped.push(name);
            return false;
        }
        true
    });
    assert_eq!(
        dropped,
        NOT_LOWERED.to_vec(),
        "the cases the front end refuses are not the cases named"
    );
    paths
        .into_iter()
        .map(|directory| {
            let name = directory
                .file_name()
                .and_then(|name| name.to_str())
                .expect("a case name")
                .to_string();
            let read = |file: &str| {
                std::fs::read_to_string(directory.join(file))
                    .unwrap_or_else(|error| panic!("{name}/{file}: {error}"))
            };
            let values_text = read("values.json");
            Case {
                type_name: read("type").trim().to_string(),
                idl: read("type.idl"),
                values: serde_json::from_str(&values_text)
                    .unwrap_or_else(|error| panic!("{name}/values.json: {error}")),
                name,
            }
        })
        .collect()
}

/// The model one case's IDL lowers to, resolved.
fn model(case: &Case) -> Ast {
    let parsed = zerodds_idl_parse::parse(&case.idl);
    assert!(
        !parsed.has_errors(),
        "{}: the IDL does not parse: {:?}",
        case.name,
        parsed.diagnostics()
    );
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), &format!("{}/type.idl", case.name));
    assert!(
        lowered.diagnostics.is_empty(),
        "{}: the IDL does not lower: {:?}",
        case.name,
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast
        .resolve_all_by_name()
        .unwrap_or_else(|error| panic!("{}: {error}", case.name));
    assert!(
        report.is_clean(),
        "{}: unresolved after the resolve pass: {:?}",
        case.name,
        report.broken
    );
    ast
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[test]
fn a_dynamic_data_writes_the_octets_a_json_value_writes() {
    let cases = corpus();
    assert!(!cases.is_empty(), "the corpus is empty");

    let mut compared = 0usize;
    let mut key_compared = 0usize;
    let mut refused: BTreeSet<String> = BTreeSet::new();
    let mut normalised: BTreeSet<String> = BTreeSet::new();
    let mut moved: Vec<String> = Vec::new();

    for case in &cases {
        let ast = model(case);
        for (encoding, endianness) in MODES {
            let request = ProbeRequest::new(&case.type_name, encoding, endianness);

            let json_bytes = match encode::<Value>(&ast, &request, &case.values) {
                Ok(bytes) => bytes,
                Err(_) => {
                    refused.insert(case.name.clone());
                    continue;
                }
            };

            // The same bytes, read back through both representations.
            let data: DynamicData = match decode(&ast, &request, &json_bytes) {
                Ok(value) => value,
                Err(error) => {
                    moved.push(format!(
                        "  {} {encoding:?}/{endianness:?}: the bytes do not read back into a \
                         DynamicData: {error}",
                        case.name
                    ));
                    continue;
                }
            };
            let back: Value = match decode(&ast, &request, &json_bytes) {
                Ok(value) => value,
                Err(error) => {
                    moved.push(format!(
                        "  {} {encoding:?}/{endianness:?}: the bytes do not read back into a \
                         serde_json::Value: {error}",
                        case.name
                    ));
                    continue;
                }
            };

            let json_again = encode::<Value>(&ast, &request, &back);
            let data_again = encode::<DynamicData>(&ast, &request, &data);
            match (json_again, data_again) {
                (Ok(left), Ok(right)) if left == right => {
                    compared += 1;
                    // Where the JSON column reproduced the sample it
                    // started from, so must this one. Where it did not —
                    // a map, whose entry order the decoder normalises —
                    // neither is asked to.
                    if left == json_bytes {
                        assert_eq!(
                            right, json_bytes,
                            "{} {encoding:?}/{endianness:?}: the JSON column reproduced the \
                             sample and the DynamicData column did not",
                            case.name
                        );
                    } else {
                        normalised.insert(case.name.clone());
                    }
                }
                (Ok(left), Ok(right)) => moved.push(format!(
                    "  {} {encoding:?}/{endianness:?}: json wrote {}, the DynamicData wrote {}",
                    case.name,
                    hex(&left),
                    hex(&right)
                )),
                (left, right) => moved.push(format!(
                    "  {} {encoding:?}/{endianness:?}: json re-encoded {left:?}, the \
                     DynamicData re-encoded {right:?}",
                    case.name
                )),
            }

            // The KeyHash is a second serialisation of the same value,
            // under §7.6.8's own layout key — big-endian, key purpose,
            // the key holder's members only. A facade that got the
            // payload right and the holder wrong would pass everything
            // above.
            let key_request = request.clone().emitting(Emit::KeyHash);
            let json_key = encode::<Value>(&ast, &key_request, &back);
            let data_key = encode::<DynamicData>(&ast, &key_request, &data);
            match (json_key, data_key) {
                (Ok(left), Ok(right)) if left == right => key_compared += 1,
                (Ok(left), Ok(right)) => moved.push(format!(
                    "  {} {encoding:?}/{endianness:?}: KeyHash json {} vs DynamicData {}",
                    case.name,
                    hex(&left),
                    hex(&right)
                )),
                // A keyless type has one instance and no KeyHash —
                // `ProbeError::NoKey` — and both sides must say so. Any
                // other pairing is a divergence.
                (Err(ProbeError::NoKey { .. }), Err(ProbeError::NoKey { .. })) => {}
                (left, right) => moved.push(format!(
                    "  {} {encoding:?}/{endianness:?}: KeyHash json {left:?} vs DynamicData \
                     {right:?}",
                    case.name
                )),
            }
        }
    }

    assert!(
        moved.is_empty(),
        "the DynamicData column differs from the JSON column in {} places:\n{}",
        moved.len(),
        moved.join("\n")
    );

    // The refusals are exact in both directions. A case that stops
    // refusing has to be seen as loudly as one that starts: the first is
    // a gap that closed and the list is where that is recorded.
    let expected: BTreeSet<String> = REFUSED.iter().map(|name| (*name).to_string()).collect();
    assert_eq!(
        refused, expected,
        "the set of cases the encoder refuses moved"
    );

    // And the walk actually ran. A filter that matched nothing would
    // otherwise pass every assertion above with an empty loop.
    let payloads = (cases.len() - REFUSED.len()) * MODES.len();
    assert_eq!(
        compared,
        payloads,
        "{compared} payload comparisons over {} cases in {} modes, and {payloads} were due",
        cases.len(),
        MODES.len()
    );
    assert!(
        key_compared > 0,
        "no case in the corpus produced a KeyHash, which cannot be right"
    );
    // Recorded so that the number this rests on is visible rather than
    // implied: 64 KeyHash comparisons — sixteen keyed cases in four
    // modes — beside the 1380 payload comparisons above. A floor and not
    // an equality, so that a corpus case added with a `@key` does not
    // turn this red; a *drop* is the walk quietly doing less.
    assert!(
        key_compared >= 64,
        "{key_compared} KeyHash comparisons, and 64 were made when this was written"
    );

    // The one place the two columns are allowed to write something other
    // than the sample they started from, and it is the decoder's map
    // normalisation rather than either representation. Exact, so that a
    // case that starts normalising — or stops — is named.
    let expected_normalised: BTreeSet<String> = NORMALISED_ENTRY_ORDER
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    assert_eq!(
        normalised, expected_normalised,
        "the set of cases whose re-encode differs from the sample moved"
    );
}

/// The same octets, reached from a `DynamicType` rather than from a name.
///
/// `encode` takes the type by scoped name out of the model; a caller
/// holding a `DynamicType` has the node already. This asserts the two
/// roads reach the same node — and so the same bytes — because a view
/// that pointed at a different node of the same name would be invisible
/// everywhere else.
#[test]
fn a_dynamic_type_names_the_node_the_encoder_encodes() {
    let cases = corpus();
    let mut seen = 0usize;
    for case in &cases {
        let ast = Arc::new(model(case));
        let type_ = DynamicType::find(&ast, &case.type_name)
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert_eq!(
            type_.qualified_name(),
            case.type_name.trim_start_matches("::"),
            "{}: the view names a different type",
            case.name
        );
        type_
            .is_consistent()
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
        seen += 1;
    }
    assert_eq!(seen, cases.len());
}

/// The case the walk drops is dropped for the reason `NOT_LOWERED`
/// gives, and for no other.
///
/// Without this the list would be an assertion about the corpus that
/// nothing measures: a case could start lowering — the §7.5.2 rule
/// withdrawn, the fixture rewritten — and the walk would go on skipping
/// it. Here the refusal itself is the measurement, and its code is named.
#[test]
fn every_dropped_case_is_one_the_front_end_refuses() {
    for name in NOT_LOWERED {
        let path = repository_root()
            .join("tests")
            .join("interop-corpus")
            .join(name)
            .join("type.idl");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let parsed = zerodds_idl_parse::parse(&source);
        assert!(
            !parsed.has_errors(),
            "{name}: the file parses; the refusal is a lowering one"
        );
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), &format!("{name}/type.idl"));
        assert!(
            lowered
                .diagnostics
                .iter()
                .any(|d| d.code == zerodds_idl_parse::DiagnosticCode::DuplicateName),
            "{name}: the front end no longer refuses this file, so it belongs back in the \
             walk: {:?}",
            lowered.diagnostics
        );
    }
}
