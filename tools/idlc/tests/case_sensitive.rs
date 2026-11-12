// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `--case-sensitive` reaches every subcommand (GitHub #66).
//!
//! OMG IDL 4.2 §7.2.3 compares identifiers without regard to case, so
//! `module shape` beside `struct Shape` is a collision. The flag reads the
//! file as `idlc -f case-sensitive` of Cyclone DDS does.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::process::Command;

const SOURCE: &str = "module shapes {\n    module shape {\n        struct Details { long value; };\n    };\n    struct Shape {\n        shape::Details details;\n    };\n};\n";

fn exit_code(argv: &[&str], file: &std::path::Path, case_sensitive: bool) -> i32 {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"));
    command.args(argv);
    if case_sensitive {
        command.arg("--case-sensitive");
    }
    command.arg(file);
    let output = command.output().expect("the binary runs");
    let code = output.status.code().expect("an exit code");
    assert!(
        code == 0 || code == 1,
        "{argv:?}: exit {code}, not a diagnostic result:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    code
}

#[test]
fn idl_7_2_3_the_flag_decides_for_every_subcommand() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("shapes.idl");
    std::fs::write(&file, SOURCE).expect("write idl");
    let out = dir.path().join("out");
    std::fs::create_dir_all(&out).expect("out dir");
    let out_text = out.to_string_lossy().into_owned();

    let subcommands: Vec<Vec<&str>> = vec![
        vec!["check"],
        vec!["lint"],
        vec!["dump-ast"],
        vec!["generate", "--lang", "rust", "--out", &out_text],
        vec!["generate", "--lang", "cpp", "--out", &out_text],
    ];
    for argv in &subcommands {
        assert_eq!(
            exit_code(argv, &file, false),
            1,
            "{argv:?} without the flag"
        );
        assert_eq!(exit_code(argv, &file, true), 0, "{argv:?} with the flag");
    }
}
