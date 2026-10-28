// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A declaration with no data form is left out and reported, and the rest
//! of the file is generated.
//!
//! DDS-XTypes 1.3 §7.3.1.1.1: IDL constructs outside the building blocks
//! "remain legal for use in IDL files provided to IDL compilers compliant
//! with this specification. However, their meanings are undefined with
//! respect to this specification. Compilers that do not support them
//! shall ignore them or issue a warning rather than halting with an
//! error." `any` has no type identifier and no wire form in DDS-XTypes;
//! `generate` answered a file with one `any` member by refusing the whole
//! file.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::process::{Command, Output};

const SOURCE: &str = "struct Plain { long a; };\n\
                      struct Holds { any a; };\n\
                      struct Deep { Holds h; };\n\
                      struct Beside { Plain p; string s; };\n";

fn tempdir(leaf: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("idlc-outside-profile")
        .join(leaf);
    std::fs::create_dir_all(&dir).expect("the temp directory");
    dir
}

/// `tag` names the test: the two tests of this file run in parallel, and a source file or an
/// output directory they share is truncated by one while the other reads it (an empty source
/// reports nothing, which `--deny-warnings` then takes as clean).
fn generate(tag: &str, language: &str, extra: &[&str]) -> (Output, std::path::PathBuf) {
    let shared = tempdir(&format!("{tag}-src"));
    let idl = shared.join("sample.idl");
    std::fs::write(&idl, SOURCE).expect("the source is written");
    let out = tempdir(&format!("{tag}-{language}")).join("out");
    std::fs::create_dir_all(&out).expect("the output directory");
    let output = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"))
        .args(["generate", "--lang", language, "--out"])
        .arg(&out)
        .args(extra)
        .arg(&idl)
        .output()
        .expect("the binary runs");
    (output, out)
}

#[test]
fn the_rest_of_the_file_is_generated_and_the_two_declarations_are_named() {
    for (language, file) in [
        ("rust", "sample.rs"),
        ("c", "sample.h"),
        ("python", "sample.py"),
    ] {
        let (output, out) = generate("the-rest", language, &[]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "{language}: generate exited {:?}:\n{stdout}\n{stderr}",
            output.status.code()
        );
        let report = format!("{stdout}{stderr}");
        for name in ["::Holds", "::Deep"] {
            assert!(
                report.contains(name) && report.contains("`any`"),
                "{language}: the warning does not name {name} and `any`:\n{report}"
            );
        }
        let text = std::fs::read_to_string(out.join(file))
            .unwrap_or_else(|error| panic!("{language}: {file}: {error}"));
        assert!(
            text.contains("Beside"),
            "{language}: Beside is missing:\n{text}"
        );
        assert!(
            text.contains("Plain"),
            "{language}: Plain is missing:\n{text}"
        );
        assert!(
            !text.contains("Holds"),
            "{language}: Holds was generated:\n{text}"
        );
        assert!(
            !text.contains("Deep"),
            "{language}: Deep was generated:\n{text}"
        );
    }
}

#[test]
fn deny_warnings_turns_it_back_into_a_failure() {
    let (output, _) = generate("deny", "rust", &["--deny-warnings"]);
    assert!(
        !output.status.success(),
        "--deny-warnings left a file with a dropped declaration clean"
    );
}
