// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Lua backend: string smoke tests (always) + a byte-identity test that runs
//! the generated Lua and compares to the Rust goldens (gated on `lua5.4` on
//! PATH and `GOLDEN_DIR` pointing at golden_{le,be}.bin).

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
use zerodds_idl_lua::{LuaGenOptions, generate_lua_module};

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
    generate_lua_module(&ast, &LuaGenOptions::default()).expect("gen")
}

/// swarm59 #21b: `module X { struct Y { ... }; }` used to be silently
/// dropped (no `Definition::Module` arm at all) — the struct must now emit.
#[test]
fn module_wrapped_struct_is_emitted_not_dropped() {
    let l = emit("module Telemetry { @final struct Reading { long value; }; };");
    // #21: a module-wrapped type is emitted with its module-qualified name.
    assert!(
        l.contains("function marshal_Telemetry_Reading(v, endian)"),
        "{l}"
    );
    assert!(l.contains("w:putU32(v.value"), "{l}");
}

/// A reopened module (`module M {} ... module M {}`) must not lose either
/// half's content once the AST builder merges the two occurrences.
#[test]
fn reopened_module_emits_both_structs() {
    let l = emit(
        "module M { @final struct A { long x; }; }; \
         module M { @final struct B { long y; }; };",
    );
    // #21: both halves emit under the module-qualified name `M_*`.
    assert!(l.contains("function marshal_M_A(v, endian)"), "{l}");
    assert!(l.contains("function marshal_M_B(v, endian)"), "{l}");
}

/// #21 cross-module collision: two different modules each declaring `Reading`
/// must emit distinct, module-qualified marshallers, never a duplicate one.
#[test]
fn cross_module_same_name_types_are_qualified() {
    let l = emit(
        "module a { @final struct Reading { long v; }; }; \
         module b { @final struct Reading { double w; }; };",
    );
    assert!(l.contains("function marshal_a_Reading(v, endian)"), "{l}");
    assert!(l.contains("function marshal_b_Reading(v, endian)"), "{l}");
    assert!(!l.contains("function marshal_Reading(v, endian)"), "{l}");
}

/// #21 cross-module reference: `module b`'s struct references `a::R`, which
/// must resolve to the qualified marshaller `a_R`, not the bare `R`.
#[test]
fn cross_module_reference_resolves_to_qualified_type() {
    let l = emit(
        "module a { @final struct R { long v; }; }; \
         module b { @final struct S { a::R r; }; };",
    );
    assert!(l.contains("function marshal_a_R(v, endian)"), "{l}");
    assert!(l.contains("function marshal_b_S(v, endian)"), "{l}");
    // S's member `r` marshals via the qualified nested marshaller a_R.
    assert!(l.contains("marshalInto_a_R"), "{l}");
    assert!(!l.contains("marshalInto_R("), "{l}");
}

