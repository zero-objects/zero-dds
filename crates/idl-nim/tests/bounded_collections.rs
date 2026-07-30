// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated Nim
//! encode/decode: a `sequence<T, N>` / `string<N>` / `wstring<N>` /
//! `map<K,V,N>` value longer than its declared bound must be rejected on
//! BOTH encode and decode (strict vendors reject on the wire; decode must
//! reject a well-formed-but-oversized wire value too — an untrusted-input
//! DoS vector otherwise). idl-nim previously had NO enforcement on either
//! side; this is the B1 follow-up fix (mirrors idl-rust/idl-cpp/idl-csharp/
//! idl-java, all fixed earlier on this branch).
//!
//! Every Nim encode/decode path (struct member, fixed-array element via
//! `build_array_put`/`build_array_get`, union case) funnels through the
//! single shared `map_type`/`map_get` pair, so the fix there covers all of
//! them for free.
//!
//! Moderate fix (deep review of #22 decode-bounds-cross-backend):
//! - `string`/`wstring`/octet-`sequence` checks now run INSIDE the shared
//!   `putString`/`putSeqU8`/`putWString` (Writer) and
//!   `getString`/`getSeqU8`/`getWString` (Reader) procs, BEFORE they
//!   materialize the value — not after, as before. The struct's generated
//!   code now just passes the bound as an extra arg (`r.getString(16)`).
//! - The wstring bound check now counts true UTF-16 code units via
//!   `wstringUnitLen` (surrogate-pair aware: a non-BMP codepoint is 2
//!   units), replacing `runeLen` (Unicode CODEPOINT count), which
//!   under-counted a non-BMP codepoint's 2-unit surrogate pair — the same
//!   class of bug flagged for idl-elixir's `String.length/1`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)]

use zerodds_idl::config::ParserConfig;
use zerodds_idl_nim::{NimGenOptions, generate_nim_module};

fn gen_nim(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_nim_module(&ast, &NimGenOptions::default()).expect("gen")
}

#[test]
fn wire_prelude_checks_before_materializing() {
    let n = gen_nim("@final struct Named { string<16> name; };");
    // The bound check must be INSIDE the shared procs (before the value is
    // materialized), not as a separate post-hoc statement in the struct body.
    assert!(
        n.contains("proc putString*(w: var Writer, s: string, maxLen: int = -1) =")
            && n.contains("if maxLen >= 0 and s.len > maxLen:"),
        "putString must check the bound BEFORE writing:\n{n}"
    );
    assert!(
        n.contains("proc getString*(r: var Reader, maxLen: int = -1): string =")
            && n.contains("if maxLen >= 0 and n > 0 and (n - 1) > maxLen:"),
        "getString must check the wire-declared length BEFORE materializing the string:\n{n}"
    );
    assert!(
        n.contains("proc wstringUnitLen*(s: string): int ="),
        "a UTF-16-unit-count helper (surrogate-pair aware) must be emitted:\n{n}"
    );
    assert!(
        !n.contains("runeLen("),
        "runeLen(...) must no longer be CALLED anywhere (it under-counts \
         non-BMP codepoints) — a doc comment may still mention the name \
         historically, hence checking for the call form, not the bare word:\n{n}"
    );
}

#[test]
fn encode_rejects_over_bound_string() {
    let n = gen_nim("@final struct Named { string<16> name; };");
    assert!(
        n.contains("w.putString(self.name, 16)"),
        "encode must pass the IDL bound to putString:\n{n}"
    );
}

#[test]
fn decode_rejects_over_bound_string() {
    let n = gen_nim("@final struct Named { string<16> name; };");
    assert!(
        n.contains("result.name = r.getString(16)"),
        "decode must pass the IDL bound to getString:\n{n}"
    );
}

#[test]
fn encode_and_decode_reject_over_bound_wstring() {
    let n = gen_nim("@final struct Named { wstring<8> name; };");
    assert!(
        n.contains("w.putWString(self.name, 8)"),
        "encode must pass the IDL bound to putWString:\n{n}"
    );
    assert!(
        n.contains("result.name = r.getWString(8)"),
        "decode must pass the IDL bound to getWString:\n{n}"
    );
}

