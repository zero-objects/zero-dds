// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! A diagnostic names the file and the line a reader finds it at.
//!
//! The spans a diagnostic carries were offsets into the preprocessor's
//! output — comments and directives gone, includes inlined, one space
//! between tokens — and `check` turned them into line and column against
//! the unit as written. Four lines of comment put an error on line 3
//! that stood on line 6, and an error inside an included file was printed
//! against the unit that included it.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::Path;
use std::process::Command;

fn check(dir: &Path, file: &str, json: bool) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zerodds-idlc"));
    command.current_dir(dir).arg("check");
    if json {
        command.arg("--json");
    }
    let output = command.arg(file).output().expect("the binary runs");
    String::from_utf8(output.stdout).expect("utf-8")
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `S` and `s` in one scope differ only in case (OMG IDL 4.2 §7.2.3), and
/// the diagnostic points into line 6 whatever stands above it.
#[test]
fn comments_above_an_error_do_not_move_it() {
    let dir = scratch("diagnostic-positions-comments");
    let body = "module t {\n  struct S { long s; };\n};\n";
    std::fs::write(dir.join("plain.idl"), body).unwrap();
    std::fs::write(
        dir.join("commented.idl"),
        format!("// one\n// two\n/* three\n   four */\n{body}"),
    )
    .unwrap();
    let plain = check(&dir, "plain.idl", false);
    let commented = check(&dir, "commented.idl", false);
    let position = |output: &str, file: &str| {
        let line = output
            .lines()
            .find(|line| line.contains("[IDL0008]"))
            .unwrap_or_else(|| panic!("no IDL0008 in:\n{output}"));
        let rest = line
            .strip_prefix(&format!("{file}:"))
            .expect("the file first");
        let mut fields = rest.split(':');
        let line_no: usize = fields.next().unwrap().parse().unwrap();
        let column: usize = fields.next().unwrap().parse().unwrap();
        (line_no, column)
    };
    let (plain_line, plain_column) = position(&plain, "plain.idl");
    let (line, column) = position(&commented, "commented.idl");
    assert_eq!(plain_line, 2);
    assert_eq!(
        (line, column),
        (plain_line + 4, plain_column),
        "{commented}"
    );
}

#[test]
fn an_error_in_an_include_names_the_include() {
    let dir = scratch("diagnostic-positions-include");
    std::fs::write(
        dir.join("inc.idl"),
        "// header\n\nmodule inc {\n  struct T { long t; };\n};\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.idl"),
        "// main\n#include \"inc.idl\"\nmodule m { struct U { long v; }; };\n",
    )
    .unwrap();
    let text = check(&dir, "main.idl", false);
    assert!(
        text.lines()
            .any(|line| line.starts_with("inc.idl:4:") && line.contains("[IDL0008]")),
        "{text}"
    );
    let json = check(&dir, "main.idl", true);
    assert!(json.contains("\"path\":\"inc.idl\""), "{json}");
    assert!(json.contains("\"line\":4"), "{json}");
}
