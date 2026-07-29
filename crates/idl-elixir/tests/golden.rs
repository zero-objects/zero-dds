// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Elixir backend: string smoke tests (always) + a byte-identity test that
//! compiles+runs the generated Elixir and compares to the Rust goldens (gated
//! on `elixir` on PATH and `GOLDEN_DIR` pointing at golden_{le,be}.bin).

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
use zerodds_idl_elixir::{ElixirGenOptions, generate_elixir_module};

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
    generate_elixir_module(&ast, &ElixirGenOptions::default()).expect("gen")
}

/// swarm59 #21b: `module X { struct Y { ... }; }` used to be silently
/// dropped (no `Definition::Module` arm at all) — the struct must now emit.
#[test]
fn module_wrapped_struct_is_emitted_not_dropped() {
    let e = emit("module Telemetry { @final struct Reading { long value; }; };");
    assert!(e.contains("defmodule Zdgen.Reading do"), "{e}");
    assert!(e.contains("defstruct [:value]"), "{e}");
}

/// A reopened module (`module M {} ... module M {}`) must not lose either
/// half's content once the AST builder merges the two occurrences.
#[test]
fn reopened_module_emits_both_structs() {
    let e = emit(
        "module M { @final struct A { long x; }; }; \
         module M { @final struct B { long y; }; };",
    );
    assert!(e.contains("defmodule Zdgen.A do"), "{e}");
    assert!(e.contains("defmodule Zdgen.B do"), "{e}");
}

#[test]
fn final_struct_emits_module_and_marshal() {
    let e = emit(GOLDEN_IDL);
    assert!(e.contains("defmodule Zdgen.Wire do"), "{e}");
    assert!(e.contains("defmodule Zdgen.Golden do"), "{e}");
    assert!(
        e.contains("defstruct [:id, :kind, :flags, :value, :stamp, :label, :raw]"),
        "{e}"
    );
    assert!(
        e.contains("def marshal_xcdr(%__MODULE__{} = v, endian) do"),
        "{e}"
    );
    assert!(e.contains("|> Zdgen.Wire.put_u32(v.id)"), "{e}");
    assert!(e.contains("|> Zdgen.Wire.put_f32(v.value)"), "{e}");
    assert!(e.contains("|> Zdgen.Wire.put_string(v.label)"), "{e}");
    assert!(e.contains("|> Zdgen.Wire.put_seq_u8(v.raw)"), "{e}");
    assert!(!e.contains("byte_size(body)"), "{e}");
}

#[test]
fn appendable_struct_frames_a_dheader() {
    let e = emit("@appendable struct S { uint32 a; };");
    assert!(e.contains("|> Zdgen.Wire.put_u32(byte_size(body))"), "{e}");
    assert!(e.contains("|> Zdgen.Wire.put_bytes(body)"), "{e}");
}

const ENUM_IDL: &str = "\
enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };
@final struct S { Mode kind; uint32 tail; };";

#[test]
fn enum_emits_value_module_and_member_marshals() {
    let e = emit(ENUM_IDL);
    assert!(e.contains("defmodule Zdgen.Mode do"), "{e}");
    assert!(e.contains("def mode_idle, do: 0"), "{e}");
    assert!(e.contains("def mode_fault, do: 2"), "{e}");
    // An enum member is a 32-bit signed integer on the wire (XTypes §7.4.5.1).
    assert!(e.contains("|> Zdgen.Wire.put_u32(v.kind)"), "{e}");
}

