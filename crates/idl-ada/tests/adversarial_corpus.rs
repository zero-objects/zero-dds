// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Adversarial corpus for the Ada backend, gated on a real GNAT toolchain
//! (`gnatmake` on PATH — skips cleanly otherwise, e.g. on macOS/CI without
//! GNAT). Every case generates Ada and compiles it with `gnatmake`:
//!
//! 1. **reserved-keyword corpus** — each Ada 2012 reserved word used as an IDL
//!    identifier at struct / member / enum / enumerator / union-branch / const
//!    / module positions must generate compilable Ada (the `_Id` escaping).
//! 2. **construct corpus** — every IDL construct minimally exercised (fixed,
//!    enum `@value`, `const` of each type, struct inheritance, unions over
//!    every discriminator kind, bitset, bitmask, `@optional`/`@extensibility`,
//!    sequence, multi-dim array, map, nested + reopened modules, interface).
//! 3. **compose-multifile** — two IDLs generated into two Ada packages and
//!    merged idiomatically (a `main` that `with`s both) must compile together.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_ada::{AdaGenOptions, generate_ada_module};

/// Ada 2012 reserved words (RM §2.9). Kept in step with `keywords::ADA_RESERVED`
/// (that list is crate-private; this integration test re-declares it).
const ADA_RESERVED: &[&str] = &[
    "abort",
    "abs",
    "abstract",
    "accept",
    "access",
    "aliased",
    "all",
    "and",
    "array",
    "at",
    "begin",
    "body",
    "case",
    "constant",
    "declare",
    "delay",
    "delta",
    "digits",
    "do",
    "else",
    "elsif",
    "end",
    "entry",
    "exception",
    "exit",
    "for",
    "function",
    "generic",
    "goto",
    "if",
    "in",
    "interface",
    "is",
    "limited",
    "loop",
    "mod",
    "new",
    "not",
    "null",
    "of",
    "or",
    "others",
    "out",
    "overriding",
    "package",
    "parallel",
    "pragma",
    "private",
    "procedure",
    "protected",
    "raise",
    "range",
    "record",
    "rem",
    "renames",
    "requeue",
    "return",
    "reverse",
    "select",
    "separate",
    "some",
    "subtype",
    "synchronized",
    "tagged",
    "task",
    "terminate",
    "then",
    "type",
    "until",
    "use",
    "when",
    "while",
    "with",
    "xor",
];

fn gnat_available() -> bool {
    Command::new("gnatmake").arg("--version").output().is_ok()
}

/// Generates the package and compiles it with `gnatmake -c` (body + spec, no
/// main needed). Returns `Ok(())` on a clean compile, `Err(log)` otherwise.
fn compile_pkg(idl: &str, pkg: &str, stem: &str) -> Result<(), String> {
    let ast =
        zerodds_idl::parse(idl, &ParserConfig::default()).map_err(|e| format!("parse: {e:?}"))?;
    let m = generate_ada_module(
        &ast,
        &AdaGenOptions {
            package_name: pkg.to_string(),
            xcdr1: false,
        },
    )
    .map_err(|e| format!("gen: {e:?}"))?;
    let dir = std::env::temp_dir().join(format!("idlada_adv_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let low = pkg.to_lowercase();
    std::fs::write(dir.join(format!("{low}.ads")), &m.spec).expect("ads");
    std::fs::write(dir.join(format!("{low}.adb")), &m.body).expect("adb");
    let out = Command::new("gnatmake")
        .args(["-c", "-f", "-q", &format!("{low}.adb")])
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    let res = if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
            String::from_utf8_lossy(&out.stderr),
            m.spec,
            m.body
        ))
    };
    let _ = std::fs::remove_dir_all(&dir);
    res
}

/// `true` if `word` is a legal IDL identifier (some Ada reserved words are also
/// IDL keywords — `case`, `const`, `in`, `interface`, `out`, `private`,
/// `abstract` — and cannot be an IDL identifier; those are skipped).
fn valid_idl_ident(word: &str) -> bool {
    zerodds_idl::parse(
        &format!("struct S_probe {{ long {word}; }};"),
        &ParserConfig::default(),
    )
    .is_ok()
}

#[test]
fn reserved_keyword_corpus_compiles() {
    if !gnat_available() {
        eprintln!("SKIP reserved_keyword_corpus_compiles: `gnatmake` not on PATH");
        return;
    }
    let mut tested = 0usize;
    for kw in ADA_RESERVED {
        if !valid_idl_ident(kw) {
            continue;
        }
        // Package A: the keyword as struct name + member, union branch, const,
        // and nested-module member. (A bare-keyword enumerator escapes to the
        // same package-visible `<kw>_Id` symbol as a bare-keyword struct type,
        // so the enumerator position lives in its own package B — mirroring
        // that an enumerator and a struct of the same name in one IDL scope is
        // itself a redefinition.)
        let idl_a = format!(
            "struct {kw} {{ long {kw}; }};\n\
             union {kw}_u switch (long) {{ case 1: long {kw}; default: long fallback; }};\n\
             const long {kw}_c = 1;\n\
             module {kw}_m {{ struct Inner {{ long {kw}; }}; }};"
        );
        if let Err(log) = compile_pkg(&idl_a, &format!("Kw_{kw}"), &format!("kw_{kw}")) {
            panic!(
                "reserved word `{kw}` (struct/member/branch/const/module) produced non-compiling Ada:\n{log}"
            );
        }
        // Package B: the keyword as an enum name and as an enumerator.
        let idl_b = format!("enum {kw}_e {{ {kw} }};");
        if let Err(log) = compile_pkg(&idl_b, &format!("Kwe_{kw}"), &format!("kwe_{kw}")) {
            panic!("reserved word `{kw}` (enumerator) produced non-compiling Ada:\n{log}");
        }
        tested += 1;
    }
    assert!(
        tested >= 60,
        "expected most reserved words tested, got {tested}"
    );
}

