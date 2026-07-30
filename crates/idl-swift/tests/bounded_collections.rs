// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated Swift
//! marshal/unmarshal code: a bounded `string<N>` / `wstring<N>` /
//! `sequence<T,N>` / `map<K,V,N>` value longer than its declared bound must
//! be rejected on both encode and decode (the decode side matters most — an
//! over-bound *decoded* wire value is untrusted-peer input).
//!
//! B1 blocker fix (deep review of #22 decode-bounds-cross-backend):
//! `marshalInto`/`marshalXCDR`/`unmarshalFrom`/`unmarshalXCDR`/`keyHash` are
//! now uniformly `throws` Swift functions, and a bound violation raises a
//! catchable `XcdrBoundError` — replacing the previous `fatalError`, which
//! aborted the whole process (an uncatchable crash) on nothing worse than an
//! over-bound value, including one supplied by a remote peer on decode. That
//! was a remote-triggerable denial-of-service: any peer could kill the
//! process by sending one oversized bounded field.

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
use zerodds_idl_swift::{SwiftGenOptions, generate_swift_module};

fn gen_swift(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_swift_module(&ast, &SwiftGenOptions::default()).expect("gen")
}

#[test]
fn marshal_and_unmarshal_are_throws() {
    let s = gen_swift("@final struct Named { string<16> name; };");
    assert!(
        s.contains("public func marshalInto(_ w: inout Writer) throws {"),
        "marshalInto must be throws:\n{s}"
    );
    assert!(
        s.contains("public func marshalXCDR(_ endian: Endianness) throws -> [UInt8] {"),
        "marshalXCDR must be throws:\n{s}"
    );
    assert!(
        s.contains("public static func unmarshalFrom(_ r: inout Reader) throws -> Named {"),
        "unmarshalFrom must be throws:\n{s}"
    );
    assert!(
        s.contains(
            "public static func unmarshalXCDR(_ buf: [UInt8], _ endian: Endianness) throws -> Named {"
        ),
        "unmarshalXCDR must be throws:\n{s}"
    );
    assert!(!s.contains("fatalError"), "fatalError must be gone:\n{s}");
}

#[test]
fn bounded_string_encode_and_decode_checks() {
    let s = gen_swift("@final struct Named { string<16> name; };");
    assert!(
        s.contains("utf8.count > 16")
            && s.contains(
                "throw XcdrBoundError(\"bounded string length exceeds its IDL bound (16)\")"
            ),
        "encode of bounded string<16> must throw on over-bound:\n{s}"
    );
    assert!(
        s.contains("throw XcdrBoundError(\"decoded string length exceeds its IDL bound (16)\")"),
        "decode of bounded string<16> must throw on over-bound:\n{s}"
    );
}

#[test]
fn bounded_wstring_encode_and_decode_checks() {
    let s = gen_swift("@final struct Named { wstring<16> name; };");
    assert!(
        s.contains("utf16.count > 16")
            && s.contains(
                "throw XcdrBoundError(\"bounded wstring length exceeds its IDL bound (16)\")"
            ),
        "encode of bounded wstring<16> must throw on over-bound:\n{s}"
    );
    assert!(
        s.contains("throw XcdrBoundError(\"decoded wstring length exceeds its IDL bound (16)\")"),
        "decode of bounded wstring<16> must throw on over-bound:\n{s}"
    );
}

#[test]
fn bounded_octet_sequence_encode_and_decode_checks() {
    let s = gen_swift("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        s.contains(".count > 4")
            && s.contains(
                "throw XcdrBoundError(\"bounded sequence length exceeds its IDL bound (4)\")"
            ),
        "encode of bounded sequence<octet,4> must throw on over-bound:\n{s}"
    );
    assert!(
        s.contains("throw XcdrBoundError(\"decoded sequence length exceeds its IDL bound (4)\")"),
        "decode of bounded sequence<octet,4> must throw on over-bound:\n{s}"
    );
}

#[test]
fn bounded_struct_sequence_encode_and_decode_checks() {
    // The non-octet-element sequence path (sequence<struct>) is a separate
    // code branch in map_sequence/map_get_sequence — must carry the check too.
    let s = gen_swift(
        "@final struct Pt { long x; long y; }; @final struct Cap { sequence<Pt, 3> pts; };",
    );
    assert!(
        s.contains("throw XcdrBoundError(\"bounded sequence length exceeds its IDL bound (3)\")"),
        "encode of bounded sequence<Pt,3> must throw on over-bound:\n{s}"
    );
    assert!(
        s.contains("if zdN0 > 3")
            && s.contains(
                "throw XcdrBoundError(\"decoded sequence length exceeds its IDL bound (3)\")"
            ),
        "decode of bounded sequence<Pt,3> must throw on over-bound:\n{s}"
    );
    // Nested-struct element decode/encode must be `try`-called now that
    // unmarshalFrom/marshalInto are throws.
    assert!(
        s.contains("try Pt.unmarshalFrom(&r)"),
        "sequence<struct> element decode must `try` the nested unmarshalFrom:\n{s}"
    );
    assert!(
        s.contains("try zdElem0.marshalInto(&zdSub0)"),
        "sequence<struct> element encode must `try` the nested marshalInto:\n{s}"
    );
}

