// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every ZeroDDS language column that records its own octets records a
//! read verdict about them.
//!
//! `record_shape.rs::a_recorded_payload_has_a_read_verdict_about_that_payload`
//! asks this of the foreign sweeps, whose two directions come out of
//! `record.sh`. The four language columns record theirs through
//! `own-column-record.sh` and `own-read-sweep.sh`, which are two
//! commands, and nothing held the two files together.
//!
//! What that cost is measurable and was measured: on 2026-08-19 the C++,
//! C#, Java and TypeScript columns carried 3 760 cells each in the
//! ledger's read column and every one of them read `not-measured` —
//! 15 040 cells that write octets nothing ever read back. Two of the
//! four had no `--in` at all; the other two had one and no recorder
//! calling it.
//!
//! The check is one-directional, like the foreign one: a write row with
//! octets must have a read row that is not `unwritten`, because
//! `unwritten` is a claim of absence — "the write direction produced
//! nothing" — standing over a measurement that exists. A read row with
//! no write row is the other kind of stale and `committed_tree.rs`
//! catches it.
//!
//! ## Why the two directions have to be counted together
//!
//! `assemble.rs::own_cell` looks the **write** cell up first and answers
//! `not-measured` for **both** directions when there is none. So a
//! recorder that printed a refusal on the terminal and wrote no row cost
//! its column two cells, not one; the C++ column's source held 3 740
//! rows where the grid is 3 760, and the Java column's 3 450. Both
//! recorders now write a `refused` row with the probe's own first line,
//! which is what makes the read sweep able to answer `unwritten` for
//! them — a verdict about a reader that had nothing to read, which is a
//! different statement from "nobody ran this".
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::Path;

use zerodds_interop_results as results;

/// The columns whose two directions are two commands.
///
/// The C and the Python columns are not here: both their directions come
/// out of one run of `tools/wireprobe/three-column-gate --full`, so a
/// write row without a read row is not a state that run can leave.
const COLUMNS: [&str; 4] = ["cpp", "csharp", "java", "typescript"];

/// One source file, keyed by `<case>/<encoding>/<order>`, carrying the
/// status word and the detail.
fn rows(path: &Path) -> BTreeMap<String, (String, String)> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 6 {
            continue;
        }
        out.insert(
            format!("{}/{}/{}", fields[0], fields[2], fields[3]),
            (fields[4].to_string(), fields[5].to_string()),
        );
    }
    out
}

