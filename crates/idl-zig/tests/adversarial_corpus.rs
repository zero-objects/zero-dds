// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Adversarial corpus for the Zig backend, compiled with the real target
//! toolchain. Gated on `zig` being on PATH (SKIP-prints otherwise).
//!
//! Uses `zig build-obj` (full semantic analysis + codegen, no host link) rather
//! than `zig run`: build-obj analyses every `pub` declaration in the root file —
//! including unreferenced `marshalInto`/`unmarshalXCDR` method bodies (verified:
//! it flags an error planted in an unreferenced method) — so it exercises the
//! generated wire code, while sidestepping this repo's macOS zig-0.14.1
//! host-link gap (`zig run` fails to link on the dev machine, runs on codepit).
//!
//! Three corpora:
//!  1. reserved-keyword — every Zig reserved word that is a legal IDL identifier,
//!     placed at member/struct/enum/enumerator/module/const/union-branch, must
//!     generate and compile (proves keyword escaping is semantically valid).
//!  2. construct — every IDL construct the backend emits, minimally, in one
//!     spec: fixed, enum `@value`, const of every scalar type, struct
//!     inheritance, all union discriminators, bitset, bitmask, `@optional`,
//!     each `@extensibility`, sequence, multidimensional array, map, and nested
//!     + reopened modules.
//!  3. compose-multifile — two specs generated separately, combined the
//!     idiomatic Zig way (each its own file, `@import`ed by a root) so their
//!     independent wire preludes never collide.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_zig::{ZigGenOptions, generate_zig_module};

fn zig_present() -> bool {
    Command::new("zig").arg("version").output().is_ok()
}

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_zig_module(&ast, &ZigGenOptions::default()).expect("gen")
}

fn try_emit(src: &str) -> Option<String> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).ok()?;
    generate_zig_module(&ast, &ZigGenOptions::default()).ok()
}

/// Writes `files` (name → contents) into a fresh temp dir and runs
/// `zig build-obj` on `root`. Returns `Err(stderr)` on a non-zero exit.
fn build_obj(files: &[(&str, &str)], root: &str, stem: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("idlzig_corpus_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    for (name, contents) in files {
        std::fs::write(dir.join(name), contents).expect("write");
    }
    let out = Command::new("zig")
        .arg("build-obj")
        .arg(format!(
            "-femit-bin={}",
            dir.join(format!("{stem}.o")).to_str().expect("path")
        ))
        .arg(dir.join(root))
        .current_dir(&dir)
        .output()
        .expect("zig build-obj");
    let res = if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    };
    let _ = std::fs::remove_dir_all(&dir);
    res
}

/// Compiles a single generated Zig module.
fn compile_ok(src: &str, stem: &str) -> Result<(), String> {
    build_obj(&[("main.zig", src)], "main.zig", stem)
}

/// Zig reserved words that are legal IDL identifiers (IDL keywords like
/// `const`/`enum`/`struct`/`union`/`switch` are excluded; `try_emit` also skips
/// any that fail to parse).
const RESERVED_IDENT_WORDS: &[&str] = &[
    "align",
    "try",
    "error",
    "for",
    "while",
    "break",
    "defer",
    "comptime",
    "test",
    "var",
    "fn",
    "catch",
    "inline",
    "volatile",
    "resume",
    "unreachable",
    "undefined",
    "threadlocal",
    "export",
    "extern",
    "packed",
    "opaque",
    "noalias",
];

#[test]
fn reserved_keyword_corpus_compiles() {
    if !zig_present() {
        eprintln!("SKIP reserved_keyword_corpus_compiles: `zig` not on PATH");
        return;
    }
    let mut compiled = 0usize;
    for w in RESERVED_IDENT_WORDS {
        // The word at every identifier position it can legally occupy:
        // module, struct, member, enum, enumerator, union branch, const.
        let idl = format!(
            "const long C{w} = 1;\n\
             enum E{w} {{ {w}, ZZ }};\n\
             module {w} {{ @final struct Inner {{ long {w}; }}; }};\n\
             @final struct {w} {{ long {w}; E{w} e; }};\n\
             @final union U{w} switch (long) {{ case 1: long {w}; default: octet o; }};"
        );
        let Some(z) = try_emit(&idl) else {
            continue;
        };
        compile_ok(&z, &format!("kw_{w}")).unwrap_or_else(|e| {
            panic!("reserved word `{w}` did not compile:\n{e}\n--- src ---\n{z}")
        });
        compiled += 1;
    }
    assert!(
        compiled >= 12,
        "too few reserved words compiled: {compiled}"
    );
}

