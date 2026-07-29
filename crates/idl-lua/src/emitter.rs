// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → Lua emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! Lua source file: a `Writer` built on `string.pack` (byte-identical to
//! `endpoints/lua`, no FFI) plus, per IDL `struct`, a `marshal_<name>(v, endian)`
//! function. `@final` and `@appendable` are supported; other extensibilities and
//! constructs raise [`IdlLuaError::Unsupported`].

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

use crate::error::{IdlLuaError, Result};
use crate::keywords::escape_lua_ident;

/// Options for the Lua backend.
#[derive(Debug, Clone, Default)]
pub struct LuaGenOptions {}

/// The shared XCDR2 `Writer` on `string.pack`, byte-identical to `endpoints/lua`.
const WIRE_PRELUDE: &str = r#"local LE, BE = "<", ">"

local Writer = {}
Writer.__index = Writer

function Writer.new(endian)
  return setmetatable({ parts = {}, len = 0, endian = endian }, Writer)
end

function Writer:raw(s)
  self.parts[#self.parts + 1] = s
  self.len = self.len + #s
end

function Writer:align(a)
  local cap = a < 4 and a or 4
  local pad = (cap - (self.len % cap)) % cap
  if pad > 0 then self:raw(string.rep("\0", pad)) end
end

function Writer:putU8(v) self:raw(string.pack("B", v & 0xff)) end
function Writer:putBool(v) self:putU8(v and 1 or 0) end
function Writer:putU16(v) self:align(2); self:raw(string.pack(self.endian .. "I2", v)) end
function Writer:putU32(v) self:align(4); self:raw(string.pack(self.endian .. "I4", v)) end
function Writer:putU64(v) self:align(4); self:raw(string.pack(self.endian .. "I8", v)) end
function Writer:putF32(v) self:align(4); self:raw(string.pack(self.endian .. "f", v)) end
function Writer:putF64(v) self:align(4); self:raw(string.pack(self.endian .. "d", v)) end
function Writer:putBytes(b) self:raw(b) end

-- `maxBytes`/`maxUnits`/`maxLen` (all optional): the IDL-declared bound N of
-- a `string<N>`/`wstring<N>`/`sequence<octet,N>` member (XTypes 1.3 §7.4.3).
-- `nil` for an unbounded member -> no check, same wire form as before.
function Writer:putString(s, maxBytes)
  if maxBytes and #s > maxBytes then
    error(string.format("bounded string length exceeds its IDL bound (%d)", maxBytes))
  end
  self:putU32(#s + 1)
  self:raw(s)
  self:putU8(0)
end

function Writer:putSeqU8(b, maxLen)
  if maxLen and #b > maxLen then
    error(string.format("bounded sequence length exceeds its IDL bound (%d)", maxLen))
  end
  self:putU32(#b)
  self:raw(b)
end

function Writer:putWString(s, maxUnits)
  local units = {}
  for _, cp in utf8.codes(s) do
    if cp <= 0xFFFF then
      units[#units + 1] = cp
    else
      local rr = cp - 0x10000
      units[#units + 1] = 0xD800 + (rr >> 10)
      units[#units + 1] = 0xDC00 + (rr & 0x3FF)
    end
  end
  if maxUnits and #units > maxUnits then
    error(string.format("bounded wstring length exceeds its IDL bound (%d)", maxUnits))
  end
  self:putU32(#units * 2)
  for _, u in ipairs(units) do self:putU16(u) end
end

function Writer:putLongDouble(v)
  local bits = string.unpack("<I8", string.pack("<d", v))
  local sign = bits >> 63
  local exp = (bits >> 52) & 0x7FF
  local mant = bits & 0xFFFFFFFFFFFFF
  local hi = sign << 63
  local lo = 0
  if not (exp == 0 and mant == 0) then
    hi = (sign << 63) | ((exp - 1023 + 16383) << 48) | (mant >> 4)
    lo = (mant & 0xF) << 60
  end
  local b = {}
  for i = 0, 7 do b[#b + 1] = (lo >> (8 * i)) & 0xff end
  for i = 0, 7 do b[#b + 1] = (hi >> (8 * i)) & 0xff end
  if self.endian == ">" then
    local rev = {}
    for i = 16, 1, -1 do rev[#rev + 1] = b[i] end
    b = rev
  end
  self:align(4)
  local out = {}
  for i = 1, 16 do out[i] = string.char(b[i]) end
  self:raw(table.concat(out))
end

function Writer:bytes() return table.concat(self.parts) end

local Reader = {}
Reader.__index = Reader

function Reader.new(buf, endian)
  return setmetatable({ buf = buf, pos = 1, endian = endian }, Reader)
end

function Reader:ralign(a)
  local cap = a < 4 and a or 4
  local off = (self.pos - 1) % cap
  if off > 0 then self.pos = self.pos + (cap - off) end
end

function Reader:unpack(fmt, sz)
  local v, np = string.unpack(fmt, self.buf, self.pos)
  self.pos = np
  return v
end

function Reader:getU8() return self:unpack("B") end
function Reader:getBool() return self:getU8() ~= 0 end
function Reader:getU16() self:ralign(2); return self:unpack(self.endian .. "I2") end
function Reader:getU32() self:ralign(4); return self:unpack(self.endian .. "I4") end
function Reader:getU64() self:ralign(4); return self:unpack(self.endian .. "I8") end
function Reader:getF32() self:ralign(4); return self:unpack(self.endian .. "f") end
function Reader:getF64() self:ralign(4); return self:unpack(self.endian .. "d") end

function Reader:getBytesN(n)
  local s = string.sub(self.buf, self.pos, self.pos + n - 1)
  self.pos = self.pos + n
  return s
end

-- Regression #22 follow-up (decode-side IDL bound parity, XTypes 1.3 §7.4.3):
-- a bounded member must reject a well-formed-but-oversized DECODED value too,
-- not just check the wire's remaining bytes. `maxBytes`/`maxLen`/`maxUnits`
-- (optional): the IDL-declared bound N; `nil` for an unbounded member -> no
-- check, unchanged behavior.
function Reader:getString(maxBytes)
  local n = self:getU32()
  if maxBytes and (n - 1) > maxBytes then
    error(string.format("decoded string length exceeds its IDL bound (%d)", maxBytes))
  end
  local s = string.sub(self.buf, self.pos, self.pos + n - 2)
  self.pos = self.pos + n
  return s
end

function Reader:getSeqU8(maxLen)
  local n = self:getU32()
  if maxLen and n > maxLen then
    error(string.format("decoded sequence length exceeds its IDL bound (%d)", maxLen))
  end
  return self:getBytesN(n)
end

function Reader:getWString(maxUnits)
  local n = self:getU32() // 2
  if maxUnits and n > maxUnits then
    error(string.format("decoded wstring length exceeds its IDL bound (%d)", maxUnits))
  end
  local units = {}
  for _ = 1, n do units[#units + 1] = self:getU16() end
  local cps = {}
  local i = 1
  while i <= n do
    local u = units[i]
    if u >= 0xD800 and u <= 0xDBFF and i + 1 <= n then
      local lo = units[i + 1]
      cps[#cps + 1] = 0x10000 + ((u - 0xD800) << 10) + (lo - 0xDC00)
      i = i + 2
    else
      cps[#cps + 1] = u
      i = i + 1
    end
  end
  return utf8.char(table.unpack(cps))
end

function Reader:getLongDouble()
  self:ralign(4)
  local raw = self:getBytesN(16)
  local b = {}
  for i = 1, 16 do b[i] = string.byte(raw, i) end
  if self.endian == ">" then
    local rev = {}
    for i = 16, 1, -1 do rev[#rev + 1] = b[i] end
    b = rev
  end
  local lo = 0
  local hi = 0
  for i = 0, 7 do lo = lo | (b[i + 1] << (8 * i)) end
  for i = 0, 7 do hi = hi | (b[i + 9] << (8 * i)) end
  local sign = hi >> 63
  local exp = (hi >> 48) & 0x7FFF
  local mant = ((hi & 0xFFFFFFFFFFFF) << 4) | (lo >> 60)
  local bits
  if exp == 0 and mant == 0 then
    bits = sign << 63
  else
    bits = (sign << 63) | ((exp - 16383 + 1023) << 52) | mant
  end
  return string.unpack("<d", string.pack("<I8", bits))
end
"#;

/// Generates a self-contained Lua module from the IDL AST.
///
/// # Errors
/// Returns [`IdlLuaError::Unsupported`] for constructs the Lua backend does not
/// yet emit (unions, nested-struct members, maps, `long double`, `@mutable`, …).
pub fn generate_lua_module(spec: &Specification, _opts: &LuaGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "-- Code generated by zerodds-idlc (Lua backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "-- SPDX-License-Identifier: Apache-2.0\n");
    out.push_str(WIRE_PRELUDE);

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
    // Self-contained MD5 (RFC 1321) for the KeyHash MD5 branch; a global fn
    // appended on demand (Lua resolves the call at run time, so order is fine).
    if out.contains("zd_md5(") {
        out.push_str(LUA_MD5);
    }
    Ok(out)
}

/// RFC 1321 MD5 over a byte string, returning the 16-byte digest (Lua 5.4 has no
/// MD5). Byte-identical to `zerodds_foundation::md5`; 32-bit ops masked to fit
/// Lua's 64-bit integers.
const LUA_MD5: &str = r#"
function zd_md5(msg)
  local S = {7,12,17,22,7,12,17,22,7,12,17,22,7,12,17,22,
    5,9,14,20,5,9,14,20,5,9,14,20,5,9,14,20,
    4,11,16,23,4,11,16,23,4,11,16,23,4,11,16,23,
    6,10,15,21,6,10,15,21,6,10,15,21,6,10,15,21}
  local K = {
    0xd76aa478,0xe8c7b756,0x242070db,0xc1bdceee,0xf57c0faf,0x4787c62a,0xa8304613,0xfd469501,
    0x698098d8,0x8b44f7af,0xffff5bb1,0x895cd7be,0x6b901122,0xfd987193,0xa679438e,0x49b40821,
    0xf61e2562,0xc040b340,0x265e5a51,0xe9b6c7aa,0xd62f105d,0x02441453,0xd8a1e681,0xe7d3fbc8,
    0x21e1cde6,0xc33707d6,0xf4d50d87,0x455a14ed,0xa9e3e905,0xfcefa3f8,0x676f02d9,0x8d2a4c8a,
    0xfffa3942,0x8771f681,0x6d9d6122,0xfde5380c,0xa4beea44,0x4bdecfa9,0xf6bb4b60,0xbebfbc70,
    0x289b7ec6,0xeaa127fa,0xd4ef3085,0x04881d05,0xd9d4d039,0xe6db99e5,0x1fa27cf8,0xc4ac5665,
    0xf4292244,0x432aff97,0xab9423a7,0xfc93a039,0x655b59c3,0x8f0ccc92,0xffeff47d,0x85845dd1,
    0x6fa87e4f,0xfe2ce6e0,0xa3014314,0x4e0811a1,0xf7537e82,0xbd3af235,0x2ad7d2bb,0xeb86d391}
  local mask = 0xFFFFFFFF
  local a0,b0,c0,d0 = 0x67452301,0xefcdab89,0x98badcfe,0x10325476
  local len = #msg
  local data = msg .. "\128"
  while (#data % 64) ~= 56 do data = data .. "\0" end
  local bitlen = len * 8
  for i = 0, 7 do data = data .. string.char((bitlen >> (8*i)) & 0xff) end
  for off = 0, #data - 1, 64 do
    local M = {}
    for j = 0, 15 do
      local p = off + j*4
      M[j] = string.byte(data, p+1) | (string.byte(data, p+2) << 8) | (string.byte(data, p+3) << 16) | (string.byte(data, p+4) << 24)
    end
    local A,B,C,D = a0,b0,c0,d0
    for i = 0, 63 do
      local F, g
      if i < 16 then F = (B & C) | ((~B & mask) & D); g = i
      elseif i < 32 then F = (D & B) | ((~D & mask) & C); g = (5*i + 1) % 16
      elseif i < 48 then F = B ~ C ~ D; g = (3*i + 5) % 16
      else F = C ~ (B | (~D & mask)); g = (7*i) % 16 end
      F = (F + A + K[i+1] + M[g]) & mask
      A = D; D = C; C = B
      local sh = S[i+1]
      B = (B + (((F << sh) | (F >> (32 - sh))) & mask)) & mask
    end
    a0 = (a0 + A) & mask
    b0 = (b0 + B) & mask
    c0 = (c0 + C) & mask
    d0 = (d0 + D) & mask
  end
  local out = {}
  local function app(v) for i = 0, 3 do out[#out+1] = string.char((v >> (8*i)) & 0xff) end end
  app(a0); app(b0); app(c0); app(d0)
  return table.concat(out)
end
"#;

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

/// Emits an IDL `enum` as a Lua constants table (its member is an i32 field).
fn emit_enum(out: &mut String, e: &EnumDef) {
    let values = enumerator_values(e);
    let ty = escape_lua_ident(&e.name.text);
    let pairs: Vec<String> = e
        .enumerators
        .iter()
        .zip(&values)
        .map(|(en, v)| format!("{} = {v}", escape_lua_ident(&en.name.text)))
        .collect();
    let _ = writeln!(
        out,
        "
local {ty} = {{ {} }}",
        pairs.join(", ")
    );
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
/// fixed array `v.<field>[zdi0][zdi1]…` (Lua tables are 1-based: `1, N`).
fn build_array_put(field: &str, sizes: &[i64], elem_put: &str) -> String {
    let idx: String = (0..sizes.len()).map(|k| format!("[zdi{k}]")).collect();
    let mut body = elem_put.replace("$elem", &format!("v.{field}{idx}"));
    for k in (0..sizes.len()).rev() {
        body = format!("for zdi{k} = 1, {} do\n{body}\nend", sizes[k]);
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
        put: String,
        get: String,
        id: u32,
        key: bool,
        // `Some((type_spec, expr))` for a Simple (non-array) declarator, so a
        // `@key` field can be re-mapped through `map_key_type` instead of
        // reusing `put` (which, for a struct-typed member, is the full
        // `marshalInto_<T>` call shared with normal, non-key encoding). `None`
        // for an array declarator — array key fields are emitted unchanged.
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
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let name = escape_lua_ident(&d.name().text);
            let (put, get, key_type) = match d {
                Declarator::Simple(_) => {
                    let expr = format!("v.{name}");
                    let p = map_type(&resolved, &expr, enum_names, struct_names)?;
                    let g = map_get(&resolved, &expr, enum_names, struct_names)?;
                    (p, g, Some((resolved.clone(), expr)))
                }
                // Fixed array: elements inline, row-major, no length prefix.
                Declarator::Array(ad) => {
                    let sizes = ad
                        .sizes
                        .iter()
                        .map(array_size)
                        .collect::<Option<Vec<i64>>>()
                        .ok_or_else(|| {
                            IdlLuaError::Unsupported(format!("non-literal array size on `{name}`"))
                        })?;
                    let elem_put = map_type(&resolved, "$elem", enum_names, struct_names)?;
                    let put = build_array_put(&name, &sizes, &elem_put);
                    let idx: String = (0..sizes.len()).map(|k| format!("[zdi{k}]")).collect();
                    let elem_get = map_get(
                        &resolved,
                        &format!("v.{name}{idx}"),
                        enum_names,
                        struct_names,
                    )?;
                    let get = build_array_get(&format!("v.{name}"), &sizes, &elem_get);
                    (put, get, None)
                }
            };
            fields.push(FieldGen {
                put,
                get,
                id,
                key,
                key_type,
            });
        }
    }

    let ty = escape_lua_ident(&s.name.text);

    // marshalInto_<T> writes into an existing writer (nested composites call this
    // so alignment stays stream-relative). @final: inline; @appendable: DHEADER.
    let _ = writeln!(out, "\nfunction marshalInto_{ty}(w, v)");
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "  local body = Writer.new(w.endian)");
        for f in &fields {
            let emh = 0x4000_0000_u32 | f.id;
            let _ = writeln!(out, "  body:putU32(0x{emh:08x})");
            let _ = writeln!(out, "  local zdMem = Writer.new(w.endian)");
            let _ = writeln!(out, "  {}", f.put.replace("$w", "zdMem"));
            let _ = writeln!(out, "  local zdMB = zdMem:bytes()");
            let _ = writeln!(out, "  body:putU32(#zdMB)");
            let _ = writeln!(out, "  body:putBytes(zdMB)");
        }
        let _ = writeln!(out, "  local zdBB = body:bytes()");
        let _ = writeln!(out, "  w:putU32(#zdBB)");
        let _ = writeln!(out, "  w:putBytes(zdBB)");
    } else {
        let wv = if ext == ExtensibilityKind::Final {
            "w"
        } else {
            let _ = writeln!(out, "  local body = Writer.new(w.endian)");
            "body"
        };
        for f in &fields {
            let _ = writeln!(out, "  {}", f.put.replace("$w", wv));
        }
        if ext != ExtensibilityKind::Final {
            let _ = writeln!(out, "  local bb = body:bytes()");
            let _ = writeln!(out, "  w:putU32(#bb)");
            let _ = writeln!(out, "  w:putBytes(bb)");
        }
    }
    let _ = writeln!(out, "end");

    let _ = writeln!(out, "\nfunction marshal_{ty}(v, endian)");
    let _ = writeln!(out, "  local w = Writer.new(endian)");
    let _ = writeln!(out, "  marshalInto_{ty}(w, v)");
    let _ = writeln!(out, "  return w:bytes()");
    let _ = writeln!(out, "end");
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
        let _ = writeln!(out, "\nfunction keyHash_{ty}(v)");
        let _ = writeln!(out, "  local kw = Writer.new(BE)");
        for put in &key_puts {
            let _ = writeln!(out, "  {}", put.replace("$w", "kw"));
        }
        let _ = writeln!(out, "  local b = kw:bytes()");
        if use_md5 {
            // KeyHolder max size > 16 → MD5(bytes)[0..16] (XTypes §7.6.8.4).
            let _ = writeln!(out, "  return zd_md5(b)");
        } else {
            let _ = writeln!(out, "  local chars = {{}}");
            let _ = writeln!(
                out,
                "  for i = 1, 16 do chars[i] = (i <= #b) and string.sub(b, i, i) or \"\\0\" end"
            );
            let _ = writeln!(out, "  return table.concat(chars)");
        }
        let _ = writeln!(out, "end");
    }

    // Decode (inverse of marshalInto_). Fills a fresh table `v`. @final reads
    // inline, @appendable skips the DHEADER, @mutable skips DHEADER then per
    // member EMHEADER + NEXTINT (members in declaration order).
    let _ = writeln!(out, "\nfunction read_{ty}(r)");
    let _ = writeln!(out, "  local v = {{}}");
    if ext == ExtensibilityKind::Mutable {
        let _ = writeln!(out, "  r:getU32()");
        for f in &fields {
            let _ = writeln!(out, "  r:getU32()");
            let _ = writeln!(out, "  r:getU32()");
            let _ = writeln!(out, "  {}", f.get.replace("$r", "r"));
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "  r:getU32()");
        }
        for f in &fields {
            let _ = writeln!(out, "  {}", f.get.replace("$r", "r"));
        }
    }
    let _ = writeln!(out, "  return v");
    let _ = writeln!(out, "end");
    let _ = writeln!(out, "\nfunction unmarshal_{ty}(buf, endian)");
    let _ = writeln!(out, "  return read_{ty}(Reader.new(buf, endian))");
    let _ = writeln!(out, "end");
    Ok(())
}

/// Emits an IDL `union` as a discriminated Lua table marshaller: put the
/// discriminator then an `if`-chain dispatches on it (XCDR2 §7.4.3.5.4).
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
        return Err(IdlLuaError::Unsupported(format!(
            "@mutable union {} (EMHEADER framing not yet emitted)",
            u.name.text
        )));
    }
    let disc_put = map_type(
        &switch_typespec(&u.switch_type),
        "v.disc",
        enum_names,
        struct_names,
    )?;
    let disc_get = map_get(
        &switch_typespec(&u.switch_type),
        "v.disc",
        enum_names,
        struct_names,
    )?;
    struct LuaCase {
        labels: Vec<i64>,
        is_default: bool,
        put: String,
        get: String,
    }
    let mut cases: Vec<LuaCase> = Vec::new();
    for c in &u.cases {
        let field = escape_lua_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let put = map_type(&resolved, &format!("v.{field}"), enum_names, struct_names)?;
        let get = map_get(&resolved, &format!("v.{field}"), enum_names, struct_names)?;
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => labels.push(array_size(e).ok_or_else(|| {
                    IdlLuaError::Unsupported(format!(
                        "non-integer union label in `{}`",
                        u.name.text
                    ))
                })?),
            }
        }
        cases.push(LuaCase {
            labels,
            is_default,
            put,
            get,
        });
    }

    let ty = escape_lua_ident(&u.name.text);
    let _ = writeln!(out, "\nfunction marshalInto_{ty}(w, v)");
    let wv = if ext == ExtensibilityKind::Final {
        "w"
    } else {
        let _ = writeln!(out, "  local body = Writer.new(w.endian)");
        "body"
    };
    let _ = writeln!(out, "  {}", disc_put.replace("$w", wv));
    for (i, c) in cases.iter().enumerate() {
        if c.is_default {
            let _ = writeln!(out, "  else");
        } else {
            let kw = if i == 0 { "if" } else { "elseif" };
            let cond = c
                .labels
                .iter()
                .map(|l| format!("v.disc == {l}"))
                .collect::<Vec<_>>()
                .join(" or ");
            let _ = writeln!(out, "  {kw} {cond} then");
        }
        let _ = writeln!(out, "    {}", c.put.replace("$w", wv));
    }
    let _ = writeln!(out, "  end");
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "  local bb = body:bytes()");
        let _ = writeln!(out, "  w:putU32(#bb)");
        let _ = writeln!(out, "  w:putBytes(bb)");
    }
    let _ = writeln!(out, "end");
    let _ = writeln!(out, "\nfunction marshal_{ty}(v, endian)");
    let _ = writeln!(out, "  local w = Writer.new(endian)");
    let _ = writeln!(out, "  marshalInto_{ty}(w, v)");
    let _ = writeln!(out, "  return w:bytes()");
    let _ = writeln!(out, "end");

    // Decode: read the discriminator, then read only the selected member
    // (@appendable skips the leading DHEADER). Unread members stay nil.
    let _ = writeln!(out, "\nfunction read_{ty}(r)");
    let _ = writeln!(out, "  local v = {{}}");
    if ext == ExtensibilityKind::Appendable {
        let _ = writeln!(out, "  r:getU32()");
    }
    let _ = writeln!(out, "  {}", disc_get.replace("$r", "r"));
    for (i, c) in cases.iter().enumerate() {
        if c.is_default {
            let _ = writeln!(out, "  else");
        } else {
            let kw = if i == 0 { "if" } else { "elseif" };
            let cond = c
                .labels
                .iter()
                .map(|l| format!("v.disc == {l}"))
                .collect::<Vec<_>>()
                .join(" or ");
            let _ = writeln!(out, "  {kw} {cond} then");
        }
        let _ = writeln!(out, "    {}", c.get.replace("$r", "r"));
    }
    if !cases.is_empty() {
        let _ = writeln!(out, "  end");
    }
    let _ = writeln!(out, "  return v");
    let _ = writeln!(out, "end");
    let _ = writeln!(out, "\nfunction unmarshal_{ty}(buf, endian)");
    let _ = writeln!(out, "  return read_{ty}(Reader.new(buf, endian))");
    let _ = writeln!(out, "end");
    Ok(())
}

/// Maps an IDL type to a put statement using `$w` as the writer placeholder.
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

/// Builds a map put: `u32 count` + key/value pairs sorted ascending by key
/// (DHEADER-framed unless the key/value pair is primitive). `bound`: the IDL
/// bound N of `map<K,V,N>` — B1 follow-up (#22 decode-side-parity work,
/// encode half), XTypes 1.3 §7.4.3, inlined ahead of the `do` block since a
/// map has no shared Writer method to carry the check.
fn build_map_put(
    expr: &str,
    key_put: &str,
    val_put: &str,
    prim: bool,
    bound: Option<&ConstExpr>,
) -> String {
    let collect = format!(
        "local zdKeys = {{}}\n  for zdK in pairs({expr}) do zdKeys[#zdKeys + 1] = zdK end\n  table.sort(zdKeys)"
    );
    let check = match bound.and_then(array_size) {
        Some(n) => format!(
            "\n  if #zdKeys > {n} then error(string.format(\"bounded map length exceeds its IDL bound (%d)\", {n})) end"
        ),
        None => String::new(),
    };
    if prim {
        format!(
            "do\n  {collect}{check}\n  $w:putU32(#zdKeys)\n  for _, zdK in ipairs(zdKeys) do\n    {key_put}\n    {val_put}\n  end\nend"
        )
    } else {
        let kp = key_put.replace("$w", "zdSub");
        let vp = val_put.replace("$w", "zdSub");
        format!(
            "do\n  {collect}{check}\n  local zdSub = Writer.new($w.endian)\n  zdSub:putU32(#zdKeys)\n  for _, zdK in ipairs(zdKeys) do\n    {kp}\n    {vp}\n  end\n  local zdBB = zdSub:bytes()\n  $w:putU32(#zdBB)\n  $w:putBytes(zdBB)\nend"
        )
    }
}

/// zerodds-lint: recursion-depth 32
fn map_type(
    t: &TypeSpec,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    match t {
        TypeSpec::Primitive(p) => map_primitive(*p, expr),
        // B1 follow-up (#22 decode-side-parity work, encode half): a bounded
        // `string<N>`/`wstring<N>` passes its IDL bound N as an extra `Writer`
        // arg — `nil` (arg omitted) for an unbounded member keeps the wire
        // form byte-identical. XTypes 1.3 §7.4.3.
        TypeSpec::String(st) if !st.wide => match st.bound.as_ref().and_then(array_size) {
            Some(n) => Ok(format!("$w:putString({expr}, {n})")),
            None => Ok(format!("$w:putString({expr})")),
        },
        TypeSpec::String(st) => match st.bound.as_ref().and_then(array_size) {
            Some(n) => Ok(format!("$w:putWString({expr}, {n})")),
            None => Ok(format!("$w:putWString({expr})")),
        },
        TypeSpec::Sequence(seq) => map_sequence(&seq.elem, seq.bound.as_ref(), expr, struct_names),
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                Ok(format!("$w:putU32({expr} & 0xffffffff)"))
            } else if struct_names.contains(&name) {
                let name = escape_lua_ident(&name);
                Ok(format!("marshalInto_{name}($w, {expr})"))
            } else {
                Err(IdlLuaError::Unsupported(format!("scoped type {name}")))
            }
        }
        // A map: entries sorted ascending by key, `u32 count` + key/value pairs
        // (no DHEADER for a primitive pair; DHEADER-framed otherwise).
        TypeSpec::Map(m) => {
            let key_put = map_type(&m.key, "zdK", enum_names, struct_names)?;
            let val_put = map_type(&m.value, &format!("{expr}[zdK]"), enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            Ok(build_map_put(expr, &key_put, &val_put, prim, m.bound.as_ref()))
        }
        other => Err(IdlLuaError::Unsupported(format!("type {other:?}"))),
    }
}

/// Maps a `@key` member's type to zero or more `KeyHash`-body put statements
/// (each using the `$w` writer placeholder, consistent with [`map_type`]).
///
/// Unlike [`map_type`] — shared with normal (non-key) member encoding, where a
/// struct-typed member always emits the struct's FULL `marshalInto_<T>` — a
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
                        return Err(IdlLuaError::Unsupported(
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
    Ok(vec![map_type(t, expr, enum_names, struct_names)?])
}

fn map_primitive(p: PrimitiveType, expr: &str) -> Result<String> {
    let put = match p {
        PrimitiveType::Octet | PrimitiveType::Char => format!("$w:putU8({expr})"),
        PrimitiveType::Boolean => format!("$w:putBool({expr})"),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => format!("$w:putF32({expr})"),
        PrimitiveType::Floating(FloatingType::Double) => format!("$w:putF64({expr})"),
        PrimitiveType::Floating(FloatingType::LongDouble) => format!("$w:putLongDouble({expr})"),
        PrimitiveType::WideChar => format!("$w:putU32({expr})"),
    };
    Ok(put)
}

fn map_integer(i: IntegerType, expr: &str) -> Result<String> {
    // Signed 16/32-bit values are masked to their unsigned wire form; putU8
    // already masks, and 64-bit packs the low 8 bytes directly.
    let put = match i {
        IntegerType::Int8 | IntegerType::UInt8 => format!("$w:putU8({expr})"),
        IntegerType::UShort | IntegerType::UInt16 => format!("$w:putU16({expr})"),
        IntegerType::Short | IntegerType::Int16 => format!("$w:putU16({expr} & 0xffff)"),
        IntegerType::ULong | IntegerType::UInt32 => format!("$w:putU32({expr})"),
        IntegerType::Long | IntegerType::Int32 => format!("$w:putU32({expr} & 0xffffffff)"),
        IntegerType::LongLong
        | IntegerType::ULongLong
        | IntegerType::Int64
        | IntegerType::UInt64 => format!("$w:putU64({expr})"),
    };
    Ok(put)
}

fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    struct_names: &HashSet<String>,
) -> Result<String> {
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        // B1 follow-up (#22 decode-side-parity work, encode half): pass the
        // IDL bound N as an extra `putSeqU8` arg — `nil` for unbounded.
        return match bound.and_then(array_size) {
            Some(n) => Ok(format!("$w:putSeqU8({expr}, {n})")),
            None => Ok(format!("$w:putSeqU8({expr})")),
        };
    }
    // sequence<struct> → collection DHEADER + count + each element. This
    // custom inline block does not go through a shared Writer method, so the
    // bound check (XTypes 1.3 §7.4.3) is inlined ahead of it directly.
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let check = match bound.and_then(array_size) {
                Some(n) => format!(
                    "if #{expr} > {n} then error(string.format(\"bounded sequence length exceeds its IDL bound (%d)\", {n})) end; "
                ),
                None => String::new(),
            };
            let name = escape_lua_ident(&name);
            let put = format!(
                "{check}do local sub = Writer.new($w.endian); sub:putU32(#{expr});                  for _, e in ipairs({expr}) do marshalInto_{name}(sub, e) end;                  local bb = sub:bytes(); $w:putU32(#bb); $w:putBytes(bb) end"
            );
            return Ok(put);
        }
    }
    Err(IdlLuaError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}

// ---- decode (inverse of the put path): a `Reader` wire-core in the prelude,
// plus `map_get` — the inverse of `map_type` — emitting a statement that reads
// one value from `$r` into the lvalue `target`. Lua tables are dynamic, so the
// holder `v = {}` is filled field-by-field (like Go). Roundtrip-verified.

/// Reads a fixed array: nested row-major loops filling each element (inverse of
/// [`build_array_put`]). `elem_get` targets the fully indexed lvalue.
fn build_array_get(target: &str, sizes: &[i64], elem_get: &str) -> String {
    /// zerodds-lint: recursion-depth 32
    fn rec(target: &str, sizes: &[i64], depth: usize, elem_get: &str) -> String {
        let idx: String = (0..depth).map(|k| format!("[zdi{k}]")).collect();
        let lval = format!("{target}{idx}");
        let s = sizes[depth];
        if depth + 1 == sizes.len() {
            format!("{lval} = {{}}\nfor zdi{depth} = 1, {s} do\n{elem_get}\nend")
        } else {
            let inner = rec(target, sizes, depth + 1, elem_get);
            format!("{lval} = {{}}\nfor zdi{depth} = 1, {s} do\n{inner}\nend")
        }
    }
    rec(target, sizes, 0, elem_get)
}

/// Emits a statement reading one value of IDL type `t` from `$r` into `target`.
/// zerodds-lint: recursion-depth 32
fn map_get(
    t: &TypeSpec,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    match t {
        TypeSpec::Primitive(p) => map_get_primitive(*p, target),
        // B1 follow-up (#22 decode-side parity): mirror the encode half —
        // pass the IDL bound N to the shared Reader method, which rejects a
        // decoded value exceeding it. XTypes 1.3 §7.4.3.
        TypeSpec::String(st) if !st.wide => match st.bound.as_ref().and_then(array_size) {
            Some(n) => Ok(format!("{target} = $r:getString({n})")),
            None => Ok(format!("{target} = $r:getString()")),
        },
        TypeSpec::String(st) => match st.bound.as_ref().and_then(array_size) {
            Some(n) => Ok(format!("{target} = $r:getWString({n})")),
            None => Ok(format!("{target} = $r:getWString()")),
        },
        TypeSpec::Sequence(seq) => {
            map_get_sequence(&seq.elem, seq.bound.as_ref(), target, struct_names)
        }
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                Ok(format!("{target} = $r:getU32()"))
            } else if struct_names.contains(&name) {
                let name = escape_lua_ident(&name);
                Ok(format!("{target} = read_{name}($r)"))
            } else {
                Err(IdlLuaError::Unsupported(format!("scoped type {name}")))
            }
        }
        TypeSpec::Map(m) => {
            let key_get = map_get(&m.key, "zdK", enum_names, struct_names)?;
            let val_get = map_get(&m.value, "zdV", enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let dh = if prim { "" } else { "$r:getU32()\n  " };
            // B1 follow-up (#22 decode-side parity): inline check right
            // after the count is read — a map has no shared Reader method
            // to carry the check, mirroring the map_sequence struct-element
            // path above.
            let check = match m.bound.as_ref().and_then(array_size) {
                Some(n) => format!(
                    "\n  if zdN > {n} then error(string.format(\"decoded map length exceeds its IDL bound (%d)\", {n})) end"
                ),
                None => String::new(),
            };
            Ok(format!(
                "do\n  {dh}local zdN = $r:getU32(){check}\n  {target} = {{}}\n  for _ = 1, zdN do\n    local zdK\n    {key_get}\n    local zdV\n    {val_get}\n    {target}[zdK] = zdV\n  end\nend"
            ))
        }
        other => Err(IdlLuaError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType, target: &str) -> Result<String> {
    let s = match p {
        PrimitiveType::Octet | PrimitiveType::Char => format!("{target} = $r:getU8()"),
        PrimitiveType::Boolean => format!("{target} = $r:getBool()"),
        PrimitiveType::Integer(i) => return map_get_integer(i, target),
        PrimitiveType::Floating(FloatingType::Float) => format!("{target} = $r:getF32()"),
        PrimitiveType::Floating(FloatingType::Double) => format!("{target} = $r:getF64()"),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            format!("{target} = $r:getLongDouble()")
        }
        PrimitiveType::WideChar => format!("{target} = $r:getU32()"),
    };
    Ok(s)
}

fn map_get_integer(i: IntegerType, target: &str) -> Result<String> {
    // Read the unsigned wire form; the put path re-masks signed fields, so the
    // roundtrip reproduces the wire regardless of the value's sign.
    let s = match i {
        IntegerType::Int8 | IntegerType::UInt8 => format!("{target} = $r:getU8()"),
        IntegerType::UShort | IntegerType::UInt16 | IntegerType::Short | IntegerType::Int16 => {
            format!("{target} = $r:getU16()")
        }
        IntegerType::ULong | IntegerType::UInt32 | IntegerType::Long | IntegerType::Int32 => {
            format!("{target} = $r:getU32()")
        }
        IntegerType::LongLong
        | IntegerType::ULongLong
        | IntegerType::Int64
        | IntegerType::UInt64 => format!("{target} = $r:getU64()"),
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
        // B1 follow-up (#22 decode-side parity): pass the IDL bound N to the
        // shared Reader method.
        return match bound.and_then(array_size) {
            Some(n) => Ok(format!("{target} = $r:getSeqU8({n})")),
            None => Ok(format!("{target} = $r:getSeqU8()")),
        };
    }
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            // Inline check right after the count is read — this custom
            // block has no shared Reader method to carry the check.
            let check = match bound.and_then(array_size) {
                Some(n) => format!(
                    "\n  if zdN > {n} then error(string.format(\"decoded sequence length exceeds its IDL bound (%d)\", {n})) end"
                ),
                None => String::new(),
            };
            let name = escape_lua_ident(&name);
            return Ok(format!(
                "do\n  $r:getU32()\n  local zdN = $r:getU32(){check}\n  {target} = {{}}\n  for zdI = 1, zdN do {target}[zdI] = read_{name}($r) end\nend"
            ));
        }
    }
    Err(IdlLuaError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}
