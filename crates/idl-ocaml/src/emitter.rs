// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → OCaml emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! OCaml source file: a `Wire` module (byte-identical to `endpoints/ocaml`) plus,
//! per IDL `struct`, a module with a record type `t` and a `marshal(v, endian)`
//! function. `@final` and `@appendable` are supported; other extensibilities and
//! constructs raise [`IdlOcamlError::Unsupported`].

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

use crate::error::{IdlOcamlError, Result};
use crate::keywords::escape_ocaml_ident;

/// Options for the OCaml backend.
#[derive(Debug, Clone, Default)]
pub struct OcamlGenOptions {}

/// The `Wire` module, byte-identical to `endpoints/ocaml`'s Wire.
const WIRE_MODULE: &str = r#"module Wire = struct
  type endian = LE | BE

  type writer = { buf : Buffer.t; endian : endian }

  let writer endian = { buf = Buffer.create 64; endian }

  let align w a =
    let cap = min a 4 in
    let pad = (cap - (Buffer.length w.buf mod cap)) mod cap in
    for _ = 1 to pad do Buffer.add_char w.buf '\000' done

  let put w a (le : bytes) =
    align w a;
    let n = Bytes.length le in
    if w.endian = BE then
      for i = n - 1 downto 0 do Buffer.add_char w.buf (Bytes.get le i) done
    else Buffer.add_bytes w.buf le

  let le_of_int v n =
    let b = Bytes.create n in
    for i = 0 to n - 1 do
      Bytes.set b i (Char.chr ((v lsr (8 * i)) land 0xff))
    done;
    b

  let put_u8 w v = Buffer.add_char w.buf (Char.chr (v land 0xff))
  let put_bool w v = put_u8 w (if v then 1 else 0)
  let put_u16 w v = put w 2 (le_of_int v 2)
  let put_u32 w v = put w 4 (le_of_int v 4)

  let put_u64 w (v : int64) =
    let b = Bytes.create 8 in
    for i = 0 to 7 do
      let byte = Int64.to_int (Int64.logand (Int64.shift_right_logical v (8 * i)) 0xffL) in
      Bytes.set b i (Char.chr byte)
    done;
    put w 4 b

  let put_f32 w (v : float) =
    let bits = Int32.bits_of_float v in
    let b = Bytes.create 4 in
    for i = 0 to 3 do
      let byte = Int32.to_int (Int32.logand (Int32.shift_right_logical bits (8 * i)) 0xffl) in
      Bytes.set b i (Char.chr byte)
    done;
    put w 4 b

  let put_f64 w (v : float) = put_u64 w (Int64.bits_of_float v)
  let put_bytes w (b : bytes) = Buffer.add_bytes w.buf b

  let put_string w s =
    put_u32 w (String.length s + 1);
    Buffer.add_string w.buf s;
    put_u8 w 0

  let put_seq_u8 w (b : bytes) =
    put_u32 w (Bytes.length b);
    Buffer.add_bytes w.buf b

  let put_wstring w s =
    let out = ref [] in
    let i = ref 0 in
    let n = String.length s in
    while !i < n do
      let d = String.get_utf_8_uchar s !i in
      let cp = Uchar.to_int (Uchar.utf_decode_uchar d) in
      i := !i + Uchar.utf_decode_length d;
      if cp <= 0xFFFF then out := cp :: !out
      else begin
        let rr = cp - 0x10000 in
        out := (0xDC00 lor (rr land 0x3FF)) :: (0xD800 lor (rr lsr 10)) :: !out
      end
    done;
    let units = List.rev !out in
    put_u32 w (List.length units * 2);
    List.iter (fun u -> put_u16 w u) units

  let put_long_double w (v : float) =
    let bits = Int64.bits_of_float v in
    let sign = Int64.shift_right_logical bits 63 in
    let exp = Int64.logand (Int64.shift_right_logical bits 52) 0x7FFL in
    let mant = Int64.logand bits 0xFFFFFFFFFFFFFL in
    let hi = ref (Int64.shift_left sign 63) in
    let lo = ref 0L in
    if not (exp = 0L && mant = 0L) then begin
      hi :=
        Int64.logor
          (Int64.logor (Int64.shift_left sign 63)
             (Int64.shift_left (Int64.add (Int64.sub exp 1023L) 16383L) 48))
          (Int64.shift_right_logical mant 4);
      lo := Int64.shift_left (Int64.logand mant 0xFL) 60
    end;
    let le = Bytes.create 16 in
    for i = 0 to 7 do
      Bytes.set le i
        (Char.chr (Int64.to_int (Int64.logand (Int64.shift_right_logical !lo (8 * i)) 0xFFL)));
      Bytes.set le (8 + i)
        (Char.chr (Int64.to_int (Int64.logand (Int64.shift_right_logical !hi (8 * i)) 0xFFL)))
    done;
    put w 4 le

  let bytes w = Buffer.to_bytes w.buf

  type reader = { rbuf : bytes; mutable pos : int; rendian : endian }

  let reader (b : bytes) endian = { rbuf = b; pos = 0; rendian = endian }

  let ralign r a =
    let cap = min a 4 in
    while r.pos mod cap <> 0 do r.pos <- r.pos + 1 done

  let get_u8 r =
    let v = Char.code (Bytes.get r.rbuf r.pos) in
    r.pos <- r.pos + 1;
    v

  let get_bool r = get_u8 r <> 0

  let get_le r a n =
    ralign r a;
    let v = ref 0L in
    if r.rendian = BE then
      for i = 0 to n - 1 do
        v := Int64.logor (Int64.shift_left !v 8) (Int64.of_int (Char.code (Bytes.get r.rbuf (r.pos + i))))
      done
    else
      for i = n - 1 downto 0 do
        v := Int64.logor (Int64.shift_left !v 8) (Int64.of_int (Char.code (Bytes.get r.rbuf (r.pos + i))))
      done;
    r.pos <- r.pos + n;
    !v

  let get_u16 r = Int64.to_int (get_le r 2 2)
  let get_u32 r = Int64.to_int (get_le r 4 4)
  let get_u64 r = get_le r 4 8
  let get_f32 r = Int32.float_of_bits (Int32.of_int (get_u32 r))
  let get_f64 r = Int64.float_of_bits (get_u64 r)

  let get_bytes_n r n =
    let b = Bytes.sub r.rbuf r.pos n in
    r.pos <- r.pos + n;
    b

  let get_string r =
    let n = get_u32 r in
    let s = Bytes.sub_string r.rbuf r.pos (n - 1) in
    r.pos <- r.pos + n;
    s

  let get_seq_u8 r =
    let n = get_u32 r in
    get_bytes_n r n

  let get_wstring r =
    let n = get_u32 r / 2 in
    let units = Array.make (max n 1) 0 in
    for i = 0 to n - 1 do units.(i) <- get_u16 r done;
    let buf = Buffer.create (n * 2) in
    let i = ref 0 in
    while !i < n do
      let u = units.(!i) in
      let cp =
        if u >= 0xD800 && u <= 0xDBFF && !i + 1 < n then begin
          let lo = units.(!i + 1) in
          i := !i + 2;
          0x10000 + ((u - 0xD800) lsl 10) + (lo - 0xDC00)
        end
        else begin
          incr i;
          u
        end
      in
      Buffer.add_utf_8_uchar buf (Uchar.of_int cp)
    done;
    Buffer.contents buf

  let get_long_double r =
    ralign r 4;
    let le = Bytes.copy (get_bytes_n r 16) in
    if r.rendian = BE then
      for i = 0 to 7 do
        let t = Bytes.get le i in
        Bytes.set le i (Bytes.get le (15 - i));
        Bytes.set le (15 - i) t
      done;
    let lo = ref 0L and hi = ref 0L in
    for i = 0 to 7 do
      lo := Int64.logor !lo (Int64.shift_left (Int64.of_int (Char.code (Bytes.get le i))) (8 * i));
      hi := Int64.logor !hi (Int64.shift_left (Int64.of_int (Char.code (Bytes.get le (8 + i)))) (8 * i))
    done;
    let sign = Int64.shift_right_logical !hi 63 in
    let exp = Int64.logand (Int64.shift_right_logical !hi 48) 0x7FFFL in
    let mant =
      Int64.logor
        (Int64.shift_left (Int64.logand !hi 0xFFFFFFFFFFFFL) 4)
        (Int64.shift_right_logical !lo 60)
    in
    let bits =
      if exp = 0L && mant = 0L then Int64.shift_left sign 63
      else
        Int64.logor
          (Int64.logor (Int64.shift_left sign 63)
             (Int64.shift_left (Int64.add (Int64.sub exp 16383L) 1023L) 52))
          mant
    in
    Int64.float_of_bits bits
