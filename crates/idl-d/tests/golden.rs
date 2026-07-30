// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! D backend: string smoke tests (always) + a byte-identity test that
//! compiles+runs the generated D and compares to the Rust goldens (gated on
//! `gdc` on PATH and `GOLDEN_DIR` pointing at golden_{le,be}.bin).

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
use zerodds_idl_d::{DGenOptions, generate_d_module};

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
    generate_d_module(&ast, &DGenOptions::default()).expect("gen")
}

/// swarm59 #21b: `module X { struct Y { ... }; }` used to be silently
/// dropped (no `Definition::Module` arm at all) — the struct must now emit.
#[test]
fn module_wrapped_struct_is_emitted_not_dropped() {
    let d = emit("module Telemetry { @final struct Reading { long value; }; };");
    // #21: a module-wrapped type is emitted with its module-qualified name.
    assert!(d.contains("struct Telemetry_Reading {"), "{d}");
    assert!(d.contains("int value;"), "{d}");
}

/// A reopened module (`module M {} ... module M {}`) must not lose either
/// half's content once the AST builder merges the two occurrences.
#[test]
fn reopened_module_emits_both_structs() {
    let d = emit(
        "module M { @final struct A { long x; }; }; \
         module M { @final struct B { long y; }; };",
    );
    // #21: both halves emit under the module-qualified name `M_*`.
    assert!(d.contains("struct M_A {"), "{d}");
    assert!(d.contains("struct M_B {"), "{d}");
}

/// #21 cross-module collision: two different modules each declaring `Reading`
/// must emit distinct, module-qualified D types, never a duplicate bare type.
#[test]
fn cross_module_same_name_types_are_qualified() {
    let d = emit(
        "module a { @final struct Reading { long v; }; }; \
         module b { @final struct Reading { double w; }; };",
    );
    assert!(d.contains("struct a_Reading {"), "{d}");
    assert!(d.contains("struct b_Reading {"), "{d}");
    assert!(!d.contains("struct Reading {"), "{d}");
    assert!(d.contains("int v;"), "{d}");
    assert!(d.contains("double w;"), "{d}");
}

/// #21 cross-module reference: `module b`'s struct references `a::R`, which
/// must resolve to the qualified type `a_R`, not the bare `R`.
#[test]
fn cross_module_reference_resolves_to_qualified_type() {
    let d = emit(
        "module a { @final struct R { long v; }; }; \
         module b { @final struct S { a::R r; }; };",
    );
    assert!(d.contains("struct a_R {"), "{d}");
    assert!(d.contains("struct b_S {"), "{d}");
    // S's member `r` has the qualified type a_R.
    assert!(d.contains("a_R r;"), "{d}");
    assert!(d.contains("r.marshalInto"), "{d}");
    assert!(!d.contains(" R r;"), "{d}");
}

/// #21 compile gate: a two-module spec with a cross-module reference must
/// produce compilable D.
#[test]
fn cross_module_reference_compiles_with_gdc() {
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP cross_module_reference_compiles_with_gdc: `gdc` not on PATH");
        return;
    }
    let mut src = emit(
        "module a { @final struct R { long v; }; }; \
         module b { @final struct S { a::R r; }; };",
    );
    src.push_str(
        r#"
void main() {
    import std.stdio : writeln;
    b_S s;
    s.r.v = 7;
    auto b = s.marshalXCDR(Endian.LE);
    auto s2 = UnmarshalXCDRb_S(b, Endian.LE);
    assert(s2.r.v == 7);
    writeln("ok");
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idld_xmod_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "ok");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn final_struct_emits_struct_and_marshal() {
    let d = emit(GOLDEN_IDL);
    assert!(d.contains("struct Golden {"), "{d}");
    assert!(d.contains("uint id;"), "{d}");
    assert!(d.contains("ushort kind;"), "{d}");
    assert!(d.contains("ubyte flags;"), "{d}");
    assert!(d.contains("float value;"), "{d}");
    assert!(d.contains("ulong stamp;"), "{d}");
    assert!(d.contains("string label;"), "{d}");
    assert!(d.contains("ubyte[] raw;"), "{d}");
    assert!(d.contains("ubyte[] marshalXCDR(Endian endian) {"), "{d}");
    assert!(d.contains("w.putU32(id);"), "{d}");
    assert!(d.contains("w.putF32(value);"), "{d}");
    assert!(d.contains("w.putString(label);"), "{d}");
    assert!(d.contains("w.putSeqU8(raw);"), "{d}");
    assert!(!d.contains("auto b = Writer"), "{d}");
}

#[test]
fn appendable_struct_frames_a_dheader() {
    let d = emit("@appendable struct S { uint32 a; };");
    assert!(d.contains("auto zdBody = Writer(w.endian);"), "{d}");
    assert!(
        d.contains("w.putU32(cast(uint) zdBody.bytes().length);"),
        "{d}"
    );
    assert!(d.contains("w.putBytes(zdBody.bytes());"), "{d}");
}

// `class`/`version`/`body`/`scope`/`template`/`delegate` are D keywords but
// legal IDL identifiers (not reserved by the IDL grammar — unlike `in`/
// `out`, which IDL itself reserves for parameter directions). Before the
// #14 fix this emitted `struct class { uint32 version; ... }` — invalid D.
const KEYWORD_IDL: &str = "\
@final struct class {
    uint32 version;
    uint32 body;
};
union scope switch (long) {
    case 0: uint32 delegate_field;
    case 1: uint32 template;
};";

