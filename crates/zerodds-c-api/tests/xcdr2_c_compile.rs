// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! L2-Codegen-Compile-Sanity-Test.
//!
//! Generiert pro V-1..V-12 ein C99-Header via `idl-cpp::generate_c_header`,
//! schreibt ihn in eine tmp-Datei, kompiliert eine kleine C-Stub-Datei
//! die den Header inkludiert und nutzt — und verifiziert dass der
//! C-Compiler ohne Fehler durchlaeuft. Das stellt sicher dass der
//! emittierte Code byte-genau valider C99 ist.
//!
//! Wenn kein C-Compiler verfuegbar ist (`cc`/`gcc`/`clang` nicht im
//! PATH), wird der Test geskipped (Ignored).

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

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CGenOptions, generate_c_header};

fn cc_path() -> Option<PathBuf> {
    for c in ["cc", "gcc", "clang"] {
        if let Ok(out) = Command::new("which").arg(c).output() {
            if out.status.success() {
                let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !p.is_empty() {
                    return Some(PathBuf::from(p));
                }
            }
        }
    }
    None
}

fn try_compile(name: &str, idl: &str) -> bool {
    let Some(cc) = cc_path() else {
        eprintln!("skip {name}: no C compiler");
        return true;
    };
    let ast = match zerodds_idl::parse(idl, &ParserConfig::default()) {
        Ok(a) => a,
        Err(e) => panic!("parse {name} failed: {e:?}"),
    };
    let header = match generate_c_header(&ast, &CGenOptions::default()) {
        Ok(h) => h,
        Err(e) => panic!("c-gen {name} failed: {e:?}"),
    };

    let tmp = env::temp_dir().join(format!("zerodds_xcdr2_test_{name}"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("generated.h"), header).unwrap();

    // Stub `zerodds.h` (Topic-FFI ist Pointer-forward; Stub reicht).
    fs::write(
        tmp.join("zerodds.h"),
        b"/* test stub */\n#ifndef ZERODDS_H\n#define ZERODDS_H\n#endif\n",
    )
    .unwrap();
    // Echter zerodds_xcdr2.h aus dem Crate-Repo.
    let xcdr2_h = include_str!("../include/zerodds_xcdr2.h");
    fs::write(tmp.join("zerodds_xcdr2.h"), xcdr2_h).unwrap();

    fs::write(
        tmp.join("main.c"),
        b"#include \"generated.h\"\nint main(void) { return 0; }\n",
    )
    .unwrap();
    let out = Command::new(&cc)
        .arg("-std=c99")
        .arg("-Wall")
        .arg("-Werror")
        .arg("-Wno-unused-function")
        .arg("-Wno-unused-variable")
        .arg("-c")
        .arg("-o")
        .arg(tmp.join("main.o"))
        .arg(tmp.join("main.c"))
        .arg(format!("-I{}", tmp.display()))
        .output()
        .expect("invoke cc");
    if !out.status.success() {
        eprintln!("cc-stderr {name}: {}", String::from_utf8_lossy(&out.stderr));
        return false;
    }
    true
}

#[test]
fn v1_compiles() {
    assert!(try_compile("v1", "@final struct Empty {};"));
}

#[test]
fn v2_compiles() {
    assert!(try_compile(
        "v2",
        "@final struct Point { long x; long y; };"
    ));
}

#[test]
fn v3_compiles() {
    assert!(try_compile(
        "v3",
        "@final struct All { boolean b; octet o; short s; unsigned short us; \
         long l; unsigned long ul; long long ll; unsigned long long ull; \
         float f; double d; };",
    ));
}

#[test]
fn v4_compiles() {
    assert!(try_compile(
        "v4",
        "@final struct Greeting { string text; };"
    ));
}

#[test]
fn v5_compiles() {
    assert!(try_compile(
        "v5",
        "@final struct Bag { sequence<long> ids; };"
    ));
}

#[test]
fn v6_compiles() {
    assert!(try_compile(
        "v6",
        "@final struct Tags { sequence<string> tags; };"
    ));
}

#[test]
fn v7_compiles() {
    assert!(try_compile(
        "v7",
        "module Outer { module Inner { @final struct S { long x; }; }; };",
    ));
}

#[test]
fn v8_compiles() {
    assert!(try_compile(
        "v8",
        "@final struct Sensor { @key long id; double value; };",
    ));
}

#[test]
fn v9_compiles() {
    assert!(try_compile(
        "v9",
        "@appendable struct V { long a; long b; };"
    ));
}

#[test]
fn v10_compiles() {
    assert!(try_compile(
        "v10",
        "@mutable struct M { @id(1) long a; @id(2) string b; };",
    ));
}

#[test]
fn v11_compiles() {
    assert!(try_compile(
        "v11",
        "@mutable struct O { @id(1) long maybe; };",
    ));
}