end
"#;

/// Generates a self-contained OCaml module from the IDL AST.
///
/// # Errors
/// Returns [`IdlOcamlError::Unsupported`] for constructs the OCaml backend does
/// not yet emit (unions, nested-struct members, maps, `long double`, `@mutable`,
/// …).
pub fn generate_ocaml_module(spec: &Specification, _opts: &OcamlGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "(* Code generated by zerodds-idlc (OCaml backend). DO NOT EDIT. *)"
    );
    let _ = writeln!(out, "(* SPDX-License-Identifier: Apache-2.0 *)\n");
    out.push_str(WIRE_MODULE);

    // `module X { ... }` content is promoted into the same flat, top-level
    // definition list (see `flatten_module_defs`) so it is no longer
    // silently dropped (swarm59 #21b).
    let flat = flatten_module_defs(&spec.definitions);

    let enum_names: HashSet<String> = flat
        .iter()
        .filter_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                Some(e.name.text.clone())
            }
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
    let struct_defs = collect_struct_defs(spec);

    for def in &flat {
        match def {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => emit_enum(&mut out, e),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                emit_struct(&mut out, s, &enum_names, &struct_names, &typedefs, &struct_defs)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                emit_union(&mut out, u, &enum_names, &struct_names, &typedefs)?;
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Resolves each enumerator's discriminant: default 0..N-1, honoring `@value`
/// (XTypes 1.3 §7.4.5.1 — the returned `i32` values match the wire encoding).
fn enumerator_values(e: &EnumDef) -> Vec<i32> {
    let mut values = Vec::with_capacity(e.enumerators.len());
    let mut next: i64 = 0;
    for en in &e.enumerators {
        let explicit = en.annotations.iter().find_map(|a| match lower_single(a) {
            Ok(Some(BuiltinAnnotation::Value(s))) => parse_int(&s),
            _ => None,
        });
        let v = explicit.unwrap_or(next);
        values.push(v as i32);
        next = i64::from(v as i32) + 1;
    }
    values
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

/// OCaml type identifier (lower-cased) from an IDL name, escaped if the
/// lower-cased form collides with an OCaml reserved word.
fn type_ident(name: &str) -> String {
    let mut c = name.chars();
    let lowered = match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    };
    escape_ocaml_ident(&lowered)
}

/// Emits an IDL `enum` as an OCaml variant type + a `<name>_to_int` function.
fn emit_enum(out: &mut String, e: &EnumDef) {
    let values = enumerator_values(e);
    let ty = type_ident(&e.name.text);
    let ctors: Vec<String> = e
        .enumerators
        .iter()
        .map(|en| en.name.text.clone())
        .collect();
    let _ = writeln!(
        out,
        "
type {ty} = {}",
        ctors.join(" | ")
    );
    let arms: Vec<String> = e
        .enumerators
        .iter()
        .zip(&values)
        .map(|(en, v)| format!("{} -> {v}", en.name.text))
        .collect();
    let _ = writeln!(out, "let {ty}_to_int = function {}", arms.join(" | "));
    // Inverse (decode): total function; unknown values fall back to the first
    // constructor (never hit for values produced by `_to_int`).
    let of_arms: Vec<String> = e
        .enumerators
        .iter()
        .zip(&values)
        .map(|(en, v)| format!("{v} -> {}", en.name.text))
        .collect();
    let fallback = ctors.first().cloned().unwrap_or_default();
    let _ = writeln!(
        out,
        "let {ty}_of_int = function {} | _ -> {fallback}",
        of_arms.join(" | ")
    );
}

fn extensibility(s: &StructDef) -> ExtensibilityKind {
    lower_annotations(&s.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable)
}

/// OCaml module identifier from an IDL name (upper-cases the first letter).
fn module_name(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
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

/// Collects top-level `struct` definitions as name -> `StructDef`, so a
/// nested-struct `@key` member can be expanded to its own `@key` subset
/// (Bug A) and `keyhash::uses_md5` can resolve a struct-typed `@key` member's
/// size instead of unconditionally forcing the MD5 branch (Bug B).
fn collect_struct_defs(spec: &Specification) -> HashMap<String, &StructDef> {
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

/// Inline UTF-8→UTF-16-code-unit-count lambda for a `wstring<N>` bound
/// check (B1 follow-up, XTypes 1.3 §7.4.3): mirrors `Wire.put_wstring`'s own
/// unit-counting loop (see `WIRE_MODULE` above) so the check counts the same
/// units the wire actually carries. No shared `Wire.*` helper is added for
/// this — `WIRE_MODULE` must stay byte-identical to the hand-written
/// `endpoints/ocaml` mirror, out of scope for this fix — so the lambda is
/// inlined at each bound-checked `wstring<N>` call site instead.
const UTF16_UNIT_COUNT_FN: &str = "(fun __zds -> let __zdi = ref 0 in let __zdc = ref 0 in let __zdn = String.length __zds in while !__zdi < __zdn do let __zdd = String.get_utf_8_uchar __zds !__zdi in let __zdcp = Uchar.to_int (Uchar.utf_decode_uchar __zdd) in __zdi := !__zdi + Uchar.utf_decode_length __zdd; __zdc := !__zdc + (if __zdcp <= 0xFFFF then 1 else 2) done; !__zdc)";

/// Evaluates an IDL bound (`string<N>` / `sequence<T,N>` / `map<K,V,N>`) to
/// its integer literal for a generated bound-check message/comparison.
/// zerodds-lint: recursion-depth 32
fn bound_literal(e: &ConstExpr, construct: &str) -> Result<i64> {
    array_size(e)
        .ok_or_else(|| IdlOcamlError::Unsupported(format!("non-literal {construct} bound")))
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

/// Wraps a per-element put (`$elem`) in nested row-major `for … done` loops over
/// a fixed array `v.<field>.(zdi0).(zdi1)…`.
fn build_array_put(field: &str, sizes: &[i64], elem_put: &str) -> String {
    let idx: String = (0..sizes.len()).map(|k| format!(".(zdi{k})")).collect();
    let mut body = elem_put.replace("$elem", &format!("v.{field}{idx}"));
    for k in (0..sizes.len()).rev() {
        body = format!("for zdi{k} = 0 to {} do\n{body}\ndone", sizes[k] - 1);
    }
    body
}

/// Maps an IDL union `switch` type to a `TypeSpec` so the discriminator reuses
/// the normal `map_type` path (integer family, char, boolean, or a named enum).
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
/// member field name, its language type, and the per-member put statement.
struct UnionCase {
    labels: Vec<i64>,
    is_default: bool,
    field: String,
    ty: String,
    put: String,
    get: String,
}

fn emit_struct(
    out: &mut String,
    s: &StructDef,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    struct_defs: &HashMap<String, &StructDef>,
) -> Result<()> {
    let ext = extensibility(s);

    struct FieldGen {
        name: String,
        ocaml_type: String,
        put: String,
        get: String,
        id: u32,
        key: bool,
        resolved_type: TypeSpec,
        array_sizes: Option<Vec<i64>>,
    }
    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    for m in &s.members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        for d in &m.declarators {
            let name = escape_ocaml_ident(&d.name().text);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let mut array_sizes: Option<Vec<i64>> = None;
            let (ocaml_type, put, get) = match d {
                Declarator::Simple(_) => {
                    let (t, p) =
                        map_type(&resolved, &format!("v.{name}"), enum_names, struct_names)?;
                    let g = map_get(&resolved, enum_names, struct_names)?;
                    (t, p, g)
                }
                // Fixed array: elements inline, row-major, no length prefix.
                Declarator::Array(ad) => {
                    let sizes = ad
                        .sizes
                        .iter()
                        .map(array_size)
                        .collect::<Option<Vec<i64>>>()
                        .ok_or_else(|| {
                            IdlOcamlError::Unsupported(format!(
                                "non-literal array size on `{name}`"
                            ))
                        })?;
                    let (elem_type, elem_put) =
                        map_type(&resolved, "$elem", enum_names, struct_names)?;
                    let ocaml_type = sizes
                        .iter()
                        .fold(elem_type.clone(), |inner, _| format!("{inner} array"));
                    let put = build_array_put(&name, &sizes, &elem_put);
                    let elem_get = map_get(&resolved, enum_names, struct_names)?;
                    let get = build_array_get(&sizes, &elem_type, &elem_get)?;
                    array_sizes = Some(sizes);
                    (ocaml_type, put, get)
                }
            };
            fields.push(FieldGen {
                name,
                ocaml_type,
                put,
                get,
                id,
                key,
                resolved_type: resolved.clone(),
                array_sizes,
            });
        }
    }

    let module = module_name(&s.name.text);
    let _ = writeln!(out, "\nmodule {module} = struct");
    let _ = writeln!(out, "  type t = {{");
    for f in &fields {
        let _ = writeln!(out, "    {} : {};", f.name, f.ocaml_type);
    }
    let _ = writeln!(out, "  }}");

    // marshal_into writes into an existing writer (nested composites call this so
    // alignment stays stream-relative). @final: inline; @appendable: DHEADER.
    let _ = writeln!(
        out,
        "\n  let marshal_into (v : t) (w : Wire.writer) (endian : Wire.endian) : unit ="
    );
    let _ = writeln!(out, "    ignore endian;");
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "    let body = Wire.writer endian in");
        for f in &fields {
            let emh = 0x4000_0000_u32 | f.id;
            let _ = writeln!(out, "    Wire.put_u32 body 0x{emh:08x};");
            let _ = writeln!(out, "    let zdMem = Wire.writer endian in");
            let _ = writeln!(out, "    {};", f.put.replace("$w", "zdMem"));
            let _ = writeln!(
                out,
                "    Wire.put_u32 body (Bytes.length (Wire.bytes zdMem));"
            );
            let _ = writeln!(out, "    Wire.put_bytes body (Wire.bytes zdMem);");
        }
        let _ = writeln!(out, "    Wire.put_u32 w (Bytes.length (Wire.bytes body));");
        let _ = writeln!(out, "    Wire.put_bytes w (Wire.bytes body)");
    } else {
        let wv = if ext == ExtensibilityKind::Final {
            "w"
        } else {
            let _ = writeln!(out, "    let body = Wire.writer endian in");
            "body"
        };
        for f in &fields {
            let _ = writeln!(out, "    {};", f.put.replace("$w", wv));
        }
        if ext != ExtensibilityKind::Final {
            let _ = writeln!(out, "    let bb = Wire.bytes body in");
            let _ = writeln!(out, "    Wire.put_u32 w (Bytes.length bb);");
            let _ = writeln!(out, "    Wire.put_bytes w bb");
        } else {
            let _ = writeln!(out, "    ()");
        }
    }

    let _ = writeln!(
        out,
        "\n  let marshal (v : t) (endian : Wire.endian) : bytes ="
    );
    let _ = writeln!(out, "    let w = Wire.writer endian in");
    let _ = writeln!(out, "    marshal_into v w endian;");
    let _ = writeln!(out, "    Wire.bytes w");
    let mut zdkeys: Vec<&FieldGen> = fields.iter().filter(|f| f.key).collect();
    zdkeys.sort_by_key(|f| f.id);
    if !zdkeys.is_empty() {
        let key_members: Vec<&Member> = s
            .members
            .iter()
            .filter(|m| {
                lower_annotations(&m.annotations)
                    .map(|l| l.has_key())
                    .unwrap_or(false)
            })
            .collect();
        let use_md5 = zerodds_idl::keyhash::uses_md5(&key_members, struct_defs, typedefs);
        let _ = writeln!(out, "\n  let key_hash (v : t) : bytes =");
        let _ = writeln!(out, "    let kw = Wire.writer Wire.BE in");
        for f in &zdkeys {
            // Bug A: a struct-typed `@key` member must expand to that
            // struct's own `@key` subset (or ALL its members if it declares
            // none — XTypes 1.3 §7.6.8), not the full member set that
            // `f.put` (shared with normal, non-key encoding) would emit via
            // `marshal_into`. Non-struct key fields are unaffected: `key_put`
            // falls back to the same `map_type` put used for `f.put`.
            let is_struct_key = matches!(&f.resolved_type, TypeSpec::Scoped(sn)
                if struct_defs.contains_key(&sn.parts.last().map(|p| p.text.clone()).unwrap_or_default()));
            let put = if is_struct_key {
                match &f.array_sizes {
                    None => key_put(
                        &format!("v.{}", f.name),
                        &f.resolved_type,
                        enum_names,
                        struct_names,
                        struct_defs,
                        typedefs,
                    )?,
                    Some(sizes) => {
                        let elem_put = key_put(
                            "$elem",
                            &f.resolved_type,
                            enum_names,
                            struct_names,
                            struct_defs,
                            typedefs,
                        )?;
                        build_array_put(&f.name, sizes, &elem_put)
                    }
                }
            } else {
                f.put.clone()
            };
            let _ = writeln!(out, "    {};", put.replace("$w", "kw"));
        }
        let _ = writeln!(out, "    let b = Wire.bytes kw in");
        if use_md5 {
            // KeyHolder max size > 16 → MD5(bytes)[0..16] (XTypes §7.6.8.4).
            let _ = writeln!(out, "    Bytes.of_string (Digest.bytes b)");
        } else {
            let _ = writeln!(out, "    let outk = Bytes.make 16 '\\000' in");
            let _ = writeln!(out, "    Bytes.blit b 0 outk 0 (min 16 (Bytes.length b));");
            let _ = writeln!(out, "    outk");
        }
    }

    // Decode (inverse of marshal_into). Records are immutable, so each field is
    // read (in order) into a `let` binding and the record is built at the end.
    // @final reads inline, @appendable skips the DHEADER, @mutable skips DHEADER
    // then per member EMHEADER + NEXTINT (members in declaration order).
    let _ = writeln!(out, "\n  let read (r : Wire.reader) : t =");
    if ext == ExtensibilityKind::Mutable {
        let _ = writeln!(out, "    ignore (Wire.get_u32 r);");
        for f in &fields {
            let g = f.get.replace("$r", "r");
            let _ = writeln!(
                out,
                "    let {} = (ignore (Wire.get_u32 r); ignore (Wire.get_u32 r); {g}) in",
                f.name
            );
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "    ignore (Wire.get_u32 r);");
        }
        for f in &fields {
            let _ = writeln!(out, "    let {} = {} in", f.name, f.get.replace("$r", "r"));
        }
    }
    let rec_fields = fields
        .iter()
        .map(|f| f.name.clone())
        .collect::<Vec<_>>()
        .join("; ");
    let _ = writeln!(out, "    {{ {rec_fields} }}");
    let _ = writeln!(
        out,
        "\n  let unmarshal (b : bytes) (endian : Wire.endian) : t ="
    );
    let _ = writeln!(out, "    read (Wire.reader b endian)");
    let _ = writeln!(out, "end");
    Ok(())
}

/// Emits an IDL `union` as a discriminated holder + a `marshalInto` that puts
/// the discriminator then dispatches on it to the selected member (XCDR2
/// §7.4.3.5.4). `@final`: inline; `@appendable`: DHEADER-framed body.
fn emit_union(
    out: &mut String,
    u: &UnionDef,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<()> {
    let ext = lower_annotations(&u.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable);
    if ext == ExtensibilityKind::Mutable {
        return Err(IdlOcamlError::Unsupported(format!(
            "@mutable union {} (EMHEADER framing not yet emitted)",
            u.name.text
        )));
    }
    let (disc_type, disc_put) = map_type(
        &switch_typespec(&u.switch_type),
        "v.disc",
        enum_names,
        struct_names,
    )?;
    let disc_get = map_get(&switch_typespec(&u.switch_type), enum_names, struct_names)?;
    let mut cases: Vec<UnionCase> = Vec::new();
    for c in &u.cases {
        let field = escape_ocaml_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ty, put) = map_type(&resolved, &format!("v.{field}"), enum_names, struct_names)?;
        let get = map_get(&resolved, enum_names, struct_names)?;
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => labels.push(array_size(e).ok_or_else(|| {
                    IdlOcamlError::Unsupported(format!(
                        "non-integer union label in `{}`",
                        u.name.text
                    ))
                })?),
            }
        }
        cases.push(UnionCase {
            labels,
            is_default,
            field,
            ty,
            put,
            get,
        });
    }
    let has_default = cases.iter().any(|c| c.is_default);

    let module = module_name(&u.name.text);
    let _ = writeln!(out, "\nmodule {module} = struct");
    let _ = writeln!(out, "  type t = {{");
    let _ = writeln!(out, "    disc : {disc_type};");
    for c in &cases {
        let _ = writeln!(out, "    {} : {};", c.field, c.ty);
    }
    let _ = writeln!(out, "  }}");
    let _ = writeln!(
        out,
        "\n  let marshal_into (v : t) (w : Wire.writer) (endian : Wire.endian) : unit ="
    );
    let _ = writeln!(out, "    ignore endian;");
    let wv = if ext == ExtensibilityKind::Final {
        "w"
    } else {
        let _ = writeln!(out, "    let body = Wire.writer endian in");
        "body"
    };
    let _ = writeln!(out, "    {};", disc_put.replace("$w", wv));
    let _ = writeln!(out, "    (match v.disc with");
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "     | _ -> {}", c.put.replace("$w", wv));
        } else {
            let lbl = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(" | ");
            let _ = writeln!(out, "     | {lbl} -> {}", c.put.replace("$w", wv));
        }
    }
    if !has_default {
        let _ = writeln!(out, "     | _ -> ()");
    }
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "    );");
        let _ = writeln!(out, "    let bb = Wire.bytes body in");
        let _ = writeln!(out, "    Wire.put_u32 w (Bytes.length bb);");
        let _ = writeln!(out, "    Wire.put_bytes w bb");
    } else {
        let _ = writeln!(out, "    )");
    }
    let _ = writeln!(
        out,
        "\n  let marshal (v : t) (endian : Wire.endian) : bytes ="
    );
    let _ = writeln!(out, "    let w = Wire.writer endian in");
    let _ = writeln!(out, "    marshal_into v w endian;");
    let _ = writeln!(out, "    Wire.bytes w");

    // Decode: read the discriminator, then build the record reading only the
    // selected member (others get a default). @appendable skips the DHEADER.
    let defaults: Vec<String> = cases
        .iter()
        .map(|c| ocaml_default(&c.ty))
        .collect::<Result<Vec<_>>>()?;
    // Renders the record literal for the branch that selects `sel` (None = the
    // catch-all: every member defaulted).
    let record_for = |sel: Option<&UnionCase>| -> String {
        let body = cases
            .iter()
            .zip(&defaults)
            .map(|(c, def)| {
                let is_sel = sel.is_some_and(|s| s.field == c.field);
                let val = if is_sel {
                    c.get.replace("$r", "r")
                } else {
                    def.clone()
                };
                format!("{} = {val}", c.field)
            })
            .collect::<Vec<_>>()
            .join("; ");
        format!("{{ disc; {body} }}")
    };
    let _ = writeln!(out, "\n  let read (r : Wire.reader) : t =");
    if ext == ExtensibilityKind::Appendable {
        let _ = writeln!(out, "    ignore (Wire.get_u32 r);");
    }
    let _ = writeln!(out, "    let disc = {} in", disc_get.replace("$r", "r"));
    let _ = writeln!(out, "    (match disc with");
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "     | _ -> {}", record_for(Some(c)));
        } else {
            let lbl = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(" | ");
            let _ = writeln!(out, "     | {lbl} -> {}", record_for(Some(c)));
        }
    }
    if !has_default {
        let _ = writeln!(out, "     | _ -> {}", record_for(None));
    }
    let _ = writeln!(out, "    )");
    let _ = writeln!(
        out,
        "\n  let unmarshal (b : bytes) (endian : Wire.endian) : t ="
    );
    let _ = writeln!(out, "    read (Wire.reader b endian)");
    let _ = writeln!(out, "end");
    Ok(())
}