#[test]
fn encode_and_decode_reject_over_bound_octet_sequence() {
    let n = gen_nim("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        n.contains("w.putSeqU8(self.data, 4)"),
        "encode must pass the IDL bound to putSeqU8:\n{n}"
    );
    assert!(
        n.contains("result.data = r.getSeqU8(4)"),
        "decode must pass the IDL bound to getSeqU8 (moderate fix: was \
         `getSeqU8()` fully materialized THEN checked `.len > 4` — now \
         checked inside getSeqU8 before allocating):\n{n}"
    );
}

#[test]
fn encode_and_decode_reject_over_bound_struct_sequence() {
    let n = gen_nim(
        "@final struct Pt { long x; long y; }; @final struct Cap { sequence<Pt, 3> pts; };",
    );
    assert!(
        n.contains("self.pts.len > 3")
            && n.contains("bounded sequence length exceeds its IDL bound (3)"),
        "bounded sequence<Pt,3> must raise on over-bound encode:\n{n}"
    );
    assert!(
        n.contains("zdN > 3") && n.contains("decoded sequence length exceeds its IDL bound (3)"),
        "bounded sequence<Pt,3> must raise on over-bound decode:\n{n}"
    );
    // The bound check must run BEFORE the `newSeq[Pt](zdN)` allocation, not
    // after — an attacker-supplied huge zdN must not drive an oversized
    // allocation/decode loop before the bound is checked.
    let check_pos = n
        .find("decoded sequence length exceeds its IDL bound (3)")
        .expect("check present");
    let alloc_pos = n.find("newSeq[Pt](zdN)").expect("allocation present");
    assert!(
        check_pos < alloc_pos,
        "bound check must run BEFORE the newSeq allocation/decode loop, not after:\n{n}"
    );
}

#[test]
fn encode_and_decode_reject_over_bound_map() {
    let n = gen_nim("@final struct M { map<string, long, 2> vals; };");
    assert!(
        n.contains("bounded map length exceeds its IDL bound (2)"),
        "bounded map<string,long,2> must raise on over-bound encode:\n{n}"
    );
    assert!(
        n.contains("decoded map length exceeds its IDL bound (2)"),
        "bounded map<string,long,2> must raise on over-bound decode:\n{n}"
    );
}

#[test]
fn decode_rejects_over_bound_string_array_element() {
    // Array decode (build_array_get) calls map_get at the leaf for every
    // dimension — no separate manual-array-decode path in Nim, so
    // array-of-bounded-string is covered for free.
    let n = gen_nim("@final struct A { string<4> names[3]; };");
    assert!(
        n.contains("r.getString(4)"),
        "array-of-bounded-string element decode must pass the bound through:\n{n}"
    );
}

#[test]
fn decode_rejects_over_bound_string_union_case() {
    let n = gen_nim("union U switch (long) { case 1: string<8> s; };");
    assert!(
        n.contains("r.getString(8)"),
        "union case decode (map_get, shared with struct members) must pass the bound through:\n{n}"
    );
}

#[test]
fn unbounded_encode_and_decode_no_bound_check() {
    let n = gen_nim("@final struct Free { string name; sequence<octet> data; };");
    // The shared WIRE_PRELUDE procs (putString/getString/putSeqU8/getSeqU8/
    // putWString/getWString) now ALWAYS carry the "exceeds its IDL bound"
    // check text (moderate fix: the check lives inside the shared proc,
    // gated at runtime by `maxLen/maxUnits >= 0`) — that text is present in
    // every generated file regardless of whether any member is bounded. The
    // per-struct call sites are what must show NO bound was passed through.
    assert!(
        n.contains("w.putString(self.name)") && !n.contains("w.putString(self.name,"),
        "unbounded string encode must call putString with NO bound arg:\n{n}"
    );
    assert!(
        n.contains("result.name = r.getString()") && !n.contains("result.name = r.getString(0"),
        "unbounded string decode must call getString with NO bound arg:\n{n}"
    );
    assert!(
        n.contains("w.putSeqU8(self.data)") && !n.contains("w.putSeqU8(self.data,"),
        "unbounded sequence encode must call putSeqU8 with NO bound arg:\n{n}"
    );
    assert!(
        n.contains("result.data = r.getSeqU8()") && !n.contains("result.data = r.getSeqU8(0"),
        "unbounded sequence decode must call getSeqU8 with NO bound arg:\n{n}"
    );
}
