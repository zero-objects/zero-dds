// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Zig backend: string smoke tests (always) + a byte-identity test that
//! compiles+runs the generated Zig and compares to the Rust goldens (gated on
//! `zig` on PATH and `GOLDEN_DIR` pointing at golden_{le,be}.bin).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::path::Path;
use std::process::Command;

use zerodds_idl::config::ParserConfig;
use zerodds_idl_zig::{ZigGenOptions, generate_zig_module};

const GOLDEN_IDL: &str = "\
@final struct Golden {
    uint32 id;
    uint16 kind;
    octet flags;
    float value;
    uint64 stamp;
    string label;
    sequence<octet> raw;
};";

fn emit(src: &str) -> String {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_zig_module(&ast, &ZigGenOptions::default()).expect("gen")
}

/// swarm59 #21b: `module X { struct Y { ... }; }` used to be silently
/// dropped (no `Definition::Module` arm at all) — the struct must now emit.
#[test]
fn module_wrapped_struct_is_emitted_not_dropped() {
    let z = emit("module Telemetry { @final struct Reading { long value; }; };");
    assert!(z.contains("pub const Reading = struct {"), "{z}");
    assert!(z.contains("value: i32,"), "{z}");
}

/// A reopened module (`module M {} ... module M {}`) must not lose either
/// half's content once the AST builder merges the two occurrences.
#[test]
fn reopened_module_emits_both_structs() {
    let z = emit(
        "module M { @final struct A { long x; }; }; \
         module M { @final struct B { long y; }; };",
    );
    assert!(z.contains("pub const A = struct {"), "{z}");
    assert!(z.contains("pub const B = struct {"), "{z}");
}

#[test]
fn final_struct_emits_type_and_marshal() {
    let z = emit(GOLDEN_IDL);
    assert!(z.contains("pub const Golden = struct {"), "{z}");
    assert!(z.contains("id: u32,"), "{z}");
    assert!(z.contains("kind: u16,"), "{z}");
    assert!(z.contains("flags: u8,"), "{z}");
    assert!(z.contains("value: f32,"), "{z}");
    assert!(z.contains("stamp: u64,"), "{z}");
    assert!(z.contains("label: []const u8,"), "{z}");
    assert!(z.contains("raw: []const u8,"), "{z}");
    assert!(
        z.contains(
            "pub fn marshalXCDR(self: Golden, endian: Endian, alloc: std.mem.Allocator) ![]u8 {"
        ),
        "{z}"
    );
    assert!(z.contains("try w.putU32(self.id);"), "{z}");
    assert!(z.contains("try w.putF32(self.value);"), "{z}");
    assert!(z.contains("try w.putString(self.label);"), "{z}");
    assert!(z.contains("try w.putSeqU8(self.raw);"), "{z}");
    assert!(!z.contains("var body = Writer.init"), "{z}");
}

#[test]
fn appendable_struct_frames_a_dheader() {
    let z = emit("@appendable struct S { uint32 a; };");
    assert!(
        z.contains("var body_s = Writer.init(w.buf.allocator, w.endian);"),
        "{z}"
    );
    assert!(
        z.contains("try w.putU32(@intCast(body.bytes().len));"),
        "{z}"
    );
    assert!(z.contains("try w.putBytes(body.bytes());"), "{z}");
}

const ENUM_IDL: &str = "\
enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };
@final struct S { Mode kind; uint32 tail; };";

#[test]
fn enum_emits_i32_enum_and_member_marshals() {
    let z = emit(ENUM_IDL);
    assert!(z.contains("pub const Mode = enum(i32) {"), "{z}");
    assert!(z.contains("MODE_FAULT = 2,"), "{z}");
    assert!(z.contains("kind: Mode,"), "{z}");
    // An enum member is a 32-bit signed integer on the wire (XTypes §7.4.5.1).
    assert!(
        z.contains("try w.putU32(@bitCast(@intFromEnum(self.kind)));"),
        "{z}"
    );
}

