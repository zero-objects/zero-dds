// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → Nim emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! Nim source file: a shared XCDR2 `Writer` (byte-identical to `endpoints/nim`)
//! plus, per IDL `struct`, a Nim `object` with a `marshalXCDR(endian)` proc.
//! `@final` and `@appendable` are supported; other extensibilities and
//! constructs raise [`IdlNimError::Unsupported`].

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

use crate::error::{IdlNimError, Result};
use crate::keywords::escape_nim_ident;

/// Options for the Nim backend.
#[derive(Debug, Clone, Default)]
pub struct NimGenOptions {}

/// The shared XCDR2 wire `Writer`, byte-identical to `endpoints/nim`.
const WIRE_PRELUDE: &str = r#"import std/unicode

type Endian* = enum
  eLE
  eBE

type Writer* = object
  buf: seq[byte]
  endian: Endian

proc initWriter*(endian: Endian): Writer =
  Writer(buf: @[], endian: endian)

proc align(w: var Writer, a: int) =
  let cap = min(a, 4)
  let pad = (cap - (w.buf.len mod cap)) mod cap
  for _ in 0 ..< pad:
    w.buf.add(0'u8)

proc put(w: var Writer, a: int, le: seq[byte]) =
  w.align(a)
  if w.endian == eBE:
    for i in countdown(le.high, 0):
      w.buf.add(le[i])
  else:
    for b in le:
      w.buf.add(b)

proc leBytes(v: uint64, n: int): seq[byte] =
  result = newSeq[byte](n)
  for i in 0 ..< n:
    result[i] = byte((v shr (8 * i)) and 0xff)

proc putU8*(w: var Writer, v: int) = w.buf.add(byte(v and 0xff))
proc putBool*(w: var Writer, v: bool) = w.putU8(if v: 1 else: 0)
proc putU16*(w: var Writer, v: int) = w.put(2, leBytes(uint64(v) and 0xFFFF'u64, 2))
proc putU32*(w: var Writer, v: uint32) = w.put(4, leBytes(uint64(v), 4))
proc putU64*(w: var Writer, v: uint64) = w.put(4, leBytes(v, 8))
proc putF32*(w: var Writer, v: float32) = w.put(4, leBytes(uint64(cast[uint32](v)), 4))
proc putF64*(w: var Writer, v: float64) = w.put(4, leBytes(cast[uint64](v), 8))

proc putBytes*(w: var Writer, b: seq[byte]) =
  for x in b:
    w.buf.add(x)

proc putString*(w: var Writer, s: string, maxLen: int = -1) =
  # Moderate fix (deep review of #22 decode-bounds-cross-backend): check the
  # IDL bound BEFORE writing anything, not after — `maxLen = -1` (the
  # default) means unbounded, matching the get-side convention below.
  if maxLen >= 0 and s.len > maxLen:
    raise newException(ValueError, "bounded string length exceeds its IDL bound (" & $maxLen & ")")
  w.putU32(uint32(s.len + 1))
  for c in s:
    w.buf.add(byte(c))
  w.putU8(0)

proc putSeqU8*(w: var Writer, b: seq[byte], maxLen: int = -1) =
  if maxLen >= 0 and b.len > maxLen:
    raise newException(ValueError, "bounded sequence length exceeds its IDL bound (" & $maxLen & ")")
  w.putU32(uint32(b.len))
  w.putBytes(b)

## UTF-16 code-unit count of `s` (surrogate-pair aware: a non-BMP codepoint is
## 2 units), matching the unit count `putWString`/`getWString` themselves
## write/read on the wire. Moderate fix (deep review of #22
## decode-bounds-cross-backend): the previous bound checks used the stdlib
## rune-length proc (Unicode CODEPOINT count), which under-counts a non-BMP codepoint (e.g. an
## emoji: 1 codepoint but 2 UTF-16 units) — the same class of bug flagged for
## idl-elixir's `String.length/1`. DDS-XTypes 1.3 §7.4.3's `wstring<N>` bound
## is in UTF-16 units.
proc wstringUnitLen*(s: string): int =
  result = 0
  for r in s.runes:
    result += (if int(r) <= 0xFFFF: 1 else: 2)

proc putWString*(w: var Writer, s: string, maxUnits: int = -1) =
  if maxUnits >= 0 and wstringUnitLen(s) > maxUnits:
    raise newException(ValueError, "bounded wstring length exceeds its IDL bound (" & $maxUnits & ")")
  var units: seq[uint16] = @[]
  for r in s.runes:
    let cp = int(r)
    if cp <= 0xFFFF:
      units.add(uint16(cp))
    else:
      let rr = cp - 0x10000
      units.add(uint16(0xD800 + (rr shr 10)))
      units.add(uint16(0xDC00 + (rr and 0x3FF)))
  w.putU32(uint32(units.len * 2))
  for u in units:
    w.putU16(int(u))

proc putLongDouble*(w: var Writer, v: float64) =
  let bits = cast[uint64](v)
  let sign = bits shr 63
  let exp = (bits shr 52) and 0x7FF
  let mant = bits and 0xFFFFFFFFFFFFF'u64
  var hi = sign shl 63
  var lo = 0'u64
  if not (exp == 0'u64 and mant == 0'u64):
    hi = (sign shl 63) or ((exp - 1023 + 16383) shl 48) or (mant shr 4)
    lo = (mant and 0xF'u64) shl 60
  var le = newSeq[byte](16)
  for i in 0 ..< 8:
    le[i] = byte((lo shr (8 * i)) and 0xff)
    le[8 + i] = byte((hi shr (8 * i)) and 0xff)
  w.put(4, le)

proc bytes*(w: Writer): seq[byte] = w.buf

type Reader* = object
  buf: seq[byte]
  pos: int
  endian: Endian

proc initReader*(buf: seq[byte], endian: Endian): Reader =
  Reader(buf: buf, pos: 0, endian: endian)

proc ralign(r: var Reader, a: int) =
  let cap = min(a, 4)
  while r.pos mod cap != 0:
    inc r.pos

proc getLE(r: var Reader, a, n: int): uint64 =
  r.ralign(a)
  var v = 0'u64
  if r.endian == eBE:
    for i in 0 ..< n:
      v = (v shl 8) or uint64(r.buf[r.pos + i])
  else:
    for i in countdown(n - 1, 0):
      v = (v shl 8) or uint64(r.buf[r.pos + i])
  r.pos += n
  v

proc getU8*(r: var Reader): int =
  result = int(r.buf[r.pos])
  inc r.pos
proc getBool*(r: var Reader): bool = r.getU8() != 0
proc getU16*(r: var Reader): int = int(r.getLE(2, 2))
proc getU32*(r: var Reader): uint32 = uint32(r.getLE(4, 4))
proc getU64*(r: var Reader): uint64 = r.getLE(4, 8)
proc getF32*(r: var Reader): float32 = cast[float32](r.getU32())
proc getF64*(r: var Reader): float64 = cast[float64](r.getU64())

proc getBytesN*(r: var Reader, n: int): seq[byte] =
  result = r.buf[r.pos ..< r.pos + n]
  r.pos += n

proc getString*(r: var Reader, maxLen: int = -1): string =
  let n = int(r.getU32())
  # Moderate fix (deep review of #22 decode-bounds-cross-backend): check the
  # wire-declared length BEFORE materializing the string — a bound violation
  # used to be checked only after the whole value had already been copied
  # into `result`. `n - 1` is the CDR byte length (n includes the NUL).
  if maxLen >= 0 and n > 0 and (n - 1) > maxLen:
    raise newException(ValueError, "decoded string length exceeds its IDL bound (" & $maxLen & ")")
  result = ""
  if n > 0:
    for i in 0 ..< n - 1:
      result.add(char(r.buf[r.pos + i]))
    r.pos += n

proc getSeqU8*(r: var Reader, maxLen: int = -1): seq[byte] =
  let n = int(r.getU32())
  if maxLen >= 0 and n > maxLen:
    raise newException(ValueError, "decoded sequence length exceeds its IDL bound (" & $maxLen & ")")
  r.getBytesN(n)

proc getWString*(r: var Reader, maxUnits: int = -1): string =
  let n = int(r.getU32()) div 2
  # Moderate fix: check the wire-declared UTF-16 unit count BEFORE reading
  # any code units or decoding to UTF-8, not after (see `wstringUnitLen`'s
  # doc comment above for why this counts units, not `runeLen` codepoints).
  if maxUnits >= 0 and n > maxUnits:
    raise newException(ValueError, "decoded wstring length exceeds its IDL bound (" & $maxUnits & ")")
  var units: seq[uint16] = @[]
  for i in 0 ..< n:
    units.add(uint16(r.getU16()))
  result = ""
  var i = 0
  while i < n:
    let u = int(units[i])
    if u >= 0xD800 and u <= 0xDBFF and i + 1 < n:
      let lo = int(units[i + 1])
      result.add(toUTF8(Rune(0x10000 + ((u - 0xD800) shl 10) + (lo - 0xDC00))))
      i += 2
    else:
      result.add(toUTF8(Rune(u)))
      inc i

proc getLongDouble*(r: var Reader): float64 =
  r.ralign(4)
  var le = r.getBytesN(16)
  if r.endian == eBE:
    for i in 0 ..< 8:
      let t = le[i]
      le[i] = le[15 - i]
      le[15 - i] = t
  var lo = 0'u64
  var hi = 0'u64
  for i in 0 ..< 8:
    lo = lo or (uint64(le[i]) shl (8 * i))
    hi = hi or (uint64(le[8 + i]) shl (8 * i))
  let sign = hi shr 63
  let exp = (hi shr 48) and 0x7FFF'u64
  let mant = ((hi and 0xFFFFFFFFFFFF'u64) shl 4) or (lo shr 60)
  let bits = if exp == 0'u64 and mant == 0'u64: (sign shl 63)
             else: (sign shl 63) or ((exp - 16383 + 1023) shl 52) or mant
  cast[float64](bits)
"#;

/// Generates a self-contained Nim module from the IDL AST.
///
/// # Errors
/// Returns [`IdlNimError::Unsupported`] for constructs the Nim backend does not
/// yet emit (unions, nested-struct members, maps, `long double`, `@mutable`, …).
pub fn generate_nim_module(spec: &Specification, _opts: &NimGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Code generated by zerodds-idlc (Nim backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "# SPDX-License-Identifier: Apache-2.0\n");
    out.push_str(WIRE_PRELUDE);

    // `module X { ... }` content is promoted into the same flat, top-level
    // definition list (see `flatten_module_defs`) so it is no longer
    // silently dropped (swarm59 #21b).
    let flat = flatten_module_defs(&spec.definitions);

    // Named enums: an enum member is a 32-bit signed integer on the wire
    // (XTypes 1.3 §7.4.5.1), byte-identical to the int32/uint32 path.
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

    // Name -> StructDef, so a nested-struct `@key` member's own `@key` subset
    // (and `keyhash::uses_md5`'s static max-size analysis) can be resolved —
    // mirrors `struct_names` above, just keeping the full def instead of only
    // the name.
    let structs: HashMap<String, &StructDef> = spec
        .definitions
        .iter()
        .filter_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                Some((s.name.text.clone(), s))
            }
            _ => None,
        })
        .collect();

    let typedefs = collect_typedefs(spec);

    for def in &flat {
        match def {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => emit_enum(&mut out, e),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                emit_struct(&mut out, s, &enum_names, &struct_names, &structs, &typedefs)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                emit_union(&mut out, u, &enum_names, &struct_names, &typedefs)?;
            }
            _ => {}
        }
    }
    // Top-of-file imports on demand: `tables`+`algorithm` for map members,
    // `md5` for the KeyHash MD5 branch.
    let mut imports = String::new();
    if out.contains("Table[") {
        imports.push_str("import std/tables\nimport std/algorithm\n");
    }
    if out.contains("toMD5(") {
        imports.push_str("import std/md5\n");
    }
    if !imports.is_empty() {
        out = out.replacen(
            "# SPDX-License-Identifier: Apache-2.0\n\n",
            &format!("# SPDX-License-Identifier: Apache-2.0\n\n{imports}\n"),
            1,
        );
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

/// Emits an IDL `enum` as a Nim `enum` with explicit i32 enumerator values.
fn emit_enum(out: &mut String, e: &EnumDef) {
    let values = enumerator_values(e);
    // `raw_ty` prefixes every enumerator (`{raw_ty}{enumerator}` is always a
    // single fused identifier, never a standalone keyword token, so it is
    // never escaped); the `type` declaration itself is a standalone
    // identifier and needs the escaped form.
    let raw_ty = &e.name.text;
    let ty = escape_nim_ident(raw_ty);
    let _ = writeln!(out, "\ntype {ty}* = enum");
    for (en, value) in e.enumerators.iter().zip(&values) {
        let _ = writeln!(out, "  {raw_ty}{} = {value}", en.name.text);
    }
}

fn extensibility(s: &StructDef) -> ExtensibilityKind {
    lower_annotations(&s.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable)
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

/// Evaluates a fixed-array bound to its integer size (literal + unary sign).
/// zerodds-lint: recursion-depth 32
/// Evaluates an IDL-declared bound (`string<N>` / `sequence<T,N>` /
/// `map<K,V,N>`) to its integer value. B1 follow-up (#22 decode-side
/// parity): shares `array_size`'s literal/unary evaluation — an IDL bound is
/// syntactically the same const-expr shape as an array size.
fn bound_value(e: &ConstExpr) -> Option<i64> {
    array_size(e)
}

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

/// Wraps a per-element put (`$elem`) in nested row-major `for` loops over a
/// fixed array `self.<field>[i0][i1]…`. Emits Nim-correct relative indentation
/// (the caller adds a 2-space base to every line).
fn build_array_put(field: &str, sizes: &[i64], elem_put: &str) -> String {
    let idx: String = (0..sizes.len()).map(|k| format!("[i{k}]")).collect();
    let leaf = elem_put.replace("$elem", &format!("self.{field}{idx}"));
    let mut lines = Vec::new();
    for (k, n) in sizes.iter().enumerate() {
        lines.push(format!("{}for i{k} in 0 ..< {n}:", "  ".repeat(k)));
    }
    lines.push(format!("{}{leaf}", "  ".repeat(sizes.len())));
    lines.join("\n")
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
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<()> {
    let ext = extensibility(s);

    struct FieldGen {
        nim_name: String,
        nim_type: String,
        put: String,
        get: String,
        id: u32,
        key: bool,
        // `Some((type_spec, expr))` for a Simple (non-array) declarator, so a
        // `@key` field can be re-mapped through `map_key_type` instead of
        // reusing `put` (which, for a struct-typed member, is the full
        // `marshalInto` call shared with normal, non-key encoding). `None`
        // for an array declarator — `map_key_type` expects a scalar
        // TypeSpec/expr pair and would otherwise encode the array's ELEMENT
        // type once against the whole array value (wrong KeyHash: scalar-
        // encoding a list). Array key fields reuse `put` unchanged instead —
        // it already emits the correct row-major, no-length-prefix element
        // encoding (mirrors `idl-lua`'s `key_type: Option<..>` guard).
        key_type: Option<(TypeSpec, String)>,
    }
    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    for m in &s.members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        for d in &m.declarators {
            let nim_name = escape_nim_ident(&d.name().text);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let (nim_type, put, get, key_type) = match d {
                Declarator::Simple(_) => {
                    let expr = format!("self.{nim_name}");
                    let (t, p) = map_type(&resolved, &expr, enum_names, struct_names)?;
                    let g = map_get(
                        &resolved,
                        &format!("result.{nim_name}"),
                        enum_names,
                        struct_names,
                    )?;
                    (t, p, g, Some((resolved.clone(), expr)))
                }
                // Fixed array: elements inline, row-major, no length prefix.
                Declarator::Array(ad) => {
                    let sizes = ad
                        .sizes
                        .iter()
                        .map(array_size)
                        .collect::<Option<Vec<i64>>>()
                        .ok_or_else(|| {
                            IdlNimError::Unsupported(format!(
                                "non-literal array size on `{nim_name}`"
                            ))
                        })?;
                    let (elem_type, elem_put) =
                        map_type(&resolved, "$elem", enum_names, struct_names)?;
                    let nim_type = sizes
                        .iter()
                        .rev()
                        .fold(elem_type, |inner, n| format!("array[{n}, {inner}]"));
                    let put = build_array_put(&nim_name, &sizes, &elem_put);
                    let idx: String = (0..sizes.len()).map(|k| format!("[i{k}]")).collect();
                    let elem_get = map_get(
                        &resolved,
                        &format!("result.{nim_name}{idx}"),
                        enum_names,
                        struct_names,
                    )?;
                    let get = build_array_get(&sizes, &elem_get);
                    (nim_type, put, get, None)
                }
            };
            fields.push(FieldGen {
                nim_name,
                nim_type,
                put,
                get,
                id,
                key,
                key_type,
            });
        }
    }

    // `ty` (raw) feeds composite proc names (`read{ty}`, `unmarshalXCDR{ty}`) —
    // concatenation never collides with a standalone keyword token, so those
    // stay raw. `ety` (escaped) is used everywhere `ty` appears as a
    // standalone type annotation.
    let ty = &s.name.text;
    let ety = escape_nim_ident(ty);
    let _ = writeln!(out, "\ntype {ety}* = object");
    for f in &fields {
        let _ = writeln!(out, "  {}*: {}", f.nim_name, f.nim_type);
    }

    // marshalInto writes into an existing writer (nested composites call this so
    // alignment stays stream-relative). @final: fields inline; @appendable:
    // a DHEADER-framed body.
    let _ = writeln!(out, "\nproc marshalInto*(self: {ety}, w: var Writer) =");
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "  var body = initWriter(w.endian)");
        for f in &fields {
            let emh = 0x4000_0000_u32 | f.id;
            let _ = writeln!(out, "  body.putU32(uint32(0x{emh:08x}))");
            let _ = writeln!(out, "  block:");
            let _ = writeln!(out, "    var mem = initWriter(w.endian)");
            for line in f.put.replace("$w", "mem").lines() {
                let _ = writeln!(out, "    {line}");
            }
            let _ = writeln!(out, "    body.putU32(uint32(mem.bytes().len))");
            let _ = writeln!(out, "    body.putBytes(mem.bytes())");
        }
        let _ = writeln!(out, "  w.putU32(uint32(body.bytes().len))");
        let _ = writeln!(out, "  w.putBytes(body.bytes())");
    } else {
        let writer_var = if ext == ExtensibilityKind::Final {
            "w"
        } else {
            let _ = writeln!(out, "  var body = initWriter(w.endian)");
            "body"
        };
        for f in &fields {
            for line in f.put.replace("$w", writer_var).lines() {
                let _ = writeln!(out, "  {line}");
            }
        }
        if ext != ExtensibilityKind::Final {
            let _ = writeln!(out, "  w.putU32(uint32(body.bytes().len))");
            let _ = writeln!(out, "  w.putBytes(body.bytes())");
        }
    }

    let _ = writeln!(
        out,
        "\nproc marshalXCDR*(self: {ety}, endian: Endian): seq[byte] ="
    );
    let _ = writeln!(out, "  var w = initWriter(endian)");
    let _ = writeln!(out, "  self.marshalInto(w)");
    let _ = writeln!(out, "  w.bytes()");
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
        let use_md5 = zerodds_idl::keyhash::uses_md5(&key_members, structs, typedefs);
        let mut key_puts: Vec<String> = Vec::new();
        for f in &zdkeys {
            match &f.key_type {
                Some((ts, expr)) => {
                    key_puts.extend(map_key_type(
                        ts,
                        expr,
                        enum_names,
                        struct_names,
                        structs,
                        typedefs,
                    )?);
                }
                None => key_puts.push(f.put.clone()),
            }
        }
        let _ = writeln!(out, "\nproc keyHash*(self: {ety}): array[16, byte] =");
        let _ = writeln!(out, "  var kw = initWriter(eBE)");
        for put in &key_puts {
            for line in put.replace("$w", "kw").lines() {
                let _ = writeln!(out, "  {line}");
            }
        }
        let _ = writeln!(out, "  let b = kw.bytes()");
        if use_md5 {
            // KeyHolder max size > 16 → MD5(bytes)[0..16] (XTypes §7.6.8.4).
            let _ = writeln!(out, "  var ss = newString(b.len)");
            let _ = writeln!(out, "  for i in 0 ..< b.len: ss[i] = char(b[i])");
            let _ = writeln!(out, "  let d = toMD5(ss)");
            let _ = writeln!(out, "  for i in 0 ..< 16: result[i] = byte(d[i])");
        } else {
            let _ = writeln!(out, "  for i in 0 ..< min(16, b.len):");
            let _ = writeln!(out, "    result[i] = b[i]");
        }
    }

    // Decode (inverse of marshalInto). `result` is a zero-initialized {ty};
    // @final reads inline, @appendable skips the DHEADER, @mutable skips DHEADER
    // then per member EMHEADER + NEXTINT (members in declaration order).
    let _ = writeln!(out, "\nproc read{ty}(r: var Reader): {ety} =");
    if ext == ExtensibilityKind::Mutable {
        let _ = writeln!(out, "  discard r.getU32()");
        for f in &fields {
            let _ = writeln!(out, "  discard r.getU32()");
            let _ = writeln!(out, "  discard r.getU32()");
            for line in f.get.replace("$r", "r").lines() {
                let _ = writeln!(out, "  {line}");
            }
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "  discard r.getU32()");
        }
        for f in &fields {
            for line in f.get.replace("$r", "r").lines() {
                let _ = writeln!(out, "  {line}");
            }
        }
    }
    if fields.is_empty() {
        let _ = writeln!(out, "  discard");
    }
    let _ = writeln!(
        out,
        "\nproc unmarshalXCDR{ty}*(buf: seq[byte], endian: Endian): {ety} ="
    );
    let _ = writeln!(out, "  var r = initReader(buf, endian)");
    let _ = writeln!(out, "  read{ty}(r)");
    Ok(())
}

/// Emits an IDL `union` as a Nim object holding the discriminator + one field
/// per case member, plus a `marshalInto` that puts the discriminator then a
/// `case` dispatches to the selected member (XCDR2 §7.4.3.5.4).
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
        return Err(IdlNimError::Unsupported(format!(
            "@mutable union {} (EMHEADER framing not yet emitted)",
            u.name.text
        )));
    }
    let (disc_type, disc_put) = map_type(
        &switch_typespec(&u.switch_type),
        "self.disc",
        enum_names,
        struct_names,
    )?;
    let disc_get = map_get(
        &switch_typespec(&u.switch_type),
        "result.disc",
        enum_names,
        struct_names,
    )?;
    let mut cases: Vec<UnionCase> = Vec::new();
    for c in &u.cases {
        let field = escape_nim_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ty, put) = map_type(
            &resolved,
            &format!("self.{field}"),
            enum_names,
            struct_names,
        )?;
        let get = map_get(
            &resolved,
            &format!("result.{field}"),
            enum_names,
            struct_names,
        )?;
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => labels.push(array_size(e).ok_or_else(|| {
                    IdlNimError::Unsupported(format!(
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

    // See the analogous split in `emit_struct`: `ty` (raw) feeds composite
    // proc names, `ety` (escaped) is used as a standalone type annotation.
    let ty = &u.name.text;
    let ety = escape_nim_ident(ty);
    let _ = writeln!(out, "\ntype {ety}* = object");
    let _ = writeln!(out, "  disc*: {disc_type}");
    for c in &cases {
        let _ = writeln!(out, "  {}*: {}", c.field, c.ty);
    }

    let _ = writeln!(out, "\nproc marshalInto*(self: {ety}, w: var Writer) =");
    let wv = if ext == ExtensibilityKind::Final {
        "w"
    } else {
        let _ = writeln!(out, "  var body = initWriter(w.endian)");
        "body"
    };
    let _ = writeln!(out, "  {}", disc_put.replace("$w", wv));
    let _ = writeln!(out, "  case self.disc");
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "  else:");
        } else {
            let labels = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "  of {labels}:");
        }
        let _ = writeln!(out, "    {}", c.put.replace("$w", wv));
    }
    if !has_default {
        let _ = writeln!(out, "  else: discard");
    }
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "  w.putU32(uint32(body.bytes().len))");
        let _ = writeln!(out, "  w.putBytes(body.bytes())");
    }

    let _ = writeln!(
        out,
        "\nproc marshalXCDR*(self: {ety}, endian: Endian): seq[byte] ="
    );
    let _ = writeln!(out, "  var w = initWriter(endian)");
    let _ = writeln!(out, "  self.marshalInto(w)");
    let _ = writeln!(out, "  w.bytes()");

    // Decode: read the discriminator, then dispatch to read the selected member
    // (@appendable skips the leading DHEADER). `result` is zero-initialized.
    let _ = writeln!(out, "\nproc read{ty}(r: var Reader): {ety} =");
    if ext == ExtensibilityKind::Appendable {
        let _ = writeln!(out, "  discard r.getU32()");
    }
    for line in disc_get.replace("$r", "r").lines() {
        let _ = writeln!(out, "  {line}");
    }
    let _ = writeln!(out, "  case result.disc");
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "  else:");
        } else {
            let labels = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "  of {labels}:");
        }
        for line in c.get.replace("$r", "r").lines() {
            let _ = writeln!(out, "    {line}");
        }
    }
    if !has_default {
        let _ = writeln!(out, "  else: discard");
    }
    let _ = writeln!(
        out,
        "\nproc unmarshalXCDR{ty}*(buf: seq[byte], endian: Endian): {ety} ="
    );
    let _ = writeln!(out, "  var r = initReader(buf, endian)");
    let _ = writeln!(out, "  read{ty}(r)");
    Ok(())
}

