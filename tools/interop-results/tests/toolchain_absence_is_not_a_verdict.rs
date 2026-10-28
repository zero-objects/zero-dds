// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A cell is never a sentence about the machine that took it.
//!
//! `docs/specs/zerodds-verdicts-1.0.md` gives two exit codes no verdict
//! at all. 2 is the command line and 9 is `toolchain-absent` — "the host
//! has not got the column's compiler" — and of both the table says the
//! same thing: **not a measurement**, the collection stops. A run that
//! cannot measure a column names the missing compiler and ends; it does
//! not fill the matrix with rows about the host.
//!
//! Nothing enforced that. The probes answered 9 and every collection
//! script read it through a `*)` arm as `refused`, which is a verdict
//! that says the toolchain was asked and said no. The spec's own
//! "Why 9 exists" counts what that came to: `zerodds-nim` wrote 17 760
//! cells saying `nim is not on the path`, `zerodds-swift` the same
//! number for `swiftc`, `zerodds-julia` and `zerodds-zig` the same
//! again, each found by accident on a cell someone happened to open.
//! Two more instances were still in the committed tree when this test
//! was written: 60 rows of `zerodds-csharp-read.tsv` and 10 of
//! `zerodds-csharp.tsv`, all of them ``no `dotnet` on PATH``.
//!
//! ## What is checked, and why it is not the message
//!
//! Not the words. A message is written by whoever wrote the probe, and
//! the next toolchain phrases its absence differently; a test that
//! matched `on PATH` would be a rule about one sentence and would pass
//! the moment someone wrote a better one.
//!
//! The structural feature is the **exit code**, which the six-column
//! sources already carry: `own-column-record.sh` writes a refusal as
//! `(<code>) <the probe's own first line>`, and the code is the probe's
//! and not the shell's reading of its message. So a cell that came out
//! of a tool failure is one whose refusal names code 2 or 9, and that is
//! what [`no_committed_cell_is_a_toolchain_failure`] rejects — for every
//! code the spec calls "not a measurement", not for the one that was
//! noticed.
//!
//! The second test is about the place a cell like that is made.
//! [`every_collection_script_has_an_arm_for_an_absent_toolchain`] holds
//! each script that turns a probe's exit code into a recorded row to
//! having an arm for 9, because a code the spec carries and a `case`
//! does not is precisely how the sixty rows got written.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::Path;

use zerodds_interop_results as results;

/// The codes `docs/specs/zerodds-verdicts-1.0.md` gives no verdict.
///
/// 2 is the command line — a probe that was called wrong measured
/// nothing — and 9 is `toolchain-absent`. Both are marked "not a
/// measurement, the collection stops" in the exit table, and neither may
/// stand as a cell.
const NOT_A_MEASUREMENT: [&str; 2] = ["2", "9"];

/// The exit code a refusal names, when it names one.
///
/// The shape is `own-column-record.sh`'s: `(<digits>) <reason>` at the
/// head of the detail column. A refusal with no code in front of it came
/// from a sweep that does not record one and is not this test's subject.
fn exit_code(detail: &str) -> Option<&str> {
    let rest = detail.strip_prefix('(')?;
    let (code, _) = rest.split_once(')')?;
    if code.is_empty() || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(code)
}

/// No committed cell is a statement about the machine that recorded it.
#[test]
fn no_committed_cell_is_a_toolchain_failure() {
    let root = results::repository_root();
    let sources = results::results_root(&root).join("sources");
    let mut offenders: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(&sources).expect("tests/interop-results/sources");
    let mut paths: Vec<_> = entries
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|kind| kind == "tsv"))
        .collect();
    paths.sort();
    for path in paths {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (index, line) in text.lines().enumerate() {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() != 6 {
                continue;
            }
            let Some(code) = exit_code(fields[5]) else {
                continue;
            };
            if !NOT_A_MEASUREMENT.contains(&code) {
                continue;
            }
            offenders.push(format!(
                "{}:{}: {} {} {}/{} carries exit {code}, which is not a measurement — `{}`",
                path.file_name().unwrap_or_default().to_string_lossy(),
                index + 1,
                fields[0],
                fields[1],
                fields[2],
                fields[3],
                fields[5].chars().take(90).collect::<String>(),
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "{} cell(s) record a run that measured nothing — exit 2 is a probe called wrong and \
         exit 9 is a host without the column's compiler, and `docs/specs/zerodds-verdicts-1.0.md` \
         gives neither a verdict. Re-record the column on a host that has the toolchain; the \
         collection scripts no longer write these:\n{}",
        offenders.len(),
        offenders
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Every script that turns a probe's exit code into a row, and the arm
/// each of them must have.
///
/// `warning-sweep.sh` is not here and cannot be: it records what a front
/// end *said*, with the front end's exit status as data beside the line,
/// and it never maps a status onto a verdict. `endpoint-sibling.sh` is
/// not here either — it compares two files of octets and runs no probe.
const SCRIPTS: [&str; 6] = [
    "tools/interop-results/read-corpus-sweep.sh",
    "tools/interop-results/own-column-record.sh",
    "tools/interop-results/vendor-corpus-sweep.sh",
    "tools/interop-results/ros-corpus-sweep.sh",
    "tools/wireprobe/wireprobe-endpoint-corpus",
    "tools/wireprobe/collect-generated",
];

/// A code the spec carries and a `case` does not is where the sixty rows
/// came from.
#[test]
fn every_collection_script_has_an_arm_for_an_absent_toolchain() {
    let root = results::repository_root();
    let mut offenders: Vec<String> = Vec::new();
    for script in SCRIPTS {
        let path = root.join(script);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        if !has_arm_for_nine(&text) {
            offenders.push(format!(
                "{script}: no `case` arm for exit 9, so `toolchain-absent` reaches whatever \
                 arm catches the rest and is recorded as a verdict"
            ));
        }
        if !text.contains("toolchain-absent") {
            offenders.push(format!(
                "{script}: does not name `toolchain-absent`, so a reader of this file cannot \
                 tell 9 apart from the codes it does list"
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "a collection script has no answer for a probe that says the toolchain is not here; \
         `docs/specs/zerodds-verdicts-1.0.md` §\"Why 9 exists\" is what it has to do \
         instead:\n{}",
        offenders.join("\n")
    );
}

/// Whether a shell script has a `case` arm that matches exit 9.
///
/// `9)` on its own or as one alternative of a pattern list — `2|9)` is
/// what four of these six write, because the spec stops on both codes
/// for the same reason.
fn has_arm_for_nine(text: &str) -> bool {
    text.lines().map(str::trim).any(|line| {
        let Some(pattern) = line.split(')').next() else {
            return false;
        };
        // `9)` and `2|9)` are arms; `$((offset * 2))` and `head -9)` are
        // not, so the whole of what precedes the parenthesis has to be a
        // pattern list of digits.
        !pattern.is_empty()
            && pattern.split('|').all(|alternative| {
                !alternative.is_empty() && alternative.bytes().all(|b| b.is_ascii_digit())
            })
            && pattern.split('|').any(|alternative| alternative == "9")
    })
}

/// The check is over what is committed, so it needs the sources to be
/// there at all: an empty directory would pass every assertion above.
#[test]
fn the_sources_this_reads_are_there() {
    let root = results::repository_root();
    let sources = results::results_root(&root).join("sources");
    let count = std::fs::read_dir(&sources)
        .expect("tests/interop-results/sources")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|kind| kind == "tsv"))
        .count();
    assert!(
        count >= 10,
        "{} source file(s) under {}; the guard above would pass over an empty directory",
        count,
        Path::new(&sources).display()
    );
}
