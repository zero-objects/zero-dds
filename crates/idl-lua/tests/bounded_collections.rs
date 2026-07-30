// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated Lua
//! codec: a `string<N>` / `wstring<N>` / `sequence<T,N>` / `map<K,V,N>` value
//! longer than its declared bound must be rejected on BOTH encode and decode
//! (untrusted-input DoS vector on decode: before this, a well-formed but
//! oversized wire value decoded without complaint).
//!
//! idl-lua had NO bound enforcement at all before this fix (neither side).
//! The check lives in the shared `Writer`/`Reader` prelude methods
//! (`putString`/`getString`, `putWString`/`getWString`, `putSeqU8`/
//! `getSeqU8`), which now take an optional IDL-bound argument the codegen
//! passes only for a bounded member — an unbounded member omits the arg and
//! keeps the exact prior wire form (byte-identical, proven by the existing
//! `golden.rs` tests, which still pass unmodified). `sequence<struct,N>` and
//! `map<K,V,N>` have no shared Writer/Reader method (custom inline blocks),
//! so those get an inlined `error()` check instead.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_lua::{LuaGenOptions, generate_lua_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_lua_module(&ast, &LuaGenOptions::default()).expect("gen")
}

/// `lua5.4` first (the interpreter the rest of this crate's tests target),
/// falling back to a bare `lua` on PATH (any 5.x is wire-compatible here —
/// no version-specific bitops/utf8 edge cases in the emitted code).
fn lua_interpreter() -> Option<&'static str> {
    ["lua5.4", "lua"]
        .into_iter()
        .find(|cmd| Command::new(cmd).arg("-v").output().is_ok())
}

