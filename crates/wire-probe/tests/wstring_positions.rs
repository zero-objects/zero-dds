// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! **Counter-check.** Nine implementations answer C1, C2, C3 and C4 the
//! same at a struct member and at a collection element.
//!
//! This is a *checked non-existence*, and it is the one a rebuild
//! destroys most easily: a position argument threaded through an axis
//! makes it cheap to give the two positions different answers by
//! accident, and nothing in a byte comparison would say so — the corpus
//! rows would simply move from matching to not matching, in a column
//! nobody was looking at.
//!
//! Measured 2026-08-12 by reconstruction against
//! `tests/interop-results/sources/`: for each implementation, each of
//! the eight cells of C1 × C2 × C3 crossed with each of the four points
//! of C4 was encoded and compared to the recorded octets. Exactly one
//! combination reproduces each row, and the combination that reproduces
//! the member row is the one that reproduces the element row.
//!
//! | implementation | C1 | C2 | C3 | C4 |
//! |---|---|---|--:|---|
//! | Cyclone | octets | absent | 2 | host-unswapped |
//! | OpenDDS | octets | absent | 2 | stream |
//! | Fast DDS | characters | absent | 2 | stream |
//! | RTI, XCDR1 | characters | present | 4 | stream |
//! | ros-cyclone | characters | absent | 4 | stream |
//! | ros-fastrtps | characters | absent | 4 | host-unswapped |
//! | omniORB | octets | absent | 2 | byte-order-mark |
//! | TAO | octets | absent | 4 | stream |
//! | JacORB | octets | absent | 2 | big-endian-always |
//!
//! The test does not hard-code that table. It derives each
//! implementation's answer from the recorded member row and then
//! requires the *same* answer to reproduce the recorded element row —
//! so an implementation that started parting the two positions fails
//! here rather than being absorbed into a new coordinate.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::json;
use zerodds_ast_layout::{Encoding, Endianness, WstringLengthPolicy, WstringUnitOrder};
use zerodds_wire_probe::encode::{encode, hex, model_from_idl};
use zerodds_wire_probe::value::ProbeRequest;

/// The corpus case a member's answer is read from, and its value.
const MEMBER_CASE: &str = "wstring-unbounded-utf8";
/// The corpus case an element's answer is read from, and its values.
const ELEMENT_CASE: &str = "array-1d-wstring";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Every recorded `wrote` row, keyed by (case, system, version, order).
///
/// Both vendor corpora, because the ROS type supports are in the second
/// one and they are two of the nine. A row that is not `wrote` — a
/// refusal, a version alias — is left out rather than parsed as octets:
/// `corba-corpus.tsv` keeps refusals in the same column behind an
/// `ERR1:` prefix, and a reader that treats one as hex reports a
/// divergence that never happened.
fn recorded() -> BTreeMap<(String, String, String, String), String> {
    let mut rows = BTreeMap::new();
    let sources = repository_root().join("tests/interop-results/sources");
    for (file, corba) in [
        ("vendor-corpus.tsv", false),
        ("ros-corpus.tsv", false),
        ("corba-corpus.tsv", true),
    ] {
        let Ok(text) = std::fs::read_to_string(sources.join(file)) else {
            continue;
        };
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if corba {
                // case, system, endian, direction, payload — one
                // version only, because CORBA CDR *is* version 1.
                if f.len() < 5 || f[3] != "write" || f[4].starts_with("ERR") {
                    continue;
                }
                rows.insert(
                    (
                        f[0].to_string(),
                        f[1].to_string(),
                        "xcdr1".to_string(),
                        f[2].to_string(),
                    ),
                    f[4].to_string(),
                );
            } else {
                // case, system, version, endian, outcome, payload
                if f.len() < 6 || f[4] != "wrote" {
                    continue;
                }
                rows.insert(
                    (
                        f[0].to_string(),
                        f[1].to_string(),
                        f[2].to_string(),
                        f[3].to_string(),
                    ),
                    f[5].to_string(),
                );
            }
        }
    }
    rows
}

/// Every combination of the two `wstring` axes: eight cells × four
/// orders.
fn combinations() -> Vec<(WstringLengthPolicy, WstringUnitOrder)> {
    let mut out = Vec::new();
    for cell in WstringLengthPolicy::ALL_CELLS {
        for order in WstringUnitOrder::ALL {
            out.push((cell, order));
        }
    }
    out
}

/// What this project writes for one corpus case under one combination.
fn written(
    case: &str,
    values: &serde_json::Value,
    endianness: Endianness,
    cell: WstringLengthPolicy,
    order: WstringUnitOrder,
) -> Option<String> {
    let directory = repository_root().join("tests/interop-corpus").join(case);
    let idl = std::fs::read_to_string(directory.join("type.idl")).ok()?;
    let ast = model_from_idl(&idl, &format!("{case}/type.idl")).ok()?;
    let mut request = ProbeRequest::new("Sample", Encoding::PlainCdr1, endianness);
    request.wstring_length = cell;
    request.wstring_unit_order = order;
    encode(&ast, &request, values).ok().map(|bytes| hex(&bytes))
}