#[test]
fn bounded_map_encode_and_decode_checks() {
    let s = gen_swift("@final struct M { map<string, long, 2> vals; };");
    assert!(
        s.contains("throw XcdrBoundError(\"bounded map length exceeds its IDL bound (2)\")"),
        "encode of bounded map<string,long,2> must throw on over-bound:\n{s}"
    );
    assert!(
        s.contains("if zdN0 > 2")
            && s.contains("throw XcdrBoundError(\"decoded map length exceeds its IDL bound (2)\")"),
        "decode of bounded map<string,long,2> must throw on over-bound:\n{s}"
    );
}

#[test]
fn bounded_string_in_union_member_checks() {
    // Union arms route through the same map_type/map_get helpers as struct
    // members, so this must carry the check for free.
    let s = gen_swift("union U switch (long) { case 1: string<8> s; };");
    assert!(
        s.contains("throw XcdrBoundError(\"bounded string length exceeds its IDL bound (8)\")")
            && s.contains(
                "throw XcdrBoundError(\"decoded string length exceeds its IDL bound (8)\")"
            ),
        "union member string<8> must carry both encode and decode checks:\n{s}"
    );
    assert!(
        s.contains("public func marshalInto(_ w: inout Writer) throws {")
            && s.contains("public static func unmarshalFrom(_ r: inout Reader) throws -> U {"),
        "union marshalInto/unmarshalFrom must be throws:\n{s}"
    );
}

#[test]
fn unbounded_string_and_octet_sequence_no_check() {
    let s = gen_swift("@final struct Free { string name; sequence<octet> data; };");
    assert!(
        !s.contains("exceeds its IDL bound"),
        "unbounded string/sequence must NOT get a bound check:\n{s}"
    );
}

#[test]
fn nested_struct_field_calls_are_try() {
    let s = gen_swift("@final struct Inner { long x; }; @final struct Outer { Inner inner; };");
    assert!(
        s.contains("try inner.marshalInto(&w)"),
        "nested struct field encode must `try` the member's marshalInto:\n{s}"
    );
    assert!(
        s.contains("try Inner.unmarshalFrom(&r)"),
        "nested struct field decode must `try` the member's unmarshalFrom:\n{s}"
    );
}

/// Real-compile proof (mirrors the go/cpp-C/lua/ocaml pattern: compile+run
/// generated code and observe the actual reject behavior, not just grep the
/// generated source). Over-bound encode/decode must now `throw` a catchable
/// `XcdrBoundError` — caught with `do`/`catch` — and the process must exit
/// cleanly (0), NOT abort. Gated on `swiftc` being on PATH (local macOS only;
/// codepit CI has no swiftc), matching golden.rs's existing gate.
#[test]
fn over_bound_encode_and_decode_throw_catchably_at_runtime() {
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!(
            "SKIP over_bound_encode_and_decode_throw_catchably_at_runtime: `swiftc` not on PATH"
        );
        return;
    }
    let mut src = gen_swift("@final struct Named { string<4> name; wstring<4> wname; };");
    src.push_str(
        r##"
let ok = Named(name: "abcd", wname: "abcd")
_ = try ok.marshalXCDR(.little)
print("within-bound-ok")

let bad = Named(name: "abcdef", wname: "abcd")
do {
    _ = try bad.marshalXCDR(.little)
    print("should-not-encode")
} catch {
    print("encode-caught: \(error)")
}

// Craft a wire buffer whose wstring exceeds the IDL bound<4>: 6 UTF-16 units
// (octet length 12), well-formed CDR, over the decode-side bound.
var w = Writer(.little)
w.putString("ok")
let overWideUnits: [UInt16] = [0x41, 0x42, 0x43, 0x44, 0x45, 0x46]
w.putU32(UInt32(overWideUnits.count * 2))
for u in overWideUnits { w.putU16(u) }
var badReader = Reader(w.bytes(), .little)
do {
    _ = try Named.unmarshalFrom(&badReader)
    print("should-not-decode")
} catch {
    print("decode-caught: \(error)")
}
print("process-exited-cleanly")
"##,
    );
    let dir = std::env::temp_dir().join(format!("idlswift_boundtrap_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let sf = dir.join("main.swift");
    std::fs::write(&sf, &src).expect("write");
    let build = Command::new("swiftc")
        .arg(&sf)
        .arg("-o")
        .arg(dir.join("main_bin"))
        .output()
        .expect("swiftc");
    assert!(
        build.status.success(),
        "swiftc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(dir.join("main_bin")).output().expect("run");
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();
    let stderr = String::from_utf8_lossy(&run.stderr).to_string();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        stdout.contains("within-bound-ok"),
        "within-bound encode must succeed first:\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        !stdout.contains("should-not-encode"),
        "over-bound encode must not silently succeed:\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("encode-caught: bounded string length exceeds its IDL bound (4)"),
        "over-bound encode must throw a catchable XcdrBoundError naming the bound:\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        !stdout.contains("should-not-decode"),
        "over-bound decode must not silently succeed:\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("decode-caught: decoded wstring length exceeds its IDL bound (4)"),
        "over-bound decode must throw a catchable XcdrBoundError naming the bound:\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("process-exited-cleanly"),
        "the process must NOT abort — both violations must be caught, not fatalError:\nstdout={stdout}\nstderr={stderr}"
    );
    assert!(
        run.status.success(),
        "process must exit 0 (no fatalError/trap) once both bound errors are caught:\nstdout={stdout}\nstderr={stderr}"
    );
}