/// Runs `src` (generated module + a trailing driver script) through the Lua
/// interpreter and returns stdout. Panics with the source + stderr on a
/// non-zero exit, so a caller that expects a Lua `error()` to fire should
/// check for it via the driver script's own pcall, not by inspecting the
/// process exit code here.
fn run_lua(src: &str) -> String {
    let interp = lua_interpreter().expect("no lua interpreter on PATH");
    let dir = std::env::temp_dir().join(format!(
        "idllua_bounds_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, src).expect("write");
    let out = Command::new(interp).arg(&lf).output().expect("run lua");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "lua run FAILED:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8")
}

// ---- codegen string-match assertions (always run, no interpreter needed) ----

#[test]
fn encode_bounded_string_passes_bound_to_put_string() {
    let l = emit("@final struct S { string<4> name; };");
    assert!(l.contains("w:putString(v.name, 4)"), "{l}");
}

#[test]
fn encode_bounded_wstring_passes_bound_to_put_wstring() {
    let l = emit("@final struct S { wstring<4> name; };");
    assert!(l.contains("w:putWString(v.name, 4)"), "{l}");
}

#[test]
fn encode_bounded_octet_sequence_passes_bound_to_put_seq_u8() {
    let l = emit("@final struct S { sequence<octet, 4> data; };");
    assert!(l.contains("w:putSeqU8(v.data, 4)"), "{l}");
}

#[test]
fn encode_bounded_map_gets_inline_check() {
    let l = emit("@final struct S { map<string, long, 2> vals; };");
    assert!(
        l.contains("bounded map length exceeds its IDL bound"),
        "{l}"
    );
}

#[test]
fn decode_bounded_string_passes_bound_to_get_string() {
    let l = emit("@final struct S { string<4> name; };");
    assert!(l.contains("r:getString(4)"), "{l}");
}

#[test]
fn decode_bounded_wstring_passes_bound_to_get_wstring() {
    let l = emit("@final struct S { wstring<4> name; };");
    assert!(l.contains("r:getWString(4)"), "{l}");
}

#[test]
fn decode_bounded_octet_sequence_passes_bound_to_get_seq_u8() {
    let l = emit("@final struct S { sequence<octet, 4> data; };");
    assert!(l.contains("r:getSeqU8(4)"), "{l}");
}

#[test]
fn decode_bounded_map_gets_inline_check() {
    let l = emit("@final struct S { map<string, long, 2> vals; };");
    assert!(
        l.contains("decoded map length exceeds its IDL bound"),
        "{l}"
    );
}

#[test]
fn decode_bounded_string_reaches_union_member() {
    // Unions route through the same shared map_get, so this proves the fix
    // covers union case members for free (no separate union-specific path).
    let l = emit("union U switch (long) { case 1: string<4> s; };");
    assert!(l.contains("r:getString(4)"), "{l}");
}

#[test]
fn decode_bounded_string_reaches_mutable_member() {
    let l = emit("@mutable struct S { string<4> name; };");
    assert!(l.contains("r:getString(4)"), "{l}");
}

#[test]
fn unbounded_members_get_no_bound_argument() {
    let l = emit(
        "@final struct S { string name; wstring wname; sequence<octet> data; \
         map<string, long> vals; };",
    );
    assert!(
        l.contains("w:putString(v.name)") && !l.contains("w:putString(v.name,"),
        "{l}"
    );
    assert!(
        l.contains("w:putWString(v.wname)") && !l.contains("w:putWString(v.wname,"),
        "{l}"
    );
    assert!(
        l.contains("w:putSeqU8(v.data)") && !l.contains("w:putSeqU8(v.data,"),
        "{l}"
    );
    assert!(
        l.contains("r:getString()") && !l.contains("r:getString(4"),
        "{l}"
    );
    // Note: the shared WIRE_PRELUDE's Writer/Reader methods always contain
    // the "exceeds its IDL bound" error-format strings (gated at runtime
    // behind `if maxBytes and ...`) — that text is present in every
    // generated module regardless of whether any struct uses a bounded
    // member, so it is not itself a useful per-struct signal. The call-site
    // assertions above (no extra numeric arg passed) are the real proof
    // that an unbounded member gets no check.
}

// ---- real interpreter runs: prove the check actually fires at runtime ----

#[test]
fn runtime_decode_rejects_over_bound_string() {
    if lua_interpreter().is_none() {
        eprintln!("SKIP: no lua interpreter on PATH");
        return;
    }
    let mut src = emit("@final struct S { string<4> name; };");
    src.push_str(
        r#"
-- Within-bound roundtrip must still work (no false positives).
local ok = { name = "abcd" }
local bytes = marshal_S(ok, LE)
local back = unmarshal_S(bytes, LE)
assert(back.name == "abcd", "within-bound roundtrip failed")

-- Forge a well-formed wire payload whose string content (6 bytes) exceeds
-- the IDL bound (4) — no encode-side check is involved, this simulates an
-- adversarial/foreign sender that does not honor the receiver's IDL.
local w = Writer.new(LE)
w:putString("abcdef")
local forged = w:bytes()

local success, err = pcall(function() return unmarshal_S(forged, LE) end)
assert(not success, "decode must reject an over-bound string, but it succeeded")
assert(
  string.find(tostring(err), "exceeds its IDL bound %(4%)"),
  "expected IDL-bound error, got: " .. tostring(err)
)
print("PASS")
"#,
    );
    let out = run_lua(&src);
    assert_eq!(out.trim(), "PASS");
}

#[test]
fn runtime_decode_rejects_over_bound_sequence() {
    if lua_interpreter().is_none() {
        eprintln!("SKIP: no lua interpreter on PATH");
        return;
    }
    let mut src = emit("@final struct S { sequence<octet, 4> data; };");
    src.push_str(
        r#"
local ok = { data = "abc" }
local bytes = marshal_S(ok, LE)
local back = unmarshal_S(bytes, LE)
assert(back.data == "abc", "within-bound roundtrip failed")

-- Forge an over-bound (6 > 4) sequence<octet> payload directly.
local w = Writer.new(LE)
w:putSeqU8("abcdef")
local forged = w:bytes()

local success, err = pcall(function() return unmarshal_S(forged, LE) end)
assert(not success, "decode must reject an over-bound sequence, but it succeeded")
assert(
  string.find(tostring(err), "exceeds its IDL bound %(4%)"),
  "expected IDL-bound error, got: " .. tostring(err)
)
print("PASS")
"#,
    );
    let out = run_lua(&src);
    assert_eq!(out.trim(), "PASS");
}

#[test]
fn runtime_encode_rejects_over_bound_string() {
    if lua_interpreter().is_none() {
        eprintln!("SKIP: no lua interpreter on PATH");
        return;
    }
    let mut src = emit("@final struct S { string<4> name; };");
    src.push_str(
        r#"
local bad = { name = "toolong" }
local success, err = pcall(function() return marshal_S(bad, LE) end)
assert(not success, "encode must reject an over-bound string, but it succeeded")
assert(
  string.find(tostring(err), "exceeds its IDL bound %(4%)"),
  "expected IDL-bound error, got: " .. tostring(err)
)
print("PASS")
"#,
    );
    let out = run_lua(&src);
    assert_eq!(out.trim(), "PASS");
}
