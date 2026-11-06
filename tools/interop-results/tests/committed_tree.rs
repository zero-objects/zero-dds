// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! What the committed tree and a run of it must say.
//!
//! The tables are a rendering of `ledger.tsv`; `ledger.tsv` is built from
//! the source TSVs and from a fresh run of this project's own columns;
//! and the `.bin` files are that ledger's bytes. That the committed tree,
//! the ledger, the two tables, `SYSTEMS.md`, `WARNINGS.md`, `REFUSALS.md`
//! and the page are what a run writes is checked by
//! `interop-results --verify` (the weekly job `interop-results`), once,
//! and not again here. These tests hold the invariants the comparison
//! cannot see: a cell with no word, a column that ran and carries no
//! bytes, a cell with two answers, a link to nothing, a machine's name on
//! the page.
//!
//! What this does **not** do is re-run the foreign systems. It cannot:
//! RTI is licensed, the ORBs are hand-built and the ROS type supports
//! need a ROS installation. Their bytes are re-derived by
//! `tools/interop-results/vendor-corpus-sweep.sh` and its two siblings on
//! a machine that has them.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;

use zerodds_interop_results as results;
use zerodds_interop_results::assemble::{self, Inputs};
use zerodds_interop_results::ledger::{Outcome, RowIds};
use zerodds_interop_results::page;
use zerodds_interop_results::systems::SYSTEMS;
use zerodds_interop_results::tables::{self, Direction};
use zerodds_interop_results::warnings;

fn built() -> (Vec<results::Case>, assemble::Built, PathBuf) {
    let root = results::repository_root();
    let results_root = results::results_root(&root);
    let cases = results::corpus(&root).expect("the corpus");
    let inputs = Inputs::read(&results_root.join("sources")).expect("the sources");
    let built = assemble::build(&cases, &inputs).expect("the ledger");
    (cases, built, results_root)
}

/// The page names nothing that belongs to the machines it was measured
/// on.
///
/// It is written to be published and pasted into a reply on a public
/// tracker. A test host's name, an absolute path on it or an internal
/// URL reaching a public page has had to be scrubbed from public
/// material before, and a scrub is a thing that gets forgotten once.
#[test]
fn the_page_names_no_private_infrastructure() {
    let results_root = results::results_root(&results::repository_root());
    let html = std::fs::read_to_string(results_root.join(page::FILE))
        .unwrap_or_else(|error| panic!("{}: {error}; run --page", page::FILE));
    let lower = html.to_lowercase();
    for marker in [
        "codepit",
        "gitlab.",
        "/root/",
        ".internal",
        "glr1",
        "glr2",
        "amstk",
    ] {
        assert!(
            !lower.contains(marker),
            "{} carries `{marker}`, which belongs to the machines this was measured on and not \
             to a page anybody may read",
            page::FILE
        );
    }
}

/// No recorded file names the machine it was measured on.
///
/// The page test above is downstream of this one: `matrix.html` carried
/// `/root/…` because `sources/` did, and rendering it out of the page
/// would have left the record itself a function of the host. It was
/// worse than that — `wireprobe-c-matrix` names its build directory
/// after [`std::process::id`], so five committed rows carried a process
/// id and a **second run on the same host** would have written five
/// different rows. `tools/interop-results/verify-record.sh` diffs a
/// re-record against `sources/` byte for byte, so that is not a
/// cosmetic difference: it is a re-record reporting rows as moved that
/// nobody moved.
///
/// `sources::without_the_machine` is the rule and `--normalise` writes
/// it over the record; this asks whether the committed files are what
/// it would write.
#[test]
fn the_record_carries_no_machine() {
    let results_root = results::results_root(&results::repository_root());
    let mut carried: Vec<String> = Vec::new();
    let mut files = vec![results_root.join("wire-goldens.tsv")];
    let sources = results_root.join("sources");
    for directory in [sources.clone(), sources.join("toolchain-logs")] {
        collect(&directory, &mut files);
    }
    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let clean = zerodds_interop_results::sources::without_the_machine(&text);
        if clean == text {
            continue;
        }
        for (number, (before, after)) in text.lines().zip(clean.lines()).enumerate() {
            if before != after {
                carried.push(format!(
                    "{}:{}: {}",
                    path.strip_prefix(&results_root).unwrap_or(path).display(),
                    number + 1,
                    before.chars().take(160).collect::<String>()
                ));
            }
        }
    }
    let shown = carried.len();
    carried.truncate(20);
    assert!(
        carried.is_empty(),
        "{shown} recorded line(s) name the checkout, the temporary directory or the process a \
         sweep happened to run in. A record that is not a function of the case cannot be \
         re-recorded and compared; run `cargo run -p zerodds-interop-results -- --normalise`:\n{}",
        carried.join("\n")
    );
}

