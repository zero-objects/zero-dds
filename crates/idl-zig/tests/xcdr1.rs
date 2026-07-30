// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XCDR1 / classic-CDR (PLAIN_CDR + PL_CDR1) parity for the Zig backend.
//!
//! idl-rust (and idl-cpp/idl-csharp/idl-java/idl-ts) emit a classic-CDR wire
//! path alongside XCDR2: 8-byte primitive alignment, NO DHEADER for
//! `@appendable`, and a PL_CDR1 parameter list (`[PID][len]` members +
//! `0x3F02` sentinel) for `@mutable`. This test suite pins that path in the
//! generated Zig — structural string assertions (toolchain-free, run in every
//! host gate) plus a link-free `zig build-obj` compile gate (works on macOS;
//! the `zig` runtime byte-roundtrip vs the idl-rust goldens runs on Linux/CI,
//! same as every other `zig`-gated byte test in this crate).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_zig::{ZigGenOptions, generate_zig_module};

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_zig_module(&ast, &ZigGenOptions::default()).expect("gen")
}

/// The shared Writer/Reader carry a representation flag and PL_CDR1 helpers.
#[test]
fn wire_prelude_has_xcdr1_machinery_ok() {
    let z = emit("@final struct S { uint64 a; };");
    // Representation flag + classic-CDR entry constructors.
    assert!(z.contains("xcdr1: bool"), "{z}");
    assert!(z.contains("pub fn initRep("), "{z}");
    // 8-byte alignment under XCDR1, capped at 4 under XCDR2.
    assert!(
        z.contains("if (self.xcdr1) a else (if (a > 4) 4 else a)"),
        "{z}"
    );
    // PL_CDR1 framing helpers (standard + extended header, sentinel).
    assert!(z.contains("pub fn putPlCdr1Member("), "{z}");
    assert!(z.contains("pub fn putPlCdr1Sentinel("), "{z}");
    assert!(z.contains("pub fn readPlCdr1Header("), "{z}");
    assert!(z.contains("0x3F01"), "{z}"); // PID_EXTENDED
    assert!(z.contains("0x3F02"), "{z}"); // PID_LIST_END
    // 8-byte primitives request natural alignment 8 (capped at 4 by XCDR2).
    assert!(z.contains("try self.putLE(8, &le);"), "{z}");
}

/// Every struct gains classic-CDR entry points beside the XCDR2 ones.
#[test]
fn struct_emits_xcdr1_entry_points_ok() {
    let z = emit("@final struct S { uint32 a; };");
    assert!(z.contains("pub fn marshalXCDR1(self: S,"), "{z}");
    assert!(z.contains("pub fn unmarshalXCDR1(buf: []const u8,"), "{z}");
    assert!(z.contains("Writer.initRep(alloc, endian, true)"), "{z}");
    assert!(
        z.contains("Reader.initRep(buf, endian, alloc, true)"),
        "{z}"
    );
}

/// `@appendable` has NO DHEADER under XCDR1 (inline members) but keeps the
/// DHEADER under XCDR2 — a runtime branch on `w.xcdr1`.
#[test]
fn appendable_suppresses_dheader_under_xcdr1_ok() {
    let z = emit("@appendable struct S { uint32 a; };");
    assert!(z.contains("if (w.xcdr1) {"), "{z}");
    // The DHEADER length prefix only appears in the XCDR2 (else) arm.
    assert!(
        z.contains("try w.putU32(@intCast(body.bytes().len));"),
        "{z}"
    );
    // Decode: DHEADER is skipped only under XCDR2.
    assert!(z.contains("if (!r.xcdr1) { _ = r.getU32(); }"), "{z}");
}

/// `@mutable` uses a PL_CDR1 parameter list under XCDR1.
#[test]
fn mutable_uses_pl_cdr1_under_xcdr1_ok() {
    let z = emit("@mutable struct S { @id(3) uint64 a; @id(7) uint32 b; };");
    // Encode: member framed by putPlCdr1Member with its id, sentinel-terminated.
    assert!(
        z.contains("try w.putPlCdr1Member(3, zdMem.bytes());"),
        "{z}"
    );
    assert!(
        z.contains("try w.putPlCdr1Member(7, zdMem.bytes());"),
        "{z}"
    );
    assert!(z.contains("try w.putPlCdr1Sentinel();"), "{z}");
    // Decode: id-dispatched, member-relative (r.base), pad-skip to 4.
    assert!(z.contains("r.readPlCdr1Header() orelse break"), "{z}");
    assert!(z.contains("r.base = zdBody;"), "{z}");
    assert!(z.contains("const zdPad = (4 - (zdH.len % 4)) % 4;"), "{z}");
    // XCDR2 EMHEADER path still present (LC4 | id, must-understand handled).
    assert!(z.contains("try body.putU32(0x4000000"), "{z}");
}

/// Unions carry the XCDR1 machinery too (final/appendable/mutable).
#[test]
fn union_emits_xcdr1_paths_ok() {
    let z = emit(
        "@mutable union U switch (long) { case 1: uint64 a; case 2: uint32 b; default: octet c; };",
    );
    assert!(z.contains("pub fn marshalXCDR1(self: U,"), "{z}");
    assert!(z.contains("pub fn unmarshalXCDR1(buf: []const u8,"), "{z}");
    // Discriminator is PL_CDR1 member id 0.
    assert!(
        z.contains("try w.putPlCdr1Member(0, zdMem.bytes());"),
        "{z}"
    );
    assert!(z.contains("try w.putPlCdr1Sentinel();"), "{z}");
}

