// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Union labels that do not fit the width the printer assumed.
//!
//! The read direction prints the decoded value as JSON, and a union's
//! branch is chosen there by a second label test — the generator's own,
//! beside the codec's. That test widened every label to `int64`, and a
//! `case 16045690984833335998:` over an `unsigned long long` has no
//! `'i64` literal at all: Nim rejects the token, so the probe answered
//! 7 (generator-defect) for `union-discriminator-ulonglong` in every
//! option set while the write direction matched the Rust column's
//! octets. The two ends of `long long` are the neighbours of the same
//! shape and the corpus has no case for either.
//!
//! `nim check` is the assertion where a compiler is on the path; where
//! there is none the shape of the literal still is.
// `print_stderr`: the one `eprintln!` says the module was not checked
// because no Nim compiler was on the path.
#![allow(clippy::expect_used, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use zerodds_ast_layout::{Dialect, Encoding, Endianness};
use zerodds_wireprobe_nim_gen::{Args, source};

/// A private directory for one case's files.
fn workspace() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "zd-nim-labels-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("the case directory");
    dir
}

/// The generated `main.nim` for one IDL source, in the read direction —
/// the direction that carries the JSON printer.
fn reader(dir: &Path, idl: &str, type_name: &str) -> String {
    let idl_path = dir.join("type.idl");
    std::fs::write(&idl_path, idl).expect("the IDL");
    // Any octets will do: `source` embeds them and the decode happens
    // when the program runs, which this test does not do.
    let input = dir.join("bytes.bin");
    std::fs::write(&input, [0_u8; 16]).expect("the octets");
    let args = Args {
        idl: idl_path,
        type_name: type_name.to_string(),
        values: None,
        input: Some(input),
        values_out: Some(dir.join("out.jsn")),
        encoding: Encoding::PlainCdr2,
        endianness: Endianness::Little,
        encapsulation: false,
        dialect: Dialect::Specification,
        out_dir: dir.to_path_buf(),
    };
    source(&args).expect("the generator produces a program")
}

/// Hand the module to `nim check` against the real `runtime/nim`.
fn compiles(dir: &Path, text: &str) {
    let found = Command::new("nim")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !found {
        eprintln!("no Nim compiler on the path: the generated module was not checked");
        return;
    }
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../runtime/nim")
        .canonicalize()
        .expect("runtime/nim");
    let file = dir.join("main.nim");
    std::fs::write(&file, text).expect("the module");
    let output = Command::new("nim")
        .args(["check", "--hints:off", "--warnings:off"])
        .arg(format!("--path:{}", runtime.display()))
        .arg(format!("--nimcache:{}", dir.join("nimcache").display()))
        .arg(&file)
        .current_dir(dir)
        .output()
        .expect("the Nim compiler runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "the generated module is not Nim:\n{stderr}\n--- source ---\n{text}"
    );
}

#[test]
fn an_unsigned_long_long_label_above_int64_stays_in_its_own_width() {
    let dir = workspace();
    let text = reader(
        &dir,
        "union Choice switch (unsigned long long) {\n\
         \x20   case 16045690984833335998: long whole;\n\
         \x20   case 7: double fraction;\n\
         };\n\
         @final struct Sample { Choice picked; long tail; };",
        "Sample",
    );
    assert!(
        text.contains("v.d == 16045690984833335998'u64"),
        "the label did not keep its width:\n{text}"
    );
    assert!(
        !text.contains("16045690984833335998'i64"),
        "the label was widened to a literal Nim refuses:\n{text}"
    );
    compiles(&dir, &text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn both_ends_of_long_long_survive_the_printer() {
    let dir = workspace();
    let text = reader(
        &dir,
        "union Choice switch (long long) {\n\
         \x20   case -9223372036854775808: long low;\n\
         \x20   case 9223372036854775807: double high;\n\
         };\n\
         @final struct Sample { Choice picked; long tail; };",
        "Sample",
    );
    compiles(&dir, &text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_enumeration_label_still_compares_through_zd_i64() {
    let dir = workspace();
    let text = reader(
        &dir,
        "enum Colour { RED, GREEN, BLUE };\n\
         union Choice switch (Colour) {\n\
         \x20   case GREEN: long whole;\n\
         \x20   default: double other;\n\
         };\n\
         @final struct Sample { Choice picked; long tail; };",
        "Sample",
    );
    assert!(
        text.contains("zdwire.zdI64(v.d) == 1'i64"),
        "the enumeration label left its widening behind:\n{text}"
    );
    compiles(&dir, &text);
    let _ = std::fs::remove_dir_all(&dir);
}
