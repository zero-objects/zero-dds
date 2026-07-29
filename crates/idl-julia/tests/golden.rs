// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Julia backend: string smoke tests (always) + a byte-identity test that
//! compiles+runs the generated Julia and compares to the Rust goldens (gated on
//! `julia` on PATH and `GOLDEN_DIR` pointing at golden_{le,be}.bin).

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
use zerodds_idl_julia::{JuliaGenOptions, generate_julia_module};

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
    generate_julia_module(&ast, &JuliaGenOptions::default()).expect("gen")
}

/// swarm59 #21b: `module X { struct Y { ... }; }` used to be silently
/// dropped (no `Definition::Module` arm at all) — the struct must now emit.
#[test]
fn module_wrapped_struct_is_emitted_not_dropped() {
    let j = emit("module Telemetry { @final struct Reading { long value; }; };");
    assert!(j.contains("struct Reading"), "{j}");
    assert!(j.contains("    value::Int32"), "{j}");
}

/// A reopened module (`module M {} ... module M {}`) must not lose either
/// half's content once the AST builder merges the two occurrences.
#[test]
fn reopened_module_emits_both_structs() {
    let j = emit(
        "module M { @final struct A { long x; }; }; \
         module M { @final struct B { long y; }; };",
    );
    assert!(j.contains("struct A"), "{j}");
    assert!(j.contains("struct B"), "{j}");
}

#[test]
fn final_struct_emits_struct_and_marshal() {
    let j = emit(GOLDEN_IDL);
    assert!(j.contains("struct Golden"), "{j}");
    assert!(j.contains("    id::UInt32"), "{j}");
    assert!(j.contains("    kind::UInt16"), "{j}");
    assert!(j.contains("    flags::UInt8"), "{j}");
    assert!(j.contains("    value::Float32"), "{j}");
    assert!(j.contains("    stamp::UInt64"), "{j}");
    assert!(j.contains("    label::String"), "{j}");
    assert!(j.contains("    raw::Vector{UInt8}"), "{j}");
    assert!(
        j.contains("function marshal_xcdr(v::Golden, endian::Endian)::Vector{UInt8}"),
        "{j}"
    );
    assert!(j.contains("put_u32!(w, v.id)"), "{j}");
    assert!(j.contains("put_f32!(w, v.value)"), "{j}");
    assert!(j.contains("put_string!(w, v.label)"), "{j}");
    assert!(j.contains("put_seq_u8!(w, v.raw)"), "{j}");
    assert!(!j.contains("bb = bytes(body)"), "{j}");
}

#[test]
fn appendable_struct_frames_a_dheader() {
    let j = emit("@appendable struct S { uint32 a; };");
    assert!(j.contains("bb = bytes(body)"), "{j}");
    assert!(j.contains("put_u32!(w, length(bb))"), "{j}");
    assert!(j.contains("put_bytes!(w, bb)"), "{j}");
}

const ENUM_IDL: &str = "\
enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };
@final struct S { Mode kind; uint32 tail; };";

#[test]
fn enum_emits_at_enum_and_member_marshals() {
    let j = emit(ENUM_IDL);
    assert!(
        j.contains("@enum Mode MODE_IDLE=0 MODE_ACTIVE=1 MODE_FAULT=2"),
        "{j}"
    );
    assert!(j.contains("    kind::Mode"), "{j}");
    // An enum member is a 32-bit signed integer on the wire (XTypes §7.4.5.1).
    assert!(
        j.contains("put_u32!(w, reinterpret(UInt32, Int32(Integer(v.kind))))"),
        "{j}"
    );
}