/// Maps an IDL type to `(OCaml type, put statement)`. The put uses `$w` as the
/// writer placeholder and `expr` as the value expression.
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

/// Builds a map put (OCaml assoc list): sort by key, then `u32 count` + key/value
/// pairs (DHEADER-framed unless the key/value pair is primitive). `endian` is in
/// scope in `marshal_into`.
fn build_map_put(
    expr: &str,
    key_put: &str,
    val_put: &str,
    prim: bool,
    bound: Option<&ConstExpr>,
) -> Result<String> {
    let sorted = format!("List.sort (fun (a, _) (b, _) -> compare a b) {expr}");
    // Bounded `map<K,V,N>` (DDS-XTypes §7.4.3): reject an over-length map
    // before it hits the wire (B1 follow-up, mirrors the other backends'
    // encode-side map bound check). Checked against the ORIGINAL list
    // (sorting doesn't change its length).
    let bound_check = match bound {
        Some(b) => {
            let n = bound_literal(b, "map")?;
            format!(
                "if List.length {expr} > {n} then failwith \"bounded map length exceeds its IDL bound ({n})\";\n     "
            )
        }
        None => String::new(),
    };
    if prim {
        Ok(format!(
            "({bound_check}let zdSorted = {sorted} in\n     Wire.put_u32 $w (List.length zdSorted);\n     List.iter (fun (zdKk, zdKv) -> {key_put}; {val_put}) zdSorted)"
        ))
    } else {
        let kp = key_put.replace("$w", "zdSub");
        let vp = val_put.replace("$w", "zdSub");
        Ok(format!(
            "({bound_check}let zdSorted = {sorted} in\n     let zdSub = Wire.writer endian in\n     Wire.put_u32 zdSub (List.length zdSorted);\n     List.iter (fun (zdKk, zdKv) -> {kp}; {vp}) zdSorted;\n     let zdBB = Wire.bytes zdSub in\n     Wire.put_u32 $w (Bytes.length zdBB);\n     Wire.put_bytes $w zdBB)"
        ))
    }
}

