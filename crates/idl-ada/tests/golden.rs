// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Ada backend: string smoke tests (always) + a byte-identity test that
//! compiles+runs the generated Ada and compares to the Rust goldens (gated on
//! `gnatmake` being on PATH and `GOLDEN_DIR` pointing at golden_{le,be}.bin).

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
use zerodds_idl_ada::{AdaGenOptions, generate_ada_module};

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

fn emit(src: &str, opts: &AdaGenOptions) -> zerodds_idl_ada::AdaModule {
    let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
    generate_ada_module(&ast, opts).expect("gen")
}

/// swarm59 #21b: `module X { struct Y { ... }; }` used to be silently
/// dropped (no `Definition::Module` arm at all) — the struct must now emit.
#[test]
fn module_wrapped_struct_is_emitted_not_dropped() {
    let m = emit(
        "module Telemetry { @final struct Reading { long value; }; };",
        &AdaGenOptions::default(),
    );
    assert!(m.spec.contains("type Reading is record"), "{}", m.spec);
    assert!(m.spec.contains("value : Integer_32;"), "{}", m.spec);
}

/// A reopened module (`module M {} ... module M {}`) must not lose either
/// half's content once the AST builder merges the two occurrences.
#[test]
fn reopened_module_emits_both_structs() {
    let m = emit(
        "module M { @final struct A { long x; }; }; \
         module M { @final struct B { long y; }; };",
        &AdaGenOptions::default(),
    );
    assert!(m.spec.contains("type A is record"), "{}", m.spec);
    assert!(m.spec.contains("type B is record"), "{}", m.spec);
}

#[test]
fn final_struct_emits_record_and_marshal() {
    let m = emit(GOLDEN_IDL, &AdaGenOptions::default());
    assert!(m.spec.contains("package Zdgen is"), "{}", m.spec);
    assert!(m.spec.contains("type Golden is record"), "{}", m.spec);
    assert!(m.spec.contains("id : Unsigned_32;"), "{}", m.spec);
    assert!(m.spec.contains("kind : Unsigned_16;"), "{}", m.spec);
    assert!(m.spec.contains("flags : Unsigned_8;"), "{}", m.spec);
    assert!(m.spec.contains("value : IEEE_Float_32;"), "{}", m.spec);
    assert!(m.spec.contains("stamp : Unsigned_64;"), "{}", m.spec);
    assert!(m.spec.contains("label : Unbounded_String;"), "{}", m.spec);
    assert!(m.spec.contains("raw : Unbounded_String;"), "{}", m.spec);
    assert!(
        m.spec.contains("function Marshal (V : Golden;"),
        "{}",
        m.spec
    );
    assert!(m.body.contains("Put_U32 (W, V.id);"), "{}", m.body);
    assert!(m.body.contains("Put_F32 (W, V.value);"), "{}", m.body);
    assert!(
        m.body.contains("Put_String (W, To_String (V.label));"),
        "{}",
        m.body
    );
    assert!(
        m.body.contains("Put_Seq_U8 (W, To_String (V.raw));"),
        "{}",
        m.body
    );
    // @final → no DHEADER body framing.
    assert!(!m.body.contains("Append (W, B);"), "{}", m.body);
}

/// #23-shape regression: a struct with neither a `sequence<struct>` member nor
/// a map member must not pull in `Ada.Containers.Vectors` / `Ordered_Maps` (or
/// any per-struct `_Vectors` instantiation) — mirrors the go/nim usage-gating
/// for `sort`/`crypto/md5`/`std/tables`. `GOLDEN_IDL`'s only collection field is
/// `sequence<octet>`, which maps to `Unbounded_String`, not a `Vectors.Vector`.
#[test]
fn plain_struct_omits_unused_containers_deps() {
    let m = emit(GOLDEN_IDL, &AdaGenOptions::default());
    assert!(
        !m.spec.contains("with Ada.Containers.Vectors;"),
        "plain struct pulled in Ada.Containers.Vectors unused:\n{}",
        m.spec
    );
    assert!(
        !m.spec.contains("with Ada.Containers.Ordered_Maps;"),
        "plain struct pulled in Ada.Containers.Ordered_Maps unused:\n{}",
        m.spec
    );
    assert!(
        !m.spec.contains("_Vectors is new Ada.Containers.Vectors"),
        "plain struct got a speculative _Vectors instantiation:\n{}",
        m.spec
    );
    assert!(
        !m.body.contains("with Ada.Containers.Ordered_Maps;"),
        "plain struct's body pulled in Ada.Containers.Ordered_Maps unused:\n{}",
        m.body
    );
}