#[test]
fn keyword_identifiers_are_escaped_with_trailing_underscore() {
    let d = emit(KEYWORD_IDL);
    assert!(d.contains("struct class_ {"), "{d}");
    assert!(!d.contains("struct class {"), "{d}");
    assert!(d.contains("uint version_;"), "{d}");
    assert!(d.contains("uint body_;"), "{d}");
    assert!(d.contains("struct scope_ {"), "{d}");
    assert!(d.contains("uint template_;"), "{d}");
    // No bare D keyword survives as a standalone declared identifier token.
    for kw in ["class", "version", "body", "scope", "template"] {
        assert!(
            !d.contains(&format!(" {kw} ")) && !d.contains(&format!(" {kw};")),
            "unescaped keyword `{kw}` leaked into output:\n{d}"
        );
    }
}

#[test]
fn keyword_struct_reference_uses_escaped_name_consistently() {
    // A scoped reference to a keyword-named struct (nested-as-sequence-of)
    // must reuse the exact same escaped name as the declaration site.
    let d = emit(
        "@final struct class { uint32 a; };\
         @final struct Holder { sequence<class> items; };",
    );
    assert!(d.contains("struct class_ {"), "{d}");
    assert!(d.contains("class_[] items;"), "{d}");
    assert!(d.contains("unmarshalFromclass_"), "{d}");
}

const ENUM_IDL: &str = "\
enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };
@final struct S { Mode kind; uint32 tail; };";

#[test]
fn enum_emits_int_type_and_member_marshals() {
    let d = emit(ENUM_IDL);
    assert!(d.contains("enum Mode : int {"), "{d}");
    assert!(d.contains("MODE_IDLE = 0,"), "{d}");
    assert!(d.contains("MODE_FAULT = 2,"), "{d}");
    assert!(d.contains("Mode kind;"), "{d}");
    // An enum member is a 32-bit signed integer on the wire (XTypes §7.4.5.1).
    assert!(d.contains("w.putU32(cast(uint) kind);"), "{d}");
}

#[test]
fn enum_member_is_byte_identical_i32() {
    // Gated: needs gdc. S{ kind: MODE_FAULT(=2), tail: 0xDEADBEEF }
    // → i32 LE 02000000 + u32 LE efbeadde.
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP enum byte test: `gdc` not on PATH");
        return;
    }
    let mut src = emit(ENUM_IDL);
    src.push_str(
        r#"
string toHex(ubyte[] b) {
    static immutable char[16] d = "0123456789abcdef";
    char[] r;
    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }
    return cast(string) r;
}

void main() {
    import std.stdio : writeln;
    S s;
    s.kind = Mode.MODE_FAULT;
    s.tail = 0xDEADBEEF;
    writeln(toHex(s.marshalXCDR(Endian.LE)));
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idld_enum_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
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
    let d = emit(NESTED_IDL);
    assert!(d.contains("void marshalInto(ref Writer w)"), "{d}");
    assert!(d.contains("Inner one;"), "{d}");
    assert!(d.contains("Inner[] many;"), "{d}");
    assert!(d.contains("one.marshalInto(zdBody);"), "{d}");
    assert!(d.contains("zdElem.marshalInto(zdSub);"), "{d}");
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
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP nested byte: `gdc` not on PATH");
        return;
    }
    let mut src = emit(NESTED_IDL);
    src.push_str(
        r#"
string toHex(ubyte[] b) {
    static immutable char[16] d = "0123456789abcdef";
    char[] r;
    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }
    return cast(string) r;
}