/// zerodds-lint: recursion-depth 32
fn map_type(
    t: &TypeSpec,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    match t {
        TypeSpec::Primitive(p) => map_primitive(*p, expr),
        // Bounded `string<N>` / `wstring<N>` (DDS-XTypes §7.4.3): reject an
        // over-bound value before it hits the wire, like the other backends'
        // encode-side checks (idl-cpp `emit_value_write`, idl-csharp
        // `emit_encode_value`, idl-java `emit_typespec_encode`). Narrow uses
        // UTF-8 byte length (matches the CDR wire, `put_string`'s own length
        // prefix); `wstring<N>` uses the UTF-16 code-unit count (matches
        // `put_wstring`'s own unit count below) via an inline UTF-8-decode
        // loop — no shared `Wire` helper is added since `WIRE_MODULE` below
        // must stay byte-identical to the hand-written `endpoints/ocaml`
        // copy (out of scope here).
        TypeSpec::String(st) if !st.wide => {
            let put = match &st.bound {
                Some(b) => {
                    let n = bound_literal(b, "string")?;
                    format!(
                        "(if String.length {expr} > {n} then failwith \"bounded string length exceeds its IDL bound ({n})\"); Wire.put_string $w {expr}"
                    )
                }
                None => format!("Wire.put_string $w {expr}"),
            };
            Ok(("string".to_string(), put))
        }
        TypeSpec::String(st) => {
            let put = match &st.bound {
                Some(b) => {
                    let n = bound_literal(b, "wstring")?;
                    format!(
                        "(if {UTF16_UNIT_COUNT_FN} {expr} > {n} then failwith \"bounded wstring length exceeds its IDL bound ({n})\"); Wire.put_wstring $w {expr}"
                    )
                }
                None => format!("Wire.put_wstring $w {expr}"),
            };
            Ok(("string".to_string(), put))
        }
        TypeSpec::Sequence(seq) => map_sequence(&seq.elem, expr, seq.bound.as_ref(), struct_names),
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                let ty = type_ident(&name);
                Ok((ty.clone(), format!("Wire.put_u32 $w ({ty}_to_int {expr})")))
            } else if struct_names.contains(&name) {
                let m = module_name(&name);
                Ok((
                    format!("{m}.t"),
                    format!("{m}.marshal_into {expr} $w endian"),
                ))
            } else {
                Err(IdlOcamlError::Unsupported(format!("scoped type {name}")))
            }
        }
        // A map (assoc list): sorted ascending by key, `u32 count` + key/value
        // pairs (no DHEADER for a primitive pair; DHEADER-framed otherwise).
        TypeSpec::Map(m) => {
            let (key_type, key_put) = map_type(&m.key, "zdKk", enum_names, struct_names)?;
            let (val_type, val_put) = map_type(&m.value, "zdKv", enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            Ok((
                format!("({key_type} * {val_type}) list"),
                build_map_put(expr, &key_put, &val_put, prim, m.bound.as_ref())?,
            ))
        }
        other => Err(IdlOcamlError::Unsupported(format!("type {other:?}"))),
    }
}