#[test]
fn appendable_struct_frames_a_dheader() {
    let m = emit(
        "@appendable struct S { uint32 a; };",
        &AdaGenOptions::default(),
    );
    assert!(
        m.body.contains("Put_U32 (W, Unsigned_32 (B.Len));"),
        "{}",
        m.body
    );
    assert!(m.body.contains("Append (W, B);"), "{}", m.body);
}

// `Package`/`Body`/`Range`/`Loop` are Ada 2012 reserved words but legal IDL
// identifiers (not reserved by the IDL grammar). Before the #14 fix this
// emitted `type Package is record Body : ...; Range : ...;` — invalid Ada,
// and — being case-insensitive — `package`/`Package`/`PACKAGE` all collide.
const ADA_KEYWORD_IDL: &str = "\
@final struct Package {
    uint32 body;
    uint32 range;
};
enum Loop { LOOP_A, LOOP_B };
@final struct Holder {
    Loop kind;
    sequence<Package> items;
};";

#[test]
fn keyword_identifiers_are_escaped_with_id_suffix() {
    let m = emit(ADA_KEYWORD_IDL, &AdaGenOptions::default());
    assert!(m.spec.contains("type Package_Id is record"), "{}", m.spec);
    assert!(!m.spec.contains("type Package is record"), "{}", m.spec);
    assert!(m.spec.contains("body_Id : Unsigned_32;"), "{}", m.spec);
    assert!(m.spec.contains("range_Id : Unsigned_32;"), "{}", m.spec);
    assert!(m.spec.contains("type Loop_Id is ("), "{}", m.spec);
    // A scoped reference (sequence<Package>, and the `Loop` enum field) must
    // reuse the exact same escaped name as the declaration site.
    assert!(m.spec.contains("Package_Id_Vectors"), "{}", m.spec);
    assert!(m.spec.contains("kind : Loop_Id;"), "{}", m.spec);
    assert!(m.body.contains("Loop_Id_To_U32"), "{}", m.body);
    assert!(m.body.contains("Loop_Id_Of_U32"), "{}", m.body);
    // No bare Ada reserved word survives as a standalone declared token
    // (case-insensitively — Ada identifiers are case-insensitive).
    for kw in ["package", "body", "range", "loop"] {
        for hay in [&m.spec, &m.body] {
            let lower = hay.to_ascii_lowercase();
            assert!(
                !lower.contains(&format!(" {kw} :")) && !lower.contains(&format!(" {kw} is")),
                "unescaped Ada keyword `{kw}` leaked into output:\n{hay}"
            );
        }
    }
}

const ENUM_IDL: &str = "\
enum Mode { MODE_IDLE, MODE_ACTIVE, MODE_FAULT };
@final struct S { Mode kind; uint32 tail; };";

#[test]
fn enum_emits_type_and_member_marshals() {
    let m = emit(ENUM_IDL, &AdaGenOptions::default());
    assert!(
        m.spec
            .contains("type Mode is (MODE_IDLE, MODE_ACTIVE, MODE_FAULT);"),
        "{}",
        m.spec
    );
    assert!(m.spec.contains("kind : Mode;"), "{}", m.spec);
    assert!(
        m.body
            .contains("function Mode_To_U32 (V : Mode) return Unsigned_32 is"),
        "{}",
        m.body
    );
    assert!(
        m.body.contains("when MODE_FAULT => return 2;"),
        "{}",
        m.body
    );
    // An enum member is a 32-bit signed integer on the wire (XTypes §7.4.5.1).
    assert!(
        m.body.contains("Put_U32 (W, Mode_To_U32 (V.kind));"),
        "{}",
        m.body
    );
}

