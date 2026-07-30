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
    Annotation, BinaryOp, BitmaskDecl, BitsetDecl, CaseLabel, ConstDecl, ConstExpr, ConstType,
    ConstrTypeDecl, Declarator, Definition, EnumDef, Export, FixedPtType, FloatingType,
    IntegerType, InterfaceDcl, Literal, LiteralKind, Member, PrimitiveType, ScopedName,
    SequenceType, Specification, StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnaryOp,
    UnionDcl, UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, PlacementKind, enum_bit_bound, enum_wire_octets,
    lower_annotations, lower_single,
};

use crate::error::{IdlNimError, Result};
use crate::keywords::{escape_nim_ident, nim_identifiers_equal};

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
    /// reference to one of these maps to a Nim holder `object` whose wire form
    /// is a single backing integer (`marshalInto`/`read<name>`) — no collection
    /// DHEADER, so it is treated as fully-descriptive (primitive) by the
    /// sequence/map framing rules (XTypes 1.3 §7.4.7).
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

/// Nim codegen language aliases matched by `@verbatim(language="...")`
/// (case-insensitive; the spec wildcard `"*"` always matches — see
/// [`Lowered::verbatims_for_language`]).
const NIM_LANG_ALIASES: &[&str] = &["nim", "nimlang"];

/// `true` if `name` resolves to a `bitset`/`bitmask` declaration (its wire form
/// is a single backing integer — fully descriptive, no collection DHEADER).
fn is_bit_name(name: &str) -> bool {
    BIT_NAMES.with(|b| b.borrow().contains(name))
}