/// Maps an IDL type to `(Nim type, put statement)`. The put uses `$w` as the
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

/// Builds a map put (Nim, indentation-correct: the caller adds a 2-space base):
/// collect keys, sort, then `u32 count` + key/value pairs (DHEADER-framed unless
/// the key/value pair is primitive).
fn build_map_put(
    key_type: &str,
    expr: &str,
    key_put: &str,
    val_put: &str,
    prim: bool,
    bound: Option<&ConstExpr>,
) -> String {
    // Bounded `map<K, V, N>` (DDS-XTypes §7.4.3): reject over-bound on
    // encode, checked against the entry count before either put form below
    // writes anything.
    let bound_check = bound
        .and_then(bound_value)
        .map(|n| {
            format!(
                "\n  if zdKeys.len > {n}: raise newException(ValueError, \"bounded map length exceeds its IDL bound ({n})\")"
            )
        })
        .unwrap_or_default();
    let collect = format!(
        "block:\n  var zdKeys: seq[{key_type}] = @[]\n  for zdK in {expr}.keys:\n    zdKeys.add(zdK)\n  sort(zdKeys){bound_check}"
    );
    if prim {
        format!(
            "{collect}\n  $w.putU32(uint32(zdKeys.len))\n  for zdK in zdKeys:\n    {key_put}\n    {val_put}"
        )
    } else {
        let kp = key_put.replace("$w", "zdSub");
        let vp = val_put.replace("$w", "zdSub");
        format!(
            "{collect}\n  var zdSub = initWriter($w.endian)\n  zdSub.putU32(uint32(zdKeys.len))\n  for zdK in zdKeys:\n    {kp}\n    {vp}\n  let zdBB = zdSub.bytes()\n  $w.putU32(uint32(zdBB.len))\n  $w.putBytes(zdBB)"
        )
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
        TypeSpec::String(st) if !st.wide => {
            // Bounded `string<N>` (DDS-XTypes §7.4.3): reject over-bound on
            // encode like strict vendors do. Nim has no existing runtime
            // invariant-check idiom in generated code to match, so this uses
            // stdlib `ValueError` (Nim's idiomatic invariant-violation
            // exception, matching C#'s ArgumentException / Java's
            // IllegalArgumentException in the sibling backends).
            //
            // Moderate fix (deep review of #22 decode-bounds-cross-backend):
            // pass the bound into `putString` so it checks BEFORE writing
            // anything, not via a separate pre-check statement (both are
            // "before", this just moves the check into the shared proc so
            // decode's `getString`/`getSeqU8`/`getWString` follow the same
            // shape — see the WIRE_PRELUDE procs above).
            let put = match &st.bound {
                Some(b) => match bound_value(b) {
                    Some(n) => format!("$w.putString({expr}, {n})"),
                    None => format!("$w.putString({expr})"),
                },
                None => format!("$w.putString({expr})"),
            };
            Ok(("string".to_string(), put))
        }
        // wstring: u32 octet-length (2·units, no BOM) + UTF-16 code units.
        // Bounded `wstring<N>`: bound is in UTF-16 code units — `wstringUnitLen`
        // (NOT `runeLen`, which counts Unicode CODEPOINTS and under-counts a
        // non-BMP codepoint's 2-unit surrogate pair; NOT `.len`, which counts
        // UTF-8 bytes) matches the unit count `putWString` actually writes.
        TypeSpec::String(st) => {
            let put = match &st.bound {
                Some(b) => match bound_value(b) {
                    Some(n) => format!("$w.putWString({expr}, {n})"),
                    None => format!("$w.putWString({expr})"),
                },
                None => format!("$w.putWString({expr})"),
            };
            Ok(("string".to_string(), put))
        }
        TypeSpec::Sequence(seq) => map_sequence(&seq.elem, seq.bound.as_ref(), expr, struct_names),
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                // Enum member: 32-bit signed integer on the wire.
                Ok((
                    escape_nim_ident(&name),
                    format!("$w.putU32(cast[uint32](int32(ord({expr}))))"),
                ))
            } else if struct_names.contains(&name) {
                // Nested struct member: marshal into the same writer.
                Ok((escape_nim_ident(&name), format!("{expr}.marshalInto($w)")))
            } else {
                Err(IdlNimError::Unsupported(format!("scoped type {name}")))
            }
        }
        // A map: entries sorted ascending by key, `u32 count` + key/value pairs
        // (no DHEADER for a primitive pair; DHEADER-framed otherwise).
        TypeSpec::Map(m) => {
            let (key_type, key_put) = map_type(&m.key, "zdK", enum_names, struct_names)?;
            let (val_type, val_put) =
                map_type(&m.value, &format!("{expr}[zdK]"), enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            Ok((
                format!("Table[{key_type}, {val_type}]"),
                build_map_put(&key_type, expr, &key_put, &val_put, prim, m.bound.as_ref()),
            ))
        }
        other => Err(IdlNimError::Unsupported(format!("type {other:?}"))),
    }
}