/// Every file under `directory`, one level and then the next.
fn collect(directory: &std::path::Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, into);
        } else if path
            .extension()
            .is_some_and(|found| found == "tsv" || found == "log")
        {
            into.push(path);
        }
    }
}

/// Every warning a front end printed has a meaning written for it.
///
/// The counts in `WARNINGS.md` are generated and the meanings are not,
/// because a meaning cannot be generated. What can be checked is that
/// none is missing: a front end that starts saying something new must
/// stop this test rather than land in a table as a row with no
/// sentence under it. That is the whole difference between capturing
/// warnings and reading them.
#[test]
fn every_warning_text_is_classified() {
    let results_root = results::results_root(&results::repository_root());
    // Both sources. The compilers' file was outside this gate when it
    // arrived, which meant the pipeline had the lines and nothing
    // required anybody to say what they meant — the same shape as not
    // having them, one step further along.
    let sources = results_root.join("sources");
    let diagnostics = warnings::read_both(
        &sources.join("warnings.tsv"),
        &sources.join("toolchain-warnings.tsv"),
    )
    .expect("the two diagnostic sources");
    let mut unread: Vec<String> = Vec::new();
    for (pair, lines) in &diagnostics {
        for line in lines {
            if line.severity != "warning" && line.severity != "unclassified" {
                continue;
            }
            if warnings::meaning(&line.text).is_none() {
                unread.push(format!("{pair}: {}", line.text));
            }
        }
    }
    unread.sort();
    unread.dedup();
    assert!(
        unread.is_empty(),
        "a front end or a compiler said something tools/interop-results/src/warnings.rs has \
         no entry for; a diagnostic is a statement and this is the one place it can be read. A \
         compiler's belongs in `COMPILER_CLASSIFICATION` under the switch it names, a front \
         end's in `CLASSIFICATION` under a substring of its own sentence:\n{}",
        unread.join("\n")
    );
}

/// A row number, once given, is never given to another row.
///
/// The registry is append-only for one reason: a row cited in a
/// document, a commit message or an issue has to stay the row it was.
/// A generated table that renumbered on every corpus addition would
/// make every citation of it wrong within a day.
#[test]
fn a_row_keeps_its_number() {
    let results_root = results::results_root(&results::repository_root());
    let text = std::fs::read_to_string(results_root.join("row-ids.tsv")).expect("the registry");
    let ids = RowIds::read(&text).expect("the registry parses");
    let mut seen: Vec<u32> = ids.by_key.values().copied().collect();
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(before, seen.len(), "two keys share a row number");
}

/// No cell is a blank.
///
/// A blank reads as agreement, and most of the holes in a
/// cross-implementation matrix are not agreements. Every one of the
/// twelve columns answers every row with one of the eleven words, and
/// every word links to a file.
#[test]
#[ignore = "builds the results from the whole corpus, 22 to 75 s in the test profile; runs in the weekly job interop-results under the cargo profile corpus"]
fn every_cell_says_something() {
    let (cases, built, _) = built();
    let expected = cases.len() * results::ENCODINGS.len() * results::ORDERS.len() * SYSTEMS.len();
    assert_eq!(
        built.ledger.entries.len(),
        expected,
        "the ledger has {} lines and the matrix is {expected} cells",
        built.ledger.entries.len()
    );
    for entry in &built.ledger.entries {
        assert!(
            !entry.write.token().is_empty() && !entry.read.token().is_empty(),
            "{}/{}/{}/{} has a cell with no word",
            entry.case,
            entry.encoding,
            entry.endian,
            entry.system
        );
    }
}

