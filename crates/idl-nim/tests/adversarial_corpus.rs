// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Adversarial corpus, gated on the real Nim toolchain (`nim` on PATH; skipped
//! otherwise — e.g. macOS dev boxes, where `invariants.rs` still runs). Every
//! generated snippet is type-checked with `nim check` (semantic pass, no C
//! backend needed); one representative struct is additionally round-tripped with
//! `nim c -r`.
//!
//! Three corpora, each batched into as few `nim` invocations as possible so a
//! shared verify host is not saturated:
//!   1. reserved-keyword corpus — every Nim keyword that is a legal IDL
//!      identifier, placed at a member / struct / enum / module / const /
//!      union-branch position (one bundled module per position);
//!   2. construct corpus — every IDL construct minimally, each in its own
//!      wrapper module, bundled into one module;
//!   3. compose-multifile — two IDLs generated separately, merged idiomatically
//!      (two Nim modules imported by a third) and checked together.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_nim::{NimGenOptions, generate_nim_module};

/// The Nim keywords (mirrors `crates/idl-nim/src/keywords.rs`).
const NIM_RESERVED: &[&str] = &[
    "addr",
    "and",
    "as",
    "asm",
    "bind",
    "block",
    "break",
    "case",
    "cast",
    "concept",
    "const",
    "continue",
    "converter",
    "defer",
    "discard",
    "distinct",
    "div",
    "do",
    "elif",
    "else",
    "end",
    "enum",
    "except",
    "export",
    "finally",
    "for",
    "from",
    "func",
    "if",
    "import",
    "in",
    "include",
    "interface",
    "is",
    "isnot",
    "iterator",
    "let",
    "macro",
    "method",
    "mixin",
    "mod",
    "nil",
    "not",
    "notin",
    "object",
    "of",
    "or",
    "out",
    "proc",
    "ptr",
    "raise",
    "ref",
    "return",
    "shl",
    "shr",
    "static",
    "template",
    "try",
    "tuple",
    "type",
    "using",
    "var",
    "when",
    "while",
    "xor",
    "yield",
];

fn nim_available() -> bool {
    Command::new("nim").arg("--version").output().is_ok()
}

fn parses(src: &str) -> bool {
    zerodds_idl::parse(src, &ParserConfig::default()).is_ok()
}

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_nim_module(&ast, &NimGenOptions::default()).expect("gen")
}

/// A fresh per-test scratch dir, removed on drop.
struct Scratch(PathBuf);
impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("idlnim_adv_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        Scratch(dir)
    }
    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, contents).expect("write");
        p
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Type-checks `file` (following its imports) with `nim check`.
fn nim_check(dir: &Path, file: &Path, ctx: &str) {
    let out = Command::new("nim")
        .args(["check", "--hints:off", "--warnings:off", "--nimcache:nimc"])
        .arg(file)
        .current_dir(dir)
        .output()
        .expect("run nim check");
    assert!(
        out.status.success(),
        "nim check failed ({ctx}):\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
}

/// Every Nim keyword that is also a legal bare IDL identifier (IDL keywords such
/// as `const`/`enum`/`case` are excluded — they cannot name an IDL member).
fn idl_legal_keywords() -> Vec<&'static str> {
    NIM_RESERVED
        .iter()
        .copied()
        .filter(|kw| parses(&format!("@final struct T {{ long {kw}; }};")))
        .collect()
}

