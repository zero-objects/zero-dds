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
    Annotation, BinaryOp, BitmaskDecl, BitsetDecl, CaseLabel, ConstDecl, ConstExpr, ConstrTypeDecl,
    Declarator, Definition, EnumDef, Export, FixedPtType, FloatingType, IntegerType, InterfaceDcl,
    Literal, LiteralKind, Member, PrimitiveType, ScopedName, SequenceType, Specification,
    StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnaryOp, UnionDcl, UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, PlacementKind, enum_bit_bound, enum_wire_octets,
    lower_annotations, lower_single,
};

use crate::error::{IdlLuaError, Result};
use crate::keywords::escape_lua_ident;

thread_local! {
    /// Fully-qualified IDL scope path of every named type declaration
    /// (e.g. `["a", "Reading"]`), populated by [`register_type_paths`] at the
    /// start of each run. A reference site resolves a (possibly partially
    /// qualified) `ScopedName` against the enclosing module scope by walking
    /// outward and matching one of these paths (§7.5.2), then flattens the
    /// match the SAME way [`qualify`] flattens the definition (#21).
    static TYPE_PATHS: std::cell::RefCell<Vec<Vec<String>>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Module scope of the aggregate currently being emitted. Set at the top of
    /// [`emit_struct`]/[`emit_union`]; empty at global scope.
    static CURRENT_SCOPE: std::cell::RefCell<Vec<String>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Flattened logical names of every `bitset`/`bitmask` declaration. A
    /// reference to one of these maps to a Lua holder table whose wire form is a
    /// single backing integer (`marshalInto_<name>`/`read_<name>`) — no
    /// collection DHEADER, so it is treated as fully-descriptive (primitive) by
    /// the sequence/map framing rules (XTypes 1.3 §7.4.7).
    static BIT_NAMES: std::cell::RefCell<HashSet<String>> =
        std::cell::RefCell::new(HashSet::new());

    /// Set whenever a `fixed<P,S>` member is emitted, so the BCD prelude helper
    /// is appended exactly once (and only when needed).
    static USED_FIXED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

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

/// Lua codegen language aliases matched by `@verbatim(language="...")`
/// (case-insensitive; the spec wildcard `"*"` always matches — see
/// [`Lowered::verbatims_for_language`]).
const LUA_LANG_ALIASES: &[&str] = &["lua"];

/// Emits every `@verbatim` block from `anns` whose language matches the Lua
/// codegen and whose `placement` equals `placement`, each line prefixed with
/// `indent`. Source order preserved; text spliced unmodified (no wire impact —
/// XTypes 1.3 §7.2.2.4.8 / IDL 4.2 §8.3.5.1). Mirrors `idl-d`'s
/// `emit_verbatim_at`.
fn emit_verbatim_at(out: &mut String, indent: &str, anns: &[Annotation], placement: PlacementKind) {
    let Ok(lowered) = lower_annotations(anns) else {
        return;
    };
    for v in lowered.verbatims_for_language(LUA_LANG_ALIASES) {
        if v.placement != placement {
            continue;
        }
        for line in v.text.lines() {
            out.push_str(indent);
            out.push_str(line);
            out.push('\n');
        }
    }
}

/// Top-level annotations of a definition, for file-scope (`BEGIN_FILE` /
/// `END_FILE`) and per-declaration `@verbatim` placement.
fn def_annotations(d: &Definition) -> &[Annotation] {
    match d {
        Definition::Module(m) => &m.annotations,
        Definition::Type(TypeDecl::Constr(c)) => match c {
            ConstrTypeDecl::Struct(StructDcl::Def(s)) => &s.annotations,
            ConstrTypeDecl::Union(UnionDcl::Def(u)) => &u.annotations,
            ConstrTypeDecl::Enum(e) => &e.annotations,
            ConstrTypeDecl::Bitset(b) => &b.annotations,
            ConstrTypeDecl::Bitmask(b) => &b.annotations,
            _ => &[],
        },
        Definition::Type(TypeDecl::Typedef(t)) => &t.annotations,
        Definition::Const(c) => &c.annotations,
        Definition::Except(e) => &e.annotations,
        _ => &[],
    }
}

/// Collision-free flattened name for a declaration `simple` in module `scope`:
/// the injective flattening of `scope + simple`, or the bare `simple` at global
/// scope (so every existing top-level golden is unchanged). Two same-simple-name
/// types in different modules become distinct types `a_Reading`/`b_Reading`
/// (#21).
fn qualify(scope: &[String], simple: &str) -> String {
    if scope.is_empty() {
        simple.to_string()
    } else {
        let mut parts = scope.to_vec();
        parts.push(simple.to_string());
        flatten_path(&parts)
    }
}

/// Injectively flattens a module-qualified path (`["a", "b", "C"]`) into a
/// single Lua identifier. Each segment's own underscores are doubled and the
/// segments joined by a single underscore, so `module A_B { struct C }`
/// (`["A_B","C"]` → `A__B_C`) never collides with `module A { module B {
/// struct C }}` (`["A","B","C"]` → `A_B_C`) — the previous `join("_")` mapped
/// both to `A_B_C` (#A35, non-injective flatten). A single (global-scope)
/// segment is returned verbatim so every existing top-level golden is
/// unchanged, and any segment without underscores (the common case) is passed
/// through untouched.
fn flatten_path(parts: &[String]) -> String {
    if parts.len() <= 1 {
        return parts.first().cloned().unwrap_or_default();
    }
    parts
        .iter()
        .map(|p| p.replace('_', "__"))
        .collect::<Vec<_>>()
        .join("_")
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
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) => {
                push_type_path(scope, &b.name.text);
            }
            Definition::Type(TypeDecl::Typedef(td)) => {
                for d in &td.declarators {
                    push_type_path(scope, &d.name().text);
                }
            }
            // Interface-nested types are promoted to the top level under the
            // interface's own scope segment (#A39), so their reference paths
            // must be registered the same way.
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                scope.push(iface.name.text.clone());
                for ex in &iface.exports {
                    if let Export::Type(td) = ex {
                        register_type_decl_path(td, scope);
                    }
                }
                scope.pop();
            }
            _ => {}
        }
    }
}