/// Every link in both tables resolves to a file that exists.
///
/// The tables are three and a half megabytes of markdown each and no
/// human will click ninety thousand links. This does.
#[test]
fn every_link_resolves() {
    let results_root = results::results_root(&results::repository_root());
    let mut checked = 0_usize;
    let mut missing: Vec<String> = Vec::new();
    for direction in [Direction::Write, Direction::Read] {
        let text = std::fs::read_to_string(results_root.join(tables::file(direction)))
            .expect("a committed table");
        for line in text.lines() {
            if !line.starts_with("| ") {
                continue;
            }
            let mut rest = line;
            while let Some(open) = rest.find("](") {
                let after = &rest[open + 2..];
                let Some(close) = after.find(')') else { break };
                let target = &after[..close];
                checked += 1;
                if !results_root.join(target).exists() && missing.len() < 10 {
                    missing.push(format!("{}: {target}", tables::file(direction)));
                }
                rest = &after[close..];
            }
        }
    }
    assert!(checked > 0, "no links were checked, which is not a pass");
    assert!(
        missing.is_empty(),
        "{checked} links checked; these do not resolve:\n{}",
        missing.join("\n")
    );
}

/// A column that ran carries bytes, and one that did not says so.
///
/// The exit code is not the measurement. Three CORBA sweeps once ran to
/// completion, exited 0 and wrote no rows — a variable was assigned and
/// never exported — and the run that consumed them read three ORBs as
/// agreeing with everyone, because a column with nothing in it and a
/// column that matched look the same from the outside. So the count is
/// asserted, per column, against the count its source was recorded
/// with.
///
/// The other half is the one that catches a floor left behind: a column
/// whose floor is zero must carry **no** bytes at all. Fast DDS was
/// absent for fifteen cases with no fill, and sixty rows simply did not
/// exist; a zero here has to mean "declared absent", never "nobody
/// looked".
#[test]
#[ignore = "builds the results from the whole corpus, 22 to 75 s in the test profile; runs in the weekly job interop-results under the cargo profile corpus"]
fn a_column_that_ran_carries_bytes() {
    let (_, built, _) = built();
    for system in &SYSTEMS {
        let carried = built
            .ledger
            .entries
            .iter()
            .filter(|entry| entry.system == system.id)
            .filter(|entry| matches!(entry.write, Outcome::Bytes(_)))
            .count();
        if system.bytes_floor == 0 {
            assert_eq!(
                carried, 0,
                "{} carries {carried} cells of bytes and its floor is 0; a column that \
                 produces bytes has a floor, so that a run which stops producing them fails \
                 here instead of reading as agreement",
                system.id
            );
            continue;
        }
        assert!(
            carried >= system.bytes_floor,
            "{} carries {carried} cells of bytes and was recorded with {}; a sweep that \
             reports success and returns fewer rows than it did is the failure this floor \
             exists to catch",
            system.id,
            system.bytes_floor
        );
    }
}

/// A `no form` cell is never a cell that also has bytes.
///
/// The eleven words are chosen so that each names a different kind of
/// answer. Two of them for one cell would mean the assembler decided
/// twice, and a reader would have no way to know which decision the
/// table printed.
#[test]
#[ignore = "builds the results from the whole corpus, 22 to 75 s in the test profile; runs in the weekly job interop-results under the cargo profile corpus"]
fn a_cell_carries_one_answer() {
    let (_, built, _) = built();
    // One case legitimately encodes to nothing, and the guard below cannot
    // tell it from a probe that exited 0 without running. `empty-struct`
    // is `struct S { }`: under XCDR1 there is no delimiter and no member,
    // so zero octets is the answer, not the absence of one. Under XCDR2 it
    // is four — a DHEADER of zero — and the guard applies there as
    // everywhere else.
    //
    // Named rather than relaxed: a second empty cell anywhere still fails,
    // which is what the guard is for.
    const ENCODES_TO_NOTHING: &[(&str, &str)] =
        &[("empty-struct", "plain-cdr1"), ("empty-struct", "pl-cdr1")];

    for entry in &built.ledger.entries {
        if let Outcome::Bytes(hex) = &entry.write {
            if ENCODES_TO_NOTHING
                .iter()
                .any(|(case, encoding)| *case == entry.case && *encoding == entry.encoding)
            {
                continue;
            }
            assert!(
                !hex.is_empty(),
                "{}/{}/{}/{} carries bytes and the bytes are empty, which is what a probe \
                 that exited 0 without running looks like",
                entry.case,
                entry.encoding,
                entry.endian,
                entry.system
            );
            assert!(
                hex.len() % 2 == 0 && hex.chars().all(|c| c.is_ascii_hexdigit()),
                "{}/{}/{}/{} carries something that is not hex",
                entry.case,
                entry.encoding,
                entry.endian,
                entry.system
            );
        }
    }
}
