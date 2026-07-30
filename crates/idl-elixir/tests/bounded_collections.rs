// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated
//! Elixir marshal/read code: a `sequence<T, N>` / `string<N>` / `map<K,V,N>`
//! value longer than its declared bound must be rejected on BOTH encode
//! (`marshal_into`) and decode (`read`), not just encode — idl-elixir
//! previously had NO enforcement on either side (B1 follow-up, #22).
//!
//! `read/1` is the single decode entry point used by every extensibility
//! kind (@final, @appendable, @mutable), so a struct with a single
//! extensibility annotation is enough to prove the check is wired in; there
//! is no separate decode path to miss the way idl-cpp's @mutable path did.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_elixir::{ElixirGenOptions, generate_elixir_module};

fn gen_ex(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_elixir_module(&ast, &ElixirGenOptions::default()).expect("gen")
}

#[test]
fn encode_rejects_over_bound_string() {
    let ex = gen_ex("@final struct Named { string<16> name; };");
    assert!(
        ex.contains("byte_size(v.name) > 16")
            && ex.contains("bounded string length exceeds its IDL bound (16)"),
        "bounded string<16> must raise on over-bound encode:\n{ex}"
    );
}

#[test]
fn decode_rejects_over_bound_string() {
    let ex = gen_ex("@final struct Named { string<16> name; };");
    assert!(
        ex.contains("get_string(r)")
            && ex.contains("decoded string length exceeds its IDL bound (16)"),
        "bounded string<16> must raise on over-bound decode:\n{ex}"
    );
}

#[test]
fn encode_rejects_over_bound_wstring() {
    let ex = gen_ex("@final struct Named { wstring<16> name; };");
    assert!(
        ex.contains("bounded wstring length exceeds its IDL bound (16)"),
        "bounded wstring<16> must raise on over-bound encode:\n{ex}"
    );
}

#[test]
fn decode_rejects_over_bound_wstring() {
    let ex = gen_ex("@final struct Named { wstring<16> name; };");
    assert!(
        ex.contains("decoded wstring length exceeds its IDL bound (16)"),
        "bounded wstring<16> must raise on over-bound decode:\n{ex}"
    );
}

#[test]
fn encode_rejects_over_bound_sequence_octet() {
    let ex = gen_ex("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        ex.contains("length(v.data) > 4")
            && ex.contains("bounded sequence length exceeds its IDL bound (4)"),
        "bounded sequence<octet,4> must raise on over-bound encode:\n{ex}"
    );
}

#[test]
fn decode_rejects_over_bound_sequence_octet() {
    let ex = gen_ex("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        ex.contains("get_seq_u8(r)")
            && ex.contains("decoded sequence length exceeds its IDL bound (4)"),
        "bounded sequence<octet,4> must raise on over-bound decode:\n{ex}"
    );
}

#[test]
fn encode_and_decode_reject_over_bound_sequence_of_struct() {
    let ex =
        gen_ex("@final struct Pt { long x; long y; }; @final struct Cap { sequence<Pt, 3> pts; };");
    assert!(
        ex.contains("length(v.pts) > 3")
            && ex
                .matches("bounded sequence length exceeds its IDL bound (3)")
                .count()
                >= 1,
        "bounded sequence<Pt,3> must raise on over-bound encode:\n{ex}"
    );
    assert!(
        ex.contains("if zn > 3")
            && ex.contains("decoded sequence length exceeds its IDL bound (3)"),
        "bounded sequence<Pt,3> must raise on over-bound decode:\n{ex}"
    );
}

#[test]
fn encode_and_decode_reject_over_bound_map() {
    let ex = gen_ex("@final struct M { map<string, long, 2> vals; };");
    assert!(
        ex.contains("map_size(v.vals) > 2")
            && ex.contains("bounded map length exceeds its IDL bound (2)"),
        "bounded map<string,long,2> must raise on over-bound encode:\n{ex}"
    );
    assert!(
        ex.contains("if zn > 2") && ex.contains("decoded map length exceeds its IDL bound (2)"),
        "bounded map<string,long,2> must raise on over-bound decode:\n{ex}"
    );
}

