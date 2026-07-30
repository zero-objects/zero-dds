// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Adversarial corpus for the Lua backend, gated on a real Lua toolchain
//! (`luac`/`luac5.4` for the byte-compile check, `lua`/`lua5.4` for the
//! round-trip). Each generated module is compiled with the target compiler, so
//! a syntactically invalid emission (an unescaped keyword, a bare `TRUE`, an
//! `L\"…\"` literal, a dropped `end`) fails loudly instead of only being caught
//! by string matching.
//!
//! Three sweeps, matching the IDL-construct fix-campaign test gate:
//!  1. reserved-keyword corpus — every Lua reserved word in each IDL identifier
//!     position (member / struct / enum / enumerator / module / const /
//!     union-branch);
//!  2. construct corpus — every IDL construct minimally, each compiled;
//!  3. compose-multifile — two IDLs generated separately, idiomatically merged
//!     (shared wire runtime deduplicated), compiled as one unit.
//!
//! On a host without a Lua toolchain the sweeps print `SKIP` and pass (mirrors
//! the crate's existing `golden.rs` gating).

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::unwrap_used
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_lua::{LuaGenOptions, generate_lua_module};

const LUA_RESERVED: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_lua_module(&ast, &LuaGenOptions::default()).expect("gen")
}

fn try_emit(src: &str) -> Option<String> {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).ok()?;
    generate_lua_module(&ast, &LuaGenOptions::default()).ok()
}

/// The Lua byte-compiler on PATH, if any (`luac5.4` first — the version the
/// crate targets — then a bare `luac`; any 5.x compiles the emitted subset).
fn lua_compiler() -> Option<&'static str> {
    ["luac5.4", "luac"]
        .into_iter()
        .find(|cmd| Command::new(cmd).arg("-v").output().is_ok())
}

/// The Lua interpreter on PATH, if any (for the round-trip sweep).
fn lua_interpreter() -> Option<&'static str> {
    ["lua5.4", "lua"]
        .into_iter()
        .find(|cmd| Command::new(cmd).arg("-v").output().is_ok())
}

fn tmp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "idllua_adv_{tag}_{}_{}.lua",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ))
}

