// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `--default-extensibility` reaches the emitted codec.
//!
//! XTypes 1.3 §7.3.1.2.1.8 makes an aggregate whose source says nothing
//! `@appendable`, and in the same paragraph provides for the option:
//!
//! > Implementations may provide a mechanism to override this default
//! > behavior; for example, IDL compilers may provide configuration
//! > options to allow users to specify whether types of unspecified
//! > extensibility are to be considered final, appendable, or mutable.
//!
//! It is not a nicety. `@appendable` is a DHEADER in XCDR2 and `@final`
//! is not, so the flag decides whether the bytes a caller generates are
//! the ones a peer expects — Cyclone's `idlc` reads the same silence as
//! `@final`, which is what the 2026-09-09 re-recording of
//! `vendor_roundtrip.rows.tsv` measures across a whole corpus.
//!
//! The library carried the axis the whole time (`ComposeOptions::
//! default_extensibility`, `zerodds-build`'s `default_extensibility`);
//! only `zerodds-idlc` did not hand it through, and nothing failed when
//! it stopped. This test fails instead.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::process::Command;

const SOURCE: &str = "struct Sample {\n  long a;\n  double b;\n};\n";

/// A directory of this test's own, under the target directory so that a
/// `cargo clean` takes it with everything else — the same shape
/// `tests/strict.rs` uses.
fn tempdir(leaf: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("idlc-default-extensibility")
        .join(leaf);
    std::fs::create_dir_all(&dir).expect("the temp directory");
    dir
}

fn emit(extensibility: Option<&str>) -> String {
    // One source path for all three runs: the emitted preamble names the
    // file it came from, so a per-run path would make the texts differ for
    // a reason that has nothing to do with the flag.
    let shared = tempdir("src");
    let idl = shared.join("sample.idl");
    std::fs::write(&idl, SOURCE).expect("the source is written");
    let out = tempdir(extensibility.unwrap_or("implied")).join("out");
    std::fs::create_dir_all(&out).expect("the output directory");

    let mut command = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"));
    command.args(["generate", "--lang", "rust", "--out"]);
    command.arg(&out);
    if let Some(value) = extensibility {
        command.args(["--default-extensibility", value]);
    }
    command.arg(&idl);
    let output = command.output().expect("the binary runs");
    assert!(
        output.status.success(),
        "generate {extensibility:?} exited {:?}:\n{}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(out.join("sample.rs")).expect("the emitted file")
}

#[test]
fn the_flag_decides_whether_a_silent_aggregate_carries_a_dheader() {
    let implied = emit(None);
    let appendable = emit(Some("appendable"));
    let final_ = emit(Some("final"));

    assert!(
        implied.contains("write_dheader"),
        "no flag must mean the specification's appendable, which is a DHEADER:\n{implied}"
    );
    assert_eq!(
        implied, appendable,
        "`--default-extensibility appendable` must be what no flag already is"
    );
    assert!(
        !final_.contains("write_dheader") && !final_.contains("read_dheader"),
        "`final` must emit no DHEADER — that is the whole point of the flag:\n{final_}"
    );
    assert_ne!(
        implied, final_,
        "the flag has to change the emitted codec, or it changes nothing"
    );
}

#[test]
fn a_value_that_names_no_extensibility_is_refused_by_name() {
    let dir = tempdir("refusal");
    let idl = dir.join("sample.idl");
    std::fs::write(&idl, SOURCE).expect("the source is written");
    let output = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
        .args(["check", "--default-extensibility", "wobble"])
        .arg(&idl)
        .output()
        .expect("the binary runs");
    assert!(!output.status.success(), "an unknown value must not pass");
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains("wobble") && said.contains("final"),
        "the refusal has to name the value and the choices, said:\n{said}"
    );
}