/// Maps a `@key` member's type to zero or more `KeyHash`-body put statements
/// (each using the `$w` writer placeholder, consistent with [`map_type`]'s
/// `put`).
///
/// Unlike [`map_type`] — shared with normal (non-key) member encoding, where a
/// struct-typed member always emits the struct's FULL `marshalInto` — a
/// nested-struct `@key` member must expand into only *that* struct's own
/// `@key` members (or ALL of its members if it declares none), in member-id
/// order (XTypes 1.3 §7.6.8 step 3). So this function intercepts only the
/// nested-struct case and recurses; every other type (primitive/string/enum/
/// sequence/map, and typedefs already dealiased by the caller) reuses
/// `map_type` unchanged — reusing it there is safe because those arms only
/// ever encode the value at `expr` itself, not a struct's full member set.
///
/// zerodds-lint: recursion-depth 16 (nested `@key` struct expansion; bounded
/// by the IDL's aggregate nesting depth).
fn map_key_type(
    t: &TypeSpec,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<Vec<String>> {
    if let TypeSpec::Scoped(sn) = t {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if let Some(sd) = structs.get(&name) {
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
            let mut puts = Vec::new();
            for (_, m) in &ordered {
                for decl in &m.declarators {
                    // Arrays of nested-key structs are out of the proof scope
                    // (matches the `idl-rust` reference); reject explicitly
                    // rather than silently dropping dimensions.
                    if matches!(decl, Declarator::Array(_)) {
                        return Err(IdlNimError::Unsupported(
                            "array @key field inside a nested-struct key".to_string(),
                        ));
                    }
                    let field = decl.name().text.clone();
                    let sub_expr = format!("{expr}.{field}");
                    let resolved_m = resolve_typedef(&m.type_spec, typedefs);
                    puts.extend(map_key_type(
                        &resolved_m,
                        &sub_expr,
                        enum_names,
                        struct_names,
                        structs,
                        typedefs,
                    )?);
                }
            }
            return Ok(puts);
        }
    }
    let (_, put) = map_type(t, expr, enum_names, struct_names)?;
    Ok(vec![put])
}

fn map_primitive(p: PrimitiveType, expr: &str) -> Result<(String, String)> {
    let (ty, put) = match p {
        PrimitiveType::Octet => ("uint8", format!("$w.putU8(int({expr}))")),
        PrimitiveType::Boolean => ("bool", format!("$w.putBool({expr})")),
        PrimitiveType::Char => ("char", format!("$w.putU8(int({expr}))")),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => ("float32", format!("$w.putF32({expr})")),
        PrimitiveType::Floating(FloatingType::Double) => ("float64", format!("$w.putF64({expr})")),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            ("float64", format!("$w.putLongDouble({expr})"))
        }
        PrimitiveType::WideChar => ("uint32", format!("$w.putU32({expr})")),
    };
    Ok((ty.to_string(), put))
}