/// Byte-compiles `lua` with `luac -p` (parse/compile only). Panics with the
/// source + compiler stderr on failure.
fn compile(compiler: &str, tag: &str, lua: &str) {
    let path = tmp_path(tag);
    std::fs::write(&path, lua).expect("write");
    let out = Command::new(compiler)
        .arg("-p")
        .arg(&path)
        .output()
        .expect("run luac");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "luac rejected `{tag}`:\n{}\n--- src ---\n{lua}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Runs `lua` (module + driver) and asserts a clean exit.
fn run(interp: &str, tag: &str, lua: &str) {
    let path = tmp_path(tag);
    std::fs::write(&path, lua).expect("write");
    let out = Command::new(interp).arg(&path).output().expect("run lua");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "lua run `{tag}` failed:\n{}\n--- src ---\n{lua}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// -------------------------------------------------------------------------
// 1. reserved-keyword corpus
// -------------------------------------------------------------------------

#[test]
fn reserved_keyword_corpus_compiles() {
    let Some(luac) = lua_compiler() else {
        eprintln!("SKIP reserved_keyword_corpus_compiles: no `luac` on PATH");
        return;
    };
    let mut compiled = 0;
    for &kw in LUA_RESERVED {
        // Every identifier position the emitter controls, all named with `kw`:
        // module, struct, member, enum, enumerator, const, union-branch field.
        let src = format!(
            "module {kw} {{ enum E_{kw} {{ {kw} }}; }};
@final struct {kw} {{ long {kw}; }};
const long {kw}_c = 1;
union U_{kw} switch (long) {{ case 1: long {kw}; default: short other; }};"
        );
        // `in`/`local` are IDL keywords too -> never reach the emitter as an
        // identifier; skip the ones that do not parse.
        let Some(lua) = try_emit(&src) else {
            continue;
        };
        compile(luac, &format!("kw_{kw}"), &lua);
        compiled += 1;
    }
    assert!(
        compiled >= 18,
        "only {compiled} reserved-keyword specs compiled (parser regression?)"
    );
}

// -------------------------------------------------------------------------
// 2. construct corpus
// -------------------------------------------------------------------------

/// Minimal IDL for each construct the backend must emit, each compiled on its
/// own (so a defect isolates to one construct).
fn construct_corpus() -> Vec<(&'static str, &'static str)> {
    vec![
        ("fixed", "@final struct S { fixed<7,3> price; };"),
        (
            "enum_value",
            "enum E { A, @value(10) B, C }; @final struct S { E e; };",
        ),
        (
            "const_all_types",
            "const octet O = 5;
const short SH = -3;
const long L = 0x7F;
const long long LL = 123456789;
const unsigned long UL = 42;
const float F = 1.5;
const double D = 2.25;
const boolean B = FALSE;
const char C = 'x';
const wchar WC = L'y';
const string STR = \"s\";
const wstring WS = L\"w\";",
        ),
        (
            "struct_inheritance",
            "@final struct Base { long a; long b; };
@final struct Mid : Base { long c; };
@final struct Leaf : Mid { long d; };",
        ),
        (
            "union_disc_integer",
            "union U switch (long) { case 1: long a; case 2: short b; default: octet o; };",
        ),
        (
            "union_disc_enum",
            "enum Kind { K0, K1, @value(5) K2 };
union U switch (Kind) { case K0: long a; case K2: double d; default: short s; };",
        ),
        (
            "union_disc_char",
            "union U switch (char) { case 'A': long a; case 'z': short b; default: octet o; };",
        ),
        (
            "union_disc_boolean",
            "union U switch (boolean) { case TRUE: long a; default: short b; };",
        ),
        ("bitset", "bitset B { bitfield<3> a; bitfield<10> b; };"),
        (
            "bitmask",
            "@bit_bound(16) bitmask M { R, G, @position(7) B };",
        ),
        (
            "optional_extensibility",
            "@final struct F { @optional long a; long b; };
@appendable struct A { @optional long a; long b; };
@mutable struct Mu { @must_understand long a; @optional long b; };",
        ),
        (
            "mutable_union",
            "@mutable union U switch (long) { case 1: long a; case 2: double b; };",
        ),
        (
            "sequence",
            "@final struct S { sequence<long> a; sequence<string> b; };",
        ),
        ("array_multidim", "@final struct S { long grid[2][3][4]; };"),
        (
            "map",
            "@final struct S { map<long, string> a; map<string, double, 4> b; };",
        ),
        (
            "module_nested_reopened",
            "module A { module B { @final struct C { long v; }; }; };
module A { @final struct D { long w; }; };",
        ),
        (
            "module_flatten_injective",
            "module X_Y { @final struct Z { long v; }; };
module X { module Y { @final struct Z { long v; }; }; };",
        ),
        (
            "interface_nested_type",
            "interface I { struct Payload { long v; }; };",
        ),
        (
            "typedef",
            "typedef long Score; typedef sequence<long> Nums;
@final struct S { Score sc; Nums ns; };",
        ),
    ]
}

#[test]
fn construct_corpus_compiles() {
    let Some(luac) = lua_compiler() else {
        eprintln!("SKIP construct_corpus_compiles: no `luac` on PATH");
        return;
    };
    for (tag, src) in construct_corpus() {
        let lua = emit(src);
        compile(luac, tag, &lua);
    }
}

/// A representative construct set round-trips through the real interpreter (not
/// just compiles): encode a value and decode it back, asserting field equality
/// for a struct with inheritance, an integer union, and a mutable struct.
#[test]
fn representative_constructs_round_trip() {
    let Some(lua) = lua_interpreter() else {
        eprintln!("SKIP representative_constructs_round_trip: no `lua` on PATH");
        return;
    };
    let module = emit(
        "@final struct Base { long a; long b; };
@final struct Derived : Base { long c; };
union U switch (long) { case 1: long x; case 2: short y; default: octet o; };
@mutable struct Mu { @must_understand long m; long n; };",
    );
    let driver = r#"
-- struct inheritance: all of a,b,c survive encode+decode
local d = { a = 11, b = 22, c = 33 }
local rd = unmarshal_Derived(marshal_Derived(d, LE), LE)
assert(rd.a == 11 and rd.b == 22 and rd.c == 33, "derived roundtrip")
-- integer union
local u = { disc = 2, y = 7 }
local ru = unmarshal_U(marshal_U(u, LE), LE)
assert(ru.disc == 2 and ru.y == 7, "union roundtrip")
-- mutable struct
local m = { m = 5, n = 6 }
local rm = unmarshal_Mu(marshal_Mu(m, LE), LE)
assert(rm.m == 5 and rm.n == 6, "mutable roundtrip")
print("ok")
"#;
    run(lua, "roundtrip", &format!("{module}\n{driver}"));
}

// -------------------------------------------------------------------------
// 3. compose-multifile
// -------------------------------------------------------------------------

/// Two IDL units generated independently, then idiomatically merged: the shared
/// wire runtime (emitted once per file) is deduplicated and only the type
/// definitions of the second file are appended, so the combined module compiles
/// and exposes both files' marshallers.
#[test]
fn compose_multifile_compiles() {
    let Some(luac) = lua_compiler() else {
        eprintln!("SKIP compose_multifile_compiles: no `luac` on PATH");
        return;
    };
    // The shared runtime prelude = an empty spec (header + wire core, no types).
    let prelude = emit("");
    let file_a = emit(
        "enum ColorA { RED, GREEN, BLUE };
@final struct Alpha { long id; ColorA c; sequence<long> vals; };",
    );
    let file_b = emit(
        "@final struct BaseB { long a; };
@final struct Beta : BaseB { long b; map<long,string> m; };
union GammaB switch (long) { case 1: long x; default: short y; };",
    );

    // Idiomatic merge: keep file A whole; strip file B's duplicated runtime.
    let b_types = file_b
        .strip_prefix(prelude.as_str())
        .expect("generated files share a byte-identical runtime prelude");
    let merged = format!("{file_a}\n{b_types}");

    // Both files' entry points must be present in the single merged unit.
    for sym in [
        "function marshal_Alpha(",
        "function marshal_Beta(",
        "function marshal_GammaB(",
    ] {
        assert!(
            merged.contains(sym),
            "merged unit missing `{sym}`:\n{merged}"
        );
    }
    compile(luac, "compose", &merged);
}

/// The merged compose unit also round-trips through the interpreter, proving the
/// two independently generated files interoperate at run time (inherited fields
/// included).
#[test]
fn compose_multifile_round_trips() {
    let Some(lua) = lua_interpreter() else {
        eprintln!("SKIP compose_multifile_round_trips: no `lua` on PATH");
        return;
    };
    let prelude = emit("");
    let file_a = emit("@final struct Alpha { long id; long tag; };");
    let file_b = emit(
        "@final struct BaseB { long a; };
@final struct Beta : BaseB { long b; };",
    );
    let b_types = file_b
        .strip_prefix(prelude.as_str())
        .expect("shared prelude");
    let merged = format!("{file_a}\n{b_types}");
    let driver = r#"
local a = unmarshal_Alpha(marshal_Alpha({ id = 1, tag = 2 }, LE), LE)
assert(a.id == 1 and a.tag == 2, "alpha")
local b = unmarshal_Beta(marshal_Beta({ a = 9, b = 8 }, LE), LE)
assert(b.a == 9 and b.b == 8, "beta inherited")
print("ok")
"#;
    run(lua, "compose_rt", &format!("{merged}\n{driver}"));
}
