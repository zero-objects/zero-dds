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
    BinaryOp, BitmaskDecl, BitsetDecl, CaseLabel, ConstDecl, ConstExpr, ConstType, ConstrTypeDecl,
    Declarator, Definition, EnumDef, Export, FixedPtType, FloatingType, IntegerType, InterfaceDcl,
    Literal, LiteralKind, Member, ModuleDef, PrimitiveType, ScopedName, SequenceType,
    Specification, StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnaryOp, UnionDcl,
    UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, PlacementKind, enum_bit_bound, enum_wire_octets,
    lower_annotations, lower_single,
};

use crate::error::{IdlAdaError, Result};
use crate::keywords::escape_ada_ident;

thread_local! {
    /// Fully-qualified IDL scope path of every named type declaration
    /// (e.g. `["a", "Reading"]`), populated by [`register_type_paths`] at the
    /// start of each run. A reference site resolves a (possibly partially
    /// qualified) `ScopedName` against the enclosing module scope by walking
    /// outward and matching one of these paths (§7.5.2), then flattens the
    /// match the SAME way [`qualify`] flattens the definition (#21).
    static TYPE_PATHS: std::cell::RefCell<Vec<Vec<String>>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Module scope of the aggregate currently being built. Set at the top of
    /// [`build_struct`]/[`build_union`]; empty at global scope.
    static CURRENT_SCOPE: std::cell::RefCell<Vec<String>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Flattened qualified enum name → signed wire holder width in OCTETS
    /// (1/2/4), from `@bit_bound` (XTypes 1.3 §7.3.1.2.1.9 + §7.4.5.1) via the
    /// shared [`enum_wire_octets`]. Populated once per run; read at the single
    /// enum encode/decode site so a `@bit_bound(8)`/`@bit_bound(16)` enum
    /// narrows to 1/2 bytes instead of the former fixed 4.
    static ENUM_WIDTHS: std::cell::RefCell<HashMap<String, u32>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Signed wire holder width in octets (1/2/4) an enum named `name` serializes
/// at, per its `@bit_bound`. Defaults to 4 for an unregistered name / no
/// `@bit_bound` (XTypes 1.3 §7.4.5.1 default bound 32).
fn enum_wire_width(name: &str) -> u32 {
    ENUM_WIDTHS
        .with(|m| m.borrow().get(name).copied())
        .unwrap_or(4)
}

/// Injective flattened name for a declaration `simple` in module `scope`, via
/// the shared [`zerodds_idl::naming::encode_scoped`] encoding. The scope
/// separator (`_s`) and a literal underscore in a name (`_u`) are distinct, so
/// `A::B_C` and `A_B::C` no longer collapse to `A_B_C` (unlike the old
/// `join("_")`, which was NOT collision-free). Two same-simple-name types in
/// different modules become distinct types (`a_sReading`/`b_sReading`, #21).
fn qualify(scope: &[String], simple: &str) -> String {
    zerodds_idl::naming::encode_scoped(scope, simple)
}

/// Case-insensitive identifier uniquifier for the components of a single Ada
/// aggregate (record). Ada identifiers are case-insensitive (RM §2.3), so two
/// IDL members differing only in case — `value`/`Value`, or a case-branch named
/// `Disc` alongside the synthetic `disc` discriminator — would collapse to the
/// same Ada component and raise a "duplicate component" error. Each colliding
/// name gets a `_U`/`_U_U`/… suffix (single underscore, never trailing/doubled,
/// so it stays a legal Ada identifier) until it is unique. Non-colliding names
/// pass through unchanged, so existing single-case goldens are untouched.
struct CiDedup {
    seen: HashSet<String>,
}

impl CiDedup {
    fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    /// Reserves `name` verbatim (used to seed a fixed component such as the
    /// union `disc` so a same-spelled branch is the one that gets suffixed).
    fn reserve(&mut self, name: &str) {
        self.seen.insert(name.to_ascii_lowercase());
    }

    /// Returns an Ada component name for `name` unique (case-insensitively)
    /// among all names handed out by this instance so far.
    fn unique(&mut self, name: &str) -> String {
        let mut cand = name.to_string();
        while !self.seen.insert(cand.to_ascii_lowercase()) {
            cand.push_str("_U");
        }
        cand
    }
}

/// Records the fully-qualified path of every named type declaration before
/// emission, so reference resolution can flatten a name the same way the
/// definition site does.
/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn register_type_paths(defs: &[Definition], scope: &mut Vec<String>) {
    for def in defs {
        match def {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                register_type_paths(&m.definitions, scope);
                scope.pop();
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                push_type_path(scope, &s.name.text);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                push_type_path(scope, &e.name.text);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                push_type_path(scope, &u.name.text);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
                push_type_path(scope, &b.name.text);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(m))) => {
                push_type_path(scope, &m.name.text);
            }
            Definition::Type(TypeDecl::Typedef(td)) => {
                for d in &td.declarators {
                    push_type_path(scope, &d.name().text);
                }
            }
            _ => {}
        }
    }
}

fn push_type_path(scope: &[String], simple: &str) {
    let mut path = scope.to_vec();
    path.push(simple.to_string());
    TYPE_PATHS.with(|t| t.borrow_mut().push(path));
}

/// Resolves a referenced `ScopedName` against [`CURRENT_SCOPE`], returning the
/// flattened logical name (`join("_")`) of the matching declaration. Mirrors
/// IDL name lookup (§7.5.2): for each prefix of the enclosing scope (longest
/// first), then the global scope, check whether `prefix + parts` is a known
/// type path. Falls back to the literal flattening of the written parts.
fn resolve_scoped_name(sn: &ScopedName) -> String {
    let parts: Vec<String> = sn.parts.iter().map(|p| p.text.clone()).collect();
    let scope = CURRENT_SCOPE.with(|s| s.borrow().clone());
    let known: Vec<Vec<String>> = TYPE_PATHS.with(|t| t.borrow().clone());
    for cut in (0..=scope.len()).rev() {
        let mut cand = scope[..cut].to_vec();
        cand.extend(parts.iter().cloned());
        if known.contains(&cand) {
            return flatten_path(&cand);
        }
    }
    flatten_path(&parts)
}

