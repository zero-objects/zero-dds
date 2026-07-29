// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Swift backend: string smoke tests (always) + a byte-identity test that
//! compiles+runs the generated Swift and compares to the Rust goldens (gated on
//! `swiftc` on PATH and `GOLDEN_DIR` pointing at golden_{le,be}.bin).

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
use zerodds_idl_swift::{SwiftGenOptions, generate_swift_module};

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
    generate_swift_module(&ast, &SwiftGenOptions::default()).expect("gen")
}

/// swarm59 #21b: `module X { struct Y { ... }; }` used to be silently
/// dropped (no `Definition::Module` arm at all) — the struct must now emit.
#[test]
fn module_wrapped_struct_is_emitted_not_dropped() {
    let s = emit("module Telemetry { @final struct Reading { long value; }; };");
    assert!(s.contains("public struct Reading {"), "{s}");
    assert!(s.contains("public var value: Int32"), "{s}");
}

/// A reopened module (`module M {} ... module M {}`) must not lose either
/// half's content once the AST builder merges the two occurrences.
#[test]
fn reopened_module_emits_both_structs() {
    let s = emit(
        "module M { @final struct A { long x; }; }; \
         module M { @final struct B { long y; }; };",
    );
    assert!(s.contains("public struct A {"), "{s}");
    assert!(s.contains("public struct B {"), "{s}");
}

#[test]
fn final_struct_emits_struct_and_marshal() {
    let s = emit(GOLDEN_IDL);
    assert!(s.contains("public struct Golden {"), "{s}");
    assert!(s.contains("public var id: UInt32"), "{s}");
    assert!(s.contains("public var kind: UInt16"), "{s}");
    assert!(s.contains("public var flags: UInt8"), "{s}");
    assert!(s.contains("public var value: Float"), "{s}");
    assert!(s.contains("public var stamp: UInt64"), "{s}");
    assert!(s.contains("public var label: String"), "{s}");
    assert!(s.contains("public var raw: [UInt8]"), "{s}");
    assert!(
        s.contains("public func marshalXCDR(_ endian: Endianness) throws -> [UInt8] {"),
        "{s}"
    );
    assert!(s.contains("w.putU32(id)"), "{s}");
    assert!(s.contains("w.putF32(value)"), "{s}");
    assert!(s.contains("w.putString(label)"), "{s}");
    assert!(s.contains("w.putSeqU8(raw)"), "{s}");
    assert!(!s.contains("let bb = body.bytes()"), "{s}");
}

#[test]
fn appendable_struct_frames_a_dheader() {
    let s = emit("@appendable struct S { uint32 a; };");
    assert!(s.contains("let bb = body.bytes()"), "{s}");
    assert!(s.contains("w.putU32(UInt32(bb.count))"), "{s}");
    assert!(s.contains("w.putBytes(bb)"), "{s}");
}

const ENUM_IDL: &str = "\
enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };
@final struct S { Mode kind; uint32 tail; };";

#[test]
fn enum_emits_int32_enum_and_member_marshals() {
    let s = emit(ENUM_IDL);
    assert!(s.contains("public enum Mode: Int32 {"), "{s}");
    assert!(s.contains("case MODE_FAULT = 2"), "{s}");
    assert!(s.contains("public var kind: Mode"), "{s}");
    // An enum member is a 32-bit signed integer on the wire (XTypes §7.4.5.1).
    assert!(
        s.contains("w.putU32(UInt32(bitPattern: kind.rawValue))"),
        "{s}"
    );
}

#[test]
fn enum_member_is_byte_identical_i32() {
    // Gated: needs swiftc (local macOS; codepit CI has no swiftc). S{ kind:
    // MODE_FAULT(=2), tail: 0xDEADBEEF } -> i32 LE 02000000 + u32 LE efbeadde.
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP enum byte test: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(ENUM_IDL);
    src.push_str(
        r##"
func toHex(_ b: [UInt8]) -> String { b.map { String(format: "%02x", $0) }.joined() }
let s = S(kind: .MODE_FAULT, tail: 0xDEADBEEF)
print(toHex(try s.marshalXCDR(.little)))
"##,
    );
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_enum_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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
        z.contains("public func marshalInto(_ w: inout Writer)"),
        "{z}"
    );
    assert!(z.contains("public var one: Inner"), "{z}");
    assert!(z.contains("public var many: [Inner]"), "{z}");
    assert!(z.contains("try one.marshalInto(&body)"), "{z}");
    assert!(z.contains("try e.marshalInto(&sub)"), "{z}");
}