/// #21 compile gate: a two-module spec with a cross-module reference must
/// produce runnable Lua.
#[test]
fn cross_module_reference_compiles_with_lua() {
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP cross_module_reference_compiles_with_lua: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(
        "module a { @final struct R { long v; }; }; \
         module b { @final struct S { a::R r; }; };",
    );
    src.push_str(
        "
local s = { r = { v = 7 } }
local _ = marshal_b_S(s, LE)
print(\"ok\")
",
    );
    let dir = std::env::temp_dir().join(format!("idllua_xmod_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn final_struct_emits_marshal() {
    let l = emit(GOLDEN_IDL);
    assert!(l.contains("function marshal_Golden(v, endian)"), "{l}");
    assert!(l.contains("w:putU32(v.id)"), "{l}");
    assert!(l.contains("w:putU16(v.kind)"), "{l}");
    assert!(l.contains("w:putU8(v.flags)"), "{l}");
    assert!(l.contains("w:putF32(v.value)"), "{l}");
    assert!(l.contains("w:putU64(v.stamp)"), "{l}");
    assert!(l.contains("w:putString(v.label)"), "{l}");
    assert!(l.contains("w:putSeqU8(v.raw)"), "{l}");
    assert!(!l.contains("local bb = body:bytes()"), "{l}");
}

#[test]
fn appendable_struct_frames_a_dheader() {
    let l = emit("@appendable struct S { uint32 a; };");
    assert!(l.contains("local bb = body:bytes()"), "{l}");
    assert!(l.contains("w:putU32(#bb)"), "{l}");
    assert!(l.contains("w:putBytes(bb)"), "{l}");
}

const ENUM_IDL: &str = "\
enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };
@final struct S { Mode kind; uint32 tail; };";

#[test]
fn enum_emits_table_and_member_marshals() {
    let l = emit(ENUM_IDL);
    assert!(
        l.contains("local Mode = { MODE_IDLE = 0, MODE_ACTIVE = 1, MODE_FAULT = 2 }"),
        "{l}"
    );
    // An enum member is a 32-bit signed integer on the wire (XTypes §7.4.5.1).
    assert!(l.contains("w:putU32(v.kind & 0xffffffff)"), "{l}");
}

#[test]
fn enum_member_is_byte_identical_i32() {
    // Gated: needs lua5.4. S{ kind: MODE_FAULT(=2), tail: 0xDEADBEEF }
    // -> i32 LE 02000000 + u32 LE efbeadde.
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP enum byte test: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(ENUM_IDL);
    src.push_str(
        "
local function toHex(s)
  local out = {}
  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end
  return table.concat(out)
end
local s = { kind = Mode.MODE_FAULT, tail = 0xDEADBEEF }
print(toHex(marshal_S(s, LE)))
",
    );
    let dir = std::env::temp_dir().join(format!("idllua_enum_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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
    let l = emit(NESTED_IDL);
    assert!(l.contains("function marshalInto_Inner(w, v)"), "{l}");
    assert!(l.contains("marshalInto_Inner(body, v.one)"), "{l}");
    assert!(l.contains("marshalInto_Inner(sub, e)"), "{l}");
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
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP nested byte: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(NESTED_IDL);
    src.push_str(
        "
local function toHex(s)
  local out = {}
  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end
  return table.concat(out)
end
local o = {
  id = 0xCAFEBABE,
  one = { a = 0x1111, b = 0x22223333 },
  many = { { a = 0xAAAA, b = 0xBBBBCCCC }, { a = 0xDDDD, b = 0xEEEEFFFF } },
  label = \"nested\",
}
print(toHex(marshal_Outer(o, LE)))
print(toHex(marshal_Outer(o, BE)))
",
    );
    let dir = std::env::temp_dir().join(format!("idllua_nested_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP byte_identity: `lua5.4` not on PATH");
        return;
    }

    let mut src = emit(GOLDEN_IDL);
    src.push_str(
        "
local function toHex(s)
  local out = {}
  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end
  return table.concat(out)
end
local g = {
  id = 0xA1B2C3D4, kind = 0x1234, flags = 0x5A, value = 3.5,
  stamp = 0x0102030405060708, label = \"bay-12\", raw = \"\\xDE\\xAD\\xBE\\xEF\",
}
print(toHex(marshal_Golden(g, LE)))
print(toHex(marshal_Golden(g, BE)))
",
    );

    let dir = std::env::temp_dir().join(format!("idllua_golden_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");

    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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
    let l = emit(TYPEDEF_IDL);
    assert!(l.contains("function marshal_Rec(v, endian)"), "{l}");
    assert!(l.contains("w:putSeqU8(v.data)"), "{l}");
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
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP typedef byte: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(TYPEDEF_IDL);
    src.push_str(
        "
local function toHex(s)
  local out = {}
  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end
  return table.concat(out)
end
local r = { id = 0xCAFEBABE, name = \"typedef\", data = \"\\x01\\x02\\x03\" }
print(toHex(marshal_Rec(r, LE)))
print(toHex(marshal_Rec(r, BE)))
",
    );
    let dir = std::env::temp_dir().join(format!("idllua_typedef_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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
fn array_emits_fixed_array_loops() {
    let l = emit(ARRAY_IDL);
    assert!(l.contains("function marshal_Arr(v, endian)"), "{l}");
    assert!(l.contains("for zdi0 = 1, 3 do"), "{l}");
    assert!(l.contains("for zdi1 = 1, 2 do"), "{l}");
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
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP array byte: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(ARRAY_IDL);
    src.push_str(
        "
local function toHex(s)
  local out = {}
  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end
  return table.concat(out)
end
local a = {
  xs = { 0x11111111, 0x22222222, 0x33333333 },
  m = { { 0x0102, 0x0304 }, { 0x0506, 0x0708 } },
  bs = { 0xAA, 0xBB, 0xCC, 0xDD },
}
print(toHex(marshal_Arr(a, LE)))
print(toHex(marshal_Arr(a, BE)))
",
    );
    let dir = std::env::temp_dir().join(format!("idllua_array_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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
    let l = emit(UNION_IDL);
    assert!(l.contains("function marshal_U(v, endian)"), "{l}");
    assert!(l.contains("if v.disc == 1 then"), "{l}");
    assert!(l.contains("elseif v.disc == 2 then"), "{l}");
    assert!(l.contains("else"), "{l}");
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
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP union byte: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(UNION_IDL);
    src.push_str(
        "
local function toHex(s)
  local out = {}
  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end
  return table.concat(out)
end
local u = { disc = 2, a = 0, b = 0x1234, c = 0 }
print(toHex(marshal_U(u, LE)))
print(toHex(marshal_U(u, BE)))
",
    );
    let dir = std::env::temp_dir().join(format!("idllua_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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
    let l = emit(MAP_IDL);
    assert!(l.contains("function marshal_HasMap(v, endian)"), "{l}");
    assert!(l.contains("table.sort(zdKeys)"), "{l}");
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
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP map byte: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(MAP_IDL);
    src.push_str(
        "
local function toHex(s)
  local out = {}
  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end
  return table.concat(out)
end
local h = { m = { [1] = 0x11111111, [2] = 0x22222222 } }
print(toHex(marshal_HasMap(h, LE)))
print(toHex(marshal_HasMap(h, BE)))
",
    );
    let dir = std::env::temp_dir().join(format!("idllua_map_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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
    let l = emit(MUTABLE_IDL);
    assert!(l.contains("body:putU32(0x4000000a)"), "{l}");
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
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP mutable byte: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(MUTABLE_IDL);
    src.push_str(
        "
local function toHex(s)
  local out = {}
  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end
  return table.concat(out)
end
local m = { x = 0xDEADBEEF, s = \"mut\", k = 0x0777 }
print(toHex(marshal_M(m, LE)))
print(toHex(marshal_M(m, BE)))
",
    );
    let dir = std::env::temp_dir().join(format!("idllua_mutable_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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

fn run_lua(idl: &str, main_body: &str, stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP {stem}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP {stem}: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(idl);
    src.push_str(main_body);
    let dir = std::env::temp_dir().join(format!("idllua_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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

const LUA_HEX: &str = "\nlocal function toHex(s)\n  local out = {}\n  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end\n  return table.concat(out)\nend\n";

#[test]
fn wide_is_byte_identical_vs_rust_golden() {
    let body = format!(
        "{LUA_HEX}local w = {{ c = 0x03A9, s = \"w\u{03c0}\" }}\nprint(toHex(marshal_W(w, LE)))\nprint(toHex(marshal_W(w, BE)))\n"
    );
    run_lua(WIDE_IDL, &body, "wide");
}

#[test]
fn longdouble_is_byte_identical_vs_rust_golden() {
    let body = format!(
        "{LUA_HEX}local l = {{ d = 1.1 }}\nprint(toHex(marshal_L(l, LE)))\nprint(toHex(marshal_L(l, BE)))\n"
    );
    run_lua(LD_IDL, &body, "longdouble");
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
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP keyhash: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_IDL);
    src.push_str("\nlocal function toHex(s)\n  local out = {}\n  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end\n  return table.concat(out)\nend\nlocal k = { a = 0x01020304, b = 0x0506, c = 0 }\nprint(toHex(keyHash_K(k)))\n");
    let dir = std::env::temp_dir().join(format!("idllua_kh_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_of(Path::new(&golden_dir).join("golden_keyhash.bin"))
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
    // (x, ignored, y) that `marshalInto_Inner` (normal encoding) emits.
    let l = emit(NESTED_KEY_PARTIAL_IDL);
    let key_hash_body = l
        .split("function keyHash_Outer(v)")
        .nth(1)
        .expect("key_hash body");
    let end = key_hash_body.find("\nend").unwrap_or(key_hash_body.len());
    let body = &key_hash_body[..end];
    assert!(body.contains("v.i.x"), "{body}");
    assert!(body.contains("v.i.y"), "{body}");
    assert!(!body.contains("v.i.ignored"), "{body}");
    // The nested struct's full `marshalInto_Inner` must NOT be called for the key.
    assert!(!body.contains("marshalInto_Inner"), "{body}");
    // Normal (non-key) encoding of `i` in `marshalInto_Outer` is untouched: it
    // must still call the struct's full marshalInto_Inner.
    assert!(l.contains("marshalInto_Inner(w, v.i)"), "{l}");
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
    let l = emit(NESTED_KEY_SMALL_IDL);
    let key_hash_body = l
        .split("function keyHash_Outer(v)")
        .nth(1)
        .expect("key_hash body");
    let end = key_hash_body.find("\nend").unwrap_or(key_hash_body.len());
    let body = &key_hash_body[..end];
    assert!(body.contains("table.concat(chars)"), "{body}");
    assert!(!body.contains("zd_md5"), "{body}");
}

fn hex_of(p: std::path::PathBuf) -> String {
    std::fs::read(&p)
        .unwrap_or_else(|_| panic!("read {}", p.display()))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Decode roundtrip: `marshal(unmarshal(golden)) == golden` for LE and BE.
/// Proves the generated `unmarshal_{ty}` is the exact inverse of `marshal_{ty}`.
fn run_roundtrip(idl: &str, ty: &str, le_file: &str, be_file: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP roundtrip {ty}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP roundtrip {ty}: `lua5.4` not on PATH");
        return;
    }
    let le = hex_of(Path::new(&golden_dir).join(le_file));
    let be = hex_of(Path::new(&golden_dir).join(be_file));
    let mut src = emit(idl);
    src.push_str(&format!(
        "{LUA_HEX}local function fromHex(h)\n  local t = {{}}\n  for i = 1, #h, 2 do t[#t + 1] = string.char(tonumber(string.sub(h, i, i + 1), 16)) end\n  return table.concat(t)\nend\nprint(toHex(marshal_{ty}(unmarshal_{ty}(fromHex(\"{le}\"), LE), LE)))\nprint(toHex(marshal_{ty}(unmarshal_{ty}(fromHex(\"{be}\"), BE), BE)))\n"
    ));
    let dir = std::env::temp_dir().join(format!("idllua_rt_{ty}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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
    // 5×@key long = 20 bytes > 16 → MD5 branch (XTypes §7.6.8.4), from-scratch MD5.
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP keyhash_md5: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP keyhash_md5: `lua5.4` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_MD5_IDL);
    src.push_str("\nlocal function toHex(s)\n  local out = {}\n  for i = 1, #s do out[i] = string.format(\"%02x\", string.byte(s, i)) end\n  return table.concat(out)\nend\nlocal k = { a = 0x01020304, b = 0x05060708, c = 0x090A0B0C, d = 0x0D0E0F10, e = 0x11121314 }\nprint(toHex(keyHash_KL(k)))\n");
    let dir = std::env::temp_dir().join(format!("idllua_kh_md5_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_of(Path::new(&golden_dir).join("golden_keyhash_md5.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- reserved-keyword escaping (Welle C.2 #14): an IDL struct/enum whose
// identifiers collide with Lua keywords must still emit syntactically valid
// Lua (trailing `_` escape), not a bare keyword used as a Name token.

// NB: IDL itself reserves `local` and `in` (OMG IDL §7.2.6, `local interface`
// / parameter-direction keywords), so those two Lua keywords cannot appear as
// IDL identifiers at all and are excluded from this fixture — `return`, `for`,
// `function`, `end`, `until`, `repeat` are Lua keywords but not IDL ones.
const KEYWORD_IDL: &str = "\
enum end { end_a, function, for };
@final struct until {
    uint32 return;
    end while_field;
};
@final struct repeat {
    until nested;
    sequence<until> items;
};";

#[test]
fn keyword_identifiers_are_escaped_not_bare() {
    let l = emit(KEYWORD_IDL);
    // Declarations use the escaped forms.
    assert!(
        l.contains("local end_ = { end_a = 0, function_ = 1, for_ = 2 }"),
        "{l}"
    );
    assert!(l.contains("function marshalInto_until_(w, v)"), "{l}");
    assert!(l.contains("function marshal_until_(v, endian)"), "{l}");
    assert!(l.contains("function read_until_(r)"), "{l}");
    assert!(l.contains("function marshalInto_repeat_(w, v)"), "{l}");
    assert!(l.contains("function read_repeat_(r)"), "{l}");
    // Field access uses the escaped field name.
    assert!(l.contains("v.return_"), "{l}");
    // Cross-type references (struct member of a keyword-named struct type,
    // and sequence<keyword-named struct>) call the *escaped* function name
    // — the declared function and the call site must agree.
    assert!(l.contains("marshalInto_until_(w, v.nested)"), "{l}");
    assert!(l.contains("v.nested = read_until_(r)"), "{l}");
    assert!(l.contains("marshalInto_until_(sub, e)"), "{l}");
    assert!(l.contains("read_until_(r)"), "{l}");
    // No bare keyword appears as a `.name` field access, a `local NAME =`
    // declaration, or an unescaped cross-type call target.
    assert!(!l.contains("v.return "), "{l}");
    assert!(!l.contains("v.return)"), "{l}");
    assert!(!l.contains("v.return\n"), "{l}");
    assert!(!l.contains("function marshalInto_until("), "{l}");
    assert!(!l.contains("function marshalInto_repeat("), "{l}");
    assert!(!l.contains("marshalInto_until(w"), "{l}");
    assert!(!l.contains("marshalInto_until(sub"), "{l}");
    assert!(!l.contains("local until "), "{l}");
    assert!(!l.contains("local until\n"), "{l}");
}

#[test]
fn keyword_identifiers_parse_with_luac() {
    if Command::new("luac").arg("-v").output().is_err() {
        eprintln!("SKIP keyword luac parse-check: `luac` not on PATH");
        return;
    }
    let l = emit(KEYWORD_IDL);
    let dir = std::env::temp_dir().join(format!("idllua_kw_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("kw.lua");
    std::fs::write(&lf, &l).expect("write");
    let out = Command::new("luac")
        .arg("-p")
        .arg(&lf)
        .output()
        .expect("luac");
    assert!(
        out.status.success(),
        "luac -p (parse-only) rejected keyword-escaped output:\n{}\n--- src ---\n{l}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Section-F Wave-1: bitset / bitmask / fixed / sequence-arbitrary /
// @optional / @verbatim. Always-on source asserts + a gated compile-and-run
// harness (`lua5.4`) whose expected wire hex is spec-derived in-test (no
// GOLDEN_DIR dependency).

/// Emits `idl`, appends `toHex` + `main_body`, runs the generated Lua under
/// `lua5.4`, and returns its trimmed stdout lines. `None` (SKIP) when `lua5.4`
/// is not on PATH — the gated tests still count as passing on such a host.
fn lua_lines(idl: &str, main_body: &str, tag: &str) -> Option<Vec<String>> {
    if Command::new("lua5.4").arg("-v").output().is_err() {
        eprintln!("SKIP {tag}: `lua5.4` not on PATH");
        return None;
    }
    let mut src = emit(idl);
    src.push_str(LUA_HEX);
    src.push_str(main_body);
    let dir = std::env::temp_dir().join(format!("idllua_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let lf = dir.join("main.lua");
    std::fs::write(&lf, &src).expect("write");
    let out = Command::new("lua5.4").arg(&lf).output().expect("lua5.4");
    assert!(
        out.status.success(),
        "lua5.4 failed:\n{}\n--- src ---\n{src}",
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
fn bitset_emits_holder_and_accessors() {
    // 4 + 8 = 12 bits → uint16 backing (XTypes §7.4.7).
    let l = emit(BITSET_IDL);
    assert!(
        l.contains("function Flags_a(v) return (v.storage >> 0) & 15 end"),
        "{l}"
    );
    assert!(
        l.contains("function Flags_b(v) return (v.storage >> 4) & 255 end"),
        "{l}"
    );
    assert!(l.contains("function Flags_set_a(v, x)"), "{l}");
    assert!(
        l.contains("function marshalInto_Flags(w, v) w:putU16(v.storage & 0xffff) end"),
        "{l}"
    );
    assert!(
        l.contains("function read_Flags(r) return { storage = r:getU16() }"),
        "{l}"
    );
}

#[test]
fn bitset_wire_is_backing_int() {
    // storage 0xABCD as a uint16 → LE "cdab", BE "abcd"; round-trips.
    // Accessors: a = 0xABCD & 0xF = 13, b = (0xABCD >> 4) & 0xFF = 188.
    let body = "\nlocal f = { storage = 0xABCD }\n\
print(toHex(marshal_Flags(f, LE)))\n\
print(toHex(marshal_Flags(f, BE)))\n\
print(toHex(marshal_Flags(unmarshal_Flags(marshal_Flags(f, LE), LE), LE)))\n\
print(Flags_a(f))\n\
print(Flags_b(f))\n";
    let Some(l) = lua_lines(BITSET_IDL, body, "bitset") else {
        return;
    };
    assert_eq!(l[0], "cdab", "LE");
    assert_eq!(l[1], "abcd", "BE");
    assert_eq!(l[2], "cdab", "round-trip");
    assert_eq!(l[3], "13", "accessor a");
    assert_eq!(l[4], "188", "accessor b");
}

#[test]
fn bitset_narrow_backing_widths() {
    // ≤8 bits → u8; 17..=32 → u32 (XTypes §7.4.7).
    let small = emit("bitset B8 { bitfield<3> x; bitfield<5> y; };");
    assert!(
        small.contains("function marshalInto_B8(w, v) w:putU8(v.storage & 0xff) end"),
        "{small}"
    );
    let big = emit("bitset B32 { bitfield<20> x; };");
    assert!(
        big.contains("function marshalInto_B32(w, v) w:putU32(v.storage & 0xffffffff) end"),
        "{big}"
    );
}

// ---- bitmask ------------------------------------------------------------

const BITMASK_IDL: &str = "bitmask Perms { PERM_READ, PERM_WRITE, PERM_EXEC };";

#[test]
fn bitmask_emits_holder_and_constants() {
    // Default @bit_bound = 32 → uint32 backing (XTypes §7.3.1.2.1.1).
    let l = emit(BITMASK_IDL);
    assert!(
        l.contains("local Perms = { PERM_READ = 1 << 0, PERM_WRITE = 1 << 1, PERM_EXEC = 1 << 2 }"),
        "{l}"
    );
    assert!(
        l.contains("function marshalInto_Perms(w, v) w:putU32(v.storage & 0xffffffff) end"),
        "{l}"
    );
}

#[test]
fn bitmask_wire_is_backing_uint32() {
    // PERM_READ | PERM_EXEC = 0x05 → LE "05000000", BE "00000005"; round-trips.
    let body = "\nlocal p = { storage = Perms.PERM_READ | Perms.PERM_EXEC }\n\
print(toHex(marshal_Perms(p, LE)))\n\
print(toHex(marshal_Perms(p, BE)))\n\
print(toHex(marshal_Perms(unmarshal_Perms(marshal_Perms(p, BE), BE), BE)))\n";
    let Some(l) = lua_lines(BITMASK_IDL, body, "bitmask") else {
        return;
    };
    assert_eq!(l[0], "05000000", "LE");
    assert_eq!(l[1], "00000005", "BE");
    assert_eq!(l[2], "00000005", "round-trip");
}

#[test]
fn bitmask_bit_bound_narrows_backing() {
    let l = emit("@bit_bound(8) bitmask Small { A, B };");
    assert!(
        l.contains("function marshalInto_Small(w, v) w:putU8(v.storage & 0xff) end"),
        "{l}"
    );
    assert!(
        l.contains("function read_Small(r) return { storage = r:getU8() }"),
        "{l}"
    );
}

// ---- fixed<d,s> ---------------------------------------------------------

const FIXED_IDL: &str = "@final struct HasFixed { fixed<5,2> price; };";

#[test]
fn fixed_emits_bcd_field_and_prelude() {
    let l = emit(FIXED_IDL);
    assert!(
        l.contains("$w:putBytes(v.price)") || l.contains("w:putBytes(v.price)"),
        "{l}"
    );
    assert!(l.contains("function zdFixedEnc(s, P, S)"), "{l}");
    // decode reads (5+2)/2 = 3 raw octets, no length prefix.
    assert!(l.contains("v.price = r:getBytesN(3)"), "{l}");
}

#[test]
fn fixed_wire_is_packed_bcd() {
    // fixed<5,2> 123.45 → BCD "12 34 5c" (odd P, no pad, CORBA §9.3.2.7); the
    // raw BCD is endian-agnostic (no byte-swap, no length prefix); round-trips.
    let body = "\nlocal h = { price = zdFixedEnc(\"123.45\", 5, 2) }\n\
print(toHex(marshal_HasFixed(h, LE)))\n\
print(toHex(marshal_HasFixed(h, BE)))\n\
print(toHex(marshal_HasFixed(unmarshal_HasFixed(marshal_HasFixed(h, LE), LE), LE)))\n";
    let Some(l) = lua_lines(FIXED_IDL, body, "fixed") else {
        return;
    };
    assert_eq!(l[0], "12345c", "LE");
    assert_eq!(l[1], "12345c", "BE");
    assert_eq!(l[2], "12345c", "round-trip");
}

#[test]
fn fixed_even_p_keeps_msd() {
    // fixed<4,0> 1234 → BCD "01 23 4c" (leading pad nibble; even P keeps MSD).
    let body = "\nlocal h = { price = zdFixedEnc(\"1234\", 4, 0) }\n\
print(toHex(marshal_HasFixed(h, LE)))\n";
    let Some(l) = lua_lines(
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
    // Was rejected pre-Wave-1 (idl-lua ~:416). Now: u32 count + per-element,
    // no collection DHEADER.
    let l = emit(SEQARB_IDL);
    assert!(l.contains("w:putU32(#v.xs)"), "{l}");
    assert!(l.contains("for _, zdElem in ipairs(v.xs) do"), "{l}");
}

#[test]
fn sequence_arbitrary_wire_count_plus_elements() {
    // [0x01020304, 0x05060708] → u32 count 2 + two i32 elements, no DHEADER.
    let body = "\nlocal s = { xs = { 0x01020304, 0x05060708 } }\n\
print(toHex(marshal_S(s, LE)))\n\
print(toHex(marshal_S(s, BE)))\n\
print(toHex(marshal_S(unmarshal_S(marshal_S(s, LE), LE), LE)))\n";
    let Some(l) = lua_lines(SEQARB_IDL, body, "seqarb") else {
        return;
    };
    assert_eq!(l[0], "020000000403020108070605", "LE");
    assert_eq!(l[1], "000000020102030405060708", "BE");
    assert_eq!(l[2], "020000000403020108070605", "round-trip");
}

#[test]
fn sequence_of_enum_is_arbitrary_path() {
    // A `sequence<enum>` must now emit (was rejected pre-Wave-1).
    let l = emit("enum E { E0, E1 }; @final struct SE { sequence<E> es; };");
    assert!(l.contains("w:putU32(#v.es)"), "{l}");
    assert!(l.contains("for _, zdElem in ipairs(v.es) do"), "{l}");
}

// ---- @optional ----------------------------------------------------------

const OPT_IDL: &str = "@final struct Opt { uint32 a; @optional uint32 b; };";

#[test]
fn optional_emits_presence_flag() {
    let l = emit(OPT_IDL);
    assert!(l.contains("w:putU8(v.b_present and 1 or 0)"), "{l}");
    assert!(l.contains("if v.b_present then"), "{l}");
    assert!(l.contains("v.b_present = r:getBool()"), "{l}");
}

#[test]
fn optional_final_wire_present_and_absent() {
    // present: u32 a, u8 flag=1, pad(3), u32 b. absent: u32 a, u8 flag=0.
    let body = "\nlocal p = { a = 0x11223344, b_present = true, b = 0xAABBCCDD }\n\
print(toHex(marshal_Opt(p, LE)))\n\
print(toHex(marshal_Opt(p, BE)))\n\
local q = { a = 0x11223344, b_present = false }\n\
print(toHex(marshal_Opt(q, LE)))\n\
print(toHex(marshal_Opt(unmarshal_Opt(marshal_Opt(p, LE), LE), LE)))\n\
print(toHex(marshal_Opt(unmarshal_Opt(marshal_Opt(q, LE), LE), LE)))\n";
    let Some(l) = lua_lines(OPT_IDL, body, "opt") else {
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
    // Appendable body is DHEADER-framed; verify decode∘encode identity for both
    // present and absent, without hand-computing the DHEADER length.
    let idl = "@appendable struct OptA { uint32 a; @optional uint32 b; };";
    let body = "\nlocal p = { a = 0xCAFEBABE, b_present = true, b = 0x01020304 }\n\
local pe = marshal_OptA(p, LE)\n\
print(toHex(pe))\n\
print(toHex(marshal_OptA(unmarshal_OptA(pe, LE), LE)))\n\
local q = { a = 0xCAFEBABE, b_present = false }\n\
local qe = marshal_OptA(q, LE)\n\
print(toHex(qe))\n\
print(toHex(marshal_OptA(unmarshal_OptA(qe, LE), LE)))\n";
    let Some(l) = lua_lines(idl, body, "opta") else {
        return;
    };
    assert_eq!(l[0], l[1], "present round-trip");
    assert_eq!(l[2], l[3], "absent round-trip");
}

// ---- @verbatim ----------------------------------------------------------

#[test]
fn verbatim_placements_inject_text() {
    let l = emit(
        "@verbatim(language=\"lua\", placement=BEGIN_FILE, text=\"-- zd-begin-file\")\n\
         @verbatim(language=\"lua\", placement=BEFORE_DECLARATION, text=\"-- zd-before\")\n\
         @verbatim(language=\"lua\", placement=BEGIN_DECLARATION, text=\"-- zd-begin-decl\")\n\
         @verbatim(language=\"lua\", placement=END_DECLARATION, text=\"-- zd-end-decl\")\n\
         @verbatim(language=\"lua\", placement=AFTER_DECLARATION, text=\"-- zd-after\")\n\
         @verbatim(language=\"lua\", placement=END_FILE, text=\"-- zd-end-file\")\n\
         @final struct V { uint32 a; };",
    );
    for marker in [
        "-- zd-begin-file",
        "-- zd-before",
        "-- zd-begin-decl",
        "-- zd-end-decl",
        "-- zd-after",
        "-- zd-end-file",
    ] {
        assert!(l.contains(marker), "missing {marker}:\n{l}");
    }
    let m = l.find("function marshalInto_V").expect("marshaller");
    // begin-file / before-decl / begin-decl precede the marshaller (in order).
    assert!(
        l.find("-- zd-begin-file").unwrap() < l.find("-- zd-before").unwrap(),
        "{l}"
    );
    assert!(
        l.find("-- zd-before").unwrap() < l.find("-- zd-begin-decl").unwrap(),
        "{l}"
    );
    assert!(l.find("-- zd-begin-decl").unwrap() < m, "{l}");
    // end-decl / after-decl / end-file trail the marshaller (in order).
    assert!(m < l.find("-- zd-end-decl").unwrap(), "{l}");
    assert!(
        l.find("-- zd-end-decl").unwrap() < l.find("-- zd-after").unwrap(),
        "{l}"
    );
    assert!(
        l.find("-- zd-after").unwrap() < l.find("-- zd-end-file").unwrap(),
        "{l}"
    );
}

#[test]
fn verbatim_language_filter_excludes_other_langs() {
    // A non-Lua language tag must NOT leak into the Lua output.
    let l = emit(
        "@verbatim(language=\"java\", placement=BEFORE_DECLARATION, text=\"-- java-only\")\n\
         @final struct V { uint32 a; };",
    );
    assert!(!l.contains("-- java-only"), "{l}");
    // The wildcard `*` still matches Lua.
    let l2 = emit(
        "@verbatim(placement=BEFORE_DECLARATION, text=\"-- wildcard\")\n\
         @final struct V { uint32 a; };",
    );
    assert!(l2.contains("-- wildcard"), "{l2}");
}

#[test]
fn verbatim_output_still_runs() {
    let idl = "@verbatim(language=\"lua\", placement=BEGIN_FILE, text=\"-- zd file header\")\n\
         @verbatim(language=\"lua\", placement=BEGIN_DECLARATION, text=\"-- zd inside struct\")\n\
         @final struct V { uint32 a; };";
    let body = "\nlocal v = { a = 0x2A }\nprint(toHex(marshal_V(v, LE)))\n";
    let Some(l) = lua_lines(idl, body, "verbatim") else {
        return;
    };
    assert_eq!(l[0], "2a000000");
}