#[test]
fn enum_member_is_byte_identical_i32() {
    // Gated: needs zig. S{ kind: MODE_FAULT(=2), tail: 0xDEADBEEF }
    // -> i32 LE 02000000 + u32 LE efbeadde.
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP enum byte test: `zig` not on PATH");
        return;
    }
    let mut src = emit(ENUM_IDL);
    src.push_str(
        r##"
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const s = S{ .kind = .MODE_FAULT, .tail = 0xDEADBEEF };
    const le = try s.marshalXCDR(.little, alloc);
    const out = std.io.getStdOut().writer();
    for (le) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
}
"##,
    );
    let dir = std::env::temp_dir().join(format!("idlzig_enum_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("le").trim(),
        "02000000efbeadde"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const NESTED_IDL: &str = "\
@appendable struct Inner { unsigned short a; unsigned long b; };
@appendable struct Outer { unsigned long id; Inner one; sequence<Inner> many; string label; };";

#[test]
fn nested_struct_emits_marshal_into() {
    let z = emit(NESTED_IDL);
    assert!(
        z.contains("pub fn marshalInto(self: Inner, w: *Writer) !void"),
        "{z}"
    );
    assert!(z.contains("one: Inner,"), "{z}");
    assert!(z.contains("many: []const Inner,"), "{z}");
    assert!(z.contains("try self.one.marshalInto(body);"), "{z}");
    assert!(z.contains("try elem.marshalInto(sub);"), "{z}");
}

#[test]
fn nested_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP nested byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP nested byte: `zig` not on PATH");
        return;
    }
    let mut src = emit(NESTED_IDL);
    src.push_str(
        r#"
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const inners = [_]Inner{ .{ .a = 0xAAAA, .b = 0xBBBBCCCC }, .{ .a = 0xDDDD, .b = 0xEEEEFFFF } };
    const o = Outer{
        .id = 0xCAFEBABE,
        .one = .{ .a = 0x1111, .b = 0x22223333 },
        .many = &inners,
        .label = "nested",
    };
    const le = try o.marshalXCDR(.little, alloc);
    const be = try o.marshalXCDR(.big, alloc);
    const out = std.io.getStdOut().writer();
    for (le) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
    for (be) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlzig_nested_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_of(Path::new(&golden_dir).join("golden_nested_le.bin"))
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_of(Path::new(&golden_dir).join("golden_nested_be.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn byte_identity_vs_rust_goldens() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP byte_identity: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP byte_identity: `zig` not on PATH");
        return;
    }

    let mut src = emit(GOLDEN_IDL);
    src.push_str(
        r#"
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const g = Golden{
        .id = 0xA1B2C3D4,
        .kind = 0x1234,
        .flags = 0x5A,
        .value = 3.5,
        .stamp = 0x0102030405060708,
        .label = "bay-12",
        .raw = &[_]u8{ 0xDE, 0xAD, 0xBE, 0xEF },
    };
    const le = try g.marshalXCDR(.little, alloc);
    const be = try g.marshalXCDR(.big, alloc);
    const out = std.io.getStdOut().writer();
    for (le) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
    for (be) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
}
"#,
    );

    let dir = std::env::temp_dir().join(format!("idlzig_golden_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");

    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    let got_le = lines.next().expect("le").trim().to_string();
    let got_be = lines.next().expect("be").trim().to_string();

    assert_eq!(
        got_le,
        hex_of(Path::new(&golden_dir).join("golden_le.bin")),
        "LE wire"
    );
    assert_eq!(
        got_be,
        hex_of(Path::new(&golden_dir).join("golden_be.bin")),
        "BE wire"
    );
}

const TYPEDEF_IDL: &str = "\
typedef unsigned long Id;
typedef Id AliasId;
typedef string Label;
typedef sequence<octet> Blob;
@final struct Rec { AliasId id; Label name; Blob data; };";

#[test]
fn typedef_resolves_to_underlying_type() {
    let z = emit(TYPEDEF_IDL);
    assert!(z.contains("data: []const u8,"), "{z}");
    assert!(z.contains("try w.putSeqU8(self.data);"), "{z}");
}

#[test]
fn typedef_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP typedef byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP typedef byte: `zig` not on PATH");
        return;
    }
    let mut src = emit(TYPEDEF_IDL);
    src.push_str(
        r#"
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const r = Rec{ .id = 0xCAFEBABE, .name = "typedef", .data = &[_]u8{ 1, 2, 3 } };
    const le = try r.marshalXCDR(.little, alloc);
    const be = try r.marshalXCDR(.big, alloc);
    const out = std.io.getStdOut().writer();
    for (le) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
    for (be) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlzig_typedef_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_of(Path::new(&golden_dir).join("golden_typedef_le.bin"))
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_of(Path::new(&golden_dir).join("golden_typedef_be.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const ARRAY_IDL: &str = "\
@final struct Arr { long xs[3]; short m[2][2]; octet bs[4]; };";

#[test]
fn array_emits_fixed_arrays_and_loops() {
    let z = emit(ARRAY_IDL);
    assert!(z.contains("xs: [3]i32,"), "{z}");
    assert!(z.contains("m: [2][2]i16,"), "{z}");
    assert!(z.contains("bs: [4]u8,"), "{z}");
    assert!(z.contains("while (zdi0 < 3)"), "{z}");
    assert!(z.contains("while (zdi1 < 2)"), "{z}");
}

#[test]
fn array_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP array byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP array byte: `zig` not on PATH");
        return;
    }
    let mut src = emit(ARRAY_IDL);
    src.push_str(
        r#"
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const a = Arr{
        .xs = .{ 0x11111111, 0x22222222, 0x33333333 },
        .m = .{ .{ 0x0102, 0x0304 }, .{ 0x0506, 0x0708 } },
        .bs = .{ 0xAA, 0xBB, 0xCC, 0xDD },
    };
    const le = try a.marshalXCDR(.little, alloc);
    const be = try a.marshalXCDR(.big, alloc);
    const out = std.io.getStdOut().writer();
    for (le) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
    for (be) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlzig_array_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_of(Path::new(&golden_dir).join("golden_array_le.bin"))
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_of(Path::new(&golden_dir).join("golden_array_be.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const UNION_IDL: &str = "\
@final union U switch (long) { case 1: unsigned long a; case 2: unsigned short b; default: octet c; };";

#[test]
fn union_emits_case_dispatch() {
    let z = emit(UNION_IDL);
    assert!(z.contains("disc: i32,"), "{z}");
    assert!(z.contains("switch (self.disc) {"), "{z}");
    assert!(z.contains("1 =>"), "{z}");
    assert!(z.contains("2 =>"), "{z}");
    assert!(z.contains("else =>"), "{z}");
}

#[test]
fn union_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP union byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP union byte: `zig` not on PATH");
        return;
    }
    let mut src = emit(UNION_IDL);
    src.push_str(
        r#"
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const u = U{ .disc = 2, .a = 0, .b = 0x1234, .c = 0 };
    const le = try u.marshalXCDR(.little, alloc);
    const be = try u.marshalXCDR(.big, alloc);
    const out = std.io.getStdOut().writer();
    for (le) |bb| try out.print("{x:0>2}", .{bb});
    try out.print("\n", .{});
    for (be) |bb| try out.print("{x:0>2}", .{bb});
    try out.print("\n", .{});
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlzig_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_of(Path::new(&golden_dir).join("golden_union_le.bin"))
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_of(Path::new(&golden_dir).join("golden_union_be.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const MAP_IDL: &str = "\
@final struct HasMap { map<long, unsigned long> m; };";

#[test]
fn map_emits_kv_and_sort() {
    let z = emit(MAP_IDL);
    assert!(
        z.contains("pub const HasMap_m_KV = struct { k: i32, v: u32 };"),
        "{z}"
    );
    assert!(z.contains("m: []const HasMap_m_KV,"), "{z}");
    assert!(z.contains("std.mem.sort(HasMap_m_KV"), "{z}");
}

#[test]
fn map_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP map byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP map byte: `zig` not on PATH");
        return;
    }
    let mut src = emit(MAP_IDL);
    src.push_str(
        r#"
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const entries = [_]HasMap_m_KV{ .{ .k = 1, .v = 0x11111111 }, .{ .k = 2, .v = 0x22222222 } };
    const h = HasMap{ .m = &entries };
    const le = try h.marshalXCDR(.little, alloc);
    const be = try h.marshalXCDR(.big, alloc);
    const out = std.io.getStdOut().writer();
    for (le) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
    for (be) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlzig_map_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_of(Path::new(&golden_dir).join("golden_map_le.bin"))
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_of(Path::new(&golden_dir).join("golden_map_be.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const MUTABLE_IDL: &str = "\
@mutable struct M { @id(10) unsigned long x; @id(20) string s; @id(30) unsigned short k; };";

#[test]
fn mutable_emits_emheader_framing() {
    let z = emit(MUTABLE_IDL);
    assert!(z.contains("try body.putU32(0x4000000a);"), "{z}");
}

#[test]
fn mutable_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP mutable byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP mutable byte: `zig` not on PATH");
        return;
    }
    let mut src = emit(MUTABLE_IDL);
    src.push_str(
        r#"
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const m = M{ .x = 0xDEADBEEF, .s = "mut", .k = 0x0777 };
    const le = try m.marshalXCDR(.little, alloc);
    const be = try m.marshalXCDR(.big, alloc);
    const out = std.io.getStdOut().writer();
    for (le) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
    for (be) |b| try out.print("{x:0>2}", .{b});
    try out.print("\n", .{});
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlzig_mutable_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_of(Path::new(&golden_dir).join("golden_mutable_le.bin"))
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_of(Path::new(&golden_dir).join("golden_mutable_be.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const WIDE_IDL: &str = "\
@final struct W { wchar c; wstring s; };";
const LD_IDL: &str = "\
@final struct L { long double d; };";

fn run_zig(idl: &str, ctor: &str, stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP {stem}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP {stem}: `zig` not on PATH");
        return;
    }
    let mut src = emit(idl);
    src.push_str(&format!("\npub fn main() !void {{\n    var gpa = std.heap.GeneralPurposeAllocator(.{{}}){{}};\n    const alloc = gpa.allocator();\n    const v = {ctor};\n    const le = try v.marshalXCDR(.little, alloc);\n    const be = try v.marshalXCDR(.big, alloc);\n    const out = std.io.getStdOut().writer();\n    for (le) |b| try out.print(\"{{x:0>2}}\", .{{b}});\n    try out.print(\"\\n\", .{{}});\n    for (be) |b| try out.print(\"{{x:0>2}}\", .{{b}});\n    try out.print(\"\\n\", .{{}});\n}}\n"));
    let dir = std::env::temp_dir().join(format!("idlzig_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_of(Path::new(&golden_dir).join(format!("golden_{stem}_le.bin")))
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_of(Path::new(&golden_dir).join(format!("golden_{stem}_be.bin")))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn wide_is_byte_identical_vs_rust_golden() {
    run_zig(WIDE_IDL, "W{ .c = 0x03A9, .s = \"w\u{03c0}\" }", "wide");
}
#[test]
fn longdouble_is_byte_identical_vs_rust_golden() {
    run_zig(LD_IDL, "L{ .d = 1.1 }", "longdouble");
}

const KEYHASH_IDL: &str = "\
@final struct K { @key long a; @key unsigned short b; long c; };";

#[test]
fn keyhash_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP keyhash: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP keyhash: `zig` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_IDL);
    src.push_str("\npub fn main() !void {\n    var gpa = std.heap.GeneralPurposeAllocator(.{}){};\n    const alloc = gpa.allocator();\n    const k = K{ .a = 0x01020304, .b = 0x0506, .c = 0 };\n    const h = try k.keyHash(alloc);\n    const out = std.io.getStdOut().writer();\n    for (h) |b| try out.print(\"{x:0>2}\", .{b});\n    try out.print(\"\\n\", .{});\n}\n");
    let dir = std::env::temp_dir().join(format!("idlzig_kh_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_of(Path::new(&golden_dir).join("golden_keyhash.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn hex_of(p: std::path::PathBuf) -> String {
    std::fs::read(&p)
        .unwrap_or_else(|_| panic!("read {}", p.display()))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Decode roundtrip: `marshal(unmarshal(golden)) == golden` for LE and BE.
/// Proves the generated `unmarshalXCDR` is the exact inverse of `marshalXCDR`.
fn run_roundtrip(idl: &str, ty: &str, le_file: &str, be_file: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP roundtrip {ty}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP roundtrip {ty}: `zig` not on PATH");
        return;
    }
    let le = hex_of(Path::new(&golden_dir).join(le_file));
    let be = hex_of(Path::new(&golden_dir).join(be_file));
    let mut src = emit(idl);
    src.push_str(&format!(
        r#"
fn fromHex(alloc: std.mem.Allocator, h: []const u8) ![]u8 {{
    const buf = try alloc.alloc(u8, h.len / 2);
    return try std.fmt.hexToBytes(buf, h);
}}
pub fn main() !void {{
    var gpa = std.heap.GeneralPurposeAllocator(.{{}}){{}};
    const alloc = gpa.allocator();
    const gle = try fromHex(alloc, "{le}");
    const gbe = try fromHex(alloc, "{be}");
    const vle = try {ty}.unmarshalXCDR(gle, .little, alloc);
    const rle = try vle.marshalXCDR(.little, alloc);
    const vbe = try {ty}.unmarshalXCDR(gbe, .big, alloc);
    const rbe = try vbe.marshalXCDR(.big, alloc);
    const out = std.io.getStdOut().writer();
    for (rle) |b| try out.print("{{x:0>2}}", .{{b}});
    try out.print("\n", .{{}});
    for (rbe) |b| try out.print("{{x:0>2}}", .{{b}});
    try out.print("\n", .{{}});
}}
"#
    ));
    let dir = std::env::temp_dir().join(format!("idlzig_rt_{ty}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(lines.next().expect("le").trim(), le, "LE roundtrip {ty}");
    assert_eq!(lines.next().expect("be").trim(), be, "BE roundtrip {ty}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn decode_roundtrip_final() {
    run_roundtrip(GOLDEN_IDL, "Golden", "golden_le.bin", "golden_be.bin");
}
#[test]
fn decode_roundtrip_nested() {
    run_roundtrip(
        NESTED_IDL,
        "Outer",
        "golden_nested_le.bin",
        "golden_nested_be.bin",
    );
}
#[test]
fn decode_roundtrip_array() {
    run_roundtrip(
        ARRAY_IDL,
        "Arr",
        "golden_array_le.bin",
        "golden_array_be.bin",
    );
}
#[test]
fn decode_roundtrip_union() {
    run_roundtrip(UNION_IDL, "U", "golden_union_le.bin", "golden_union_be.bin");
}
#[test]
fn decode_roundtrip_map() {
    run_roundtrip(MAP_IDL, "HasMap", "golden_map_le.bin", "golden_map_be.bin");
}
#[test]
fn decode_roundtrip_mutable() {
    run_roundtrip(
        MUTABLE_IDL,
        "M",
        "golden_mutable_le.bin",
        "golden_mutable_be.bin",
    );
}
#[test]
fn decode_roundtrip_wide() {
    run_roundtrip(WIDE_IDL, "W", "golden_wide_le.bin", "golden_wide_be.bin");
}
#[test]
fn decode_roundtrip_longdouble() {
    run_roundtrip(
        LD_IDL,
        "L",
        "golden_longdouble_le.bin",
        "golden_longdouble_be.bin",
    );
}

const KEYHASH_MD5_IDL: &str = "\
@final struct KL { @key long a; @key long b; @key long c; @key long d; @key long e; };";

#[test]
fn keyhash_md5_is_byte_identical_vs_rust_golden() {
    // 5×@key long = 20 bytes > 16 → MD5 branch (XTypes §7.6.8.4).
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP keyhash_md5: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP keyhash_md5: `zig` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_MD5_IDL);
    src.push_str("\npub fn main() !void {\n    var gpa = std.heap.GeneralPurposeAllocator(.{}){};\n    const alloc = gpa.allocator();\n    const k = KL{ .a = 0x01020304, .b = 0x05060708, .c = 0x090A0B0C, .d = 0x0D0E0F10, .e = 0x11121314 };\n    const h = try k.keyHash(alloc);\n    const out = std.io.getStdOut().writer();\n    for (h) |b| try out.print(\"{x:0>2}\", .{b});\n    try out.print(\"\\n\", .{});\n}\n");
    let dir = std::env::temp_dir().join(format!("idlzig_kh_md5_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("main.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("run")
        .arg(&zf)
        .output()
        .expect("zig run");
    assert!(
        out.status.success(),
        "zig run failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_of(Path::new(&golden_dir).join("golden_keyhash_md5.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// --- Regression: nested-struct `@key` member must expand to only the inner
// struct's own `@key` members (XTypes 1.3 §7.6.8), not its full member set.
// Before the fix, `Outer`'s `keyHash` called `Inner.marshalInto`, which
// serializes `x`, `ignored`, AND `y` — leaking the non-key `ignored` field
// into the KeyHash.

const NESTED_KEY_SUBSET_IDL: &str = "\
@final struct Inner { @key long x; long ignored; @key long y; };
@final struct Outer { @key Inner i; };";

#[test]
fn nested_key_member_excludes_non_key_fields() {
    let o = emit(NESTED_KEY_SUBSET_IDL);
    // `Inner` also has its own `@key` members (x, y), so it emits its own
    // `keyHash` too — take the LAST occurrence (Outer's, since Outer is
    // emitted after Inner) rather than the first.
    let key_hash = o
        .rsplit("pub fn keyHash(")
        .next()
        .expect("Outer struct should emit a keyHash function");
    // Bug A fix: the inner struct's own `@key` fields x and y are written
    // directly (the marshal never calls `Inner.marshalInto`, which would
    // pull in `ignored` too).
    assert!(
        !key_hash.contains("self.i.marshalInto"),
        "keyHash must not call the nested struct's full marshalInto: {o}"
    );
    assert!(
        key_hash.contains("self.i.x") && key_hash.contains("self.i.y"),
        "keyHash must encode the nested struct's own @key fields x and y: {o}"
    );
    assert!(
        !key_hash.contains("self.i.ignored"),
        "keyHash must NOT encode the nested struct's non-key field `ignored`: {o}"
    );
}

// --- Regression: `uses_md5` must be given a real `structs` map so a small
// struct-typed `@key` member (whose static max size is <=16 bytes) takes the
// zero-pad branch, not MD5 (Bug B: an empty structs map makes `atom_size`
// unconditionally fail to resolve the struct, forcing MD5 for ANY
// struct-typed key regardless of actual size).

const NESTED_SMALL_KEY_IDL: &str = "\
@final struct Inner { @key octet a; };
@final struct Outer { @key Inner i; };";

#[test]
fn small_nested_struct_key_takes_zero_pad_branch_not_md5() {
    let o = emit(NESTED_SMALL_KEY_IDL);
    // `Inner` also emits its own trivial `keyHash` (single `@key octet`);
    // take the LAST occurrence (Outer's) rather than the first.
    let key_hash = o
        .rsplit("pub fn keyHash(")
        .next()
        .expect("Outer struct should emit a keyHash function");
    // Inner has a single @key octet -> KeyHolder max size 1 byte <= 16 ->
    // zero-pad branch ([_]u8{0} ** 16), NOT the MD5 branch.
    assert!(
        key_hash.contains("[_]u8{0} ** 16"),
        "a 1-byte nested-struct key must take the zero-pad branch: {o}"
    );
    assert!(
        !key_hash.contains("Md5"),
        "a 1-byte nested-struct key must NOT take the MD5 branch: {o}"
    );
    assert!(key_hash.contains("self.i.a"), "{o}");
}

// Welle C.2 #14: IDL identifiers colliding with Zig keywords must be
// escaped with `@"..."`, at every emit site (type names, enum names,
// enumerator names, field names) — not just structurally present but
// legal Zig once compiled.
const KEYWORD_IDL: &str = "\
enum for { return, break };
@final struct error {
    unsigned long try;
    unsigned long align;
};";

#[test]
fn keyword_identifiers_are_escaped_in_output() {
    let z = emit(KEYWORD_IDL);
    assert!(z.contains("pub const @\"for\" = enum(i32)"), "{z}");
    assert!(z.contains("@\"return\" = 0,"), "{z}");
    assert!(z.contains("@\"break\" = 1,"), "{z}");
    assert!(z.contains("pub const @\"error\" = struct"), "{z}");
    assert!(z.contains("@\"try\": u32,"), "{z}");
    assert!(z.contains("@\"align\": u32,"), "{z}");
    // No bare (unescaped) keyword declaration token leaked through.
    assert!(!z.contains("pub const for = enum"), "{z}");
    assert!(!z.contains("pub const error = struct"), "{z}");
}

#[test]
fn keyword_identifiers_compile() {
    // Gated: needs zig. Structural check above proves escaping; this proves
    // the escaped output is *actually* legal, semantically-checked Zig.
    // Uses `build-obj` (full sema + codegen, no link) rather than `run`:
    // this dev machine's zig 0.14.1 cannot link a host executable at all
    // (`undefined symbol: _abort` etc. even for a trivial `std.debug.print`
    // hello-world with no IDL-generated code involved) — a pre-existing
    // local toolchain/SDK gap, unrelated to keyword-escaping correctness.
    if Command::new("zig").arg("version").output().is_err() {
        eprintln!("SKIP keyword_identifiers_compile: `zig` not on PATH");
        return;
    }
    let mut src = emit(KEYWORD_IDL);
    src.push_str(
        r#"
pub fn useIt() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const s = @"error"{ .@"try" = 1, .@"align" = 2 };
    const bytes = try s.marshalXCDR(.little, alloc);
    const back = try @"error".unmarshalXCDR(bytes, .little, alloc);
    if (back.@"try" != 1 or back.@"align" != 2) return error.RoundtripMismatch;
    const k: @"for" = .@"return";
    if (k != .@"return") return error.EnumMismatch;
}
comptime {
    _ = useIt;
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlzig_kw_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let zf = dir.join("kw.zig");
    std::fs::write(&zf, &src).expect("write");
    let out = Command::new("zig")
        .arg("build-obj")
        .arg("-femit-bin=".to_string() + dir.join("kw.o").to_str().expect("utf8 path"))
        .arg(&zf)
        .current_dir(&dir)
        .output()
        .expect("zig build-obj");
    assert!(
        out.status.success(),
        "zig build-obj failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