#[test]
fn mutable_struct_string_gets_same_check() {
    // read/1 is the single shared decode entry for every extensibility kind
    // — this proves @mutable inherits the check for free, same design as
    // idl-csharp/idl-java's single shared decode function.
    let ex = gen_ex("@mutable struct Named { string<8> name; };");
    assert!(
        ex.contains("bounded string length exceeds its IDL bound (8)")
            && ex.contains("decoded string length exceeds its IDL bound (8)"),
        "@mutable struct must carry both encode and decode checks:\n{ex}"
    );
}

#[test]
fn unbounded_encode_and_decode_no_bound_check() {
    let ex = gen_ex("@final struct Free { string name; sequence<octet> data; };");
    assert!(
        !ex.contains("exceeds its IDL bound"),
        "unbounded string/sequence must NOT get a bound check:\n{ex}"
    );
}

/// Real-interpreter proof (moderate fix, deep review of #22
/// decode-bounds-cross-backend): the wstring bound check used to count
/// Elixir codepoints via `String.length/1`, which under-counts a non-BMP
/// codepoint (e.g. an emoji — 1 Elixir codepoint but 2 UTF-16 code units on
/// the wire). Three emoji is 3 codepoints (`String.length` says "3 <= 4",
/// wrongly accepting) but 6 UTF-16 units (over the `wstring<4>` bound) — the
/// exact case the old check missed. Gated on `elixir` on PATH (matches
/// golden.rs's existing gate; not installed in every dev/CI environment).
#[test]
fn wstring_bound_counts_utf16_units_not_codepoints_at_runtime() {
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!(
            "SKIP wstring_bound_counts_utf16_units_not_codepoints_at_runtime: `elixir` not on PATH"
        );
        return;
    }
    let mut src = gen_ex("@final struct Named { wstring<4> wname; };");
    src.push_str(
        r#"
alias Zdgen.Named

# 3 emoji = 3 Elixir codepoints (String.length says "3 <= 4": the OLD,
# buggy check would accept this) but 6 UTF-16 code units (over the
# wstring<4> bound: the FIXED check must reject it).
over_bound = String.duplicate("\u{1F600}", 3)
ok = struct(Named, wname: "ab")
_ = Named.marshal_xcdr(ok, :little)
IO.puts("within-bound-ok")

try do
  _ = Named.marshal_xcdr(struct(Named, wname: over_bound), :little)
  IO.puts("should-not-encode")
rescue
  e in ArgumentError -> IO.puts("encode-caught: " <> Exception.message(e))
end

# Same value via decode: hand-encode the over-bound wstring onto the wire
# (bypassing marshal_xcdr's own encode-side check) and prove `unmarshal/2`
# rejects it too.
bad_bin = Zdgen.Wire.writer(:little) |> Zdgen.Wire.put_wstring(over_bound) |> Zdgen.Wire.bytes()
try do
  _ = Named.unmarshal(bad_bin, :little)
  IO.puts("should-not-decode")
rescue
  e in ArgumentError -> IO.puts("decode-caught: " <> Exception.message(e))
end
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlelixir_wstring_utf16_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "elixir failed:\nstdout={stdout}\nstderr={stderr}\n--- src ---\n{src}"
    );
    assert!(
        stdout.contains("within-bound-ok"),
        "within-bound encode must succeed first:\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        !stdout.contains("should-not-encode"),
        "3-emoji (6 UTF-16 units) over wstring<4> must NOT silently encode:\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("encode-caught: bounded wstring length exceeds its IDL bound (4)"),
        "encode must reject by UTF-16 unit count, not codepoint count:\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        !stdout.contains("should-not-decode"),
        "3-emoji (6 UTF-16 units) over wstring<4> must NOT silently decode:\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("decode-caught: decoded wstring length exceeds its IDL bound (4)"),
        "decode must reject by UTF-16 unit count, not codepoint count:\nstdout={stdout}\nstderr={stderr}"
    );
}