/// Emits every `@verbatim` block from `anns` whose language matches the Nim
/// codegen and whose `placement` equals `placement`, each line prefixed with
/// `indent`. Source order preserved; text spliced unmodified (no wire impact —
/// XTypes 1.3 §7.2.2.4.8 / IDL 4.2 §8.3.5.1). Mirrors `idl-d`/`idl-rust`.
fn emit_verbatim_at(out: &mut String, indent: &str, anns: &[Annotation], placement: PlacementKind) {
    let Ok(lowered) = lower_annotations(anns) else {
        return;
    };
    for v in lowered.verbatims_for_language(NIM_LANG_ALIASES) {
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
/// `scope.join("_") + "_" + simple`, or the bare `simple` at global scope (so
/// every existing top-level golden is unchanged). Two same-simple-name types in
/// different modules become distinct types `a_Reading`/`b_Reading` (#21).
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
/// single Nim identifier. Each segment's own underscores are doubled and the
/// segments joined by a single underscore, so `module A_B { struct C }`
/// (`["A_B","C"]` → `A__B_C`) never collides with `module A { module B {
/// struct C }}` (`["A","B","C"]` → `A_B_C`) — the previous `join("_")` mapped
/// both to `A_B_C` (#A35, non-injective flatten). A single (global-scope)
/// segment is returned verbatim so every existing top-level golden is
/// unchanged, and any segment without underscores (the common case) passes
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
            Definition::Type(td) => register_type_decl_path(td, scope),
            // #A39: type declarations nested in an interface body are promoted
            // under the interface's own scope segment, so their reference paths
            // resolve the same way at the definition and use sites.
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

/// Records the flattened path(s) of a single (module- or interface-nested)
/// `TypeDecl`.
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

/// BCD codec for `fixed<P,S>`. Appended once when any `fixed` member is emitted.
/// Builds the packed-BCD octet sequence (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5)
/// from a decimal string: an optional leading pad nibble (so the nibble count
/// is even), `P` digit nibbles most-significant first, then the sign nibble
/// (`0xC` positive, `0xD` negative). Byte count `(P+2)/2`, no length prefix.
const FIXED_PRELUDE: &str = r#"
proc zdFixedEnc*(s: string, P: int, S: int): seq[byte] =
  var sign = true
  var i = 0
  if s.len > 0 and (s[0] == '-' or s[0] == '+'):
    sign = s[0] != '-'
    i = 1
  let rest = s[i .. ^1]
  var dot = rest.len
  for k in 0 ..< rest.len:
    if rest[k] == '.':
      dot = k
      break
  let ip = rest[0 ..< dot]
  let fp = if dot < rest.len: rest[dot + 1 .. ^1] else: ""
  var db = ""
  let intNeeded = P - S
  if ip.len < intNeeded:
    for zj in ip.len ..< intNeeded: db.add('0')
  db.add(ip)
  db.add(fp)
  if fp.len < S:
    for zj in fp.len ..< S: db.add('0')
  var nib: seq[byte] = @[]
  if (P + 1) mod 2 == 1: nib.add(0'u8)
  for c in db: nib.add(byte(ord(c) - ord('0')))
  nib.add(byte(if sign: 0x0C else: 0x0D))
  result = @[]
  var k = 0
  while k < nib.len:
    result.add(byte((nib[k] shl 4) or nib[k + 1]))
    k += 2
"#;

/// Generates a self-contained Nim module from the IDL AST.
///
/// # Errors
/// Returns [`IdlNimError::Unsupported`] for constructs the Nim backend does not
/// yet emit (e.g. `@mutable` unions and non-literal array/sequence bounds).
pub fn generate_nim_module(spec: &Specification, _opts: &NimGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Code generated by zerodds-idlc (Nim backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "# SPDX-License-Identifier: Apache-2.0\n");
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
    // #A39: type declarations nested in an interface body, promoted to the top
    // level under the interface's own scope segment, so their DDS data types
    // survive instead of being silently dropped with the interface body.
    let iface_types = flatten_iface_types(&spec.definitions);

    // Every named type decl (module-level + interface-nested), for the
    // reference-resolution name sets below.
    let all_type_decls = || {
        flat.iter()
            .filter_map(|(s, d)| match d {
                Definition::Type(td) => Some((s, td)),
                _ => None,
            })
            .chain(iface_types.iter().map(|(s, td)| (s, *td)))
    };

    // Named enums/structs keyed by their flattened module-qualified name. An
    // enum member is a 32-bit signed integer on the wire (XTypes 1.3 §7.4.5.1).
    // `enum_defs` additionally keeps the def so an enum-discriminated union can
    // resolve `case ENUMERATOR:` labels (#P4/A11).
    let mut enum_names: HashSet<String> = HashSet::new();
    let mut struct_names: HashSet<String> = HashSet::new();
    let mut bit_names: HashSet<String> = HashSet::new();
    let mut enum_defs: HashMap<String, &EnumDef> = HashMap::new();
    // Qualified-name -> StructDef, so a nested-struct `@key` member's own `@key`
    // subset (and `keyhash::uses_md5`'s static max-size analysis), plus a
    // `struct D : Base` base's members (#A10), can be resolved.
    let mut structs: HashMap<String, &StructDef> = HashMap::new();
    for (scope, td) in all_type_decls() {
        match td {
            TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => {
                let n = qualify(scope, &e.name.text);
                enum_defs.insert(n.clone(), e);
                enum_names.insert(n);
            }
            TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
                let n = qualify(scope, &s.name.text);
                struct_names.insert(n.clone());
                structs.insert(n, s);
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

    // typedef qualified-name → aliased type-spec (wire-transparent; resolved
    // before mapping). Interface-nested typedefs are folded in too (#A39).
    let mut typedefs = collect_typedefs(spec);
    for (scope, td) in &iface_types {
        if let TypeDecl::Typedef(tdd) = td {
            for d in &tdd.declarators {
                if let Declarator::Simple(name) = d {
                    typedefs.insert(qualify(scope, &name.text), tdd.type_spec.clone());
                }
            }
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
            // catch-all arm; emit it as a Nim module-level constant.
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
    // The BCD codec prelude is appended once if any `fixed<P,S>` was emitted.
    if USED_FIXED.with(std::cell::Cell::get) {
        out.push_str(FIXED_PRELUDE);
    }
    Ok(out)
}

/// Emits a single `TypeDecl` (module-level or interface-nested) into `out`.
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

/// Recursively descends into `Definition::Interface` bodies, returning every
/// interface-nested `Export::Type` declaration paired with the scope path
/// `enclosing_module… + interface_name` (#A39). Nim has no interface / nested-
/// type construct, so these are promoted to the top level under the interface's
/// own name segment (two interfaces in one module therefore do not collide).
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

/// Emits an IDL `const` as a Nim module-level constant (#A5/P1 — a top-level
/// const was silently dropped by the former catch-all arm). The name is
/// module-qualified and keyword-escaped; the value is skipped only when it
/// references another (scoped) const the Nim backend cannot re-express.
fn emit_const(out: &mut String, c: &ConstDecl, scope: &[String]) {
    let Some(val) = const_expr_to_nim(&c.value) else {
        return;
    };
    let name = escape_nim_ident(&qualify(scope, &c.name.text));
    match const_nim_type(&c.type_) {
        Some(ty) => {
            let _ = writeln!(out, "\nconst {name}*: {ty} = {val}");
        }
        None => {
            let _ = writeln!(out, "\nconst {name}* = {val}");
        }
    }
}

/// Nim type for a `const` declaration (`None` = emit an untyped constant so Nim
/// infers it — used for `char`/`wchar`/`fixed`/scoped values).
fn const_nim_type(ct: &ConstType) -> Option<&'static str> {
    Some(match ct {
        ConstType::Integer(i) => nim_int_type(*i),
        ConstType::Floating(FloatingType::Float) => "float32",
        ConstType::Floating(FloatingType::Double | FloatingType::LongDouble) => "float64",
        ConstType::Boolean => "bool",
        ConstType::Octet => "uint8",
        ConstType::String { .. } => "string",
        // `char`/`wchar`/`fixed`/scoped values are left untyped: a char literal
        // infers `char`, a fixed decimal is a `string`, and an enum-valued
        // scoped reference is skipped by `const_expr_to_nim`.
        ConstType::Char | ConstType::WideChar | ConstType::Fixed | ConstType::Scoped(_) => {
            return None;
        }
    })
}

/// The Nim integer type for an IDL integer type (matches `map_integer`).
fn nim_int_type(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Int8 => "int8",
        IntegerType::UInt8 => "uint8",
        IntegerType::Short | IntegerType::Int16 => "int16",
        IntegerType::UShort | IntegerType::UInt16 => "uint16",
        IntegerType::Long | IntegerType::Int32 => "int32",
        IntegerType::ULong | IntegerType::UInt32 => "uint32",
        IntegerType::LongLong | IntegerType::Int64 => "int64",
        IntegerType::ULongLong | IntegerType::UInt64 => "uint64",
    }
}

/// Renders a `ConstExpr` as a Nim constant expression, or `None` for a form the
/// Nim backend does not express (an enum-valued / const-alias scoped reference —
/// a wrong Nim identifier would break the build, and the const is only a codegen
/// convenience).
/// zerodds-lint: recursion-depth 32 (const expression tree; bounded by the IDL
/// grammar's expression nesting).
fn const_expr_to_nim(e: &ConstExpr) -> Option<String> {
    match e {
        ConstExpr::Literal(l) => const_literal_to_nim(l),
        ConstExpr::Scoped(_) => None,
        ConstExpr::Unary { op, operand, .. } => {
            let v = const_expr_to_nim(operand)?;
            let o = match op {
                UnaryOp::Plus => "+",
                UnaryOp::Minus => "-",
                UnaryOp::BitNot => "not ",
            };
            Some(format!("{o}{v}"))
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let l = const_expr_to_nim(lhs)?;
            let r = const_expr_to_nim(rhs)?;
            let o = match op {
                BinaryOp::Or => "or",
                BinaryOp::Xor => "xor",
                BinaryOp::And => "and",
                BinaryOp::Shl => "shl",
                BinaryOp::Shr => "shr",
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "div",
                BinaryOp::Mod => "mod",
            };
            Some(format!("({l} {o} {r})"))
        }
    }
}

/// Renders a single literal as valid Nim source.
fn const_literal_to_nim(l: &Literal) -> Option<String> {
    let raw = l.raw.trim();
    Some(match l.kind {
        // Nim accepts decimal / `0x` / `0o` / `0b` integer literals as-is.
        LiteralKind::Integer => raw.to_string(),
        // Strip a trailing IDL float/fixed suffix (`d`/`f`/`l`) Nim rejects.
        LiteralKind::Floating => raw
            .trim_end_matches(['d', 'D', 'f', 'F', 'l', 'L'])
            .to_string(),
        // A `fixed` decimal has no native Nim type — render as a string.
        LiteralKind::Fixed => format!(
            "\"{}\"",
            raw.trim_end_matches(['d', 'D']).replace('"', "\\\"")
        ),
        // Normalize the IDL boolean keyword to Nim's `true`/`false` (never emit a
        // bare `TRUE`/`FALSE` token, which is not a Nim identifier — #A13).
        LiteralKind::Boolean => {
            if raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // Narrow string/char literals pass through; wide literals drop the `L`
        // prefix (`L"x"`/`L'x'` is not valid Nim — #A7-pattern).
        LiteralKind::String | LiteralKind::Char => raw.to_string(),
        LiteralKind::WideString | LiteralKind::WideChar => {
            raw.strip_prefix('L').unwrap_or(raw).to_string()
        }
    })
}

/// Collects a struct's effective members base-first (#A10/P3): the base struct's
/// members (recursively) precede the derived struct's own, so the generated Nim
/// `object` and its wire form carry the inherited fields — matching cpp/csharp/
/// java. Without this a `struct D : Base` dropped every inherited field from both
/// the type and the wire.
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

/// Evaluates a union case label `e` to its integer discriminant, resolving enum
/// enumerators (via `enum_vals`, name → value of the switch enum), `char` code
/// points, and the `boolean` keywords `TRUE`/`FALSE` (#P4: A11/A12/A13).
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
        ConstExpr::Scoped(sn) => enum_vals.get(&sn.parts.last()?.text).copied(),
        ConstExpr::Unary { op, operand, .. } => {
            let v = eval_union_label(operand, enum_vals)?;
            Some(match op {
                UnaryOp::Plus => v,
                UnaryOp::Minus => -v,
                UnaryOp::BitNot => !v,
            })
        }
        ConstExpr::Binary { .. } => None,
    }
}

/// Evaluates a `char`/`wchar` literal (`'A'`, `L'x'`, `'\n'`, `'\xHH'`) to its
/// code point. Used by the union label evaluator (#A12) so `case 'A':` resolves
/// to the discriminant 65.
fn char_literal_value(raw: &str) -> Option<i64> {
    let s = raw.trim();
    let s = s.strip_prefix('L').unwrap_or(s);
    let inner = s.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut it = inner.chars();
    let c = it.next()?;
    if c == '\\' {
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
            'x' => return i64::from_str_radix(it.as_str(), 16).ok(),
            _ => return None,
        };
        Some(v)
    } else {
        Some(i64::from(u32::from(c)))
    }
}

/// De-duplicates `name` against `used` under Nim's style-insensitive identifier
/// equality (#A37: `my_field` and `myField` are the *same* Nim identifier). The
/// first spelling is kept verbatim (so single-name goldens are unchanged); a
/// later colliding member gets `_2`, `_3`, … appended until distinct. The raw
/// (pre-escape) spelling is tracked, since escaping only wraps keywords.
fn dedup_nim_ident(used: &mut Vec<String>, name: &str) -> String {
    let collides =
        |cand: &str, used: &[String]| used.iter().any(|u| nim_identifiers_equal(u, cand));
    if !collides(name, used) {
        used.push(name.to_string());
        return name.to_string();
    }
    let mut i = 2;
    loop {
        let cand = format!("{name}_{i}");
        if !collides(&cand, used) {
            used.push(cand.clone());
            return cand;
        }
        i += 1;
    }
}

/// Writes one `@mutable` member into writer `wv`: its EMHEADER (must-understand
/// bit 31 when `mu` — #A17) then NEXTINT (body length, LC4 per the coordinated
/// wire baseline — see the A19 scope note) then the member body. `put` uses the
/// `$w` placeholder; every line is emitted at `base` + two spaces.
fn write_mutable_member_encode(
    out: &mut String,
    base: &str,
    wv: &str,
    id: u32,
    mu: bool,
    put: &str,
) {
    let mu_bit = if mu { 0x8000_0000_u32 } else { 0 };
    let emh = mu_bit | 0x4000_0000 | (id & 0x0FFF_FFFF);
    let _ = writeln!(out, "{base}  {wv}.putU32(uint32(0x{emh:08x}))");
    let _ = writeln!(out, "{base}  block:");
    let _ = writeln!(out, "{base}    var mem = initWriter({wv}.endian)");
    for line in put.replace("$w", "mem").lines() {
        let _ = writeln!(out, "{base}    {line}");
    }
    let _ = writeln!(out, "{base}    {wv}.putU32(uint32(mem.bytes().len))");
    let _ = writeln!(out, "{base}    {wv}.putBytes(mem.bytes())");
}

/// Reads one `@mutable` member (its EMHEADER + NEXTINT, then the value via `get`,
/// `$r` placeholder) — the positional inverse of [`write_mutable_member_encode`].
fn write_mutable_member_decode(out: &mut String, base: &str, get: &str) {
    let _ = writeln!(out, "{base}discard r.getU32()");
    let _ = writeln!(out, "{base}discard r.getU32()");
    for line in get.replace("$r", "r").lines() {
        let _ = writeln!(out, "{base}{line}");
    }
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
fn emit_enum(out: &mut String, e: &EnumDef, scope: &[String]) {
    let values = enumerator_values(e);
    // `raw_ty` prefixes every enumerator (`{raw_ty}{enumerator}` is always a
    // single fused identifier, never a standalone keyword token, so it is
    // never escaped); the `type` declaration itself is a standalone
    // identifier and needs the escaped form. Module-qualified (#21).
    let raw_ty = qualify(scope, &e.name.text);
    let ty = escape_nim_ident(&raw_ty);
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

/// Backing-integer storage for a bit container of `total_bits` bits: XTypes 1.3
/// §7.4.7 — the smallest holder that fits (`≤8`→uint8, `≤16`→uint16, `≤32`→
/// uint32, else uint64). Returns `(Nim type, marshal-put statement referencing
/// `self.storage`, reader expression reading from `r`)`.
fn bit_storage(total_bits: usize) -> (&'static str, &'static str, &'static str) {
    match total_bits {
        0..=8 => ("uint8", "w.putU8(int(self.storage))", "uint8(r.getU8())"),
        9..=16 => (
            "uint16",
            "w.putU16(int(self.storage))",
            "uint16(r.getU16())",
        ),
        17..=32 => ("uint32", "w.putU32(self.storage)", "r.getU32()"),
        _ => ("uint64", "w.putU64(self.storage)", "r.getU64()"),
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

/// Emits an IDL `bitset` as a Nim holder `object` over its backing integer, a
/// bit-accessor pair per named bitfield, and an XCDR2 marshal/unmarshal that
/// writes the backing integer (XTypes 1.3 §7.4.7 — wire = backing int).
///
/// # Errors
/// [`IdlNimError::Unsupported`] if a bitfield width is not a codegen-time integer.
fn emit_bitset(out: &mut String, b: &BitsetDecl, scope: &[String]) -> Result<()> {
    let mut widths: Vec<usize> = Vec::with_capacity(b.bitfields.len());
    for bf in &b.bitfields {
        let w = array_size(&bf.spec.width)
            .filter(|w| *w >= 0)
            .ok_or_else(|| {
                IdlNimError::Unsupported(format!(
                    "non-integer bitfield width in bitset {}",
                    b.name.text
                ))
            })? as usize;
        widths.push(w);
    }
    let total: usize = widths.iter().sum();
    let (storage, put, get) = bit_storage(total);
    let raw_ty = qualify(scope, &b.name.text);
    let ty = escape_nim_ident(&raw_ty);

    let _ = writeln!(out, "\ntype {ty}* = object");
    let _ = writeln!(out, "  storage*: {storage}");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "", &b.annotations, PlacementKind::BeginDeclaration);
    let mut offset: usize = 0;
    for (bf, width) in b.bitfields.iter().zip(&widths) {
        if let Some(name) = &bf.name {
            let field = escape_nim_ident(&name.text);
            if *width == 1 {
                let _ = writeln!(
                    out,
                    "\nproc {field}*(self: {ty}): bool = ((self.storage shr {offset}) and 1) != 0"
                );
                let _ = writeln!(out, "proc `set_{field}`*(self: var {ty}, v: bool) =");
                let _ = writeln!(out, "  let m = {storage}(1) shl {offset}");
                let _ = writeln!(out, "  if v: self.storage = self.storage or m");
                let _ = writeln!(out, "  else: self.storage = self.storage and not m");
            } else {
                let mask: u128 = if *width >= 128 {
                    u128::MAX
                } else {
                    (1u128 << *width) - 1
                };
                let _ = writeln!(
                    out,
                    "\nproc {field}*(self: {ty}): {storage} = {storage}((self.storage shr {offset}) and {storage}({mask}))"
                );
                let _ = writeln!(out, "proc `set_{field}`*(self: var {ty}, v: {storage}) =");
                let _ = writeln!(out, "  let m = {storage}({mask}) shl {offset}");
                let _ = writeln!(
                    out,
                    "  self.storage = (self.storage and not m) or ((v and {storage}({mask})) shl {offset})"
                );
            }
        }
        offset += width;
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "\nproc marshalInto*(self: {ty}, w: var Writer) =");
    let _ = writeln!(out, "  {put}");
    let _ = writeln!(
        out,
        "\nproc marshalXCDR*(self: {ty}, endian: Endian): seq[byte] ="
    );
    let _ = writeln!(out, "  var w = initWriter(endian)");
    let _ = writeln!(out, "  self.marshalInto(w)");
    let _ = writeln!(out, "  w.bytes()");
    let _ = writeln!(out, "\nproc read{raw_ty}(r: var Reader): {ty} =");
    let _ = writeln!(out, "  result.storage = {get}");
    let _ = writeln!(
        out,
        "\nproc unmarshalXCDR{raw_ty}*(buf: seq[byte], endian: Endian): {ty} ="
    );
    let _ = writeln!(out, "  var r = initReader(buf, endian)");
    let _ = writeln!(out, "  read{raw_ty}(r)");
    Ok(())
}

/// Emits an IDL `bitmask` as a Nim holder `object` over its `@bit_bound` backing
/// integer (default 32), an OR-able manifest `const` per bit value, and an
/// XCDR2 marshal/unmarshal writing the backing integer (XTypes 1.3 §7.4.7).
fn emit_bitmask(out: &mut String, b: &BitmaskDecl, scope: &[String]) {
    let (storage, put, get) = bit_storage(bitmask_bit_bound(&b.annotations) as usize);
    let raw_ty = qualify(scope, &b.name.text);
    let ty = escape_nim_ident(&raw_ty);

    let _ = writeln!(out, "\ntype {ty}* = object");
    let _ = writeln!(out, "  storage*: {storage}");
    emit_verbatim_at(out, "", &b.annotations, PlacementKind::BeginDeclaration);
    // Manifest constants (`{raw_ty}{VALUE}`), single fused identifiers like the
    // enum emitter's `{raw_ty}{enumerator}`, so no keyword escaping is needed.
    for (idx, v) in b.values.iter().enumerate() {
        let pos = bit_position(&v.annotations).unwrap_or(idx as u32);
        let _ = writeln!(
            out,
            "const {raw_ty}{}*: {storage} = {storage}(1) shl {pos}",
            v.name.text
        );
    }
    emit_verbatim_at(out, "", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "\nproc marshalInto*(self: {ty}, w: var Writer) =");
    let _ = writeln!(out, "  {put}");
    let _ = writeln!(
        out,
        "\nproc marshalXCDR*(self: {ty}, endian: Endian): seq[byte] ="
    );
    let _ = writeln!(out, "  var w = initWriter(endian)");
    let _ = writeln!(out, "  self.marshalInto(w)");
    let _ = writeln!(out, "  w.bytes()");
    let _ = writeln!(out, "\nproc read{raw_ty}(r: var Reader): {ty} =");
    let _ = writeln!(out, "  result.storage = {get}");
    let _ = writeln!(
        out,
        "\nproc unmarshalXCDR{raw_ty}*(buf: seq[byte], endian: Endian): {ty} ="
    );
    let _ = writeln!(out, "  var r = initReader(buf, endian)");
    let _ = writeln!(out, "  read{raw_ty}(r)");
}

/// Resolves a `fixed<P,S>`'s digit count `P` and scale `S` to codegen-time
/// integers.
///
/// # Errors
/// [`IdlNimError::Unsupported`] if either is not a resolvable non-negative
/// integer literal.
fn fixed_ps(f: &FixedPtType) -> Result<(i64, i64)> {
    let p = array_size(&f.digits)
        .filter(|v| *v > 0)
        .ok_or_else(|| IdlNimError::Unsupported("non-integer fixed digit count".to_string()))?;
    let s = array_size(&f.scale)
        .filter(|v| *v >= 0)
        .ok_or_else(|| IdlNimError::Unsupported("non-integer fixed scale".to_string()))?;
    Ok((p, s))
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
/// Evaluates an IDL-declared bound (`string<N>` / `sequence<T,N>` /
/// `map<K,V,N>`) to its integer value. B1 follow-up (#22 decode-side
/// parity): shares `array_size`'s literal/unary evaluation — an IDL bound is
/// syntactically the same const-expr shape as an array size.
fn bound_value(e: &ConstExpr) -> Option<i64> {
    array_size(e)
}

/// zerodds-lint: recursion-depth 16 (const-expr operand walk; bounded by IDL nesting).
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

/// A generated union case: rendered Nim case-branch labels (empty + is_default =
/// `default`), the member field name, its language type, and the per-member
/// put/get statements.
struct UnionCase {
    labels: Vec<String>,
    is_default: bool,
    field: String,
    ty: String,
    put: String,
    get: String,
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
        // `@optional`: a companion uint8 presence flag precedes the value on
        // the wire (XTypes 1.3 §7.4.5.1.4).
        optional: bool,
        // `@must_understand`: sets EMHEADER bit 31 in the `@mutable` framing
        // (#A17). Wire-neutral for `@final`/`@appendable`.
        must_understand: bool,
        // `@non_serialized`: kept in the Nim object, off every wire form.
        non_serialized: bool,
    }
    // #A10/P3: base-first effective member list — inherited members precede the
    // struct's own, so both the Nim `object` and its wire carry them.
    let mut all_members: Vec<&Member> = Vec::new();
    collect_base_members(s, structs, &mut all_members);
    let mut fields: Vec<FieldGen> = Vec::new();
    // #A37: Nim identifiers are style-insensitive; keep member field names
    // distinct under that equality so `my_field`/`myField` do not redefine.
    let mut used_names: Vec<String> = Vec::new();
    // Container-level `@autoid(HASH)` (XTypes 1.3 §7.3.1.2.1.1). When set, every
    // member with no explicit `@id`/`@hashid` takes a name-hashed member id
    // instead of a sequential one. Resolved through the shared frontend so the
    // ids match idl-rust/idl-cpp and the TypeObject (findings A31/A32).
    let container_hash = zerodds_idl::semantics::member_id::container_autoid_hash(&s.annotations);
    // Sequential fallback counter (`@autoid(SEQUENTIAL)`): advances ONLY for
    // members that take the positional default — an explicit `@id`, `@hashid`,
    // or `@autoid(HASH)` id does not consume a slot, matching the canonical
    // `resolve_member_ids` in `zerodds-types`.
    let mut next_seq: u32 = 0;
    for m in &all_members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
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
        // P0-5 (#2): a `@non_serialized` member keeps its Nim field but is off
        // the wire and does NOT consume a sequential id slot (ids compact).
        let non_serialized =
            zerodds_idl::semantics::annotations::member_is_non_serialized(&m.annotations);
        for d in &m.declarators {
            // Raw IDL member name (never the escaped Nim identifier): the wire
            // member-id hash is over the source spelling (XTypes §7.3.1.2.1.4).
            let raw_name = d.name().text.clone();
            let nim_name = escape_nim_ident(&dedup_nim_ident(&mut used_names, &raw_name));
            let id = if non_serialized {
                0
            } else {
                match zerodds_idl::semantics::member_id::fixed_member_id(
                    container_hash,
                    &m.annotations,
                    &raw_name,
                ) {
                    Some(fixed) => fixed,
                    None => {
                        let seq = next_seq;
                        next_seq += 1;
                        seq
                    }
                }
            };
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
                optional,
                must_understand,
                non_serialized,
            });
        }
    }

    // `ty` (raw) feeds composite proc names (`read{ty}`, `unmarshalXCDR{ty}`) —
    // concatenation never collides with a standalone keyword token, so those
    // stay raw. `ety` (escaped) is used everywhere `ty` appears as a
    // standalone type annotation.
    let ty = qualify(scope, &s.name.text);
    let ety = escape_nim_ident(&ty);
    let _ = writeln!(out, "\ntype {ety}* = object");
    for f in &fields {
        // An `@optional` member carries a companion presence flag (XTypes 1.3
        // §7.4.5.1.4: uint8 present-flag then the value if present).
        if f.optional {
            let _ = writeln!(out, "  {}_present*: bool", f.nim_name);
        }
        let _ = writeln!(out, "  {}*: {}", f.nim_name, f.nim_type);
    }
    // §7.2.2.4.8 — text as the first element inside the declaration (emitted at
    // top level between the object type and its procs; a nim `object` body
    // admits only fields, so declaration-scoped verbatim rides here).
    emit_verbatim_at(out, "", &s.annotations, PlacementKind::BeginDeclaration);

    // marshalInto writes into an existing writer (nested composites call this so
    // alignment stays stream-relative). @final: fields inline; @appendable:
    // a DHEADER-framed body.
    let _ = writeln!(out, "\nproc marshalInto*(self: {ety}, w: var Writer) =");
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER
        // (must-understand bit 31 when @must_understand — #A17; LC=4 = body
        // length per the coordinated wire baseline, see the A19 scope note) +
        // NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "  var body = initWriter(w.endian)");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            // An `@optional` member is omitted from the member list when absent
            // (XTypes 1.3 §7.4.3.4.2): guard its EMHEADER+body on the flag. The
            // decode side below rides the naive per-member decoder (it does not
            // reconstruct absence), so mutable-optional decode is not claimed
            // complete — only encode honors the presence flag here.
            let base = if f.optional {
                let _ = writeln!(out, "  if self.{}_present:", f.nim_name);
                "  "
            } else {
                ""
            };
            write_mutable_member_encode(out, base, "body", f.id, f.must_understand, &f.put);
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
            if f.non_serialized {
                continue;
            }
            if f.optional {
                // uint8 presence flag then the value if present (§7.4.5.1.4).
                let _ = writeln!(
                    out,
                    "  {writer_var}.putU8(if self.{name}_present: 1 else: 0)",
                    name = f.nim_name
                );
                let _ = writeln!(out, "  if self.{}_present:", f.nim_name);
                for line in f.put.replace("$w", writer_var).lines() {
                    let _ = writeln!(out, "    {line}");
                }
            } else {
                for line in f.put.replace("$w", writer_var).lines() {
                    let _ = writeln!(out, "  {line}");
                }
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
    let mut zdkeys: Vec<&FieldGen> = fields
        .iter()
        .filter(|f| f.key && !f.non_serialized)
        .collect();
    zdkeys.sort_by_key(|f| f.id);
    if !zdkeys.is_empty() {
        // #A10: key detection spans the base-first effective member list.
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
            if f.non_serialized {
                continue;
            }
            write_mutable_member_decode(out, "  ", &f.get);
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "  discard r.getU32()");
        }
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            if f.optional {
                // uint8 presence flag then the value only if present (§7.4.5.1.4).
                let _ = writeln!(out, "  result.{}_present = r.getBool()", f.nim_name);
                let _ = writeln!(out, "  if result.{}_present:", f.nim_name);
                for line in f.get.replace("$r", "r").lines() {
                    let _ = writeln!(out, "    {line}");
                }
            } else {
                for line in f.get.replace("$r", "r").lines() {
                    let _ = writeln!(out, "  {line}");
                }
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
    // §7.2.2.4.8 — text as the last element inside the declaration (emitted at
    // top level trailing the type's procs).
    emit_verbatim_at(out, "", &s.annotations, PlacementKind::EndDeclaration);
    Ok(())
}

/// Emits an IDL `union` as a Nim object holding the discriminator + one field
/// per case member, plus a `marshalInto` that puts the discriminator then a
/// `case` dispatches to the selected member (XCDR2 §7.4.3.5.4).
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
    let disc_ts = switch_typespec(&u.switch_type);
    let (disc_type, disc_put) = map_type(&disc_ts, "self.disc", enum_names, struct_names)?;
    let disc_get = map_get(&disc_ts, "result.disc", enum_names, struct_names)?;

    // #P4 (A11): when the discriminator is an enum, resolve `case ENUMERATOR:`
    // labels — `enum_vals` maps an enumerator name → its wire value, and
    // `val_to_ident` maps a wire value → the flattened Nim enumerator identifier
    // (`{enum}{ENUMERATOR}`, matching `emit_enum`).
    let (enum_vals, val_to_ident): (HashMap<String, i64>, HashMap<i64, String>) =
        match &u.switch_type {
            SwitchTypeSpec::Scoped(sn) => enum_defs
                .get(&resolve_scoped_name(sn))
                .map(|e| {
                    let raw_ty = resolve_scoped_name(sn);
                    let mut names = HashMap::new();
                    let mut idents = HashMap::new();
                    for (en, v) in e.enumerators.iter().zip(enumerator_values(e)) {
                        names.insert(en.name.text.clone(), i64::from(v));
                        idents.insert(i64::from(v), format!("{raw_ty}{}", en.name.text));
                    }
                    (names, idents)
                })
                .unwrap_or_default(),
            _ => (HashMap::new(), HashMap::new()),
        };

    // Renders one label's evaluated integer as a Nim case-branch expression,
    // matching the discriminator's Nim type (#P4: A11 enum / A12 char / A13 bool).
    let render = |v: i64| -> Option<String> {
        match &u.switch_type {
            SwitchTypeSpec::Boolean => Some(if v != 0 {
                "true".into()
            } else {
                "false".into()
            }),
            SwitchTypeSpec::Char => Some(format!("'\\x{:02X}'", v as u8)),
            SwitchTypeSpec::Scoped(_) => val_to_ident.get(&v).cloned(),
            _ => Some(v.to_string()),
        }
    };

    // #A37: union field names are distinct Nim identifiers (the `disc` field is
    // reserved first, then each case member).
    let mut used_names: Vec<String> = vec!["disc".to_string()];
    let mut cases: Vec<UnionCase> = Vec::new();
    // Evaluated discriminant values covered by explicit `case` labels, used to
    // decide whether a Nim `else` branch is required (see `exhaustive` below).
    let mut covered: HashSet<i64> = HashSet::new();
    for c in &u.cases {
        let field = escape_nim_ident(&dedup_nim_ident(
            &mut used_names,
            &c.element.declarator.name().text,
        ));
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
                CaseLabel::Value(e) => {
                    let v = eval_union_label(e, &enum_vals).ok_or_else(|| {
                        IdlNimError::Unsupported(format!(
                            "non-evaluable union label in `{}`",
                            u.name.text
                        ))
                    })?;
                    covered.insert(v);
                    labels.push(render(v).ok_or_else(|| {
                        IdlNimError::Unsupported(format!(
                            "union label {v} has no enumerator in `{}`",
                            u.name.text
                        ))
                    })?);
                }
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
    // A Nim `case` over an ordinal type rejects an `else` branch once every
    // possible value is already covered. Only a fully-enumerated `boolean`
    // (both `true` and `false`) or enum (all enumerators) switch can reach that
    // — an integer/char switch never does. Emit the fallback `else: discard`
    // exactly when the dispatch is neither closed by a `default` case nor
    // exhaustive, so both a missing-arm and a redundant-else error are avoided.
    let exhaustive = match &u.switch_type {
        SwitchTypeSpec::Boolean => covered.contains(&0) && covered.contains(&1),
        SwitchTypeSpec::Scoped(_) => {
            !val_to_ident.is_empty() && val_to_ident.keys().all(|v| covered.contains(v))
        }
        _ => false,
    };
    let need_else = !has_default && !exhaustive;

    // See the analogous split in `emit_struct`: `ty` (raw) feeds composite
    // proc names, `ety` (escaped) is used as a standalone type annotation.
    let ty = qualify(scope, &u.name.text);
    let ety = escape_nim_ident(&ty);
    let _ = writeln!(out, "\ntype {ety}* = object");
    let _ = writeln!(out, "  disc*: {disc_type}");
    for c in &cases {
        let _ = writeln!(out, "  {}*: {}", c.field, c.ty);
    }
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "", &u.annotations, PlacementKind::BeginDeclaration);

    // Renders a case-branch head (`of L1, L2:` or `else:`).
    let branch_head = |out: &mut String, c: &UnionCase| {
        if c.is_default {
            let _ = writeln!(out, "  else:");
        } else {
            let _ = writeln!(out, "  of {}:", c.labels.join(", "));
        }
    };

    let _ = writeln!(out, "\nproc marshalInto*(self: {ety}, w: var Writer) =");
    if ext == ExtensibilityKind::Mutable {
        // #A16: @mutable union — DHEADER-framed member list. The discriminator
        // is member id 0, each branch its 1-based id, wrapped in the struct's
        // DHEADER (XTypes §7.4.3.4.2 / §7.4.3.5.4).
        let _ = writeln!(out, "  var body = initWriter(w.endian)");
        write_mutable_member_encode(out, "", "body", 0, false, &disc_put);
        let _ = writeln!(out, "  case self.disc");
        for (i, c) in cases.iter().enumerate() {
            branch_head(out, c);
            let id = u32::try_from(i + 1).unwrap_or(0);
            write_mutable_member_encode(out, "  ", "body", id, false, &c.put);
        }
        if need_else {
            let _ = writeln!(out, "  else: discard");
        }
        let _ = writeln!(out, "  w.putU32(uint32(body.bytes().len))");
        let _ = writeln!(out, "  w.putBytes(body.bytes())");
    } else {
        let wv = if ext == ExtensibilityKind::Final {
            "w"
        } else {
            let _ = writeln!(out, "  var body = initWriter(w.endian)");
            "body"
        };
        let _ = writeln!(out, "  {}", disc_put.replace("$w", wv));
        let _ = writeln!(out, "  case self.disc");
        for c in &cases {
            branch_head(out, c);
            let _ = writeln!(out, "    {}", c.put.replace("$w", wv));
        }
        if need_else {
            let _ = writeln!(out, "  else: discard");
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

    // Decode: read the discriminator, then dispatch to read the selected member
    // (@appendable skips the leading DHEADER; @mutable skips DHEADER then reads
    // the discriminator EMHEADER + value, positionally). `result` zero-init.
    let _ = writeln!(out, "\nproc read{ty}(r: var Reader): {ety} =");
    if ext == ExtensibilityKind::Mutable {
        let _ = writeln!(out, "  discard r.getU32()");
        write_mutable_member_decode(out, "  ", &disc_get);
        let _ = writeln!(out, "  case result.disc");
        for c in &cases {
            branch_head(out, c);
            write_mutable_member_decode(out, "    ", &c.get);
        }
        if need_else {
            let _ = writeln!(out, "  else: discard");
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "  discard r.getU32()");
        }
        for line in disc_get.replace("$r", "r").lines() {
            let _ = writeln!(out, "  {line}");
        }
        let _ = writeln!(out, "  case result.disc");
        for c in &cases {
            branch_head(out, c);
            for line in c.get.replace("$r", "r").lines() {
                let _ = writeln!(out, "    {line}");
            }
        }
        if need_else {
            let _ = writeln!(out, "  else: discard");
        }
    }
    let _ = writeln!(
        out,
        "\nproc unmarshalXCDR{ty}*(buf: seq[byte], endian: Endian): {ety} ="
    );
    let _ = writeln!(out, "  var r = initReader(buf, endian)");
    let _ = writeln!(out, "  read{ty}(r)");
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "", &u.annotations, PlacementKind::EndDeclaration);
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
            let n = resolve_scoped_name(sn);
            enum_names.contains(&n) || is_bit_name(&n)
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
        TypeSpec::Sequence(seq) => map_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            expr,
            enum_names,
            struct_names,
        ),
        // A `fixed<P,S>` decimal: packed BCD, `(P+2)/2` raw octets, no length
        // prefix and no alignment (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5). The
        // Nim field holds the BCD bytes directly (`seq[byte]`); the generated
        // `zdFixedEnc` prelude builds them from a decimal string.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let _ = fixed_ps(f)?; // validate P/S resolve at codegen time
            Ok(("seq[byte]".to_string(), format!("$w.putBytes({expr})")))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Enum holder width follows @bit_bound (XTypes 1.3 §7.4.5.1);
                // putU8/putU16 mask internally, so the ordinal is passed as-is.
                let put = match enum_wire_width(&name) {
                    1 => format!("$w.putU8(int(ord({expr})))"),
                    2 => format!("$w.putU16(int(ord({expr})))"),
                    _ => format!("$w.putU32(cast[uint32](int32(ord({expr}))))"),
                };
                Ok((escape_nim_ident(&name), put))
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                // Nested struct / bitset / bitmask member: marshal into the same
                // writer (a bit holder's wire form is its backing integer).
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
            // Nested-struct key members serialize in ascending member-id order
            // (XTypes 1.3 §7.6.8), honoring the nested struct's own
            // `@autoid(HASH)` plus per-member `@id`/`@hashid` — mirrors idl-rust
            // `compute_key_holder_max_size`/`encode_key_holder` (findings A31/A32).
            let nested_hash =
                zerodds_idl::semantics::member_id::container_autoid_hash(&sd.annotations);
            let mut ordered: Vec<(u32, &Member)> = effective
                .iter()
                .enumerate()
                .map(|(idx, m)| {
                    let raw_name = m
                        .declarators
                        .first()
                        .map(|d| d.name().text.clone())
                        .unwrap_or_default();
                    let id = zerodds_idl::semantics::member_id::resolved_member_id(
                        nested_hash,
                        &m.annotations,
                        &raw_name,
                        idx as u32,
                    );
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    enum_names: &HashSet<String>,
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
        let name = resolve_scoped_name(sn);
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
    // sequence<arbitrary> → u32 count + per-element encode (no collection
    // DHEADER; the element type is fully descriptive on the wire for the
    // primitive / enum / bitset / bitmask cases reaching here). Mirrors the
    // `idl-go`/`idl-d` fallback.
    let (elem_ty, elem_put) = map_type(elem, "zdElem", enum_names, struct_names)?;
    let body = indent(&elem_put, 2);
    let put = format!("{bc}$w.putU32(uint32({expr}.len))\nfor zdElem in {expr}:\n{body}");
    Ok((format!("seq[{elem_ty}]"), put))
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
        TypeSpec::Sequence(seq) => map_get_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            target,
            enum_names,
            struct_names,
        ),
        // `fixed<P,S>`: read the statically-known `(P+2)/2` BCD octets (no
        // length prefix, no alignment).
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let (p, _) = fixed_ps(f)?;
            let n = (p + 2) / 2;
            Ok(format!("{target} = $r.getBytesN({n})"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                let esc = escape_nim_ident(&name);
                // Read the @bit_bound-wide holder and sign-extend to int via a
                // signed cast (XTypes 1.3 §7.4.5.1).
                let get = match enum_wire_width(&name) {
                    1 => format!("{target} = {esc}(int(cast[int8](uint8($r.getU8()))))"),
                    2 => format!("{target} = {esc}(int(cast[int16](uint16($r.getU16()))))"),
                    _ => format!("{target} = {esc}(int($r.getU32()))"),
                };
                Ok(get)
            } else if struct_names.contains(&name) || is_bit_name(&name) {
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    target: &str,
    enum_names: &HashSet<String>,
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
        let name = resolve_scoped_name(sn);
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
    // sequence<arbitrary> → u32 count + per-element decode (no collection
    // DHEADER; mirrors the encode-side `map_sequence` fallback).
    let bound_check = n
        .map(|n| {
            format!(
                "\n  if zdN > {n}: raise newException(ValueError, \"decoded sequence length exceeds its IDL bound ({n})\")"
            )
        })
        .unwrap_or_default();
    let (elem_ty, _) = map_type(elem, "zdElem", enum_names, struct_names)?;
    let elem_get = map_get(elem, &format!("{target}[zdI]"), enum_names, struct_names)?;
    let body = indent(&elem_get, 4);
    Ok(format!(
        "block:\n  let zdN = int($r.getU32()){bound_check}\n  {target} = newSeq[{elem_ty}](zdN)\n  for zdI in 0 ..< zdN:\n{body}"
    ))
}
