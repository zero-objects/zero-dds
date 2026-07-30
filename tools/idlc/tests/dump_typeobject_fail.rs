//! `dump-typeobject` error-contract integration tests (P1).
//!
//! Drives the real `zerodds-idlc` binary against a spec that mixes an
//! independently-lowerable type with a recursive one. Asserts that
//! `dump-typeobject`
//!
//! * still prints the lowerable type's TypeObject to stdout (per-SCC
//!   isolation keeps unrelated types), and
//! * exits non-zero with a diagnostic on stderr — never the old "print the
//!   failure on stdout, return exit 0" behaviour.

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

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .expect("spawn zerodds-idlc")
}

#[test]
fn dump_typeobject_fails_on_mapping_error() {
    let idl = fixture("recursive_typeobject.idl");
    let out = run(&["--dump-typeobject", idl.to_str().unwrap()]);

    // Contract: a mapping error is a hard failure, not a stdout note.
    assert!(
        !out.status.success(),
        "dump-typeobject on a non-lowerable spec must exit non-zero, got {:?}",
        out.status
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // The independent, well-formed type is still dumped to stdout.
    assert!(
        stdout.contains("TypeObject for Flat"),
        "independent type Flat must still be dumped, stdout:\n{stdout}"
    );
    // The failure is reported on stderr and names the dropped type.
    assert!(
        stderr.contains("Node"),
        "stderr must name the dropped recursive type, stderr:\n{stderr}"
    );
    // The old bug printed the failure to stdout with exit 0 — guard against it.
    assert!(
        !stdout.contains("mapping failed") && !stdout.contains("mapping incomplete"),
        "the mapping error must not be laundered onto stdout, stdout:\n{stdout}"
    );
}

#[test]
fn dump_typeobject_succeeds_on_total_mapping() {
    let idl = fixture("semantic_valid.idl");
    let out = run(&["--dump-typeobject", idl.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "dump-typeobject on a fully-lowerable spec must exit 0, stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