void main() {
    import std.stdio : writeln;
    Outer o;
    o.id = 0xCAFEBABE;
    o.one = Inner(0x1111, 0x22223333);
    o.many = [Inner(0xAAAA, 0xBBBBCCCC), Inner(0xDDDD, 0xEEEEFFFF)];
    o.label = "nested";
    writeln(toHex(o.marshalXCDR(Endian.LE)));
    writeln(toHex(o.marshalXCDR(Endian.BE)));
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idld_nested_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
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
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP byte_identity: `gdc` not on PATH");
        return;
    }

    let mut src = emit(GOLDEN_IDL);
    src.push_str(
        r#"
string toHex(ubyte[] b) {
    static immutable char[16] d = "0123456789abcdef";
    char[] r;
    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }
    return cast(string) r;
}

void main() {
    import std.stdio : writeln;
    Golden g;
    g.id = 0xA1B2C3D4;
    g.kind = 0x1234;
    g.flags = 0x5A;
    g.value = 3.5;
    g.stamp = 0x0102030405060708;
    g.label = "bay-12";
    g.raw = [cast(ubyte) 0xDE, 0xAD, 0xBE, 0xEF];
    writeln(toHex(g.marshalXCDR(Endian.LE)));
    writeln(toHex(g.marshalXCDR(Endian.BE)));
}
"#,
    );

    let dir = std::env::temp_dir().join(format!("idld_golden_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");

    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
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
    let d = emit(TYPEDEF_IDL);
    assert!(d.contains("string name;"), "{d}");
    assert!(d.contains("ubyte[] data;"), "{d}");
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
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP typedef byte: `gdc` not on PATH");
        return;
    }
    let mut src = emit(TYPEDEF_IDL);
    src.push_str(
        r#"
string toHex(ubyte[] b) {
    static immutable char[16] d = "0123456789abcdef";
    char[] r;
    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }
    return cast(string) r;
}

void main() {
    import std.stdio : writeln;
    Rec r;
    r.id = 0xCAFEBABE;
    r.name = "typedef";
    r.data = [cast(ubyte) 1, 2, 3];
    writeln(toHex(r.marshalXCDR(Endian.LE)));
    writeln(toHex(r.marshalXCDR(Endian.BE)));
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idld_typedef_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
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
    let d = emit(ARRAY_IDL);
    assert!(d.contains("int[3] xs;"), "{d}");
    assert!(d.contains("short[2][2] m;"), "{d}");
    assert!(d.contains("ubyte[4] bs;"), "{d}");
    assert!(d.contains("for (size_t zdi0 = 0; zdi0 < 3; zdi0++)"), "{d}");
    assert!(d.contains("for (size_t zdi1 = 0; zdi1 < 2; zdi1++)"), "{d}");
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
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP array byte: `gdc` not on PATH");
        return;
    }
    let mut src = emit(ARRAY_IDL);
    src.push_str(
        r#"
string toHex(ubyte[] b) {
    static immutable char[16] d = "0123456789abcdef";
    char[] r;
    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }
    return cast(string) r;
}

void main() {
    import std.stdio : writeln;
    Arr a;
    a.xs = [0x11111111, 0x22222222, 0x33333333];
    a.m = [[cast(short) 0x0102, 0x0304], [cast(short) 0x0506, 0x0708]];
    a.bs = [cast(ubyte) 0xAA, 0xBB, 0xCC, 0xDD];
    writeln(toHex(a.marshalXCDR(Endian.LE)));
    writeln(toHex(a.marshalXCDR(Endian.BE)));
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idld_array_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
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
    let d = emit(UNION_IDL);
    assert!(d.contains("int disc;"), "{d}");
    assert!(d.contains("if (disc == 1)"), "{d}");
    assert!(d.contains("else if (disc == 2)"), "{d}");
    assert!(d.contains("else {"), "{d}");
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
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP union byte: `gdc` not on PATH");
        return;
    }
    let mut src = emit(UNION_IDL);
    src.push_str(
        r#"
string toHex(ubyte[] b) {
    static immutable char[16] d = "0123456789abcdef";
    char[] r;
    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }
    return cast(string) r;
}

void main() {
    import std.stdio : writeln;
    U u;
    u.disc = 2;
    u.b = 0x1234;
    writeln(toHex(u.marshalXCDR(Endian.LE)));
    writeln(toHex(u.marshalXCDR(Endian.BE)));
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idld_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
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
    let d = emit(MAP_IDL);
    assert!(d.contains("uint[int] m;"), "{d}");
    assert!(d.contains("zdKeys.sort();"), "{d}");
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
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP map byte: `gdc` not on PATH");
        return;
    }
    let mut src = emit(MAP_IDL);
    src.push_str(
        r#"
string toHex(ubyte[] b) {
    static immutable char[16] d = "0123456789abcdef";
    char[] r;
    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }
    return cast(string) r;
}

