// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Every corpus case is registered everywhere it has to be, in one run.
//!
//! A new case has to appear in seven independent places, and **none of
//! them knows about the others**. Each has its own test, each reports
//! only its own gap, and a case missing from four of them takes four
//! rounds to find — one failing test per round, because the first one
//! stops the suite before the next is reached.
//!
//! Twenty-nine cases were added in one session in August 2026 and cost
//! exactly that: five rounds, one per place that was missed. This test
//! exists so the next twenty-nine cost one.
//!
//! ## What it checks, from the corpus directory alone
//!
//! | | place | what is missing when it fails |
//! |---|---|---|
//! | 1 | the case directory | `type.idl`, `type` or `values.json` |
//! | 2 | its `README.md` | the file, or its `## Probe status` section |
//! | 3 | the probe status rows | a row for one of the four modes |
//! | 4 | the Rust baseline | a digest line in `rust_column_baseline.txt` |
//! | 5 | `tests/interop-results/` | the case's directory in the result tree |
//!
//! Places 6 and 7 — the refusal lists and the exact counters — are not
//! checked here and cannot be from the corpus alone: whether a case
//! belongs on a refusal list is a fact about a backend, not about the
//! case. Their own tests report them, and `docs/updating-a-vendor.md`
//! names all seven.
//!
//! ## Why one test and not five
//!
//! Because the failure mode being fixed is *serialised discovery*. Five
//! tests find one gap per run; this finds all of them in the first run,
//! which is the difference between one round of work and five.

#![allow(clippy::expect_used, clippy::panic, missing_docs)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the repository root, two levels above crates/wire-probe")
        .to_path_buf()
}

/// Every case directory name in the corpus.
fn corpus_cases(root: &Path) -> Vec<String> {
    let directory = root.join("tests").join("interop-corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
    {
        let path = entry.expect("a corpus entry").path();
        if path.join("type").is_file() {
            out.push(
                path.file_name()
                    .expect("a directory name")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    out.sort();
    out
}

/// The four `## Probe status` modes every README carries.
const MODES: [&str; 4] = [
    "--encoding xcdr2 --endian le",
    "--encoding xcdr2 --endian be",
    "--encoding xcdr1 --endian le",
    "--encoding xcdr1 --endian be",
];

#[test]
fn every_case_is_registered_in_every_place_that_needs_it() {
    let root = repository_root();
    let cases = corpus_cases(&root);
    assert!(
        !cases.is_empty(),
        "the corpus is empty, which cannot be right"
    );

    let baseline: BTreeSet<String> = std::fs::read_to_string(
        root.join("crates/wire-probe/tests/fixtures/rust_column_baseline.txt"),
    )
    .expect("the Rust baseline")
    .lines()
    .filter(|line| !line.starts_with('#'))
    .filter_map(|line| line.split('\t').next().map(ToString::to_string))
    .collect();

    let results_root = root.join("tests").join("interop-results");

    let mut trouble: Vec<String> = Vec::new();
    for case in &cases {
        let directory = root.join("tests").join("interop-corpus").join(case);

        // 1 — the case's own three files.
        for file in ["type.idl", "type", "values.json"] {
            if !directory.join(file).is_file() {
                trouble.push(format!("{case}: has no `{file}`"));
            }
        }

        // 2 and 3 — the README and its recorded rows.
        let readme_path = directory.join("README.md");
        match std::fs::read_to_string(&readme_path) {
            Err(_) => trouble.push(format!("{case}: has no README.md")),
            Ok(readme) => {
                // `## Evolution` carries a second table; the probe status
                // reader stops there and so does this.
                let head = readme.split("## Evolution").next().unwrap_or(&readme);
                if !head.contains("## Probe status") {
                    trouble.push(format!(
                        "{case}: README.md has no `## Probe status` section — the \
                         rewriter fills that section and does not create it"
                    ));
                } else {
                    for mode in MODES {
                        if !head.contains(mode) {
                            trouble
                                .push(format!("{case}: `## Probe status` has no row for `{mode}`"));
                        }
                    }
                    if head.contains("| pending |") {
                        trouble.push(format!(
                            "{case}: `## Probe status` still says `pending` — run \
                             `cargo test -p zerodds-wire-probe --test interop_corpus -- \
                             --ignored rerecord_the_probe_status_tables`"
                        ));
                    }
                }
            }
        }

        // 4 — the Rust column's baseline digest.
        if !baseline.contains(case) {
            trouble.push(format!(
                "{case}: not in rust_column_baseline.txt — run with \
                 ZERODDS_REBASELINE_RUST_COLUMN=1"
            ));
        }

        // 5 — the committed result tree.
        if !results_root.join(case).is_dir() {
            trouble.push(format!(
                "{case}: no directory under tests/interop-results/ — run \
                 `interop-results --regenerate`"
            ));
        }
    }

    assert!(
        trouble.is_empty(),
        "{} corpus registration gap(s), all of them at once:\n  {}\n\n\
         docs/updating-a-vendor.md, \"When the corpus grows instead\", lists all \
         seven places and the command for each.",
        trouble.len(),
        trouble.join("\n  ")
    );
}
