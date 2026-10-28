// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! L2 codegen compile check.
//!
//! Generates the C declarations for V-1..V-12 of
//! `zerodds-xcdr2-bindings-conformance-1.0`, writes them to a temporary
//! file, and compiles a translation unit that includes them under
//! `-std=c99 -Wall -Werror`. `xcdr2_c_codegen.rs` reads individual lines
//! out of the same output; this reads none of them and asks a C
//! compiler instead, which is the only check that covers the emit sites
//! no `contains` assertion happens to name.
//!
//! The generated header is self-contained: the prelude declares
//! `zd_string`, `zd_wstring` and the `zd_seq_*` element types it uses,
//! so nothing but `<stdbool.h>`, `<stddef.h>` and `<stdint.h>` is
//! needed. `idl-cpp`'s header referenced `zerodds_xcdr2.h` for its
//! TypeSupport table and that file had to be staged alongside it; the
//! rebuilt backend emits no TypeSupport table (see
//! `xcdr2_c_codegen.rs`'s module doc), so it no longer does.
//!
//! Without a C compiler on PATH (`cc`/`gcc`/`clang`) the test reports
//! the skip and passes.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_lang_cst::c;
use zerodds_lang_cst::common::{Doc, Ident};
use zerodds_lang_rules::adapter::c::c_items;
use zerodds_lang_rules::rule;
use zerodds_lang_rules::{ModelGraph, apply};

fn gen_c(src: &str) -> String {
    let parsed = zerodds_idl_parse::parse(src);
    assert!(!parsed.has_errors(), "parse: {:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "conformance.idl");
    assert!(
        lowered.diagnostics.is_empty(),
        "lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.is_clean(), "unresolved: {:?}", report.broken);

    let model = ModelGraph::of(&ast);
    let applied = apply(&model, &rule::c::catalogue()).expect("apply the C rule catalogue");
    let decls = c_items(&ast, &applied).expect("materialise the C declarations");

    zerodds_lang_render::c::file(&c::File {
        doc: Doc::lines(&["Generated for the XCDR2 C binding compile test."]),
        guard: Some(Ident::new("ZD_CONFORMANCE_H")),
        includes: vec![
            c::Include::system("stdbool.h"),
            c::Include::system("stddef.h"),
            c::Include::system("stdint.h"),
        ],
        decls,
    })
}

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
        eprintln!("skip {name}: no C compiler on PATH");
        return true;
    };
    let header = gen_c(idl);

    let tmp = env::temp_dir().join(format!("zerodds_xcdr2_c_{name}"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("generated.h"), header).unwrap();
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
        // The generated header of a map includes `zerodds_map.h` (XTypes 1.3 7.5.1.3.2).
        .arg(format!(
            "-I{}",
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../runtime/c/include")
                .display()
        ))
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
    // An empty aggregate has no C form and the materialiser refuses it,
    // so there is nothing here to compile.
    //
    // This test *was* green, and that is the part worth recording: it
    // builds with `-std=c99 -Wall -Werror` and **not** `-Wpedantic`, and
    // gcc and clang both accept `struct Empty { };` as an extension.
    // C23 §6.7.2.1p8 leaves it undefined. The wire probe that produces
    // the measured C column builds with `-Wpedantic -Werror`
    // (`tools/wireprobe/wireprobe-zerodds-c`), where the same header is
    // an error — so the case compiled here and could never be measured.
    //
    // A header that builds only under a compiler extension is not a C
    // form an interop project can offer. The refusal is asserted
    // instead, and `xcdr2_c_codegen.rs::v1_codegen_empty_final` holds
    // the same line one layer up.
    let refused = std::panic::catch_unwind(|| try_compile("v1", "@final struct Empty {};"));
    assert!(
        refused.is_err(),
        "C has no empty aggregate; the materialiser has to refuse before a \
         compiler is asked"
    );
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
        "@final struct Tags { sequence<string> labels; };"
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

#[test]
fn v12_keyword_identifiers_compiles() {
    // `for`, `register`, `static` and `int` are legal IDL identifiers and
    // C keywords; the escape has to reach the structure tag, the member
    // names and the union branch alike, which `-Werror` decides and a
    // `contains` assertion cannot.
    assert!(try_compile(
        "v12",
        "union U switch (long) { case 0: long for; case 1: long while_val; }; \
         @final struct int { long register; long static; U discriminated; };",
    ));
}