/// Whether two byte strings agree on every octet the peer determined.
///
/// `tools/wireprobe/omniorb-template.cpp` and `tao-template.cpp` write
/// `0xAA` at the alignment fill on purpose: measured over GIOP, both
/// ORBs put the *previous request's* octets there
/// (`tools/wire-capture`), so the fill is not a function of the sample
/// and no buffer content records it. The marker names those positions
/// instead of hiding them under a zero.
///
/// So a comparison against a recorded ORB row is a comparison over the
/// octets the ORB decided, and an `aa` where this project wrote `00` is
/// the marker rather than a disagreement. It forgives nothing else: the
/// lengths have to match, and an `aa` where this project wrote a value
/// is a difference in that value.
fn agrees_where_the_peer_decided(mine: &str, theirs: &str) -> bool {
    if mine.len() != theirs.len() {
        return false;
    }
    mine.as_bytes()
        .chunks_exact(2)
        .zip(theirs.as_bytes().chunks_exact(2))
        .all(|(a, b)| a == b || (a == b"00" && b == b"aa"))
}

/// Which combinations reproduce a recorded row, over the octets the
/// peer decided — see [`agrees_where_the_peer_decided`].
fn combinations_matching(
    case: &str,
    values: &serde_json::Value,
    endian: &str,
    theirs: &str,
) -> Vec<(WstringLengthPolicy, WstringUnitOrder)> {
    let endianness = if endian == "be" {
        Endianness::Big
    } else {
        Endianness::Little
    };
    combinations()
        .into_iter()
        .filter(|(cell, order)| {
            written(case, values, endianness, *cell, *order)
                .is_some_and(|mine| agrees_where_the_peer_decided(&mine, theirs))
        })
        .collect()
}

/// The nine implementations, under the names the corpora file them by.
const IMPLEMENTATIONS: [&str; 9] = [
    "cyclone",
    "opendds",
    "fastdds",
    "rti",
    "ros-cyclone",
    "ros-fastrtps",
    "omniorb",
    "tao",
    "jacorb",
];

#[test]
fn the_two_positions_still_agree() {
    let rows = recorded();
    assert!(
        !rows.is_empty(),
        "no recorded rows were read from tests/interop-results/sources — this check cannot \
         pass by finding nothing, because finding nothing is what it is here to notice"
    );
    let member_values = json!({ "text": "äöü ሴ Z", "tail": -1 });
    let element_values = json!({ "items": ["", "äZ", "ሴ"], "tail": -1 });

    let mut checked = 0usize;
    let mut compared = Vec::new();
    for system in IMPLEMENTATIONS {
        for endian in ["le", "be"] {
            let member_key = (
                MEMBER_CASE.to_string(),
                system.to_string(),
                "xcdr1".to_string(),
                endian.to_string(),
            );
            let element_key = (
                ELEMENT_CASE.to_string(),
                system.to_string(),
                "xcdr1".to_string(),
                endian.to_string(),
            );
            let (Some(member), Some(element)) = (rows.get(&member_key), rows.get(&element_key))
            else {
                // An implementation that wrote only one of the two, or
                // only one byte order, is not evidence either way. The
                // count below is what stops the whole test from passing
                // on absence.
                continue;
            };

            let at_member = combinations_matching(MEMBER_CASE, &member_values, endian, member);
            assert!(
                !at_member.is_empty(),
                "{system}/{endian}: no cell of C1 x C2 x C3 crossed with any of the four \
                 orders reproduces the recorded member row {member}. A form is on the wire \
                 that the product cannot express, which is the defect the product replaced \
                 an enumeration to prevent"
            );
            let at_element = combinations_matching(ELEMENT_CASE, &element_values, endian, element);
            assert!(
                !at_element.is_empty(),
                "{system}/{endian}: nothing reproduces the recorded element row {element}"
            );

            let shared: Vec<_> = at_member
                .iter()
                .filter(|combination| at_element.contains(combination))
                .collect();
            assert!(
                !shared.is_empty(),
                "{system}/{endian}: the member position answers {} and the element \
                 position answers {}, and no combination answers both. The two positions \
                 have parted on C1/C2/C3/C4, which nine implementations did not do when \
                 this was measured — record the split on the axis rather than deleting \
                 this check",
                describe(&at_member),
                describe(&at_element)
            );
            compared.push(format!(
                "{system}/{endian} {}",
                describe(&shared.into_iter().copied().collect::<Vec<_>>())
            ));
            checked += 1;
        }
    }

    // The guard against passing on absence, which is the failure this
    // whole file is written against: a test that compares nothing
    // passes, and looks exactly like a test that compared everything.
    //
    // Fifteen is the measured count, 2026-08-12 — all nine
    // implementations, both byte orders for six of them and one order
    // for three (`ros-cyclone` refuses a byte order it is not given,
    // `tao` and `jacorb` write the host's). More is fine; fewer means
    // the corpus has stopped carrying what this reads.
    assert!(
        checked >= 15,
        "only {checked} (implementation, byte order) pairs had both a member row and an \
         element row to compare — the corpus has stopped carrying what this check reads, \
         so it is no longer checking anything. Compared: {compared:?}"
    );
}

fn describe(combinations: &[(WstringLengthPolicy, WstringUnitOrder)]) -> String {
    combinations
        .iter()
        .map(|(cell, order)| format!("{}+{}", cell.as_str(), order.as_str()))
        .collect::<Vec<_>>()
        .join(" or ")
}
