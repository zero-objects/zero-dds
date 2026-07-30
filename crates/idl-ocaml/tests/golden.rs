// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! OCaml backend: string smoke tests (always) + a byte-identity test that
//! compiles+runs the generated OCaml and compares to the Rust goldens (gated on
//! `ocamlfind` on PATH and `GOLDEN_DIR` pointing at golden_{le,be}.bin).

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
use zerodds_idl_ocaml::{OcamlGenOptions, generate_ocaml_module};

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
    generate_ocaml_module(&ast, &OcamlGenOptions::default()).expect("gen")
}

/// swarm59 #21b: `module X { struct Y { ... }; }` used to be silently
/// dropped (no `Definition::Module` arm at all) — the struct must now emit.
#[test]
fn module_wrapped_struct_is_emitted_not_dropped() {
    let o = emit("module Telemetry { @final struct Reading { long value; }; };");
    // #21: a module-wrapped type is emitted as its module-qualified OCaml module.
    assert!(o.contains("module Telemetry_sReading = struct"), "{o}");
    assert!(o.contains("value : int;"), "{o}");
}

/// A reopened module (`module M {} ... module M {}`) must not lose either
/// half's content once the AST builder merges the two occurrences.
#[test]
fn reopened_module_emits_both_structs() {
    let o = emit(
        "module M { @final struct A { long x; }; }; \
         module M { @final struct B { long y; }; };",
    );
    // #21: both halves emit under the module-qualified OCaml module `M_*`.
    assert!(o.contains("module M_sA = struct"), "{o}");
    assert!(o.contains("module M_sB = struct"), "{o}");
}

/// #21 cross-module collision: two different modules each declaring `Reading`
/// must emit distinct, module-qualified OCaml modules, never a duplicate one.
#[test]
fn cross_module_same_name_types_are_qualified() {
    let o = emit(
        "module a { @final struct Reading { long v; }; }; \
         module b { @final struct Reading { double w; }; };",
    );
    assert!(o.contains("module A_sReading = struct"), "{o}");
    assert!(o.contains("module B_sReading = struct"), "{o}");
    assert!(!o.contains("module Reading = struct"), "{o}");
    assert!(o.contains("v : int;"), "{o}");
    assert!(o.contains("w : float;"), "{o}");
}

/// #21 cross-module reference: `module b`'s struct references `a::R`, which
/// must resolve to the qualified OCaml module `A_R`, not the bare `R`.
#[test]
fn cross_module_reference_resolves_to_qualified_type() {
    let o = emit(
        "module a { @final struct R { long v; }; }; \
         module b { @final struct S { a::R r; }; };",
    );
    assert!(o.contains("module A_sR = struct"), "{o}");
    assert!(o.contains("module B_sS = struct"), "{o}");
    // S's member `r` has the qualified type A_sR.t and marshals via it.
    assert!(o.contains("r : A_sR.t;"), "{o}");
    assert!(o.contains("A_sR.marshal_into"), "{o}");
}