/// Builds a KeyHash-writer statement (using the `$w` placeholder like
/// `map_type`'s put strings) for one `@key` member value. Reuses the shared
/// per-field `map_type` put for primitive/string/enum/sequence/typedef
/// members — safe, since normal and key encoding agree there. For a
/// struct-typed key member (Bug A: nested-struct `@key` member must not
/// include non-key fields, XTypes 1.3 §7.6.8), does NOT call the struct's
/// full `marshal_into`; instead expands to the struct's own `@key` members
/// (or ALL members if it declares none), in member-id order, recursing for
/// further nesting. Rejects (loud error) an array-typed field found inside a
/// nested-struct key — mirrors the `idl-rust` reference fix
/// (`emit_key_field_write`), which rejects the same shape.
///
/// zerodds-lint: recursion-depth 16 (nested `@key` struct expansion; bounded
/// by the IDL's aggregate nesting depth).
fn key_put(
    expr: &str,
    type_spec: &TypeSpec,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    struct_defs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<String> {
    let resolved = resolve_typedef(type_spec, typedefs);
    if let TypeSpec::Scoped(sn) = &resolved {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if let Some(sd) = struct_defs.get(&name) {
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
                .map(|(i, m)| {
                    let id = lower_annotations(&m.annotations)
                        .ok()
                        .and_then(|l| l.explicit_id())
                        .unwrap_or(i as u32);
                    (id, *m)
                })
                .collect();
            ordered.sort_by_key(|(id, _)| *id);
            let mut stmts: Vec<String> = Vec::new();
            for (_, m) in &ordered {
                for d in &m.declarators {
                    if matches!(d, Declarator::Array(_)) {
                        return Err(IdlOcamlError::Unsupported(
                            "array @key field inside a nested-struct key".to_string(),
                        ));
                    }
                    let field = d.name().text.clone();
                    stmts.push(key_put(
                        &format!("{expr}.{field}"),
                        &m.type_spec,
                        enum_names,
                        struct_names,
                        struct_defs,
                        typedefs,
                    )?);
                }
            }
            return Ok(stmts.join("; "));
        }
    }
    let (_, put) = map_type(&resolved, expr, enum_names, struct_names)?;
    Ok(put)
}

fn map_primitive(p: PrimitiveType, expr: &str) -> Result<(String, String)> {
    let (ty, put) = match p {
        PrimitiveType::Octet => ("int", format!("Wire.put_u8 $w {expr}")),
        PrimitiveType::Boolean => ("bool", format!("Wire.put_bool $w {expr}")),
        PrimitiveType::Char => ("char", format!("Wire.put_u8 $w (Char.code {expr})")),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => {
            ("float", format!("Wire.put_f32 $w {expr}"))
        }
        PrimitiveType::Floating(FloatingType::Double) => {
            ("float", format!("Wire.put_f64 $w {expr}"))
        }
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            ("float", format!("Wire.put_long_double $w {expr}"))
        }
        PrimitiveType::WideChar => ("int", format!("Wire.put_u32 $w {expr}")),
    };
    Ok((ty.to_string(), put))
}