#[test]
fn enum_member_is_byte_identical_i32() {
    // Gated: needs julia. S{ kind: MODE_FAULT(=2), tail: 0xDEADBEEF }
    // → i32 LE 02000000 + u32 LE efbeadde.
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP enum byte test: `julia` not on PATH");
        return;
    }
    let mut src = emit(ENUM_IDL);
    src.push_str(
        r#"
function main()
    s = S(MODE_FAULT, 0xDEADBEEF)
    println(bytes2hex(marshal_xcdr(s, LE)))
end
main()
"#,
    );
    let dir = std::env::temp_dir().join(format!("idljulia_enum_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
    let j = emit(NESTED_IDL);
    assert!(
        j.contains("function marshal_into!(v::Inner, w::Writer)"),
        "{j}"
    );
    assert!(j.contains("    one::Inner"), "{j}");
    assert!(j.contains("    many::Vector{Inner}"), "{j}");
    assert!(j.contains("marshal_into!(v.one, body)"), "{j}");
    assert!(j.contains("marshal_into!(e, sub)"), "{j}");
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
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP nested byte: `julia` not on PATH");
        return;
    }
    let mut src = emit(NESTED_IDL);
    src.push_str(
        r#"
function main()
    o = Outer(0xCAFEBABE, Inner(0x1111, 0x22223333), Inner[Inner(0xAAAA, 0xBBBBCCCC), Inner(0xDDDD, 0xEEEEFFFF)], "nested")
    println(bytes2hex(marshal_xcdr(o, LE)))
    println(bytes2hex(marshal_xcdr(o, BE)))
end
main()
"#,
    );
    let dir = std::env::temp_dir().join(format!("idljulia_nested_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP byte_identity: `julia` not on PATH");
        return;
    }

    let mut src = emit(GOLDEN_IDL);
    src.push_str(
        r#"
function main()
    g = Golden(0xA1B2C3D4, 0x1234, 0x5A, 3.5f0, 0x0102030405060708, "bay-12", UInt8[0xDE, 0xAD, 0xBE, 0xEF])
    println(bytes2hex(marshal_xcdr(g, LE)))
    println(bytes2hex(marshal_xcdr(g, BE)))
end
main()
"#,
    );

    let dir = std::env::temp_dir().join(format!("idljulia_golden_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");

    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
    let j = emit(TYPEDEF_IDL);
    assert!(j.contains("    name::String"), "{j}");
    assert!(j.contains("    data::Vector{UInt8}"), "{j}");
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
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP typedef byte: `julia` not on PATH");
        return;
    }
    let mut src = emit(TYPEDEF_IDL);
    src.push_str(
        r#"
function main()
    r = Rec(0xCAFEBABE, "typedef", UInt8[1, 2, 3])
    println(bytes2hex(marshal_xcdr(r, LE)))
    println(bytes2hex(marshal_xcdr(r, BE)))
end
main()
"#,
    );
    let dir = std::env::temp_dir().join(format!("idljulia_typedef_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
    let j = emit(ARRAY_IDL);
    assert!(j.contains("xs::Vector{Int32}"), "{j}");
    assert!(j.contains("m::Vector{Vector{Int16}}"), "{j}");
    assert!(j.contains("bs::Vector{UInt8}"), "{j}");
    assert!(j.contains("for zdi0 in 1:3"), "{j}");
    assert!(j.contains("for zdi1 in 1:2"), "{j}");
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
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP array byte: `julia` not on PATH");
        return;
    }
    let mut src = emit(ARRAY_IDL);
    src.push_str(
        r#"
function main()
    a = Arr(Int32[0x11111111, 0x22222222, 0x33333333],
            [Int16[0x0102, 0x0304], Int16[0x0506, 0x0708]],
            UInt8[0xAA, 0xBB, 0xCC, 0xDD])
    println(bytes2hex(marshal_xcdr(a, LE)))
    println(bytes2hex(marshal_xcdr(a, BE)))
end
main()
"#,
    );
    let dir = std::env::temp_dir().join(format!("idljulia_array_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
    let j = emit(UNION_IDL);
    assert!(j.contains("disc::Int32"), "{j}");
    assert!(j.contains("if v.disc == 1"), "{j}");
    assert!(j.contains("elseif v.disc == 2"), "{j}");
    assert!(j.contains("else"), "{j}");
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
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP union byte: `julia` not on PATH");
        return;
    }
    let mut src = emit(UNION_IDL);
    src.push_str(
        r#"
function main()
    u = U(2, 0, 0x1234, 0)
    println(bytes2hex(marshal_xcdr(u, LE)))
    println(bytes2hex(marshal_xcdr(u, BE)))
end
main()
"#,
    );
    let dir = std::env::temp_dir().join(format!("idljulia_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
    let j = emit(MAP_IDL);
    assert!(j.contains("m::Dict{Int32, UInt32}"), "{j}");
    assert!(j.contains("sort(collect(keys(v.m)))"), "{j}");
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
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP map byte: `julia` not on PATH");
        return;
    }
    let mut src = emit(MAP_IDL);
    src.push_str(
        r#"
function main()
    h = HasMap(Dict{Int32, UInt32}(1 => 0x11111111, 2 => 0x22222222))
    println(bytes2hex(marshal_xcdr(h, LE)))
    println(bytes2hex(marshal_xcdr(h, BE)))
end
main()
"#,
    );
    let dir = std::env::temp_dir().join(format!("idljulia_map_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
    let j = emit(MUTABLE_IDL);
    assert!(j.contains("put_u32!(body, 0x4000000a)"), "{j}");
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
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP mutable byte: `julia` not on PATH");
        return;
    }
    let mut src = emit(MUTABLE_IDL);
    src.push_str(
        r#"
function main()
    m = M(0xDEADBEEF, "mut", 0x0777)
    println(bytes2hex(marshal_xcdr(m, LE)))
    println(bytes2hex(marshal_xcdr(m, BE)))
end
main()
"#,
    );
    let dir = std::env::temp_dir().join(format!("idljulia_mutable_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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

fn run_jl(idl: &str, main_body: &str, stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP {stem}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP {stem}: `julia` not on PATH");
        return;
    }
    let mut src = emit(idl);
    src.push_str(main_body);
    let dir = std::env::temp_dir().join(format!("idljulia_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
    run_jl(
        WIDE_IDL,
        "\nfunction main()\n    w = W(UInt32(0x03A9), \"w\u{03c0}\")\n    println(bytes2hex(marshal_xcdr(w, LE)))\n    println(bytes2hex(marshal_xcdr(w, BE)))\nend\nmain()\n",
        "wide",
    );
}

#[test]
fn longdouble_is_byte_identical_vs_rust_golden() {
    run_jl(
        LD_IDL,
        "\nfunction main()\n    l = L(1.1)\n    println(bytes2hex(marshal_xcdr(l, LE)))\n    println(bytes2hex(marshal_xcdr(l, BE)))\nend\nmain()\n",
        "longdouble",
    );
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
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash: `julia` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_IDL);
    src.push_str("\nk = K(0x01020304, 0x0506, 0)\nprintln(bytes2hex(key_hash(k)))\n");
    let dir = std::env::temp_dir().join(format!("idljulia_kh_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
    // (x, ignored, y) that `marshal_into!` (normal encoding) emits for Inner.
    let j = emit(NESTED_KEY_PARTIAL_IDL);
    let key_hash_body = j
        .split("function key_hash(v::Outer)::Vector{UInt8}")
        .nth(1)
        .expect("key_hash body");
    let end = key_hash_body.find("\nend").unwrap_or(key_hash_body.len());
    let body = &key_hash_body[..end];
    assert!(body.contains("v.i.x"), "{body}");
    assert!(body.contains("v.i.y"), "{body}");
    assert!(!body.contains("v.i.ignored"), "{body}");
    // The nested struct's full `marshal_into!` must NOT be called for the key.
    assert!(!body.contains("marshal_into!(v.i,"), "{body}");
    // Normal (non-key) encoding of `i` in `marshal_into!` is untouched: it
    // must still call the struct's full marshal_into!.
    assert!(j.contains("marshal_into!(v.i, w)"), "{j}");
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
    let j = emit(NESTED_KEY_SMALL_IDL);
    let key_hash_body = j
        .split("function key_hash(v::Outer)::Vector{UInt8}")
        .nth(1)
        .expect("key_hash body");
    let end = key_hash_body.find("\nend").unwrap_or(key_hash_body.len());
    let body = &key_hash_body[..end];
    assert!(body.contains("outk = zeros(UInt8, 16)"), "{body}");
    assert!(!body.contains("zd_md5"), "{body}");
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
    // single call against the WHOLE ARRAY value instead of iterating its 4
    // elements. Fixed: an array-declarator `@key` field now reuses `f.put`
    // unchanged (the same indexed `for` loop the general, non-key encoder
    // uses), mirroring `idl-lua`'s `key_type: Option<..>` guard (`None` for
    // `Declarator::Array`).
    let j = emit(ARRAY_KEY_IDL);
    let key_hash_body = j
        .split("function key_hash(v::S)::Vector{UInt8}")
        .nth(1)
        .expect("key_hash body");
    let end = key_hash_body.find("\nend").unwrap_or(key_hash_body.len());
    let body = &key_hash_body[..end];
    assert!(
        body.contains("for zdi0 in 1:4"),
        "array @key field must iterate its elements (same shape as the general encoder):\n{body}"
    );
    assert!(
        body.contains("v.id[zdi0]"),
        "array @key field must index each element, not read the whole array:\n{body}"
    );
    assert!(
        !body.contains(", v.id)") && !body.contains("(v.id)"),
        "array @key field must NOT be scalar-encoded against the whole array value:\n{body}"
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
/// Proves the generated `unmarshal_xcdr_{ty}` is the exact inverse of `marshal_xcdr`.
fn run_roundtrip(idl: &str, ty: &str, le_file: &str, be_file: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP roundtrip {ty}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP roundtrip {ty}: `julia` not on PATH");
        return;
    }
    let le = hex_of(Path::new(&golden_dir).join(le_file));
    let be = hex_of(Path::new(&golden_dir).join(be_file));
    let mut src = emit(idl);
    src.push_str(&format!(
        r#"
function main()
    println(bytes2hex(marshal_xcdr(unmarshal_xcdr_{ty}(hex2bytes("{le}"), LE), LE)))
    println(bytes2hex(marshal_xcdr(unmarshal_xcdr_{ty}(hex2bytes("{be}"), BE), BE)))
end
main()
"#
    ));
    let dir = std::env::temp_dir().join(format!("idljulia_rt_{ty}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash_md5: `julia` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_MD5_IDL);
    src.push_str("\nk = KL(0x01020304, 0x05060708, 0x090A0B0C, 0x0D0E0F10, 0x11121314)\nprintln(bytes2hex(key_hash(k)))\n");
    let dir = std::env::temp_dir().join(format!("idljulia_kh_md5_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
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
enum Kind { for, try, catch };
@final struct Golden2 {
    uint32 for;
    uint32 end;
    Kind function;
};";

/// A struct/field/enum-value set chosen to collide with Julia reserved
/// keywords (`for`, `end`, `function`, `try`, `catch`) — was silent invalid
/// code before the `escape_julia_ident` wiring (Welle C.2 #14).
#[test]
fn keyword_colliding_identifiers_are_escaped() {
    let j = emit(KEYWORD_COLLISION_IDL);
    assert!(j.contains("@enum Kind for_=0 try_=1 catch_=2"), "{j}");
    assert!(j.contains("    for_::UInt32"), "{j}");
    assert!(j.contains("    end_::UInt32"), "{j}");
    assert!(j.contains("    function_::Kind"), "{j}");
    assert!(j.contains("v.for_"), "{j}");
    assert!(j.contains("v.end_"), "{j}");
    assert!(j.contains("v.function_"), "{j}");
    // No raw reserved-word tokens leaked into declaration position.
    assert!(!j.contains("    for::UInt32"), "{j}");
    assert!(!j.contains("    end::UInt32"), "{j}");
    assert!(!j.contains("    function::Kind"), "{j}");
}

#[test]
fn keyword_colliding_names_compile_and_run_with_julia() {
    if Command::new("julia").arg("--version").output().is_err() {
        eprintln!("SKIP keyword_colliding_names_compile_and_run_with_julia: `julia` not on PATH");
        return;
    }
    let mut src = emit(KEYWORD_COLLISION_IDL);
    src.push_str("\nv = Golden2(1, 2, for_)\nb = marshal_xcdr(v, LE)\nv2 = unmarshal_xcdr_Golden2(b, LE)\n@assert v2.for_ == 1\n@assert v2.end_ == 2\nprintln(\"ok\")\n");
    let dir = std::env::temp_dir().join(format!("idljulia_kw_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jf = dir.join("main.jl");
    std::fs::write(&jf, &src).expect("write");
    let out = Command::new("julia").arg(&jf).output().expect("julia");
    assert!(
        out.status.success(),
        "julia failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}
