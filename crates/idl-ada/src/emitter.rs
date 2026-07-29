// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → Ada emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! Ada 2012 package (spec + body): a bounded XCDR2 wire buffer (byte-identical
//! to `endpoints/ada-native`) plus, per IDL `struct`, an Ada record and a
//! `Marshal` function. `@final` (compact) and `@appendable` (a DHEADER-framed
//! body) are supported; other extensibilities and constructs raise
//! [`IdlAdaError::Unsupported`].

use std::fmt::Write as _;

use std::collections::{HashMap, HashSet};

use zerodds_idl::ast::types::{
    CaseLabel, ConstExpr, ConstrTypeDecl, Declarator, Definition, EnumDef, FloatingType,
    IntegerType, Literal, LiteralKind, Member, PrimitiveType, SequenceType, Specification,
    StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnaryOp, UnionDcl, UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, lower_annotations, lower_single,
};

use crate::error::{IdlAdaError, Result};
use crate::keywords::escape_ada_ident;

/// Options for the Ada backend.
#[derive(Debug, Clone)]
pub struct AdaGenOptions {
    /// The Ada package (unit) name; GNAT maps it to `<lower>.ads` / `.adb`.
    pub package_name: String,
}

impl Default for AdaGenOptions {
    fn default() -> Self {
        Self {
            package_name: "Zdgen".to_string(),
        }
    }
}

/// A generated Ada compilation unit: the package spec and body.
#[derive(Debug, Clone)]
pub struct AdaModule {
    /// The `.ads` spec source.
    pub spec: String,
    /// The `.adb` body source.
    pub body: String,
}