/// Bitset/bitmask holders expose the classic-CDR entry points as well.
#[test]
fn holder_emits_xcdr1_entry_points_ok() {
    let z = emit("@bit_bound(16) bitmask Flags { FA, FB };");
    assert!(z.contains("pub fn marshalXCDR1(self: Flags,"), "{z}");
    assert!(z.contains("pub fn unmarshalXCDR1(buf: []const u8,"), "{z}");
}

/// Named-constant and arithmetic collection bounds now resolve (idl-rust parity
/// for `const_expr` §7.4.1.4.4) — previously a named array bound was
/// `Unsupported` and a named sequence bound silently degraded to unbounded.
#[test]
fn const_expr_bounds_resolve_ok() {
    let z = emit(
        "const long LEN = 4;\n\
         const long CAP = 2 * 5;\n\
         @final struct S { long fixedArr[LEN]; sequence<octet, CAP> payload; };",
    );
    // Fixed array size resolved to 4 (Zig `[4]i32`).
    assert!(z.contains("[4]i32"), "{z}");
    // Sequence bound resolved to 10 and enforced on encode + decode.
    assert!(z.contains("> 10) return error.BoundExceeded"), "{z}");
}

/// A union `case` label naming an enumerator / const folds to its integer.
#[test]
fn const_expr_union_labels_resolve_ok() {
    let z = emit(
        "enum Color { RED, GREEN, BLUE };\n\
         const long TWO = 1 + 1;\n\
         union U switch (long) { case TWO: uint32 x; default: octet y; };",
    );
    // `case TWO` (=2) becomes an integer label, not an Unsupported error.
    assert!(z.contains("2 => {"), "{z}");
    let _ = "Color enum keeps RED/GREEN/BLUE registered for label resolution";
}

/// Link-free full compile of the generated XCDR1 code via `zig build-obj`
/// (works on macOS, where `zig run`/linking fails on libSystem). Exercises a
/// rich spec: final/appendable/mutable structs, `@optional`, 8-byte fields,
/// nested struct, a mutable union, and a holder — plus a `main` that drives the
/// XCDR1 round-trip so the new code paths are semantically analysed.
#[test]
fn generated_xcdr1_compiles_ok() {
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP generated_xcdr1_compiles_ok: `zig` not on PATH");
        return;
    }
    let mut src = emit(
        "@final struct Inner { uint64 stamp; uint16 kind; };\n\
         @appendable struct Mid { uint32 a; Inner inner; string label; };\n\
         @mutable struct Top {\n\
            @id(1) uint64 big;\n\
            @id(2) @optional uint32 maybe;\n\
            @id(3) Mid mid;\n\
            @id(4) sequence<octet> raw;\n\
         };\n\
         @mutable union Sel switch (long) { case 1: uint64 a; case 2: Inner b; default: octet c; };\n\
         @appendable union AppU switch (long) { case 1: uint64 a; default: uint32 b; };\n\
         @final union FinU switch (long) { case 1: uint64 a; default: uint32 b; };\n\
         @bit_bound(32) bitmask Flags { FA, FB, FC };\n",
    );
    src.push_str(
        r##"
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const t = Top{
        .big = 0x0102030405060708,
        .maybe = 42,
        .mid = Mid{ .a = 7, .inner = Inner{ .stamp = 99, .kind = 3 }, .label = "hi" },
        .raw = &[_]u8{ 1, 2, 3 },
    };
    inline for (.{ Endian.little, Endian.big }) |e| {
        const b1 = try t.marshalXCDR1(e, alloc);
        const back = try Top.unmarshalXCDR1(b1, e, alloc);
        if (back.big != t.big) return error.Mismatch;
        if (back.maybe.? != 42) return error.Mismatch;
        const s = Sel{ .disc = 1, .a = 5, .b = undefined, .c = undefined };
        const bs = try s.marshalXCDR1(e, alloc);
        const sb = try Sel.unmarshalXCDR1(bs, e, alloc);
        if (sb.a != 5) return error.Mismatch;
        const au = AppU{ .disc = 1, .a = 77, .b = undefined };
        const bau = try au.marshalXCDR1(e, alloc);
        if ((try AppU.unmarshalXCDR1(bau, e, alloc)).a != 77) return error.Mismatch;
        const fu = FinU{ .disc = 9, .a = undefined, .b = 88 };
        const bfu = try fu.marshalXCDR1(e, alloc);
        if ((try FinU.unmarshalXCDR1(bfu, e, alloc)).b != 88) return error.Mismatch;
        const f = Flags{ .storage = 0b101 };
        _ = try f.marshalXCDR1(e, alloc);
    }
    const out = std.io.getStdOut().writer();
    try out.print("ok\n", .{});
}
"##,
    );
    let dir = std::env::temp_dir().join(format!("idlzig_xcdr1_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let obj = dir.join("main.o");
    let out = Command::new("zig")
        .arg("build-obj")
        .arg(&zf)
        .arg(format!("-femit-bin={}", obj.display()))
        .output()
        .expect("zig build-obj");
    assert!(
        out.status.success(),
        "zig build-obj failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