fn map_integer(i: IntegerType, expr: &str) -> Result<(String, String)> {
    // Signed IDL integers reinterpret to the unsigned wire via `cast`.
    let (ty, put) = match i {
        IntegerType::UInt8 => ("uint8", format!("$w.putU8(int({expr}))")),
        IntegerType::Int8 => ("int8", format!("$w.putU8(int(cast[uint8]({expr})))")),
        IntegerType::UShort | IntegerType::UInt16 => ("uint16", format!("$w.putU16(int({expr}))")),
        IntegerType::Short | IntegerType::Int16 => {
            ("int16", format!("$w.putU16(int(cast[uint16]({expr})))"))
        }
        IntegerType::ULong | IntegerType::UInt32 => ("uint32", format!("$w.putU32({expr})")),
        IntegerType::Long | IntegerType::Int32 => {
            ("int32", format!("$w.putU32(cast[uint32]({expr}))"))
        }
        IntegerType::ULongLong | IntegerType::UInt64 => ("uint64", format!("$w.putU64({expr})")),
        IntegerType::LongLong | IntegerType::Int64 => {
            ("int64", format!("$w.putU64(cast[uint64]({expr}))"))
        }
    };
    Ok((ty.to_string(), put))
}

fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    let n = bound.and_then(bound_value);
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        // Moderate fix (deep review of #22 decode-bounds-cross-backend): pass
        // the bound into `putSeqU8` (checked inside, before writing anything)
        // instead of a separate pre-check statement — one check site,
        // matching `putString`/`putWString` above.
        return Ok((
            "seq[byte]".to_string(),
            match n {
                Some(n) => format!("$w.putSeqU8({expr}, {n})"),
                None => format!("$w.putSeqU8({expr})"),
            },
        ));
    }
    // Bounded `sequence<T, N>` of a struct element (DDS-XTypes §7.4.3):
    // reject over-bound on encode, checked against the element count
    // (`.len`) before the multi-line put below writes anything — no shared
    // Writer proc exists for this multi-statement form, so the check stays
    // inline here.
    let bound_check = n.map(|n| {
        format!(
            "(if {expr}.len > {n}: raise newException(ValueError, \"bounded sequence length exceeds its IDL bound ({n})\"))\n"
        )
    });
    let bc = bound_check.unwrap_or_default();
    // sequence<struct> → collection DHEADER (u32 body length) + u32 count + each
    // element (XTypes 1.3 §7.4.3.5.3). Multi-line put, unique vars per field.
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let suf = expr.rsplit('.').next().unwrap_or("seq");
            let put = [
                bc,
                format!("var sub_{suf} = initWriter($w.endian)"),
                format!("sub_{suf}.putU32(uint32({expr}.len))"),
                format!("for e_{suf} in {expr}: e_{suf}.marshalInto(sub_{suf})"),
                format!("$w.putU32(uint32(sub_{suf}.bytes().len))"),
                format!("$w.putBytes(sub_{suf}.bytes())"),
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
            return Ok((format!("seq[{}]", escape_nim_ident(&name)), put));
        }
    }
    Err(IdlNimError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}