/// Registers the flattened path of a single `TypeDecl` (module- or
/// interface-scoped), mirroring the per-kind arms in [`register_type_paths`].
fn register_type_decl_path(td: &TypeDecl, scope: &[String]) {
    match td {
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            push_type_path(scope, &s.name.text);
        }
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => push_type_path(scope, &e.name.text),
        TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u))) => {
            push_type_path(scope, &u.name.text);
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => push_type_path(scope, &b.name.text),
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => push_type_path(scope, &b.name.text),
        TypeDecl::Typedef(td) => {
            for d in &td.declarators {
                push_type_path(scope, &d.name().text);
            }
        }
        _ => {}
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
/// yet emit (e.g. `@mutable` unions and non-literal array/sequence bounds).
pub fn generate_lua_module(spec: &Specification, _opts: &LuaGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "-- Code generated by zerodds-idlc (Lua backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "-- SPDX-License-Identifier: Apache-2.0\n");
    out.push_str(WIRE_PRELUDE);

    // Register every named type's fully-qualified path so reference sites can
    // resolve a `ScopedName` against its enclosing scope (#21 cross-module).
    TYPE_PATHS.with(|t| t.borrow_mut().clear());
    register_type_paths(&spec.definitions, &mut Vec::new());
    USED_FIXED.with(|f| f.set(false));

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_FILE)` from all top-level defs
    // (source order), emitted after the wire prelude, before any type.
    for def in &spec.definitions {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::BeginFile);
    }

    // `module X { ... }` content is promoted to the top level, each definition
    // paired with its module scope path (see `flatten_module_defs`).
    let flat = flatten_module_defs(&spec.definitions);
    // Interface-nested type declarations (#A39): promoted to the top level under
    // the interface's own scope segment, so their DDS data types survive instead
    // of being silently dropped with the interface body.
    let iface_types = flatten_iface_types(&spec.definitions);

    // Named enums/structs/bit-containers referenced by members, keyed by their
    // flattened module-qualified name. `bit_names` is published to `BIT_NAMES` so
    // a reference site resolves them to the integer-backed holder (no collection
    // DHEADER). Interface-nested types are folded in the same way (#A39).
    let mut bit_names: HashSet<String> = HashSet::new();
    let mut enum_names: HashSet<String> = HashSet::new();
    let mut struct_names: HashSet<String> = HashSet::new();
    let mut enum_defs: HashMap<String, &EnumDef> = HashMap::new();
    for (scope, td) in flat
        .iter()
        .filter_map(|(s, d)| match d {
            Definition::Type(td) => Some((s, td)),
            _ => None,
        })
        .chain(iface_types.iter().map(|(s, td)| (s, *td)))
    {
        match td {
            TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => {
                let n = qualify(scope, &e.name.text);
                enum_defs.insert(n.clone(), e);
                enum_names.insert(n);
            }
            TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
                struct_names.insert(qualify(scope, &s.name.text));
            }
            TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => {
                bit_names.insert(qualify(scope, &b.name.text));
            }
            TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => {
                bit_names.insert(qualify(scope, &b.name.text));
            }
            _ => {}
        }
    }
    BIT_NAMES.with(|b| *b.borrow_mut() = bit_names);
    // Register each enum's @bit_bound-derived wire width (1/2/4 octets), P1.
    ENUM_WIDTHS.with(|m| {
        let mut m = m.borrow_mut();
        m.clear();
        for (name, e) in &enum_defs {
            m.insert(
                name.clone(),
                u32::from(enum_wire_octets(enum_bit_bound(&e.annotations))),
            );
        }
    });

    // Qualified-name -> StructDef, so a nested-struct `@key` member's own
    // `@key` subset (and `keyhash::uses_md5`'s static max-size analysis) can be
    // resolved — mirrors `struct_names` above, keeping the full def. Typedef
    // aliases are wire-transparent and resolved before mapping. Interface-nested
    // structs/typedefs are folded in too (#A39).
    let mut typedefs = collect_typedefs(spec);
    let mut structs: HashMap<String, &StructDef> = flat
        .iter()
        .filter_map(|(scope, d)| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                Some((qualify(scope, &s.name.text), s))
            }
            _ => None,
        })
        .collect();
    for (scope, td) in &iface_types {
        match td {
            TypeDecl::Typedef(tdd) => {
                for d in &tdd.declarators {
                    if let Declarator::Simple(name) = d {
                        typedefs.insert(qualify(scope, &name.text), tdd.type_spec.clone());
                    }
                }
            }
            TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
                structs.insert(qualify(scope, &s.name.text), s);
            }
            _ => {}
        }
    }

    for (scope, def) in &flat {
        let anns = def_annotations(def);
        // §7.2.2.4.8 — text directly before the annotated declaration.
        emit_verbatim_at(&mut out, "", anns, PlacementKind::BeforeDeclaration);
        match def {
            Definition::Type(td) => emit_type_decl(
                &mut out,
                td,
                scope,
                &enum_names,
                &struct_names,
                &structs,
                &typedefs,
                &enum_defs,
            )?,
            // #A5/P1 — a top-level `const` was silently dropped by the former
            // catch-all arm; emit it as a Lua chunk-local binding.
            Definition::Const(c) => emit_const(&mut out, c, scope),
            _ => {}
        }
        // §7.2.2.4.8 — text directly after the annotated declaration.
        emit_verbatim_at(&mut out, "", anns, PlacementKind::AfterDeclaration);
    }

    // Interface-nested types (#A39), emitted after the module-level defs.
    for (scope, td) in &iface_types {
        emit_type_decl(
            &mut out,
            td,
            scope,
            &enum_names,
            &struct_names,
            &structs,
            &typedefs,
            &enum_defs,
        )?;
    }

    // §7.2.2.4.8 — `@verbatim(placement=END_FILE)` from all top-level defs.
    for def in &spec.definitions {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::EndFile);
    }

    // The BCD codec prelude is appended once if any `fixed<P,S>` was emitted.
    if USED_FIXED.with(std::cell::Cell::get) {
        out.push_str(FIXED_PRELUDE);
    }
    // Self-contained MD5 (RFC 1321) for the KeyHash MD5 branch; a global fn
    // appended on demand (Lua resolves the call at run time, so order is fine).
    if out.contains("zd_md5(") {
        out.push_str(LUA_MD5);
    }
    Ok(out)
}

/// BCD codec for `fixed<P,S>`. Appended once when any `fixed` member is emitted.
/// Builds the packed-BCD octet string (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5)
/// from a decimal string: an optional leading pad nibble (so the nibble count
/// is even), `P` digit nibbles most-significant first, then the sign nibble
/// (`0xC` positive, `0xD` negative). Byte count `(P+2)/2`, no length prefix.
const FIXED_PRELUDE: &str = r#"
function zdFixedEnc(s, P, S)
  local sign = true
  local i = 1
  local c1 = string.sub(s, 1, 1)
  if c1 == "-" or c1 == "+" then sign = (c1 ~= "-"); i = 2 end
  local rest = string.sub(s, i)
  local dot = string.find(rest, ".", 1, true)
  local ip, fp
  if dot then ip = string.sub(rest, 1, dot - 1); fp = string.sub(rest, dot + 1) else ip = rest; fp = "" end
  local db = ""
  local intNeeded = P - S
  if #ip < intNeeded then db = string.rep("0", intNeeded - #ip) end
  db = db .. ip .. fp
  if #fp < S then db = db .. string.rep("0", S - #fp) end
  local nib = {}
  if (P + 1) % 2 == 1 then nib[#nib + 1] = 0 end
  local zero = string.byte("0")
  for k = 1, #db do nib[#nib + 1] = string.byte(db, k) - zero end
  nib[#nib + 1] = sign and 0x0C or 0x0D
  local out = {}
  for k = 1, #nib, 2 do out[#out + 1] = string.char((nib[k] << 4) | nib[k + 1]) end
  return table.concat(out)
end
"#;

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

/// Dispatches a single `TypeDecl` (module- or interface-scoped) to its emitter.
/// Shared by the top-level loop and the interface-nested-type pass (#A39).
#[allow(clippy::too_many_arguments)]
fn emit_type_decl(
    out: &mut String,
    td: &TypeDecl,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<()> {
    match td {
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => emit_enum(out, e, scope),
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            emit_struct(out, s, scope, enum_names, struct_names, structs, typedefs)?;
        }
        TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u))) => {
            emit_union(out, u, scope, enum_names, struct_names, typedefs, enum_defs)?;
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => emit_bitset(out, b, scope)?,
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => emit_bitmask(out, b, scope),
        _ => {}
    }
    Ok(())
}

/// Emits an IDL `enum` as a Lua constants table (its member is an i32 field).
fn emit_enum(out: &mut String, e: &EnumDef, scope: &[String]) {
    let values = enumerator_values(e);
    let ty = escape_lua_ident(&qualify(scope, &e.name.text));
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

/// Backing-integer storage for a bit container of `total_bits` bits: XTypes 1.3
/// §7.4.7 — the smallest holder that fits (`≤8`→u8, `≤16`→u16, `≤32`→u32, else
/// u64). Returns `(put-method, get-method, mask literal)`. The mask keeps the
/// stored value within its backing width before `string.pack` (which raises on
/// an out-of-range integer for the 2/4/8-byte forms).
fn bit_storage(total_bits: usize) -> (&'static str, &'static str, &'static str) {
    match total_bits {
        0..=8 => ("putU8", "getU8", "0xff"),
        9..=16 => ("putU16", "getU16", "0xffff"),
        17..=32 => ("putU32", "getU32", "0xffffffff"),
        _ => ("putU64", "getU64", "0xffffffffffffffff"),
    }
}

/// Effective `@bit_bound` of a bitmask (default 32 — XTypes 1.3 §7.3.1.2.1.1:
/// an unannotated bitmask is a UInt32 on the wire, NOT the count of bits).
fn bitmask_bit_bound(anns: &[Annotation]) -> u32 {
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

/// `@position(n)` of a bitmask value, if present.
fn bit_position(anns: &[Annotation]) -> Option<u32> {
    lower_annotations(anns).ok().and_then(|l| {
        l.builtins.iter().find_map(|a| match a {
            BuiltinAnnotation::Position(n) => Some(*n),
            _ => None,
        })
    })
}

/// `true` if `name` resolves to a `bitset`/`bitmask` declaration (its wire form
/// is a single backing integer — fully descriptive, no collection DHEADER).
fn is_bit_name(name: &str) -> bool {
    BIT_NAMES.with(|b| b.borrow().contains(name))
}

/// Emits an IDL `bitset` as a Lua holder table over its backing integer
/// (`v.storage`), a bit-accessor pair per named bitfield, and an XCDR2
/// marshal/read that writes the backing integer (XTypes 1.3 §7.4.7 — wire =
/// backing int, no DHEADER).
///
/// # Errors
/// [`IdlLuaError::Unsupported`] if a bitfield width is not a codegen-time
/// non-negative integer.
fn emit_bitset(out: &mut String, b: &BitsetDecl, scope: &[String]) -> Result<()> {
    let mut widths: Vec<usize> = Vec::with_capacity(b.bitfields.len());
    for bf in &b.bitfields {
        let w = array_size(&bf.spec.width)
            .filter(|w| *w >= 0)
            .ok_or_else(|| {
                IdlLuaError::Unsupported(format!(
                    "non-integer bitfield width in bitset {}",
                    b.name.text
                ))
            })? as usize;
        widths.push(w);
    }
    let total: usize = widths.iter().sum();
    let (put, get, mask) = bit_storage(total);
    let ty = escape_lua_ident(&qualify(scope, &b.name.text));

    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "", &b.annotations, PlacementKind::BeginDeclaration);
    let mut offset: usize = 0;
    for (bf, width) in b.bitfields.iter().zip(&widths) {
        if let Some(name) = &bf.name {
            let field = escape_lua_ident(&name.text);
            if *width == 1 {
                let _ = writeln!(
                    out,
                    "\nfunction {ty}_{field}(v) return ((v.storage >> {offset}) & 1) ~= 0 end"
                );
                let _ = writeln!(
                    out,
                    "function {ty}_set_{field}(v, x) local m = 1 << {offset}; if x then v.storage = v.storage | m else v.storage = v.storage & ~m end end"
                );
            } else {
                let bmask: u128 = if *width >= 128 {
                    u128::MAX
                } else {
                    (1u128 << *width) - 1
                };
                let _ = writeln!(
                    out,
                    "\nfunction {ty}_{field}(v) return (v.storage >> {offset}) & {bmask} end"
                );
                let _ = writeln!(
                    out,
                    "function {ty}_set_{field}(v, x) local m = {bmask} << {offset}; v.storage = (v.storage & ~m) | ((x & {bmask}) << {offset}) end"
                );
            }
        }
        offset += width;
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(
        out,
        "\nfunction marshalInto_{ty}(w, v) w:{put}(v.storage & {mask}) end"
    );
    let _ = writeln!(
        out,
        "function marshal_{ty}(v, endian) local w = Writer.new(endian); marshalInto_{ty}(w, v); return w:bytes() end"
    );
    let _ = writeln!(
        out,
        "function read_{ty}(r) return {{ storage = r:{get}() }} end"
    );
    let _ = writeln!(
        out,
        "function unmarshal_{ty}(buf, endian) return read_{ty}(Reader.new(buf, endian)) end"
    );
    Ok(())
}

/// Emits an IDL `bitmask` as a Lua holder (`v.storage`) plus an OR-able
/// manifest-constant table (one `1<<pos` entry per bit value) and an XCDR2
/// marshal/read writing the `@bit_bound` backing integer (default 32 — XTypes
/// 1.3 §7.4.7).
fn emit_bitmask(out: &mut String, b: &BitmaskDecl, scope: &[String]) {
    let (put, get, mask) = bit_storage(bitmask_bit_bound(&b.annotations) as usize);
    let ty = escape_lua_ident(&qualify(scope, &b.name.text));

    emit_verbatim_at(out, "", &b.annotations, PlacementKind::BeginDeclaration);
    let consts: Vec<String> = b
        .values
        .iter()
        .enumerate()
        .map(|(idx, v)| {
            let pos = bit_position(&v.annotations).unwrap_or(idx as u32);
            format!("{} = 1 << {pos}", escape_lua_ident(&v.name.text))
        })
        .collect();
    let _ = writeln!(out, "\nlocal {ty} = {{ {} }}", consts.join(", "));
    emit_verbatim_at(out, "", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(
        out,
        "\nfunction marshalInto_{ty}(w, v) w:{put}(v.storage & {mask}) end"
    );
    let _ = writeln!(
        out,
        "function marshal_{ty}(v, endian) local w = Writer.new(endian); marshalInto_{ty}(w, v); return w:bytes() end"
    );
    let _ = writeln!(
        out,
        "function read_{ty}(r) return {{ storage = r:{get}() }} end"
    );
    let _ = writeln!(
        out,
        "function unmarshal_{ty}(buf, endian) return read_{ty}(Reader.new(buf, endian)) end"
    );
}

/// Resolves a `fixed<P,S>`'s digit count `P` and scale `S` to codegen-time
/// integers.
///
/// # Errors
/// [`IdlLuaError::Unsupported`] if either is not a resolvable non-negative
/// integer literal.
fn fixed_ps(f: &FixedPtType) -> Result<(i64, i64)> {
    let p = array_size(&f.digits)
        .filter(|v| *v > 0)
        .ok_or_else(|| IdlLuaError::Unsupported("non-integer fixed digit count".to_string()))?;
    let s = array_size(&f.scale)
        .filter(|v| *v >= 0)
        .ok_or_else(|| IdlLuaError::Unsupported("non-integer fixed scale".to_string()))?;
    Ok((p, s))
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

/// Recursively descends into `Definition::Interface` bodies, returning every
/// interface-nested `Export::Type` declaration paired with the scope path
/// `enclosing_module… + interface_name` (#A39). Lua has no nested-type
/// construct, so these are promoted to the top level under the interface's own
/// name segment (so two interfaces in one module do not collide).
/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_iface_types(defs: &[Definition]) -> Vec<(Vec<String>, &TypeDecl)> {
    let mut out = Vec::new();
    let mut scope = Vec::new();
    flatten_iface_types_into(defs, &mut scope, &mut out);
    out
}

/// zerodds-lint: recursion-depth 16 (module nesting; bounded by the IDL grammar).
fn flatten_iface_types_into<'a>(
    defs: &'a [Definition],
    scope: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, &'a TypeDecl)>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                flatten_iface_types_into(&m.definitions, scope, out);
                scope.pop();
            }
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                scope.push(iface.name.text.clone());
                for ex in &iface.exports {
                    if let Export::Type(td) = ex {
                        out.push((scope.clone(), td));
                    }
                }
                scope.pop();
            }
            _ => {}
        }
    }
}

/// Emits a top-level IDL `const` as a Lua chunk-local binding (`local NAME =
/// value`) — #A5/P1. The former catch-all arm dropped every `const`. The value
/// is a codegen convenience (no wire impact); a form the Lua backend cannot
/// render (an enum-valued / const-alias scoped reference) is skipped rather than
/// emitting an invalid identifier.
fn emit_const(out: &mut String, c: &ConstDecl, scope: &[String]) {
    let Some(val) = const_expr_to_lua(&c.value) else {
        return;
    };
    let name = escape_lua_ident(&qualify(scope, &c.name.text));
    let _ = writeln!(out, "\nlocal {name} = {val}");
}

/// Renders a `ConstExpr` as a Lua expression, or `None` for a form the Lua
/// backend does not express (an enum-valued / const-alias scoped reference).
/// zerodds-lint: recursion-depth 32 (const expression tree; bounded by the IDL
/// grammar's expression nesting).
fn const_expr_to_lua(e: &ConstExpr) -> Option<String> {
    match e {
        ConstExpr::Literal(l) => const_literal_to_lua(l),
        // An enum-valued or const-alias scoped reference cannot be rendered from
        // the bare last segment; skip (wire-neutral).
        ConstExpr::Scoped(_) => None,
        ConstExpr::Unary { op, operand, .. } => {
            let v = const_expr_to_lua(operand)?;
            // Lua has no unary `+` operator, so a leading plus is dropped.
            let o = match op {
                UnaryOp::Plus => "",
                UnaryOp::Minus => "-",
                UnaryOp::BitNot => "~",
            };
            Some(format!("{o}{v}"))
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let l = const_expr_to_lua(lhs)?;
            let r = const_expr_to_lua(rhs)?;
            let o = match op {
                BinaryOp::Or => "|",
                BinaryOp::Xor => "~",
                BinaryOp::And => "&",
                BinaryOp::Shl => "<<",
                BinaryOp::Shr => ">>",
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "//",
                BinaryOp::Mod => "%",
            };
            Some(format!("({l} {o} {r})"))
        }
    }
}

/// Renders a single literal as a valid Lua expression.
fn const_literal_to_lua(l: &Literal) -> Option<String> {
    let raw = l.raw.trim();
    Some(match l.kind {
        // Re-render integers in decimal so an IDL octal/hex literal maps to the
        // right Lua value (Lua reads a leading-zero literal as decimal, not
        // octal). Fall back to the raw text if it is not a plain int/hex.
        LiteralKind::Integer => parse_int(raw).map_or_else(|| raw.to_string(), |v| v.to_string()),
        // Strip a trailing IDL float/fixed suffix (`d`/`f`/`l`) Lua rejects.
        LiteralKind::Floating => raw
            .trim_end_matches(['d', 'D', 'f', 'F', 'l', 'L'])
            .to_string(),
        // A `fixed` decimal has no native Lua type — render as a string literal.
        LiteralKind::Fixed => format!(
            "\"{}\"",
            raw.trim_end_matches(['d', 'D']).replace('"', "\\\"")
        ),
        // A char/wchar const has no native Lua type — render its code point as an
        // integer (`'A'` → 65). Fall back to the raw text if it cannot be parsed.
        LiteralKind::Char | LiteralKind::WideChar => {
            char_literal_value(raw).map_or_else(|| raw.to_string(), |v| v.to_string())
        }
        // The IDL boolean keywords map to Lua `true`/`false` (never a bare
        // `TRUE`/`FALSE` token, which is not a Lua identifier — #A13).
        LiteralKind::Boolean => {
            if raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // Narrow string literals pass through; wide literals drop the `L` prefix
        // (`L"x"` is not valid Lua).
        LiteralKind::String => raw.to_string(),
        LiteralKind::WideString => raw.strip_prefix('L').unwrap_or(raw).to_string(),
    })
}

/// Evaluates a `char`/`wchar` literal (`'A'`, `L'x'`, `'\n'`) to its code point.
/// Used by the union label evaluator (#A12) so a `case 'A':` resolves to the
/// discriminant 65, and by the char const renderer.
fn char_literal_value(raw: &str) -> Option<i64> {
    let s = raw.trim().strip_prefix('L').unwrap_or(raw.trim());
    let inner = s.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut it = inner.chars();
    let c = it.next()?;
    if c == '\\' {
        // Common C-style escapes (XTypes/IDL char literal grammar).
        let e = it.next()?;
        let v = match e {
            'n' => 0x0A,
            't' => 0x09,
            'r' => 0x0D,
            '0' => 0x00,
            '\\' => 0x5C,
            '\'' => 0x27,
            '"' => 0x22,
            'a' => 0x07,
            'b' => 0x08,
            'f' => 0x0C,
            'v' => 0x0B,
            // `\xHH` hex escape.
            'x' => return i64::from_str_radix(it.as_str(), 16).ok(),
            _ => return None,
        };
        Some(v)
    } else {
        Some(i64::from(u32::from(c)))
    }
}

/// Evaluates a union case label (`case RED:`, `case 'A':`, `case TRUE:`,
/// `case 3:`) to its integer discriminant (#A11/A12/A13/P4). Beyond the plain
/// integer literals the former `array_size` accepted, this resolves enum
/// enumerators (via `enum_vals`, name → value of the switch enum), `char` code
/// points, and the `boolean` keywords `TRUE`/`FALSE`.
/// zerodds-lint: recursion-depth 64 (Const-Expr-Tree; bounded by IDL nesting)
fn eval_union_label(e: &ConstExpr, enum_vals: &HashMap<String, i64>) -> Option<i64> {
    match e {
        ConstExpr::Literal(Literal { kind, raw, .. }) => match kind {
            LiteralKind::Integer => parse_int(raw),
            LiteralKind::Char | LiteralKind::WideChar => char_literal_value(raw),
            LiteralKind::Boolean => Some(i64::from(raw.trim().eq_ignore_ascii_case("true"))),
            _ => None,
        },
        // `case ENUMERATOR:` — the label names an enumerator of the switch enum
        // (resolved by its simple, i.e. last, segment).
        ConstExpr::Scoped(sn) => {
            let last = sn.parts.last()?.text.clone();
            enum_vals.get(&last).copied()
        }
        ConstExpr::Unary { op, operand, .. } => {
            let v = eval_union_label(operand, enum_vals)?;
            match op {
                UnaryOp::Plus => Some(v),
                UnaryOp::Minus => Some(-v),
                UnaryOp::BitNot => Some(!v),
            }
        }
        ConstExpr::Binary { .. } => None,
    }
}

/// Collects `typedef` aliases (simple declarators) as qualified-name -> aliased
/// type-spec. A typedef is wire-transparent, so members are resolved to the
/// underlying type before mapping (`typedef long Score; Score s;` → `long`).
fn collect_typedefs(spec: &Specification) -> HashMap<String, TypeSpec> {
    let mut m = HashMap::new();
    for (scope, def) in flatten_module_defs(&spec.definitions) {
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

/// Collects a struct's effective members base-first (#A10/P3): the base
/// struct's members (recursively) precede the derived struct's own, so the
/// generated marshaller and its wire form carry the inherited fields — matching
/// cpp/csharp/java. Without this a `struct D : Base` dropped every inherited
/// field from both the emitted holder and the wire.
/// zerodds-lint: recursion-depth 16 (struct inheritance chain; bounded by the
/// IDL aggregate nesting depth).
fn collect_base_members<'a>(
    s: &'a StructDef,
    structs: &HashMap<String, &'a StructDef>,
    out: &mut Vec<&'a Member>,
) {
    if let Some(base) = &s.base {
        if let Some(bs) = structs.get(&resolve_scoped_name(base)) {
            collect_base_members(bs, structs, out);
        }
    }
    for m in &s.members {
        out.push(m);
    }
}

fn emit_struct(
    out: &mut String,
    s: &StructDef,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    structs: &HashMap<String, &StructDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<()> {
    // Member references resolve against this struct's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = extensibility(s);
    // #A10/P3: base-first effective member list (inherited members precede the
    // derived struct's own, and share the same sequential member-id space).
    let mut all_members: Vec<&Member> = Vec::new();
    collect_base_members(s, structs, &mut all_members);

    struct FieldGen {
        name: String,
        put: String,
        get: String,
        id: u32,
        key: bool,
        // `@optional`: a companion uint8 presence flag precedes the value on
        // the wire (final/appendable); the mutable encoder instead gates the
        // whole EMHEADER+body on the flag (XTypes 1.3 §7.4.5.1.4 / §7.4.3.4.2).
        optional: bool,
        // `Some((type_spec, expr))` for a Simple (non-array) declarator, so a
        // `@key` field can be re-mapped through `map_key_type` instead of
        // reusing `put` (which, for a struct-typed member, is the full
        // `marshalInto_<T>` call shared with normal, non-key encoding). `None`
        // for an array declarator — array key fields are emitted unchanged.
        key_type: Option<(TypeSpec, String)>,
        // `@must_understand`: sets EMHEADER bit 31 in the `@mutable` encoder
        // (#A17); wire-neutral for final/appendable.
        must_understand: bool,
        // `@non_serialized`: stays in the Lua table (dynamic field), off the wire.
        non_serialized: bool,
    }
    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    for m in &all_members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        let optional = lowered.as_ref().is_some_and(|l| {
            l.builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::Optional))
        });
        let must_understand = lowered.as_ref().is_some_and(|l| {
            l.builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::MustUnderstand))
        });
        // P0-5 (#2): a `@non_serialized` member stays in the Lua table but is off
        // the wire and does NOT consume a sequential id slot (ids compact).
        let non_serialized =
            zerodds_idl::semantics::annotations::member_is_non_serialized(&m.annotations);
        for d in &m.declarators {
            let id = if non_serialized {
                0
            } else {
                let assigned = explicit_id.unwrap_or(next_id);
                next_id = assigned + 1;
                assigned
            };
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
                name,
                put,
                get,
                id,
                key,
                optional,
                key_type,
                must_understand,
                non_serialized,
            });
        }
    }

    let ty = escape_lua_ident(&qualify(scope, &s.name.text));

    // §7.2.2.4.8 — text as the first element inside the declaration (Lua has no
    // struct block, so declaration-scoped verbatim rides just before the
    // marshaller group).
    emit_verbatim_at(out, "", &s.annotations, PlacementKind::BeginDeclaration);

    // marshalInto_<T> writes into an existing writer (nested composites call this
    // so alignment stays stream-relative). @final: inline; @appendable: DHEADER.
    let _ = writeln!(out, "\nfunction marshalInto_{ty}(w, v)");
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "  local body = Writer.new(w.endian)");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            // An `@optional` member is omitted from the member list when absent
            // (XTypes 1.3 §7.4.3.4.2): gate its EMHEADER+body on the flag. The
            // presence is signaled by the EMHEADER's existence, so no companion
            // uint8 flag is written inside the body here (unlike final/appendable).
            if f.optional {
                let _ = writeln!(out, "  if v.{}_present then", f.name);
            }
            // LC4 (bits 30-28 = 0b100) | member id, plus the must-understand bit
            // 31 when `@must_understand` (#A17). LC4 is the always-decodable form
            // shared with the golden reference and every thin backend; the
            // compact per-width length codes (#A19) are a separate coordinated
            // cross-backend wire change and deliberately not applied here.
            let mu_bit = if f.must_understand {
                0x8000_0000_u32
            } else {
                0
            };
            let emh = mu_bit | 0x4000_0000 | (f.id & 0x0FFF_FFFF);
            let _ = writeln!(out, "  body:putU32(0x{emh:08x})");
            let _ = writeln!(out, "  local zdMem = Writer.new(w.endian)");
            let _ = writeln!(out, "  {}", f.put.replace("$w", "zdMem"));
            let _ = writeln!(out, "  local zdMB = zdMem:bytes()");
            let _ = writeln!(out, "  body:putU32(#zdMB)");
            let _ = writeln!(out, "  body:putBytes(zdMB)");
            if f.optional {
                let _ = writeln!(out, "  end");
            }
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
            if f.non_serialized {
                continue;
            }
            let put = f.put.replace("$w", wv);
            if f.optional {
                // uint8 presence flag then the value if present (§7.4.5.1.4).
                let _ = writeln!(out, "  {wv}:putU8(v.{}_present and 1 or 0)", f.name);
                let _ = writeln!(out, "  if v.{}_present then {put} end", f.name);
            } else {
                let _ = writeln!(out, "  {put}");
            }
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
    let mut zdkeys: Vec<&FieldGen> = fields
        .iter()
        .filter(|f| f.key && !f.non_serialized)
        .collect();
    zdkeys.sort_by_key(|f| f.id);
    if !zdkeys.is_empty() {
        let key_members: Vec<&Member> = all_members
            .iter()
            .copied()
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
        // Naive @mutable decoder: reads members in declaration order, one
        // EMHEADER+NEXTINT per member. An `@optional` member absent on the wire
        // omits its EMHEADER, so this decoder only reconstructs a mutable
        // struct whose optional members were all present at encode time — the
        // absent-optional case is NOT claimed (matches idl-d / idl-nim).
        let _ = writeln!(out, "  r:getU32()");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            let _ = writeln!(out, "  r:getU32()");
            let _ = writeln!(out, "  r:getU32()");
            let _ = writeln!(out, "  {}", f.get.replace("$r", "r"));
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "  r:getU32()");
        }
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            let get = f.get.replace("$r", "r");
            if f.optional {
                // uint8 presence flag then the value only if present (§7.4.5.1.4).
                let _ = writeln!(out, "  v.{}_present = r:getBool()", f.name);
                let _ = writeln!(out, "  if v.{}_present then {get} end", f.name);
            } else {
                let _ = writeln!(out, "  {get}");
            }
        }
    }
    let _ = writeln!(out, "  return v");
    let _ = writeln!(out, "end");
    let _ = writeln!(out, "\nfunction unmarshal_{ty}(buf, endian)");
    let _ = writeln!(out, "  return read_{ty}(Reader.new(buf, endian))");
    let _ = writeln!(out, "end");
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "", &s.annotations, PlacementKind::EndDeclaration);
    Ok(())
}