/// Flattens a full type path (module components + simple name) exactly as the
/// definition site does via [`qualify`]/[`zerodds_idl::naming::encode_scoped`],
/// so a reference resolves to the same identifier the declaration emitted.
fn flatten_path(path: &[String]) -> String {
    match path.split_last() {
        Some((simple, scope)) => zerodds_idl::naming::encode_scoped(scope, simple),
        None => String::new(),
    }
}

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
/// yet emit (e.g. `@mutable` unions and non-literal array/sequence bounds).
pub fn generate_ada_module(spec: &Specification, opts: &AdaGenOptions) -> Result<AdaModule> {
    let pkg = &opts.package_name;

    // Rewrite interfaces into modules holding their type/const exports so
    // interface-nested declarations are emitted like any other type (§7.4.7).
    let definitions = expand_interfaces(&spec.definitions);

    // Register every named type's fully-qualified path so reference sites can
    // resolve a `ScopedName` against its enclosing scope (#21 cross-module).
    TYPE_PATHS.with(|t| t.borrow_mut().clear());
    register_type_paths(&definitions, &mut Vec::new());

    // `module X { ... }` content is promoted to the top level, each definition
    // paired with its module scope path (see `flatten_module_defs`).
    let flat = flatten_module_defs(&definitions);

    // Keyed by the flattened module-qualified *raw* IDL name (not the
    // Ada-escaped `ada_name`) so `Scoped` type-reference lookups below — which
    // resolve raw AST text via `resolve_scoped_name` — match regardless of
    // keyword escaping. `a::Reading` and `b::Reading` become distinct keys
    // `a_Reading`/`b_Reading` (#21).
    let enum_names: HashSet<String> = flat
        .iter()
        .filter_map(|(scope, d)| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                Some(qualify(scope, &e.name.text))
            }
            _ => None,
        })
        .collect();
    let enums: Vec<EnumGen> = flat
        .iter()
        .filter_map(|(scope, d)| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                Some(build_enum(e, scope))
            }
            _ => None,
        })
        .collect();
    // Register each enum's @bit_bound-derived wire width (1/2/4 octets), P1.
    ENUM_WIDTHS.with(|m| {
        let mut m = m.borrow_mut();
        m.clear();
        for (scope, d) in &flat {
            if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) = d {
                m.insert(
                    qualify(scope, &e.name.text),
                    u32::from(enum_wire_octets(enum_bit_bound(&e.annotations))),
                );
            }
        }
    });

    let real_struct_names: HashSet<String> = flat
        .iter()
        .filter_map(|(scope, d)| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                Some(qualify(scope, &s.name.text))
            }
            _ => None,
        })
        .collect();

    // Bitsets/bitmasks are emitted as "pseudo-structs": a record with a single
    // backing-integer `storage`, a `Marshal_Into`/`Read_<name>` pair, plus bit
    // accessors (bitset) or OR-able value constants (bitmask). Registering their
    // names alongside the real structs lets a member reference resolve through
    // the SAME `map_type`/`map_get` struct path (`Marshal_Into`/`Read_`).
    let bitsets: Vec<BitsetGen> = flat
        .iter()
        .filter_map(|(scope, d)| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
                Some(build_bitset(b, scope))
            }
            _ => None,
        })
        .collect::<Result<Vec<_>>>()?;
    let bitmasks: Vec<BitmaskGen> = flat
        .iter()
        .filter_map(|(scope, d)| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(m))) => {
                Some(build_bitmask(m, scope))
            }
            _ => None,
        })
        .collect::<Result<Vec<_>>>()?;

    // Names resolvable as a marshalable aggregate (`Marshal_Into`/`Read_`):
    // real structs plus the bitset/bitmask pseudo-structs. Passed everywhere a
    // struct-reference lookup happens; `struct_defs` (nested-`@key` expansion)
    // stays real-structs-only so a bitset key member falls to the generic put.
    let mut struct_names = real_struct_names.clone();
    for b in &bitsets {
        struct_names.insert(b.raw_name.clone());
    }
    for m in &bitmasks {
        struct_names.insert(m.raw_name.clone());
    }

    let typedefs = collect_typedefs(&definitions);
    // struct qualified-name → def, so a nested-struct `@key` member's own
    // `@key` subset can be resolved for Key_Hash emission (Bug A) and for the
    // static MD5-vs-zero-pad branch decision (Bug B) — mirrors `collect_typedefs`.
    let struct_defs = collect_structs(&definitions);

    let mut structs: Vec<StructGen> = Vec::new();
    let mut unions: Vec<UnionGen> = Vec::new();
    for (scope, def) in &flat {
        match def {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                structs.push(build_struct(
                    s,
                    scope,
                    &enum_names,
                    &struct_names,
                    &typedefs,
                    &struct_defs,
                )?);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                unions.push(build_union(
                    u,
                    scope,
                    &enum_names,
                    &struct_names,
                    &typedefs,
                )?);
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
    // A `sequence<T>` whose element `T` is a primitive/enum/string (not a
    // marshalable aggregate) needs a generic `Ada.Containers.Vectors` instance
    // keyed by the element's Ada type (e.g. `Integer_32_Vectors`). These are
    // emitted once, after the enum declarations and before any record that uses
    // them (their element types — Interfaces integers, enums, Unbounded_String
    // — are all already in scope there). Aggregate-element vectors keep being
    // emitted next to their (pseudo-)struct record.
    let mut elem_vector_types: Vec<String> = vectors_used
        .iter()
        .filter(|n| !struct_names.contains(*n))
        .cloned()
        .collect();
    elem_vector_types.sort();

    let any_map = structs.iter().any(|sg| !sg.map_packages.is_empty());

    // Distinct `fixed<P,S>` layouts used anywhere → one `Fixed_<P>_<S>` subtype
    // each, plus the shared packed-BCD prelude (emitted once).
    let mut fixed_layouts: Vec<(u32, u32)> = Vec::new();
    for sg in &structs {
        for f in &sg.fields {
            if let Some(ps) = f.fixed_layout {
                if !fixed_layouts.contains(&ps) {
                    fixed_layouts.push(ps);
                }
            }
        }
    }
    for ug in &unions {
        for c in &ug.cases {
            if let Some(ps) = c.fixed_layout {
                if !fixed_layouts.contains(&ps) {
                    fixed_layouts.push(ps);
                }
            }
        }
    }
    fixed_layouts.sort_unstable();
    let any_fixed = !fixed_layouts.is_empty();

    // File-scope `@verbatim` blocks (BEGIN_FILE / END_FILE), gathered from every
    // top-level declaration's annotations in document order (§8.3.5.1).
    let verbatim_begin_file = collect_file_verbatim(&flat, PlacementKind::BeginFile);
    let verbatim_end_file = collect_file_verbatim(&flat, PlacementKind::EndFile);

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
    // The package body is always emitted (it carries the wire helpers). Ada
    // (RM §7.2) only permits a body when the spec "requires" one — i.e. declares
    // a subprogram. A spec with only types/constants (enum-only, const-only, or
    // a lone empty struct with nothing to marshal declared as a subprogram)
    // needs `pragma Elaborate_Body` so its body is legal. Structs, unions,
    // bitsets, bitmasks, and `fixed<>` all declare subprograms in the spec.
    let spec_declares_subprogram = !structs.is_empty()
        || !unions.is_empty()
        || !bitsets.is_empty()
        || !bitmasks.is_empty()
        || any_fixed;
    if !spec_declares_subprogram {
        let _ = writeln!(spec_src, "   pragma Elaborate_Body;\n");
    }
    for line in &verbatim_begin_file {
        let _ = writeln!(spec_src, "   {line}");
    }
    let _ = writeln!(spec_src, "   type Byte is new Interfaces.Unsigned_8;");
    let _ = writeln!(
        spec_src,
        "   type Byte_Array is array (Natural range <>) of Byte;"
    );
    let _ = writeln!(spec_src, "   type Endianness is (Little, Big);\n");
    // `fixed<P,S>`: packed-BCD storage of `(P + 2) / 2` octets (CORBA/GIOP
    // §9.3.2.7 ≡ XCDR2 §7.4.4.5); one constrained subtype per distinct layout.
    for (p, s) in &fixed_layouts {
        let n = fixed_byte_len(*p);
        let _ = writeln!(
            spec_src,
            "   subtype Fixed_{p}_{s} is Byte_Array (0 .. {});",
            n - 1
        );
    }
    if any_fixed {
        let _ = writeln!(
            spec_src,
            "   function Fixed_From_String (Img : String; P : Natural; S : Natural) return Byte_Array;"
        );
        let _ = writeln!(
            spec_src,
            "   function Fixed_To_String (Bytes : Byte_Array; S : Natural) return String;\n"
        );
    }
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
    // `const` declarations (§7.4.1.4.4) as Ada named constants, emitted after
    // the enum types so an enum-typed constant's type is in scope. Previously
    // every `const` fell through the definition dispatch and was dropped.
    let mut any_const = false;
    for (scope, def) in &flat {
        if let Definition::Const(cd) = def {
            let _ = writeln!(spec_src, "   {}", render_const_decl(scope, cd)?);
            any_const = true;
        }
    }
    if any_const {
        let _ = writeln!(spec_src);
    }
    // Generic vector instances for `sequence<primitive|enum|string>` members.
    for t in &elem_vector_types {
        let _ = writeln!(
            spec_src,
            "   package {t}_Vectors is new Ada.Containers.Vectors (Natural, {t});"
        );
    }
    if !elem_vector_types.is_empty() {
        let _ = writeln!(spec_src);
    }
    // Bitset/bitmask pseudo-structs (record + backing int + accessors/consts).
    for bg in &bitsets {
        emit_bitset_spec(&mut spec_src, bg, &vectors_used);
    }
    for mg in &bitmasks {
        emit_bitmask_spec(&mut spec_src, mg, &vectors_used);
    }
    for sg in &structs {
        for line in &sg.verbatim_before {
            let _ = writeln!(spec_src, "   {line}");
        }
        for ot in &sg.opt_types {
            let _ = writeln!(spec_src, "   {ot}");
        }
        for at in &sg.array_types {
            let _ = writeln!(spec_src, "   {at}");
        }
        for mp in &sg.map_packages {
            let _ = writeln!(spec_src, "   {mp}");
        }
        // An IDL struct with no members (or one whose members all vanished)
        // maps to a component-less Ada record. `record end record;` with no
        // component list is a GNAT syntax error, so emit the `null record`
        // form (Ada RM §3.8) — the byte-identical empty-aggregate wire.
        if sg.fields.is_empty() {
            let _ = writeln!(spec_src, "   type {} is null record;", sg.ada_name);
        } else {
            let _ = writeln!(spec_src, "   type {} is record", sg.ada_name);
            for f in &sg.fields {
                let _ = writeln!(spec_src, "      {} : {};", f.ada_name, f.ada_type);
            }
            let _ = writeln!(spec_src, "   end record;");
        }
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
        for line in &sg.verbatim_after {
            let _ = writeln!(spec_src, "   {line}");
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
    for line in &verbatim_end_file {
        let _ = writeln!(spec_src, "   {line}");
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
    if any_fixed {
        body_src.push_str(FIXED_BODY);
    }
    for eg in &enums {
        emit_enum_to_u32(&mut body_src, eg);
        emit_u32_to_enum(&mut body_src, eg);
    }
    for bg in &bitsets {
        emit_bitset_body(&mut body_src, bg);
    }
    for mg in &bitmasks {
        emit_bitmask_body(&mut body_src, mg);
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
fn build_enum(e: &EnumDef, scope: &[String]) -> EnumGen {
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
        ada_name: escape_ada_ident(&qualify(scope, &e.name.text)),
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

/// `true` if the member carries `@optional` (XTypes 1.3 §7.4.5.1.4).
fn member_is_optional(anns: &[zerodds_idl::ast::types::Annotation]) -> bool {
    lower_annotations(anns)
        .map(|l| {
            l.builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::Optional))
        })
        .unwrap_or(false)
}

/// Packed-BCD octet count of a `fixed<P,S>` = `(P + 2) / 2` (CORBA §9.3.2.7).
fn fixed_byte_len(p: u32) -> u32 {
    (p + 2) / 2
}

/// Evaluates a `fixed<P,S>`'s digit count and scale from its const-expr fields.
fn fixed_ps(f: &FixedPtType) -> Result<(u32, u32)> {
    let p = array_size(&f.digits)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| IdlAdaError::Unsupported("non-literal fixed<> digit count".to_string()))?;
    let s = array_size(&f.scale)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| IdlAdaError::Unsupported("non-literal fixed<> scale".to_string()))?;
    Ok((p, s))
}

/// The Ada `Interfaces.Unsigned_*` backing type + `Put_*`/`Get_*` suffix for a
/// bit width (bitset total width / bitmask `@bit_bound`). ≤8 → U8, ≤16 → U16,
/// ≤32 → U32, else U64 (mirrors `idl-rust`'s `bitset_storage_type`).
fn bit_storage(total_bits: u32) -> (&'static str, &'static str) {
    match total_bits {
        0..=8 => ("Unsigned_8", "U8"),
        9..=16 => ("Unsigned_16", "U16"),
        17..=32 => ("Unsigned_32", "U32"),
        _ => ("Unsigned_64", "U64"),
    }
}

/// The packed-BCD `fixed<P,S>` codec, emitted into the package body once when
/// any `fixed` layout is present. `Fixed_From_String`/`Fixed_To_String` convert
/// between a decimal string and the CORBA/GIOP §9.3.2.7 packed-BCD octets; the
/// wire (`Put_Fixed`/`Get_Fixed`) is those raw octets, no length prefix
/// (XCDR2 §7.4.4.5, byte count statically known from P).
const FIXED_BODY: &str = r#"   function Fixed_From_String (Img : String; P : Natural; S : Natural) return Byte_Array is
      N          : constant Natural := (P + 2) / 2;
      Result     : Byte_Array (0 .. N - 1) := (others => 0);
      Sign_Pos   : Boolean := True;
      First      : Natural := Img'First;
      Last       : constant Natural := Img'Last;
      Dot        : Integer := -1;
      Int_Needed : constant Natural := P - S;
      Digits_Buf : String (1 .. P) := (others => '0');
      Int_Last   : Natural;
      Int_Len    : Natural;
   begin
      if First <= Last and then (Img (First) = '-' or Img (First) = '+') then
         Sign_Pos := Img (First) /= '-';
         First := First + 1;
      end if;
      for I in First .. Last loop
         if Img (I) = '.' then
            Dot := I;
         end if;
      end loop;
      Int_Last := (if Dot >= 0 then Dot - 1 else Last);
      Int_Len := (if Int_Last >= First then Int_Last - First + 1 else 0);
      for K in 0 .. Int_Len - 1 loop
         Digits_Buf (Int_Needed - Int_Len + 1 + K) := Img (First + K);
      end loop;
      if Dot >= 0 then
         for K in 0 .. Last - Dot - 1 loop
            Digits_Buf (Int_Needed + 1 + K) := Img (Dot + 1 + K);
         end loop;
      end if;
      declare
         Nibbles : array (0 .. N * 2 - 1) of Unsigned_8 := (others => 0);
         Idx     : Natural := 0;
      begin
         if (P + 1) mod 2 = 1 then
            Idx := Idx + 1;
         end if;
         for K in 1 .. P loop
            Nibbles (Idx) :=
              Unsigned_8 (Character'Pos (Digits_Buf (K)) - Character'Pos ('0'));
            Idx := Idx + 1;
         end loop;
         Nibbles (Idx) := (if Sign_Pos then 16#0C# else 16#0D#);
         for B in 0 .. N - 1 loop
            Result (B) :=
              Byte (Shift_Left (Nibbles (2 * B), 4) or Nibbles (2 * B + 1));
         end loop;
      end;
      return Result;
   end Fixed_From_String;

   function Fixed_To_String (Bytes : Byte_Array; S : Natural) return String is
      Chars : String (1 .. Bytes'Length * 2) := (others => '0');
      Neg   : Boolean := False;
      Cnt   : Natural := 0;
   begin
      for I in Bytes'Range loop
         declare
            Hi : constant Natural :=
              Natural (Shift_Right (Unsigned_8 (Bytes (I)), 4) and 16#0F#);
            Lo : constant Natural := Natural (Unsigned_8 (Bytes (I)) and 16#0F#);
         begin
            Cnt := Cnt + 1;
            Chars (Cnt) := Character'Val (Character'Pos ('0') + Hi);
            if I = Bytes'Last then
               Neg := Lo = 16#0D#;
            else
               Cnt := Cnt + 1;
               Chars (Cnt) := Character'Val (Character'Pos ('0') + Lo);
            end if;
         end;
      end loop;
      declare
         Start : Natural := 1;
      begin
         while Cnt - Start + 1 > S + 1 and then Chars (Start) = '0' loop
            Start := Start + 1;
         end loop;
         declare
            Dot_At : constant Natural := (Cnt - Start + 1) - S;
            Out_S  : String
              (1 .. (Cnt - Start + 1)
                    + (if S > 0 then 1 else 0)
                    + (if Neg then 1 else 0));
            O : Natural := 0;
         begin
            if Neg then
               O := O + 1;
               Out_S (O) := '-';
            end if;
            for K in 0 .. Cnt - Start loop
               if S > 0 and then K = Dot_At then
                  O := O + 1;
                  Out_S (O) := '.';
               end if;
               O := O + 1;
               Out_S (O) := Chars (Start + K);
            end loop;
            return Out_S (1 .. O);
         end;
      end;
   end Fixed_To_String;

   procedure Put_Fixed (W : in out Buf_T; V : Byte_Array) is
   begin
      for I in V'Range loop
         W.Data (W.Len) := V (I);
         W.Len := W.Len + 1;
      end loop;
   end Put_Fixed;

   function Get_Fixed (Data : Byte_Array; Pos : in out Natural; N : Positive) return Byte_Array is
      R : Byte_Array (0 .. N - 1);
   begin
      for I in 0 .. N - 1 loop
         R (I) := Data (Data'First + Pos + I);
      end loop;
      Pos := Pos + N;
      return R;
   end Get_Fixed;
"#;

/// A generated bitset: an Ada record wrapping a backing integer, plus bit
/// getters/setters (XTypes 1.3 §7.4.7). The wire form is the backing integer.
struct BitsetGen {
    raw_name: String,
    ada_name: String,
    storage_type: &'static str,
    put_suffix: &'static str,
    /// `(field_name, offset, width)` for each *named* bitfield.
    fields: Vec<(String, u32, u32)>,
}

/// A generated bitmask: an Ada record wrapping a backing integer sized by
/// `@bit_bound` (default 32), plus one OR-able constant per bit value.
struct BitmaskGen {
    raw_name: String,
    ada_name: String,
    storage_type: &'static str,
    put_suffix: &'static str,
    /// `(const_name, bit_position)`.
    values: Vec<(String, u32)>,
}

/// zerodds-lint: recursion-depth 32
fn const_expr_u32(e: &ConstExpr) -> Option<u32> {
    array_size(e).and_then(|v| u32::try_from(v).ok())
}

fn build_bitset(b: &BitsetDecl, scope: &[String]) -> Result<BitsetGen> {
    let mut total: u32 = 0;
    let mut fields = Vec::new();
    for bf in &b.bitfields {
        let width = const_expr_u32(&bf.spec.width).ok_or_else(|| {
            IdlAdaError::Unsupported(format!(
                "non-integer bitfield width in bitset {}",
                b.name.text
            ))
        })?;
        if let Some(name) = &bf.name {
            fields.push((escape_ada_ident(&name.text), total, width));
        }
        total += width;
    }
    let (storage_type, put_suffix) = bit_storage(total.max(1));
    let raw_name = qualify(scope, &b.name.text);
    Ok(BitsetGen {
        ada_name: escape_ada_ident(&raw_name),
        raw_name,
        storage_type,
        put_suffix,
        fields,
    })
}

/// The default bitmask holder width is `@bit_bound` (XTypes §7.3.1.2.1.1),
/// default 32 — NOT the declared-bit count. An unannotated bitmask is a uint32.
fn bitmask_bit_bound(anns: &[zerodds_idl::ast::types::Annotation]) -> u32 {
    lower_annotations(anns)
        .ok()
        .and_then(|l| {
            l.builtins.iter().find_map(|a| match a {
                BuiltinAnnotation::BitBound(n) => Some(u32::from(*n)),
                _ => None,
            })
        })
        .unwrap_or(32)
}

fn build_bitmask(m: &BitmaskDecl, scope: &[String]) -> Result<BitmaskGen> {
    let (storage_type, put_suffix) = bit_storage(bitmask_bit_bound(&m.annotations));
    let mut values = Vec::new();
    for (idx, v) in m.values.iter().enumerate() {
        let pos = lower_annotations(&v.annotations)
            .ok()
            .and_then(|l| {
                l.builtins.iter().find_map(|a| match a {
                    BuiltinAnnotation::Position(p) => Some(*p),
                    _ => None,
                })
            })
            .unwrap_or(idx as u32);
        values.push((escape_ada_ident(&v.name.text), pos));
    }
    let raw_name = qualify(scope, &m.name.text);
    Ok(BitmaskGen {
        ada_name: escape_ada_ident(&raw_name),
        raw_name,
        storage_type,
        put_suffix,
        values,
    })
}

fn emit_bitset_spec(out: &mut String, bg: &BitsetGen, vectors_used: &HashSet<String>) {
    let n = &bg.ada_name;
    let st = bg.storage_type;
    let _ = writeln!(out, "   type {n} is record");
    let _ = writeln!(out, "      storage : {st} := 0;");
    let _ = writeln!(out, "   end record;");
    for (field, offset, width) in &bg.fields {
        if *width == 1 {
            let _ = writeln!(
                out,
                "   function {field} (V : {n}) return Boolean is \
                 ((Shift_Right (V.storage, {offset}) and 1) /= 0);"
            );
        } else {
            let mask = (1u128 << width) - 1;
            let _ = writeln!(
                out,
                "   function {field} (V : {n}) return {st} is \
                 (Shift_Right (V.storage, {offset}) and {mask});"
            );
        }
        let ty = if *width == 1 { "Boolean" } else { st };
        let _ = writeln!(
            out,
            "   procedure Set_{field} (V : in out {n}; Val : {ty});"
        );
    }
    emit_pseudo_struct_ops_decl(out, n, vectors_used);
}

fn emit_bitmask_spec(out: &mut String, mg: &BitmaskGen, vectors_used: &HashSet<String>) {
    let n = &mg.ada_name;
    let st = mg.storage_type;
    let _ = writeln!(out, "   type {n} is record");
    let _ = writeln!(out, "      storage : {st} := 0;");
    let _ = writeln!(out, "   end record;");
    for (name, pos) in &mg.values {
        let bit: u128 = 1u128 << pos;
        let _ = writeln!(out, "   {name} : constant {n} := (storage => {bit});");
    }
    let _ = writeln!(
        out,
        "   function \"or\" (L, R : {n}) return {n} is ((storage => L.storage or R.storage));"
    );
    let _ = writeln!(
        out,
        "   function \"and\" (L, R : {n}) return {n} is ((storage => L.storage and R.storage));"
    );
    let _ = writeln!(
        out,
        "   function Bits (V : {n}) return {st} is (V.storage);"
    );
    emit_pseudo_struct_ops_decl(out, n, vectors_used);
}

/// Common spec declarations for a pseudo-struct (bitset/bitmask): an optional
/// `_Vectors` instance (when used as a `sequence<>` element) plus the
/// `Marshal`/`Unmarshal` pair (the `Marshal_Into`/`Read_` body operations let a
/// member reference reuse the struct `map_type`/`map_get` path).
fn emit_pseudo_struct_ops_decl(out: &mut String, n: &str, vectors: &HashSet<String>) {
    if vectors.contains(n) {
        let _ = writeln!(
            out,
            "   package {n}_Vectors is new Ada.Containers.Vectors (Natural, {n});"
        );
    }
    let _ = writeln!(
        out,
        "   function Marshal (V : {n}; Endian : Endianness) return Byte_Array;"
    );
    let _ = writeln!(
        out,
        "   function Unmarshal (Data : Byte_Array; Endian : Endianness) return {n};\n"
    );
}

fn emit_bitset_body(out: &mut String, bg: &BitsetGen) {
    let n = &bg.ada_name;
    let st = bg.storage_type;
    for (field, offset, width) in &bg.fields {
        let ty = if *width == 1 { "Boolean" } else { st };
        let _ = writeln!(
            out,
            "\n   procedure Set_{field} (V : in out {n}; Val : {ty}) is"
        );
        if *width == 1 {
            let _ = writeln!(
                out,
                "      Mask : constant {st} := Shift_Left ({st} (1), {offset});"
            );
            let _ = writeln!(out, "   begin");
            let _ = writeln!(out, "      if Val then");
            let _ = writeln!(out, "         V.storage := V.storage or Mask;");
            let _ = writeln!(out, "      else");
            let _ = writeln!(out, "         V.storage := V.storage and not Mask;");
            let _ = writeln!(out, "      end if;");
        } else {
            let mask = (1u128 << width) - 1;
            let _ = writeln!(
                out,
                "      Mask : constant {st} := Shift_Left ({st} ({mask}), {offset});"
            );
            let _ = writeln!(out, "   begin");
            let _ = writeln!(
                out,
                "      V.storage := (V.storage and not Mask) or Shift_Left (Val and {mask}, {offset});"
            );
        }
        let _ = writeln!(out, "   end Set_{field};");
    }
    emit_pseudo_struct_body(out, n, st, bg.put_suffix);
}

fn emit_bitmask_body(out: &mut String, mg: &BitmaskGen) {
    emit_pseudo_struct_body(out, &mg.ada_name, mg.storage_type, mg.put_suffix);
}

/// The `Marshal_Into`/`Marshal`/`Read_`/`Unmarshal` body for a pseudo-struct:
/// the backing integer is written/read directly (the bitset/bitmask wire form).
fn emit_pseudo_struct_body(out: &mut String, n: &str, st: &str, suffix: &str) {
    let _ = writeln!(
        out,
        "\n   procedure Marshal_Into (V : {n}; W : in out Buf_T) is"
    );
    let _ = writeln!(out, "   begin");
    // Put_U8 takes no endianness; the wider Put_U16/32/64 read it from W.
    let _ = writeln!(out, "      Put_{suffix} (W, V.storage);");
    let _ = writeln!(out, "   end Marshal_Into;");
    let _ = writeln!(
        out,
        "\n   function Marshal (V : {n}; Endian : Endianness) return Byte_Array is"
    );
    let _ = writeln!(out, "      W : Buf_T;");
    let _ = writeln!(out, "   begin");
    let _ = writeln!(out, "      W.Endian := Endian;");
    let _ = writeln!(out, "      Marshal_Into (V, W);");
    let _ = writeln!(out, "      return W.Data (0 .. W.Len - 1);");
    let _ = writeln!(out, "   end Marshal;");
    let get_call = if suffix == "U8" {
        "Get_U8 (Data, Pos)".to_string()
    } else {
        format!("Get_{suffix} (Data, Pos, Endian)")
    };
    let _ = st;
    let _ = writeln!(
        out,
        "\n   function Read_{n} (Data : Byte_Array; Pos : in out Natural; Endian : Endianness) return {n} is"
    );
    let _ = writeln!(out, "      V : {n};");
    let _ = writeln!(out, "   begin");
    let _ = writeln!(out, "      V.storage := {get_call};");
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

/// Gathers `@verbatim` text lines for a file-scope placement (BEGIN_FILE /
/// END_FILE) from every top-level declaration, in document order. Ada codegen
/// language tag: `ada` (plus the `*` wildcard, handled by `verbatims_for_language`).
fn collect_file_verbatim(
    flat: &[(Vec<String>, &Definition)],
    placement: PlacementKind,
) -> Vec<String> {
    let mut out = Vec::new();
    for (_, def) in flat {
        for anns in definition_annotations(def) {
            collect_verbatim_lines(&mut out, anns, placement);
        }
    }
    out
}

/// The annotation list(s) attached to a top-level definition (struct/union/
/// enum/bitset/bitmask), used for `@verbatim` gathering.
fn definition_annotations(def: &Definition) -> Vec<&[zerodds_idl::ast::types::Annotation]> {
    match def {
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
            vec![s.annotations.as_slice()]
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
            vec![u.annotations.as_slice()]
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
            vec![e.annotations.as_slice()]
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
            vec![b.annotations.as_slice()]
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(m))) => {
            vec![m.annotations.as_slice()]
        }
        _ => Vec::new(),
    }
}

/// Appends each source line of every `@verbatim` block matching `placement` and
/// the Ada language tag to `out`.
fn collect_verbatim_lines(
    out: &mut Vec<String>,
    anns: &[zerodds_idl::ast::types::Annotation],
    placement: PlacementKind,
) {
    if let Ok(lowered) = lower_annotations(anns) {
        for v in lowered.verbatims_for_language(&["ada"]) {
            if v.placement == placement {
                for line in v.text.lines() {
                    out.push(line.to_string());
                }
            }
        }
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
    /// `Some((P, S))` when this member's (dealiased) type is `fixed<P,S>`, so
    /// the enclosing spec knows to emit the `Fixed_<P>_<S>` subtype + BCD prelude.
    fixed_layout: Option<(u32, u32)>,
    /// `@non_serialized`: kept as an Ada record component, off every wire form.
    non_serialized: bool,
}

struct StructGen {
    ada_name: String,
    /// Named array-type declarations (Ada forbids anonymous array record
    /// components), emitted in the spec before the record.
    array_types: Vec<String>,
    /// `@optional`-member wrapper record types (`Present : Boolean; Value : T`),
    /// emitted in the spec before the record.
    opt_types: Vec<String>,
    /// `@verbatim(placement=BEFORE_DECLARATION)` text lines (Ada tag), emitted
    /// immediately before the record type.
    verbatim_before: Vec<String>,
    /// `@verbatim(placement=AFTER_DECLARATION)` text lines, emitted after the
    /// record + its `Marshal`/`Unmarshal`/`Key_Hash` declarations.
    verbatim_after: Vec<String>,
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

/// Evaluates a constant integer expression (literals, unary, and the binary
/// operators of §7.4.1.4.4) to an `i64`. Used for `const` integer/octet values
/// and integer union labels, which — unlike array sizes — may be full
/// expressions (`1 << 3`, `A | B`). `None` for non-integer or unresolvable
/// operands (e.g. a named constant reference, which this backend does not
/// track).
///
/// zerodds-lint: recursion-depth 64 (const-expr tree; bounded by IDL nesting).
fn eval_int(e: &ConstExpr) -> Option<i64> {
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => parse_int(raw),
        ConstExpr::Unary { op, operand, .. } => {
            let v = eval_int(operand)?;
            match op {
                UnaryOp::Plus => Some(v),
                UnaryOp::Minus => Some(v.checked_neg()?),
                UnaryOp::BitNot => Some(!v),
            }
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let a = eval_int(lhs)?;
            let b = eval_int(rhs)?;
            match op {
                BinaryOp::Or => Some(a | b),
                BinaryOp::Xor => Some(a ^ b),
                BinaryOp::And => Some(a & b),
                BinaryOp::Shl => Some(a.checked_shl(u32::try_from(b).ok()?)?),
                BinaryOp::Shr => Some(a.checked_shr(u32::try_from(b).ok()?)?),
                BinaryOp::Add => a.checked_add(b),
                BinaryOp::Sub => a.checked_sub(b),
                BinaryOp::Mul => a.checked_mul(b),
                BinaryOp::Div => a.checked_div(b),
                BinaryOp::Mod => a.checked_rem(b),
            }
        }
        ConstExpr::Literal(_) | ConstExpr::Scoped(_) => None,
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
/// uses for type-reference resolution — a module's members are promoted to the
/// top level, each paired with its module scope path so the definition and
/// reference sites can flatten each name to `scope_simple` ([`qualify`] /
/// [`resolve_scoped_name`]). Two same-simple-name types in different modules
/// therefore become distinct types rather than colliding (#21).
///
/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_module_defs(defs: &[Definition]) -> Vec<(Vec<String>, &Definition)> {
    let mut out = Vec::new();
    let mut scope = Vec::new();
    flatten_module_defs_into(defs, &mut scope, &mut out);
    out
}

/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_module_defs_into<'a>(
    defs: &'a [Definition],
    scope: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, &'a Definition)>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                flatten_module_defs_into(&m.definitions, scope, out);
                scope.pop();
            }
            other => out.push((scope.clone(), other)),
        }
    }
}

/// Rewrites every `interface` into a same-named module holding its `type` and
/// `const` exports (IDL 4.2 §7.4.7 — an interface is a naming scope, and its
/// nested type/const declarations are data types the backend must still emit).
/// Operations and attributes carry no serializable data type and are dropped.
/// The result is an owned definition list the rest of the emitter treats
/// exactly like ordinary modules, so a struct declared inside an interface
/// becomes `<Iface>_<Struct>` — previously the whole interface body was
/// silently discarded.
///
/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn expand_interfaces(defs: &[Definition]) -> Vec<Definition> {
    let mut out = Vec::with_capacity(defs.len());
    for d in defs {
        match d {
            Definition::Module(m) => {
                out.push(Definition::Module(ModuleDef {
                    name: m.name.clone(),
                    definitions: expand_interfaces(&m.definitions),
                    annotations: m.annotations.clone(),
                    span: m.span,
                    reopen_spans: m.reopen_spans.clone(),
                }));
            }
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                let mut inner = Vec::new();
                for e in &iface.exports {
                    match e {
                        Export::Type(t) => inner.push(Definition::Type(t.clone())),
                        Export::Const(c) => inner.push(Definition::Const(c.clone())),
                        Export::Op(_) | Export::Attr(_) | Export::Except(_) => {}
                    }
                }
                out.push(Definition::Module(ModuleDef {
                    name: iface.name.clone(),
                    definitions: expand_interfaces(&inner),
                    annotations: Vec::new(),
                    span: iface.span,
                    reopen_spans: Vec::new(),
                }));
            }
            other => out.push(other.clone()),
        }
    }
    out
}

/// Collects `typedef` aliases (simple declarators) as name -> aliased type-spec.
/// A typedef is wire-transparent, so members are resolved to the underlying
/// type before mapping (`typedef long Score; Score s;` marshals as `long`).
fn collect_typedefs(defs: &[Definition]) -> HashMap<String, TypeSpec> {
    let mut m = HashMap::new();
    for (scope, def) in flatten_module_defs(defs) {
        if let Definition::Type(TypeDecl::Typedef(td)) = def {
            for d in &td.declarators {
                if let Declarator::Simple(name) = d {
                    m.insert(qualify(&scope, &name.text), td.type_spec.clone());
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
fn collect_structs(defs: &[Definition]) -> HashMap<String, &StructDef> {
    let mut m = HashMap::new();
    for (scope, def) in flatten_module_defs(defs) {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) = def {
            m.insert(qualify(&scope, &s.name.text), s);
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
            let name = resolve_scoped_name(sn);
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
        TypeSpec::Scoped(sn) => enum_names.contains(&resolve_scoped_name(sn)),
        _ => false,
    }
}

/// Returns a struct's effective members: every inherited member from its base
/// chain (base-most first, in declaration order) followed by its own, cloned so
/// the caller owns a single flat sequence (Extended Data Types BB §7.4.13). The
/// base is resolved through the same `resolve_scoped_name`/`struct_defs` path
/// used for member references; an unresolvable base contributes nothing rather
/// than aborting (mirrors how a dangling reference degrades elsewhere).
///
/// zerodds-lint: recursion-depth 16 (struct inheritance chain; bounded by the
/// IDL's aggregate-inheritance depth).
fn effective_members(s: &StructDef, struct_defs: &HashMap<String, &StructDef>) -> Vec<Member> {
    let mut out = Vec::new();
    if let Some(base) = &s.base {
        if let Some(bs) = struct_defs.get(&resolve_scoped_name(base)) {
            out.extend(effective_members(bs, struct_defs));
        }
    }
    out.extend(s.members.iter().cloned());
    out
}

fn build_struct(
    s: &StructDef,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    struct_defs: &HashMap<String, &StructDef>,
) -> Result<StructGen> {
    // Member references resolve against this struct's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = lower_annotations(&s.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable);
    // Struct inheritance (Extended Data Types BB §7.4.13): a derived struct's
    // wire is its base's members first, then its own, in a single member-id
    // sequence (XTypes 1.3 §7.3.1.2.1). Flatten the base chain here so the
    // record, marshal, and key logic all see the full effective member set;
    // previously `.base` was ignored and inherited members silently vanished.
    let members = effective_members(s, struct_defs);
    // `@optional` in `@mutable` is expressed by omitting the absent member's
    // EMHEADER (XTypes 1.3 §7.4.3.4.2), not an inline presence byte — the Ada
    // backend does not yet emit that conditional member framing, so reject it
    // rather than emit a wrong wire form (final/appendable optional IS emitted).
    if ext == ExtensibilityKind::Mutable
        && members.iter().any(|m| member_is_optional(&m.annotations))
    {
        return Err(IdlAdaError::Unsupported(format!(
            "@optional member in @mutable struct {} (member-presence framing not yet emitted)",
            s.name.text
        )));
    }
    let struct_ada_name = escape_ada_ident(&qualify(scope, &s.name.text));
    let mut fields = Vec::new();
    let mut array_types = Vec::new();
    let mut opt_types = Vec::new();
    let mut array_vector_elems = Vec::new();
    let mut map_packages = Vec::new();
    let mut next_id: u32 = 0;
    // Ada records are case-insensitive; keep the emitted component names
    // case-insensitively distinct (see `CiDedup`).
    let mut dedup = CiDedup::new();
    for m in &members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        let optional = member_is_optional(&m.annotations);
        // P0-5 (#2): a `@non_serialized` member keeps its Ada record component
        // but is off the wire and does NOT consume a sequential id slot.
        let non_serialized =
            zerodds_idl::semantics::annotations::member_is_non_serialized(&m.annotations);
        for d in &m.declarators {
            let name = dedup.unique(&escape_ada_ident(&d.name().text));
            let id = if non_serialized {
                0
            } else {
                let assigned = explicit_id.unwrap_or(next_id);
                next_id = assigned + 1;
                assigned
            };
            let simple = matches!(d, Declarator::Simple(_));
            let fixed_layout = match &resolved {
                TypeSpec::Fixed(f) => Some(fixed_ps(f)?),
                _ => None,
            };
            // `@optional` (XTypes §7.4.5.1.4, final/appendable): a `uint8`
            // presence flag then the value if present. Represented as an Ada
            // wrapper record `{ Present : Boolean; Value : T }`. Supported for
            // simple (non-array, non-map) members; the value goes through the
            // ordinary `map_type`/`map_get` path via `V.<name>.Value`.
            if optional {
                if !simple || matches!(resolved, TypeSpec::Map(_)) {
                    return Err(IdlAdaError::Unsupported(format!(
                        "@optional on an array or map member `{name}`"
                    )));
                }
                let inner = format!("V.{name}.Value");
                let (t, p) = map_type(&resolved, &inner, enum_names, struct_names)?;
                let g = map_get(&resolved, &inner, enum_names, struct_names)?;
                let opt_type = format!("{struct_ada_name}_{name}_Opt");
                opt_types.push(format!(
                    "type {opt_type} is record\n      Present : Boolean := False;\n      Value   : {t};\n   end record;"
                ));
                let put = format!(
                    "Put_Bool ($w, V.{name}.Present); if V.{name}.Present then {p} end if;"
                );
                let get = format!(
                    "V.{name}.Present := Get_Bool (Data, Pos); if V.{name}.Present then {g} end if;"
                );
                fields.push(FieldGen {
                    ada_name: name,
                    ada_type: opt_type,
                    put,
                    get,
                    id,
                    key,
                    resolved: resolved.clone(),
                    simple,
                    fixed_layout,
                    non_serialized,
                });
                continue;
            }
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
                fixed_layout,
                non_serialized,
            });
        }
    }
    let key_members: Vec<&Member> = members
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
    let mut zdkeys: Vec<&FieldGen> = fields
        .iter()
        .filter(|f| f.key && !f.non_serialized)
        .collect();
    zdkeys.sort_by_key(|f| f.id);
    let mut key_puts: Vec<String> = Vec::new();
    for f in &zdkeys {
        let nested_struct = if f.simple {
            match &f.resolved {
                TypeSpec::Scoped(sn) => struct_defs.get(&resolve_scoped_name(sn)).copied(),
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

    let mut verbatim_before = Vec::new();
    collect_verbatim_lines(
        &mut verbatim_before,
        &s.annotations,
        PlacementKind::BeforeDeclaration,
    );
    let mut verbatim_after = Vec::new();
    collect_verbatim_lines(
        &mut verbatim_after,
        &s.annotations,
        PlacementKind::AfterDeclaration,
    );

    Ok(StructGen {
        ada_name: struct_ada_name,
        array_types,
        opt_types,
        verbatim_before,
        verbatim_after,
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
                let name = resolve_scoped_name(sn);
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
            if f.non_serialized {
                continue;
            }
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
            if f.non_serialized {
                continue;
            }
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
            if f.non_serialized {
                continue;
            }
            let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
            let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
            let _ = writeln!(out, "      {}", f.get);
        }
    } else {
        if sg.appendable {
            let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
        }
        for f in &sg.fields {
            if f.non_serialized {
                continue;
            }
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

/// A generated union case: Ada-rendered case-choice labels (empty +
/// is_default = `default`), the member field, its Ada type, and its put
/// statement. Labels are already rendered for the discriminator's Ada type —
/// integer literals, enumerator names, `Character'Val (n)`, or `True`/`False` —
/// so a non-integer discriminator (enum/char/bool) emits a legal `when` choice.
struct UnionCaseAda {
    labels: Vec<String>,
    is_default: bool,
    field: String,
    ada_type: String,
    put: String,
    get: String,
    fixed_layout: Option<(u32, u32)>,
}

struct UnionGen {
    ada_name: String,
    disc_type: String,
    disc_put: String,
    disc_get: String,
    cases: Vec<UnionCaseAda>,
    appendable: bool,
    mutable: bool,
}

fn build_union(
    u: &UnionDef,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<UnionGen> {
    // Member references resolve against this union's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = lower_annotations(&u.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable);
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
    // The record's synthetic `disc` component: reserve its name so a case
    // branch spelled `Disc`/`DISC` (Ada is case-insensitive) is suffixed.
    let mut dedup = CiDedup::new();
    dedup.reserve("disc");
    let mut cases = Vec::new();
    for c in &u.cases {
        let field = dedup.unique(&escape_ada_ident(&c.element.declarator.name().text));
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ada_type, put) = map_type(&resolved, &format!("V.{field}"), enum_names, struct_names)?;
        let get = map_get(&resolved, &format!("V.{field}"), enum_names, struct_names)?;
        let fixed_layout = match &resolved {
            TypeSpec::Fixed(f) => Some(fixed_ps(f)?),
            _ => None,
        };
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => {
                    labels.push(render_union_label(e, &u.switch_type, &u.name.text)?);
                }
            }
        }
        cases.push(UnionCaseAda {
            labels,
            is_default,
            field,
            ada_type,
            put,
            get,
            fixed_layout,
        });
    }
    Ok(UnionGen {
        ada_name: escape_ada_ident(&qualify(scope, &u.name.text)),
        disc_type,
        disc_put,
        disc_get,
        cases,
        appendable: ext == ExtensibilityKind::Appendable,
        mutable: ext == ExtensibilityKind::Mutable,
    })
}

/// Renders one union case-label constant as an Ada case-choice literal of the
/// discriminator's Ada type (XTypes 1.3 §7.4.3.5 / IDL 4.2 §7.4.1.4). An
/// integer/octet switch yields the decimal value; an enum switch yields the
/// referenced enumerator's (escaped) simple name — resolved by the case
/// expression's known type, so unqualified is legal even across overloaded
/// enumerals; a `char` switch yields `Character'Val (codepoint)`; a `boolean`
/// switch yields `True`/`False`. Non-evaluable labels are rejected loudly
/// rather than mis-emitted.
fn render_union_label(e: &ConstExpr, sw: &SwitchTypeSpec, uname: &str) -> Result<String> {
    let bad = || IdlAdaError::Unsupported(format!("unresolvable union label in `{uname}`"));
    match sw {
        SwitchTypeSpec::Integer(_) | SwitchTypeSpec::Octet => {
            eval_int(e).map(|v| v.to_string()).ok_or_else(bad)
        }
        SwitchTypeSpec::Boolean => match e {
            ConstExpr::Literal(Literal {
                kind: LiteralKind::Boolean,
                raw,
                ..
            }) => Ok(if raw.eq_ignore_ascii_case("true") {
                "True".to_string()
            } else {
                "False".to_string()
            }),
            _ => Err(bad()),
        },
        SwitchTypeSpec::Char => char_literal_codepoint(e)
            .map(|c| format!("Character'Val ({c})"))
            .ok_or_else(bad),
        SwitchTypeSpec::Scoped(_) => match e {
            // A bare or qualified enumerator reference: the last path segment
            // is the enumerator; the case expression's type disambiguates it.
            ConstExpr::Scoped(sn) => sn
                .parts
                .last()
                .map(|p| escape_ada_ident(&p.text))
                .ok_or_else(bad),
            _ => Err(bad()),
        },
    }
}

/// Codepoint of a `'c'` char-literal `ConstExpr` (source text incl. quotes),
/// handling the common C-style escapes. `None` for anything else.
fn char_literal_codepoint(e: &ConstExpr) -> Option<u32> {
    let raw = match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Char | LiteralKind::WideChar,
            raw,
            ..
        }) => raw,
        _ => return None,
    };
    let inner = raw.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut it = inner.chars();
    let c = it.next()?;
    if c != '\\' {
        return Some(c as u32);
    }
    let esc = it.next()?;
    match esc {
        'n' => Some(0x0A),
        't' => Some(0x09),
        'r' => Some(0x0D),
        '0' => Some(0x00),
        'a' => Some(0x07),
        'b' => Some(0x08),
        'f' => Some(0x0C),
        'v' => Some(0x0B),
        '\\' => Some(0x5C),
        '\'' => Some(0x27),
        '"' => Some(0x22),
        'x' => u32::from_str_radix(&it.collect::<String>(), 16).ok(),
        _ => None,
    }
}

/// Renders one IDL `const` as an Ada `Name : constant Type := Value;` line
/// (§7.4.1.4.4). The value is evaluated for the constant's declared type:
/// integers/octet fold the expression to a decimal, floats keep an Ada-legal
/// literal, chars/wchars become `'Val` codepoints, booleans `True`/`False`,
/// strings a re-quoted Ada string, and an enum-typed constant its enumerator.
fn render_const_decl(scope: &[String], cd: &ConstDecl) -> Result<String> {
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let name = escape_ada_ident(&qualify(scope, &cd.name.text));
    let bad = || IdlAdaError::Unsupported(format!("unresolvable const value for `{name}`"));
    let (ty, val): (String, String) = match &cd.type_ {
        ConstType::Integer(i) => {
            let (t, _) = map_integer(*i, "x")?;
            (t, eval_int(&cd.value).ok_or_else(bad)?.to_string())
        }
        ConstType::Octet => (
            "Unsigned_8".to_string(),
            eval_int(&cd.value).ok_or_else(bad)?.to_string(),
        ),
        ConstType::Boolean => (
            "Boolean".to_string(),
            bool_literal_str(&cd.value).ok_or_else(bad)?,
        ),
        ConstType::Char => (
            "Character".to_string(),
            format!(
                "Character'Val ({})",
                char_literal_codepoint(&cd.value).ok_or_else(bad)?
            ),
        ),
        ConstType::WideChar => (
            "Wide_Character".to_string(),
            format!(
                "Wide_Character'Val ({})",
                char_literal_codepoint(&cd.value).ok_or_else(bad)?
            ),
        ),
        ConstType::Floating(FloatingType::Float) => (
            "IEEE_Float_32".to_string(),
            float_literal_str(&cd.value).ok_or_else(bad)?,
        ),
        ConstType::Floating(FloatingType::Double | FloatingType::LongDouble) => (
            "IEEE_Float_64".to_string(),
            float_literal_str(&cd.value).ok_or_else(bad)?,
        ),
        ConstType::String { wide: false } => (
            "String".to_string(),
            ada_string_from_raw(&cd.value).ok_or_else(bad)?,
        ),
        ConstType::String { wide: true } => (
            "Wide_String".to_string(),
            ada_string_from_raw(&cd.value).ok_or_else(bad)?,
        ),
        // A `fixed<P,S>` constant has no primitive Ada type; keep its decimal
        // image as a String constant (the wire form is BCD, computed at use).
        ConstType::Fixed => (
            "String".to_string(),
            match &cd.value {
                ConstExpr::Literal(l) => format!("\"{}\"", l.raw.trim_end_matches(['d', 'D'])),
                _ => return Err(bad()),
            },
        ),
        // An enum-typed constant: the type is the (escaped, flattened) enum
        // name; the value is the referenced enumerator's simple name.
        ConstType::Scoped(sn) => {
            let ty = escape_ada_ident(&resolve_scoped_name(sn));
            let v = match &cd.value {
                ConstExpr::Scoped(vn) => vn
                    .parts
                    .last()
                    .map(|p| escape_ada_ident(&p.text))
                    .ok_or_else(bad)?,
                _ => return Err(bad()),
            };
            (ty, v)
        }
    };
    Ok(format!("{name} : constant {ty} := {val};"))
}

/// `True`/`False` for a boolean-literal const-expr; `None` otherwise.
fn bool_literal_str(e: &ConstExpr) -> Option<String> {
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Boolean,
            raw,
            ..
        }) => Some(if raw.eq_ignore_ascii_case("true") {
            "True".to_string()
        } else {
            "False".to_string()
        }),
        _ => None,
    }
}

/// An Ada floating literal for a float/integer const-expr (with optional leading
/// sign), normalising the source text to Ada syntax (a mantissa needs digits on
/// both sides of the point). `None` for non-numeric expressions.
///
/// zerodds-lint: recursion-depth 8 (leading unary signs; bounded by IDL syntax).
fn float_literal_str(e: &ConstExpr) -> Option<String> {
    match e {
        ConstExpr::Unary {
            op: UnaryOp::Minus,
            operand,
            ..
        } => Some(format!("-{}", float_literal_str(operand)?)),
        ConstExpr::Unary {
            op: UnaryOp::Plus,
            operand,
            ..
        } => float_literal_str(operand),
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Floating,
            raw,
            ..
        }) => Some(sanitize_ada_float(raw)),
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => parse_int(raw).map(|n| format!("{n}.0")),
        _ => None,
    }
}

/// Normalises an IDL float literal to an Ada-legal one: drops any `f`/`d`/`l`
/// suffix, gives the mantissa digits on both sides of the point (`.5` → `0.5`,
/// `3.` → `3.0`, `1e9` → `1.0e9`).
fn sanitize_ada_float(raw: &str) -> String {
    let s = raw.trim().trim_end_matches(['f', 'F', 'd', 'D', 'l', 'L']);
    let (mut mant, exp) = match s.find(['e', 'E']) {
        Some(i) => (s[..i].to_string(), s[i..].to_string()),
        None => (s.to_string(), String::new()),
    };
    if !mant.contains('.') {
        mant.push_str(".0");
    }
    if mant.starts_with('.') {
        mant.insert(0, '0');
    }
    if mant.ends_with('.') {
        mant.push('0');
    }
    format!("{mant}{exp}")
}

/// Re-quotes a string/wstring literal (source text incl. quotes and an optional
/// `L` prefix) as an Ada string literal, doubling embedded quotes. `None` if the
/// const-expr is not a string literal.
fn ada_string_from_raw(e: &ConstExpr) -> Option<String> {
    let raw = match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::String | LiteralKind::WideString,
            raw,
            ..
        }) => raw,
        _ => return None,
    };
    let t = raw.strip_prefix('L').unwrap_or(raw);
    let inner = t
        .strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .unwrap_or(t);
    Some(format!("\"{}\"", inner.replace('"', "\"\"")))
}