#[test]
fn reserved_keywords_at_every_position_compile() {
    if !nim_available() {
        eprintln!("SKIP reserved_keywords_at_every_position_compile: `nim` not on PATH");
        return;
    }
    let kws = idl_legal_keywords();
    assert!(
        kws.len() > 20,
        "expected most Nim keywords legal in IDL: {kws:?}"
    );
    let scratch = Scratch::new("kw");

    // One bundled IDL per position; each generated to its own Nim module and
    // checked once (6 `nim check` runs total).
    let mut positions: Vec<(&str, String)> = Vec::new();

    // member position
    let mut members = String::new();
    for (i, kw) in kws.iter().enumerate() {
        members.push_str(&format!("@final struct KwMem{i} {{ long {kw}; }};\n"));
    }
    positions.push(("member", members));

    // struct-name position (skip keywords the IDL also reserves as type names)
    let mut names = String::new();
    for kw in &kws {
        let idl = format!("@final struct {kw} {{ long a; }};");
        if parses(&idl) {
            names.push_str(&idl);
            names.push('\n');
        }
    }
    positions.push(("struct_name", names));

    // enum-name + enumerator position
    let mut enums = String::new();
    for (i, kw) in kws.iter().enumerate() {
        let idl = format!("enum {kw} {{ EK{i}_A, EK{i}_B }};");
        if parses(&idl) {
            enums.push_str(&idl);
            enums.push('\n');
        }
    }
    positions.push(("enum_name", enums));

    // module-name position
    let mut mods = String::new();
    for (i, kw) in kws.iter().enumerate() {
        let idl = format!("module {kw} {{ @final struct Mk{i} {{ long a; }}; }};");
        if parses(&idl) {
            mods.push_str(&idl);
            mods.push('\n');
        }
    }
    positions.push(("module_name", mods));

    // const-name position
    let mut consts = String::new();
    for kw in &kws {
        let idl = format!("const long {kw} = 1;");
        if parses(&idl) {
            consts.push_str(&idl);
            consts.push('\n');
        }
    }
    positions.push(("const_name", consts));

    // union-branch (member) position
    let mut branches = String::new();
    for (i, kw) in kws.iter().enumerate() {
        let idl = format!("@final union UK{i} switch(long) {{ case 0: long {kw}; }};");
        if parses(&idl) {
            branches.push_str(&idl);
            branches.push('\n');
        }
    }
    positions.push(("union_branch", branches));

    for (pos, idl) in positions {
        assert!(!idl.is_empty(), "no legal keywords for position `{pos}`");
        let nim = emit(&idl);
        let f = scratch.write(&format!("kw_{pos}.nim"), &nim);
        nim_check(&scratch.0, &f, pos);
    }
}

/// Every IDL construct, minimally, each namespaced in its own wrapper module and
/// bundled into a single Nim module checked once.
#[test]
fn every_construct_compiles() {
    if !nim_available() {
        eprintln!("SKIP every_construct_compiles: `nim` not on PATH");
        return;
    }
    let constructs: &[&str] = &[
        "@final struct Sf { long a; double b; string s; wstring w; boolean f; char c; octet o; };",
        "@appendable struct Sa { long a; };",
        "@mutable struct Sm { @must_understand long a; long b; };",
        "@final struct Base { long a; }; @final struct Mid : Base { long b; }; \
         @final struct Der : Mid { long c; };",
        "enum Ev { A, @value(7) B, C };",
        "const long CL = 1 + 2; const unsigned long long CULL = 42; const float CF = 1.5; \
         const double CD = 3.14; const boolean CB = TRUE; const char CC = 'x'; \
         const string CS = \"hi\"; const octet CO = 3;",
        "@final union Ui switch(long) { case 1: long a; case 2: double b; default: octet d; };",
        "enum Color { RED, GREEN, BLUE }; \
         @final union Ue switch(Color) { case RED: long r; case GREEN: long g; default: long d; };",
        "enum Two { LO, HI }; @final union Ux switch(Two) { case LO: long a; case HI: long b; };",
        "@final union Uc switch(char) { case 'A': long a; case 'B': long b; };",
        "@final union Ub switch(boolean) { case TRUE: long t; case FALSE: long f; };",
        "@mutable union Um switch(long) { case 1: long a; case 2: double b; };",
        "bitset Bs { bitfield<3> a; bitfield<5> b; };",
        "@bit_bound(16) bitmask Bm { FLAG_A, FLAG_B, FLAG_C };",
        "@final struct So { @optional long a; long b; };",
        "@final struct Sx { fixed<9,2> price; };",
        "@final struct Sq { sequence<long> xs; sequence<octet,4> raw; };",
        "@final struct Sr { long grid[2][3]; };",
        "@final struct Sp { map<long, double> m; };",
        "module na { module nb { @final struct C { long x; }; }; };",
        "module Mr { @final struct A { long x; }; }; module Mr { @final struct B { long y; }; };",
        "module A_B { @final struct C { long x; }; }; \
         module A { module B { @final struct C { long y; }; }; };",
        "@final struct Scf { long my_field; long myField; };",
        "interface I { struct Nested { long n; }; };",
    ];
    let mut bundle = String::new();
    for (i, c) in constructs.iter().enumerate() {
        bundle.push_str(&format!("module c{i} {{ {c} }};\n"));
    }
    assert!(parses(&bundle), "construct bundle must parse");
    let nim = emit(&bundle);
    let scratch = Scratch::new("construct");
    let f = scratch.write("constructs.nim", &nim);
    nim_check(&scratch.0, &f, "constructs");
}

