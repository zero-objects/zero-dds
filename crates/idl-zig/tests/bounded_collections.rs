// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bounded-collection enforcement (DDS-XTypes §7.4.3) in the generated Zig
//! codec: a `string<N>` / `sequence<T, N>` / `map<K,V,N>` value longer than
//! its declared bound must be rejected on encode AND decode.
//!
//! idl-zig had NO IDL-declared-bound enforcement at all before this fix
//! (unlike idl-cpp/idl-csharp/idl-java, which had encode-only checks) — its
//! `map_type`/`map_get` never consulted `.bound`. Every representation
//! (`@final`/`@appendable`/`@mutable`) reuses the same per-field `put`/`get`
//! statement strings (`FieldGen.put`/`.get`), and array declarators reuse
//! `map_type`/`map_get` at the leaf too, so fixing those two functions (plus
//! the separately-inlined map-member put/get) covers every path and every
//! declarator shape for free.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_zig::{ZigGenOptions, generate_zig_module};

fn gen_zig(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_zig_module(&ast, &ZigGenOptions::default()).expect("gen")
}

#[test]
fn bounded_string_encode_and_decode_checks() {
    let z = gen_zig("@final struct Named { string<16> name; };");
    assert!(
        z.contains("if (self.name.len > 16) return error.BoundExceeded;")
            && z.contains("putString(self.name)"),
        "bounded string<16> must throw on over-bound encode:\n{z}"
    );
    assert!(
        z.contains("if (zdS.len > 16) return error.BoundExceeded;") && z.contains("r.getString()"),
        "bounded string<16> must throw on over-bound decode:\n{z}"
    );
}

#[test]
fn bounded_wstring_encode_and_decode_checks() {
    // Moderate fix (deep review of #22 decode-bounds-cross-backend): the
    // wstring<N> bound is in UTF-16 code units — checked via `wstringUnitLen`
    // (surrogate-pair aware), NOT `.len` (the UTF-8 BYTE length of the
    // `[]const u8` value, which is wrong for any non-ASCII text).
    let z = gen_zig("@final struct Named { wstring<16> name; };");
    assert!(
        z.contains("if (try wstringUnitLen(self.name) > 16) return error.BoundExceeded;")
            && z.contains("putWString(self.name)"),
        "bounded wstring<16> must throw on over-bound encode, counting UTF-16 units:\n{z}"
    );
    assert!(
        z.contains("if (try wstringUnitLen(zdS) > 16) return error.BoundExceeded;")
            && z.contains("r.getWString()"),
        "bounded wstring<16> must throw on over-bound decode, counting UTF-16 units:\n{z}"
    );
    assert!(
        z.contains("fn wstringUnitLen(s: []const u8) !usize {"),
        "the UTF-16-unit-count helper must be emitted:\n{z}"
    );
}

#[test]
fn bounded_sequence_octet_encode_and_decode_checks() {
    let z = gen_zig("@final struct Cap { sequence<octet, 4> data; };");
    assert!(
        z.contains("if (self.data.len > 4) return error.BoundExceeded;")
            && z.contains("putSeqU8(self.data)"),
        "bounded sequence<octet,4> must throw on over-bound encode:\n{z}"
    );
    assert!(
        z.contains("if (zdS.len > 4) return error.BoundExceeded;") && z.contains("r.getSeqU8()"),
        "bounded sequence<octet,4> must throw on over-bound decode:\n{z}"
    );
}

#[test]
fn bounded_sequence_of_struct_encode_and_decode_checks() {
    let z = gen_zig(
        "@final struct Pt { long x; long y; }; @final struct Cap { sequence<Pt, 3> pts; };",
    );
    assert!(
        z.contains("if (self.pts.len > 3) return error.BoundExceeded;"),
        "bounded sequence<Pt,3> must throw on over-bound encode:\n{z}"
    );
    assert!(
        z.contains("if (zdN > 3) return error.BoundExceeded;"),
        "bounded sequence<Pt,3> must throw on over-bound decode:\n{z}"
    );
}

#[test]
fn bounded_map_encode_and_decode_checks() {
    let z = gen_zig("@final struct M { map<string, long, 2> vals; };");
    assert!(
        z.contains("if (self.vals.len > 2) return error.BoundExceeded;"),
        "bounded map<string,long,2> must throw on over-bound encode:\n{z}"
    );
    assert!(
        z.contains("if (zdN > 2) return error.BoundExceeded;"),
        "bounded map<string,long,2> must throw on over-bound decode:\n{z}"
    );
}

#[test]
fn bounded_string_array_element_checks() {
    // Array declarators reuse map_type/map_get at the leaf — proves
    // array-of-bounded-element is covered for free (no separate manual-array
    // path like idl-rust needed).
    let z = gen_zig("@final struct A { string<4> names[3]; };");
    assert!(
        z.contains("if ($elem.len > 4) return error.BoundExceeded;")
            || z.contains("return error.BoundExceeded;") && z.contains("names"),
        "array-of-bounded-string element must carry a bound check:\n{z}"
    );
}

#[test]
fn appendable_struct_reuses_same_bound_checked_field_codec() {
    // @appendable emits the SAME per-field put/get strings as @final (via
    // FieldGen.put/.get reused verbatim across representations) — proves the
    // check is not accidentally final-only.
    let z = gen_zig("@appendable struct Named { string<8> name; };");
    let checks = z.matches("return error.BoundExceeded;").count();
    assert!(
        checks >= 2,
        "@appendable struct must carry both the encode and decode bound check:\n{z}"
    );
}

#[test]
fn unbounded_no_check() {
    let z = gen_zig("@final struct Free { string name; sequence<octet> data; };");
    assert!(
        !z.contains("BoundExceeded"),
        "unbounded string/sequence must NOT get a bound check:\n{z}"
    );
}

/// Real-interpreter proof (moderate fix, deep review of #22
/// decode-bounds-cross-backend): the wstring bound check used to count
/// `.len` — the UTF-8 BYTE length of the `[]const u8` value — instead of the
/// UTF-16 code-unit count DDS-XTypes 1.3 §7.4.3 actually bounds. "日本語" (3
/// CJK codepoints) is exactly 3 UTF-16 units but 9 UTF-8 bytes: the old,
/// buggy check would have WRONGLY REJECTED this valid `wstring<3>` value (9 >
/// 3); the fixed check (`wstringUnitLen` = 3 <= 3) must accept it, and a
/// roundtrip through encode+decode must recover the same string. Gated on
/// `zig` on PATH (matches golden.rs's existing gate).
#[test]
fn wstring_bound_counts_utf16_units_not_utf8_bytes_at_runtime() {
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!(
            "SKIP wstring_bound_counts_utf16_units_not_utf8_bytes_at_runtime: `zig` not on PATH"
        );
        return;
    }
    let mut src = gen_zig("@final struct Named { wstring<3> name; };");
    src.push_str(
        r##"
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const v = Named{ .name = "日本語" };
    const bytes = try v.marshalXCDR(.little, alloc);
    const back = try Named.unmarshalXCDR(bytes, .little, alloc);
    const out = std.io.getStdOut().writer();
    try out.print("roundtrip-ok name={s}\n", .{back.name});
}
"##,
    );
    let dir = std::env::temp_dir().join(format!("idlzig_wstring_utf16_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "zig run failed (a within-bound wstring<3> of 3 CJK codepoints must \
         NOT be rejected — that would prove the UTF-8-byte-length bug is \
         still present):\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("line"),
        "roundtrip-ok name=日本語"
    );
}
