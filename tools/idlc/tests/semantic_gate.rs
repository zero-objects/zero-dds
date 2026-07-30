//! Semantic-gate integration tests (P0-1).
//!
//! Drives the real `zerodds-idlc` binary against the `semantic_gaps.idl`
//! repro and its valid counterpart, asserting that `check` and `generate`
//! (with and without `--no-typeobject`) refuse semantically invalid IDL with
//! a non-zero exit code and write no output file, while valid IDL still
//! passes with exit 0.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_zerodds-idlc");

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn unique_out(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("zdidlc-semgate-{tag}-{nanos}"))
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .expect("spawn zerodds-idlc")
}

#[test]
fn check_rejects_duplicate_member_and_unknown_type() {
    let idl = fixture("semantic_gaps.idl");
    let out = run(&["check", idl.to_str().unwrap()]);
    assert!(
        !out.status.success(),
        "check on semantic_gaps.idl must fail, got status {:?}",
        out.status
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("value"),
        "expected a duplicate-member diagnostic, got:\n{stderr}"
    );
    assert!(
        stderr.contains("DoesNotExist"),
        "expected an unresolved-type diagnostic, got:\n{stderr}"
    );
}

#[test]
fn generate_rejects_invalid_idl_and_writes_nothing() {
    let idl = fixture("semantic_gaps.idl");
    let out_dir = unique_out("gen");
    let out = run(&[
        "generate",
        "--rust",
        "-o",
        out_dir.to_str().unwrap(),
        idl.to_str().unwrap(),
    ]);
    assert!(
        !out.status.success(),
        "generate on semantic_gaps.idl must fail, got status {:?}",
        out.status
    );
    // No .rs file must have been written.
    let wrote_file = std::fs::read_dir(&out_dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .any(|e| e.path().extension().is_some_and(|x| x == "rs"))
        })
        .unwrap_or(false);
    assert!(
        !wrote_file,
        "generate must not write an output file when the gate fails"
    );
    std::fs::remove_dir_all(&out_dir).ok();
}

#[test]
fn no_typeobject_does_not_bypass_the_gate() {
    let idl = fixture("semantic_gaps.idl");
    let out_dir = unique_out("gen-noto");
    let out = run(&[
        "generate",
        "--rust",
        "--no-typeobject",
        "-o",
        out_dir.to_str().unwrap(),
        idl.to_str().unwrap(),
    ]);
    assert!(
        !out.status.success(),
        "generate --no-typeobject on semantic_gaps.idl must still fail, got status {:?}",
        out.status
    );
    let wrote_file = std::fs::read_dir(&out_dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .any(|e| e.path().extension().is_some_and(|x| x == "rs"))
        })
        .unwrap_or(false);
    assert!(
        !wrote_file,
        "generate --no-typeobject must not write an output file when the gate fails"
    );
    std::fs::remove_dir_all(&out_dir).ok();
}

#[test]
fn valid_idl_passes_check() {
    let idl = fixture("semantic_valid.idl");
    let out = run(&["check", idl.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "check on semantic_valid.idl must pass, stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn valid_idl_generates_with_no_typeobject() {
    let idl = fixture("semantic_valid.idl");
    let out_dir = unique_out("gen-valid");
    let out = run(&[
        "generate",
        "--rust",
        "--no-typeobject",
        "-o",
        out_dir.to_str().unwrap(),
        idl.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "generate on semantic_valid.idl must pass, stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let wrote_file = std::fs::read_dir(&out_dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .any(|e| e.path().extension().is_some_and(|x| x == "rs"))
        })
        .unwrap_or(false);
    assert!(wrote_file, "valid IDL must produce a .rs output file");
    std::fs::remove_dir_all(&out_dir).ok();
}