fn map_integer(i: IntegerType, expr: &str) -> Result<(String, String)> {
    // OCaml's 63-bit int holds every 8/16/32-bit value; 64-bit uses Int64.
    let (ty, put) = match i {
        IntegerType::Int8 | IntegerType::UInt8 => ("int", format!("Wire.put_u8 $w {expr}")),
        IntegerType::Short | IntegerType::UShort | IntegerType::Int16 | IntegerType::UInt16 => {
            ("int", format!("Wire.put_u16 $w {expr}"))
        }
        IntegerType::Long | IntegerType::ULong | IntegerType::Int32 | IntegerType::UInt32 => {
            ("int", format!("Wire.put_u32 $w {expr}"))
        }
        IntegerType::LongLong
        | IntegerType::ULongLong
        | IntegerType::Int64
        | IntegerType::UInt64 => ("int64", format!("Wire.put_u64 $w {expr}")),
    };
    Ok((ty.to_string(), put))
}

fn map_sequence(
    elem: &TypeSpec,
    expr: &str,
    bound: Option<&ConstExpr>,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    // Bounded `sequence<T,N>` (DDS-XTypes §7.4.3): reject an over-length
    // list before it hits the wire (B1 follow-up, mirrors the other
    // backends' encode-side sequence bound check).
    let bound_check = match bound {
        Some(b) => {
            let n = bound_literal(b, "sequence")?;
            format!(
                "(if List.length {expr} > {n} then failwith \"bounded sequence length exceeds its IDL bound ({n})\"); "
            )
        }
        None => String::new(),
    };
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok((
            "bytes".to_string(),
            format!("{bound_check}Wire.put_seq_u8 $w {expr}"),
        ));
    }
    // sequence<struct> → collection DHEADER + count + each element.
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let m = module_name(&name);
            let put = format!(
                "({bound_check}let sub = Wire.writer endian in                  Wire.put_u32 sub (List.length {expr});                  List.iter (fun e -> {m}.marshal_into e sub endian) {expr};                  let bb = Wire.bytes sub in                  Wire.put_u32 $w (Bytes.length bb); Wire.put_bytes $w bb)"
            );
            return Ok((format!("{m}.t list"), put));
        }
    }
    Err(IdlOcamlError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}

