// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Regression: a typedef reaching a member gave its dumper the same
//! name as some other `id` in the model, twice over.
//!
//! `crates/wire-probe/tests/three_column_gate.rs`'s Python column found
//! both shapes as soon as it ran the whole corpus rather than the one
//! case `docs/python-column.md` had recorded:
//!
//! - **A typedef of a typedef of a typedef of a primitive**
//!   (`typedef-chain-primitive` and its four siblings below) —
//!   `dumper_name` collapses every alias in the chain to the same
//!   string as the one after it, `zd_json_long`, so the *first* alias
//!   the reachability walk met used to write `def zd_json_long(v,
//!   path): return zd_json_long(v, path)`, a forwarder to itself, and
//!   the struct's own dumper called a name that either did not exist or
//!   recursed for ever depending on which alias the walk met last.
//! - **A self-referential struct reached through a typedef**
//!   (`recursive-typedef-sequence`) — the alias's `dumper_name` matches
//!   its *target's*, one hop away, not a further one, so the earlier,
//!   narrower fix (`refuse the alias entirely`) closed this shape and
//!   reopened the first one over the whole `sequence-of-typedef` family.
//!   Both are here so a fix for one does not reopen the other.
//!
//! `tools/wireprobe/wireprobe-zerodds-python-gen/src/dump.rs`'s own
//! module doc on [`dump`] and the `TypeKind::Alias` arm of [`function`]
//! carry the fix and the reasoning; this is what keeps it fixed.
//!
//! [`dump`]: zerodds_wireprobe_python_gen (private; see the crate source)
//! [`function`]: zerodds_wireprobe_python_gen (private; see the crate source)
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wireprobe_python_gen::{Args, source};

fn repository_root() -> PathBuf {
    // `tools/wireprobe/wireprobe-zerodds-python-gen` is three below the
    // root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("the repository root")
        .to_path_buf()
}

fn python_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Every `def <name>(` this generated module declares at module scope,
/// once each — a second `def` under a name already used is exactly the
/// silent-clobber shape both defects took, and it never fails to
/// compile, so the module's own text is what has to be checked, not
/// only its behaviour on one sample.
fn defined_names(source_text: &str) -> Vec<&str> {
    source_text
        .lines()
        .filter_map(|line| line.strip_prefix("def "))
        .filter_map(|rest| rest.split('(').next())
        .collect()
}

fn assert_no_duplicate_definitions(source_text: &str, case: &str) {
    let names = defined_names(source_text);
    let mut seen: Vec<&str> = Vec::new();
    for name in names {
        assert!(
            !seen.contains(&name),
            "{case}: `{name}` is defined more than once — the silent-clobber shape \
             `dump.rs`'s module doc records"
        );
        seen.push(name);
    }
}

/// One case, read back through its own recorded Rust-column bytes, and
/// checked against `values.json`.
fn read_case(case: &str, type_name: &str) {
    let root = repository_root();
    let directory = root.join("tests").join("interop-corpus").join(case);
    let recorded = root
        .join("tests")
        .join("interop-results")
        .join(case)
        .join("zerodds-rust")
        .join("plain-cdr2.le.bin");
    assert!(
        recorded.is_file(),
        "{}: no recorded Rust-column bytes to read back",
        recorded.display()
    );
    let values_out = std::env::temp_dir().join(format!(
        "zd-python-gen-alias-chain-{case}-{}.json",
        std::process::id()
    ));

    let args = Args {
        idl: directory.join("type.idl"),
        type_name: type_name.to_string(),
        input: Some(recorded),
        values_out: Some(values_out.clone()),
        encoding: Encoding::PlainCdr2,
        endianness: Endianness::Little,
        dialect: Dialect::Specification,
        ..Args::default()
    };
    let generated = source(&args).unwrap_or_else(|refusal| {
        panic!("{case}: the generator refused: {}", refusal.message());
    });
    assert_no_duplicate_definitions(&generated, case);

    if !python_available() {
        eprintln!("SKIPPED: `python3` is not on the path; {case} is not run here");
        return;
    }
    let script = std::env::temp_dir().join(format!(
        "zd-python-gen-alias-chain-{case}-{}.py",
        std::process::id()
    ));
    std::fs::write(&script, &generated).expect("the generated module is written");
    let runtime = repository_root().join("runtime").join("python");
    let output = Command::new("python3")
        .arg(&script)
        .env("PYTHONPATH", &runtime)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .expect("python3 starts");
    assert!(
        output.status.success(),
        "{case}: the generated module did not run:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let found_text = std::fs::read_to_string(&values_out)
        .unwrap_or_else(|error| panic!("{}: {error}", values_out.display()));
    let found: serde_json::Value =
        serde_json::from_str(&found_text).expect("the values file is JSON");
    let expected_text =
        std::fs::read_to_string(directory.join("values.json")).expect("the case's own values.json");
    let expected: serde_json::Value =
        serde_json::from_str(&expected_text).expect("values.json is JSON");
    assert_eq!(
        found, expected,
        "{case}: the read direction did not return the sample that was written"
    );

    let _ = std::fs::remove_file(&script);
    let _ = std::fs::remove_file(&values_out);
}

#[test]
fn a_typedef_chain_over_a_primitive_reads_back() {
    read_case("typedef-chain-primitive", "Sample");
}

#[test]
fn a_typedef_chain_over_a_primitive_unannotated_reads_back() {
    read_case("typedef-chain-primitive-unannotated", "Sample");
}

#[test]
fn a_sequence_of_a_typedef_chain_reads_back() {
    read_case("sequence-of-typedef", "Sample");
}

#[test]
fn a_sequence_of_a_typedef_chain_unannotated_reads_back() {
    read_case("sequence-of-typedef-unannotated", "Sample");
}

#[test]
fn a_three_deep_typedef_chain_into_a_sequence_reads_back() {
    read_case("resolve-typedef-chain-three", "m::S");
}

#[test]
fn a_self_referential_struct_reached_through_a_typedef_reads_back() {
    read_case("recursive-typedef-sequence", "m::Node");
}
