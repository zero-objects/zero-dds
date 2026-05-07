//! §8.2 ABI-Compat-Snapshot-Test.
//!
//! Vergleicht den aktuellen Symbol-Surface von `include/zerodds.h` gegen
//! `tests/abi.snapshot.json`. Schlägt fehl, sobald eine Funktion entfernt
//! oder umbenannt wurde — additive Änderungen sind erlaubt (Minor-Bump-
//! Semantik per `zerodds-c-api-1.0` §7).
//!
//! Snapshot wird durch Setzen von `ZERODDS_ABI_REGENERATE=1` neu geschrieben.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

const HEADER_REL: &str = "include/zerodds.h";
const SNAPSHOT_REL: &str = "tests/abi.snapshot.json";

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn extract_symbols(header: &str) -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();
    for line in header.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('*') {
            continue;
        }
        if let Some(name) = parse_function_name(trimmed) {
            symbols.insert(name.to_string());
        }
    }
    symbols
}

fn parse_function_name(line: &str) -> Option<&str> {
    let paren = line.find('(')?;
    let head = &line[..paren];
    let last_token = head
        .rsplit(|c: char| c.is_whitespace() || c == '*')
        .next()?;
    if last_token.starts_with("zerodds_") {
        Some(last_token)
    } else {
        None
    }
}

#[test]
fn abi_snapshot_matches() {
    let root = crate_root();
    let header = fs::read_to_string(root.join(HEADER_REL)).expect("zerodds.h header readable");
    let current = extract_symbols(&header);

    let snapshot_path = root.join(SNAPSHOT_REL);

    if std::env::var("ZERODDS_ABI_REGENERATE").is_ok() {
        let json = serde_json::to_string_pretty(&current).expect("serialize");
        fs::write(&snapshot_path, json).expect("write snapshot");
        eprintln!("abi.snapshot.json regenerated: {} symbols", current.len());
        return;
    }

    let raw = fs::read_to_string(&snapshot_path)
        .expect("abi.snapshot.json present — run with ZERODDS_ABI_REGENERATE=1 to seed");
    let baseline: BTreeSet<String> =
        serde_json::from_str(&raw).expect("snapshot is JSON array of symbol names");

    let removed: Vec<&String> = baseline.difference(&current).collect();
    let added: Vec<&String> = current.difference(&baseline).collect();

    assert!(
        removed.is_empty(),
        "ABI regression: {} symbols removed/renamed since snapshot — {:?}\n\
         If this is intentional (major bump), regenerate via ZERODDS_ABI_REGENERATE=1.",
        removed.len(),
        removed
    );

    if !added.is_empty() {
        eprintln!(
            "abi: {} new symbols since snapshot (additive minor — OK): {:?}",
            added.len(),
            added
        );
    }
}

#[test]
fn extract_symbols_finds_known_functions() {
    let header = "\
struct zerodds_ZeroDdsRuntime *zerodds_runtime_create(uint32_t domain_id);\n\
void zerodds_runtime_destroy(struct zerodds_ZeroDdsRuntime *runtime);\n\
int zerodds_writer_write(struct zerodds_ZeroDdsWriter *writer);\n\
// not_a_function comment\n\
typedef struct zerodds_OpaqueType zerodds_OpaqueType;\n\
";
    let symbols = extract_symbols(header);
    assert!(symbols.contains("zerodds_runtime_create"));
    assert!(symbols.contains("zerodds_runtime_destroy"));
    assert!(symbols.contains("zerodds_writer_write"));
    assert!(!symbols.contains("zerodds_OpaqueType"));
}