// ---- decode (inverse of the put path): a `Wire.reader` wire-core in the module,
// plus `map_get` — the inverse of `map_type` — returning an EXPRESSION that reads
// one value from `$r`. OCaml records are immutable, so each field is read into a
// `let` binding and the record is built at the end. Roundtrip-verified.

/// A zero value of an OCaml type, used to pre-allocate fixed arrays before the
/// row-major fill (the values are all overwritten).
fn ocaml_default(t: &str) -> Result<String> {
    let d = if t == "int" {
        "0"
    } else if t == "int64" {
        "0L"
    } else if t == "float" {
        "0.0"
    } else if t == "bool" {
        "false"
    } else if t == "char" {
        "'\\000'"
    } else if t == "string" {
        "\"\""
    } else if t == "bytes" {
        "Bytes.empty"
    } else if t.ends_with(" array") {
        "[||]"
    } else {
        return Err(IdlOcamlError::Unsupported(format!(
            "no default for array element type `{t}`"
        )));
    };
    Ok(d.to_string())
}

/// Reads a fixed array: allocate the nested `array` then row-major `for` loops
/// filling each element (inverse of [`build_array_put`]). `elem_get` is an
/// expression reading one element from `$r`.
fn build_array_get(sizes: &[i64], elem_type: &str, elem_get: &str) -> Result<String> {
    let default = ocaml_default(elem_type)?;
    // Allocation: innermost `Array.make s d`, outer `Array.init s (fun _ -> …)`.
    fn alloc(sizes: &[i64], default: &str) -> String {
        if sizes.len() == 1 {
            format!("Array.make {} {default}", sizes[0])
        } else {
            format!(
                "Array.init {} (fun _ -> {})",
                sizes[0],
                alloc(&sizes[1..], default)
            )
        }
    }
    let idx: String = (0..sizes.len()).map(|k| format!(".(zdi{k})")).collect();
    let mut fill = format!("zda{idx} <- {elem_get}");
    for k in (0..sizes.len()).rev() {
        fill = format!("for zdi{k} = 0 to {} do {fill} done", sizes[k] - 1);
    }
    Ok(format!(
        "(let zda = {} in {fill}; zda)",
        alloc(sizes, &default)
    ))
}