void main() {
    import std.stdio : writeln;
    HasMap h;
    h.m = [1: 0x11111111u, 2: 0x22222222u];
    writeln(toHex(h.marshalXCDR(Endian.LE)));
    writeln(toHex(h.marshalXCDR(Endian.BE)));
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idld_map_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
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
    let d = emit(MUTABLE_IDL);
    assert!(d.contains("zdBody.putU32(0x4000000au);"), "{d}");
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
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP mutable byte: `gdc` not on PATH");
        return;
    }
    let mut src = emit(MUTABLE_IDL);
    src.push_str(
        r#"
string toHex(ubyte[] b) {
    static immutable char[16] d = "0123456789abcdef";
    char[] r;
    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }
    return cast(string) r;
}

void main() {
    import std.stdio : writeln;
    M m;
    m.x = 0xDEADBEEF;
    m.s = "mut";
    m.k = 0x0777;
    writeln(toHex(m.marshalXCDR(Endian.LE)));
    writeln(toHex(m.marshalXCDR(Endian.BE)));
}
"#,
    );
    let dir = std::env::temp_dir().join(format!("idld_mutable_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
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

fn run_d(idl: &str, main_body: &str, stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP {stem}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP {stem}: `gdc` not on PATH");
        return;
    }
    let mut src = emit(idl);
    src.push_str(main_body);
    let dir = std::env::temp_dir().join(format!("idld_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
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

const D_HEX: &str = r#"
string toHex(ubyte[] b) {
    static immutable char[16] d = "0123456789abcdef";
    char[] r;
    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }
    return cast(string) r;
}
"#;

#[test]
fn wide_is_byte_identical_vs_rust_golden() {
    let body = format!(
        "{D_HEX}\nvoid main() {{ import std.stdio : writeln; W w; w.c = 0x03A9; w.s = \"w\u{03c0}\"; writeln(toHex(w.marshalXCDR(Endian.LE))); writeln(toHex(w.marshalXCDR(Endian.BE))); }}\n"
    );
    run_d(WIDE_IDL, &body, "wide");
}

#[test]
fn longdouble_is_byte_identical_vs_rust_golden() {
    let body = format!(
        "{D_HEX}\nvoid main() {{ import std.stdio : writeln; L l; l.d = 1.1; writeln(toHex(l.marshalXCDR(Endian.LE))); writeln(toHex(l.marshalXCDR(Endian.BE))); }}\n"
    );
    run_d(LD_IDL, &body, "longdouble");
}

const NESTED_KEY_IDL: &str = "\
@final struct Inner { @key long x; long ignored; @key long y; };\n\
@final struct Outer { @key Inner i; };";

/// Bug A regression: a `@key` member whose type is itself a struct must
/// expand to ONLY that struct's own `@key` members (x, y), not its full
/// member set — `ignored` must not appear in the keyHash body.
#[test]
fn nested_struct_key_excludes_non_key_fields() {
    let d = emit(NESTED_KEY_IDL);
    let outer = d.find("struct Outer {").expect("Outer struct");
    let start = outer
        + d[outer..]
            .find("ubyte[16] keyHash() {")
            .expect("keyHash method");
    let end = d[start..]
        .find("\n    }\n")
        .map(|i| start + i)
        .unwrap_or(d.len());
    let body = &d[start..end];
    assert!(body.contains("i.x"), "{body}");
    assert!(body.contains("i.y"), "{body}");
    assert!(!body.contains("i.ignored"), "{body}");
    // The full-marshal call must not be reused for the key encoding.
    assert!(!body.contains("i.marshalInto"), "{body}");
}

const NESTED_KEY_SMALL_IDL: &str = "\
@final struct Inner { @key octet a; };\n\
@final struct Outer { @key Inner i; };";

/// Bug B regression: `uses_md5` must be given a real `structs` map so a
/// small nested-struct `@key` (here: 1 octet = 1 byte <= 16) resolves and
/// takes the zero-pad branch, not the MD5 branch forced by an unresolvable
/// (empty-map) struct lookup.
#[test]
fn nested_struct_key_small_takes_zero_pad_branch() {
    let d = emit(NESTED_KEY_SMALL_IDL);
    let outer = d.find("struct Outer {").expect("Outer struct");
    let start = outer
        + d[outer..]
            .find("ubyte[16] keyHash() {")
            .expect("keyHash method");
    let end = d[start..]
        .find("\n    }\n")
        .map(|i| start + i)
        .unwrap_or(d.len());
    let body = &d[start..end];
    assert!(
        body.contains("foreach (i, x; b) if (i < 16) outk[i] = x;"),
        "{body}"
    );
    assert!(!body.contains("md5Of"), "{body}");
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
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash: `gdc` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_IDL);
    src.push_str("\nstring toHex(ubyte[] b) {\n    static immutable char[16] d = \"0123456789abcdef\";\n    char[] r;\n    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }\n    return cast(string) r;\n}\n\nvoid main() {\n    import std.stdio : writeln;\n    K k;\n    k.a = 0x01020304;\n    k.b = 0x0506;\n    writeln(toHex(k.keyHash()));\n}\n");
    let dir = std::env::temp_dir().join(format!("idld_kh_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
    let stdout = String::from_utf8(run.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_of(Path::new(&golden_dir).join("golden_keyhash.bin"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn run_roundtrip(idl: &str, ty: &str, stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP rt: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP rt: `gdc` not on PATH");
        return;
    }
    let file = |e: &str| {
        if stem.is_empty() {
            format!("golden_{e}.bin")
        } else {
            format!("golden_{stem}_{e}.bin")
        }
    };
    let lit = |b: &[u8]| {
        b.iter()
            .map(|x| format!("cast(ubyte) 0x{x:02x}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let hx = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let le = std::fs::read(Path::new(&golden_dir).join(file("le"))).expect("le");
    let be = std::fs::read(Path::new(&golden_dir).join(file("be"))).expect("be");
    let mut src = emit(idl);
    src.push_str(&format!("\nstring toHex(ubyte[] b) {{ static immutable char[16] d = \"0123456789abcdef\"; char[] r; foreach (x; b) {{ r ~= d[x >> 4]; r ~= d[x & 0xf]; }} return cast(string) r; }}\nvoid main() {{ import std.stdio : writeln;\n  ubyte[] le = [{}];\n  ubyte[] be = [{}];\n  writeln(toHex(UnmarshalXCDR{ty}(le, Endian.LE).marshalXCDR(Endian.LE)));\n  writeln(toHex(UnmarshalXCDR{ty}(be, Endian.BE).marshalXCDR(Endian.BE)));\n}}\n", lit(&le), lit(&be)));
    let dir = std::env::temp_dir().join(format!("idld_rt_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
    let stdout = String::from_utf8(run.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(lines.next().expect("le").trim(), hx(&le), "LE");
    assert_eq!(lines.next().expect("be").trim(), hx(&be), "BE");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn decode_roundtrip_final() {
    run_roundtrip(GOLDEN_IDL, "Golden", "");
}
#[test]
fn decode_roundtrip_nested() {
    run_roundtrip(NESTED_IDL, "Outer", "nested");
}
#[test]
fn decode_roundtrip_array() {
    run_roundtrip(ARRAY_IDL, "Arr", "array");
}
#[test]
fn decode_roundtrip_union() {
    run_roundtrip(UNION_IDL, "U", "union");
}
#[test]
fn decode_roundtrip_map() {
    run_roundtrip(MAP_IDL, "HasMap", "map");
}
#[test]
fn decode_roundtrip_mutable() {
    run_roundtrip(MUTABLE_IDL, "M", "mutable");
}
#[test]
fn decode_roundtrip_wide() {
    run_roundtrip(WIDE_IDL, "W", "wide");
}
#[test]
fn decode_roundtrip_longdouble() {
    run_roundtrip(LD_IDL, "L", "longdouble");
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
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash_md5: `gdc` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_MD5_IDL);
    src.push_str("\nstring toHex(ubyte[] b) {\n    static immutable char[16] d = \"0123456789abcdef\";\n    char[] r;\n    foreach (x; b) { r ~= d[x >> 4]; r ~= d[x & 0xf]; }\n    return cast(string) r;\n}\n\nvoid main() {\n    import std.stdio : writeln;\n    KL k;\n    k.a = 0x01020304;\n    k.b = 0x05060708;\n    k.c = 0x090A0B0C;\n    k.d = 0x0D0E0F10;\n    k.e = 0x11121314;\n    writeln(toHex(k.keyHash()));\n}\n");
    let dir = std::env::temp_dir().join(format!("idld_kh_md5_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
    let stdout = String::from_utf8(run.stdout).expect("utf8");
    assert_eq!(
        stdout.lines().next().expect("h").trim(),
        hex_of(Path::new(&golden_dir).join("golden_keyhash_md5.bin"))
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

// ===========================================================================
// Section-F Wave-1: bitset/bitmask, @optional, fixed<d,s>, sequence-arbitrary,
// @verbatim. Always-on source-asserts + gdc-gated compile-and-run tests whose
// expected wire hex is derived from the spec IN the test (no GOLDEN_DIR oracle).
// ===========================================================================

/// Compiles `emit(idl) + D_HEX + main_body` with `gdc`, runs it, and returns
/// the trimmed stdout lines. `None` (skip) if `gdc` is not on PATH.
fn gdc_lines(idl: &str, main_body: &str, tag: &str) -> Option<Vec<String>> {
    if Command::new("gdc").arg("--version").output().is_err() {
        eprintln!("SKIP {tag}: `gdc` not on PATH");
        return None;
    }
    let mut src = emit(idl);
    src.push_str(D_HEX);
    src.push_str(main_body);
    let dir = std::env::temp_dir().join(format!("idld_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.d"), &src).expect("write");
    let build = Command::new("gdc")
        .args(["main.d", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new("./main_bin")
        .current_dir(&dir)
        .output()
        .expect("run");
    assert!(
        run.status.success(),
        "run failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let out = String::from_utf8(run.stdout).expect("utf8");
    let lines: Vec<String> = out.lines().map(|l| l.trim().to_string()).collect();
    let _ = std::fs::remove_dir_all(&dir);
    Some(lines)
}

// ---- bitset -------------------------------------------------------------

const BITSET_IDL: &str = "bitset Flags { bitfield<4> a; bitfield<8> b; };";

#[test]
fn bitset_emits_holder_struct_and_accessors() {
    let d = emit(BITSET_IDL);
    // 4 + 8 = 12 bits → ushort backing (XTypes §7.4.7).
    assert!(d.contains("struct Flags {"), "{d}");
    assert!(d.contains("ushort storage;"), "{d}");
    assert!(d.contains("ushort a()"), "{d}");
    assert!(d.contains("ushort b()"), "{d}");
    assert!(
        d.contains("void marshalInto(ref Writer w) { w.putU16(storage); }"),
        "{d}"
    );
    assert!(d.contains("unmarshalFromFlags"), "{d}");
}

#[test]
fn bitset_wire_is_backing_int() {
    // storage 0xABCD as a ushort → LE "cdab", BE "abcd"; round-trips.
    let body = r#"
void main() {
    import std.stdio : writeln;
    Flags f;
    f.storage = 0xABCD;
    writeln(toHex(f.marshalXCDR(Endian.LE)));
    writeln(toHex(f.marshalXCDR(Endian.BE)));
    writeln(toHex(UnmarshalXCDRFlags(f.marshalXCDR(Endian.LE), Endian.LE).marshalXCDR(Endian.LE)));
}
"#;
    let Some(l) = gdc_lines(BITSET_IDL, body, "bitset") else {
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
    let d = emit(BITMASK_IDL);
    // Default @bit_bound = 32 → uint backing (XTypes §7.3.1.2.1.1).
    assert!(d.contains("struct Perms {"), "{d}");
    assert!(d.contains("uint storage;"), "{d}");
    assert!(
        d.contains("enum uint PERM_READ = cast(uint)(1) << 0;"),
        "{d}"
    );
    assert!(
        d.contains("enum uint PERM_EXEC = cast(uint)(1) << 2;"),
        "{d}"
    );
    assert!(
        d.contains("void marshalInto(ref Writer w) { w.putU32(storage); }"),
        "{d}"
    );
}

#[test]
fn bitmask_wire_is_backing_uint32() {
    // PERM_READ | PERM_EXEC = 0x05 → LE "05000000", BE "00000005"; round-trips.
    let body = r#"
void main() {
    import std.stdio : writeln;
    Perms p;
    p.storage = Perms.PERM_READ | Perms.PERM_EXEC;
    writeln(toHex(p.marshalXCDR(Endian.LE)));
    writeln(toHex(p.marshalXCDR(Endian.BE)));
    writeln(toHex(UnmarshalXCDRPerms(p.marshalXCDR(Endian.BE), Endian.BE).marshalXCDR(Endian.BE)));
}
"#;
    let Some(l) = gdc_lines(BITMASK_IDL, body, "bitmask") else {
        return;
    };
    assert_eq!(l[0], "05000000", "LE");
    assert_eq!(l[1], "00000005", "BE");
    assert_eq!(l[2], "00000005", "round-trip");
}

#[test]
fn bitmask_bit_bound_narrows_backing() {
    let d = emit("@bit_bound(8) bitmask Small { A, B };");
    assert!(d.contains("ubyte storage;"), "{d}");
    assert!(
        d.contains("void marshalInto(ref Writer w) { w.putU8(storage); }"),
        "{d}"
    );
}

// ---- fixed<d,s> ---------------------------------------------------------

const FIXED_IDL: &str = "@final struct HasFixed { fixed<5,2> price; };";

#[test]
fn fixed_emits_bcd_field_and_prelude() {
    let d = emit(FIXED_IDL);
    assert!(d.contains("ubyte[] price;"), "{d}");
    assert!(d.contains("w.putBytes(price);"), "{d}");
    assert!(
        d.contains("ubyte[] zdFixedEnc(string s, uint P, uint S)"),
        "{d}"
    );
}

#[test]
fn fixed_wire_is_packed_bcd() {
    // fixed<5,2> 123.45 → BCD "12 34 5c" (odd P, no pad, CORBA §9.3.2.7).
    let body = r#"
void main() {
    import std.stdio : writeln;
    HasFixed h;
    h.price = zdFixedEnc("123.45", 5, 2);
    writeln(toHex(h.marshalXCDR(Endian.LE)));
    writeln(toHex(h.marshalXCDR(Endian.BE)));
    writeln(toHex(UnmarshalXCDRHasFixed(h.marshalXCDR(Endian.LE), Endian.LE).marshalXCDR(Endian.LE)));
}
"#;
    let Some(l) = gdc_lines(FIXED_IDL, body, "fixed") else {
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
void main() {
    import std.stdio : writeln;
    HasFixed h;
    h.price = zdFixedEnc("1234", 4, 0);
    writeln(toHex(h.marshalXCDR(Endian.LE)));
}
"#;
    let Some(l) = gdc_lines(
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
    let d = emit(SEQARB_IDL);
    assert!(d.contains("int[] xs;"), "{d}");
    assert!(d.contains("w.putU32(cast(uint) xs.length);"), "{d}");
    assert!(d.contains("foreach (zdElem; xs)"), "{d}");
}

#[test]
fn sequence_arbitrary_wire_count_plus_elements() {
    // [0x01020304, 0x05060708] → u32 count 2 + two i32 elements, no DHEADER.
    let body = r#"
void main() {
    import std.stdio : writeln;
    S s;
    s.xs = [0x01020304, 0x05060708];
    writeln(toHex(s.marshalXCDR(Endian.LE)));
    writeln(toHex(s.marshalXCDR(Endian.BE)));
    writeln(toHex(UnmarshalXCDRS(s.marshalXCDR(Endian.LE), Endian.LE).marshalXCDR(Endian.LE)));
}
"#;
    let Some(l) = gdc_lines(SEQARB_IDL, body, "seqarb") else {
        return;
    };
    assert_eq!(l[0], "020000000403020108070605", "LE");
    assert_eq!(l[1], "000000020102030405060708", "BE");
    assert_eq!(l[2], "020000000403020108070605", "round-trip");
}

#[test]
fn sequence_of_enum_is_arbitrary_path() {
    // A `sequence<enum>` must now emit (was rejected pre-Wave-1).
    let d = emit("enum E { E0, E1 }; @final struct SE { sequence<E> es; };");
    assert!(d.contains("E[] es;"), "{d}");
    assert!(d.contains("foreach (zdElem; es)"), "{d}");
    assert!(d.contains("w.putU32(cast(uint) es.length);"), "{d}");
}

// ---- @optional ----------------------------------------------------------

const OPT_IDL: &str = "@final struct Opt { uint32 a; @optional uint32 b; };";

#[test]
fn optional_emits_presence_flag() {
    let d = emit(OPT_IDL);
    assert!(d.contains("bool b_present;"), "{d}");
    assert!(
        d.contains("w.putU8(b_present ? 1 : 0); if (b_present) {"),
        "{d}"
    );
    assert!(
        d.contains("v.b_present = r.getBool(); if (v.b_present) {"),
        "{d}"
    );
}

#[test]
fn optional_final_wire_present_and_absent() {
    // present: u32 a, u8 flag=1, pad(3), u32 b. absent: u32 a, u8 flag=0.
    let body = r#"
void main() {
    import std.stdio : writeln;
    Opt p;
    p.a = 0x11223344;
    p.b_present = true;
    p.b = 0xAABBCCDD;
    writeln(toHex(p.marshalXCDR(Endian.LE)));
    writeln(toHex(p.marshalXCDR(Endian.BE)));
    Opt q;
    q.a = 0x11223344;
    q.b_present = false;
    writeln(toHex(q.marshalXCDR(Endian.LE)));
    // round-trips
    writeln(toHex(UnmarshalXCDROpt(p.marshalXCDR(Endian.LE), Endian.LE).marshalXCDR(Endian.LE)));
    writeln(toHex(UnmarshalXCDROpt(q.marshalXCDR(Endian.LE), Endian.LE).marshalXCDR(Endian.LE)));
}
"#;
    let Some(l) = gdc_lines(OPT_IDL, body, "opt") else {
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
void main() {
    import std.stdio : writeln;
    OptA p;
    p.a = 0xCAFEBABE;
    p.b_present = true;
    p.b = 0x01020304;
    auto pe = p.marshalXCDR(Endian.LE);
    writeln(toHex(pe));
    writeln(toHex(UnmarshalXCDROptA(pe, Endian.LE).marshalXCDR(Endian.LE)));
    OptA q;
    q.a = 0xCAFEBABE;
    q.b_present = false;
    auto qe = q.marshalXCDR(Endian.LE);
    writeln(toHex(qe));
    writeln(toHex(UnmarshalXCDROptA(qe, Endian.LE).marshalXCDR(Endian.LE)));
}
"#;
    let Some(l) = gdc_lines(idl, body, "opta") else {
        return;
    };
    assert_eq!(l[0], l[1], "present round-trip");
    assert_eq!(l[2], l[3], "absent round-trip");
}

// ---- @verbatim ----------------------------------------------------------

#[test]
fn verbatim_placements_inject_text() {
    let d = emit(
        "@verbatim(language=\"d\", placement=BEGIN_FILE, text=\"// zd-begin-file\")\n\
         @verbatim(language=\"d\", placement=BEFORE_DECLARATION, text=\"// zd-before\")\n\
         @verbatim(language=\"d\", placement=BEGIN_DECLARATION, text=\"// zd-begin-decl\")\n\
         @verbatim(language=\"d\", placement=END_DECLARATION, text=\"// zd-end-decl\")\n\
         @verbatim(language=\"d\", placement=AFTER_DECLARATION, text=\"// zd-after\")\n\
         @verbatim(language=\"d\", placement=END_FILE, text=\"// zd-end-file\")\n\
         @final struct V { uint32 a; };",
    );
    for marker in [
        "// zd-begin-file",
        "// zd-before",
        "// zd-begin-decl",
        "// zd-end-decl",
        "// zd-after",
        "// zd-end-file",
    ] {
        assert!(d.contains(marker), "missing {marker}:\n{d}");
    }
    // Ordering: begin-file before the struct; before-decl before `struct V`;
    // begin-decl after the opening brace; after-decl/end-file trail the type.
    let sidx = d.find("struct V {").expect("struct");
    assert!(d.find("// zd-begin-file").unwrap() < sidx, "{d}");
    assert!(d.find("// zd-before").unwrap() < sidx, "{d}");
    assert!(d.find("// zd-begin-decl").unwrap() > sidx, "{d}");
    assert!(d.find("// zd-end-file").unwrap() > sidx, "{d}");
}

#[test]
fn verbatim_language_filter_excludes_other_langs() {
    // A non-D language tag must NOT leak into the D output.
    let d = emit(
        "@verbatim(language=\"java\", placement=BEFORE_DECLARATION, text=\"// java-only\")\n\
         @final struct V { uint32 a; };",
    );
    assert!(!d.contains("// java-only"), "{d}");
    // The wildcard `*` still matches D.
    let d2 = emit(
        "@verbatim(placement=BEFORE_DECLARATION, text=\"// wildcard\")\n\
         @final struct V { uint32 a; };",
    );
    assert!(d2.contains("// wildcard"), "{d2}");
}

#[test]
fn verbatim_output_still_compiles() {
    let idl = "@verbatim(language=\"d\", placement=BEGIN_FILE, text=\"// zd file header\")\n\
         @verbatim(language=\"d\", placement=BEGIN_DECLARATION, text=\"// zd inside struct\")\n\
         @final struct V { uint32 a; };";
    let body = r#"
void main() {
    import std.stdio : writeln;
    V v;
    v.a = 0x2A;
    writeln(toHex(v.marshalXCDR(Endian.LE)));
}
"#;
    let Some(l) = gdc_lines(idl, body, "verbatim") else {
        return;
    };
    assert_eq!(l[0], "2a000000");
}