// ---- decode (inverse of the put path): a `Reader` wire-core in the prelude,
// plus `map_get` — the inverse of `map_type` — emitting statements that read one
// value from `r` (placeholder `$r`) into the lvalue `target`. Roundtrip-verified
// against the goldens: `marshal(unmarshal(golden)) == golden` for LE and BE.

/// Indents every line of a (possibly multi-line) statement block by `n` spaces.
fn indent(s: &str, n: usize) -> String {
    let pad = " ".repeat(n);
    s.lines()
        .map(|l| format!("{pad}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Reads a fixed array: nested row-major `for` loops assigning into the indexed
/// lvalue (inverse of [`build_array_put`]). `elem_get` targets `{target}[i0]…`.
fn build_array_get(sizes: &[i64], elem_get: &str) -> String {
    let mut lines = Vec::new();
    for (k, n) in sizes.iter().enumerate() {
        lines.push(format!("{}for i{k} in 0 ..< {n}:", "  ".repeat(k)));
    }
    for line in elem_get.lines() {
        lines.push(format!("{}{line}", "  ".repeat(sizes.len())));
    }
    lines.join("\n")
}

/// Emits statements reading one value of IDL type `t` from `$r` into `target`.
/// zerodds-lint: recursion-depth 32
fn map_get(
    t: &TypeSpec,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    match t {
        TypeSpec::Primitive(p) => map_get_primitive(*p, target),
        // B1 follow-up (#22 decode-side parity): mirror the encode-side
        // bound check (`map_type` above) on decode too — XTypes 1.3 §7.4.3
        // requires enforcement on BOTH sides; `getString`/`getWString` only
        // ever validated the wire's remaining bytes, never the IDL bound.
        //
        // Moderate fix (deep review of #22 decode-bounds-cross-backend):
        // pass the bound into `getString`/`getWString` so they check the
        // wire-declared length BEFORE materializing the value (was
        // `getString()`/`getWString()` fully decoded first, THEN checked —
        // see the WIRE_PRELUDE procs above). The wstring check also now
        // counts true UTF-16 units (via `wstringUnitLen`, inside
        // `getWString`) instead of `runeLen` codepoints, which under-counted
        // non-BMP characters.
        TypeSpec::String(st) if !st.wide => match st.bound.as_ref().and_then(bound_value) {
            Some(n) => Ok(format!("{target} = $r.getString({n})")),
            None => Ok(format!("{target} = $r.getString()")),
        },
        TypeSpec::String(st) => match st.bound.as_ref().and_then(bound_value) {
            Some(n) => Ok(format!("{target} = $r.getWString({n})")),
            None => Ok(format!("{target} = $r.getWString()")),
        },
        TypeSpec::Sequence(seq) => {
            map_get_sequence(&seq.elem, seq.bound.as_ref(), target, struct_names)
        }
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                Ok(format!(
                    "{target} = {}(int($r.getU32()))",
                    escape_nim_ident(&name)
                ))
            } else if struct_names.contains(&name) {
                Ok(format!("{target} = read{name}($r)"))
            } else {
                Err(IdlNimError::Unsupported(format!("scoped type {name}")))
            }
        }
        TypeSpec::Map(m) => {
            let (key_type, _) = map_type(&m.key, "zdK", enum_names, struct_names)?;
            let (val_type, _) = map_type(&m.value, "zdV", enum_names, struct_names)?;
            let key_get = map_get(&m.key, "zdK", enum_names, struct_names)?;
            let val_get = map_get(&m.value, "zdV", enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let dh = if prim { "" } else { "discard $r.getU32()\n" };
            // B1 follow-up (#22 decode-side parity): mirror the encode-side
            // bound check — XTypes 1.3 §7.4.3.
            let bound_check = m
                .bound
                .as_ref()
                .and_then(bound_value)
                .map(|n| {
                    format!(
                        "\n  if zdN > {n}: raise newException(ValueError, \"decoded map length exceeds its IDL bound ({n})\")"
                    )
                })
                .unwrap_or_default();
            Ok(format!(
                "{dh}block:\n  let zdN = int($r.getU32()){bound_check}\n  {target} = initTable[{key_type}, {val_type}]()\n  for _ in 0 ..< zdN:\n    var zdK: {key_type}\n{key}\n    var zdV: {val_type}\n{val}\n    {target}[zdK] = zdV",
                key = indent(&key_get, 4),
                val = indent(&val_get, 4)
            ))
        }
        other => Err(IdlNimError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType, target: &str) -> Result<String> {
    let s = match p {
        PrimitiveType::Octet => format!("{target} = uint8($r.getU8())"),
        PrimitiveType::Char => format!("{target} = char($r.getU8())"),
        PrimitiveType::Boolean => format!("{target} = $r.getBool()"),
        PrimitiveType::Integer(i) => return map_get_integer(i, target),
        PrimitiveType::Floating(FloatingType::Float) => format!("{target} = $r.getF32()"),
        PrimitiveType::Floating(FloatingType::Double) => format!("{target} = $r.getF64()"),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            format!("{target} = $r.getLongDouble()")
        }
        PrimitiveType::WideChar => format!("{target} = $r.getU32()"),
    };
    Ok(s)
}

fn map_get_integer(i: IntegerType, target: &str) -> Result<String> {
    let s = match i {
        IntegerType::UInt8 => format!("{target} = uint8($r.getU8())"),
        IntegerType::Int8 => format!("{target} = cast[int8](uint8($r.getU8()))"),
        IntegerType::UShort | IntegerType::UInt16 => format!("{target} = uint16($r.getU16())"),
        IntegerType::Short | IntegerType::Int16 => {
            format!("{target} = cast[int16](uint16($r.getU16()))")
        }
        IntegerType::ULong | IntegerType::UInt32 => format!("{target} = $r.getU32()"),
        IntegerType::Long | IntegerType::Int32 => format!("{target} = cast[int32]($r.getU32())"),
        IntegerType::ULongLong | IntegerType::UInt64 => format!("{target} = $r.getU64()"),
        IntegerType::LongLong | IntegerType::Int64 => {
            format!("{target} = cast[int64]($r.getU64())")
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
    // B1 follow-up (#22 decode-side parity): mirror the encode-side bound
    // check (`map_sequence` above) — XTypes 1.3 §7.4.3.
    let n = bound.and_then(bound_value);
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        // Moderate fix (deep review of #22 decode-bounds-cross-backend): pass
        // the bound into `getSeqU8` so it checks the wire-declared length
        // BEFORE allocating the byte seq, not after (was `getSeqU8()` then
        // `.len > n` post-hoc, mirroring the pattern already fixed for
        // `getString`/`getWString`/`putString`/`putSeqU8`/`putWString`
        // above).
        return Ok(match n {
            Some(n) => format!("{target} = $r.getSeqU8({n})"),
            None => format!("{target} = $r.getSeqU8()"),
        });
    }
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let bound_check = n
                .map(|n| {
                    format!(
                        "\n  if zdN > {n}: raise newException(ValueError, \"decoded sequence length exceeds its IDL bound ({n})\")"
                    )
                })
                .unwrap_or_default();
            let ename = escape_nim_ident(&name);
            return Ok(format!(
                "discard $r.getU32()\nblock:\n  let zdN = int($r.getU32()){bound_check}\n  {target} = newSeq[{ename}](zdN)\n  for zdI in 0 ..< zdN:\n    {target}[zdI] = read{name}($r)"
            ));
        }
    }
    Err(IdlNimError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}