/// Every IDL construct, minimally, that the Ada backend must compile.
fn construct_corpus() -> Vec<(&'static str, &'static str)> {
    vec![
        ("fixed", "@final struct C { fixed<10,2> amount; };"),
        (
            "enum_value",
            "enum E { @value(5) A, @value(10) B, @value(11) C };",
        ),
        (
            "const_all_types",
            "const long I = -3; const octet O = 3; const float F = 3.14; \
             const double D = 2.5; const boolean B = TRUE; const char C = 'x'; \
             const string S = \"s\";",
        ),
        (
            "struct_inheritance",
            "@final struct Base { long a; }; @final struct Derived : Base { long b; };",
        ),
        (
            "union_disc_integer",
            "union U switch (long) { case 1: long x; default: long y; };",
        ),
        (
            "union_disc_enum",
            "enum Col { R, G, B }; \
             union U switch (Col) { case R: long x; case G: double y; default: octet z; };",
        ),
        (
            "union_disc_char",
            "union U switch (char) { case 'a': long x; case 'b': long y; };",
        ),
        (
            "union_disc_bool",
            "union U switch (boolean) { case TRUE: long x; default: long y; };",
        ),
        (
            "union_disc_octet",
            "union U switch (octet) { case 1: long x; default: long y; };",
        ),
        (
            "mutable_union",
            "@mutable union U switch (long) { case 1: long x; case 2: double y; };",
        ),
        ("empty_struct", "@final struct Empty {};"),
        (
            "bitset",
            "bitset Flags { bitfield<1> a; bitfield<3> b; bitfield<4> c; };",
        ),
        ("bitmask", "bitmask Perms { R, W, X };"),
        (
            "optional_extensibility",
            "@appendable struct S { @optional long a; long b; };",
        ),
        (
            "sequence",
            "@final struct S { sequence<long> v; sequence<double,4> w; };",
        ),
        ("array_multidim", "@final struct S { long grid[2][3]; };"),
        ("map", "@final struct S { map<long, double> m; };"),
        (
            "module_nested_reopened",
            "module A { module B { @final struct T { long v; }; }; }; \
             module A { @final struct U { long w; }; };",
        ),
        (
            "interface_nested",
            "interface Svc { struct Req { long id; }; const long V = 5; };",
        ),
        (
            "typedef",
            "typedef long Score; @final struct S { Score a; };",
        ),
    ]
}

#[test]
fn construct_corpus_compiles() {
    if !gnat_available() {
        eprintln!("SKIP construct_corpus_compiles: `gnatmake` not on PATH");
        return;
    }
    for (name, idl) in construct_corpus() {
        let pkg = format!("Con_{name}");
        if let Err(log) = compile_pkg(idl, &pkg, &format!("con_{name}")) {
            panic!("construct `{name}` produced non-compiling Ada:\n{log}");
        }
    }
}

#[test]
fn compose_multifile_compiles_together() {
    if !gnat_available() {
        eprintln!("SKIP compose_multifile_compiles_together: `gnatmake` not on PATH");
        return;
    }
    // Two IDLs generated independently into two Ada packages, then merged
    // idiomatically by a `main` that `with`s both and uses a type from each.
    let cfg = ParserConfig::default();
    let a = generate_ada_module(
        &zerodds_idl::parse("@final struct Alpha { long a; };", &cfg).expect("parse a"),
        &AdaGenOptions {
            package_name: "Pkg_A".to_string(),
            xcdr1: false,
        },
    )
    .expect("gen a");
    let b = generate_ada_module(
        &zerodds_idl::parse("@final struct Beta { double b; };", &cfg).expect("parse b"),
        &AdaGenOptions {
            package_name: "Pkg_B".to_string(),
            xcdr1: false,
        },
    )
    .expect("gen b");

    let dir = std::env::temp_dir().join(format!("idlada_compose_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("pkg_a.ads"), &a.spec).expect("a.ads");
    std::fs::write(dir.join("pkg_a.adb"), &a.body).expect("a.adb");
    std::fs::write(dir.join("pkg_b.ads"), &b.spec).expect("b.ads");
    std::fs::write(dir.join("pkg_b.adb"), &b.body).expect("b.adb");
    // Each self-contained package declares its own `Byte_Array`/`Endianness`,
    // so idiomatic composition qualifies both fully rather than `use`-ing them
    // (a `use` of both would make those names ambiguous — the correct Ada way
    // to merge two independent generated units).
    let main = "with Pkg_A;\n\
                with Pkg_B;\n\
                with Ada.Text_IO; use Ada.Text_IO;\n\
                procedure Main is\n\
                   X  : constant Pkg_A.Alpha := (a => 1);\n\
                   Y  : constant Pkg_B.Beta := (b => 2.0);\n\
                   Ba : constant Pkg_A.Byte_Array := Pkg_A.Marshal (X, Pkg_A.Little);\n\
                   Bb : constant Pkg_B.Byte_Array := Pkg_B.Marshal (Y, Pkg_B.Little);\n\
                begin\n\
                   Put_Line (Integer'Image (Ba'Length + Bb'Length));\n\
                end Main;\n";
    std::fs::write(dir.join("main.adb"), main).expect("main");
    let out = Command::new("gnatmake")
        .args(["-f", "main.adb"])
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    let ok = out.status.success();
    let log = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        ok,
        "compose of Pkg_A + Pkg_B failed to compile:\n{log}\n--- a.ads ---\n{}\n--- b.ads ---\n{}",
        a.spec, b.spec
    );
}