/// Emits one `@mutable` member: its EMHEADER (LC4 length code | member id, with
/// must-understand bit 31 when `mu` — #A17) then the value as NEXTINT-prefixed
/// body bytes, at `indent`. LC4 is the always-decodable form shared with the
/// golden reference and every thin backend, keeping the wire byte-identical; the
/// compact per-width length codes (#A19) are a separate coordinated cross-backend
/// change and are deliberately not applied here.
fn emit_mutable_member(out: &mut String, indent: &str, wv: &str, id: u32, mu: bool, put: &str) {
    let mu_bit = if mu { 0x8000_0000_u32 } else { 0 };
    let emh = mu_bit | 0x4000_0000 | (id & 0x0FFF_FFFF);
    let _ = writeln!(out, "{indent}{wv}:putU32(0x{emh:08x})");
    let _ = writeln!(out, "{indent}local zdMem = Writer.new({wv}.endian)");
    let _ = writeln!(out, "{indent}{}", put.replace("$w", "zdMem"));
    let _ = writeln!(out, "{indent}local zdMB = zdMem:bytes()");
    let _ = writeln!(out, "{indent}{wv}:putU32(#zdMB)");
    let _ = writeln!(out, "{indent}{wv}:putBytes(zdMB)");
}

/// Reads one `@mutable` member: skips its EMHEADER + NEXTINT (LC4) then reads the
/// value via `get`. Positional — relies on members arriving in id order.
fn emit_mutable_member_decode(out: &mut String, indent: &str, get: &str) {
    let _ = writeln!(out, "{indent}r:getU32()");
    let _ = writeln!(out, "{indent}r:getU32()");
    let _ = writeln!(out, "{indent}{get}");
}

