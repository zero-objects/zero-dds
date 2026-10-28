// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `--strict` reaches every subcommand.
//!
//! The flag is documented as "OMG IDL 4.2 only; reject every vendor
//! extension". It used to be parsed and then dropped — `analyse()` read
//! it into `let _`, and `Parser::over` wired `Dialect::ALL_VENDORS` in
//! regardless — so over the 356 probes of `tools/idl-4.2-gap-scan` there
//! was not one where `check` and `check --strict` differed.
//!
//! A flag that only one of six subcommands honours makes the same
//! promise and breaks it five times, which is why this test is a matrix:
//! every subcommand, every vendor extension the front end acts on, and
//! both dialects. A `check` that refuses while `format` rewrites the same
//! file hands the user a file their tool discards on the next step.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::process::Command;

/// The subcommands, with the arguments each needs beyond the file.
///
/// `format` writes to stdout and `generate` needs a language and a
/// directory; the rest take the file and nothing else.
fn subcommands(out: &std::path::Path) -> Vec<(&'static str, Vec<String>)> {
    vec![
        ("check", vec![String::from("check")]),
        ("lint", vec![String::from("lint")]),
        ("format", vec![String::from("format")]),
        ("dump-ast", vec![String::from("dump-ast")]),
        ("print-deps", vec![String::from("print-deps")]),
        (
            "generate",
            vec![
                String::from("generate"),
                String::from("--lang"),
                String::from("rust"),
                String::from("--out"),
                out.to_string_lossy().into_owned(),
            ],
        ),
    ]
}

/// Run one subcommand over `file`, and say whether it exited clean.
fn runs_clean(argv: &[String], file: &std::path::Path, strict: bool) -> bool {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"));
    command.args(argv);
    if strict {
        command.arg("--strict");
    }
    command.arg(file);
    let output = command.output().expect("the binary runs");
    // Exit 2 is a bad command line and exit 3 an unreadable file; either
    // would make this test pass for the wrong reason.
    let code = output.status.code().expect("an exit code");
    assert!(
        code == 0 || code == 1,
        "{argv:?}: exit {code}, not a diagnostic result:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    code == 0
}

/// One file per vendor extension the front end acts on.
fn vendor_extensions() -> [(&'static str, &'static str); 4] {
    [
        ("rti-keylist", "struct Sa { long m; };\nkeylist Sa (m);\n"),
        (
            "cyclone-pragma-keylist",
            "#pragma keylist Sa m\nstruct Sa { long m; };\n",
        ),
        (
            "opendds-pragma-datatype",
            "#pragma DCPS_DATA_TYPE \"Sa\"\nstruct Sa { long m; };\n",
        ),
        (
            "opendds-pragma-datakey",
            "#pragma DCPS_DATA_TYPE \"Sa\"\n#pragma DCPS_DATA_KEY Sa m\n\
             struct Sa { long m; };\n",
        ),
    ]
}

#[test]
fn every_subcommand_refuses_every_vendor_extension_under_strict() {
    let dir = tempdir("refuses");
    for (name, source) in vendor_extensions() {
        let file = dir.join(format!("{name}.idl"));
        std::fs::write(&file, source).expect("write");
        for (subcommand, argv) in subcommands(&dir) {
            assert!(
                !runs_clean(&argv, &file, true),
                "{subcommand} --strict accepted {name}"
            );
        }
    }
}

/// The other half, and the half that keeps the rule honest: the same
/// files, the same subcommands, no `--strict`, and every one of them
/// gets through. A rule that refuses under both dialects would satisfy
/// the test above and break every file in the tree that uses one of
/// these directives.
#[test]
fn every_subcommand_accepts_the_same_files_without_strict() {
    let dir = tempdir("accepts");
    for (name, source) in vendor_extensions() {
        let file = dir.join(format!("{name}.idl"));
        std::fs::write(&file, source).expect("write");
        for (subcommand, argv) in subcommands(&dir) {
            assert!(
                runs_clean(&argv, &file, false),
                "{subcommand} refused {name} without --strict"
            );
        }
    }
}

/// IDL 4.2 §7.4.15.4.2: "in case an unknown annotation is encountered, it
/// should be ignored by the IDL-processing tools." `@Key`, `@RTI_*` and
/// `@RTPS_*` are unknown annotations the front end does not act on, so
/// `--strict` refusing them would break conforming IDL to gate nothing.
#[test]
fn strict_does_not_refuse_an_unknown_annotation() {
    let dir = tempdir("unknown-annotation");
    for (name, source) in [
        ("fastdds-key-capital", "struct Sa { @Key long m; };\n"),
        (
            "rti-annotation",
            "@RTI_resource_limits(initial_count=10) struct Sa { long m; };\n",
        ),
        ("unknown", "@nosuchthing struct Sa { long m; };\n"),
    ] {
        let file = dir.join(format!("{name}.idl"));
        std::fs::write(&file, source).expect("write");
        for (subcommand, argv) in subcommands(&dir) {
            assert!(
                runs_clean(&argv, &file, true),
                "{subcommand} --strict refused {name}, which §7.4.15.4.2 says to ignore"
            );
        }
    }
}

/// A directory of this test's own, under the target directory so that a
/// `cargo clean` takes it with everything else.
///
/// One per test: the tests run in parallel and write the same file names,
/// and `std::fs::write` truncates before it writes — a subcommand of one
/// test that read the file in between saw it empty, and an empty file is
/// clean under `--strict` too ("lint --strict accepted
/// cyclone-pragma-keylist").
fn tempdir(test: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("idlc-strict")
        .join(test);
    std::fs::create_dir_all(&dir).expect("the temp directory");
    dir
}