const ENUM_MAIN_ADB: &str = r#"with Enum_Gen; use Enum_Gen;
with Ada.Strings.Unbounded; use Ada.Strings.Unbounded;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   S : constant Enum_Gen.S := (Kind => MODE_FAULT, Tail => 16#DEADBEEF#);
begin
   Put_Line (Hex (Marshal (S, Little)));
end Main;
"#;

#[test]
fn enum_member_is_byte_identical_i32() {
    // Gated: needs gnatmake. S{ kind: MODE_FAULT(=2), tail: 0xDEADBEEF }
    // -> i32 LE 02000000 + u32 LE efbeadde.
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP enum byte test: `gnatmake` not on PATH");
        return;
    }
    let m = emit(
        ENUM_IDL,
        &AdaGenOptions {
            package_name: "Enum_Gen".to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_enum_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("enum_gen.ads"), &m.spec).expect("write ads");
    std::fs::write(dir.join("enum_gen.adb"), &m.body).expect("write adb");
    std::fs::write(dir.join("main.adb"), ENUM_MAIN_ADB).expect("write main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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

const NESTED_MAIN_ADB: &str = r#"with Nested_Gen; use Nested_Gen;
with Ada.Strings.Unbounded; use Ada.Strings.Unbounded;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   M : Inner_Vectors.Vector;
   O : Outer;
begin
   M.Append ((A => 16#AAAA#, B => 16#BBBBCCCC#));
   M.Append ((A => 16#DDDD#, B => 16#EEEEFFFF#));
   O := (Id => 16#CAFEBABE#,
         One => (A => 16#1111#, B => 16#22223333#),
         Many => M,
         Label => To_Unbounded_String ("nested"));
   Put_Line (Hex (Marshal (O, Little)));
   Put_Line (Hex (Marshal (O, Big)));
end Main;
"#;

#[test]
fn nested_struct_emits_marshal_into() {
    let m = emit(NESTED_IDL, &AdaGenOptions::default());
    assert!(
        m.spec.contains("with Ada.Containers.Vectors;"),
        "sequence<struct> member must still pull in Ada.Containers.Vectors:\n{}",
        m.spec
    );
    assert!(
        m.spec
            .contains("package Inner_Vectors is new Ada.Containers.Vectors (Natural, Inner);"),
        "{}",
        m.spec
    );
    assert!(
        m.spec.contains("many : Inner_Vectors.Vector;"),
        "{}",
        m.spec
    );
    assert!(
        m.body
            .contains("procedure Marshal_Into (V : Inner; W : in out Buf_T) is"),
        "{}",
        m.body
    );
    assert!(m.body.contains("Marshal_Into (V.one, B);"), "{}", m.body);
    assert!(m.body.contains("Marshal_Into (E, Sub);"), "{}", m.body);
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
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP nested byte: `gnatmake` not on PATH");
        return;
    }
    let m = emit(
        NESTED_IDL,
        &AdaGenOptions {
            package_name: "Nested_Gen".to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_nested_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("nested_gen.ads"), &m.spec).expect("write ads");
    std::fs::write(dir.join("nested_gen.adb"), &m.body).expect("write adb");
    std::fs::write(dir.join("main.adb"), NESTED_MAIN_ADB).expect("write main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP byte_identity: `gnatmake` not on PATH");
        return;
    }

    let m = emit(
        GOLDEN_IDL,
        &AdaGenOptions {
            package_name: "Golden_Gen".to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_golden_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("golden_gen.ads"), &m.spec).expect("write ads");
    std::fs::write(dir.join("golden_gen.adb"), &m.body).expect("write adb");
    std::fs::write(dir.join("main.adb"), MAIN_ADB).expect("write main");

    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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

const TYPEDEF_MAIN_ADB: &str = r#"with Typedef_Gen; use Typedef_Gen;
with Ada.Strings.Unbounded; use Ada.Strings.Unbounded;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   R : Rec;
begin
   R := (Id => 16#CAFEBABE#,
         Name => To_Unbounded_String ("typedef"),
         Data => To_Unbounded_String
                   (Character'Val (1) & Character'Val (2) & Character'Val (3)));
   Put_Line (Hex (Marshal (R, Little)));
   Put_Line (Hex (Marshal (R, Big)));
end Main;
"#;

#[test]
fn typedef_resolves_to_underlying_type() {
    let m = emit(TYPEDEF_IDL, &AdaGenOptions::default());
    assert!(m.spec.contains("id : Unsigned_32;"), "{}", m.spec);
    assert!(m.spec.contains("name : Unbounded_String;"), "{}", m.spec);
    assert!(m.spec.contains("data : Unbounded_String;"), "{}", m.spec);
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
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP typedef byte: `gnatmake` not on PATH");
        return;
    }
    let m = emit(
        TYPEDEF_IDL,
        &AdaGenOptions {
            package_name: "Typedef_Gen".to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_typedef_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("typedef_gen.ads"), &m.spec).expect("write ads");
    std::fs::write(dir.join("typedef_gen.adb"), &m.body).expect("write adb");
    std::fs::write(dir.join("main.adb"), TYPEDEF_MAIN_ADB).expect("write main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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

const ARRAY_MAIN_ADB: &str = r#"with Array_Gen; use Array_Gen;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   A : Arr;
begin
   A := (Xs => (16#11111111#, 16#22222222#, 16#33333333#),
         M  => ((16#0102#, 16#0304#), (16#0506#, 16#0708#)),
         Bs => (16#AA#, 16#BB#, 16#CC#, 16#DD#));
   Put_Line (Hex (Marshal (A, Little)));
   Put_Line (Hex (Marshal (A, Big)));
end Main;
"#;

#[test]
fn array_emits_named_array_types() {
    let m = emit(ARRAY_IDL, &AdaGenOptions::default());
    assert!(
        m.spec
            .contains("type Arr_xs_A is array (0 .. 2) of Integer_32;"),
        "{}",
        m.spec
    );
    assert!(
        m.spec
            .contains("type Arr_m_A is array (0 .. 1, 0 .. 1) of Integer_16;"),
        "{}",
        m.spec
    );
    assert!(
        m.spec
            .contains("type Arr_bs_A is array (0 .. 3) of Unsigned_8;"),
        "{}",
        m.spec
    );
    assert!(m.body.contains("for i0 in 0 .. 2 loop"), "{}", m.body);
    assert!(m.body.contains("for i1 in 0 .. 1 loop"), "{}", m.body);
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
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP array byte: `gnatmake` not on PATH");
        return;
    }
    let m = emit(
        ARRAY_IDL,
        &AdaGenOptions {
            package_name: "Array_Gen".to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_array_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("array_gen.ads"), &m.spec).expect("write ads");
    std::fs::write(dir.join("array_gen.adb"), &m.body).expect("write adb");
    std::fs::write(dir.join("main.adb"), ARRAY_MAIN_ADB).expect("write main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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

const ARRAY_OF_SEQ_IDL: &str = "\
@final struct Elem { unsigned long x; };
@final struct Holder { sequence<Elem> items[3]; };";

/// #23-shape regression: `sequence<Struct> f[N]` — a `sequence<struct>` member
/// combined with an array declarator (not a plain `sequence<struct>` member,
/// which `nested_struct_emits_marshal_into` already covers). `map_type` on the
/// array's resolved (sequence) type returns `elem_type = "Elem_Vectors.Vector"`,
/// which is embedded into the named array type
/// (`type Holder_items_A is array (0 .. 2) of Elem_Vectors.Vector;`) — but
/// `FieldGen.ada_type` only sees the synthetic array type *name*
/// (`Holder_items_A`), not the element type string. The `vectors_used` gate
/// must therefore also pick this up (via `StructGen.array_vector_elems`), or
/// the generated code references an undefined `Elem_Vectors` package with no
/// `with Ada.Containers.Vectors;` and no instantiation (this compiled fine
/// before the usage-gate was added).
#[test]
fn array_of_sequence_of_struct_pulls_in_vectors() {
    let m = emit(ARRAY_OF_SEQ_IDL, &AdaGenOptions::default());
    assert!(
        m.spec.contains("with Ada.Containers.Vectors;"),
        "sequence<struct> array element must pull in Ada.Containers.Vectors:\n{}",
        m.spec
    );
    assert!(
        m.spec
            .contains("package Elem_Vectors is new Ada.Containers.Vectors (Natural, Elem);"),
        "{}",
        m.spec
    );
    assert!(
        m.spec
            .contains("type Holder_items_A is array (0 .. 2) of Elem_Vectors.Vector;"),
        "{}",
        m.spec
    );
    assert!(m.spec.contains("items : Holder_items_A;"), "{}", m.spec);
}

const UNION_IDL: &str = "\
@final union U switch (long) { case 1: unsigned long a; case 2: unsigned short b; default: octet c; };";

const UNION_MAIN_ADB: &str = r#"with Union_Gen; use Union_Gen;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   V : U;
begin
   V := (disc => 2, a => 0, b => 16#1234#, c => 0);
   Put_Line (Hex (Marshal (V, Little)));
   Put_Line (Hex (Marshal (V, Big)));
end Main;
"#;

#[test]
fn union_emits_case_dispatch() {
    let m = emit(UNION_IDL, &AdaGenOptions::default());
    assert!(m.spec.contains("disc : Integer_32;"), "{}", m.spec);
    assert!(m.body.contains("case V.disc is"), "{}", m.body);
    assert!(m.body.contains("when 1 =>"), "{}", m.body);
    assert!(m.body.contains("when 2 =>"), "{}", m.body);
    assert!(m.body.contains("when others =>"), "{}", m.body);
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
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP union byte: `gnatmake` not on PATH");
        return;
    }
    let m = emit(
        UNION_IDL,
        &AdaGenOptions {
            package_name: "Union_Gen".to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_union_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("union_gen.ads"), &m.spec).expect("write ads");
    std::fs::write(dir.join("union_gen.adb"), &m.body).expect("write adb");
    std::fs::write(dir.join("main.adb"), UNION_MAIN_ADB).expect("write main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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

const MAP_MAIN_ADB: &str = r#"with Map_Gen; use Map_Gen;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   H : HasMap;
begin
   HasMap_m_Map.Insert (H.m, 1, 16#11111111#);
   HasMap_m_Map.Insert (H.m, 2, 16#22222222#);
   Put_Line (Hex (Marshal (H, Little)));
   Put_Line (Hex (Marshal (H, Big)));
end Main;
"#;

#[test]
fn map_emits_ordered_map() {
    let m = emit(MAP_IDL, &AdaGenOptions::default());
    assert!(
        m.spec.contains("with Ada.Containers.Ordered_Maps;"),
        "map member must still pull in Ada.Containers.Ordered_Maps (spec):\n{}",
        m.spec
    );
    assert!(
        m.body.contains("with Ada.Containers.Ordered_Maps;"),
        "map member must still pull in Ada.Containers.Ordered_Maps (body):\n{}",
        m.body
    );
    assert!(
        m.spec.contains(
            "package HasMap_m_Map is new Ada.Containers.Ordered_Maps (Integer_32, Unsigned_32);"
        ),
        "{}",
        m.spec
    );
    assert!(m.spec.contains("m : HasMap_m_Map.Map;"), "{}", m.spec);
    assert!(
        m.body.contains("HasMap_m_Map.Has_Element (C)"),
        "{}",
        m.body
    );
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
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP map byte: `gnatmake` not on PATH");
        return;
    }
    let m = emit(
        MAP_IDL,
        &AdaGenOptions {
            package_name: "Map_Gen".to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_map_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("map_gen.ads"), &m.spec).expect("write ads");
    std::fs::write(dir.join("map_gen.adb"), &m.body).expect("write adb");
    std::fs::write(dir.join("main.adb"), MAP_MAIN_ADB).expect("write main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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

const MUTABLE_MAIN_ADB: &str = r#"with Mutable_Gen; use Mutable_Gen;
with Ada.Strings.Unbounded; use Ada.Strings.Unbounded;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   V : M;
begin
   V := (x => 16#DEADBEEF#, s => To_Unbounded_String ("mut"), k => 16#0777#);
   Put_Line (Hex (Marshal (V, Little)));
   Put_Line (Hex (Marshal (V, Big)));
end Main;
"#;

#[test]
fn mutable_emits_emheader_framing() {
    let m = emit(MUTABLE_IDL, &AdaGenOptions::default());
    assert!(m.body.contains("Put_U32 (B, 16#4000000A#);"), "{}", m.body);
    assert!(m.body.contains("Put_U32 (B, 16#40000014#);"), "{}", m.body);
    assert!(m.body.contains("Put_U32 (B, 16#4000001E#);"), "{}", m.body);
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
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP mutable byte: `gnatmake` not on PATH");
        return;
    }
    let m = emit(
        MUTABLE_IDL,
        &AdaGenOptions {
            package_name: "Mutable_Gen".to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_mutable_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("mutable_gen.ads"), &m.spec).expect("write ads");
    std::fs::write(dir.join("mutable_gen.adb"), &m.body).expect("write adb");
    std::fs::write(dir.join("main.adb"), MUTABLE_MAIN_ADB).expect("write main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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

const HEX_FN: &str = r#"
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
"#;

fn run_ada(idl: &str, pkg: &str, main_adb: &str, stem: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP {stem}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP {stem}: `gnatmake` not on PATH");
        return;
    }
    let m = emit(
        idl,
        &AdaGenOptions {
            package_name: pkg.to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_{stem}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join(format!("{}.ads", pkg.to_lowercase())), &m.spec).expect("ads");
    std::fs::write(dir.join(format!("{}.adb", pkg.to_lowercase())), &m.body).expect("adb");
    std::fs::write(dir.join("main.adb"), main_adb).expect("main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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

#[test]
fn wide_is_byte_identical_vs_rust_golden() {
    let main = format!(
        "with Wide_Gen; use Wide_Gen;\nwith Ada.Strings.Unbounded; use Ada.Strings.Unbounded;\nwith Ada.Text_IO; use Ada.Text_IO;\nwith Interfaces; use Interfaces;\nprocedure Main is{HEX_FN}   V : W;\nbegin\n   V := (c => 16#03A9#, s => To_Unbounded_String (\"w\" & Character'Val (16#CF#) & Character'Val (16#80#)));\n   Put_Line (Hex (Marshal (V, Little)));\n   Put_Line (Hex (Marshal (V, Big)));\nend Main;\n"
    );
    run_ada(WIDE_IDL, "Wide_Gen", &main, "wide");
}

#[test]
fn longdouble_is_byte_identical_vs_rust_golden() {
    let main = format!(
        "with Ld_Gen; use Ld_Gen;\nwith Ada.Text_IO; use Ada.Text_IO;\nwith Interfaces; use Interfaces;\nprocedure Main is{HEX_FN}   V : L;\nbegin\n   V := (d => 1.1);\n   Put_Line (Hex (Marshal (V, Little)));\n   Put_Line (Hex (Marshal (V, Big)));\nend Main;\n"
    );
    run_ada(LD_IDL, "Ld_Gen", &main, "longdouble");
}

const NESTED_KEY_IDL: &str = "\
@final struct Inner { @key long x; long ignored; @key long y; };\n\
@final struct Outer { @key Inner i; };";

/// Bug A regression: a `@key` member whose type is itself a struct must
/// expand to ONLY that struct's own `@key` members (x, y), not its full
/// member set — `ignored` must not appear in the Key_Hash body.
#[test]
fn nested_struct_key_excludes_non_key_fields() {
    let m = emit(NESTED_KEY_IDL, &AdaGenOptions::default());
    let start = m
        .body
        .find("function Key_Hash (V : Outer)")
        .expect("Key_Hash function");
    let end = m.body[start..]
        .find("end Key_Hash;")
        .map(|i| start + i)
        .unwrap_or(m.body.len());
    let body = &m.body[start..end];
    assert!(body.contains("V.i.x"), "{body}");
    assert!(body.contains("V.i.y"), "{body}");
    assert!(!body.contains("V.i.ignored"), "{body}");
    // The full-marshal call must not be reused for the key encoding.
    assert!(!body.contains("Marshal_Into (V.i, KW);"), "{body}");
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
    let m = emit(NESTED_KEY_SMALL_IDL, &AdaGenOptions::default());
    let start = m
        .body
        .find("function Key_Hash (V : Outer)")
        .expect("Key_Hash function");
    let end = m.body[start..]
        .find("end Key_Hash;")
        .map(|i| start + i)
        .unwrap_or(m.body.len());
    let body = &m.body[start..end];
    assert!(body.contains("Outk (I) := KW.Data (I);"), "{body}");
    assert!(!body.contains("GNAT.MD5"), "{body}");
    assert!(!body.contains("MD5.Digest"), "{body}");
}

const KEYHASH_IDL: &str = "\
@final struct K { @key long a; @key unsigned short b; long c; };";

const KEYHASH_MAIN_ADB: &str = r#"with Kh_Gen; use Kh_Gen;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   V : K;
begin
   V := (a => 16#01020304#, b => 16#0506#, c => 0);
   Put_Line (Hex (Key_Hash (V)));
end Main;
"#;

#[test]
fn keyhash_is_byte_identical_vs_rust_golden() {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP keyhash: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash: `gnatmake` not on PATH");
        return;
    }
    let m = emit(
        KEYHASH_IDL,
        &AdaGenOptions {
            package_name: "Kh_Gen".to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_kh_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("kh_gen.ads"), &m.spec).expect("ads");
    std::fs::write(dir.join("kh_gen.adb"), &m.body).expect("adb");
    std::fs::write(dir.join("main.adb"), KEYHASH_MAIN_ADB).expect("main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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

/// Decode roundtrip: `Marshal (Unmarshal (golden)) = golden` for LE and BE.
/// Proves the generated `Unmarshal` is the exact inverse of `Marshal`.
fn run_roundtrip(idl: &str, type_name: &str, le_file: &str, be_file: &str) {
    let golden_dir = match std::env::var("GOLDEN_DIR") {
        Ok(d) => d,
        Err(_) => {
            eprintln!("SKIP roundtrip {type_name}: GOLDEN_DIR unset");
            return;
        }
    };
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP roundtrip {type_name}: `gnatmake` not on PATH");
        return;
    }
    let le = hex_of(Path::new(&golden_dir).join(le_file));
    let be = hex_of(Path::new(&golden_dir).join(be_file));
    let m = emit(
        idl,
        &AdaGenOptions {
            package_name: "Rt_Gen".to_string(),
        },
    );
    let main = format!(
        r#"with Rt_Gen; use Rt_Gen;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   function Nib (C : Character) return Natural is
   begin
      case C is
         when '0' .. '9' => return Character'Pos (C) - Character'Pos ('0');
         when 'a' .. 'f' => return Character'Pos (C) - Character'Pos ('a') + 10;
         when others => return 0;
      end case;
   end Nib;
   function From_Hex (H : String) return Byte_Array is
      R : Byte_Array (0 .. H'Length / 2 - 1);
   begin
      for I in R'Range loop
         R (I) := Byte (Nib (H (H'First + I * 2)) * 16 + Nib (H (H'First + I * 2 + 1)));
      end loop;
      return R;
   end From_Hex;
   Gle : constant Byte_Array := From_Hex ("{le}");
   Gbe : constant Byte_Array := From_Hex ("{be}");
   Vle : constant {type_name} := Unmarshal (Gle, Little);
   Vbe : constant {type_name} := Unmarshal (Gbe, Big);
begin
   Put_Line (Hex (Marshal (Vle, Little)));
   Put_Line (Hex (Marshal (Vbe, Big)));
end Main;
"#
    );
    let dir = std::env::temp_dir().join(format!("idlada_rt_{type_name}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("rt_gen.ads"), &m.spec).expect("write ads");
    std::fs::write(dir.join("rt_gen.adb"), &m.body).expect("write adb");
    std::fs::write(dir.join("main.adb"), &main).expect("write main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}\n--- main ---\n{main}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
        .current_dir(&dir)
        .output()
        .expect("run");
    let stdout = String::from_utf8(run.stdout).expect("utf8");
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next().expect("le").trim(),
        le,
        "LE roundtrip {type_name}"
    );
    assert_eq!(
        lines.next().expect("be").trim(),
        be,
        "BE roundtrip {type_name}"
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

const MAIN_ADB: &str = r#"with Golden_Gen; use Golden_Gen;
with Ada.Strings.Unbounded; use Ada.Strings.Unbounded;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   G : Golden;
begin
   G.id    := 16#A1B2C3D4#;
   G.kind  := 16#1234#;
   G.flags := 16#5A#;
   G.value := 3.5;
   G.stamp := 16#0102030405060708#;
   G.label := To_Unbounded_String ("bay-12");
   G.raw   := To_Unbounded_String
     (Character'Val (16#DE#) & Character'Val (16#AD#)
      & Character'Val (16#BE#) & Character'Val (16#EF#));
   Put_Line (Hex (Marshal (G, Little)));
   Put_Line (Hex (Marshal (G, Big)));
end Main;
"#;

const KEYHASH_MD5_IDL: &str = "\
@final struct KL { @key long a; @key long b; @key long c; @key long d; @key long e; };";

const KEYHASH_MD5_MAIN_ADB: &str = r#"with Md5kh_Gen; use Md5kh_Gen;
with Ada.Text_IO; use Ada.Text_IO;
with Interfaces; use Interfaces;
procedure Main is
   function Hex (B : Byte_Array) return String is
      Nib : constant String := "0123456789abcdef";
      R   : String (1 .. B'Length * 2);
      I   : Natural := 1;
   begin
      for X of B loop
         R (I)     := Nib (Natural (X / 16) + 1);
         R (I + 1) := Nib (Natural (X mod 16) + 1);
         I := I + 2;
      end loop;
      return R;
   end Hex;
   V : KL;
begin
   V := (a => 16#01020304#, b => 16#05060708#, c => 16#090A0B0C#, d => 16#0D0E0F10#, e => 16#11121314#);
   Put_Line (Hex (Key_Hash (V)));
end Main;
"#;

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
    if Command::new("gnatmake").arg("--version").output().is_err() {
        eprintln!("SKIP keyhash_md5: `gnatmake` not on PATH");
        return;
    }
    let m = emit(
        KEYHASH_MD5_IDL,
        &AdaGenOptions {
            package_name: "Md5kh_Gen".to_string(),
        },
    );
    let dir = std::env::temp_dir().join(format!("idlada_kh_md5_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("md5kh_gen.ads"), &m.spec).expect("ads");
    std::fs::write(dir.join("md5kh_gen.adb"), &m.body).expect("adb");
    std::fs::write(dir.join("main.adb"), KEYHASH_MD5_MAIN_ADB).expect("main");
    let build = Command::new("gnatmake")
        .arg("main.adb")
        .current_dir(&dir)
        .output()
        .expect("gnatmake");
    assert!(
        build.status.success(),
        "gnatmake failed:\n{}\n--- spec ---\n{}\n--- body ---\n{}",
        String::from_utf8_lossy(&build.stderr),
        m.spec,
        m.body
    );
    let run = Command::new("./main")
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