/// #21 compile gate: a two-module spec with a cross-module reference must
/// produce compilable OCaml.
#[test]
fn cross_module_reference_compiles_with_ocaml() {
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP cross_module_reference_compiles_with_ocaml: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(
        "module a { @final struct R { long v; }; }; \
         module b { @final struct S { a::R r; }; };",
    );
    src.push_str(
        "
let () =
  let s : B_sS.t = { r = ({ v = 7 } : A_sR.t) } in
  let _ = B_sS.marshal s Wire.LE in
  print_endline \"ok\"
",
    );
    let dir = std::env::temp_dir().join(format!("idlocaml_xmod_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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
fn final_struct_emits_record_and_marshal() {
    let o = emit(GOLDEN_IDL);
    assert!(o.contains("module Golden = struct"), "{o}");
    assert!(o.contains("id : int;"), "{o}");
    assert!(o.contains("kind : int;"), "{o}");
    assert!(o.contains("value : float;"), "{o}");
    assert!(o.contains("stamp : int64;"), "{o}");
    assert!(o.contains("label : string;"), "{o}");
    assert!(o.contains("raw : bytes;"), "{o}");
    assert!(
        o.contains("let marshal (v : t) (endian : Wire.endian) : bytes ="),
        "{o}"
    );
    assert!(o.contains("Wire.put_u32 w v.id;"), "{o}");
    assert!(o.contains("Wire.put_f32 w v.value;"), "{o}");
    assert!(o.contains("Wire.put_string w v.label;"), "{o}");
    assert!(o.contains("Wire.put_seq_u8 w v.raw;"), "{o}");
    assert!(!o.contains("let bb = Wire.bytes body"), "{o}");
}

#[test]
fn appendable_struct_frames_a_dheader() {
    let o = emit("@appendable struct S { uint32 a; };");
    assert!(o.contains("let bb = Wire.bytes body in"), "{o}");
    assert!(o.contains("Wire.put_u32 w (Bytes.length bb);"), "{o}");
    assert!(o.contains("Wire.put_bytes w bb"), "{o}");
}

const ENUM_IDL: &str = "\
enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };
@final struct S { Mode kind; uint32 tail; };";

#[test]
fn enum_emits_variant_and_member_marshals() {
    let o = emit(ENUM_IDL);
    assert!(
        o.contains("type mode = MODE_IDLE | MODE_ACTIVE | MODE_FAULT"),
        "{o}"
    );
    assert!(
        o.contains(
            "let mode_to_int = function MODE_IDLE -> 0 | MODE_ACTIVE -> 1 | MODE_FAULT -> 2"
        ),
        "{o}"
    );
    assert!(o.contains("kind : mode;"), "{o}");
    // An enum member is a 32-bit signed integer on the wire (XTypes §7.4.5.1).
    assert!(o.contains("Wire.put_u32 w (mode_to_int v.kind)"), "{o}");
}

#[test]
fn enum_member_is_byte_identical_i32() {
    // Gated: needs ocamlfind. S{ kind: MODE_FAULT(=2), tail: 0xDEADBEEF }
    // -> i32 LE 02000000 + u32 LE efbeadde.
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP enum byte test: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(ENUM_IDL);
    src.push_str(
        "
let () =
  let s : S.t = { kind = MODE_FAULT; tail = 0xDEADBEEF } in
  let hex b =
    String.concat \"\"
      (List.init (Bytes.length b) (fun i -> Printf.sprintf \"%02x\" (Char.code (Bytes.get b i))))
  in
  print_endline (hex (S.marshal s Wire.LE))
",
    );
    let dir = std::env::temp_dir().join(format!("idlocaml_enum_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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
    let o = emit(NESTED_IDL);
    assert!(
        o.contains("let marshal_into (v : t) (w : Wire.writer) (endian : Wire.endian) : unit ="),
        "{o}"
    );
    assert!(o.contains("one : Inner.t;"), "{o}");
    assert!(o.contains("many : Inner.t list;"), "{o}");
    assert!(o.contains("Inner.marshal_into v.one body endian"), "{o}");
    assert!(o.contains("Inner.marshal_into e sub endian"), "{o}");
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
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP nested byte: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(NESTED_IDL);
    src.push_str(
        r#"
let () =
  let o : Outer.t =
    { id = 0xCAFEBABE;
      one = { Inner.a = 0x1111; b = 0x22223333 };
      many = [ { Inner.a = 0xAAAA; b = 0xBBBBCCCC }; { Inner.a = 0xDDDD; b = 0xEEEEFFFF } ];
      label = "nested" }
  in
  let hex b =
    String.concat ""
      (List.init (Bytes.length b) (fun i -> Printf.sprintf "%02x" (Char.code (Bytes.get b i))))
  in
  print_endline (hex (Outer.marshal o Wire.LE));
  print_endline (hex (Outer.marshal o Wire.BE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlocaml_nested_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP byte_identity: `ocamlfind` not on PATH");
        return;
    }

    let mut src = emit(GOLDEN_IDL);
    src.push_str(
        r#"
let () =
  let g : Golden.t =
    { id = 0xA1B2C3D4; kind = 0x1234; flags = 0x5A; value = 3.5;
      stamp = 0x0102030405060708L; label = "bay-12";
      raw = Bytes.of_string "\xDE\xAD\xBE\xEF" }
  in
  let hex b =
    String.concat ""
      (List.init (Bytes.length b) (fun i -> Printf.sprintf "%02x" (Char.code (Bytes.get b i))))
  in
  print_endline (hex (Golden.marshal g Wire.LE));
  print_endline (hex (Golden.marshal g Wire.BE))
"#,
    );

    let dir = std::env::temp_dir().join(format!("idlocaml_golden_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");

    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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
    let o = emit(TYPEDEF_IDL);
    assert!(o.contains("name : string;"), "{o}");
    assert!(o.contains("data : bytes;"), "{o}");
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
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP typedef byte: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(TYPEDEF_IDL);
    src.push_str(
        r#"
let () =
  let r : Rec.t =
    { id = 0xCAFEBABE;
      name = "typedef";
      data = Bytes.of_string "\x01\x02\x03" }
  in
  let hex b =
    String.concat ""
      (List.init (Bytes.length b) (fun i -> Printf.sprintf "%02x" (Char.code (Bytes.get b i))))
  in
  print_endline (hex (Rec.marshal r Wire.LE));
  print_endline (hex (Rec.marshal r Wire.BE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlocaml_typedef_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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
    let o = emit(ARRAY_IDL);
    assert!(o.contains("xs : int array;"), "{o}");
    assert!(o.contains("m : int array array;"), "{o}");
    assert!(o.contains("bs : int array;"), "{o}");
    assert!(o.contains("for zdi0 = 0 to 2 do"), "{o}");
    assert!(o.contains("for zdi1 = 0 to 1 do"), "{o}");
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
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP array byte: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(ARRAY_IDL);
    src.push_str(
        r#"
let () =
  let a : Arr.t =
    { xs = [| 0x11111111; 0x22222222; 0x33333333 |];
      m = [| [| 0x0102; 0x0304 |]; [| 0x0506; 0x0708 |] |];
      bs = [| 0xAA; 0xBB; 0xCC; 0xDD |] }
  in
  let hex b =
    String.concat ""
      (List.init (Bytes.length b) (fun i -> Printf.sprintf "%02x" (Char.code (Bytes.get b i))))
  in
  print_endline (hex (Arr.marshal a Wire.LE));
  print_endline (hex (Arr.marshal a Wire.BE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlocaml_array_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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
    let o = emit(UNION_IDL);
    assert!(o.contains("disc : int;"), "{o}");
    assert!(o.contains("match v.disc with"), "{o}");
    assert!(o.contains("| 1 ->"), "{o}");
    assert!(o.contains("| 2 ->"), "{o}");
    assert!(o.contains("| _ ->"), "{o}");
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
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP union byte: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(UNION_IDL);
    src.push_str(
        r#"
let () =
  let u : U.t = { U.disc = 2; a = 0; b = 0x1234; c = 0 } in
  let hex b =
    String.concat ""
      (List.init (Bytes.length b) (fun i -> Printf.sprintf "%02x" (Char.code (Bytes.get b i))))
  in
  print_endline (hex (U.marshal u Wire.LE));
  print_endline (hex (U.marshal u Wire.BE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlocaml_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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
    let o = emit(MAP_IDL);
    assert!(o.contains("m : (int * int) list;"), "{o}");
    assert!(o.contains("List.sort"), "{o}");
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
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP map byte: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(MAP_IDL);
    src.push_str(
        r#"
let () =
  let h : HasMap.t = { HasMap.m = [(1, 0x11111111); (2, 0x22222222)] } in
  let hex b =
    String.concat ""
      (List.init (Bytes.length b) (fun i -> Printf.sprintf "%02x" (Char.code (Bytes.get b i))))
  in
  print_endline (hex (HasMap.marshal h Wire.LE));
  print_endline (hex (HasMap.marshal h Wire.BE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlocaml_map_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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
    let o = emit(MUTABLE_IDL);
    assert!(o.contains("Wire.put_u32 body 0x4000000a;"), "{o}");
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
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP mutable byte: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(MUTABLE_IDL);
    src.push_str(
        r#"
let () =
  let m : M.t = { M.x = 0xDEADBEEF; s = "mut"; k = 0x0777 } in
  let hex b =
    String.concat ""
      (List.init (Bytes.length b) (fun i -> Printf.sprintf "%02x" (Char.code (Bytes.get b i))))
  in
  print_endline (hex (M.marshal m Wire.LE));
  print_endline (hex (M.marshal m Wire.BE))
"#,
    );
    let dir = std::env::temp_dir().join(format!("idlocaml_mutable_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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

fn run_ocaml(idl: &str, main_body: &str, stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP {stem}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP {stem}: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(idl);
    src.push_str(main_body);
    let dir = std::env::temp_dir().join(format!("idlocaml_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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

const OC_HEX: &str = "\n  let hex b =\n    String.concat \"\"\n      (List.init (Bytes.length b) (fun i -> Printf.sprintf \"%02x\" (Char.code (Bytes.get b i))))\n  in\n";

#[test]
fn wide_is_byte_identical_vs_rust_golden() {
    let body = format!(
        "\nlet () =\n  let v : W.t = {{ W.c = 0x03A9; s = \"w\u{03c0}\" }} in{OC_HEX}  print_endline (hex (W.marshal v Wire.LE));\n  print_endline (hex (W.marshal v Wire.BE))\n"
    );
    run_ocaml(WIDE_IDL, &body, "wide");
}
#[test]
fn longdouble_is_byte_identical_vs_rust_golden() {
    let body = format!(
        "\nlet () =\n  let v : L.t = {{ L.d = 1.1 }} in{OC_HEX}  print_endline (hex (L.marshal v Wire.LE));\n  print_endline (hex (L.marshal v Wire.BE))\n"
    );
    run_ocaml(LD_IDL, &body, "longdouble");
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
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP keyhash: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_IDL);
    src.push_str("\nlet () =\n  let k : K.t = { K.a = 0x01020304; b = 0x0506; c = 0 } in\n  let b = K.key_hash k in\n  let hex bb = String.concat \"\" (List.init (Bytes.length bb) (fun i -> Printf.sprintf \"%02x\" (Char.code (Bytes.get bb i)))) in\n  print_endline (hex b)\n");
    let dir = std::env::temp_dir().join(format!("idlocaml_kh_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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

fn hex_of(p: std::path::PathBuf) -> String {
    std::fs::read(&p)
        .unwrap_or_else(|_| panic!("read {}", p.display()))
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Decode roundtrip: `marshal(unmarshal(golden)) == golden` for LE and BE.
/// Proves the generated `unmarshal` is the exact inverse of `marshal`.
fn run_roundtrip(idl: &str, module: &str, le_file: &str, be_file: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP roundtrip {module}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP roundtrip {module}: `ocamlfind` not on PATH");
        return;
    }
    let le = hex_of(Path::new(&golden_dir).join(le_file));
    let be = hex_of(Path::new(&golden_dir).join(be_file));
    let mut src = emit(idl);
    src.push_str(&format!(
        "\nlet () ={OC_HEX}  let from_hex h =\n    let n = String.length h / 2 in\n    let b = Bytes.create n in\n    for i = 0 to n - 1 do\n      Bytes.set b i (Char.chr (int_of_string (\"0x\" ^ String.sub h (i * 2) 2)))\n    done;\n    b\n  in\n  print_endline (hex ({module}.marshal ({module}.unmarshal (from_hex \"{le}\") Wire.LE) Wire.LE));\n  print_endline (hex ({module}.marshal ({module}.unmarshal (from_hex \"{be}\") Wire.BE) Wire.BE))\n"
    ));
    let dir = std::env::temp_dir().join(format!("idlocaml_rt_{module}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP keyhash_md5: `ocamlfind` not on PATH");
        return;
    }
    let mut src = emit(KEYHASH_MD5_IDL);
    src.push_str("\nlet () =\n  let k : KL.t = { KL.a = 0x01020304; b = 0x05060708; c = 0x090A0B0C; d = 0x0D0E0F10; e = 0x11121314 } in\n  let b = KL.key_hash k in\n  let hex bb = String.concat \"\" (List.init (Bytes.length bb) (fun i -> Printf.sprintf \"%02x\" (Char.code (Bytes.get bb i)))) in\n  print_endline (hex b)\n");
    let dir = std::env::temp_dir().join(format!("idlocaml_kh_md5_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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

// --- Regression: nested-struct `@key` member must expand to only the inner
// struct's own `@key` members (XTypes 1.3 §7.6.8), not its full member set.
// Before the fix, `Outer`'s `key_hash` called `Inner.marshal_into`, which
// serializes `x`, `ignored`, AND `y` — leaking the non-key `ignored` field
// into the KeyHash.

const NESTED_KEY_SUBSET_IDL: &str = "\
@final struct Inner { @key long x; long ignored; @key long y; };
@final struct Outer { @key Inner i; };";

#[test]
fn nested_key_member_excludes_non_key_fields() {
    let o = emit(NESTED_KEY_SUBSET_IDL);
    // `Inner` also has its own `@key` members (x, y), so it emits its own
    // `key_hash` function too — take the LAST occurrence (Outer's, since
    // Outer is emitted after Inner) rather than the first.
    let key_hash = o
        .rsplit("let key_hash (v : t) : bytes =")
        .next()
        .expect("Outer module should emit a key_hash function");
    // Bug A fix: the inner struct's own `@key` fields x and y are written
    // directly (the marshal never calls `Inner.marshal_into`, which would
    // pull in `ignored` too).
    assert!(
        !key_hash.contains("Inner.marshal_into"),
        "key_hash must not call the nested struct's full marshal_into: {o}"
    );
    assert!(
        key_hash.contains("v.i.x") && key_hash.contains("v.i.y"),
        "key_hash must encode the nested struct's own @key fields x and y: {o}"
    );
    assert!(
        !key_hash.contains("v.i.ignored"),
        "key_hash must NOT encode the nested struct's non-key field `ignored`: {o}"
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
    // `Inner` also emits its own trivial `key_hash` (single `@key octet`);
    // take the LAST occurrence (Outer's) rather than the first.
    let key_hash = o
        .rsplit("let key_hash (v : t) : bytes =")
        .next()
        .expect("Outer module should emit a key_hash function");
    // Inner has a single @key octet -> KeyHolder max size 1 byte <= 16 ->
    // zero-pad branch (Bytes.make 16 + blit), NOT the MD5 branch.
    assert!(
        key_hash.contains("Bytes.make 16"),
        "a 1-byte nested-struct key must take the zero-pad branch: {o}"
    );
    assert!(
        !key_hash.contains("Digest.bytes"),
        "a 1-byte nested-struct key must NOT take the MD5 branch: {o}"
    );
    assert!(key_hash.contains("v.i.a"), "{o}");
}

// ---- reserved-keyword escaping (Welle C.2 #14): an OCaml keyword colliding
// with a lower-cased enum type name or a record field label must be escaped
// (trailing `_`), not emitted bare. Enumerator constructors are left out of
// this fixture: they are uppercase-initial (Type -> A|B|C) by construction
// and so can never collide with a lowercase-only OCaml keyword, sidestepping
// this backend's separate, pre-existing constructor-casing behavior (out of
// scope for keyword escaping).
const KEYWORD_IDL: &str = "\
enum Type { A, B, C };
@final struct Container {
    uint32 and;
    Type mod;
};
@final struct Outer {
    Container nested;
    sequence<Container> items;
};";

#[test]
fn keyword_identifiers_are_escaped_not_bare() {
    let o = emit(KEYWORD_IDL);
    // Enum type name `Type` lower-cases to the keyword `type` -> escaped.
    assert!(o.contains("type type_ = A | B | C"), "{o}");
    assert!(
        o.contains("let type__to_int = function A -> 0 | B -> 1 | C -> 2"),
        "{o}"
    );
    assert!(
        o.contains("let type__of_int = function 0 -> A | 1 -> B | 2 -> C | _ -> A"),
        "{o}"
    );
    // Record field labels `and` / `mod` are OCaml keywords -> escaped.
    assert!(o.contains("and_ : int;"), "{o}");
    assert!(o.contains("mod_ : type_;"), "{o}");
    assert!(o.contains("Wire.put_u32 w v.and_;"), "{o}");
    assert!(o.contains("Wire.put_u32 w (type__to_int v.mod_);"), "{o}");
    assert!(o.contains("let and_ = (Wire.get_u32 r) in"), "{o}");
    assert!(
        o.contains("let mod_ = (type__of_int (Wire.get_u32 r)) in"),
        "{o}"
    );
    assert!(o.contains("{ and_; mod_ }"), "{o}");
    // Cross-module references (Outer -> Container) are unaffected: struct
    // module names are uppercase-forced (module_name) and can never collide
    // with a lowercase OCaml keyword.
    assert!(o.contains("nested : Container.t;"), "{o}");
    assert!(o.contains("items : Container.t list;"), "{o}");
    assert!(
        o.contains("Container.marshal_into v.nested w endian;"),
        "{o}"
    );
    assert!(o.contains("let nested = (Container.read r) in"), "{o}");
    // No bare `type`/`and`/`mod` identifier position remains.
    assert!(!o.contains("type type ="), "{o}");
    assert!(!o.contains("and : int;"), "{o}");
    assert!(!o.contains("mod : type"), "{o}");
    assert!(!o.contains("v.and;"), "{o}");
    assert!(!o.contains("v.mod)"), "{o}");
}

#[test]
fn keyword_identifiers_compile_with_ocamlfind() {
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!(
            "SKIP keyword ocamlfind compile-check: `ocamlfind` not on PATH (pending central serial validation on codepit)"
        );
        return;
    }
    let src = emit(KEYWORD_IDL);
    let dir = std::env::temp_dir().join(format!("idlocaml_kw_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt rejected keyword-escaped output:\n{}\n--- src ---\n{src}",
        String::from_utf8_lossy(&build.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ===========================================================================
// Section-F Wave-1: bitset/bitmask, @optional, fixed<d,s>, sequence-arbitrary,
// @verbatim. Always-on source-asserts + `ocamlfind`-gated compile-and-run
// tests whose expected wire hex is derived from the spec IN the test (no
// GOLDEN_DIR oracle — mirrors the idl-d reference).
// ===========================================================================

/// A top-level `zdhex : bytes -> string` helper, appended before a test's
/// `let () = ...` so main bodies can hex-dump wire output.
const OC_HEX_TOP: &str = "\nlet zdhex (b : bytes) : string =\n  String.concat \"\" (List.init (Bytes.length b) (fun i -> Printf.sprintf \"%02x\" (Char.code (Bytes.get b i))))\n";

/// Compiles `emit(idl) + OC_HEX_TOP + main_body` with `ocamlfind ocamlopt`,
/// runs it, and returns the trimmed stdout lines. `None` (skip) if `ocamlfind`
/// is not on PATH.
fn ocaml_lines(idl: &str, main_body: &str, tag: &str) -> Option<Vec<String>> {
    if Command::new("ocamlfind").arg("printconf").output().is_err() {
        eprintln!("SKIP {tag}: `ocamlfind` not on PATH");
        return None;
    }
    let mut src = emit(idl);
    src.push_str(OC_HEX_TOP);
    src.push_str(main_body);
    let dir = std::env::temp_dir().join(format!("idlocaml_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("main.ml"), &src).expect("write");
    let build = Command::new("ocamlfind")
        .args(["ocamlopt", "main.ml", "-o", "main_bin"])
        .current_dir(&dir)
        .output()
        .expect("ocamlfind");
    assert!(
        build.status.success(),
        "ocamlfind ocamlopt failed:\n{}\n--- src ---\n{src}",
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
fn bitset_emits_holder_module_and_accessors() {
    let o = emit(BITSET_IDL);
    // 4 + 8 = 12 bits → int backing via put_u16 (XTypes §7.4.7).
    assert!(o.contains("module Flags = struct"), "{o}");
    assert!(o.contains("mutable storage : int"), "{o}");
    assert!(
        o.contains("let a (v : t) : int = (v.storage lsr 0) land 15"),
        "{o}"
    );
    assert!(
        o.contains("let b (v : t) : int = (v.storage lsr 4) land 255"),
        "{o}"
    );
    assert!(o.contains("Wire.put_u16 w v.storage"), "{o}");
    assert!(o.contains("{ storage = Wire.get_u16 r }"), "{o}");
}

#[test]
fn bitset_wire_is_backing_int_and_accessors_read() {
    // storage 0xABCD → LE "cdab", BE "abcd"; a = 0xD (13), b = 0xBC (188).
    let body = "\nlet () =\n  let f = { Flags.storage = 0xABCD } in\n  print_endline (zdhex (Flags.marshal f Wire.LE));\n  print_endline (zdhex (Flags.marshal f Wire.BE));\n  print_endline (zdhex (Flags.marshal (Flags.unmarshal (Flags.marshal f Wire.LE) Wire.LE) Wire.LE));\n  print_endline (string_of_int (Flags.a f));\n  print_endline (string_of_int (Flags.b f))\n";
    let Some(l) = ocaml_lines(BITSET_IDL, body, "bitset") else {
        return;
    };
    assert_eq!(l[0], "cdab", "LE");
    assert_eq!(l[1], "abcd", "BE");
    assert_eq!(l[2], "cdab", "round-trip");
    assert_eq!(l[3], "13", "accessor a");
    assert_eq!(l[4], "188", "accessor b");
}

const BITSET_WIDE_IDL: &str = "bitset Big { bitfield<40> x; };";

#[test]
fn bitset_over_32_bits_uses_int64_backing() {
    let o = emit(BITSET_WIDE_IDL);
    // 40 bits → int64 backing via put_u64 (XTypes §7.4.7).
    assert!(o.contains("mutable storage : int64"), "{o}");
    assert!(o.contains("Wire.put_u64 w v.storage"), "{o}");
    assert!(o.contains("let x (v : t) : int64 = Int64.logand"), "{o}");
}

#[test]
fn bitset_int64_backing_wire_and_accessor() {
    // storage 0x0102030405 (40-bit) → put_u64 = 8 LE bytes; accessor returns it.
    let body = "\nlet () =\n  let f = { Big.storage = 0x0102030405L } in\n  print_endline (zdhex (Big.marshal f Wire.LE));\n  print_endline (zdhex (Big.marshal (Big.unmarshal (Big.marshal f Wire.LE) Wire.LE) Wire.LE));\n  print_endline (Int64.to_string (Big.x f))\n";
    let Some(l) = ocaml_lines(BITSET_WIDE_IDL, body, "bitset64") else {
        return;
    };
    assert_eq!(l[0], "0504030201000000", "LE u64");
    assert_eq!(l[1], "0504030201000000", "round-trip");
    assert_eq!(l[2], "4328719365", "accessor x = 0x0102030405");
}

// ---- bitmask ------------------------------------------------------------

const BITMASK_IDL: &str = "bitmask Perms { PERM_READ, PERM_WRITE, PERM_EXEC };";

#[test]
fn bitmask_emits_holder_and_constants() {
    let o = emit(BITMASK_IDL);
    // Default @bit_bound = 32 → int backing via put_u32 (XTypes §7.3.1.2.1.1).
    assert!(o.contains("module Perms = struct"), "{o}");
    assert!(o.contains("mutable storage : int"), "{o}");
    // Manifest constants lower-cased (OCaml `let` names are lowercase-initial).
    assert!(o.contains("let perm_read : int = 1 lsl 0"), "{o}");
    assert!(o.contains("let perm_exec : int = 1 lsl 2"), "{o}");
    assert!(o.contains("Wire.put_u32 w v.storage"), "{o}");
}

#[test]
fn bitmask_wire_is_backing_uint32() {
    // perm_read | perm_exec = 0x05 → LE "05000000", BE "00000005"; round-trips.
    let body = "\nlet () =\n  let p = { Perms.storage = Perms.perm_read lor Perms.perm_exec } in\n  print_endline (zdhex (Perms.marshal p Wire.LE));\n  print_endline (zdhex (Perms.marshal p Wire.BE));\n  print_endline (zdhex (Perms.marshal (Perms.unmarshal (Perms.marshal p Wire.BE) Wire.BE) Wire.BE))\n";
    let Some(l) = ocaml_lines(BITMASK_IDL, body, "bitmask") else {
        return;
    };
    assert_eq!(l[0], "05000000", "LE");
    assert_eq!(l[1], "00000005", "BE");
    assert_eq!(l[2], "00000005", "round-trip");
}

#[test]
fn bitmask_bit_bound_narrows_backing() {
    let o = emit("@bit_bound(8) bitmask Small { A, B };");
    assert!(o.contains("Wire.put_u8 w v.storage"), "{o}");
    assert!(o.contains("{ storage = Wire.get_u8 r }"), "{o}");
}

// ---- fixed<d,s> ---------------------------------------------------------

const FIXED_IDL: &str = "@final struct HasFixed { fixed<5,2> price; };";

#[test]
fn fixed_emits_bcd_field_and_prelude() {
    let o = emit(FIXED_IDL);
    assert!(o.contains("price : bytes;"), "{o}");
    assert!(o.contains("Wire.put_bytes w v.price"), "{o}");
    assert!(o.contains("Wire.get_bytes_n r 3"), "{o}"); // (5+2)/2 = 3 octets
    assert!(o.contains("let zd_fixed_enc (s : string)"), "{o}");
}

#[test]
fn fixed_wire_is_packed_bcd() {
    // fixed<5,2> 123.45 → BCD "12 34 5c" (odd P, no pad, CORBA §9.3.2.7).
    // Raw BCD bytes: identical for LE and BE (no byte-swap, no length prefix).
    let body = "\nlet () =\n  let h = { HasFixed.price = zd_fixed_enc \"123.45\" 5 2 } in\n  print_endline (zdhex (HasFixed.marshal h Wire.LE));\n  print_endline (zdhex (HasFixed.marshal h Wire.BE));\n  print_endline (zdhex (HasFixed.marshal (HasFixed.unmarshal (HasFixed.marshal h Wire.LE) Wire.LE) Wire.LE))\n";
    let Some(l) = ocaml_lines(FIXED_IDL, body, "fixed") else {
        return;
    };
    assert_eq!(l[0], "12345c", "LE");
    assert_eq!(l[1], "12345c", "BE");
    assert_eq!(l[2], "12345c", "round-trip");
}

#[test]
fn fixed_even_p_keeps_msd() {
    // fixed<4,0> 1234 → BCD "01 23 4c" (leading pad nibble; even P keeps MSD).
    let idl = "@final struct HasFixed { fixed<4,0> price; };";
    let body = "\nlet () =\n  let h = { HasFixed.price = zd_fixed_enc \"1234\" 4 0 } in\n  print_endline (zdhex (HasFixed.marshal h Wire.LE))\n";
    let Some(l) = ocaml_lines(idl, body, "fixed40") else {
        return;
    };
    assert_eq!(l[0], "01234c");
}

// ---- sequence-arbitrary -------------------------------------------------

const SEQARB_IDL: &str = "@final struct S { sequence<long> xs; };";

#[test]
fn sequence_arbitrary_emits_count_and_loop() {
    let o = emit(SEQARB_IDL);
    assert!(o.contains("xs : int list;"), "{o}");
    assert!(o.contains("Wire.put_u32 w (List.length v.xs)"), "{o}");
    assert!(
        o.contains("List.iter (fun zdElem -> Wire.put_u32 w zdElem) v.xs"),
        "{o}"
    );
}

#[test]
fn sequence_arbitrary_wire_count_plus_elements() {
    // [0x01020304, 0x05060708] → u32 count 2 + two i32 elements, no DHEADER.
    let body = "\nlet () =\n  let s = { S.xs = [0x01020304; 0x05060708] } in\n  print_endline (zdhex (S.marshal s Wire.LE));\n  print_endline (zdhex (S.marshal s Wire.BE));\n  print_endline (zdhex (S.marshal (S.unmarshal (S.marshal s Wire.LE) Wire.LE) Wire.LE))\n";
    let Some(l) = ocaml_lines(SEQARB_IDL, body, "seqarb") else {
        return;
    };
    assert_eq!(l[0], "020000000403020108070605", "LE");
    assert_eq!(l[1], "000000020102030405060708", "BE");
    assert_eq!(l[2], "020000000403020108070605", "round-trip");
}

#[test]
fn sequence_of_enum_is_arbitrary_path() {
    // A `sequence<enum>` must now emit (was rejected pre-Wave-1), no DHEADER.
    let o = emit("enum E { E0, E1 }; @final struct SE { sequence<E> es; };");
    assert!(o.contains("es : e list;"), "{o}");
    assert!(o.contains("Wire.put_u32 w (List.length v.es)"), "{o}");
    assert!(
        o.contains("List.iter (fun zdElem -> Wire.put_u32 w (e_to_int zdElem)) v.es"),
        "{o}"
    );
}

// ---- @optional ----------------------------------------------------------

const OPT_IDL: &str = "@final struct Opt { uint32 a; @optional uint32 b; };";

#[test]
fn optional_emits_option_field_and_presence_flag() {
    let o = emit(OPT_IDL);
    assert!(o.contains("b : int option;"), "{o}");
    assert!(
        o.contains("(match v.b with Some zdOpt -> Wire.put_u8 w 1; Wire.put_u32 w zdOpt | None -> Wire.put_u8 w 0)"),
        "{o}"
    );
    assert!(
        o.contains("let b = (if Wire.get_bool r then Some ((Wire.get_u32 r)) else None) in"),
        "{o}"
    );
}

#[test]
fn optional_final_wire_present_and_absent() {
    // present: u32 a, u8 flag=1, pad(3), u32 b. absent: u32 a, u8 flag=0.
    let body = "\nlet () =\n  let p = { Opt.a = 0x11223344; b = Some 0xAABBCCDD } in\n  print_endline (zdhex (Opt.marshal p Wire.LE));\n  print_endline (zdhex (Opt.marshal p Wire.BE));\n  let q = { Opt.a = 0x11223344; b = None } in\n  print_endline (zdhex (Opt.marshal q Wire.LE));\n  print_endline (zdhex (Opt.marshal (Opt.unmarshal (Opt.marshal p Wire.LE) Wire.LE) Wire.LE));\n  print_endline (zdhex (Opt.marshal (Opt.unmarshal (Opt.marshal q Wire.LE) Wire.LE) Wire.LE))\n";
    let Some(l) = ocaml_lines(OPT_IDL, body, "opt") else {
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
    // both present and absent without hand-computing the DHEADER length.
    let idl = "@appendable struct OptA { uint32 a; @optional uint32 b; };";
    let body = "\nlet () =\n  let p = { OptA.a = 0xCAFEBABE; b = Some 0x01020304 } in\n  let pe = OptA.marshal p Wire.LE in\n  print_endline (zdhex pe);\n  print_endline (zdhex (OptA.marshal (OptA.unmarshal pe Wire.LE) Wire.LE));\n  let q = { OptA.a = 0xCAFEBABE; b = None } in\n  let qe = OptA.marshal q Wire.LE in\n  print_endline (zdhex qe);\n  print_endline (zdhex (OptA.marshal (OptA.unmarshal qe Wire.LE) Wire.LE))\n";
    let Some(l) = ocaml_lines(idl, body, "opta") else {
        return;
    };
    assert_eq!(l[0], l[1], "present round-trip");
    assert_eq!(l[2], l[3], "absent round-trip");
}

// ---- @verbatim ----------------------------------------------------------

#[test]
fn verbatim_placements_inject_text() {
    let o = emit(
        "@verbatim(language=\"ocaml\", placement=BEGIN_FILE, text=\"(* zd-begin-file *)\")\n\
         @verbatim(language=\"ocaml\", placement=BEFORE_DECLARATION, text=\"(* zd-before *)\")\n\
         @verbatim(language=\"ocaml\", placement=BEGIN_DECLARATION, text=\"(* zd-begin-decl *)\")\n\
         @verbatim(language=\"ocaml\", placement=END_DECLARATION, text=\"(* zd-end-decl *)\")\n\
         @verbatim(language=\"ocaml\", placement=AFTER_DECLARATION, text=\"(* zd-after *)\")\n\
         @verbatim(language=\"ocaml\", placement=END_FILE, text=\"(* zd-end-file *)\")\n\
         @final struct V { uint32 a; };",
    );
    for marker in [
        "(* zd-begin-file *)",
        "(* zd-before *)",
        "(* zd-begin-decl *)",
        "(* zd-end-decl *)",
        "(* zd-after *)",
        "(* zd-end-file *)",
    ] {
        assert!(o.contains(marker), "missing {marker}:\n{o}");
    }
    // Ordering: begin-file before the module; before-decl before `module V`;
    // begin-decl after the module open; end-file trails the type.
    let midx = o.find("module V = struct").expect("module");
    assert!(o.find("(* zd-begin-file *)").unwrap() < midx, "{o}");
    assert!(o.find("(* zd-before *)").unwrap() < midx, "{o}");
    assert!(o.find("(* zd-begin-decl *)").unwrap() > midx, "{o}");
    assert!(o.find("(* zd-end-file *)").unwrap() > midx, "{o}");
}

#[test]
fn verbatim_language_filter_excludes_other_langs() {
    // A non-OCaml language tag must NOT leak into the OCaml output.
    let o = emit(
        "@verbatim(language=\"java\", placement=BEFORE_DECLARATION, text=\"(* java-only *)\")\n\
         @final struct V { uint32 a; };",
    );
    assert!(!o.contains("(* java-only *)"), "{o}");
    // The wildcard `*` still matches OCaml.
    let o2 = emit(
        "@verbatim(placement=BEFORE_DECLARATION, text=\"(* wildcard *)\")\n\
         @final struct V { uint32 a; };",
    );
    assert!(o2.contains("(* wildcard *)"), "{o2}");
}

#[test]
fn verbatim_output_still_compiles() {
    let idl = "@verbatim(language=\"ocaml\", placement=BEGIN_FILE, text=\"(* zd file header *)\")\n\
         @verbatim(language=\"ocaml\", placement=BEGIN_DECLARATION, text=\"(* zd inside module *)\")\n\
         @final struct V { uint32 a; };";
    let body =
        "\nlet () =\n  let v = { V.a = 0x2A } in\n  print_endline (zdhex (V.marshal v Wire.LE))\n";
    let Some(l) = ocaml_lines(idl, body, "verbatim") else {
        return;
    };
    assert_eq!(l[0], "2a000000");
}