/// The bounded XCDR2 wire helpers, emitted into the package body. Byte-identical
/// to the `endpoints/ada-native` core (LE bytes, reversed for BE, cap-4 align).
const WIRE_BODY: &str = r#"   Max_Buffer : constant := 4096;

   type Buf_T is record
      Data   : Byte_Array (0 .. Max_Buffer - 1) := (others => 0);
      Len    : Natural := 0;
      Endian : Endianness := Little;
   end record;

   function F32_Bits is new Ada.Unchecked_Conversion (IEEE_Float_32, Unsigned_32);
   function F64_Bits is new Ada.Unchecked_Conversion (IEEE_Float_64, Unsigned_64);

   procedure Align (W : in out Buf_T; A : Positive) is
      Cap : constant Positive := (if A > 4 then 4 else A);
   begin
      while (W.Len mod Cap) /= 0 loop
         W.Data (W.Len) := 0;
         W.Len := W.Len + 1;
      end loop;
   end Align;

   procedure Put_LE (W : in out Buf_T; A : Positive; LE : Byte_Array) is
   begin
      Align (W, A);
      if W.Endian = Big then
         for I in reverse LE'Range loop
            W.Data (W.Len) := LE (I);
            W.Len := W.Len + 1;
         end loop;
      else
         for I in LE'Range loop
            W.Data (W.Len) := LE (I);
            W.Len := W.Len + 1;
         end loop;
      end if;
   end Put_LE;

   procedure Put_U8 (W : in out Buf_T; V : Unsigned_8) is
   begin
      W.Data (W.Len) := Byte (V);
      W.Len := W.Len + 1;
   end Put_U8;

   procedure Put_Bool (W : in out Buf_T; V : Boolean) is
   begin
      Put_U8 (W, (if V then 1 else 0));
   end Put_Bool;

   procedure Put_U16 (W : in out Buf_T; V : Unsigned_16) is
   begin
      Put_LE (W, 2, (Byte (V and 16#FF#), Byte (Shift_Right (V, 8) and 16#FF#)));
   end Put_U16;

   procedure Put_U32 (W : in out Buf_T; V : Unsigned_32) is
   begin
      Put_LE (W, 4,
        (Byte (V and 16#FF#),
         Byte (Shift_Right (V, 8) and 16#FF#),
         Byte (Shift_Right (V, 16) and 16#FF#),
         Byte (Shift_Right (V, 24) and 16#FF#)));
   end Put_U32;

   procedure Put_U64 (W : in out Buf_T; V : Unsigned_64) is
   begin
      Put_LE (W, 4,
        (Byte (V and 16#FF#),
         Byte (Shift_Right (V, 8) and 16#FF#),
         Byte (Shift_Right (V, 16) and 16#FF#),
         Byte (Shift_Right (V, 24) and 16#FF#),
         Byte (Shift_Right (V, 32) and 16#FF#),
         Byte (Shift_Right (V, 40) and 16#FF#),
         Byte (Shift_Right (V, 48) and 16#FF#),
         Byte (Shift_Right (V, 56) and 16#FF#)));
   end Put_U64;

   procedure Put_F32 (W : in out Buf_T; V : IEEE_Float_32) is
   begin
      Put_U32 (W, F32_Bits (V));
   end Put_F32;

   procedure Put_F64 (W : in out Buf_T; V : IEEE_Float_64) is
   begin
      Put_U64 (W, F64_Bits (V));
   end Put_F64;

   procedure Put_Raw (W : in out Buf_T; S : String) is
   begin
      for C of S loop
         W.Data (W.Len) := Byte (Character'Pos (C));
         W.Len := W.Len + 1;
      end loop;
   end Put_Raw;

   procedure Put_String (W : in out Buf_T; S : String) is
   begin
      Put_U32 (W, Unsigned_32 (S'Length + 1));
      Put_Raw (W, S);
      Put_U8 (W, 0);
   end Put_String;

   procedure Put_Seq_U8 (W : in out Buf_T; S : String) is
   begin
      Put_U32 (W, Unsigned_32 (S'Length));
      Put_Raw (W, S);
   end Put_Seq_U8;

   procedure Put_WString (W : in out Buf_T; S : String) is
      WW : constant Wide_Wide_String :=
        Ada.Strings.UTF_Encoding.Wide_Wide_Strings.Decode (S);
      Units : array (1 .. 2 * WW'Length) of Unsigned_16;
      N     : Natural := 0;
   begin
      for C of WW loop
         declare
            CP : constant Unsigned_32 := Wide_Wide_Character'Pos (C);
         begin
            if CP <= 16#FFFF# then
               N := N + 1;
               Units (N) := Unsigned_16 (CP);
            else
               declare
                  RR : constant Unsigned_32 := CP - 16#10000#;
               begin
                  N := N + 1;
                  Units (N) := Unsigned_16 (16#D800# + Shift_Right (RR, 10));
                  N := N + 1;
                  Units (N) := Unsigned_16 (16#DC00# + (RR and 16#3FF#));
               end;
            end if;
         end;
      end loop;
      Put_U32 (W, Unsigned_32 (N * 2));
      for I in 1 .. N loop
         Put_U16 (W, Units (I));
      end loop;
   end Put_WString;

   procedure Put_Long_Double (W : in out Buf_T; V : IEEE_Float_64) is
      Bits : constant Unsigned_64 := F64_Bits (V);
      Sign : constant Unsigned_64 := Shift_Right (Bits, 63);
      Exp  : constant Unsigned_64 := Shift_Right (Bits, 52) and 16#7FF#;
      Mant : constant Unsigned_64 := Bits and 16#F_FFFF_FFFF_FFFF#;
      Hi   : Unsigned_64 := Shift_Left (Sign, 63);
      Lo   : Unsigned_64 := 0;
      LE   : Byte_Array (0 .. 15);
   begin
      if not (Exp = 0 and Mant = 0) then
         Hi := Shift_Left (Sign, 63) or Shift_Left (Exp - 1023 + 16383, 48)
               or Shift_Right (Mant, 4);
         Lo := Shift_Left (Mant and 16#F#, 60);
      end if;
      for I in 0 .. 7 loop
         LE (I) := Byte (Shift_Right (Lo, 8 * I) and 16#FF#);
         LE (8 + I) := Byte (Shift_Right (Hi, 8 * I) and 16#FF#);
      end loop;
      Put_LE (W, 4, LE);
   end Put_Long_Double;

   procedure Append (W : in out Buf_T; Src : Buf_T) is
   begin
      for I in 0 .. Src.Len - 1 loop
         W.Data (W.Len) := Src.Data (I);
         W.Len := W.Len + 1;
      end loop;
   end Append;

   --  Reader: functions consume from Data at offset Pos (advanced in place).
   --  Alignment is stream-relative (Pos from Data'First), inverse of the Writer.
   function U32_F32 is new Ada.Unchecked_Conversion (Unsigned_32, IEEE_Float_32);
   function U64_F64 is new Ada.Unchecked_Conversion (Unsigned_64, IEEE_Float_64);
   function U8_I8 is new Ada.Unchecked_Conversion (Unsigned_8, Integer_8);
   function U16_I16 is new Ada.Unchecked_Conversion (Unsigned_16, Integer_16);
   function U32_I32 is new Ada.Unchecked_Conversion (Unsigned_32, Integer_32);
   function U64_I64 is new Ada.Unchecked_Conversion (Unsigned_64, Integer_64);

   procedure Ralign (Pos : in out Natural; A : Positive) is
      Cap : constant Positive := (if A > 4 then 4 else A);
   begin
      while (Pos mod Cap) /= 0 loop
         Pos := Pos + 1;
      end loop;
   end Ralign;

   function Get_LE
     (Data : Byte_Array; Pos : in out Natural; A : Positive; N : Positive;
      Endian : Endianness) return Unsigned_64
   is
      V : Unsigned_64 := 0;
   begin
      Ralign (Pos, A);
      if Endian = Big then
         for I in 0 .. N - 1 loop
            V := Shift_Left (V, 8) or Unsigned_64 (Data (Data'First + Pos + I));
         end loop;
      else
         for I in reverse 0 .. N - 1 loop
            V := Shift_Left (V, 8) or Unsigned_64 (Data (Data'First + Pos + I));
         end loop;
      end if;
      Pos := Pos + N;
      return V;
   end Get_LE;

   function Get_U8 (Data : Byte_Array; Pos : in out Natural) return Unsigned_8 is
      V : constant Unsigned_8 := Unsigned_8 (Data (Data'First + Pos));
   begin
      Pos := Pos + 1;
      return V;
   end Get_U8;

   function Get_Bool (Data : Byte_Array; Pos : in out Natural) return Boolean is
   begin
      return Get_U8 (Data, Pos) /= 0;
   end Get_Bool;

   function Get_U16 (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return Unsigned_16 is
   begin
      return Unsigned_16 (Get_LE (Data, Pos, 2, 2, Endian));
   end Get_U16;

   function Get_U32 (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return Unsigned_32 is
   begin
      return Unsigned_32 (Get_LE (Data, Pos, 4, 4, Endian));
   end Get_U32;

   function Get_U64 (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return Unsigned_64 is
   begin
      return Get_LE (Data, Pos, 4, 8, Endian);
   end Get_U64;

   function Get_F32 (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return IEEE_Float_32 is
   begin
      return U32_F32 (Get_U32 (Data, Pos, Endian));
   end Get_F32;

   function Get_F64 (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return IEEE_Float_64 is
   begin
      return U64_F64 (Get_U64 (Data, Pos, Endian));
   end Get_F64;

   procedure Skip_U32 (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) is
      V : constant Unsigned_32 := Get_U32 (Data, Pos, Endian);
      pragma Unreferenced (V);
   begin
      null;
   end Skip_U32;

   function Get_String (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return Unbounded_String is
      N : constant Natural := Natural (Get_U32 (Data, Pos, Endian));
      S : String (1 .. N - 1);
   begin
      for I in S'Range loop
         S (I) := Character'Val (Natural (Data (Data'First + Pos + I - 1)));
      end loop;
      Pos := Pos + N;
      return To_Unbounded_String (S);
   end Get_String;

   function Get_Seq_U8 (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return Unbounded_String is
      N : constant Natural := Natural (Get_U32 (Data, Pos, Endian));
      S : String (1 .. N);
   begin
      for I in S'Range loop
         S (I) := Character'Val (Natural (Data (Data'First + Pos + I - 1)));
      end loop;
      Pos := Pos + N;
      return To_Unbounded_String (S);
   end Get_Seq_U8;

   function Get_WString (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return Unbounded_String is
      N     : constant Natural := Natural (Get_U32 (Data, Pos, Endian)) / 2;
      Units : array (1 .. N) of Unsigned_16;
      WW    : Wide_Wide_String (1 .. N);
      WN    : Natural := 0;
      I     : Natural := 1;
   begin
      for K in 1 .. N loop
         Units (K) := Get_U16 (Data, Pos, Endian);
      end loop;
      while I <= N loop
         declare
            U : constant Unsigned_32 := Unsigned_32 (Units (I));
         begin
            if U >= 16#D800# and U <= 16#DBFF# and I + 1 <= N then
               declare
                  Lo : constant Unsigned_32 := Unsigned_32 (Units (I + 1));
               begin
                  WN := WN + 1;
                  WW (WN) := Wide_Wide_Character'Val
                    (16#10000# + Shift_Left (U - 16#D800#, 10) + (Lo - 16#DC00#));
                  I := I + 2;
               end;
            else
               WN := WN + 1;
               WW (WN) := Wide_Wide_Character'Val (U);
               I := I + 1;
            end if;
         end;
      end loop;
      return To_Unbounded_String
        (Ada.Strings.UTF_Encoding.Wide_Wide_Strings.Encode (WW (1 .. WN)));
   end Get_WString;

   function Get_Long_Double (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return IEEE_Float_64 is
      LE : Byte_Array (0 .. 15);
      Lo : Unsigned_64 := 0;
      Hi : Unsigned_64 := 0;
   begin
      Ralign (Pos, 4);
      for I in 0 .. 15 loop
         LE (I) := Data (Data'First + Pos + I);
      end loop;
      Pos := Pos + 16;
      if Endian = Big then
         declare
            Tmp : Byte;
         begin
            for I in 0 .. 7 loop
               Tmp := LE (I);
               LE (I) := LE (15 - I);
               LE (15 - I) := Tmp;
            end loop;
         end;
      end if;
      for I in 0 .. 7 loop
         Lo := Lo or Shift_Left (Unsigned_64 (LE (I)), 8 * I);
         Hi := Hi or Shift_Left (Unsigned_64 (LE (8 + I)), 8 * I);
      end loop;
      declare
         Sign : constant Unsigned_64 := Shift_Right (Hi, 63);
         Exp  : constant Unsigned_64 := Shift_Right (Hi, 48) and 16#7FFF#;
         Mant : constant Unsigned_64 :=
           Shift_Left (Hi and 16#FFFF_FFFF_FFFF#, 4) or Shift_Right (Lo, 60);
         Bits : Unsigned_64;
      begin
         if Exp = 0 and Mant = 0 then
            Bits := Shift_Left (Sign, 63);
         else
            Bits := Shift_Left (Sign, 63) or Shift_Left (Exp - 16383 + 1023, 52) or Mant;
         end if;
         return U64_F64 (Bits);
      end;
   end Get_Long_Double;
"#;

/// Generates a self-contained Ada package (spec + body) from the IDL AST.
///
/// # Errors
/// Returns [`IdlAdaError::Unsupported`] for constructs the Ada backend does not
/// yet emit (unions, nested-struct members, maps, `long double`, `@mutable`, …).
pub fn generate_ada_module(spec: &Specification, opts: &AdaGenOptions) -> Result<AdaModule> {
    let pkg = &opts.package_name;

    // `module X { ... }` content is promoted into the same flat, top-level
    // definition list (see `flatten_module_defs`) so it is no longer
    // silently dropped (swarm59 #21b).
    let flat = flatten_module_defs(&spec.definitions);

    // Keyed by the *raw* IDL name (not the Ada-escaped `ada_name`) so that
    // `Scoped` type-reference lookups below — which start from raw AST text
    // — resolve correctly regardless of keyword escaping.
    let enum_names: HashSet<String> = flat
        .iter()
        .filter_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                Some(e.name.text.clone())
            }
            _ => None,
        })
        .collect();
    let enums: Vec<EnumGen> = flat
        .iter()
        .filter_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => Some(build_enum(e)),
            _ => None,
        })
        .collect();

    let struct_names: HashSet<String> = flat
        .iter()
        .filter_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                Some(s.name.text.clone())
            }
            _ => None,
        })
        .collect();

    let typedefs = collect_typedefs(spec);
    // struct name → def, so a nested-struct `@key` member's own `@key`
    // subset can be resolved for Key_Hash emission (Bug A) and for the
    // static MD5-vs-zero-pad branch decision (Bug B) — mirrors
    // `collect_typedefs`.
    let struct_defs = collect_structs(spec);

    let mut structs: Vec<StructGen> = Vec::new();
    let mut unions: Vec<UnionGen> = Vec::new();
    for def in &flat {
        match def {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                structs.push(build_struct(
                    s,
                    &enum_names,
                    &struct_names,
                    &typedefs,
                    &struct_defs,
                )?);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                unions.push(build_union(u, &enum_names, &struct_names, &typedefs)?);
            }
            _ => {}
        }
    }

    // Struct names actually used as a `sequence<Struct>` element somewhere in
    // this spec (struct members, union cases, or as the element type of a
    // fixed array — `sequence<Struct> f[N]`) — only those need a generated
    // `{Struct}_Vectors` package. Detected from the already-resolved field/case
    // `ada_type` strings, which `map_sequence` renders as `"{name}_Vectors.Vector"`,
    // plus `StructGen.array_vector_elems` for the array-declarator case (there
    // `FieldGen.ada_type` holds the synthetic array type name, not the element
    // type, so the vector-element struct name is captured separately).
    let mut vectors_used: HashSet<String> = HashSet::new();
    for sg in &structs {
        for f in &sg.fields {
            if let Some(n) = f.ada_type.strip_suffix("_Vectors.Vector") {
                vectors_used.insert(n.to_string());
            }
        }
        for n in &sg.array_vector_elems {
            vectors_used.insert(n.clone());
        }
    }
    for ug in &unions {
        for c in &ug.cases {
            if let Some(n) = c.ada_type.strip_suffix("_Vectors.Vector") {
                vectors_used.insert(n.to_string());
            }
        }
    }
    let any_map = structs.iter().any(|sg| !sg.map_packages.is_empty());

    // --- spec (.ads) ---
    let mut spec_src = String::new();
    let _ = writeln!(
        spec_src,
        "-- Code generated by zerodds-idlc (Ada backend). DO NOT EDIT."
    );
    let _ = writeln!(spec_src, "-- SPDX-License-Identifier: Apache-2.0");
    let _ = writeln!(spec_src, "with Interfaces; use Interfaces;");
    let _ = writeln!(
        spec_src,
        "with Ada.Strings.Unbounded; use Ada.Strings.Unbounded;"
    );
    if !vectors_used.is_empty() {
        let _ = writeln!(spec_src, "with Ada.Containers.Vectors;");
    }
    if any_map {
        let _ = writeln!(spec_src, "with Ada.Containers.Ordered_Maps;");
    }
    let _ = writeln!(spec_src, "package {pkg} is\n");
    let _ = writeln!(spec_src, "   type Byte is new Interfaces.Unsigned_8;");
    let _ = writeln!(
        spec_src,
        "   type Byte_Array is array (Natural range <>) of Byte;"
    );
    let _ = writeln!(spec_src, "   type Endianness is (Little, Big);\n");
    // Named enums (32-bit signed integer on the wire, XTypes 1.3 §7.4.5.1).
    for eg in &enums {
        let _ = writeln!(
            spec_src,
            "   type {} is ({});",
            eg.ada_name,
            eg.ctors.join(", ")
        );
    }
    if !enums.is_empty() {
        let _ = writeln!(spec_src);
    }
    for sg in &structs {
        for at in &sg.array_types {
            let _ = writeln!(spec_src, "   {at}");
        }
        for mp in &sg.map_packages {
            let _ = writeln!(spec_src, "   {mp}");
        }
        let _ = writeln!(spec_src, "   type {} is record", sg.ada_name);
        for f in &sg.fields {
            let _ = writeln!(spec_src, "      {} : {};", f.ada_name, f.ada_type);
        }
        let _ = writeln!(spec_src, "   end record;");
        // A vector package so this struct can be a sequence<struct> element —
        // only when something in the spec actually declares `sequence<{n}>`.
        if vectors_used.contains(&sg.ada_name) {
            let _ = writeln!(
                spec_src,
                "   package {n}_Vectors is new Ada.Containers.Vectors (Natural, {n});",
                n = sg.ada_name
            );
        }
        let _ = writeln!(
            spec_src,
            "   function Marshal (V : {}; Endian : Endianness) return Byte_Array;",
            sg.ada_name
        );
        let _ = writeln!(
            spec_src,
            "   function Unmarshal (Data : Byte_Array; Endian : Endianness) return {};\n",
            sg.ada_name
        );
        if sg.fields.iter().any(|f| f.key) {
            let _ = writeln!(
                spec_src,
                "   function Key_Hash (V : {}) return Byte_Array;\n",
                sg.ada_name
            );
        }
    }
    for ug in &unions {
        let _ = writeln!(spec_src, "   type {} is record", ug.ada_name);
        let _ = writeln!(spec_src, "      disc : {};", ug.disc_type);
        for c in &ug.cases {
            let _ = writeln!(spec_src, "      {} : {};", c.field, c.ada_type);
        }
        let _ = writeln!(spec_src, "   end record;");
        let _ = writeln!(
            spec_src,
            "   function Marshal (V : {}; Endian : Endianness) return Byte_Array;",
            ug.ada_name
        );
        let _ = writeln!(
            spec_src,
            "   function Unmarshal (Data : Byte_Array; Endian : Endianness) return {};\n",
            ug.ada_name
        );
    }
    let _ = writeln!(spec_src, "end {pkg};");

    // --- body (.adb) ---
    let mut body_src = String::new();
    let _ = writeln!(
        body_src,
        "-- Code generated by zerodds-idlc (Ada backend). DO NOT EDIT."
    );
    let _ = writeln!(body_src, "-- SPDX-License-Identifier: Apache-2.0");
    let _ = writeln!(body_src, "with Interfaces; use Interfaces;");
    let _ = writeln!(
        body_src,
        "with Ada.Strings.Unbounded; use Ada.Strings.Unbounded;"
    );
    let _ = writeln!(body_src, "with Ada.Unchecked_Conversion;");
    let _ = writeln!(body_src, "with Ada.Strings.UTF_Encoding.Wide_Wide_Strings;");
    if any_map {
        let _ = writeln!(body_src, "with Ada.Containers.Ordered_Maps;");
    }
    // GNAT.MD5 only for the KeyHash MD5 branch.
    if structs
        .iter()
        .any(|sg| sg.md5_key && sg.fields.iter().any(|f| f.key))
    {
        let _ = writeln!(body_src, "with GNAT.MD5;");
    }
    let _ = writeln!(body_src, "package body {pkg} is\n");
    body_src.push_str(WIRE_BODY);
    for eg in &enums {
        emit_enum_to_u32(&mut body_src, eg);
        emit_u32_to_enum(&mut body_src, eg);
    }
    for sg in &structs {
        emit_marshal(&mut body_src, sg);
    }
    for ug in &unions {
        emit_union_marshal(&mut body_src, ug);
    }
    let _ = writeln!(body_src, "\nend {pkg};");

    Ok(AdaModule {
        spec: spec_src,
        body: body_src,
    })
}

/// A generated enum: Ada type name, enumerators, and their i32 wire values.
struct EnumGen {
    ada_name: String,
    ctors: Vec<String>,
    u32vals: Vec<u32>,
}

/// Resolves each enumerator's discriminant: default 0..N-1, honoring `@value`
/// (XTypes 1.3 §7.4.5.1). Values are returned as their `u32` wire bit pattern.
fn build_enum(e: &EnumDef) -> EnumGen {
    let mut u32vals = Vec::with_capacity(e.enumerators.len());
    let mut next: i64 = 0;
    for en in &e.enumerators {
        let explicit = en.annotations.iter().find_map(|a| match lower_single(a) {
            Ok(Some(BuiltinAnnotation::Value(s))) => parse_int(&s),
            _ => None,
        });
        let v = explicit.unwrap_or(next) as i32;
        u32vals.push(v as u32);
        next = i64::from(v) + 1;
    }
    EnumGen {
        ada_name: escape_ada_ident(&e.name.text),
        ctors: e
            .enumerators
            .iter()
            .map(|en| escape_ada_ident(&en.name.text))
            .collect(),
        u32vals,
    }
}

/// Parses a decimal or `0x` hex integer literal (possibly signed).
fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()
    } else {
        t.parse::<i64>().ok()
    }
}

/// Emits the `<Name>_To_U32` mapping function (portable Ada `case`, no GNAT attr).
fn emit_enum_to_u32(out: &mut String, eg: &EnumGen) {
    let n = &eg.ada_name;
    let _ = writeln!(
        out,
        "   function {n}_To_U32 (V : {n}) return Unsigned_32 is"
    );
    let _ = writeln!(out, "   begin");
    let _ = writeln!(out, "      case V is");
    for (c, v) in eg.ctors.iter().zip(&eg.u32vals) {
        let _ = writeln!(out, "         when {c} => return {v};");
    }
    let _ = writeln!(out, "      end case;");
    let _ = writeln!(out, "   end {n}_To_U32;");
    let _ = writeln!(out);
}

/// Emits the `<Name>_Of_U32` inverse mapping (decode; unknown values fall back to
/// the first enumerator — never hit for values produced by `<Name>_To_U32`).
fn emit_u32_to_enum(out: &mut String, eg: &EnumGen) {
    let n = &eg.ada_name;
    let fallback = eg.ctors.first().cloned().unwrap_or_default();
    let _ = writeln!(
        out,
        "   function {n}_Of_U32 (V : Unsigned_32) return {n} is"
    );
    let _ = writeln!(out, "   begin");
    let _ = writeln!(out, "      case V is");
    for (c, v) in eg.ctors.iter().zip(&eg.u32vals) {
        let _ = writeln!(out, "         when {v} => return {c};");
    }
    let _ = writeln!(out, "         when others => return {fallback};");
    let _ = writeln!(out, "      end case;");
    let _ = writeln!(out, "   end {n}_Of_U32;");
    let _ = writeln!(out);
}

struct FieldGen {
    ada_name: String,
    ada_type: String,
    // put statement operating on writer `$w`, referencing `V.<ada_name>`.
    put: String,
    // decode statement(s) reading into `V.<ada_name>` from `Data`/`Pos`/`Endian`.
    get: String,
    id: u32,
    key: bool,
    /// Typedef-dealiased type of this field.
    resolved: TypeSpec,
    /// `true` for a `Declarator::Simple` (not a fixed array).
    simple: bool,
}

struct StructGen {
    ada_name: String,
    /// Named array-type declarations (Ada forbids anonymous array record
    /// components), emitted in the spec before the record.
    array_types: Vec<String>,
    /// Struct names used as the *element type* of a fixed array declared in
    /// `array_types` (i.e. `sequence<Struct> f[N]`). The array element type is
    /// `{Struct}_Vectors.Vector`, but that string never reaches
    /// `FieldGen.ada_type` (which instead gets the synthetic array type name,
    /// e.g. `Outer_f_A`), so it is captured here to still gate/emit the
    /// element struct's `{Struct}_Vectors` package.
    array_vector_elems: Vec<String>,
    /// Ordered_Maps package instantiations for map members.
    map_packages: Vec<String>,
    fields: Vec<FieldGen>,
    appendable: bool,
    mutable: bool,
    /// KeyHash uses the MD5 branch (max `@key` size > 16 or dynamically sized).
    md5_key: bool,
    /// Fully-expanded Key_Hash put statements (each using the `$w` writer
    /// placeholder), in member-id order. A `@key` member whose type is a
    /// nested struct is already expanded here to only that struct's own
    /// `@key` members (Bug A) — NOT the full-member `put` in `fields`.
    key_puts: Vec<String>,
}

/// Evaluates a fixed-array bound to its integer size (literal + unary sign).
/// zerodds-lint: recursion-depth 32
fn array_size(e: &ConstExpr) -> Option<i64> {
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => parse_int(raw),
        ConstExpr::Unary { op, operand, .. } => {
            let v = array_size(operand)?;
            match op {
                UnaryOp::Plus => Some(v),
                UnaryOp::Minus => Some(-v),
                UnaryOp::BitNot => Some(!v),
            }
        }
        _ => None,
    }
}

/// Wraps a per-element put (`$elem`) in nested row-major `for … loop` loops over
/// a fixed Ada array `V.<field> (i0, i1)` (Ada true multi-dim indexing).
fn build_array_put(field: &str, sizes: &[i64], elem_put: &str) -> String {
    let idx = (0..sizes.len())
        .map(|k| format!("i{k}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut body = elem_put.replace("$elem", &format!("V.{field} ({idx})"));
    for k in (0..sizes.len()).rev() {
        body = format!("for i{k} in 0 .. {} loop\n{body}\nend loop;", sizes[k] - 1);
    }
    body
}

/// Recursively descends into `Definition::Module`, returning every
/// non-module definition (struct/enum/union/typedef/…) in document order.
/// The IDL AST builder already merges a reopened `module M {} ... module
/// M {}` into one AST node (`crates/idl/src/ast/builder.rs`); this promotes
/// a module's members into the same flat namespace this backend already
/// uses for type-reference resolution (`sn.parts.last()` below) — module
/// content is no longer silently dropped (swarm59 #21b), it is simply not
/// namespaced: two same-named types in different modules collide, exactly
/// as two same-named top-level types would.
///
/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_module_defs(defs: &[Definition]) -> Vec<&Definition> {
    let mut out = Vec::new();
    flatten_module_defs_into(defs, &mut out);
    out
}

/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_module_defs_into<'a>(defs: &'a [Definition], out: &mut Vec<&'a Definition>) {
    for d in defs {
        match d {
            Definition::Module(m) => flatten_module_defs_into(&m.definitions, out),
            other => out.push(other),
        }
    }
}

/// Collects `typedef` aliases (simple declarators) as name -> aliased type-spec.
/// A typedef is wire-transparent, so members are resolved to the underlying
/// type before mapping (`typedef long Score; Score s;` marshals as `long`).
fn collect_typedefs(spec: &Specification) -> HashMap<String, TypeSpec> {
    let mut m = HashMap::new();
    for def in flatten_module_defs(&spec.definitions) {
        if let Definition::Type(TypeDecl::Typedef(td)) = def {
            for d in &td.declarators {
                if let Declarator::Simple(name) = d {
                    m.insert(name.text.clone(), td.type_spec.clone());
                }
            }
        }
    }
    m
}

/// Collects top-level `struct` definitions as name → def, so a nested-struct
/// `@key` member can be expanded into its own `@key` subset (XTypes 1.3
/// §7.6.8) for Key_Hash emission and for the static max-size (MD5 vs.
/// zero-pad) branch decision.
fn collect_structs(spec: &Specification) -> HashMap<String, &StructDef> {
    let mut m = HashMap::new();
    for def in &spec.definitions {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) = def
        {
            m.insert(s.name.text.clone(), s);
        }
    }
    m
}

/// Resolves a typedef chain to its underlying type-spec (recursing into
/// sequence elements). Non-typedef types pass through unchanged.
///
/// zerodds-lint: recursion-depth 32 (typedef alias chains + nested sequence
/// elements; bounded by the IDL's alias/collection nesting depth).
fn resolve_typedef(t: &TypeSpec, typedefs: &HashMap<String, TypeSpec>) -> TypeSpec {
    match t {
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            match typedefs.get(&name) {
                Some(u) => resolve_typedef(u, typedefs),
                None => t.clone(),
            }
        }
        TypeSpec::Sequence(seq) => TypeSpec::Sequence(SequenceType {
            elem: Box::new(resolve_typedef(&seq.elem, typedefs)),
            bound: seq.bound.clone(),
            span: seq.span,
        }),
        other => other.clone(),
    }
}

/// A type is "primitive" for the map-DHEADER rule if it is fully descriptive on
/// the wire: an IDL primitive or an enum (i32). Others force a collection DHEADER.
fn is_primitive(t: &TypeSpec, enum_names: &HashSet<String>) -> bool {
    match t {
        TypeSpec::Primitive(_) => true,
        TypeSpec::Scoped(sn) => {
            enum_names.contains(&sn.parts.last().map(|p| p.text.clone()).unwrap_or_default())
        }
        _ => false,
    }
}

fn build_struct(
    s: &StructDef,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    struct_defs: &HashMap<String, &StructDef>,
) -> Result<StructGen> {
    let ext = lower_annotations(&s.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable);
    let struct_ada_name = escape_ada_ident(&s.name.text);
    let mut fields = Vec::new();
    let mut array_types = Vec::new();
    let mut array_vector_elems = Vec::new();
    let mut map_packages = Vec::new();
    let mut next_id: u32 = 0;
    for m in &s.members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        for d in &m.declarators {
            let name = escape_ada_ident(&d.name().text);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let simple = matches!(d, Declarator::Simple(_));
            let (ada_type, put, get) = match (&resolved, d) {
                // A map: an Ada.Containers.Ordered_Maps instance (iterates
                // ascending by key) — `u32 count` + key/value pairs, DHEADER-
                // framed only when the key/value pair is non-primitive.
                (TypeSpec::Map(mp), Declarator::Simple(_)) => {
                    let (key_type, key_put) = map_type(&mp.key, "$K", enum_names, struct_names)?;
                    let (val_type, val_put) = map_type(&mp.value, "$V", enum_names, struct_names)?;
                    let pkg_name = format!("{struct_ada_name}_{name}_Map");
                    map_packages.push(format!(
                        "package {pkg_name} is new Ada.Containers.Ordered_Maps \
                         ({key_type}, {val_type});"
                    ));
                    let prim =
                        is_primitive(&mp.key, enum_names) && is_primitive(&mp.value, enum_names);
                    let loop_body = |wv: &str| {
                        let kp = key_put
                            .replace("$K", &format!("{pkg_name}.Key (C)"))
                            .replace("$w", wv);
                        let vp = val_put
                            .replace("$V", &format!("{pkg_name}.Element (C)"))
                            .replace("$w", wv);
                        format!(
                            "declare C : {pkg_name}.Cursor := V.{name}.First; begin \
                             while {pkg_name}.Has_Element (C) loop {kp} {vp} \
                             {pkg_name}.Next (C); end loop; end;"
                        )
                    };
                    // XTypes 1.3 §7.4.3: bound check on encode (before the
                    // count is written) and its decode-side mirror (after
                    // the count is read, before the fill loop allocates).
                    let put_check = mp
                        .bound
                        .as_ref()
                        .map(|b| {
                            bound_check_stmt(
                                &format!("Natural (V.{name}.Length)"),
                                b,
                                "bounded",
                                "map",
                            )
                        })
                        .transpose()?
                        .map(|s| format!("{s} "))
                        .unwrap_or_default();
                    let put = if prim {
                        format!(
                            "{put_check}Put_U32 ($w, Unsigned_32 (Natural (V.{name}.Length))); {}",
                            loop_body("$w")
                        )
                    } else {
                        format!(
                            "{put_check}declare B2 : Buf_T; begin B2.Endian := $w.Endian; \
                             Put_U32 (B2, Unsigned_32 (Natural (V.{name}.Length))); {} \
                             Put_U32 ($w, Unsigned_32 (B2.Len)); Append ($w, B2); end;",
                            loop_body("B2")
                        )
                    };
                    let key_get = map_get(&mp.key, "Zk", enum_names, struct_names)?;
                    let val_get = map_get(&mp.value, "Zv", enum_names, struct_names)?;
                    let dh = if prim {
                        ""
                    } else {
                        "Skip_U32 (Data, Pos, Endian); "
                    };
                    let get_check = mp
                        .bound
                        .as_ref()
                        .map(|b| bound_check_stmt("Zn", b, "decoded", "map"))
                        .transpose()?
                        .map(|s| format!("{s} "))
                        .unwrap_or_default();
                    let get = format!(
                        "declare Zn : Natural; Zk : {key_type}; Zv : {val_type}; begin {dh}Zn := Natural (Get_U32 (Data, Pos, Endian)); {get_check}V.{name}.Clear; for Zi in 1 .. Zn loop {key_get} {val_get} V.{name}.Insert (Zk, Zv); end loop; end;"
                    );
                    (format!("{pkg_name}.Map"), put, get)
                }
                (_, Declarator::Simple(_)) => {
                    let (t, p) =
                        map_type(&resolved, &format!("V.{name}"), enum_names, struct_names)?;
                    let g = map_get(&resolved, &format!("V.{name}"), enum_names, struct_names)?;
                    (t, p, g)
                }
                // Fixed array: a named Ada multi-dim array type + inline row-major
                // marshal loop (elements inline, no length prefix).
                (_, Declarator::Array(ad)) => {
                    let sizes = ad
                        .sizes
                        .iter()
                        .map(array_size)
                        .collect::<Option<Vec<i64>>>()
                        .ok_or_else(|| {
                            IdlAdaError::Unsupported(format!("non-literal array size on `{name}`"))
                        })?;
                    let (elem_type, elem_put) =
                        map_type(&resolved, "$elem", enum_names, struct_names)?;
                    if let Some(n) = elem_type.strip_suffix("_Vectors.Vector") {
                        array_vector_elems.push(n.to_string());
                    }
                    let type_name = format!("{struct_ada_name}_{name}_A");
                    let ranges = sizes
                        .iter()
                        .map(|n| format!("0 .. {}", n - 1))
                        .collect::<Vec<_>>()
                        .join(", ");
                    array_types.push(format!(
                        "type {type_name} is array ({ranges}) of {elem_type};"
                    ));
                    let put = build_array_put(&name, &sizes, &elem_put);
                    let elem_get = map_get(&resolved, "$L", enum_names, struct_names)?;
                    let get = build_array_get(&name, &sizes, &elem_get);
                    (type_name, put, get)
                }
            };
            fields.push(FieldGen {
                ada_name: name,
                ada_type,
                put,
                get,
                id,
                key,
                resolved: resolved.clone(),
                simple,
            });
        }
    }
    let key_members: Vec<&Member> = s
        .members
        .iter()
        .filter(|m| {
            lower_annotations(&m.annotations)
                .map(|l| l.has_key())
                .unwrap_or(false)
        })
        .collect();
    let md5_key = zerodds_idl::keyhash::uses_md5(&key_members, struct_defs, typedefs);

    // Bug A: a `@key` member whose (typedef-dealiased) type is itself a
    // struct must expand to ONLY that struct's own `@key` members (or ALL
    // its members if it declares none), in member-id order — not the
    // struct's full member set. `f.put` reuses the generic per-field
    // mapper, which is correct for normal (non-key) struct encoding but
    // always encodes the FULL member set, so it must not be used here for a
    // nested-struct key.
    let mut zdkeys: Vec<&FieldGen> = fields.iter().filter(|f| f.key).collect();
    zdkeys.sort_by_key(|f| f.id);
    let mut key_puts: Vec<String> = Vec::new();
    for f in &zdkeys {
        let nested_struct = if f.simple {
            match &f.resolved {
                TypeSpec::Scoped(sn) => {
                    let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
                    struct_defs.get(&name).copied()
                }
                _ => None,
            }
        } else {
            None
        };
        if let Some(sd) = nested_struct {
            emit_key_struct_member(
                &mut key_puts,
                sd,
                &format!("V.{}", f.ada_name),
                enum_names,
                struct_names,
                typedefs,
                struct_defs,
            )?;
        } else {
            key_puts.push(f.put.clone());
        }
    }

    Ok(StructGen {
        ada_name: struct_ada_name,
        array_types,
        array_vector_elems,
        map_packages,
        fields,
        appendable: ext == ExtensibilityKind::Appendable,
        mutable: ext == ExtensibilityKind::Mutable,
        md5_key,
        key_puts,
    })
}

/// Appends Key_Hash put statements (each using the `$w` writer placeholder)
/// for a nested-struct `@key` member: expands to `sd`'s own `@key` members
/// (or ALL members if it declares none — XTypes 1.3 §7.6.8), in member-id
/// order, recursing again if one of those members is itself a nested
/// struct. Mirrors `idl-rust`'s `emit_key_field_write` (see
/// `crates/idl-rust/src/struct_emit.rs`).
///
/// zerodds-lint: recursion-depth 16 (nested `@key` struct expansion;
/// bounded by the IDL's aggregate nesting depth).
fn emit_key_struct_member(
    out: &mut Vec<String>,
    sd: &StructDef,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    struct_defs: &HashMap<String, &StructDef>,
) -> Result<()> {
    let nested_keys: Vec<&Member> = sd
        .members
        .iter()
        .filter(|m| {
            lower_annotations(&m.annotations)
                .map(|l| l.has_key())
                .unwrap_or(false)
        })
        .collect();
    let effective: Vec<&Member> = if nested_keys.is_empty() {
        sd.members.iter().collect()
    } else {
        nested_keys
    };
    let mut ordered: Vec<(u32, &Member)> = effective
        .iter()
        .enumerate()
        .map(|(idx, m)| {
            let id = lower_annotations(&m.annotations)
                .ok()
                .and_then(|l| l.explicit_id())
                .unwrap_or(idx as u32);
            (id, *m)
        })
        .collect();
    ordered.sort_by_key(|(id, _)| *id);
    for (_, m) in &ordered {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        for d in &m.declarators {
            // Arrays inside a nested-struct key are out of scope; reject
            // explicitly rather than silently emitting a wrong KeyHash
            // (matches the `idl-rust` reference).
            if matches!(d, Declarator::Array(_)) {
                return Err(IdlAdaError::Unsupported(
                    "array @key field inside a nested-struct key".to_string(),
                ));
            }
            let field = d.name().text.clone();
            let nested_expr = format!("{expr}.{field}");
            if let TypeSpec::Scoped(sn) = &resolved {
                let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
                if let Some(nested_sd) = struct_defs.get(&name) {
                    emit_key_struct_member(
                        out,
                        nested_sd,
                        &nested_expr,
                        enum_names,
                        struct_names,
                        typedefs,
                        struct_defs,
                    )?;
                    continue;
                }
            }
            let (_, put) = map_type(&resolved, &nested_expr, enum_names, struct_names)?;
            out.push(put);
        }
    }
    Ok(())
}

fn emit_marshal(out: &mut String, sg: &StructGen) {
    // Body-local Marshal_Into: writes into an existing writer (nested composites
    // call this so alignment stays stream-relative). Overloaded by the record
    // type. @final: fields inline; @appendable: DHEADER-framed body.
    let _ = writeln!(
        out,
        "\n   procedure Marshal_Into (V : {}; W : in out Buf_T) is",
        sg.ada_name
    );
    if sg.appendable || sg.mutable {
        let _ = writeln!(out, "      B : Buf_T;");
    }
    let _ = writeln!(out, "   begin");
    if sg.mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "      B.Endian := W.Endian;");
        for f in &sg.fields {
            let emh = 0x4000_0000_u32 | f.id;
            let _ = writeln!(out, "      Put_U32 (B, 16#{emh:08X}#);");
            let _ = writeln!(out, "      declare");
            let _ = writeln!(out, "         M2 : Buf_T;");
            let _ = writeln!(out, "      begin");
            let _ = writeln!(out, "         M2.Endian := W.Endian;");
            let _ = writeln!(out, "         {}", f.put.replace("$w", "M2"));
            let _ = writeln!(out, "         Put_U32 (B, Unsigned_32 (M2.Len));");
            let _ = writeln!(out, "         Append (B, M2);");
            let _ = writeln!(out, "      end;");
        }
        let _ = writeln!(out, "      Put_U32 (W, Unsigned_32 (B.Len));");
        let _ = writeln!(out, "      Append (W, B);");
    } else {
        let wv = if sg.appendable {
            let _ = writeln!(out, "      B.Endian := W.Endian;");
            "B"
        } else {
            "W"
        };
        for f in &sg.fields {
            let _ = writeln!(out, "      {}", f.put.replace("$w", wv));
        }
        if sg.appendable {
            let _ = writeln!(out, "      Put_U32 (W, Unsigned_32 (B.Len));");
            let _ = writeln!(out, "      Append (W, B);");
        } else {
            let _ = writeln!(out, "      null;");
        }
    }
    let _ = writeln!(out, "   end Marshal_Into;");

    let _ = writeln!(
        out,
        "\n   function Marshal (V : {}; Endian : Endianness) return Byte_Array is",
        sg.ada_name
    );
    let _ = writeln!(out, "      W : Buf_T;");
    let _ = writeln!(out, "   begin");
    let _ = writeln!(out, "      W.Endian := Endian;");
    let _ = writeln!(out, "      Marshal_Into (V, W);");
    let _ = writeln!(out, "      return W.Data (0 .. W.Len - 1);");
    let _ = writeln!(out, "   end Marshal;");

    if !sg.key_puts.is_empty() {
        // KeyHash (XTypes §7.6.8): @key members PLAIN_CDR2-BE, zero-padded to 16.
        let _ = writeln!(
            out,
            "\n   function Key_Hash (V : {}) return Byte_Array is",
            sg.ada_name
        );
        let _ = writeln!(out, "      KW   : Buf_T;");
        let _ = writeln!(out, "      Outk : Byte_Array (0 .. 15) := (others => 0);");
        let _ = writeln!(out, "   begin");
        let _ = writeln!(out, "      KW.Endian := Big;");
        for put in &sg.key_puts {
            let _ = writeln!(out, "      {}", put.replace("$w", "KW"));
        }
        if sg.md5_key {
            // KeyHolder max size > 16 → MD5(bytes)[0..16] (XTypes §7.6.8.4).
            // GNAT.MD5.Digest yields a 32-char lowercase hex string; parse it.
            let _ = writeln!(out, "      declare");
            let _ = writeln!(out, "         S : String (1 .. KW.Len);");
            let _ = writeln!(out, "      begin");
            let _ = writeln!(out, "         for I in 0 .. KW.Len - 1 loop");
            let _ = writeln!(
                out,
                "            S (I + 1) := Character'Val (Natural (KW.Data (I)));"
            );
            let _ = writeln!(out, "         end loop;");
            let _ = writeln!(out, "         declare");
            let _ = writeln!(
                out,
                "            Hex : constant String := GNAT.MD5.Digest (S);"
            );
            let _ = writeln!(
                out,
                "            function Nib (C : Character) return Natural is"
            );
            let _ = writeln!(
                out,
                "              (if C in '0' .. '9' then Character'Pos (C) - Character'Pos ('0')"
            );
            let _ = writeln!(
                out,
                "               else Character'Pos (C) - Character'Pos ('a') + 10);"
            );
            let _ = writeln!(out, "         begin");
            let _ = writeln!(out, "            for I in 0 .. 15 loop");
            let _ = writeln!(
                out,
                "               Outk (I) := Byte (Nib (Hex (Hex'First + I * 2)) * 16 + Nib (Hex (Hex'First + I * 2 + 1)));"
            );
            let _ = writeln!(out, "            end loop;");
            let _ = writeln!(out, "         end;");
            let _ = writeln!(out, "      end;");
        } else {
            let _ = writeln!(out, "      for I in 0 .. Natural'Min (15, KW.Len - 1) loop");
            let _ = writeln!(out, "         Outk (I) := KW.Data (I);");
            let _ = writeln!(out, "      end loop;");
        }
        let _ = writeln!(out, "      return Outk;");
        let _ = writeln!(out, "   end Key_Hash;");
    }

    // Decode (inverse of Marshal_Into). The record is filled field-by-field.
    // @final reads inline, @appendable skips the DHEADER, @mutable skips DHEADER
    // then per member EMHEADER + NEXTINT (members in declaration order).
    let n = &sg.ada_name;
    let _ = writeln!(
        out,
        "\n   function Read_{n} (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return {n} is"
    );
    let _ = writeln!(out, "      V : {n};");
    let _ = writeln!(out, "   begin");
    if sg.mutable {
        let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
        for f in &sg.fields {
            let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
            let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
            let _ = writeln!(out, "      {}", f.get);
        }
    } else {
        if sg.appendable {
            let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
        }
        for f in &sg.fields {
            let _ = writeln!(out, "      {}", f.get);
        }
    }
    if sg.fields.is_empty() {
        let _ = writeln!(out, "      null;");
    }
    let _ = writeln!(out, "      return V;");
    let _ = writeln!(out, "   end Read_{n};");
    let _ = writeln!(
        out,
        "\n   function Unmarshal (Data : Byte_Array; Endian : Endianness) return {n} is"
    );
    let _ = writeln!(out, "      Pos : Natural := 0;");
    let _ = writeln!(out, "   begin");
    let _ = writeln!(out, "      return Read_{n} (Data, Pos, Endian);");
    let _ = writeln!(out, "   end Unmarshal;");
}

/// Maps an IDL union `switch` type to a `TypeSpec` so the discriminator reuses
/// the normal `map_type` path.
fn switch_typespec(s: &SwitchTypeSpec) -> TypeSpec {
    match s {
        SwitchTypeSpec::Integer(i) => TypeSpec::Primitive(PrimitiveType::Integer(*i)),
        SwitchTypeSpec::Char => TypeSpec::Primitive(PrimitiveType::Char),
        SwitchTypeSpec::Boolean => TypeSpec::Primitive(PrimitiveType::Boolean),
        SwitchTypeSpec::Octet => TypeSpec::Primitive(PrimitiveType::Octet),
        SwitchTypeSpec::Scoped(sn) => TypeSpec::Scoped(sn.clone()),
    }
}

/// A generated union case: integer labels (empty + is_default = `default`), the
/// member field, its Ada type, and its put statement.
struct UnionCaseAda {
    labels: Vec<i64>,
    is_default: bool,
    field: String,
    ada_type: String,
    put: String,
    get: String,
}

struct UnionGen {
    ada_name: String,
    disc_type: String,
    disc_put: String,
    disc_get: String,
    cases: Vec<UnionCaseAda>,
    appendable: bool,
}

fn build_union(
    u: &UnionDef,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<UnionGen> {
    let ext = lower_annotations(&u.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable);
    if ext == ExtensibilityKind::Mutable {
        return Err(IdlAdaError::Unsupported(format!(
            "@mutable union {} (EMHEADER framing not yet emitted)",
            u.name.text
        )));
    }
    let (disc_type, disc_put) = map_type(
        &switch_typespec(&u.switch_type),
        "V.disc",
        enum_names,
        struct_names,
    )?;
    let disc_get = map_get(
        &switch_typespec(&u.switch_type),
        "V.disc",
        enum_names,
        struct_names,
    )?;
    let mut cases = Vec::new();
    for c in &u.cases {
        let field = escape_ada_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ada_type, put) = map_type(&resolved, &format!("V.{field}"), enum_names, struct_names)?;
        let get = map_get(&resolved, &format!("V.{field}"), enum_names, struct_names)?;
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => labels.push(array_size(e).ok_or_else(|| {
                    IdlAdaError::Unsupported(format!(
                        "non-integer union label in `{}`",
                        u.name.text
                    ))
                })?),
            }
        }
        cases.push(UnionCaseAda {
            labels,
            is_default,
            field,
            ada_type,
            put,
            get,
        });
    }
    Ok(UnionGen {
        ada_name: escape_ada_ident(&u.name.text),
        disc_type,
        disc_put,
        disc_get,
        cases,
        appendable: ext == ExtensibilityKind::Appendable,
    })
}

/// Emits a union's body-local `Marshal_Into` (discriminator + `case` dispatch)
/// and its `Marshal` wrapper (XCDR2 §7.4.3.5.4).
fn emit_union_marshal(out: &mut String, ug: &UnionGen) {
    let _ = writeln!(
        out,
        "\n   procedure Marshal_Into (V : {}; W : in out Buf_T) is",
        ug.ada_name
    );
    if ug.appendable {
        let _ = writeln!(out, "      B : Buf_T;");
    }
    let _ = writeln!(out, "   begin");
    let wv = if ug.appendable {
        let _ = writeln!(out, "      B.Endian := W.Endian;");
        "B"
    } else {
        "W"
    };
    let _ = writeln!(out, "      {}", ug.disc_put.replace("$w", wv));
    let _ = writeln!(out, "      case V.disc is");
    let has_default = ug.cases.iter().any(|c| c.is_default);
    for c in &ug.cases {
        if c.is_default {
            let _ = writeln!(out, "         when others => {}", c.put.replace("$w", wv));
        } else {
            let labels = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(" | ");
            let _ = writeln!(out, "         when {labels} => {}", c.put.replace("$w", wv));
        }
    }
    if !has_default {
        let _ = writeln!(out, "         when others => null;");
    }
    let _ = writeln!(out, "      end case;");
    if ug.appendable {
        let _ = writeln!(out, "      Put_U32 (W, Unsigned_32 (B.Len));");
        let _ = writeln!(out, "      Append (W, B);");
    }
    let _ = writeln!(out, "   end Marshal_Into;");

    let _ = writeln!(
        out,
        "\n   function Marshal (V : {}; Endian : Endianness) return Byte_Array is",
        ug.ada_name
    );
    let _ = writeln!(out, "      W : Buf_T;");
    let _ = writeln!(out, "   begin");
    let _ = writeln!(out, "      W.Endian := Endian;");
    let _ = writeln!(out, "      Marshal_Into (V, W);");
    let _ = writeln!(out, "      return W.Data (0 .. W.Len - 1);");
    let _ = writeln!(out, "   end Marshal;");

    // Decode: read the discriminator, then a `case` reads only the selected
    // member (@appendable skips the leading DHEADER). Unread members stay default.
    let n = &ug.ada_name;
    let has_default = ug.cases.iter().any(|c| c.is_default);
    let _ = writeln!(
        out,
        "\n   function Read_{n} (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return {n} is"
    );
    let _ = writeln!(out, "      V : {n};");
    let _ = writeln!(out, "   begin");
    if ug.appendable {
        let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
    }
    let _ = writeln!(out, "      {}", ug.disc_get);
    let _ = writeln!(out, "      case V.disc is");
    for c in &ug.cases {
        if c.is_default {
            let _ = writeln!(out, "         when others => {}", c.get);
        } else {
            let labels = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(" | ");
            let _ = writeln!(out, "         when {labels} => {}", c.get);
        }
    }
    if !has_default {
        let _ = writeln!(out, "         when others => null;");
    }
    let _ = writeln!(out, "      end case;");
    let _ = writeln!(out, "      return V;");
    let _ = writeln!(out, "   end Read_{n};");
    let _ = writeln!(
        out,
        "\n   function Unmarshal (Data : Byte_Array; Endian : Endianness) return {n} is"
    );
    let _ = writeln!(out, "      Pos : Natural := 0;");
    let _ = writeln!(out, "   begin");
    let _ = writeln!(out, "      return Read_{n} (Data, Pos, Endian);");
    let _ = writeln!(out, "   end Unmarshal;");
}

/// Maps an IDL type to `(Ada type, put statement)`. The put uses `$w` as the
/// writer placeholder and `expr` as the value expression.
/// XTypes 1.3 §7.4.3: an Ada statement rejecting an over-bound
/// `string<N>`/`wstring<N>`/`sequence<T,N>`/`map<K,V,N>` value with
/// `Constraint_Error` — Ada's own "value violates a declared constraint"
/// exception, the natural house idiom for a bound violation (no existing
/// generated-code exception convention predates this; the sibling
/// C++/C#/Java backends this session each reused THEIR house exception —
/// `std::length_error` / `System.ArgumentException` / `IllegalArgumentException`
/// — so `Constraint_Error` is the Ada-idiomatic analogue). `prefix` is
/// `"bounded"` on encode (mirrors the pre-write check) or `"decoded"` on
/// decode (mirrors the post-read check, XTypes 1.3 §7.4.3 requires
/// enforcement on BOTH sides — decode only ever validated the wire's
/// remaining bytes, never the IDL-declared bound). Returns `Err` when the
/// bound is a non-literal `ConstExpr` — `array_size` cannot evaluate it,
/// matching how array sizes / union labels already fail elsewhere in this
/// backend rather than silently skipping enforcement.
fn bound_check_stmt(
    len_expr: &str,
    bound: &ConstExpr,
    prefix: &str,
    what: &str,
) -> Result<String> {
    let bv = array_size(bound).ok_or_else(|| {
        IdlAdaError::Unsupported(format!("non-literal bound on {what} `{len_expr}`"))
    })?;
    Ok(format!(
        "if {len_expr} > {bv} then raise Constraint_Error with \"{prefix} {what} length exceeds its IDL bound ({bv})\"; end if;"
    ))
}

fn map_type(
    t: &TypeSpec,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    match t {
        TypeSpec::Primitive(p) => map_primitive(*p, expr),
        TypeSpec::String(st) if !st.wide => {
            let put = match &st.bound {
                Some(b) => format!(
                    "{} Put_String ($w, To_String ({expr}));",
                    bound_check_stmt(&format!("Length ({expr})"), b, "bounded", "string")?
                ),
                None => format!("Put_String ($w, To_String ({expr}));"),
            };
            Ok(("Unbounded_String".to_string(), put))
        }
        TypeSpec::String(st) => {
            let put = match &st.bound {
                Some(b) => format!(
                    "{} Put_WString ($w, To_String ({expr}));",
                    bound_check_stmt(&format!("Length ({expr})"), b, "bounded", "wstring")?
                ),
                None => format!("Put_WString ($w, To_String ({expr}));"),
            };
            Ok(("Unbounded_String".to_string(), put))
        }
        TypeSpec::Sequence(seq) => map_sequence(&seq.elem, seq.bound.as_ref(), expr, struct_names),
        // A named enum (i32 wire) or a nested struct member (inline marshal).
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            let esc = escape_ada_ident(&name);
            if enum_names.contains(&name) {
                Ok((esc.clone(), format!("Put_U32 ($w, {esc}_To_U32 ({expr}));")))
            } else if struct_names.contains(&name) {
                Ok((esc, format!("Marshal_Into ({expr}, $w);")))
            } else {
                Err(IdlAdaError::Unsupported(format!("scoped type {name}")))
            }
        }
        other => Err(IdlAdaError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_primitive(p: PrimitiveType, expr: &str) -> Result<(String, String)> {
    let (ty, put) = match p {
        PrimitiveType::Octet => ("Unsigned_8", format!("Put_U8 ($w, {expr});")),
        PrimitiveType::Boolean => ("Boolean", format!("Put_Bool ($w, {expr});")),
        PrimitiveType::Char => (
            "Character",
            format!("Put_U8 ($w, Unsigned_8 (Character'Pos ({expr})));"),
        ),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => {
            ("IEEE_Float_32", format!("Put_F32 ($w, {expr});"))
        }
        PrimitiveType::Floating(FloatingType::Double) => {
            ("IEEE_Float_64", format!("Put_F64 ($w, {expr});"))
        }
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            ("IEEE_Float_64", format!("Put_Long_Double ($w, {expr});"))
        }
        PrimitiveType::WideChar => ("Unsigned_32", format!("Put_U32 ($w, {expr});")),
    };
    Ok((ty.to_string(), put))
}

fn map_integer(i: IntegerType, expr: &str) -> Result<(String, String)> {
    // Signed IDL integers use `'Mod` to reinterpret into the unsigned wire.
    let (ty, put) = match i {
        IntegerType::UInt8 => ("Unsigned_8", format!("Put_U8 ($w, {expr});")),
        IntegerType::Int8 => (
            "Integer_8",
            format!("Put_U8 ($w, Unsigned_8'Mod ({expr}));"),
        ),
        IntegerType::UShort | IntegerType::UInt16 => {
            ("Unsigned_16", format!("Put_U16 ($w, {expr});"))
        }
        IntegerType::Short | IntegerType::Int16 => (
            "Integer_16",
            format!("Put_U16 ($w, Unsigned_16'Mod ({expr}));"),
        ),
        IntegerType::ULong | IntegerType::UInt32 => {
            ("Unsigned_32", format!("Put_U32 ($w, {expr});"))
        }
        IntegerType::Long | IntegerType::Int32 => (
            "Integer_32",
            format!("Put_U32 ($w, Unsigned_32'Mod ({expr}));"),
        ),
        IntegerType::ULongLong | IntegerType::UInt64 => {
            ("Unsigned_64", format!("Put_U64 ($w, {expr});"))
        }
        IntegerType::LongLong | IntegerType::Int64 => (
            "Integer_64",
            format!("Put_U64 ($w, Unsigned_64'Mod ({expr}));"),
        ),
    };
    Ok((ty.to_string(), put))
}

fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        let check = bound
            .map(|b| bound_check_stmt(&format!("Length ({expr})"), b, "bounded", "sequence"))
            .transpose()?
            .map(|s| format!("{s} "))
            .unwrap_or_default();
        return Ok((
            "Unbounded_String".to_string(),
            format!("{check}Put_Seq_U8 ($w, To_String ({expr}));"),
        ));
    }
    // sequence<struct> → collection DHEADER + count + each element.
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let check = bound
                .map(|b| {
                    bound_check_stmt(
                        &format!("Natural ({expr}.Length)"),
                        b,
                        "bounded",
                        "sequence",
                    )
                })
                .transpose()?
                .map(|s| format!("{s} "))
                .unwrap_or_default();
            let put = format!(
                "{check}declare Sub : Buf_T; begin Sub.Endian := $w.Endian;                  Put_U32 (Sub, Unsigned_32 (Natural ({expr}.Length)));                  for E of {expr} loop Marshal_Into (E, Sub); end loop;                  Put_U32 ($w, Unsigned_32 (Sub.Len)); Append ($w, Sub); end;"
            );
            return Ok((format!("{}_Vectors.Vector", escape_ada_ident(&name)), put));
        }
    }
    Err(IdlAdaError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}

// ---- decode (inverse of the put path): a `Reader` (Get_* over `Data`/`Pos`) in
// the body, plus `map_get` — the inverse of `map_type` — emitting a statement
// that reads one value into the lvalue `target`. Records are mutable, so the
// value is filled field-by-field. Roundtrip-verified.

/// Reads a fixed array: nested row-major `for` loops assigning into the value
/// array `V.<field> (i0, i1)` (inverse of [`build_array_put`]). `elem_get`
/// targets the placeholder `$L`.
fn build_array_get(field: &str, sizes: &[i64], elem_get: &str) -> String {
    let idx = (0..sizes.len())
        .map(|k| format!("i{k}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut body = elem_get.replace("$L", &format!("V.{field} ({idx})"));
    for k in (0..sizes.len()).rev() {
        body = format!("for i{k} in 0 .. {} loop\n{body}\nend loop;", sizes[k] - 1);
    }
    body
}

/// Emits a statement reading one value of IDL type `t` into the lvalue `target`.
fn map_get(
    t: &TypeSpec,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    match t {
        TypeSpec::Primitive(p) => map_get_primitive(*p, target),
        TypeSpec::String(st) if !st.wide => {
            let read = format!("{target} := Get_String (Data, Pos, Endian);");
            match &st.bound {
                Some(b) => Ok(format!(
                    "{read} {}",
                    bound_check_stmt(&format!("Length ({target})"), b, "decoded", "string")?
                )),
                None => Ok(read),
            }
        }
        TypeSpec::String(st) => {
            let read = format!("{target} := Get_WString (Data, Pos, Endian);");
            match &st.bound {
                Some(b) => Ok(format!(
                    "{read} {}",
                    bound_check_stmt(&format!("Length ({target})"), b, "decoded", "wstring")?
                )),
                None => Ok(read),
            }
        }
        TypeSpec::Sequence(seq) => map_get_sequence(&seq.elem, seq.bound.as_ref(), target, struct_names),
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            let esc = escape_ada_ident(&name);
            if enum_names.contains(&name) {
                Ok(format!(
                    "{target} := {esc}_Of_U32 (Get_U32 (Data, Pos, Endian));"
                ))
            } else if struct_names.contains(&name) {
                Ok(format!("{target} := Read_{esc} (Data, Pos, Endian);"))
            } else {
                Err(IdlAdaError::Unsupported(format!("scoped type {name}")))
            }
        }
        other => Err(IdlAdaError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType, target: &str) -> Result<String> {
    let s = match p {
        PrimitiveType::Octet => format!("{target} := Get_U8 (Data, Pos);"),
        PrimitiveType::Char => {
            format!("{target} := Character'Val (Natural (Get_U8 (Data, Pos)));")
        }
        PrimitiveType::Boolean => format!("{target} := Get_Bool (Data, Pos);"),
        PrimitiveType::Integer(i) => return map_get_integer(i, target),
        PrimitiveType::Floating(FloatingType::Float) => {
            format!("{target} := Get_F32 (Data, Pos, Endian);")
        }
        PrimitiveType::Floating(FloatingType::Double) => {
            format!("{target} := Get_F64 (Data, Pos, Endian);")
        }
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            format!("{target} := Get_Long_Double (Data, Pos, Endian);")
        }
        PrimitiveType::WideChar => format!("{target} := Get_U32 (Data, Pos, Endian);"),
    };
    Ok(s)
}

fn map_get_integer(i: IntegerType, target: &str) -> Result<String> {
    let s = match i {
        IntegerType::UInt8 => format!("{target} := Get_U8 (Data, Pos);"),
        IntegerType::Int8 => format!("{target} := U8_I8 (Get_U8 (Data, Pos));"),
        IntegerType::UShort | IntegerType::UInt16 => {
            format!("{target} := Get_U16 (Data, Pos, Endian);")
        }
        IntegerType::Short | IntegerType::Int16 => {
            format!("{target} := U16_I16 (Get_U16 (Data, Pos, Endian));")
        }
        IntegerType::ULong | IntegerType::UInt32 => {
            format!("{target} := Get_U32 (Data, Pos, Endian);")
        }
        IntegerType::Long | IntegerType::Int32 => {
            format!("{target} := U32_I32 (Get_U32 (Data, Pos, Endian));")
        }
        IntegerType::ULongLong | IntegerType::UInt64 => {
            format!("{target} := Get_U64 (Data, Pos, Endian);")
        }
        IntegerType::LongLong | IntegerType::Int64 => {
            format!("{target} := U64_I64 (Get_U64 (Data, Pos, Endian));")
        }
    };
    Ok(s)
}

fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    target: &str,
    struct_names: &HashSet<String>,
) -> Result<String> {
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        let read = format!("{target} := Get_Seq_U8 (Data, Pos, Endian);");
        return match bound {
            Some(b) => Ok(format!(
                "{read} {}",
                bound_check_stmt(&format!("Length ({target})"), b, "decoded", "sequence")?
            )),
            None => Ok(read),
        };
    }
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let check = bound
                .map(|b| bound_check_stmt("Zn", b, "decoded", "sequence"))
                .transpose()?
                .map(|s| format!("{s} "))
                .unwrap_or_default();
            let esc = escape_ada_ident(&name);
            return Ok(format!(
                "declare Zn : Natural; begin Skip_U32 (Data, Pos, Endian); Zn := Natural (Get_U32 (Data, Pos, Endian)); {check}{target}.Clear; for Zi in 1 .. Zn loop {target}.Append (Read_{esc} (Data, Pos, Endian)); end loop; end;"
            ));
        }
    }
    Err(IdlAdaError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}
