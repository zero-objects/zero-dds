// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Nim backend: string smoke tests (always) + a byte-identity test that
//! compiles+runs the generated Nim and compares to the Rust goldens (gated on
//! `nim` on PATH and `GOLDEN_DIR` pointing at golden_{le,be}.bin).

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
use zerodds_idl_nim::{NimGenOptions, generate_nim_module};

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
    generate_nim_module(&ast, &NimGenOptions::default()).expect("gen")
}

/// swarm59 #21b: `module X { struct Y { ... }; }` used to be silently
/// dropped (no `Definition::Module` arm at all) — the struct must now emit.
#[test]
fn module_wrapped_struct_is_emitted_not_dropped() {
    let n = emit("module Telemetry { @final struct Reading { long value; }; };");
    // #21: a module-wrapped type is emitted with its module-qualified name.
    assert!(n.contains("type Telemetry_Reading* = object"), "{n}");
    assert!(n.contains("value*: int32"), "{n}");
}

/// A reopened module (`module M {} ... module M {}`) must not lose either
/// half's content once the AST builder merges the two occurrences.
#[test]
fn reopened_module_emits_both_structs() {
    let n = emit(
        "module M { @final struct A { long x; }; }; \
         module M { @final struct B { long y; }; };",
    );
    // #21: both halves emit under the module-qualified name `M_*`.
    assert!(n.contains("type M_A* = object"), "{n}");
    assert!(n.contains("type M_B* = object"), "{n}");
}

/// #21 cross-module collision: two different modules each declaring `Reading`
/// must emit distinct, module-qualified Nim types, never a duplicate one.
#[test]
fn cross_module_same_name_types_are_qualified() {
    let n = emit(
        "module a { @final struct Reading { long v; }; }; \
         module b { @final struct Reading { double w; }; };",
    );
    assert!(n.contains("type a_Reading* = object"), "{n}");
    assert!(n.contains("type b_Reading* = object"), "{n}");
    assert!(!n.contains("type Reading* = object"), "{n}");
    assert!(n.contains("v*: int32"), "{n}");
    assert!(n.contains("w*: float64"), "{n}");
}

/// #21 cross-module reference: `module b`'s struct references `a::R`, which
/// must resolve to the qualified type `a_R`, not the bare `R`.
#[test]
fn cross_module_reference_resolves_to_qualified_type() {
    let n = emit(
        "module a { @final struct R { long v; }; }; \
         module b { @final struct S { a::R r; }; };",
    );
    assert!(n.contains("type a_R* = object"), "{n}");
    assert!(n.contains("type b_S* = object"), "{n}");
    // S's member `r` has the qualified type a_R.
    assert!(n.contains("r*: a_R"), "{n}");
}