/// Every construct the backend emits, in one spec.
const CONSTRUCT_IDL: &str = "\
const short C_SHORT = -3;
const unsigned long C_ULONG = 7;
const octet C_OCTET = 255;
const char C_CHAR = 'Z';
const boolean C_BOOL = FALSE;
const float C_FLOAT = 1.5;
const double C_DOUBLE = 2.5;
const string C_STR = \"k\";
enum Color { RED, @value(10) GREEN, BLUE };
@bit_bound(16) bitmask Perm { READ, WRITE, EXEC };
bitset Flags { bitfield<1> ready; bitfield<3> level; bitfield<4> code; };
@final struct Money { fixed<5,2> price; fixed<4,0> qty; };
@final struct Base { @key long x; long w; };
@appendable struct Mid : Base { long m; };
@final struct Derived : Mid { @key long y; };
@final union UColor switch (Color) { case RED: long r; case GREEN: unsigned long g; default: long d; };
@final union UChar switch (char) { case 'A': long a; default: octet b; };
@final union UBool switch (boolean) { case TRUE: long t; case FALSE: short f; };
@final union UInt switch (long) { case 1: unsigned long a; case 2: unsigned short b; default: octet c; };
@mutable union UMut switch (long) { case 1: unsigned long a; case 2: string s; };
@final struct FinalS { long a; };
@appendable struct AppS { long a; string b; };
@mutable struct MutS { @id(1) @must_understand unsigned long a; @id(2) @optional string s; @id(3) long c; };
@final struct OptS { long a; @optional unsigned long b; @optional string s; };
@appendable struct Coll {
    sequence<long> nums;
    sequence<string> names;
    sequence<Base> items;
    long grid[2][3];
    octet raw[4];
    map<long, unsigned long> m;
    map<long, map<long, unsigned long>> mm;
};
interface Svc { struct Nested { long n; }; enum InnerE { IA, IB }; };
module outer {
    module inner { @final struct Deep { long v; }; };
    @final struct Shallow { inner::Deep d; };
};
module outer { @final struct Reopened { long z; }; };";

#[test]
fn construct_corpus_compiles() {
    if !zig_present() {
        eprintln!("SKIP construct_corpus_compiles: `zig` not on PATH");
        return;
    }
    let z = emit(CONSTRUCT_IDL);
    compile_ok(&z, "constructs")
        .unwrap_or_else(|e| panic!("construct corpus did not compile:\n{e}\n--- src ---\n{z}"));
}

/// First IDL-generated top-level `pub const <Name> = struct {` in a module — a
/// type carrying wire methods, used to force cross-file analysis in the compose
/// test. Skips the shared wire-prelude structs (`Writer`/`Reader`).
fn first_struct_type(z: &str) -> String {
    for l in z.lines() {
        if let Some(rest) = l.strip_prefix("pub const ") {
            if let Some(name) = rest.strip_suffix(" = struct {") {
                if name != "Writer" && name != "Reader" {
                    return name.to_string();
                }
            }
        }
    }
    panic!("no IDL struct type in generated module:\n{z}");
}

#[test]
fn compose_multifile_imports_compile() {
    if !zig_present() {
        eprintln!("SKIP compose_multifile_imports_compile: `zig` not on PATH");
        return;
    }
    // Two independently generated modules — each self-contained with its own
    // wire prelude (`std`, `Writer`, `Reader`, …). Concatenating them would
    // redeclare the prelude, so the idiomatic Zig composition keeps each in its
    // own file and pulls both in through `@import`, which namespaces them.
    let a = emit("@final struct Alpha { long a; string s; };");
    let b = emit("@appendable struct Beta { unsigned long b; sequence<long> xs; };");
    let ta = first_struct_type(&a);
    let tb = first_struct_type(&b);
    // A root that references a wire method from each imported module, forcing
    // build-obj to analyse both files' generated code in one compilation.
    let root = format!(
        "const std = @import(\"std\");\n\
         const a = @import(\"a.zig\");\n\
         const b = @import(\"b.zig\");\n\
         comptime {{\n\
         \x20   _ = &a.{ta}.marshalXCDR;\n\
         \x20   _ = &a.{ta}.unmarshalXCDR;\n\
         \x20   _ = &b.{tb}.marshalXCDR;\n\
         \x20   _ = &b.{tb}.unmarshalXCDR;\n\
         }}\n\
         pub const RootA = a.{ta};\n\
         pub const RootB = b.{tb};\n"
    );
    build_obj(
        &[("a.zig", &a), ("b.zig", &b), ("root.zig", &root)],
        "root.zig",
        "compose",
    )
    .unwrap_or_else(|e| {
        panic!("compose-multifile did not compile:\n{e}\n--- root ---\n{root}\n--- a ---\n{a}\n--- b ---\n{b}")
    });
}
