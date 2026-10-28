//! `zerodds-ast-1.0` §11 — the coverage obligation the spec puts on its own
//! coverage document, checked instead of asserted.
//!
//! §11 demands four things of `docs/spec-coverage/zerodds-ast-1.0.md`:
//! it is a bilingual pair; every row names the specification feature, the
//! carrier (core node, facet or layout view), the implementing code path
//! and the test that exercises it; an empty cell is an open break and has
//! to be reported as one; and coverage may not be claimed from a grep
//! count. The first three are mechanical, and that is what this file
//! holds. The fourth is a reading, not a check.
//!
//! Paths are resolved from the repository root, two levels above this
//! crate's manifest (`tools/traceability`).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const DE: &str = "docs/spec-coverage/zerodds-ast-1.0.md";
const EN: &str = "docs/spec-coverage/zerodds-ast-1.0.en.md";

/// The three crates the coverage document names as the implementation,
/// plus the directory this file lives in — §11's own row is carried by the
/// checks below rather than by an AST crate.
const CRATES: [&str; 4] = [
    "crates/ast-core",
    "crates/ast",
    "crates/ast-layout",
    "tools/traceability/tests",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repository root above tools/traceability")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// One row of the coverage document: its heading and the text up to the
/// next heading.
struct Row {
    title: String,
    body: String,
}

fn rows(text: &str) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            out.push(Row {
                title: title.trim().to_string(),
                body: String::new(),
            });
        } else if let Some(row) = out.last_mut() {
            row.body.push_str(line);
            row.body.push('\n');
        }
    }
    // The closing `## Audit-Status` / `## Audit status` block is the
    // aggregate, not a row.
    out.retain(|r| r.body.contains("**Status:**"));
    out
}

fn field<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let marker = format!("**{name}:**");
    let at = body.find(&marker)?;
    let rest = &body[at + marker.len()..];
    let end = rest
        .match_indices("\n**")
        .map(|(i, _)| i)
        .next()
        .unwrap_or(rest.len());
    Some(rest[..end].trim())
}

/// Every identifier in backticks that reads like a Rust test function name.
fn cited_test_names(tests: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = tests;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        let code = &after[..close];
        rest = &after[close + 1..];
        let last = code.rsplit("::").next().unwrap_or(code);
        if !last.is_empty()
            && last
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && last.contains('_')
        {
            names.push(last.to_string());
        }
    }
    names
}

/// Every `fn` name defined in the three implementation crates.
fn defined_fns() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for c in CRATES {
        let dir = repo_root().join(c);
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    if p.file_name().is_some_and(|n| n == "target") {
                        continue;
                    }
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let Ok(src) = std::fs::read_to_string(&p) else {
                        continue;
                    };
                    for (i, _) in src.match_indices("fn ") {
                        let tail = &src[i + 3..];
                        let name: String = tail
                            .chars()
                            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                            .collect();
                        if !name.is_empty() {
                            names.insert(name);
                        }
                    }
                }
            }
        }
    }
    names
}

/// §11: "bilingual pair". The two files carry the same rows in the same
/// order, so a reader of either sees the same inventory.
#[test]
fn the_pair_carries_the_same_rows_in_the_same_order() {
    let de = rows(&read(DE));
    let en = rows(&read(EN));
    assert_eq!(
        de.len(),
        en.len(),
        "the German file has {} rows, the English one {}",
        de.len(),
        en.len()
    );
    assert!(!de.is_empty(), "the coverage document has no rows at all");

    for (d, e) in de.iter().zip(en.iter()) {
        let ds = field(&d.body, "Status").expect("German row has a status");
        let es = field(&e.body, "Status").expect("English row has a status");
        let first = |s: &str| {
            s.split(['—', '\n'])
                .next()
                .unwrap_or("")
                .trim()
                .trim_matches('`')
                .trim()
                .to_string()
        };
        assert_eq!(
            first(ds),
            first(es),
            "row {:?} / {:?}: the two files disagree on the status",
            d.title,
            e.title
        );
    }
}

/// §11: "each row states whether it is carried by a core node, a facet or
/// the layout view, with the implementing code path and the test that
/// exercises it. An empty cell is an open break." So no row that claims
/// `done` may leave a cell empty.
#[test]
fn no_done_row_leaves_a_cell_empty() {
    for file in [DE, EN] {
        for row in rows(&read(file)) {
            let status = field(&row.body, "Status").expect("a row without a status");
            let done = status.trim_start_matches('`').starts_with("done");
            for name in ["Spec", "Repo", "Tests"] {
                let cell = field(&row.body, name);
                if !done {
                    continue;
                }
                let cell = cell.unwrap_or_else(|| {
                    panic!("{file}: row {:?} has no **{name}:** cell", row.title)
                });
                assert!(
                    !cell.is_empty() && cell != "—" && cell != "-",
                    "{file}: row {:?} claims done with an empty {name} cell",
                    row.title
                );
            }
        }
    }
}

/// §11: "the test that exercises it". A cited name has to be a test that
/// exists in one of the three crates — a name that is nowhere is a claim,
/// not evidence.
#[test]
fn every_cited_test_name_exists_in_the_three_crates() {
    let defined = defined_fns();
    let mut cited_total = 0usize;
    for row in rows(&read(DE)) {
        let Some(tests) = field(&row.body, "Tests") else {
            continue;
        };
        for name in cited_test_names(tests) {
            cited_total += 1;
            assert!(
                defined.contains(&name),
                "row {:?} cites `{name}`, which no function in {CRATES:?} defines",
                row.title
            );
        }
    }
    assert!(
        cited_total >= 40,
        "only {cited_total} test names cited across the document — \
         §11 wants the test per row, not a sample"
    );
}

/// §11: every row names its carrier — core node, facet or layout view.
#[test]
fn every_row_names_its_carrier() {
    for row in rows(&read(DE)) {
        let repo = field(&row.body, "Repo").expect("a row without a Repo cell");
        assert!(
            repo.contains("Träger: Kern")
                || repo.contains("Träger: Facette")
                || repo.contains("Träger: Layout")
                || repo.contains("Träger: alle"),
            "row {:?} does not name its carrier in the Repo cell",
            row.title
        );
    }
}

/// The aggregate at the end has to agree with the rows above it —
/// otherwise the document reports a coverage it does not carry.
#[test]
fn the_audit_aggregate_matches_the_rows() {
    let text = read(DE);
    let counted = rows(&text).len();
    let line = text
        .lines()
        .find(|l| l.contains("done /") && l.contains("n/a (rejected)"))
        .expect("the Audit-Status line with the five counts");
    let numbers: Vec<usize> = line
        .split_whitespace()
        .filter_map(|t| t.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok())
        .collect();
    assert!(
        numbers.len() >= 5,
        "the Audit-Status line does not carry five counts: {line}"
    );
    let declared: usize = numbers[..5].iter().sum();
    assert_eq!(
        declared, counted,
        "the Audit-Status declares {declared} items, the document carries {counted}"
    );
}