#[test]
fn nested_is_byte_identical_vs_rust_golden() {
    // Gated: swiftc (local macOS; codepit CI has no swiftc).
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP nested byte: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP nested byte: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(NESTED_IDL);
    src.push_str(
        r##"
func toHex(_ b: [UInt8]) -> String { b.map { String(format: "%02x", $0) }.joined() }
let inners = [Inner(a: 0xAAAA, b: 0xBBBBCCCC), Inner(a: 0xDDDD, b: 0xEEEEFFFF)]
let o = Outer(id: 0xCAFEBABE, one: Inner(a: 0x1111, b: 0x22223333), many: inners, label: "nested")
print(toHex(try o.marshalXCDR(.little)))
print(toHex(try o.marshalXCDR(.big)))
"##,
    );
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_nested_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP byte_identity: `swiftc` not on PATH");
        return;
    }

    let mut src = emit(GOLDEN_IDL);
    src.push_str(
        r#"
func toHex(_ b: [UInt8]) -> String {
    b.map { String(format: "%02x", $0) }.joined()
}
import Foundation
let g = Golden(
    id: 0xA1B2C3D4, kind: 0x1234, flags: 0x5A, value: 3.5,
    stamp: 0x0102030405060708, label: "bay-12", raw: [0xDE, 0xAD, 0xBE, 0xEF]
)
print(toHex(try g.marshalXCDR(.little)))
print(toHex(try g.marshalXCDR(.big)))
"#,
    );
    // Foundation import must precede code; hoist it to the top.
    let src = src.replacen("\nimport Foundation\n", "\n", 1);
    let src = format!("import Foundation\n{src}");

    let dir = std::env::temp_dir().join(format!("idlswift_golden_{}", std::process::id()));
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
    assert!(
        run.status.success(),
        "run failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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
    let s = emit(TYPEDEF_IDL);
    assert!(s.contains("public var name: String"), "{s}");
    assert!(s.contains("public var data: [UInt8]"), "{s}");
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
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP typedef byte: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(TYPEDEF_IDL);
    src.push_str(
        r##"
func toHex(_ b: [UInt8]) -> String { b.map { String(format: "%02x", $0) }.joined() }
let r = Rec(id: 0xCAFEBABE, name: "typedef", data: [1, 2, 3])
print(toHex(try r.marshalXCDR(.little)))
print(toHex(try r.marshalXCDR(.big)))
"##,
    );
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_typedef_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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
    let s = emit(ARRAY_IDL);
    assert!(s.contains("public var xs: [Int32]"), "{s}");
    assert!(s.contains("public var m: [[Int16]]"), "{s}");
    assert!(s.contains("public var bs: [UInt8]"), "{s}");
    assert!(s.contains("for zdi0 in 0..<3"), "{s}");
    assert!(s.contains("for zdi1 in 0..<2"), "{s}");
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
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP array byte: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(ARRAY_IDL);
    src.push_str(
        r##"
func toHex(_ b: [UInt8]) -> String { b.map { String(format: "%02x", $0) }.joined() }
let a = Arr(xs: [0x11111111, 0x22222222, 0x33333333], m: [[0x0102, 0x0304], [0x0506, 0x0708]], bs: [0xAA, 0xBB, 0xCC, 0xDD])
print(toHex(try a.marshalXCDR(.little)))
print(toHex(try a.marshalXCDR(.big)))
"##,
    );
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_array_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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
    let s = emit(UNION_IDL);
    assert!(s.contains("public var disc: Int32"), "{s}");
    assert!(s.contains("switch disc {"), "{s}");
    assert!(s.contains("case 1:"), "{s}");
    assert!(s.contains("case 2:"), "{s}");
    assert!(s.contains("default:"), "{s}");
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
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP union byte: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(UNION_IDL);
    src.push_str(
        r##"
func toHex(_ b: [UInt8]) -> String { b.map { String(format: "%02x", $0) }.joined() }
let u = U(disc: 2, a: 0, b: 0x1234, c: 0)
print(toHex(try u.marshalXCDR(.little)))
print(toHex(try u.marshalXCDR(.big)))
"##,
    );
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_union_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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
    let s = emit(MAP_IDL);
    assert!(s.contains("public var m: [Int32: UInt32]"), "{s}");
    assert!(s.contains(".keys.sorted()"), "{s}");
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
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP map byte: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(MAP_IDL);
    src.push_str(
        r##"
func toHex(_ b: [UInt8]) -> String { b.map { String(format: "%02x", $0) }.joined() }
let h = HasMap(m: [1: 0x11111111, 2: 0x22222222])
print(toHex(try h.marshalXCDR(.little)))
print(toHex(try h.marshalXCDR(.big)))
"##,
    );
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_map_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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
    let s = emit(MUTABLE_IDL);
    assert!(s.contains("body.putU32(0x4000000a)"), "{s}");
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
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP mutable byte: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(MUTABLE_IDL);
    src.push_str(
        r##"
func toHex(_ b: [UInt8]) -> String { b.map { String(format: "%02x", $0) }.joined() }
let m = M(x: 0xDEADBEEF, s: "mut", k: 0x0777)
print(toHex(try m.marshalXCDR(.little)))
print(toHex(try m.marshalXCDR(.big)))
"##,
    );
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_mutable_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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

fn run_swift(idl: &str, ctor: &str, stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP {stem}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP {stem}: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(idl);
    src.push_str(&format!("\nfunc toHex(_ b: [UInt8]) -> String {{ b.map {{ String(format: \"%02x\", $0) }}.joined() }}\nlet v = {ctor}\nprint(toHex(try v.marshalXCDR(.little)))\nprint(toHex(try v.marshalXCDR(.big)))\n"));
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_{stem}_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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
    run_swift(WIDE_IDL, "W(c: 0x03A9, s: \"w\u{03c0}\")", "wide");
}
#[test]
fn longdouble_is_byte_identical_vs_rust_golden() {
    run_swift(LD_IDL, "L(d: 1.1)", "longdouble");
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
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_IDL);
    src.push_str("\nfunc toHex(_ b: [UInt8]) -> String { b.map { String(format: \"%02x\", $0) }.joined() }\nlet k = K(a: 0x01020304, b: 0x0506, c: 0)\nprint(toHex(try k.keyHash()))\n");
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_kh_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP roundtrip {ty}: `swiftc` not on PATH");
        return;
    }
    let le = hex_of(Path::new(&golden_dir).join(le_file));
    let be = hex_of(Path::new(&golden_dir).join(be_file));
    let mut src = emit(idl);
    src.push_str(&format!(
        r##"
func toHex(_ b: [UInt8]) -> String {{ b.map {{ String(format: "%02x", $0) }}.joined() }}
func fromHex(_ s: String) -> [UInt8] {{
    var r = [UInt8](); var i = s.startIndex
    while i < s.endIndex {{ let j = s.index(i, offsetBy: 2); r.append(UInt8(s[i..<j], radix: 16)!); i = j }}
    return r
}}
print(toHex(try {ty}.unmarshalXCDR(fromHex("{le}"), .little).marshalXCDR(.little)))
print(toHex(try {ty}.unmarshalXCDR(fromHex("{be}"), .big).marshalXCDR(.big)))
"##
    ));
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_rt_{ty}_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
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
    // 5×@key long = 20 bytes > 16 → MD5 branch (XTypes §7.6.8.4). Local swiftc.
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP keyhash_md5: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash_md5: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_MD5_IDL);
    src.push_str("\nfunc toHex(_ b: [UInt8]) -> String { b.map { String(format: \"%02x\", $0) }.joined() }\nlet k = KL(a: 0x01020304, b: 0x05060708, c: 0x090A0B0C, d: 0x0D0E0F10, e: 0x11121314)\nprint(toHex(try k.keyHash()))\n");
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_kh_md5_{}", std::process::id()));
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
    let stdout = String::from_utf8(run.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_of(Path::new(&golden_dir).join("golden_keyhash_md5.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// --- Regression: nested-struct `@key` member must expand to only the inner
// struct's own `@key` members (XTypes 1.3 §7.6.8), not its full member set.
// Before the fix, `Outer`'s `keyHash()` called `Inner.marshalInto`, which
// serializes `x`, `ignored`, AND `y` — leaking the non-key `ignored` field
// into the KeyHash.

const NESTED_KEY_SUBSET_IDL: &str = "\
@final struct Inner { @key long x; long ignored; @key long y; };
@final struct Outer { @key Inner i; };";

#[test]
fn nested_key_member_excludes_non_key_fields() {
    let o = emit(NESTED_KEY_SUBSET_IDL);
    // `Inner` also has its own `@key` members (x, y), so it emits its own
    // `keyHash()` too — take the LAST occurrence (Outer's, since Outer is
    // emitted after Inner) rather than the first.
    let key_hash = o
        .rsplit("public func keyHash() throws -> [UInt8] {")
        .next()
        .expect("Outer struct should emit a keyHash() function");
    // Bug A fix: the inner struct's own `@key` fields x and y are written
    // directly (the marshal never calls `Inner.marshalInto`, which would
    // pull in `ignored` too).
    assert!(
        !key_hash.contains("i.marshalInto"),
        "keyHash must not call the nested struct's full marshalInto: {o}"
    );
    assert!(
        key_hash.contains("i.x") && key_hash.contains("i.y"),
        "keyHash must encode the nested struct's own @key fields x and y: {o}"
    );
    assert!(
        !key_hash.contains("i.ignored"),
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
    // `Inner` also emits its own trivial `keyHash()` (single `@key octet`);
    // take the LAST occurrence (Outer's) rather than the first.
    let key_hash = o
        .rsplit("public func keyHash() throws -> [UInt8] {")
        .next()
        .expect("Outer struct should emit a keyHash() function");
    // Inner has a single @key octet -> KeyHolder max size 1 byte <= 16 ->
    // zero-pad branch ([UInt8](repeating: 0, ...)), NOT the MD5 branch.
    assert!(
        key_hash.contains("repeating: 0, count: 16"),
        "a 1-byte nested-struct key must take the zero-pad branch: {o}"
    );
    assert!(
        !key_hash.contains("Insecure.MD5"),
        "a 1-byte nested-struct key must NOT take the MD5 branch: {o}"
    );
    assert!(key_hash.contains("i.a"), "{o}");
}

const KEYWORD_IDL: &str = "\
enum Self { guard, protocol, extension };
@final struct class {
    long var;
    unsigned short for;
    Self func;
};";

#[test]
fn keyword_colliding_identifiers_are_backtick_escaped() {
    let s = emit(KEYWORD_IDL);
    assert!(s.contains("public enum `Self`: Int32 {"), "{s}");
    assert!(s.contains("case `guard` = 0"), "{s}");
    assert!(s.contains("case `protocol` = 1"), "{s}");
    assert!(s.contains("case `extension` = 2"), "{s}");
    assert!(s.contains("public struct `class` {"), "{s}");
    assert!(s.contains("public var `var`: Int32"), "{s}");
    assert!(s.contains("public var `for`: UInt16"), "{s}");
    assert!(s.contains("public var `func`: `Self`"), "{s}");
    // No bare (unescaped) keyword ever appears as a declaration/type token.
    assert!(!s.contains("public enum Self:"), "{s}");
    assert!(!s.contains("public struct class {"), "{s}");
}

#[test]
fn keyword_colliding_identifiers_compile_with_swiftc() {
    if Command::new("swiftc").arg("--version").output().is_err() {
        eprintln!("SKIP keyword_colliding_identifiers_compile_with_swiftc: `swiftc` not on PATH");
        return;
    }
    let mut src = emit(KEYWORD_IDL);
    src.push_str(
        "\nlet v = `class`(var: 1, for: 2, func: .guard)\n_ = try v.marshalXCDR(.little)\nlet r = try `class`.unmarshalXCDR(v.marshalXCDR(.little), .little)\nprecondition(r.var == 1)\n",
    );
    let src = format!("import Foundation\n{src}");
    let dir = std::env::temp_dir().join(format!("idlswift_kw_{}", std::process::id()));
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
    assert!(run.status.success(), "runtime precondition failed");
    let _ = std::fs::remove_dir_all(&dir);
}
