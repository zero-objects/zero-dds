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
    assert!(o.contains("module Reading = struct"), "{o}");
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
    assert!(o.contains("module A = struct"), "{o}");
    assert!(o.contains("module B = struct"), "{o}");
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
