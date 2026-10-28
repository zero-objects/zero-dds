// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Does the committed tree still say what a run says?
//!
//! This is the comparison the whole exercise rests on. The tables are a
//! rendering of `ledger.tsv`; `ledger.tsv` is built from the source
//! TSVs and from a fresh run of this project's own columns; and the
//! `.bin` files are that ledger's bytes. If any of the four can drift
//! from any other, a figure in a table and a figure a gate prints can
//! disagree, which is the defect this tree exists to remove.
//!
//! What this test does **not** do is re-run the foreign systems. It
//! cannot: RTI is licensed, the ORBs are hand-built and the ROS type
//! supports need a ROS installation. Their bytes are re-derived by
//! `tools/interop-results/vendor-corpus-sweep.sh` and its two siblings
//! on a machine that has them, and the comparison that proves *those*
//! did not move is the CI job. What this test proves is that the tree
//! and the two tables are what the committed record says, which is what
//! a clone with a Rust toolchain and nothing else can check.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::PathBuf;

use zerodds_interop_results as results;
use zerodds_interop_results::assemble::{self, Inputs};
use zerodds_interop_results::ledger::{self, Outcome, RowIds};
use zerodds_interop_results::page;
use zerodds_interop_results::systems::SYSTEMS;
use zerodds_interop_results::tables::{self, Direction};
use zerodds_interop_results::tree;
use zerodds_interop_results::warnings;

fn built() -> (Vec<results::Case>, assemble::Built, PathBuf) {
    let root = results::repository_root();
    let results_root = results::results_root(&root);
    let cases = results::corpus(&root).expect("the corpus");
    let inputs = Inputs::read(&results_root.join("sources")).expect("the sources");
    let built = assemble::build(&cases, &inputs).expect("the ledger");
    (cases, built, results_root)
}

/// Every byte in the tree is a byte the ledger carries.
#[test]
fn the_committed_bytes_are_the_ones_a_run_produces() {
    let (_, built, results_root) = built();
    let differences = tree::verify(&results_root, &built.ledger).expect("the comparison");
    assert!(
        differences.is_empty(),
        "{} cell(s) in the tree are not what a run produces; the first few are:\n{}",
        differences.len(),
        differences
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The ledger and the two tables are what a run writes.
#[test]
fn the_committed_documents_are_the_ones_a_run_writes() {
    let (cases, built, results_root) = built();
    let committed =
        std::fs::read_to_string(results_root.join("ledger.tsv")).expect("the committed ledger");
    assert_eq!(
        committed,
        ledger::render(&built.ledger),
        "ledger.tsv is not what a run writes; run tools/interop-results/regenerate.sh"
    );

    let text = std::fs::read_to_string(results_root.join("row-ids.tsv")).unwrap_or_default();
    let mut ids = RowIds::read(&text).expect("the row-id registry");
    let before = ids.by_key.len();
    for direction in [Direction::Write, Direction::Read] {
        let fresh = tables::render(direction, &built.ledger, &cases, &built.sweeps, &mut ids);
        let path = results_root.join(tables::file(direction));
        let committed = std::fs::read_to_string(&path).expect("a committed table");
        assert_eq!(
            committed.len(),
            fresh.len(),
            "{} is not what a run writes; run tools/interop-results/regenerate.sh",
            tables::file(direction)
        );
        assert!(
            committed == fresh,
            "{} is not what a run writes; run tools/interop-results/regenerate.sh",
            tables::file(direction)
        );
    }
    assert_eq!(
        ids.by_key.len(),
        before,
        "{} row(s) have no number in row-ids.tsv; run tools/interop-results/regenerate.sh",
        ids.by_key.len() - before
    );
}

/// `SYSTEMS.md` and `WARNINGS.md` are what a run writes too.
///
/// Both carry generated arithmetic — the ORB cell counts in one, the
/// warning case counts in the other — and a generated figure that no
/// run recomputes is a figure that goes stale the next time a case is
/// added or a vendor is upgraded.
#[test]
fn the_two_generated_documents_are_the_ones_a_run_writes() {
    let root = results::repository_root();
    let results_root = results::results_root(&root);
    let cases = results::corpus(&root).expect("the corpus");
    let inputs = Inputs::read(&results_root.join("sources")).expect("the sources");
    let built = assemble::build(&cases, &inputs).expect("the ledger");
    for (name, fresh) in [
        ("SYSTEMS.md", tables::systems_document(&built.ledger)),
        (
            "WARNINGS.md",
            warnings::document(&inputs.diagnostics, &SYSTEMS),
        ),
    ] {
        let committed = std::fs::read_to_string(results_root.join(name)).unwrap_or_else(|error| {
            panic!("{name}: {error}; run tools/interop-results/regenerate.sh")
        });
        assert!(
            committed == fresh,
            "{name} is not what a run writes; run tools/interop-results/regenerate.sh"
        );
    }
}

/// `matrix.html` is what a run writes, and it is deterministic.
///
/// The page carries the acceptance answer, the dialect matrix and every
/// per-system figure, all derived. It is also the page a bug report is
/// answered with, so a figure on it that no run recomputes is a figure
/// somebody quotes at a reporter after it stopped being true.
#[test]
fn the_page_is_the_one_a_run_writes() {
    let root = results::repository_root();
    let (cases, built, results_root) = built();
    let documents = page::documents(&root, &cases).expect("the documents the page quotes");
    let mut ids = RowIds::read(
        &std::fs::read_to_string(results_root.join("row-ids.tsv")).unwrap_or_default(),
    )
    .expect("the row numbers");
    let fresh = page::render(
        &built.ledger,
        &cases,
        &built.sweeps,
        &mut ids,
        &documents,
        "",
    )
    .expect("the page");
    let committed = std::fs::read_to_string(results_root.join(page::FILE))
        .unwrap_or_else(|error| panic!("{}: {error}; run --page", page::FILE));
    assert!(
        committed == fresh,
        "{} is not what a run writes; run `cargo run -p zerodds-interop-results -- --page`",
        page::FILE
    );
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