/// Emits a union's body-local `Marshal_Into` (discriminator + `case` dispatch)
/// and its `Marshal` wrapper (XCDR2 §7.4.3.5.4).
fn emit_union_marshal(out: &mut String, ug: &UnionGen) {
    let _ = writeln!(
        out,
        "\n   procedure Marshal_Into (V : {}; W : in out Buf_T) is",
        ug.ada_name
    );
    if ug.appendable || ug.mutable {
        let _ = writeln!(out, "      B : Buf_T;");
    }
    let _ = writeln!(out, "   begin");
    let has_default = ug.cases.iter().any(|c| c.is_default);
    if ug.mutable {
        // @mutable union (XTypes 1.3 §7.4.3.5.3): PL_CDR2 — an outer DHEADER
        // framing an EMHEADER-tagged member list, exactly like a @mutable
        // struct. The discriminator is member id 0; the selected branch is a
        // second EMHEADER-framed member (id branch-index+1). The Ada backend
        // uses the universal LC4 EMHEADER framing throughout (see the @mutable
        // struct path); a compact-LC variant is the coordinated cross-backend
        // wire change tracked separately.
        emit_union_mutable_marshal_body(out, ug, has_default);
    } else {
        let wv = if ug.appendable {
            let _ = writeln!(out, "      B.Endian := W.Endian;");
            "B"
        } else {
            "W"
        };
        let _ = writeln!(out, "      {}", ug.disc_put.replace("$w", wv));
        let _ = writeln!(out, "      case V.disc is");
        for c in &ug.cases {
            if c.is_default {
                let _ = writeln!(out, "         when others => {}", c.put.replace("$w", wv));
            } else {
                let labels = c.labels.join(" | ");
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
    // @appendable skips the leading DHEADER; @mutable skips the DHEADER then
    // the discriminator's EMHEADER + NEXTINT (members read in fixed order, so
    // the member-id/length tags are skipped, not interpreted).
    if ug.appendable || ug.mutable {
        let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
    }
    if ug.mutable {
        let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
        let _ = writeln!(out, "      Skip_U32 (Data, Pos, Endian);");
    }
    let _ = writeln!(out, "      {}", ug.disc_get);
    let _ = writeln!(out, "      case V.disc is");
    for c in &ug.cases {
        // For @mutable, the selected branch is EMHEADER + NEXTINT framed.
        let skip = if ug.mutable {
            "Skip_U32 (Data, Pos, Endian); Skip_U32 (Data, Pos, Endian); "
        } else {
            ""
        };
        if c.is_default {
            let _ = writeln!(out, "         when others => {skip}{}", c.get);
        } else {
            let labels = c.labels.join(" | ");
            let _ = writeln!(out, "         when {labels} => {skip}{}", c.get);
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

/// Emits the `@mutable` union `Marshal_Into` body: DHEADER-framed member list
/// with the discriminator (member id 0) then the selected branch (member id
/// branch-index+1), each wrapped in an LC4 EMHEADER + NEXTINT — the same
/// universal framing the `@mutable` struct path uses (`emit_marshal`). Writes
/// into `B`, then flushes `B` behind an outer DHEADER.
fn emit_union_mutable_marshal_body(out: &mut String, ug: &UnionGen, has_default: bool) {
    let _ = writeln!(out, "      B.Endian := W.Endian;");
    // Discriminator — member id 0.
    let _ = writeln!(out, "      Put_U32 (B, 16#40000000#);");
    let _ = writeln!(out, "      declare");
    let _ = writeln!(out, "         M2 : Buf_T;");
    let _ = writeln!(out, "      begin");
    let _ = writeln!(out, "         M2.Endian := W.Endian;");
    let _ = writeln!(out, "         {}", ug.disc_put.replace("$w", "M2"));
    let _ = writeln!(out, "         Put_U32 (B, Unsigned_32 (M2.Len));");
    let _ = writeln!(out, "         Append (B, M2);");
    let _ = writeln!(out, "      end;");
    // Selected branch — member id branch-index + 1, EMHEADER + NEXTINT framed.
    let _ = writeln!(out, "      case V.disc is");
    for (idx, c) in ug.cases.iter().enumerate() {
        let emh = 0x4000_0000_u32 | (u32::try_from(idx).unwrap_or(0) + 1);
        let branch = format!(
            "declare M2 : Buf_T; begin M2.Endian := W.Endian; Put_U32 (B, 16#{emh:08X}#); {} Put_U32 (B, Unsigned_32 (M2.Len)); Append (B, M2); end;",
            c.put.replace("$w", "M2")
        );
        if c.is_default {
            let _ = writeln!(out, "         when others => {branch}");
        } else {
            let labels = c.labels.join(" | ");
            let _ = writeln!(out, "         when {labels} => {branch}");
        }
    }
    if !has_default {
        let _ = writeln!(out, "         when others => null;");
    }
    let _ = writeln!(out, "      end case;");
    let _ = writeln!(out, "      Put_U32 (W, Unsigned_32 (B.Len));");
    let _ = writeln!(out, "      Append (W, B);");
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
fn bound_check_stmt(len_expr: &str, bound: &ConstExpr, prefix: &str, what: &str) -> Result<String> {
    let bv = array_size(bound).ok_or_else(|| {
        IdlAdaError::Unsupported(format!("non-literal bound on {what} `{len_expr}`"))
    })?;
    Ok(format!(
        "if {len_expr} > {bv} then raise Constraint_Error with \"{prefix} {what} length exceeds its IDL bound ({bv})\"; end if;"
    ))
}

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
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
        TypeSpec::Sequence(seq) => map_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            expr,
            enum_names,
            struct_names,
        ),
        // fixed<P,S>: packed-BCD octets (CORBA §9.3.2.7 ≡ XCDR2 §7.4.4.5),
        // written raw with no length prefix (the octet count is P-derived).
        TypeSpec::Fixed(f) => {
            let (p, s) = fixed_ps(f)?;
            Ok((format!("Fixed_{p}_{s}"), format!("Put_Fixed ($w, {expr});")))
        }
        // A named enum (i32 wire) or a nested struct member (inline marshal).
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            let esc = escape_ada_ident(&name);
            if enum_names.contains(&name) {
                // Enum holder width follows @bit_bound (XTypes 1.3 §7.4.5.1);
                // narrow the Unsigned_32 codec value to 1/2 octets.
                let put = match enum_wire_width(&name) {
                    1 => format!("Put_U8 ($w, Unsigned_8 ({esc}_To_U32 ({expr}) and 16#FF#));"),
                    2 => format!("Put_U16 ($w, Unsigned_16 ({esc}_To_U32 ({expr}) and 16#FFFF#));"),
                    _ => format!("Put_U32 ($w, {esc}_To_U32 ({expr}));"),
                };
                Ok((esc.clone(), put))
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    enum_names: &HashSet<String>,
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
        let name = resolve_scoped_name(sn);
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
    // sequence<primitive|enum|string> (#9, thin→thin per idl-go): a `u32` count
    // followed by each element encoded inline — a fully-descriptive element type
    // takes no collection DHEADER (XCDR2 §7.4.3.5.3). Element goes through the
    // normal `map_type` mapper; the Ada value is an `Ada.Containers.Vectors`
    // instance keyed by the element's Ada type (e.g. `Integer_32_Vectors`).
    if elem_is_arbitrary_seq_element(elem, enum_names, struct_names) {
        let (elem_ty, elem_put) = map_type(elem, "E", enum_names, struct_names)?;
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
            "{check}Put_U32 ($w, Unsigned_32 (Natural ({expr}.Length))); \
             for E of {expr} loop {elem_put} end loop;"
        );
        return Ok((format!("{elem_ty}_Vectors.Vector"), put));
    }
    Err(IdlAdaError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}

/// `true` if `elem` is a primitive, enum, or string — an element type that
/// [`map_sequence`] can encode as a plain `u32`-count-prefixed vector. Nested
/// sequences/maps (which would need a named nested vector/map type) and structs
/// (handled by the DHEADER-framed path above) return `false`.
fn elem_is_arbitrary_seq_element(
    elem: &TypeSpec,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> bool {
    match elem {
        TypeSpec::Primitive(_) | TypeSpec::String(_) => true,
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            enum_names.contains(&name) && !struct_names.contains(&name)
        }
        _ => false,
    }
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
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
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
        TypeSpec::Sequence(seq) => map_get_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            target,
            enum_names,
            struct_names,
        ),
        TypeSpec::Fixed(f) => {
            let (p, _s) = fixed_ps(f)?;
            let n = fixed_byte_len(p);
            Ok(format!("{target} := Get_Fixed (Data, Pos, {n});"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            let esc = escape_ada_ident(&name);
            if enum_names.contains(&name) {
                // Read the @bit_bound-wide holder (XTypes 1.3 §7.4.5.1); Get_U8
                // takes no Endian, Get_U16/Get_U32 do.
                let get = match enum_wire_width(&name) {
                    1 => format!("{target} := {esc}_Of_U32 (Unsigned_32 (Get_U8 (Data, Pos)));"),
                    2 => format!(
                        "{target} := {esc}_Of_U32 (Unsigned_32 (Get_U16 (Data, Pos, Endian)));"
                    ),
                    _ => format!("{target} := {esc}_Of_U32 (Get_U32 (Data, Pos, Endian));"),
                };
                Ok(get)
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    target: &str,
    enum_names: &HashSet<String>,
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
        let name = resolve_scoped_name(sn);
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
    // sequence<primitive|enum|string> decode: `u32` count then per-element read,
    // no leading collection DHEADER (inverse of the arbitrary encode branch).
    if elem_is_arbitrary_seq_element(elem, enum_names, struct_names) {
        let (elem_ty, _) = map_type(elem, "E", enum_names, struct_names)?;
        let elem_get = map_get(elem, "E", enum_names, struct_names)?;
        let check = bound
            .map(|b| bound_check_stmt("Zn", b, "decoded", "sequence"))
            .transpose()?
            .map(|s| format!("{s} "))
            .unwrap_or_default();
        return Ok(format!(
            "declare Zn : Natural; begin Zn := Natural (Get_U32 (Data, Pos, Endian)); {check}{target}.Clear; for Zi in 1 .. Zn loop declare E : {elem_ty}; begin {elem_get} {target}.Append (E); end; end loop; end;"
        ));
    }
    Err(IdlAdaError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}