/// Returns an expression reading one value of IDL type `t` from `$r`.
/// zerodds-lint: recursion-depth 32
fn map_get(
    t: &TypeSpec,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    match t {
        TypeSpec::Primitive(p) => map_get_primitive(*p),
        // Bounded `string<N>` / `wstring<N>` decode (B1 follow-up, XTypes
        // 1.3 §7.4.3): mirror the encode-side check above on decode too — a
        // well-formed-but-oversized wire value must be rejected, not just
        // whatever the wire's own remaining-byte check already does.
        //
        // Documented tradeoff (deep review of #22 decode-bounds-cross-backend,
        // moderate item): this checks the length AFTER `Wire.get_string`/
        // `Wire.get_wstring` has already materialized the value, not before.
        // That is the SAME order idl-rust's own reference decoder uses
        // (`struct_emit.rs::emit_decode_bound_checks`: "checked post-decode,
        // not pre-encode, since the value already exists in memory by the
        // time this runs") — intentional, not an oversight, for a single
        // primitive-collection read: its cost is bounded by the wire's own
        // remaining-buffer length, not by the attacker-declared bound, so
        // there's no separate amplification to guard against here. Checking
        // BEFORE materializing would require duplicating `Wire.get_string`'s
        // length-prefix-then-copy logic outside the `Wire` module, which is
        // kept byte-identical to the hand-maintained `endpoints/ocaml` copy
        // (see the module's own doc comment) — out of scope for this fix.
        // Contrast `map_get_sequence` below: a `sequence<struct, N>`'s
        // per-element DECODE LOOP is checked BEFORE it runs (right after
        // reading the wire count, before the loop), because there the
        // attacker-controlled count drives repeated allocation/decode work,
        // not a single bounded read.
        TypeSpec::String(st) if !st.wide => match &st.bound {
            Some(b) => {
                let n = bound_literal(b, "string")?;
                Ok(format!(
                    "(let __zdv = Wire.get_string $r in if String.length __zdv > {n} then failwith \"decoded string length exceeds its IDL bound ({n})\" else __zdv)"
                ))
            }
            None => Ok("(Wire.get_string $r)".to_string()),
        },
        TypeSpec::String(st) => match &st.bound {
            Some(b) => {
                let n = bound_literal(b, "wstring")?;
                Ok(format!(
                    "(let __zdv = Wire.get_wstring $r in if {UTF16_UNIT_COUNT_FN} __zdv > {n} then failwith \"decoded wstring length exceeds its IDL bound ({n})\" else __zdv)"
                ))
            }
            None => Ok("(Wire.get_wstring $r)".to_string()),
        },
        TypeSpec::Sequence(seq) => map_get_sequence(&seq.elem, seq.bound.as_ref(), struct_names),
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                let ty = type_ident(&name);
                Ok(format!("({ty}_of_int (Wire.get_u32 $r))"))
            } else if struct_names.contains(&name) {
                let m = module_name(&name);
                Ok(format!("({m}.read $r)"))
            } else {
                Err(IdlOcamlError::Unsupported(format!("scoped type {name}")))
            }
        }
        TypeSpec::Map(m) => {
            let key_get = map_get(&m.key, enum_names, struct_names)?;
            let val_get = map_get(&m.value, enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let dh = if prim {
                ""
            } else {
                "ignore (Wire.get_u32 $r); "
            };
            // B1 follow-up (#22 decode-side parity): mirror the encode-side
            // map bound check on decode — XTypes 1.3 §7.4.3.
            let bound_check = match &m.bound {
                Some(b) => {
                    let n = bound_literal(b, "map")?;
                    format!(
                        "if zdn > {n} then failwith \"decoded map length exceeds its IDL bound ({n})\"; "
                    )
                }
                None => String::new(),
            };
            Ok(format!(
                "({dh}let zdn = Wire.get_u32 $r in {bound_check}let rec zdloop k acc = if k = 0 then List.rev acc else (let zdk = {key_get} in let zdv = {val_get} in zdloop (k - 1) ((zdk, zdv) :: acc)) in zdloop zdn [])"
            ))
        }
        other => Err(IdlOcamlError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType) -> Result<String> {
    let e = match p {
        PrimitiveType::Octet => "(Wire.get_u8 $r)",
        PrimitiveType::Char => "(Char.chr (Wire.get_u8 $r))",
        PrimitiveType::Boolean => "(Wire.get_bool $r)",
        PrimitiveType::Integer(i) => return map_get_integer(i),
        PrimitiveType::Floating(FloatingType::Float) => "(Wire.get_f32 $r)",
        PrimitiveType::Floating(FloatingType::Double) => "(Wire.get_f64 $r)",
        PrimitiveType::Floating(FloatingType::LongDouble) => "(Wire.get_long_double $r)",
        PrimitiveType::WideChar => "(Wire.get_u32 $r)",
    };
    Ok(e.to_string())
}

fn map_get_integer(i: IntegerType) -> Result<String> {
    let e = match i {
        IntegerType::Int8 | IntegerType::UInt8 => "(Wire.get_u8 $r)",
        IntegerType::Short | IntegerType::UShort | IntegerType::Int16 | IntegerType::UInt16 => {
            "(Wire.get_u16 $r)"
        }
        IntegerType::Long | IntegerType::ULong | IntegerType::Int32 | IntegerType::UInt32 => {
            "(Wire.get_u32 $r)"
        }
        IntegerType::LongLong
        | IntegerType::ULongLong
        | IntegerType::Int64
        | IntegerType::UInt64 => "(Wire.get_u64 $r)",
    };
    Ok(e.to_string())
}

fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    // B1 follow-up (#22 decode-side parity): mirror the encode-side
    // sequence bound check on decode — XTypes 1.3 §7.4.3.
    //
    // Documented tradeoff (moderate item, deep review of #22): like the
    // narrow string/wstring case in `map_get` above, this octet sequence
    // checks AFTER `Wire.get_seq_u8` has already read the bytes — a single
    // bounded-by-remaining-buffer read, not a decode loop, so there's no
    // amplification vector (matches idl-rust's own post-decode-check
    // convention). The struct-element branch below is different: its count
    // drives a decode LOOP, so it's checked BEFORE the loop starts.
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return match bound {
            Some(b) => {
                let n = bound_literal(b, "sequence")?;
                Ok(format!(
                    "(let __zdv = Wire.get_seq_u8 $r in if Bytes.length __zdv > {n} then failwith \"decoded sequence length exceeds its IDL bound ({n})\" else __zdv)"
                ))
            }
            None => Ok("(Wire.get_seq_u8 $r)".to_string()),
        };
    }
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let m = module_name(&name);
            // Checked BEFORE `zdloop` runs (right after reading the wire
            // count `zdn`, before any element is decoded) — an
            // attacker-supplied huge `zdn` must not drive an unbounded
            // decode loop before the bound is ever checked.
            let bound_check = match bound {
                Some(b) => {
                    let n = bound_literal(b, "sequence")?;
                    format!(
                        "if zdn > {n} then failwith \"decoded sequence length exceeds its IDL bound ({n})\"; "
                    )
                }
                None => String::new(),
            };
            return Ok(format!(
                "(ignore (Wire.get_u32 $r); let zdn = Wire.get_u32 $r in {bound_check}let rec zdloop k acc = if k = 0 then List.rev acc else (let e = {m}.read $r in zdloop (k - 1) (e :: acc)) in zdloop zdn [])"
            ));
        }
    }
    Err(IdlOcamlError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}