/// A representative struct (every primitive + a nested composite) round-trips
/// through `marshalXCDR`/`unmarshalXCDR` for both endiannesses.
#[test]
fn representative_struct_round_trips() {
    if !nim_available() {
        eprintln!("SKIP representative_struct_round_trips: `nim` not on PATH");
        return;
    }
    let mut src = emit(
        "@final struct Inner { long v; }; \
         @final struct Outer { uint32 id; int16 s; float f; double d; boolean b; \
                               string label; sequence<octet> raw; Inner nested; };",
    );
    src.push_str(
        r#"
when isMainModule:
  var o: Outer
  o.id = 0xDEADBEEF'u32
  o.s = -7'i16
  o.f = 1.5'f32
  o.d = 2.5'f64
  o.b = true
  o.label = "hi"
  o.raw = @[1'u8, 2'u8, 3'u8]
  o.nested.v = 42'i32
  for en in [eLE, eBE]:
    let bytes = o.marshalXCDR(en)
    let back = unmarshalXCDROuter(bytes, en)
    doAssert back.id == o.id
    doAssert back.s == o.s
    doAssert back.f == o.f
    doAssert back.d == o.d
    doAssert back.b == o.b
    doAssert back.label == o.label
    doAssert back.raw == o.raw
    doAssert back.nested.v == o.nested.v
  echo "ok"
"#,
    );
    let scratch = Scratch::new("rt");
    let f = scratch.write("main.nim", &src);
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&f)
        .current_dir(&scratch.0)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
}

/// compose-multifile: two IDLs generated to separate self-contained Nim modules,
/// imported by a third that instantiates a type from each — the idiomatic Nim
/// composition. `nim check main` type-checks all three together.
#[test]
fn compose_multifile_two_modules() {
    if !nim_available() {
        eprintln!("SKIP compose_multifile_two_modules: `nim` not on PATH");
        return;
    }
    let a = emit("module alpha { @final struct Shared { long a; string s; }; };");
    let b = emit("module beta { @final struct Shared { double x; sequence<long> ys; }; };");
    // Both generated modules export the shared wire prelude (`Endian`, `eLE`,
    // …); main qualifies those duplicated symbols and lets overload resolution
    // pick each `marshalXCDR` by its receiver type. The distinct data types
    // (`alpha_Shared` / `beta_Shared`) are already unambiguous.
    let main = r#"import gen_a
import gen_b

var sa: alpha_Shared
sa.a = 1'i32
sa.s = "x"
var sb: beta_Shared
sb.x = 2.5'f64
sb.ys = @[3'i32]
discard sa.marshalXCDR(gen_a.eLE)
discard sb.marshalXCDR(gen_b.eBE)
"#;
    let scratch = Scratch::new("compose");
    scratch.write("gen_a.nim", &a);
    scratch.write("gen_b.nim", &b);
    let m = scratch.write("main.nim", main);
    nim_check(&scratch.0, &m, "compose");
}