#[test]
fn enum_member_is_byte_identical_i32() {
    // Gated: needs elixir. S{ kind: MODE_FAULT(=2), tail: 0xDEADBEEF }
    // → i32 LE 02000000 + u32 LE efbeadde.
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP enum byte test: `elixir` not on PATH");
        return;
    }
    let mut src = emit(ENUM_IDL);
    src.push_str(
        r#"
s = struct(Zdgen.S, kind: Zdgen.Mode.mode_fault(), tail: 0xDEADBEEF)
IO.puts(Base.encode16(Zdgen.S.marshal_xcdr(s, :little), case: :lower))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlelixir_enum_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
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
    let e = emit(NESTED_IDL);
    assert!(
        e.contains("def marshal_into(w, %__MODULE__{} = v) do"),
        "{e}"
    );
    assert!(e.contains("|> Zdgen.Inner.marshal_into(v.one)"), "{e}");
    assert!(e.contains("Zdgen.Inner.marshal_into(acc, e)"), "{e}");
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
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP nested byte: `elixir` not on PATH");
        return;
    }
    let mut src = emit(NESTED_IDL);
    src.push_str(
        r#"
o = struct(Zdgen.Outer,
  id: 0xCAFEBABE,
  one: struct(Zdgen.Inner, a: 0x1111, b: 0x22223333),
  many: [struct(Zdgen.Inner, a: 0xAAAA, b: 0xBBBBCCCC), struct(Zdgen.Inner, a: 0xDDDD, b: 0xEEEEFFFF)],
  label: "nested"
)
IO.puts(Base.encode16(Zdgen.Outer.marshal_xcdr(o, :little), case: :lower))
IO.puts(Base.encode16(Zdgen.Outer.marshal_xcdr(o, :big), case: :lower))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlelixir_nested_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
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
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP byte_identity: `elixir` not on PATH");
        return;
    }

    let mut src = emit(GOLDEN_IDL);
    src.push_str(
        r#"
raw = <<0xDE, 0xAD, 0xBE, 0xEF>>
g = struct(Zdgen.Golden,
  id: 0xA1B2C3D4,
  kind: 0x1234,
  flags: 0x5A,
  value: 3.5,
  stamp: 0x0102030405060708,
  label: "bay-12",
  raw: raw
)
IO.puts(Base.encode16(Zdgen.Golden.marshal_xcdr(g, :little), case: :lower))
IO.puts(Base.encode16(Zdgen.Golden.marshal_xcdr(g, :big), case: :lower))
"#,
    );

    let dir = std::env::temp_dir().join(format!("idlelixir_golden_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");

    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
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
    let e = emit(TYPEDEF_IDL);
    assert!(e.contains("defstruct [:id, :name, :data]"), "{e}");
    assert!(e.contains("|> Zdgen.Wire.put_seq_u8(v.data)"), "{e}");
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
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP typedef byte: `elixir` not on PATH");
        return;
    }
    let mut src = emit(TYPEDEF_IDL);
    src.push_str(
        r#"
r = struct(Zdgen.Rec, id: 0xCAFEBABE, name: "typedef", data: <<1, 2, 3>>)
IO.puts(Base.encode16(Zdgen.Rec.marshal_xcdr(r, :little), case: :lower))
IO.puts(Base.encode16(Zdgen.Rec.marshal_xcdr(r, :big), case: :lower))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlelixir_typedef_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
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
fn array_emits_reduce_loops() {
    let e = emit(ARRAY_IDL);
    assert!(e.contains("Enum.reduce(v.xs, w,"), "{e}");
    assert!(e.contains("Enum.reduce(v.m, w,"), "{e}");
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
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP array byte: `elixir` not on PATH");
        return;
    }
    let mut src = emit(ARRAY_IDL);
    src.push_str(
        r#"
a = struct(Zdgen.Arr,
  xs: [0x11111111, 0x22222222, 0x33333333],
  m: [[0x0102, 0x0304], [0x0506, 0x0708]],
  bs: [0xAA, 0xBB, 0xCC, 0xDD]
)
IO.puts(Base.encode16(Zdgen.Arr.marshal_xcdr(a, :little), case: :lower))
IO.puts(Base.encode16(Zdgen.Arr.marshal_xcdr(a, :big), case: :lower))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlelixir_array_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
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
    let e = emit(UNION_IDL);
    assert!(e.contains("defstruct [:disc, :a, :b, :c]"), "{e}");
    assert!(e.contains("case v.disc do"), "{e}");
    assert!(e.contains("1 -> w |>"), "{e}");
    assert!(e.contains("2 -> w |>"), "{e}");
    assert!(e.contains("_ -> w |>"), "{e}");
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
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP union byte: `elixir` not on PATH");
        return;
    }
    let mut src = emit(UNION_IDL);
    src.push_str(
        r#"
u = struct(Zdgen.U, disc: 2, a: 0, b: 0x1234, c: 0)
IO.puts(Base.encode16(Zdgen.U.marshal_xcdr(u, :little), case: :lower))
IO.puts(Base.encode16(Zdgen.U.marshal_xcdr(u, :big), case: :lower))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlelixir_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
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
    let e = emit(MAP_IDL);
    assert!(e.contains("Enum.sort(Map.to_list(v.m))"), "{e}");
    assert!(e.contains("map_size(v.m)"), "{e}");
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
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP map byte: `elixir` not on PATH");
        return;
    }
    let mut src = emit(MAP_IDL);
    src.push_str(
        r#"
h = struct(Zdgen.HasMap, m: %{1 => 0x11111111, 2 => 0x22222222})
IO.puts(Base.encode16(Zdgen.HasMap.marshal_xcdr(h, :little), case: :lower))
IO.puts(Base.encode16(Zdgen.HasMap.marshal_xcdr(h, :big), case: :lower))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlelixir_map_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
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
    let e = emit(MUTABLE_IDL);
    assert!(e.contains("Zdgen.Wire.put_u32(0x4000000a)"), "{e}");
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
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP mutable byte: `elixir` not on PATH");
        return;
    }
    let mut src = emit(MUTABLE_IDL);
    src.push_str(
        r#"
m = struct(Zdgen.M, x: 0xDEADBEEF, s: "mut", k: 0x0777)
IO.puts(Base.encode16(Zdgen.M.marshal_xcdr(m, :little), case: :lower))
IO.puts(Base.encode16(Zdgen.M.marshal_xcdr(m, :big), case: :lower))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlelixir_mutable_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
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

fn run_ex(idl: &str, ctor: &str, stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP {stem}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP {stem}: `elixir` not on PATH");
        return;
    }
    let mut src = emit(idl);
    src.push_str(&format!("\nv = {ctor}\nIO.puts(Base.encode16(Zdgen.{mod}.marshal_xcdr(v, :little), case: :lower))\nIO.puts(Base.encode16(Zdgen.{mod}.marshal_xcdr(v, :big), case: :lower))\n", mod = if stem == "wide" { "W" } else { "L" }));
    let dir = std::env::temp_dir().join(format!("idlelixir_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
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
    run_ex(
        WIDE_IDL,
        "struct(Zdgen.W, c: 0x03A9, s: \"w\u{03c0}\")",
        "wide",
    );
}
#[test]
fn longdouble_is_byte_identical_vs_rust_golden() {
    run_ex(LD_IDL, "struct(Zdgen.L, d: 1.1)", "longdouble");
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
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash: `elixir` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_IDL);
    src.push_str("\nk = struct(Zdgen.K, a: 0x01020304, b: 0x0506, c: 0)\nIO.puts(Base.encode16(Zdgen.K.key_hash(k), case: :lower))\n");
    let dir = std::env::temp_dir().join(format!("idlelixir_kh_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
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
    // (x, ignored, y) that `Inner.marshal_into/2` (normal encoding) emits.
    let e = emit(NESTED_KEY_PARTIAL_IDL);
    assert!(e.contains("def key_hash(%__MODULE__{} = v) do"), "{e}");
    let outer_module = e
        .split("defmodule Zdgen.Outer do")
        .nth(1)
        .expect("Outer module");
    let key_hash_body = outer_module
        .split("def key_hash(%__MODULE__{} = v) do")
        .nth(1)
        .expect("key_hash body");
    assert!(key_hash_body.contains("v.i.x"), "{key_hash_body}");
    assert!(key_hash_body.contains("v.i.y"), "{key_hash_body}");
    assert!(!key_hash_body.contains("v.i.ignored"), "{key_hash_body}");
    // The nested struct's full `marshal_into` must NOT be called for the key.
    assert!(
        !key_hash_body.contains("Zdgen.Inner.marshal_into"),
        "{key_hash_body}"
    );
    // Normal (non-key) encoding of `i` in `marshal_into` is untouched: it must
    // still call the struct's full marshal_into.
    assert!(e.contains("|> Zdgen.Inner.marshal_into(v.i)"), "{e}");
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
    let e = emit(NESTED_KEY_SMALL_IDL);
    let outer_module = e
        .split("defmodule Zdgen.Outer do")
        .nth(1)
        .expect("Outer module");
    let key_hash_body = outer_module
        .split("def key_hash(%__MODULE__{} = v) do")
        .nth(1)
        .expect("key_hash body");
    let end = key_hash_body.find("\n  end").unwrap_or(key_hash_body.len());
    let body = &key_hash_body[..end];
    assert!(
        body.contains("binary_part(b <> <<0::128>>, 0, 16)"),
        "expected zero-pad branch, got:\n{body}"
    );
    assert!(!body.contains(":erlang.md5"), "expected no MD5 branch:\n{body}");
}

const ARRAY_KEY_IDL: &str = "\
@final struct S { @key octet id[4]; long tail; };";

#[test]
fn array_key_field_iterates_elements_not_scalar_encoded() {
    // REGRESSION (over/mis-inclusion, same bug class as #20): `FieldGen.
    // type_spec` used to be set to `resolved.clone()` identically for
    // `Declarator::Simple` AND `Declarator::Array` — for Array, `resolved`
    // is the ELEMENT type (`octet`, not "array of 4 octets"). `key_hash`
    // then called `map_key_type(&f.type_spec, "v.id", ..)` for the `@key`
    // field, which — believing it was handed a scalar octet — emitted a
    // single `Pkg.Wire.put_u8(v.id)` against the WHOLE LIST value instead of
    // iterating its 4 elements. Fixed: an array-declarator `@key` field now
    // reuses `f.seg` unchanged (the same `Enum.reduce` element loop the
    // general, non-key encoder uses), mirroring `idl-lua`'s `key_type:
    // Option<..>` guard (`None` for `Declarator::Array`).
    let e = emit(ARRAY_KEY_IDL);
    let s_module = e.split("defmodule Zdgen.S do").nth(1).expect("S module");
    let key_hash_body = s_module
        .split("def key_hash(%__MODULE__{} = v) do")
        .nth(1)
        .expect("key_hash body");
    let end = key_hash_body.find("\n  end").unwrap_or(key_hash_body.len());
    let body = &key_hash_body[..end];
    assert!(
        body.contains("Enum.reduce(v.id, "),
        "array @key field must iterate its elements (same shape as the general encoder):\n{body}"
    );
    assert!(
        !body.contains("put_u8(v.id)"),
        "array @key field must NOT be scalar-encoded against the whole list value:\n{body}"
    );
    // `tail` (not `@key`) must not appear in the KeyHash body at all.
    assert!(!body.contains("v.tail"), "{body}");
}

fn hex_of(p: std::path::PathBuf) -> String {
    std::fs::read(&p)
        .unwrap_or_else(|_| panic!("read {}", p.display()))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Decode roundtrip: `marshal(unmarshal(golden)) == golden` for LE and BE.
/// Proves the generated `unmarshal` is the exact inverse of `marshal_xcdr`.
fn run_roundtrip(idl: &str, module: &str, le_file: &str, be_file: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP roundtrip {module}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP roundtrip {module}: `elixir` not on PATH");
        return;
    }
    let le = hex_of(Path::new(&golden_dir).join(le_file));
    let be = hex_of(Path::new(&golden_dir).join(be_file));
    let mut src = emit(idl);
    src.push_str(&format!(
        "\ngle = Base.decode16!(\"{le}\", case: :lower)\ngbe = Base.decode16!(\"{be}\", case: :lower)\nIO.puts(Base.encode16(Zdgen.{module}.marshal_xcdr(Zdgen.{module}.unmarshal(gle, :little), :little), case: :lower))\nIO.puts(Base.encode16(Zdgen.{module}.marshal_xcdr(Zdgen.{module}.unmarshal(gbe, :big), :big), case: :lower))\n"
    ));
    let dir = std::env::temp_dir().join(format!("idlelixir_rt_{module}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        le,
        "LE roundtrip {module}"
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        be,
        "BE roundtrip {module}"
    );
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
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash_md5: `elixir` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_MD5_IDL);
    src.push_str("\nk = struct(Zdgen.KL, a: 0x01020304, b: 0x05060708, c: 0x090A0B0C, d: 0x0D0E0F10, e: 0x11121314)\nIO.puts(Base.encode16(Zdgen.KL.key_hash(k), case: :lower))\n");
    let dir = std::env::temp_dir().join(format!("idlelixir_kh_md5_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_of(Path::new(&golden_dir).join("golden_keyhash_md5.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

const KEYWORD_COLLISION_IDL: &str = "\
enum Signal { AFTER, CATCH, CONTINUE };
@final struct Do {
    uint32 do;
    uint32 end;
    uint32 fn;
    Signal not;
};";

/// A struct/field/enum-value set chosen to collide with Elixir's true
/// reserved words (`do`, `end`, `fn`, `not`, and the lowercased enumerator
/// `after`) once lowercased into Elixir syntax — was silent invalid code
/// before the `escape_elixir_ident` wiring (Welle C.2 #14).
#[test]
fn keyword_colliding_identifiers_are_escaped() {
    let e = emit(KEYWORD_COLLISION_IDL);
    // Field atoms/dot-access/bindings get a trailing underscore.
    assert!(e.contains(":do_"), "{e}");
    assert!(e.contains(":end_"), "{e}");
    assert!(e.contains(":fn_"), "{e}");
    assert!(e.contains(":not_"), "{e}");
    assert!(e.contains("v.do_"), "{e}");
    assert!(e.contains("v.end_"), "{e}");
    assert!(e.contains("v.fn_"), "{e}");
    assert!(e.contains("v.not_"), "{e}");
    // Enumerator `AFTER` lowercases to the reserved word `after`.
    assert!(e.contains("def after_, do: 0"), "{e}");
    // No raw reserved-word tokens leaked into atom/binding position.
    assert!(!e.contains(":do,"), "{e}");
    assert!(!e.contains(":end,"), "{e}");
    assert!(!e.contains(":fn]"), "{e}");
    assert!(!e.contains("def after,"), "{e}");
}

#[test]
fn keyword_colliding_module_name_compiles_with_elixir() {
    if Command::new("elixir").arg("--version").output().is_err() {
        eprintln!("SKIP keyword_colliding_module_name_compiles_with_elixir: `elixir` not on PATH");
        return;
    }
    let mut src = emit(KEYWORD_COLLISION_IDL);
    src.push_str("\nIO.puts(\"ok\")\n");
    let dir = std::env::temp_dir().join(format!("idlelixir_kw_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let ef = dir.join("main.exs");
    std::fs::write(&ef, &src).expect("write");
    let out = Command::new("elixir").arg(&ef).output().expect("elixir");
    assert!(
        out.status.success(),
        "elixir failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