/// Emits an IDL `union` as a discriminated Lua table marshaller: put the
/// discriminator then an `if`-chain dispatches on it (XCDR2 §7.4.3.5.4).
fn emit_union(
    out: &mut String,
    u: &UnionDef,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<()> {
    // Member references resolve against this union's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = lower_annotations(&u.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable);
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

    // #P4: when the discriminator is an enum, build enumerator-name → value so
    // `case ENUMERATOR:` labels resolve to their integer discriminant.
    let enum_vals: HashMap<String, i64> = match &u.switch_type {
        SwitchTypeSpec::Scoped(sn) => enum_defs
            .get(&resolve_scoped_name(sn))
            .map(|e| {
                e.enumerators
                    .iter()
                    .zip(enumerator_values(e))
                    .map(|(en, v)| (en.name.text.clone(), i64::from(v)))
                    .collect()
            })
            .unwrap_or_default(),
        _ => HashMap::new(),
    };
    // A boolean discriminator compares against Lua `true`/`false`, not integers
    // (the decoded `v.disc` is a Lua boolean); every other discriminator is an
    // integer/enum/char number.
    let disc_is_bool = matches!(u.switch_type, SwitchTypeSpec::Boolean);
    let render_label = |l: i64| -> String {
        if disc_is_bool {
            (l != 0).to_string()
        } else {
            l.to_string()
        }
    };

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
                // #A11/A12/A13/P4: resolve enum/char/boolean labels, not only the
                // plain integer literals the former `array_size` accepted.
                CaseLabel::Value(e) => {
                    labels.push(eval_union_label(e, &enum_vals).ok_or_else(|| {
                        IdlLuaError::Unsupported(format!(
                            "non-integer union label in `{}`",
                            u.name.text
                        ))
                    })?);
                }
            }
        }
        cases.push(LuaCase {
            labels,
            is_default,
            put,
            get,
        });
    }

    let ty = escape_lua_ident(&qualify(scope, &u.name.text));
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "", &u.annotations, PlacementKind::BeginDeclaration);
    // The `if`/`elseif` label condition for a case (`v.disc == L or v.disc == M`).
    let cond_of = |c: &LuaCase, i: usize| -> Option<String> {
        if c.is_default {
            None
        } else {
            let kw = if i == 0 { "if" } else { "elseif" };
            let cond = c
                .labels
                .iter()
                .map(|&l| format!("v.disc == {}", render_label(l)))
                .collect::<Vec<_>>()
                .join(" or ");
            Some(format!("{kw} {cond} then"))
        }
    };

    let _ = writeln!(out, "\nfunction marshalInto_{ty}(w, v)");
    if ext == ExtensibilityKind::Mutable {
        // #A16: @mutable union — DHEADER-framed member list. The discriminator is
        // member id 0, each branch its 1-based id; every member is an EMHEADER
        // (LC4, must-understand bit 31 unset) + NEXTINT + body (XTypes §7.4.3.5.4).
        let _ = writeln!(out, "  local body = Writer.new(w.endian)");
        emit_mutable_member(out, "  ", "body", 0, false, &disc_put);
        for (i, c) in cases.iter().enumerate() {
            match cond_of(c, i) {
                Some(cond) => {
                    let _ = writeln!(out, "  {cond}");
                }
                None => {
                    let _ = writeln!(out, "  else");
                }
            }
            let id = u32::try_from(i + 1).unwrap_or(0);
            emit_mutable_member(out, "    ", "body", id, false, &c.put);
        }
        let _ = writeln!(out, "  end");
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
        let _ = writeln!(out, "  {}", disc_put.replace("$w", wv));
        for (i, c) in cases.iter().enumerate() {
            match cond_of(c, i) {
                Some(cond) => {
                    let _ = writeln!(out, "  {cond}");
                }
                None => {
                    let _ = writeln!(out, "  else");
                }
            }
            let _ = writeln!(out, "    {}", c.put.replace("$w", wv));
        }
        let _ = writeln!(out, "  end");
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

    // Decode: read the discriminator, then read only the selected member
    // (@appendable skips the leading DHEADER; @mutable reads each member's
    // EMHEADER + NEXTINT positionally). Unread members stay nil.
    let _ = writeln!(out, "\nfunction read_{ty}(r)");
    let _ = writeln!(out, "  local v = {{}}");
    let mutable = ext == ExtensibilityKind::Mutable;
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "  r:getU32()");
    }
    if mutable {
        // Positional @mutable decode: a fully-present union round-trips (matches
        // the naive @mutable struct decoder above).
        emit_mutable_member_decode(out, "  ", &disc_get.replace("$r", "r"));
    } else {
        let _ = writeln!(out, "  {}", disc_get.replace("$r", "r"));
    }
    for (i, c) in cases.iter().enumerate() {
        let indent = match cond_of(c, i) {
            Some(cond) => {
                let _ = writeln!(out, "  {cond}");
                "    "
            }
            None => {
                let _ = writeln!(out, "  else");
                "    "
            }
        };
        if mutable {
            emit_mutable_member_decode(out, indent, &c.get.replace("$r", "r"));
        } else {
            let _ = writeln!(out, "{indent}{}", c.get.replace("$r", "r"));
        }
    }
    if !cases.is_empty() {
        let _ = writeln!(out, "  end");
    }
    let _ = writeln!(out, "  return v");
    let _ = writeln!(out, "end");
    let _ = writeln!(out, "\nfunction unmarshal_{ty}(buf, endian)");
    let _ = writeln!(out, "  return read_{ty}(Reader.new(buf, endian))");
    let _ = writeln!(out, "end");
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "", &u.annotations, PlacementKind::EndDeclaration);
    Ok(())
}