/// #21 compile gate: a two-module spec with a cross-module reference must
/// produce compilable Nim.
#[test]
fn cross_module_reference_compiles_with_nim() {
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP cross_module_reference_compiles_with_nim: `nim` not on PATH");
        return;
    }
    let mut src = emit(
        "module a { @final struct R { long v; }; }; \
         module b { @final struct S { a::R r; }; };",
    );
    src.push_str(
        r#"
when isMainModule:
  var s: b_S
  s.r.v = 7'i32
  discard s.marshalXCDR(eLE)
  echo "ok"
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlnim_xmod_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn final_struct_emits_object_and_marshal() {
    let n = emit(GOLDEN_IDL);
    assert!(n.contains("type Golden* = object"), "{n}");
    assert!(n.contains("id*: uint32"), "{n}");
    assert!(n.contains("kind*: uint16"), "{n}");
    assert!(n.contains("flags*: uint8"), "{n}");
    assert!(n.contains("value*: float32"), "{n}");
    assert!(n.contains("stamp*: uint64"), "{n}");
    assert!(n.contains("label*: string"), "{n}");
    assert!(n.contains("raw*: seq[byte]"), "{n}");
    assert!(
        n.contains("proc marshalXCDR*(self: Golden, endian: Endian): seq[byte] ="),
        "{n}"
    );
    assert!(n.contains("w.putU32(self.id)"), "{n}");
    assert!(n.contains("w.putF32(self.value)"), "{n}");
    assert!(n.contains("w.putString(self.label)"), "{n}");
    assert!(n.contains("w.putSeqU8(self.raw)"), "{n}");
    assert!(!n.contains("var body = initWriter"), "{n}");
}

#[test]
fn appendable_struct_frames_a_dheader() {
    let n = emit("@appendable struct S { uint32 a; };");
    assert!(n.contains("var body = initWriter(w.endian)"), "{n}");
    assert!(n.contains("w.putU32(uint32(body.bytes().len))"), "{n}");
    assert!(n.contains("w.putBytes(body.bytes())"), "{n}");
}

const ENUM_IDL: &str = "\
enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };
@final struct S { Mode kind; uint32 tail; };";

#[test]
fn enum_emits_int32_type_and_member_marshals() {
    let n = emit(ENUM_IDL);
    assert!(n.contains("type Mode* = enum"), "{n}");
    assert!(n.contains("ModeMODE_IDLE = 0"), "{n}");
    assert!(n.contains("ModeMODE_FAULT = 2"), "{n}");
    assert!(n.contains("kind*: Mode"), "{n}");
    // An enum member is a 32-bit signed integer on the wire (XTypes §7.4.5.1).
    assert!(
        n.contains("w.putU32(cast[uint32](int32(ord(self.kind))))"),
        "{n}"
    );
}

#[test]
fn enum_member_is_byte_identical_i32() {
    // Gated: needs the Nim toolchain. S{ kind: MODE_FAULT(=2), tail: 0xDEADBEEF }
    // → i32 LE 02000000 + u32 LE efbeadde.
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP enum byte test: `nim` not on PATH");
        return;
    }
    let mut src = emit(ENUM_IDL);
    src.push_str(
        r#"
when isMainModule:
  proc toHex(b: seq[byte]): string =
    const hexd = "0123456789abcdef"
    result = newStringOfCap(b.len * 2)
    for x in b:
      result.add(hexd[int(x) shr 4])
      result.add(hexd[int(x) and 0xf])
  var s: S
  s.kind = ModeMODE_FAULT
  s.tail = 0xDEADBEEF'u32
  echo toHex(s.marshalXCDR(eLE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlnim_enum_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
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
    let n = emit(NESTED_IDL);
    assert!(
        n.contains("proc marshalInto*(self: Inner, w: var Writer) ="),
        "{n}"
    );
    assert!(n.contains("one*: Inner"), "{n}");
    assert!(n.contains("many*: seq[Inner]"), "{n}");
    assert!(n.contains("self.one.marshalInto(body)"), "{n}");
    assert!(n.contains(".marshalInto(sub_many)"), "{n}");
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
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP nested byte: `nim` not on PATH");
        return;
    }
    let mut src = emit(NESTED_IDL);
    src.push_str(
        r#"
when isMainModule:
  proc toHex(b: seq[byte]): string =
    const hexd = "0123456789abcdef"
    result = newStringOfCap(b.len * 2)
    for x in b:
      result.add(hexd[int(x) shr 4])
      result.add(hexd[int(x) and 0xf])
  var o: Outer
  o.id = 0xCAFEBABE'u32
  o.one = Inner(a: 0x1111'u16, b: 0x22223333'u32)
  o.many = @[Inner(a: 0xAAAA'u16, b: 0xBBBBCCCC'u32), Inner(a: 0xDDDD'u16, b: 0xEEEEFFFF'u32)]
  o.label = "nested"
  echo toHex(o.marshalXCDR(eLE))
  echo toHex(o.marshalXCDR(eBE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlnim_nested_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
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

const TYPEDEF_IDL: &str = "\
typedef unsigned long Id;
typedef Id AliasId;
typedef string Label;
typedef sequence<octet> Blob;
@final struct Rec { AliasId id; Label name; Blob data; };";

#[test]
fn typedef_resolves_to_underlying_type() {
    let n = emit(TYPEDEF_IDL);
    assert!(n.contains("id*: uint32"), "{n}");
    assert!(n.contains("name*: string"), "{n}");
    assert!(n.contains("data*: seq[byte]"), "{n}");
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
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP typedef byte: `nim` not on PATH");
        return;
    }
    let mut src = emit(TYPEDEF_IDL);
    src.push_str(
        r#"
when isMainModule:
  proc toHex(b: seq[byte]): string =
    const hexd = "0123456789abcdef"
    result = newStringOfCap(b.len * 2)
    for x in b:
      result.add(hexd[int(x) shr 4])
      result.add(hexd[int(x) and 0xf])
  var r: Rec
  r.id = 0xCAFEBABE'u32
  r.name = "typedef"
  r.data = @[1'u8, 2, 3]
  echo toHex(r.marshalXCDR(eLE))
  echo toHex(r.marshalXCDR(eBE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlnim_typedef_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
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

#[test]
fn byte_identity_vs_rust_goldens() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP byte_identity: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP byte_identity: `nim` not on PATH");
        return;
    }

    let mut src = emit(GOLDEN_IDL);
    src.push_str(
        r#"
when isMainModule:
  proc toHex(b: seq[byte]): string =
    const hexd = "0123456789abcdef"
    result = newStringOfCap(b.len * 2)
    for x in b:
      result.add(hexd[int(x) shr 4])
      result.add(hexd[int(x) and 0xf])
  var g: Golden
  g.id = 0xA1B2C3D4'u32
  g.kind = 0x1234'u16
  g.flags = 0x5A'u8
  g.value = 3.5'f32
  g.stamp = 0x0102030405060708'u64
  g.label = "bay-12"
  g.raw = @[0xDE'u8, 0xAD, 0xBE, 0xEF]
  echo toHex(g.marshalXCDR(eLE))
  echo toHex(g.marshalXCDR(eBE))
"#,
    );

    let dir = std::env::temp_dir().join(format!("idlnim_golden_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");

    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
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

const ARRAY_IDL: &str = "\
@final struct Arr { long xs[3]; short m[2][2]; octet bs[4]; };";

#[test]
fn array_emits_fixed_arrays_and_loops() {
    let n = emit(ARRAY_IDL);
    assert!(n.contains("xs*: array[3, int32]"), "{n}");
    assert!(n.contains("m*: array[2, array[2, int16]]"), "{n}");
    assert!(n.contains("bs*: array[4, uint8]"), "{n}");
    assert!(n.contains("for i0 in 0 ..< 3:"), "{n}");
    assert!(n.contains("for i1 in 0 ..< 2:"), "{n}");
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
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP array byte: `nim` not on PATH");
        return;
    }
    let mut src = emit(ARRAY_IDL);
    src.push_str(
        r#"
when isMainModule:
  proc toHex(b: seq[byte]): string =
    const hexd = "0123456789abcdef"
    result = newStringOfCap(b.len * 2)
    for x in b:
      result.add(hexd[int(x) shr 4])
      result.add(hexd[int(x) and 0xf])
  var a: Arr
  a.xs = [0x11111111'i32, 0x22222222'i32, 0x33333333'i32]
  a.m = [[0x0102'i16, 0x0304'i16], [0x0506'i16, 0x0708'i16]]
  a.bs = [0xAA'u8, 0xBB'u8, 0xCC'u8, 0xDD'u8]
  echo toHex(a.marshalXCDR(eLE))
  echo toHex(a.marshalXCDR(eBE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlnim_array_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
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
    let n = emit(UNION_IDL);
    assert!(n.contains("disc*: int32"), "{n}");
    assert!(n.contains("case self.disc"), "{n}");
    assert!(n.contains("of 1:"), "{n}");
    assert!(n.contains("of 2:"), "{n}");
    assert!(n.contains("else:"), "{n}");
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
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP union byte: `nim` not on PATH");
        return;
    }
    let mut src = emit(UNION_IDL);
    src.push_str(
        r#"
when isMainModule:
  proc toHex(b: seq[byte]): string =
    const hexd = "0123456789abcdef"
    result = newStringOfCap(b.len * 2)
    for x in b:
      result.add(hexd[int(x) shr 4])
      result.add(hexd[int(x) and 0xf])
  var u: U
  u.disc = 2'i32
  u.b = 0x1234'u16
  echo toHex(u.marshalXCDR(eLE))
  echo toHex(u.marshalXCDR(eBE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlnim_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
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
fn map_emits_sorted_marshal() {
    let n = emit(MAP_IDL);
    assert!(n.contains("m*: Table[int32, uint32]"), "{n}");
    assert!(n.contains("sort(zdKeys)"), "{n}");
    assert!(n.contains("import std/tables"), "{n}");
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
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP map byte: `nim` not on PATH");
        return;
    }
    let mut src = emit(MAP_IDL);
    src.push_str(
        r#"
when isMainModule:
  proc toHex(b: seq[byte]): string =
    const hexd = "0123456789abcdef"
    result = newStringOfCap(b.len * 2)
    for x in b:
      result.add(hexd[int(x) shr 4])
      result.add(hexd[int(x) and 0xf])
  var h: HasMap
  h.m = {1'i32: 0x11111111'u32, 2'i32: 0x22222222'u32}.toTable
  echo toHex(h.marshalXCDR(eLE))
  echo toHex(h.marshalXCDR(eBE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlnim_map_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
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
    let n = emit(MUTABLE_IDL);
    assert!(n.contains("body.putU32(uint32(0x4000000a))"), "{n}");
    assert!(n.contains("body.putU32(uint32(0x40000014))"), "{n}");
    assert!(n.contains("body.putU32(uint32(0x4000001e))"), "{n}");
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
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP mutable byte: `nim` not on PATH");
        return;
    }
    let mut src = emit(MUTABLE_IDL);
    src.push_str(
        r#"
when isMainModule:
  proc toHex(b: seq[byte]): string =
    const hexd = "0123456789abcdef"
    result = newStringOfCap(b.len * 2)
    for x in b:
      result.add(hexd[int(x) shr 4])
      result.add(hexd[int(x) and 0xf])
  var m: M
  m.x = 0xDEADBEEF'u32
  m.s = "mut"
  m.k = 0x0777'u16
  echo toHex(m.marshalXCDR(eLE))
  echo toHex(m.marshalXCDR(eBE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlnim_mutable_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
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

#[test]
fn wide_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP wide byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP wide byte: `nim` not on PATH");
        return;
    }
    let mut src = emit(WIDE_IDL);
    src.push_str(
        "\nwhen isMainModule:\n  proc toHex(b: seq[byte]): string =\n    const hexd = \"0123456789abcdef\"\n    result = newStringOfCap(b.len * 2)\n    for x in b:\n      result.add(hexd[int(x) shr 4])\n      result.add(hexd[int(x) and 0xf])\n  var w: W\n  w.c = 0x03A9'u32\n  w.s = \"w\\u03c0\"\n  echo toHex(w.marshalXCDR(eLE))\n  echo toHex(w.marshalXCDR(eBE))\n",
    );
    let dir = std::env::temp_dir().join(format!("idlnim_wide_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_of(Path::new(&golden_dir).join("golden_wide_le.bin"))
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_of(Path::new(&golden_dir).join("golden_wide_be.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn longdouble_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP ld byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP ld byte: `nim` not on PATH");
        return;
    }
    let mut src = emit(LD_IDL);
    src.push_str(
        "\nwhen isMainModule:\n  proc toHex(b: seq[byte]): string =\n    const hexd = \"0123456789abcdef\"\n    result = newStringOfCap(b.len * 2)\n    for x in b:\n      result.add(hexd[int(x) shr 4])\n      result.add(hexd[int(x) and 0xf])\n  var l: L\n  l.d = 1.1\n  echo toHex(l.marshalXCDR(eLE))\n  echo toHex(l.marshalXCDR(eBE))\n",
    );
    let dir = std::env::temp_dir().join(format!("idlnim_ld_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        hex_of(Path::new(&golden_dir).join("golden_longdouble_le.bin"))
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        hex_of(Path::new(&golden_dir).join("golden_longdouble_be.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
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
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash: `nim` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_IDL);
    src.push_str("\nwhen isMainModule:\n  proc toHex(b: seq[byte]): string =\n    const hexd = \"0123456789abcdef\"\n    result = newStringOfCap(b.len * 2)\n    for x in b:\n      result.add(hexd[int(x) shr 4])\n      result.add(hexd[int(x) and 0xf])\n  var k: K\n  k.a = 0x01020304'i32\n  k.b = 0x0506'u16\n  echo toHex(@(k.keyHash()))\n");
    let dir = std::env::temp_dir().join(format!("idlnim_kh_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_of(Path::new(&golden_dir).join("golden_keyhash.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
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
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash_md5: `nim` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_MD5_IDL);
    src.push_str("\nwhen isMainModule:\n  proc toHex(b: seq[byte]): string =\n    const hexd = \"0123456789abcdef\"\n    result = newStringOfCap(b.len * 2)\n    for x in b:\n      result.add(hexd[int(x) shr 4])\n      result.add(hexd[int(x) and 0xf])\n  var k: KL\n  k.a = 0x01020304'i32\n  k.b = 0x05060708'i32\n  k.c = 0x090A0B0C'i32\n  k.d = 0x0D0E0F10'i32\n  k.e = 0x11121314'i32\n  echo toHex(@(k.keyHash()))\n");
    let dir = std::env::temp_dir().join(format!("idlnim_kh_md5_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_of(Path::new(&golden_dir).join("golden_keyhash_md5.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const NESTED_KEY_PARTIAL_IDL: &str = "\
@final struct Inner { @key long x; long ignored; @key long y; };
@final struct Outer { @key Inner i; };";

#[test]
fn nested_struct_key_hash_includes_only_nested_key_members() {
    // Bug A regression: `i`'s KeyHash contribution must be exactly `Inner`'s
    // own `@key` members (x, y), in member-id order — NOT the full member set
    // (x, ignored, y) that `marshalInto` (normal encoding) emits for Inner.
    let n = emit(NESTED_KEY_PARTIAL_IDL);
    let key_hash_body = n
        .split("proc keyHash*(self: Outer): array[16, byte] =")
        .nth(1)
        .expect("key_hash body");
    let end = key_hash_body
        .find("\n\nproc")
        .unwrap_or(key_hash_body.len());
    let body = &key_hash_body[..end];
    assert!(body.contains("self.i.x"), "{body}");
    assert!(body.contains("self.i.y"), "{body}");
    assert!(!body.contains("self.i.ignored"), "{body}");
    // The nested struct's full `marshalInto` must NOT be called for the key.
    assert!(!body.contains("marshalInto"), "{body}");
    // Normal (non-key) encoding of `i` in `marshalInto` (Outer) is untouched:
    // it must still call the struct's full marshalInto.
    assert!(n.contains("self.i.marshalInto(w)"), "{n}");
}

const NESTED_KEY_SMALL_IDL: &str = "\
@final struct Inner { @key octet a; };
@final struct Outer { @key Inner i; };";

#[test]
fn nested_struct_small_key_takes_zero_pad_branch_not_md5() {
    // Bug B regression: with a real `structs` map fed into `uses_md5`, a
    // small nested-struct `@key` (1 byte, well under the 16-byte KeyHash
    // boundary — XTypes 1.3 §7.6.8.4 step 5) must take the zero-pad branch,
    // not be forced into MD5 by an unresolvable (previously empty) structs
    // map.
    let n = emit(NESTED_KEY_SMALL_IDL);
    let key_hash_body = n
        .split("proc keyHash*(self: Outer): array[16, byte] =")
        .nth(1)
        .expect("key_hash body");
    let end = key_hash_body
        .find("\n\nproc")
        .unwrap_or(key_hash_body.len());
    let body = &key_hash_body[..end];
    assert!(body.contains("for i in 0 ..< min(16, b.len):"), "{body}");
    assert!(!body.contains("toMD5"), "{body}");
}

const ARRAY_KEY_IDL: &str = "\
@final struct S { @key octet id[4]; long tail; };";

#[test]
fn array_key_field_iterates_elements_not_scalar_encoded() {
    // REGRESSION (over/mis-inclusion, same bug class as #20): `FieldGen.
    // type_spec` used to be set to `resolved.clone()` identically for
    // `Declarator::Simple` AND `Declarator::Array` — for Array, `resolved`
    // is the ELEMENT type (`octet`, not "array of 4 octets"). `keyHash`
    // then called `map_key_type(&f.type_spec, "self.id", ..)` for the `@key`
    // field, which — believing it was handed a scalar octet — emitted a
    // single call against the WHOLE ARRAY value instead of iterating its 4
    // elements. Fixed: an array-declarator `@key` field now reuses `f.put`
    // unchanged (the same indexed `for` loop the general, non-key encoder
    // uses), mirroring `idl-lua`'s `key_type: Option<..>` guard (`None` for
    // `Declarator::Array`).
    let n = emit(ARRAY_KEY_IDL);
    let key_hash_body = n
        .split("proc keyHash*(self: S): array[16, byte] =")
        .nth(1)
        .expect("key_hash body");
    let end = key_hash_body
        .find("\n\nproc")
        .unwrap_or(key_hash_body.len());
    let body = &key_hash_body[..end];
    assert!(
        body.contains("for i0 in 0 ..< 4:"),
        "array @key field must iterate its elements (same shape as the general encoder):\n{body}"
    );
    assert!(
        body.contains("self.id[i0]"),
        "array @key field must index each element, not read the whole array:\n{body}"
    );
    assert!(
        !body.contains(", self.id)") && !body.contains("(self.id)"),
        "array @key field must NOT be scalar-encoded against the whole array value:\n{body}"
    );
    // `tail` (not `@key`) must not appear in the KeyHash body at all.
    assert!(!body.contains("self.tail"), "{body}");
}

fn hex_of(p: std::path::PathBuf) -> String {
    std::fs::read(&p)
        .unwrap_or_else(|_| panic!("read {}", p.display()))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Decode roundtrip: `marshal(unmarshal(golden)) == golden` for LE and BE.
/// Proves the generated `unmarshalXCDR{ty}` is the exact inverse of `marshalXCDR`.
fn run_roundtrip(idl: &str, ty: &str, le_file: &str, be_file: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP roundtrip {ty}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP roundtrip {ty}: `nim` not on PATH");
        return;
    }
    let le = hex_of(Path::new(&golden_dir).join(le_file));
    let be = hex_of(Path::new(&golden_dir).join(be_file));
    let mut src = emit(idl);
    src.push_str(&format!(
        r#"
when isMainModule:
  proc toHex(b: seq[byte]): string =
    const hexd = "0123456789abcdef"
    result = newStringOfCap(b.len * 2)
    for x in b:
      result.add(hexd[int(x) shr 4])
      result.add(hexd[int(x) and 0xf])
  proc nib(c: char): int =
    if c >= '0' and c <= '9': int(c) - int('0')
    elif c >= 'a' and c <= 'f': int(c) - int('a') + 10
    else: int(c) - int('A') + 10
  proc fromHex(s: string): seq[byte] =
    result = @[]
    var i = 0
    while i < s.len:
      result.add(byte((nib(s[i]) shl 4) or nib(s[i + 1])))
      i += 2
  echo toHex(unmarshalXCDR{ty}(fromHex("{le}"), eLE).marshalXCDR(eLE))
  echo toHex(unmarshalXCDR{ty}(fromHex("{be}"), eBE).marshalXCDR(eBE))
"#
    ));
    let dir = std::env::temp_dir().join(format!("idlnim_rt_{ty}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
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

// Welle C.2 #14: IDL identifiers colliding with Nim keywords must be
// escaped with backtick stropping, at every emit site (type names, enum
// names, field names) — not just structurally present but legal Nim.
// `nim` is not installed on this dev machine, so real-compile validation
// is deferred to central serial validation on codepit; this test proves
// structural escaping correctness only.
const NIM_KEYWORD_IDL: &str = "\
enum type { proc, object };
@final struct block {
    unsigned long var;
    unsigned long template;
};";

#[test]
fn keyword_identifiers_are_escaped_in_output() {
    let n = emit(NIM_KEYWORD_IDL);
    assert!(n.contains("type `type`* = enum"), "{n}");
    // Enumerators are always fused with the raw (unescaped) enum name —
    // `typeproc`/`typeobject` never collide with a standalone keyword.
    assert!(n.contains("typeproc = 0"), "{n}");
    assert!(n.contains("typeobject = 1"), "{n}");
    assert!(n.contains("type `block`* = object"), "{n}");
    assert!(n.contains("`var`*: uint32"), "{n}");
    assert!(n.contains("`template`*: uint32"), "{n}");
    // No bare (unescaped) keyword declaration token leaked through.
    assert!(!n.contains("type type* = enum"), "{n}");
    assert!(!n.contains("type block* = object"), "{n}");
}

// ===========================================================================
// Section-F Wave-1: bitset/bitmask, @optional, fixed<d,s>, sequence-arbitrary,
// @verbatim. Always-on source-asserts + nim-gated compile-and-run tests whose
// expected wire hex is derived from the spec IN the test (no GOLDEN_DIR oracle).
// ===========================================================================

/// A shared `toHex` helper (module scope), appended before each test's main.
const NIM_HEX: &str = r#"
proc toHex(b: seq[byte]): string =
  const hexd = "0123456789abcdef"
  result = newStringOfCap(b.len * 2)
  for x in b:
    result.add(hexd[int(x) shr 4])
    result.add(hexd[int(x) and 0xf])
"#;

/// Compiles `emit(idl) + NIM_HEX + main_body` with `nim c -r`, runs it, and
/// returns the trimmed stdout lines. `None` (skip) if `nim` is not on PATH.
fn nim_lines(idl: &str, main_body: &str, tag: &str) -> Option<Vec<String>> {
    if Command::new("nim").arg("--version").output().is_err() {
        eprintln!("SKIP {tag}: `nim` not on PATH");
        return None;
    }
    let mut src = emit(idl);
    src.push_str(NIM_HEX);
    src.push_str(main_body);
    let dir = std::env::temp_dir().join(format!("idlnim_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let nf = dir.join("main.nim");
    std::fs::write(&nf, &src).expect("write");
    let out = Command::new("nim")
        .args([
            "c",
            "-r",
            "--hints:off",
            "--warnings:off",
            "--nimcache:nimc",
        ])
        .arg(&nf)
        .current_dir(&dir)
        .output()
        .expect("nim c -r");
    assert!(
        out.status.success(),
        "nim c -r failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let lines: Vec<String> = stdout.lines().map(|l| l.trim().to_string()).collect();
    let _ = std::fs::remove_dir_all(&dir);
    Some(lines)
}

// ---- bitset -------------------------------------------------------------

const BITSET_IDL: &str = "bitset Flags { bitfield<4> a; bitfield<8> b; };";

#[test]
fn bitset_emits_holder_object_and_accessors() {
    let n = emit(BITSET_IDL);
    // 4 + 8 = 12 bits → uint16 backing (XTypes §7.4.7).
    assert!(n.contains("type Flags* = object"), "{n}");
    assert!(n.contains("storage*: uint16"), "{n}");
    assert!(n.contains("proc a*(self: Flags): uint16"), "{n}");
    assert!(n.contains("proc b*(self: Flags): uint16"), "{n}");
    assert!(
        n.contains("proc marshalInto*(self: Flags, w: var Writer) ="),
        "{n}"
    );
    assert!(n.contains("w.putU16(int(self.storage))"), "{n}");
    assert!(n.contains("proc unmarshalXCDRFlags*"), "{n}");
}

#[test]
fn bitset_wire_is_backing_int() {
    // storage 0xABCD as a uint16 → LE "cdab", BE "abcd"; round-trips.
    let body = r#"
when isMainModule:
  var f: Flags
  f.storage = 0xABCD'u16
  echo toHex(f.marshalXCDR(eLE))
  echo toHex(f.marshalXCDR(eBE))
  echo toHex(unmarshalXCDRFlags(f.marshalXCDR(eLE), eLE).marshalXCDR(eLE))
"#;
    let Some(l) = nim_lines(BITSET_IDL, body, "bitset") else {
        return;
    };
    assert_eq!(l[0], "cdab", "LE");
    assert_eq!(l[1], "abcd", "BE");
    assert_eq!(l[2], "cdab", "round-trip");
}

// ---- bitmask ------------------------------------------------------------

const BITMASK_IDL: &str = "bitmask Perms { PERM_READ, PERM_WRITE, PERM_EXEC };";

#[test]
fn bitmask_emits_holder_and_constants() {
    let n = emit(BITMASK_IDL);
    // Default @bit_bound = 32 → uint32 backing (XTypes §7.3.1.2.1.1).
    assert!(n.contains("type Perms* = object"), "{n}");
    assert!(n.contains("storage*: uint32"), "{n}");
    assert!(
        n.contains("const PermsPERM_READ*: uint32 = uint32(1) shl 0"),
        "{n}"
    );
    assert!(
        n.contains("const PermsPERM_EXEC*: uint32 = uint32(1) shl 2"),
        "{n}"
    );
    assert!(n.contains("w.putU32(self.storage)"), "{n}");
}

#[test]
fn bitmask_wire_is_backing_uint32() {
    // PERM_READ | PERM_EXEC = 0x05 → LE "05000000", BE "00000005"; round-trips.
    let body = r#"
when isMainModule:
  var p: Perms
  p.storage = PermsPERM_READ or PermsPERM_EXEC
  echo toHex(p.marshalXCDR(eLE))
  echo toHex(p.marshalXCDR(eBE))
  echo toHex(unmarshalXCDRPerms(p.marshalXCDR(eBE), eBE).marshalXCDR(eBE))
"#;
    let Some(l) = nim_lines(BITMASK_IDL, body, "bitmask") else {
        return;
    };
    assert_eq!(l[0], "05000000", "LE");
    assert_eq!(l[1], "00000005", "BE");
    assert_eq!(l[2], "00000005", "round-trip");
}

#[test]
fn bitmask_bit_bound_narrows_backing() {
    let n = emit("@bit_bound(8) bitmask Small { A, B };");
    assert!(n.contains("storage*: uint8"), "{n}");
    assert!(n.contains("w.putU8(int(self.storage))"), "{n}");
}

// ---- fixed<d,s> ---------------------------------------------------------

const FIXED_IDL: &str = "@final struct HasFixed { fixed<5,2> price; };";

#[test]
fn fixed_emits_bcd_field_and_prelude() {
    let n = emit(FIXED_IDL);
    assert!(n.contains("price*: seq[byte]"), "{n}");
    assert!(n.contains("w.putBytes(self.price)"), "{n}");
    assert!(
        n.contains("proc zdFixedEnc*(s: string, P: int, S: int): seq[byte] ="),
        "{n}"
    );
}

#[test]
fn fixed_wire_is_packed_bcd() {
    // fixed<5,2> 123.45 → BCD "12 34 5c" (odd P, no pad, CORBA §9.3.2.7).
    let body = r#"
when isMainModule:
  var h: HasFixed
  h.price = zdFixedEnc("123.45", 5, 2)
  echo toHex(h.marshalXCDR(eLE))
  echo toHex(h.marshalXCDR(eBE))
  echo toHex(unmarshalXCDRHasFixed(h.marshalXCDR(eLE), eLE).marshalXCDR(eLE))
"#;
    let Some(l) = nim_lines(FIXED_IDL, body, "fixed") else {
        return;
    };
    // Raw BCD bytes: identical for LE and BE (no byte-swap, no length prefix).
    assert_eq!(l[0], "12345c", "LE");
    assert_eq!(l[1], "12345c", "BE");
    assert_eq!(l[2], "12345c", "round-trip");
}

#[test]
fn fixed_even_p_keeps_msd() {
    // fixed<4,0> 1234 → BCD "01 23 4c" (leading pad nibble; even P keeps MSD).
    let body = r#"
when isMainModule:
  var h: HasFixed
  h.price = zdFixedEnc("1234", 4, 0)
  echo toHex(h.marshalXCDR(eLE))
"#;
    let Some(l) = nim_lines(
        "@final struct HasFixed { fixed<4,0> price; };",
        body,
        "fixed40",
    ) else {
        return;
    };
    assert_eq!(l[0], "01234c");
}

// ---- sequence-arbitrary -------------------------------------------------

const SEQARB_IDL: &str = "@final struct S { sequence<long> xs; };";

#[test]
fn sequence_arbitrary_emits_count_and_loop() {
    let n = emit(SEQARB_IDL);
    assert!(n.contains("xs*: seq[int32]"), "{n}");
    assert!(n.contains("w.putU32(uint32(self.xs.len))"), "{n}");
    assert!(n.contains("for zdElem in self.xs:"), "{n}");
}

#[test]
fn sequence_arbitrary_wire_count_plus_elements() {
    // [0x01020304, 0x05060708] → u32 count 2 + two i32 elements, no DHEADER.
    let body = r#"
when isMainModule:
  var s: S
  s.xs = @[0x01020304'i32, 0x05060708'i32]
  echo toHex(s.marshalXCDR(eLE))
  echo toHex(s.marshalXCDR(eBE))
  echo toHex(unmarshalXCDRS(s.marshalXCDR(eLE), eLE).marshalXCDR(eLE))
"#;
    let Some(l) = nim_lines(SEQARB_IDL, body, "seqarb") else {
        return;
    };
    assert_eq!(l[0], "020000000403020108070605", "LE");
    assert_eq!(l[1], "000000020102030405060708", "BE");
    assert_eq!(l[2], "020000000403020108070605", "round-trip");
}

#[test]
fn sequence_of_enum_is_arbitrary_path() {
    // A `sequence<enum>` must now emit (was rejected pre-Wave-1).
    let n = emit("enum E { E0, E1 }; @final struct SE { sequence<E> es; };");
    assert!(n.contains("es*: seq[E]"), "{n}");
    assert!(n.contains("for zdElem in self.es:"), "{n}");
    assert!(n.contains("w.putU32(uint32(self.es.len))"), "{n}");
}

// ---- @optional ----------------------------------------------------------

const OPT_IDL: &str = "@final struct Opt { uint32 a; @optional uint32 b; };";

#[test]
fn optional_emits_presence_flag() {
    let n = emit(OPT_IDL);
    assert!(n.contains("b_present*: bool"), "{n}");
    assert!(n.contains("w.putU8(if self.b_present: 1 else: 0)"), "{n}");
    assert!(n.contains("if self.b_present:"), "{n}");
    assert!(n.contains("result.b_present = r.getBool()"), "{n}");
    assert!(n.contains("if result.b_present:"), "{n}");
}

#[test]
fn optional_final_wire_present_and_absent() {
    // present: u32 a, u8 flag=1, pad(3), u32 b. absent: u32 a, u8 flag=0.
    let body = r#"
when isMainModule:
  var p: Opt
  p.a = 0x11223344'u32
  p.b_present = true
  p.b = 0xAABBCCDD'u32
  echo toHex(p.marshalXCDR(eLE))
  echo toHex(p.marshalXCDR(eBE))
  var q: Opt
  q.a = 0x11223344'u32
  q.b_present = false
  echo toHex(q.marshalXCDR(eLE))
  echo toHex(unmarshalXCDROpt(p.marshalXCDR(eLE), eLE).marshalXCDR(eLE))
  echo toHex(unmarshalXCDROpt(q.marshalXCDR(eLE), eLE).marshalXCDR(eLE))
"#;
    let Some(l) = nim_lines(OPT_IDL, body, "opt") else {
        return;
    };
    assert_eq!(l[0], "4433221101000000ddccbbaa", "present LE");
    assert_eq!(l[1], "1122334401000000aabbccdd", "present BE");
    assert_eq!(l[2], "4433221100", "absent LE");
    assert_eq!(l[3], "4433221101000000ddccbbaa", "present round-trip");
    assert_eq!(l[4], "4433221100", "absent round-trip");
}

#[test]
fn optional_appendable_round_trips() {
    // Appendable body is DHEADER-framed; verify decode∘encode identity for
    // both present and absent, without hand-computing the DHEADER length.
    let idl = "@appendable struct OptA { uint32 a; @optional uint32 b; };";
    let body = r#"
when isMainModule:
  var p: OptA
  p.a = 0xCAFEBABE'u32
  p.b_present = true
  p.b = 0x01020304'u32
  let pe = p.marshalXCDR(eLE)
  echo toHex(pe)
  echo toHex(unmarshalXCDROptA(pe, eLE).marshalXCDR(eLE))
  var q: OptA
  q.a = 0xCAFEBABE'u32
  q.b_present = false
  let qe = q.marshalXCDR(eLE)
  echo toHex(qe)
  echo toHex(unmarshalXCDROptA(qe, eLE).marshalXCDR(eLE))
"#;
    let Some(l) = nim_lines(idl, body, "opta") else {
        return;
    };
    assert_eq!(l[0], l[1], "present round-trip");
    assert_eq!(l[2], l[3], "absent round-trip");
}

// ---- @verbatim ----------------------------------------------------------

#[test]
fn verbatim_placements_inject_text() {
    let n = emit(
        "@verbatim(language=\"nim\", placement=BEGIN_FILE, text=\"# zd-begin-file\")\n\
         @verbatim(language=\"nim\", placement=BEFORE_DECLARATION, text=\"# zd-before\")\n\
         @verbatim(language=\"nim\", placement=BEGIN_DECLARATION, text=\"# zd-begin-decl\")\n\
         @verbatim(language=\"nim\", placement=END_DECLARATION, text=\"# zd-end-decl\")\n\
         @verbatim(language=\"nim\", placement=AFTER_DECLARATION, text=\"# zd-after\")\n\
         @verbatim(language=\"nim\", placement=END_FILE, text=\"# zd-end-file\")\n\
         @final struct V { uint32 a; };",
    );
    for marker in [
        "# zd-begin-file",
        "# zd-before",
        "# zd-begin-decl",
        "# zd-end-decl",
        "# zd-after",
        "# zd-end-file",
    ] {
        assert!(n.contains(marker), "missing {marker}:\n{n}");
    }
    // Ordering: begin-file before the object; before-decl before `type V`;
    // begin-decl after the object header; after-decl/end-file trail the type.
    let sidx = n.find("type V* = object").expect("object");
    assert!(n.find("# zd-begin-file").unwrap() < sidx, "{n}");
    assert!(n.find("# zd-before").unwrap() < sidx, "{n}");
    assert!(n.find("# zd-begin-decl").unwrap() > sidx, "{n}");
    assert!(n.find("# zd-end-file").unwrap() > sidx, "{n}");
}

#[test]
fn verbatim_language_filter_excludes_other_langs() {
    // A non-Nim language tag must NOT leak into the Nim output.
    let n = emit(
        "@verbatim(language=\"java\", placement=BEFORE_DECLARATION, text=\"# java-only\")\n\
         @final struct V { uint32 a; };",
    );
    assert!(!n.contains("# java-only"), "{n}");
    // The wildcard `*` still matches Nim.
    let n2 = emit(
        "@verbatim(placement=BEFORE_DECLARATION, text=\"# wildcard\")\n\
         @final struct V { uint32 a; };",
    );
    assert!(n2.contains("# wildcard"), "{n2}");
}

#[test]
fn verbatim_output_still_compiles() {
    let idl = "@verbatim(language=\"nim\", placement=BEGIN_FILE, text=\"# zd file header\")\n\
         @verbatim(language=\"nim\", placement=BEGIN_DECLARATION, text=\"# zd inside struct\")\n\
         @final struct V { uint32 a; };";
    let body = r#"
when isMainModule:
  var v: V
  v.a = 0x2A'u32
  echo toHex(v.marshalXCDR(eLE))
"#;
    let Some(l) = nim_lines(idl, body, "verbatim") else {
        return;
    };
    assert_eq!(l[0], "2a000000");
}