#[test]
fn every_language_column_records_both_directions() {
    let root = results::repository_root();
    let sources = results::results_root(&root).join("sources");
    let mut offenders: Vec<String> = Vec::new();
    for column in COLUMNS {
        let write = rows(&sources.join(format!("zerodds-{column}.tsv")));
        let read = rows(&sources.join(format!("zerodds-{column}-read.tsv")));
        assert!(
            !write.is_empty(),
            "tests/interop-results/sources/zerodds-{column}.tsv is empty or absent; \
             tools/interop-results/own-column-record.sh {column} writes it"
        );
        assert!(
            !read.is_empty(),
            "tests/interop-results/sources/zerodds-{column}-read.tsv is empty or absent; \
             tools/interop-results/own-read-sweep.sh --columns {column} writes it"
        );
        for (slot, (status, _)) in &write {
            match (
                status.as_str(),
                read.get(slot).map(|(word, _)| word.as_str()),
            ) {
                // A refusal in the write direction is a reader with
                // nothing to read, and `unwritten` is the word for it.
                ("refused", Some(_)) => {}
                ("wrote", Some("unwritten")) => offenders.push(format!(
                    "zerodds-{column} {slot}: the write record has octets and the read \
                     record says the write direction produced nothing"
                )),
                ("wrote", Some(_)) => {}
                (_, None) => offenders.push(format!(
                    "zerodds-{column} {slot}: a write row and no read row at all"
                )),
                (other, _) => offenders.push(format!(
                    "zerodds-{column} {slot}: `{other}` is not a write status"
                )),
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "{} cell(s) where a language column's two directions disagree about what was \
         measured; `tools/interop-results/own-read-sweep.sh` is what re-derives the read \
         half:\n{}",
        offenders.len(),
        offenders
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The grid is the corpus by every encoding by both orders, in both
/// directions, for every one of the four.
///
/// A count and not a floor. `systems.rs` carries a `bytes_floor` per
/// column, which is how many of the cells carry *octets*; this is how
/// many carry an answer at all, and it is the same number for every
/// column because it is the corpus times the slots. A column short of
/// it has cases nobody handed it, which is the shape both the C++ and
/// the Java sources were in.
#[test]
fn each_language_column_covers_the_whole_grid_in_both_directions() {
    let root = results::repository_root();
    let sources = results::results_root(&root).join("sources");
    let cases = results::corpus(&root).expect("the corpus");
    let expected = cases.len() * results::ENCODINGS.len() * results::ORDERS.len();
    let mut short: Vec<String> = Vec::new();
    for column in COLUMNS {
        for direction in ["", "-read"] {
            let file = format!("zerodds-{column}{direction}.tsv");
            let found = rows(&sources.join(&file)).len();
            if found != expected {
                short.push(format!(
                    "{file}: {found} cell(s) of {expected} — {} cases, {} encodings, \
                     {} byte orders",
                    cases.len(),
                    results::ENCODINGS.len(),
                    results::ORDERS.len()
                ));
            }
        }
    }
    assert!(
        short.is_empty(),
        "a language column does not cover the grid:\n{}",
        short.join("\n")
    );
}

/// No `different` and no `refused` read cell of a foreign column stands
/// without a cause.
///
/// Sixty-five cells read `different` with the comparison's own sentence
/// in them — "differs Sample.picked.whole: wrote -1234567, read 0" — and
/// nothing else. That says *what* came back and not *why*, and a
/// difference nobody has attributed is indistinguishable from one nobody
/// has looked at.
///
/// `assemble::attribute_reads` works the cause out per cell, from the
/// plan and from this project's own reader over the peer's own recorded
/// octets. Two answers are possible and both are a cause:
///
/// - the frame carries nothing to read, which is `mapping-divergence`
///   with the rule that made it so;
/// - the frame carries the sample and the peer's own reader did not
///   return it, which is [`assemble::READER_LOST_IT`] and a finding
///   about the peer.
///
/// A cell that reaches neither is a difference this tree cannot yet
/// account for, and it fails here rather than being committed. That is
/// the point: the failure is a question to answer, not a number to
/// carry.
#[test]
fn every_disagreeing_read_cell_names_its_cause() {
    use zerodds_interop_results::assemble::{self, Inputs, READER_LOST_IT};
    use zerodds_interop_results::ledger::Outcome;
    use zerodds_interop_results::systems::{Kind, SYSTEMS};

    let root = results::repository_root();
    let sources = results::results_root(&root).join("sources");
    let cases = results::corpus(&root).expect("the corpus");
    let inputs = Inputs::read(&sources).expect("the sources");
    let built = assemble::build(&cases, &inputs).expect("the ledger");

    let foreign: Vec<&str> = SYSTEMS
        .iter()
        .filter(|system| system.kind != Kind::Own)
        .map(|system| system.id)
        .collect();
    let mut bare: Vec<String> = Vec::new();
    for entry in &built.ledger.entries {
        if !foreign.contains(&entry.system) {
            continue;
        }
        let detail = match &entry.read {
            Outcome::Different(detail) | Outcome::Refused(detail) => detail,
            // A `mapping-divergence` carries its rule by construction,
            // and the verdicts that say nothing ran — `generator-defect`,
            // `harness-gap`, `no-form` — carry the probe's own line.
            _ => continue,
        };
        if detail.contains(READER_LOST_IT) {
            continue;
        }
        bare.push(format!(
            "{}/{}/{}/{}: {detail}",
            entry.case, entry.system, entry.encoding, entry.endian
        ));
    }
    assert!(
        bare.is_empty(),
        "{} read cell(s) disagree with the sample and name no cause; \
         `assemble::attribute_reads` is where a cause is worked out:\n{}",
        bare.len(),
        bare.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
    );
}
