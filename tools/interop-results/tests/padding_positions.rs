// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The padding classification, checked against the layout rather than
//! against the octets.
//!
//! Group G sets a row aside when the difference reaches no reader. The
//! rule that decided it was an observation about the two byte strings:
//! same length, and every differing octet one this project wrote as
//! `00`. That is not a statement that the position **is** a fill
//! position — `00` is what this project writes wherever it has nothing
//! to say, which includes a zero value, a length of zero and an absent
//! `@optional` member's declared length.
//!
//! The criterion here is computed the other way round: XTypes 1.3
//! §7.4.1's alignment rules decide which offsets carry fill, the
//! planner and `zerodds_wire::Writer::align_to` are where this project
//! implements them, and the writer notes every span it emits. A peer's
//! differing octet outside that set is a difference in a value however
//! it looks.
//!
//! The two criteria are independent and neither implies the other, so
//! their agreeing over the whole corpus is a measurement. This file is
//! where it is measured.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use zerodds_interop_results as results;
use zerodds_interop_results::assemble::{self, Inputs};
use zerodds_interop_results::ledger::RowIds;
use zerodds_interop_results::page::{self, PaddingVerdict};

fn verdicts() -> Vec<PaddingVerdict> {
    let root = results::repository_root();
    let results_root = results::results_root(&root);
    let cases = results::corpus(&root).expect("the corpus");
    let inputs = Inputs::read(&results_root.join("sources")).expect("the sources");
    let built = assemble::build(&cases, &inputs).expect("the ledger");
    let text = std::fs::read_to_string(results_root.join("row-ids.tsv")).unwrap_or_default();
    let mut ids = RowIds::read(&text).expect("the row ids");
    page::padding_audit(&built.ledger, &cases, &built.sweeps, &mut ids)
}

/// The three criteria answer alike on every row any of them reaches.
///
/// A disagreement is the finding this file exists for, and it is
/// printed in full: which row, which peer, and both byte strings. A row
/// the octet rule calls padding and the layout does not is a value
/// difference that has been counted as harmless.
#[test]
fn the_octet_rule_and_the_layout_agree_on_every_row() {
    let found = verdicts();
    let split: Vec<&PaddingVerdict> = found.iter().filter(|row| !row.agreed()).collect();
    assert!(
        split.is_empty(),
        "{} row(s) where the padding criteria disagree:\n{}",
        split.len(),
        split
            .iter()
            .map(|row| row.line())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The layout criterion is reached, and by the rows group G names.
///
/// A criterion nothing exercises is a criterion nobody has seen answer.
/// The count and the two peers are asserted rather than printed: if a
/// vendor recording changes them, this is the check that says so.
///
/// It said so. The count was 104 — RTI 50, omniORB 54 — until the fill
/// probes were changed to record what `tools/wire-capture` measured over
/// each peer's own transport. RTI's went to **zero**: its 50 rows were
/// the probe's own `static char BUFFER` carrying one `--matrix`
/// combination's octets into the next, and a cleared buffer records
/// RTI's RTPS layer, which writes `00`. omniORB's 54 became **200** and
/// TAO gained **96** it did not have: over GIOP both put the previous
/// request in the fill, so their probes carry a `0xAA` marker and every
/// row with a fill span now differs there — visibly, and at exactly the
/// offsets §7.4.1 puts fill at. TAO had no row here at all before,
/// because its recorded zeros were a fresh kernel page.
#[test]
fn the_layout_criterion_classifies_group_g_and_nothing_else() {
    let found = verdicts();
    let padding: Vec<&PaddingVerdict> = found.iter().filter(|row| row.layout_rule).collect();
    // 296 before the 411-case five-vendor record.
    //
    // 332 -> 341 with the seven cases that stood as not-measured. The
    // nine are the same rows `dialect_completeness`'s group-G assertion
    // counts — omniORB 224 -> 230 and TAO 108 -> 111, over
    // `annotation-external-{typedef,union-branch,union-branch-unannotated}`
    // — and 230 + 111 is this number. The two gates read the same rows
    // through different criteria and have to agree, which is why they
    // are asserted separately.
    //
    // 341 -> 371: OpenSplice's own 30, all `plain-cdr1/le` — the one
    // byte order and encoding version it writes — over alignment fill
    // `idlpp`'s C++ leaves unwritten. The same 30
    // `dialect_completeness`'s group-G assertion counts.
    assert_eq!(
        padding.len(),
        371,
        "group G is 371 rows; the layout criterion reached {}",
        padding.len()
    );
    let mut by_system: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for row in &padding {
        *by_system.entry(row.system).or_default() += 1;
    }
    // The same nine rows as the count above, attributed: omniORB
    // 224 -> 230 and TAO 108 -> 111 over
    // `annotation-external-{typedef,union-branch,union-branch-unannotated}`,
    // which is what the seven cases that stood as not-measured added.
    // Asserted a second time here because a count can hold while the
    // attribution moves.
    assert_eq!(
        by_system,
        [("omniorb", 230), ("opensplice", 30), ("tao", 111)]
            .into_iter()
            .collect(),
        "group G is omniORB 230, OpenSplice 30 and TAO 111"
    );
}