/// Maps an IDL type to a put statement using `$w` as the writer placeholder.
/// A type is "primitive" for the map-DHEADER rule if it is fully descriptive on
/// the wire: an IDL primitive, an enum (i32), or a bitset/bitmask (backing
/// int). Others force a collection DHEADER.
fn is_primitive(t: &TypeSpec, enum_names: &HashSet<String>) -> bool {
    match t {
        TypeSpec::Primitive(_) => true,
        TypeSpec::Scoped(sn) => {
            let n = resolve_scoped_name(sn);
            enum_names.contains(&n) || is_bit_name(&n)
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
        TypeSpec::Sequence(seq) => map_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            expr,
            enum_names,
            struct_names,
        ),
        // A `fixed<P,S>` decimal: packed BCD, `(P+2)/2` raw octets, no length
        // prefix and no alignment (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5). The
        // Lua field holds the BCD byte string directly; `zdFixedEnc` builds it
        // from a decimal string.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let _ = fixed_ps(f)?; // validate P/S resolve at codegen time
            Ok(format!("$w:putBytes({expr})"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Enum holder width follows @bit_bound (XTypes 1.3 §7.4.5.1);
                // `bit_storage` gives the put method + mask for 1/2/4 octets.
                let (put, _get, mask) = bit_storage((enum_wire_width(&name) * 8) as usize);
                Ok(format!("$w:{put}({expr} & {mask})"))
            } else if struct_names.contains(&name) || is_bit_name(&name) {
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
            Ok(build_map_put(
                expr,
                &key_put,
                &val_put,
                prim,
                m.bound.as_ref(),
            ))
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
        let name = resolve_scoped_name(sn);
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    enum_names: &HashSet<String>,
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
    let check = match bound.and_then(array_size) {
        Some(n) => format!(
            "if #{expr} > {n} then error(string.format(\"bounded sequence length exceeds its IDL bound (%d)\", {n})) end; "
        ),
        None => String::new(),
    };
    // sequence<struct> → collection DHEADER + count + each element. This
    // custom inline block does not go through a shared Writer method, so the
    // bound check (XTypes 1.3 §7.4.3) is inlined ahead of it directly.
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let name = escape_lua_ident(&name);
            let put = format!(
                "{check}do local sub = Writer.new($w.endian); sub:putU32(#{expr});                  for _, e in ipairs({expr}) do marshalInto_{name}(sub, e) end;                  local bb = sub:bytes(); $w:putU32(#bb); $w:putBytes(bb) end"
            );
            return Ok(put);
        }
    }
    // sequence<arbitrary> → u32 count + per-element encode (no collection
    // DHEADER; the element type is fully descriptive on the wire for the
    // primitive / enum / bitset / bitmask cases handled here). Mirrors the
    // `idl-go`/`idl-d` fallback.
    let elem_put = map_type(elem, "zdElem", enum_names, struct_names)?;
    Ok(format!(
        "{check}do $w:putU32(#{expr}); for _, zdElem in ipairs({expr}) do {elem_put} end end"
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
        TypeSpec::Sequence(seq) => map_get_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            target,
            enum_names,
            struct_names,
        ),
        // `fixed<P,S>`: read the statically-known `(P+2)/2` BCD octets as a
        // byte string (no length prefix, no alignment).
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let (p, _) = fixed_ps(f)?;
            let n = (p + 2) / 2;
            Ok(format!("{target} = $r:getBytesN({n})"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Read the @bit_bound-wide holder (XTypes 1.3 §7.4.5.1).
                let (_put, get, _mask) = bit_storage((enum_wire_width(&name) * 8) as usize);
                Ok(format!("{target} = $r:{get}()"))
            } else if struct_names.contains(&name) || is_bit_name(&name) {
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
        // B1 follow-up (#22 decode-side parity): pass the IDL bound N to the
        // shared Reader method.
        return match bound.and_then(array_size) {
            Some(n) => Ok(format!("{target} = $r:getSeqU8({n})")),
            None => Ok(format!("{target} = $r:getSeqU8()")),
        };
    }
    // Inline check right after the count is read — these custom blocks have no
    // shared Reader method to carry the check.
    let check = match bound.and_then(array_size) {
        Some(n) => format!(
            "\n  if zdN > {n} then error(string.format(\"decoded sequence length exceeds its IDL bound (%d)\", {n})) end"
        ),
        None => String::new(),
    };
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let name = escape_lua_ident(&name);
            return Ok(format!(
                "do\n  $r:getU32()\n  local zdN = $r:getU32(){check}\n  {target} = {{}}\n  for zdI = 1, zdN do {target}[zdI] = read_{name}($r) end\nend"
            ));
        }
    }
    // sequence<arbitrary> → u32 count + per-element decode, no collection
    // DHEADER (inverse of the arbitrary encode path in `map_sequence`).
    let elem_get = map_get(elem, &format!("{target}[zdI]"), enum_names, struct_names)?;
    Ok(format!(
        "do\n  local zdN = $r:getU32(){check}\n  {target} = {{}}\n  for zdI = 1, zdN do {elem_get} end\nend"
    ))
}
