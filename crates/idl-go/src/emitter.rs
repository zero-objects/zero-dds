// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → Go emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! Go source file: a shared XCDR2 wire `Writer` (byte-identical to the
//! hand-written `endpoints/go` core) plus, per IDL `struct`, a Go struct and a
//! `MarshalXCDR(endian)` method. `@final` (compact) and `@appendable` (a
//! DHEADER-framed body) are supported; other extensibilities and constructs
//! raise [`IdlGoError::Unsupported`].
//!
//! NOTED, not fixed (deep review of #22 decode-bounds-cross-backend,
//! moderate item): every IDL-bound violation in this backend — decode's
//! `panic("decoded ... exceeds its IDL bound (N)")` (`map_get`,
//! `map_get_sequence`, `map_get_map`) as well as the pre-existing
//! wire-format errors the whole reader already raises on a malformed/
//! truncated buffer — surfaces as a Go `panic`, not an `error` return. That
//! is pre-existing, whole-`Reader` debt (the reader's primitive `Get*`
//! methods panic on out-of-range reads too; a `panic`-to-`error` conversion
//! would need to thread `error` through every `Get*`/`Unmarshal*` signature,
//! not just the bound-check call sites this fix touches), so it is called
//! out here rather than fixed as part of this change.

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

use crate::error::{IdlGoError, Result};
use crate::keywords::escape_go_ident;

thread_local! {
    /// Fully-qualified IDL scope path of every named type declaration
    /// (e.g. `["a", "Reading"]`), populated by [`register_type_paths`] at the
    /// start of each run. A reference site resolves a (possibly partially
    /// qualified) `ScopedName` against the enclosing module scope by walking
    /// outward and matching one of these paths (§7.5.2), then flattens the
    /// match the SAME way [`qualify`] flattens the definition. Without this a
    /// member `Reading` inside `module a` would emit the bare name `Reading`,
    /// but the type is defined flattened as `a_Reading` (#21 cross-module).
    static TYPE_PATHS: std::cell::RefCell<Vec<Vec<String>>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Module scope (`["a"]`, `["a", "b"]`, …) of the aggregate currently being
    /// emitted. [`resolve_scoped_name`] walks outward from this scope, so every
    /// member reference resolves exactly as IDL name lookup would. Set at the
    /// top of [`emit_struct`]/[`emit_union`]; empty at global scope.
    static CURRENT_SCOPE: std::cell::RefCell<Vec<String>> =
        const { std::cell::RefCell::new(Vec::new()) };

    /// Flattened logical names of every `bitset`/`bitmask` declaration. A
    /// reference to one of these maps to a Go holder struct whose wire form is a
    /// single backing integer (`marshalInto`/`unmarshalFrom<name>`) — no
    /// collection DHEADER, so it is treated as fully-descriptive (primitive) by
    /// the sequence/map framing rules (XTypes 1.3 §7.4.7).
    static BIT_NAMES: std::cell::RefCell<HashSet<String>> =
        std::cell::RefCell::new(HashSet::new());

    /// Set whenever a `fixed<P,S>` member is emitted, so the BCD prelude helper
    /// is appended exactly once (and only when needed).
    static USED_FIXED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

    /// Flattened qualified enum name → signed wire holder width in OCTETS
    /// (1/2/4), derived from `@bit_bound` (XTypes 1.3 §7.3.1.2.1.9 + §7.4.5.1)
    /// via the shared [`enum_wire_octets`]. Populated once per run alongside
    /// [`enum_names`]; read at the single enum encode/decode site so a
    /// `@bit_bound(8)`/`@bit_bound(16)` enum narrows to 1/2 bytes instead of the
    /// former fixed 4. Mirrors idl-cpp's `ENUM_BYTES`.
    static ENUM_WIDTHS: std::cell::RefCell<HashMap<String, u32>> =
        std::cell::RefCell::new(HashMap::new());

    /// Central const/enum symbol table for the whole spec, built once per run by
    /// [`build_symbol_table`]. Every collection bound and fixed-array dimension
    /// is resolved through it (in [`array_size`]) so a bound written as `CAP` or
    /// `BASE*2` yields its evaluated integer instead of aborting the backend
    /// (`non-literal array size`) as the former literal-only path did (audit P1
    /// "Const-Eval driftet").
    static CONST_SYMS: std::cell::RefCell<zerodds_idl::semantics::SymbolTable> =
        std::cell::RefCell::new(zerodds_idl::semantics::SymbolTable::new());
}

/// Signed wire holder width in octets (1/2/4) an enum named `name` serializes
/// at, per its `@bit_bound`. Defaults to 4 for an unregistered name / no
/// `@bit_bound` (XTypes 1.3 §7.4.5.1 default bound 32).
fn enum_wire_width(name: &str) -> u32 {
    ENUM_WIDTHS
        .with(|m| m.borrow().get(name).copied())
        .unwrap_or(4)
}

/// Go codegen language aliases matched by `@verbatim(language="...")`
/// (case-insensitive; the spec wildcard `"*"` always matches — see
/// [`Lowered::verbatims_for_language`]).
const GO_LANG_ALIASES: &[&str] = &["go", "golang"];

/// Emits every `@verbatim` block from `anns` whose language matches the Go
/// codegen and whose `placement` equals `placement`, each line prefixed with
/// `indent`. Source order preserved; text spliced unmodified (no wire impact —
/// XTypes 1.3 §7.2.2.4.8 / IDL 4.2 §8.3.5.1). Mirrors `idl-rust`'s
/// `verbatim::emit_verbatim_at`.
fn emit_verbatim_at(out: &mut String, indent: &str, anns: &[Annotation], placement: PlacementKind) {
    let Ok(lowered) = lower_annotations(anns) else {
        return;
    };
    for v in lowered.verbatims_for_language(GO_LANG_ALIASES) {
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
/// `END_FILE`) and per-declaration `@verbatim` placement. Mirrors `idl-rust`'s
/// `top_level_annotations`.
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
/// different modules (`a::Reading`, `b::Reading`) become distinct Go types
/// `a_Reading`/`b_Reading` instead of a duplicate `Reading` (#21).
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
/// single Go identifier. Each segment's own underscores are doubled and the
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
            Definition::Type(td) => register_type_decl_path(td, scope),
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

/// Registers the fully-qualified path of a single `TypeDecl` (used for both
/// module-level and interface-nested declarations).
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
/// type path. Falls back to the literal flattening of the written parts for a
/// name with no registered declaration (built-in/forward-only).
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

/// Options for the Go backend.
#[derive(Debug, Clone)]
pub struct GoGenOptions {
    /// The `package` name emitted at the top of the file.
    pub package_name: String,
}

impl Default for GoGenOptions {
    fn default() -> Self {
        Self {
            package_name: "zdgen".to_string(),
        }
    }
}

/// The shared XCDR2 wire `Writer`, byte-identical to `endpoints/go/wire.go`.
/// Emitted once per file so the generated structs stay self-contained.
const WIRE_PRELUDE: &str = r#"// Endianness is the wire byte order (the XCDR encapsulation flag, not the host).
type Endianness int

const (
	// Little wire byte order.
	Little Endianness = iota
	// Big wire byte order.
	Big
)

const xcdr2MaxAlign = 4

// Writer accumulates XCDR bytes into a growable buffer.
type Writer struct {
	Buf    []byte
	Endian Endianness
}

// NewWriter returns a Writer for the given wire byte order.
func NewWriter(endian Endianness) *Writer { return &Writer{Endian: endian} }

func (w *Writer) align(a int) {
	if a > xcdr2MaxAlign {
		a = xcdr2MaxAlign
	}
	for (len(w.Buf) % a) != 0 {
		w.Buf = append(w.Buf, 0)
	}
}

func (w *Writer) putLE(a int, le []byte) {
	w.align(a)
	if w.Endian == Big {
		for i := len(le) - 1; i >= 0; i-- {
			w.Buf = append(w.Buf, le[i])
		}
	} else {
		w.Buf = append(w.Buf, le...)
	}
}

// PutU8 appends one byte (no alignment).
func (w *Writer) PutU8(v byte) { w.Buf = append(w.Buf, v) }

// PutBool appends a boolean as one octet.
func (w *Writer) PutBool(v bool) {
	if v {
		w.PutU8(1)
	} else {
		w.PutU8(0)
	}
}

// PutU16 appends a 2-aligned uint16.
func (w *Writer) PutU16(v uint16) { w.putLE(2, []byte{byte(v), byte(v >> 8)}) }

// PutU32 appends a 4-aligned uint32.
func (w *Writer) PutU32(v uint32) {
	w.putLE(4, []byte{byte(v), byte(v >> 8), byte(v >> 16), byte(v >> 24)})
}

// PutU64 appends a uint64 (XCDR2: 4-aligned).
func (w *Writer) PutU64(v uint64) {
	le := make([]byte, 8)
	for i := 0; i < 8; i++ {
		le[i] = byte(v >> (8 * i))
	}
	w.putLE(4, le)
}

// PutF32 appends a float32 as its IEEE-754 bit pattern.
func (w *Writer) PutF32(v float32) { w.PutU32(math.Float32bits(v)) }

// PutF64 appends a float64 as its IEEE-754 bit pattern.
func (w *Writer) PutF64(v float64) { w.PutU64(math.Float64bits(v)) }

// PutBytes appends raw bytes (no alignment).
func (w *Writer) PutBytes(b []byte) { w.Buf = append(w.Buf, b...) }

// PutString appends a CDR string: u32 length (incl. NUL) + bytes + one NUL.
func (w *Writer) PutString(s string) {
	w.PutU32(uint32(len(s) + 1))
	w.PutBytes([]byte(s))
	w.PutU8(0)
}

// PutSeqU8 appends a sequence<octet>: u32 length + raw bytes.
func (w *Writer) PutSeqU8(b []byte) {
	w.PutU32(uint32(len(b)))
	w.PutBytes(b)
}

// PutWString appends a wstring: u32 octet length (2·units, no BOM) + UTF-16 units.
func (w *Writer) PutWString(s string) {
	var units []uint16
	for _, r := range s {
		if r <= 0xFFFF {
			units = append(units, uint16(r))
		} else {
			r -= 0x10000
			units = append(units, uint16(0xD800+(r>>10)), uint16(0xDC00+(r&0x3FF)))
		}
	}
	w.PutU32(uint32(len(units) * 2))
	for _, u := range units {
		w.PutU16(u)
	}
}

// wstringUnitLen returns s's length in UTF-16 code units (surrogate pairs for
// non-BMP runes), matching PutWString's wire unit count — used only to check
// a `wstring<N>` value against its IDL-declared bound (XTypes 1.3 §7.4.3).
func wstringUnitLen(s string) int {
	n := 0
	for _, r := range s {
		if r <= 0xFFFF {
			n++
		} else {
			n += 2
		}
	}
	return n
}

// PutLongDouble appends a long double: the IEEE binary128 widened from a float64.
func (w *Writer) PutLongDouble(v float64) {
	bits := math.Float64bits(v)
	sign := bits >> 63
	exp := (bits >> 52) & 0x7FF
	mant := bits & 0xFFFFFFFFFFFFF
	var hi, lo uint64
	if exp == 0 && mant == 0 {
		hi = sign << 63
	} else {
		hi = (sign << 63) | ((exp - 1023 + 16383) << 48) | (mant >> 4)
		lo = (mant & 0xF) << 60
	}
	le := make([]byte, 16)
	for i := 0; i < 8; i++ {
		le[i] = byte(lo >> (8 * i))
		le[8+i] = byte(hi >> (8 * i))
	}
	w.putLE(4, le)
}

// Bytes returns the accumulated wire buffer.
func (w *Writer) Bytes() []byte { return w.Buf }

// Reader consumes XCDR bytes (the inverse of Writer).
type Reader struct {
	Buf    []byte
	Pos    int
	Endian Endianness
}

// NewReader returns a Reader over buf for the given wire byte order.
func NewReader(buf []byte, endian Endianness) *Reader { return &Reader{Buf: buf, Endian: endian} }

func (r *Reader) ralign(a int) {
	if a > xcdr2MaxAlign {
		a = xcdr2MaxAlign
	}
	for r.Pos%a != 0 {
		r.Pos++
	}
}
func (r *Reader) getLE(a, n int) uint64 {
	r.ralign(a)
	var v uint64
	if r.Endian == Big {
		for i := 0; i < n; i++ {
			v = (v << 8) | uint64(r.Buf[r.Pos+i])
		}
	} else {
		for i := n - 1; i >= 0; i-- {
			v = (v << 8) | uint64(r.Buf[r.Pos+i])
		}
	}
	r.Pos += n
	return v
}

// GetU8 reads one byte.
func (r *Reader) GetU8() byte { v := r.Buf[r.Pos]; r.Pos++; return v }

// GetBool reads a boolean octet.
func (r *Reader) GetBool() bool { return r.GetU8() != 0 }

// GetU16 reads a 2-aligned uint16.
func (r *Reader) GetU16() uint16 { return uint16(r.getLE(2, 2)) }

// GetU32 reads a 4-aligned uint32.
func (r *Reader) GetU32() uint32 { return uint32(r.getLE(4, 4)) }

// GetU64 reads a uint64 (XCDR2: 4-aligned).
func (r *Reader) GetU64() uint64 { return r.getLE(4, 8) }

// GetF32 reads a float32.
func (r *Reader) GetF32() float32 { return math.Float32frombits(r.GetU32()) }

// GetF64 reads a float64.
func (r *Reader) GetF64() float64 { return math.Float64frombits(r.GetU64()) }

// GetBytesN reads n raw bytes.
func (r *Reader) GetBytesN(n int) []byte { b := r.Buf[r.Pos : r.Pos+n]; r.Pos += n; return b }

// GetString reads a CDR string (u32 length incl. NUL + bytes).
func (r *Reader) GetString() string {
	n := int(r.GetU32())
	b := r.GetBytesN(n)
	if n > 0 {
		return string(b[:n-1])
	}
	return ""
}

// GetSeqU8 reads a sequence<octet> (u32 length + bytes).
func (r *Reader) GetSeqU8() []byte {
	n := int(r.GetU32())
	out := make([]byte, n)
	copy(out, r.GetBytesN(n))
	return out
}

// GetWString reads a wstring (u32 octet length + UTF-16 units).
func (r *Reader) GetWString() string {
	n := int(r.GetU32()) / 2
	units := make([]uint16, n)
	for i := 0; i < n; i++ {
		units[i] = r.GetU16()
	}
	var out []rune
	for i := 0; i < n; i++ {
		u := units[i]
		if u >= 0xD800 && u <= 0xDBFF && i+1 < n {
			out = append(out, 0x10000+(rune(u-0xD800)<<10)+rune(units[i+1]-0xDC00))
			i++
		} else {
			out = append(out, rune(u))
		}
	}
	return string(out)
}

// GetLongDouble reads a long double (IEEE binary128) narrowed to a float64.
func (r *Reader) GetLongDouble() float64 {
	r.ralign(4)
	le := make([]byte, 16)
	copy(le, r.GetBytesN(16))
	if r.Endian == Big {
		for i := 0; i < 8; i++ {
			le[i], le[15-i] = le[15-i], le[i]
		}
	}
	var lo, hi uint64
	for i := 0; i < 8; i++ {
		lo |= uint64(le[i]) << (8 * i)
		hi |= uint64(le[8+i]) << (8 * i)
	}
	sign := hi >> 63
	exp := (hi >> 48) & 0x7FFF
	mant := ((hi & 0xFFFFFFFFFFFF) << 4) | (lo >> 60)
	var bits uint64
	if exp == 0 && mant == 0 {
		bits = sign << 63
	} else {
		bits = (sign << 63) | ((exp - 16383 + 1023) << 52) | mant
	}
	return math.Float64frombits(bits)
}
"#;

/// BCD codec for `fixed<P,S>`. Appended once when any `fixed` member is emitted.
/// Builds the packed-BCD octet sequence (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5)
/// from a decimal string: an optional leading pad nibble (so the nibble count
/// is even), `P` digit nibbles most-significant first, then the sign nibble
/// (`0xC` positive, `0xD` negative). Byte count `(P+2)/2`, no length prefix.
const FIXED_PRELUDE: &str = r#"
// zdFixedEnc packs a decimal string into the CORBA/XCDR2 fixed<P,S> BCD form:
// (P+2)/2 octets, no length prefix (CORBA §9.3.2.7 ≡ XCDR2 §7.4.4.5).
func zdFixedEnc(s string, P uint, S uint) []byte {
	sign := true
	i := 0
	if len(s) > 0 && (s[0] == '-' || s[0] == '+') {
		sign = s[0] != '-'
		i = 1
	}
	rest := s[i:]
	dot := len(rest)
	for k := 0; k < len(rest); k++ {
		if rest[k] == '.' {
			dot = k
			break
		}
	}
	ip := rest[:dot]
	fp := ""
	if dot < len(rest) {
		fp = rest[dot+1:]
	}
	var db []byte
	intNeeded := int(P - S)
	for j := len(ip); j < intNeeded; j++ {
		db = append(db, '0')
	}
	db = append(db, ip...)
	db = append(db, fp...)
	for j := len(fp); j < int(S); j++ {
		db = append(db, '0')
	}
	var nib []byte
	if (P+1)%2 == 1 {
		nib = append(nib, 0)
	}
	for _, c := range db {
		nib = append(nib, c-'0')
	}
	if sign {
		nib = append(nib, 0x0C)
	} else {
		nib = append(nib, 0x0D)
	}
	var outb []byte
	for k := 0; k < len(nib); k += 2 {
		outb = append(outb, (nib[k]<<4)|nib[k+1])
	}
	return outb
}
"#;

/// Generates a self-contained Go module from the IDL AST: the shared XCDR2
/// wire `Writer`/`Reader` prelude followed by every generated type.
///
/// # Errors
/// Returns [`IdlGoError::Unsupported`] for constructs the Go backend does not
/// yet emit (e.g. non-literal array/sequence bounds).
pub fn generate_go_module(spec: &Specification, opts: &GoGenOptions) -> Result<String> {
    generate(spec, opts, true)
}

/// Generates a Go **fragment** for the given IDL AST: the generated types
/// only, WITHOUT the shared wire prelude (`Writer`/`Reader`/`Endianness`). Use
/// this for every file but the first in a multi-file compose so the prelude is
/// defined exactly once across the package (#C-go — the whole-prelude was
/// previously emitted per file, so a second generated file re-declared
/// `Writer`/`Reader`/`Endianness` and the package failed to build). The
/// `package` name is taken from `opts` (parametrized, not hard-coded).
///
/// # Errors
/// As [`generate_go_module`].
pub fn generate_go_fragment(spec: &Specification, opts: &GoGenOptions) -> Result<String> {
    generate(spec, opts, false)
}

fn generate(spec: &Specification, opts: &GoGenOptions, emit_prelude: bool) -> Result<String> {
    // Register every named type's fully-qualified path so reference sites can
    // resolve a `ScopedName` against its enclosing scope (#21 cross-module,
    // #A39 interface-nested).
    TYPE_PATHS.with(|t| t.borrow_mut().clear());
    register_type_paths(&spec.definitions, &mut Vec::new());
    USED_FIXED.with(|f| f.set(false));

    let mut body = String::new();
    if emit_prelude {
        body.push_str(WIRE_PRELUDE);
    }

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_FILE)` from all top-level defs
    // (source order), emitted after the wire prelude, before any type.
    for def in &spec.definitions {
        emit_verbatim_at(
            &mut body,
            "",
            def_annotations(def),
            PlacementKind::BeginFile,
        );
    }

    // `module X { ... }` content is promoted to the top level, each definition
    // paired with its module scope path (see `flatten_module_defs`).
    let flat = flatten_module_defs(&spec.definitions);
    // Interface-nested type declarations (#A39): promoted to the top level
    // under the interface's own scope segment, so their DDS data types survive
    // instead of being silently dropped with the interface body.
    let iface_types = flatten_iface_types(&spec.definitions);

    // Named enums/structs/bit-containers referenced by members, keyed by their
    // flattened module-qualified name (matching the definition site). An enum
    // member is a 32-bit signed integer on the wire (XTypes 1.3 §7.4.5.1).
    let mut enum_names: HashSet<String> = HashSet::new();
    let mut struct_names: HashSet<String> = HashSet::new();
    let mut bit_names: HashSet<String> = HashSet::new();
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
    // Central const/enum table for bound + array-dimension resolution (P1).
    CONST_SYMS.with(|c| *c.borrow_mut() = zerodds_idl::semantics::build_symbol_table(spec));
    // Register each enum's @bit_bound-derived wire width (1/2/4 octets) so the
    // encode/decode sites can narrow the holder (P1, XTypes 1.3 §7.3.1.2.1.9).
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
    // before mapping) and struct qualified-name → def (for nested-struct `@key`
    // KeyHash expansion). Interface-nested typedefs/structs are folded in too.
    let mut typedefs = collect_typedefs(spec);
    let mut structs = collect_structs(spec);
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
        emit_verbatim_at(&mut body, "", anns, PlacementKind::BeforeDeclaration);
        match def {
            Definition::Type(td) => emit_type_decl(
                &mut body,
                td,
                scope,
                &enum_names,
                &struct_names,
                &typedefs,
                &structs,
                &enum_defs,
            )?,
            // #A5/P1 — a top-level `const` was silently dropped by the former
            // catch-all arm; emit it as a Go package-level constant.
            Definition::Const(c) => emit_const(&mut body, c, scope),
            _ => {}
        }
        // §7.2.2.4.8 — text directly after the annotated declaration.
        emit_verbatim_at(&mut body, "", anns, PlacementKind::AfterDeclaration);
    }

    // Interface-nested types (#A39), emitted after the module-level defs.
    for (scope, td) in &iface_types {
        emit_type_decl(
            &mut body,
            td,
            scope,
            &enum_names,
            &struct_names,
            &typedefs,
            &structs,
            &enum_defs,
        )?;
    }

    // §7.2.2.4.8 — `@verbatim(placement=END_FILE)` from all top-level defs.
    for def in &spec.definitions {
        emit_verbatim_at(&mut body, "", def_annotations(def), PlacementKind::EndFile);
    }

    // The BCD codec prelude is appended once if any `fixed<P,S>` was emitted.
    // Only the prelude-carrying file owns the shared helper (#C-go).
    if emit_prelude && USED_FIXED.with(std::cell::Cell::get) {
        body.push_str(FIXED_PRELUDE);
    }

    // Assemble: header + package + on-demand imports (Go rejects unused ones) +
    // body. `math` is only referenced by the prelude's IEEE-754 helpers; `sort`
    // by a map put's `sort.Slice`; `crypto/md5` by the KeyHash MD5 branch.
    let mut imports: Vec<&str> = Vec::new();
    if emit_prelude {
        imports.push("math");
    }
    if body.contains("sort.Slice") {
        imports.push("sort");
    }
    if body.contains("md5.Sum(") {
        imports.push("crypto/md5");
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Code generated by zerodds-idlc (Go backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "// SPDX-License-Identifier: Apache-2.0");
    let _ = writeln!(out, "package {}\n", opts.package_name);
    match imports.as_slice() {
        [] => {}
        [one] => {
            let _ = writeln!(out, "import \"{one}\"\n");
        }
        many => {
            out.push_str("import (\n");
            for i in many {
                let _ = writeln!(out, "\t\"{i}\"");
            }
            out.push_str(")\n\n");
        }
    }
    out.push_str(&body);
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
    typedefs: &HashMap<String, TypeSpec>,
    structs: &HashMap<String, &StructDef>,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<()> {
    match td {
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => emit_enum(out, e, scope),
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            emit_struct(out, s, scope, enum_names, struct_names, typedefs, structs)?;
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
/// `enclosing_module… + interface_name` (#A39). Go has no nested-type
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

/// A generated union case: its integer labels (empty = `default`), the member
/// field name, its type, and the per-member put statement.
struct UnionCase {
    labels: Vec<i64>,
    is_default: bool,
    field: String,
    go_type: String,
    put: String,
    get: String,
}

/// Evaluates a union case label (`case RED:`, `case 'A':`, `case TRUE:`,
/// `case 3:`) to its integer discriminant (#A11/A12/A13/P4). Beyond the plain
/// integer literals the former `array_size` accepted, this resolves enum
/// enumerators (via `enum_vals`, name → value of the switch enum), `char`
/// code points, and the `boolean` keywords `TRUE`/`FALSE`.
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

/// Emits an IDL `union` as a Go struct holding the discriminator + one field per
/// case member, plus a `marshalInto` that puts the discriminator then switches
/// on it to marshal the selected member (XCDR2 §7.4.3.5.4). `@final`: inline;
/// `@appendable`: a DHEADER-framed body; `@mutable`: an EMHEADER-framed member
/// list (discriminator = member id 0, each branch = its 1-based id — #A16).
#[allow(clippy::too_many_arguments)]
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
    let (disc_type, disc_put) = map_type(&disc_ts, "v.Disc", enum_names, struct_names, 0)?;
    let disc_get = map_get(&disc_ts, "v.Disc", enum_names, struct_names, 0)?;

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

    let mut cases: Vec<UnionCase> = Vec::new();
    for c in &u.cases {
        let field = exported(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (go_type, put) = map_type(
            &resolved,
            &format!("v.{field}"),
            enum_names,
            struct_names,
            0,
        )?;
        let get = map_get(
            &resolved,
            &format!("v.{field}"),
            enum_names,
            struct_names,
            0,
        )?;
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => {
                    labels.push(eval_union_label(e, &enum_vals).ok_or_else(|| {
                        IdlGoError::Unsupported(format!(
                            "non-integer union label in `{}`",
                            u.name.text
                        ))
                    })?)
                }
            }
        }
        cases.push(UnionCase {
            labels,
            is_default,
            field,
            go_type,
            put,
            get,
        });
    }

    let ty = escape_go_ident(&qualify(scope, &u.name.text));
    let _ = writeln!(
        out,
        "\n// {ty} is generated from the IDL union of the same name."
    );
    let _ = writeln!(out, "type {ty} struct {{");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "\t", &u.annotations, PlacementKind::BeginDeclaration);
    let _ = writeln!(out, "\tDisc {disc_type}");
    for c in &cases {
        let _ = writeln!(out, "\t{} {}", c.field, c.go_type);
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "\t", &u.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}}");

    // A boolean discriminator switches on Go `true`/`false`, not integers
    // (`v.Disc` is a `bool`); every other discriminator is an integer/enum/char.
    let disc_is_bool = matches!(u.switch_type, SwitchTypeSpec::Boolean);
    let case_label = |out: &mut String, c: &UnionCase| {
        if c.is_default {
            let _ = writeln!(out, "\tdefault:");
        } else {
            let labels = c
                .labels
                .iter()
                .map(|&v| {
                    if disc_is_bool {
                        (v != 0).to_string()
                    } else {
                        v.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "\tcase {labels}:");
        }
    };

    let _ = writeln!(out, "\nfunc (v {ty}) marshalInto(w *Writer) {{");
    match ext {
        ExtensibilityKind::Mutable => {
            // #A16: EMHEADER-framed member list — discriminator is member id 0,
            // each branch its 1-based id, wrapped in the struct's DHEADER.
            let _ = writeln!(out, "\tbody := NewWriter(w.Endian)");
            write_mutable_member_encode(out, "\t", "body", 0, false, &disc_put);
            let _ = writeln!(out, "\tswitch v.Disc {{");
            for (i, c) in cases.iter().enumerate() {
                case_label(out, c);
                let id = u32::try_from(i + 1).unwrap_or(0);
                write_mutable_member_encode(out, "\t\t", "body", id, false, &c.put);
            }
            let _ = writeln!(out, "\t}}");
            let _ = writeln!(out, "\tw.PutU32(uint32(len(body.Buf)))");
            let _ = writeln!(out, "\tw.PutBytes(body.Buf)");
        }
        _ => {
            let wv = if ext == ExtensibilityKind::Final {
                "w"
            } else {
                let _ = writeln!(out, "\tbody := NewWriter(w.Endian)");
                "body"
            };
            let _ = writeln!(out, "\t{}", disc_put.replace("$w", wv));
            let _ = writeln!(out, "\tswitch v.Disc {{");
            for c in &cases {
                case_label(out, c);
                let _ = writeln!(out, "\t\t{}", c.put.replace("$w", wv));
            }
            let _ = writeln!(out, "\t}}");
            if ext == ExtensibilityKind::Appendable {
                let _ = writeln!(out, "\tw.PutU32(uint32(len(body.Buf)))");
                let _ = writeln!(out, "\tw.PutBytes(body.Buf)");
            }
        }
    }
    let _ = writeln!(out, "}}");

    let _ = writeln!(
        out,
        "\n// MarshalXCDR encodes {ty} as XCDR2 for the given wire byte order."
    );
    let _ = writeln!(
        out,
        "func (v {ty}) MarshalXCDR(endian Endianness) []byte {{"
    );
    let _ = writeln!(out, "\tw := NewWriter(endian)");
    let _ = writeln!(out, "\tv.marshalInto(w)");
    let _ = writeln!(out, "\treturn w.Bytes()");
    let _ = writeln!(out, "}}");

    // unmarshalFrom decodes the union: read the discriminator, then the selected
    // member (the inverse of marshalInto). The mutable path is positional: it
    // reads the discriminator EMHEADER + value, switches, then reads the one
    // selected branch's EMHEADER + value (a fully-present union round-trips).
    let _ = writeln!(out, "\nfunc unmarshalFrom{ty}(r *Reader) {ty} {{");
    let _ = writeln!(out, "\tvar v {ty}");
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "\t_ = r.GetU32() // DHEADER");
    }
    if ext == ExtensibilityKind::Mutable {
        write_mutable_member_decode(out, "\t", &disc_get);
    } else {
        let _ = writeln!(out, "\t{disc_get}");
    }
    let _ = writeln!(out, "\tswitch v.Disc {{");
    for c in &cases {
        case_label(out, c);
        if ext == ExtensibilityKind::Mutable {
            write_mutable_member_decode(out, "\t\t", &c.get);
        } else {
            let _ = writeln!(out, "\t\t{}", c.get);
        }
    }
    let _ = writeln!(out, "\t}}");
    let _ = writeln!(out, "\treturn v");
    let _ = writeln!(out, "}}");

    let _ = writeln!(
        out,
        "\nfunc UnmarshalXCDR{ty}(buf []byte, endian Endianness) {ty} {{"
    );
    let _ = writeln!(out, "\treturn unmarshalFrom{ty}(NewReader(buf, endian))");
    let _ = writeln!(out, "}}");
    Ok(())
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

/// Emits an IDL `enum` as a Go `int32`-based type + its enumerator constants.
fn emit_enum(out: &mut String, e: &EnumDef, scope: &[String]) {
    let values = enumerator_values(e);
    let ty = exported(&qualify(scope, &e.name.text));
    let _ = writeln!(
        out,
        "\n// {ty} is generated from the IDL enum of the same name."
    );
    let _ = writeln!(out, "type {ty} int32");
    let _ = writeln!(out, "const (");
    for (en, value) in e.enumerators.iter().zip(&values) {
        let _ = writeln!(out, "\t{ty}{} {ty} = {value}", exported(&en.name.text));
    }
    let _ = writeln!(out, ")");
}

/// Emits an IDL `const` as a Go package-level constant (#A5/P1). A `const` of
/// any type used to vanish through the top-level catch-all arm. The value is
/// rendered from the `ConstExpr` (Boolean literals normalized to `true`/`false`
/// and any wide `L"…"`/`L'…'` prefix stripped, so the output is always valid
/// Go). Values the Go type system cannot express as a compile-time constant
/// (an enum-typed reference, a `fixed` decimal) are skipped rather than
/// emitting ill-formed source.
fn emit_const(out: &mut String, c: &ConstDecl, scope: &[String]) {
    let Some(val) = const_expr_to_go(&c.value) else {
        return;
    };
    let name = exported(&qualify(scope, &c.name.text));
    match const_go_type(&c.type_) {
        Some(ty) => {
            let _ = writeln!(out, "\nconst {name} {ty} = {val}");
        }
        None => {
            let _ = writeln!(out, "\nconst {name} = {val}");
        }
    }
}

/// Go type for a `const` declaration (`None` = emit an untyped constant).
fn const_go_type(ct: &ConstType) -> Option<&'static str> {
    Some(match ct {
        ConstType::Integer(i) => go_int_type(*i),
        ConstType::Floating(FloatingType::Float) => "float32",
        ConstType::Floating(FloatingType::Double | FloatingType::LongDouble) => "float64",
        ConstType::Char | ConstType::Octet => "byte",
        ConstType::WideChar => "rune",
        ConstType::Boolean => "bool",
        ConstType::String { .. } => "string",
        // A `fixed` const has no native Go compile-time type; render its decimal
        // as a string constant.
        ConstType::Fixed => "string",
        // An enum-typed / scoped const value cannot be reconstructed from the
        // bare enumerator name; leave untyped and let the value renderer decide.
        ConstType::Scoped(_) => return None,
    })
}

/// The Go integer type for an IDL integer type.
fn go_int_type(i: IntegerType) -> &'static str {
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

/// Renders a `ConstExpr` as a Go constant expression, or `None` for a form the
/// Go backend does not express (an enum-valued scoped reference).
/// zerodds-lint: recursion-depth 32 (const expression tree; bounded by the IDL
/// grammar's expression nesting).
fn const_expr_to_go(e: &ConstExpr) -> Option<String> {
    match e {
        ConstExpr::Literal(l) => const_literal_to_go(l),
        // An enum-valued or const-alias scoped reference cannot be rendered from
        // the bare last segment; skip (wire-neutral — the const is a codegen
        // convenience, and a wrong Go identifier would break the build).
        ConstExpr::Scoped(_) => None,
        ConstExpr::Unary { op, operand, .. } => {
            let v = const_expr_to_go(operand)?;
            let o = match op {
                UnaryOp::Plus => "+",
                UnaryOp::Minus => "-",
                UnaryOp::BitNot => "^",
            };
            Some(format!("{o}{v}"))
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let l = const_expr_to_go(lhs)?;
            let r = const_expr_to_go(rhs)?;
            let o = match op {
                BinaryOp::Or => "|",
                BinaryOp::Xor => "^",
                BinaryOp::And => "&",
                BinaryOp::Shl => "<<",
                BinaryOp::Shr => ">>",
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
            };
            Some(format!("({l} {o} {r})"))
        }
    }
}

/// Renders a single literal as valid Go source.
fn const_literal_to_go(l: &Literal) -> Option<String> {
    let raw = l.raw.trim();
    Some(match l.kind {
        // Go accepts decimal / `0x` / `0o` / `0b` integer literals as-is.
        LiteralKind::Integer => raw.to_string(),
        // Strip a trailing IDL float/fixed suffix (`d`/`f`/`l`) Go rejects.
        LiteralKind::Floating => raw
            .trim_end_matches(['d', 'D', 'f', 'F', 'l', 'L'])
            .to_string(),
        // A `fixed` decimal has no native Go type — render as a string.
        LiteralKind::Fixed => format!(
            "\"{}\"",
            raw.trim_end_matches(['d', 'D']).replace('"', "\\\"")
        ),
        // Normalize the IDL boolean keyword to Go's `true`/`false` (never emit a
        // bare `TRUE`/`FALSE` token, which is not a Go identifier — #A13).
        LiteralKind::Boolean => {
            if raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // Narrow string/char literals pass through; wide literals drop the
        // `L` prefix (#A7-pattern: `L"x"`/`L'x'` is not valid Go).
        LiteralKind::String | LiteralKind::Char => raw.to_string(),
        LiteralKind::WideString | LiteralKind::WideChar => {
            raw.strip_prefix('L').unwrap_or(raw).to_string()
        }
    })
}

/// Evaluates a `char`/`wchar` literal (`'A'`, `L'x'`, `'\n'`) to its code point.
/// Used by the union label evaluator (#A12) so a `case 'A':` resolves to the
/// discriminant 65.
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

/// Backing-integer storage for a bit container of `total_bits` bits: XTypes 1.3
/// §7.4.7 — the smallest holder that fits (`≤8`→uint8, `≤16`→uint16, `≤32`→
/// uint32, else uint64). Returns `(Go type, Put-method, Get-method)`.
fn bit_storage(total_bits: usize) -> (&'static str, &'static str, &'static str) {
    match total_bits {
        0..=8 => ("uint8", "PutU8", "GetU8"),
        9..=16 => ("uint16", "PutU16", "GetU16"),
        17..=32 => ("uint32", "PutU32", "GetU32"),
        _ => ("uint64", "PutU64", "GetU64"),
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

/// Emits an IDL `bitset` as a Go holder struct over its backing integer, with a
/// bit-accessor per named bitfield and an XCDR2 marshal/unmarshal that writes
/// the backing integer (XTypes 1.3 §7.4.7 — wire = backing int).
///
/// # Errors
/// [`IdlGoError::Unsupported`] if a bitfield width is not a codegen-time integer.
fn emit_bitset(out: &mut String, b: &BitsetDecl, scope: &[String]) -> Result<()> {
    let mut widths: Vec<usize> = Vec::with_capacity(b.bitfields.len());
    for bf in &b.bitfields {
        let w = array_size(&bf.spec.width)
            .filter(|w| *w >= 0)
            .ok_or_else(|| {
                IdlGoError::Unsupported(format!(
                    "non-integer bitfield width in bitset {}",
                    b.name.text
                ))
            })? as usize;
        widths.push(w);
    }
    let total: usize = widths.iter().sum();
    let (storage, put, get) = bit_storage(total);
    let ty = escape_go_ident(&qualify(scope, &b.name.text));

    let _ = writeln!(
        out,
        "\n// {ty} is generated from the IDL bitset of the same name."
    );
    let _ = writeln!(out, "type {ty} struct {{");
    let _ = writeln!(out, "\tStorage {storage}");
    // §7.2.2.4.8 — text as the first/last element inside the declaration.
    emit_verbatim_at(out, "\t", &b.annotations, PlacementKind::BeginDeclaration);
    emit_verbatim_at(out, "\t", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}}");

    let mut offset: usize = 0;
    for (bf, width) in b.bitfields.iter().zip(&widths) {
        if let Some(name) = &bf.name {
            let field = exported(&name.text);
            if *width == 1 {
                let _ = writeln!(
                    out,
                    "\nfunc (v {ty}) {field}() bool {{ return ((v.Storage >> {offset}) & 1) != 0 }}"
                );
                let _ = writeln!(
                    out,
                    "func (v *{ty}) Set{field}(x bool) {{ m := {storage}(1) << {offset}; if x {{ v.Storage |= m }} else {{ v.Storage &^= m }} }}"
                );
            } else {
                let mask: u128 = if *width >= 128 {
                    u128::MAX
                } else {
                    (1u128 << *width) - 1
                };
                let _ = writeln!(
                    out,
                    "\nfunc (v {ty}) {field}() {storage} {{ return {storage}((v.Storage >> {offset}) & {mask}) }}"
                );
                let _ = writeln!(
                    out,
                    "func (v *{ty}) Set{field}(x {storage}) {{ m := {storage}({mask}) << {offset}; v.Storage = (v.Storage &^ m) | ((x & {mask}) << {offset}) }}"
                );
            }
        }
        offset += width;
    }
    emit_bit_wire(out, &ty, storage, put, get);
    Ok(())
}

/// Emits an IDL `bitmask` as a Go holder struct over its `@bit_bound` backing
/// integer (default 32), with an OR-able manifest constant per bit value and an
/// XCDR2 marshal/unmarshal writing the backing integer (XTypes 1.3 §7.4.7).
fn emit_bitmask(out: &mut String, b: &BitmaskDecl, scope: &[String]) {
    let (storage, put, get) = bit_storage(bitmask_bit_bound(&b.annotations) as usize);
    let ty = escape_go_ident(&qualify(scope, &b.name.text));

    let _ = writeln!(
        out,
        "\n// {ty} is generated from the IDL bitmask of the same name."
    );
    let _ = writeln!(out, "type {ty} struct {{");
    let _ = writeln!(out, "\tStorage {storage}");
    emit_verbatim_at(out, "\t", &b.annotations, PlacementKind::BeginDeclaration);
    emit_verbatim_at(out, "\t", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}}");
    let _ = writeln!(out, "const (");
    for (idx, v) in b.values.iter().enumerate() {
        let pos = bit_position(&v.annotations).unwrap_or(idx as u32);
        let cname = format!("{ty}{}", exported(&v.name.text));
        let _ = writeln!(out, "\t{cname} {storage} = 1 << {pos}");
    }
    let _ = writeln!(out, ")");
    emit_bit_wire(out, &ty, storage, put, get);
}

/// Emits the shared marshal/unmarshal quartet for a bit-container holder `ty`
/// over backing integer `storage` (put/get method names). Wire = backing int.
fn emit_bit_wire(out: &mut String, ty: &str, storage: &str, put: &str, get: &str) {
    let _ = writeln!(
        out,
        "\nfunc (v {ty}) marshalInto(w *Writer) {{ w.{put}(v.Storage) }}"
    );
    let _ = writeln!(
        out,
        "\n// MarshalXCDR encodes {ty} as XCDR2 for the given wire byte order."
    );
    let _ = writeln!(
        out,
        "func (v {ty}) MarshalXCDR(endian Endianness) []byte {{ w := NewWriter(endian); v.marshalInto(w); return w.Bytes() }}"
    );
    let _ = writeln!(
        out,
        "func unmarshalFrom{ty}(r *Reader) {ty} {{ var v {ty}; v.Storage = {storage}(r.{get}()); return v }}"
    );
    let _ = writeln!(
        out,
        "func UnmarshalXCDR{ty}(buf []byte, endian Endianness) {ty} {{ return unmarshalFrom{ty}(NewReader(buf, endian)) }}"
    );
}

/// Resolves a `fixed<P,S>`'s digit count `P` and scale `S` to codegen-time
/// integers.
///
/// # Errors
/// [`IdlGoError::Unsupported`] if either is not a resolvable non-negative
/// integer literal.
fn fixed_ps(f: &FixedPtType) -> Result<(i64, i64)> {
    let p = array_size(&f.digits)
        .filter(|v| *v > 0)
        .ok_or_else(|| IdlGoError::Unsupported("non-integer fixed digit count".to_string()))?;
    let s = array_size(&f.scale)
        .filter(|v| *v >= 0)
        .ok_or_else(|| IdlGoError::Unsupported("non-integer fixed scale".to_string()))?;
    Ok((p, s))
}

/// Extensibility of a struct (defaults to `@appendable`, the post-SX2 default).
fn extensibility(s: &StructDef) -> ExtensibilityKind {
    lower_annotations(&s.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable)
}

/// Writes one `@mutable` member into writer `wv` (`"body"`): its EMHEADER
/// (must-understand bit 31 when `mu` — #A17) then the value as NEXTINT-prefixed
/// body bytes. Uses the LC4 length code — the universal, always-decodable form
/// emitted by the shared `zerodds-cdr` `MutableStructEncoder`, the golden
/// reference (`endpoints/golden-gen`) and every thin backend, so the wire stays
/// byte-identical across them. (Compact per-width length codes — #A19 — are a
/// separate cross-backend change; see the honest-scope note.)
fn write_mutable_member_encode(
    out: &mut String,
    indent: &str,
    wv: &str,
    id: u32,
    mu: bool,
    put: &str,
) {
    let mu_bit = if mu { 0x8000_0000_u32 } else { 0 };
    // LC4 (bits 30-28 = 0b100) | member id (28 bits).
    let emh = mu_bit | 0x4000_0000 | (id & 0x0FFF_FFFF);
    let _ = writeln!(out, "{indent}{wv}.PutU32(0x{emh:08x})");
    let _ = writeln!(out, "{indent}{{");
    let _ = writeln!(out, "{indent}\tmem := NewWriter({wv}.Endian)");
    let _ = writeln!(out, "{indent}\t{}", put.replace("$w", "mem"));
    let _ = writeln!(out, "{indent}\t{wv}.PutU32(uint32(len(mem.Buf)))");
    let _ = writeln!(out, "{indent}\t{wv}.PutBytes(mem.Buf)");
    let _ = writeln!(out, "{indent}}}");
}

/// Reads one `@mutable` member: its EMHEADER + NEXTINT (LC4) then the value via
/// `get`. Positional — it relies on members arriving in id order (see the
/// decode NOTE in `emit_struct`).
fn write_mutable_member_decode(out: &mut String, indent: &str, get: &str) {
    let _ = writeln!(out, "{indent}_ = r.GetU32() // EMHEADER");
    let _ = writeln!(out, "{indent}_ = r.GetU32() // NEXTINT");
    let _ = writeln!(out, "{indent}{get}");
}

/// Collects a struct's effective members base-first (#A10/P3): the base
/// struct's members (recursively) precede the derived struct's own, so the
/// generated Go type and its wire form carry the inherited fields — matching
/// cpp/csharp/java (`resolve_wire_members`). Without this a `struct D : Base`
/// dropped every inherited field from both the type and the wire.
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

/// Exported Go identifier from an IDL name (upper-cases the first letter,
/// then defensively escapes — capitalization alone already dodges every
/// Go keyword since all 25 are strictly lowercase, but this keeps the
/// invariant explicit rather than relied-upon).
fn exported(name: &str) -> String {
    let mut chars = name.chars();
    let capitalized = match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    };
    escape_go_ident(&capitalized)
}

/// Returns `name` if unused, otherwise the first `name_2`, `name_3`, … not yet
/// in `used`, inserting the result. Deterministic de-duplication for target-Go
/// names that collide after the exported-name rule (#A41).
fn dedup_name(used: &mut HashSet<String>, name: String) -> String {
    if used.insert(name.clone()) {
        return name;
    }
    let mut i = 2;
    loop {
        let candidate = format!("{name}_{i}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        i += 1;
    }
}

/// Recursively descends into `Definition::Module`, returning every non-module
/// definition (struct/enum/union/typedef/…) paired with its module scope path,
/// in document order. The IDL AST builder already merges a reopened `module M
/// {} ... module M {}` into one AST node (`crates/idl/src/ast/builder.rs`).
/// Go has no nested-type-declaration construct, so a module's members are
/// promoted to the top level; the scope path is carried so the definition and
/// reference sites can flatten each name to `scope_simple` ([`qualify`] /
/// [`resolve_scoped_name`]). Two same-simple-name types in different modules
/// therefore become distinct Go types rather than colliding (#21).
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

/// Collects `typedef` aliases (simple declarators) as name → aliased type-spec.
/// A typedef is wire-transparent, so members are resolved to the underlying
/// type before mapping (`typedef long Score; Score s;` marshals as `long`).
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

/// Collects top-level `struct` definitions as name → def, so a nested-struct
/// `@key` member can be expanded into its own `@key` subset (XTypes 1.3
/// §7.6.8) for KeyHash emission and for the static max-size (MD5 vs.
/// zero-pad) branch decision.
fn collect_structs(spec: &Specification) -> HashMap<String, &StructDef> {
    let mut m = HashMap::new();
    for (scope, def) in flatten_module_defs(&spec.definitions) {
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

/// Evaluates a fixed-array bound (`long xs[3]`) to its integer size. Handles
/// integer literals and unary sign; other forms are rejected upstream.
/// zerodds-lint: recursion-depth 32
fn array_size(e: &ConstExpr) -> Option<i64> {
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => parse_int(raw),
        // A non-integer literal is not a valid dimension/bound.
        ConstExpr::Literal(_) => None,
        ConstExpr::Unary { op, operand, .. } => {
            let v = array_size(operand)?;
            match op {
                UnaryOp::Plus => Some(v),
                UnaryOp::Minus => Some(-v),
                UnaryOp::BitNot => Some(!v),
            }
        }
        // Const reference (`data[CAP]`) or a const-expression (`data[BASE*2]`):
        // resolve through the central evaluator + spec-wide symbol table instead
        // of aborting the backend. The literal/unary fast paths above keep their
        // exact former rendering; only previously-unsupported forms reach here.
        ConstExpr::Scoped(_) | ConstExpr::Binary { .. } => CONST_SYMS
            .with(|c| zerodds_idl::semantics::evaluate(e, &c.borrow()).ok())?
            .as_i64(),
    }
}

/// Wraps a per-element put (`$elem` placeholder) in nested row-major index loops
/// over a fixed array `v.<Field>[i0][i1]…`.
fn build_array_put(field: &str, sizes: &[i64], elem_put: &str) -> String {
    let idx: String = (0..sizes.len()).map(|k| format!("[i{k}]")).collect();
    let mut body = elem_put.replace("$elem", &format!("v.{field}{idx}"));
    for k in (0..sizes.len()).rev() {
        body = format!(
            "for i{k} := 0; i{k} < {}; i{k}++ {{\n\t\t{body}\n\t}}",
            sizes[k]
        );
    }
    body
}

fn emit_struct(
    out: &mut String,
    s: &StructDef,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    structs: &HashMap<String, &StructDef>,
) -> Result<()> {
    // Member references resolve against this struct's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = extensibility(s);

    struct FieldGen {
        go_name: String,
        go_type: String,
        put: String,        // statement operating on writer `$w`, referencing v.<go_name>
        get: String,        // decode statement reading from `r` into v.<go_name>
        id: u32,            // XTypes member id (@id(n) or sequential)
        key: bool,          // @key member
        resolved: TypeSpec, // typedef-dealiased type of this field
        simple: bool,       // true for a `Declarator::Simple` (not a fixed array)
        optional: bool,     // `@optional`: uint8 presence flag then value
        must_understand: bool, // `@must_understand`: EMHEADER bit 31 (@mutable)
        external: bool,     // `@external`: wire-neutral, surfaced as a doc marker
    }
    // #A10/P3: base-first effective member list (inherited members precede the
    // derived struct's own, in the type and on the wire).
    let mut all_members: Vec<&Member> = Vec::new();
    collect_base_members(s, structs, &mut all_members);

    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    // #A41: cross-member de-duplication after the Go name rule — two IDL members
    // that fold to the same exported name (`foo`/`Foo`, `my_field`/`myField`)
    // would otherwise emit a duplicate Go field. (The primary fix is the §7.2.3
    // frontend gate; this keeps the backend from crashing on names the frontend
    // still lets through.)
    let mut used_names: HashSet<String> = HashSet::new();
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
        let external = lowered.as_ref().is_some_and(|l| {
            l.builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::External))
        });
        for d in &m.declarators {
            let go_name = dedup_name(&mut used_names, exported(&d.name().text));
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let simple = matches!(d, Declarator::Simple(_));
            let (go_type, put, get) = match d {
                Declarator::Simple(_) => {
                    let (t, put) = map_type(
                        &resolved,
                        &format!("v.{go_name}"),
                        enum_names,
                        struct_names,
                        0,
                    )?;
                    let get = map_get(
                        &resolved,
                        &format!("v.{go_name}"),
                        enum_names,
                        struct_names,
                        0,
                    )?;
                    (t, put, get)
                }
                // Fixed array: XCDR2 marshals the elements inline, row-major, no
                // length prefix (§7.4.3.5.3).
                Declarator::Array(ad) => {
                    let sizes = ad
                        .sizes
                        .iter()
                        .map(array_size)
                        .collect::<Option<Vec<i64>>>()
                        .ok_or_else(|| {
                            IdlGoError::Unsupported(format!(
                                "non-literal array size on `{go_name}`"
                            ))
                        })?;
                    let (elem_type, elem_put) =
                        map_type(&resolved, "$elem", enum_names, struct_names, 0)?;
                    let go_type =
                        sizes.iter().map(|n| format!("[{n}]")).collect::<String>() + &elem_type;
                    let get =
                        build_array_get(&go_name, &sizes, &resolved, enum_names, struct_names)?;
                    (go_type, build_array_put(&go_name, &sizes, &elem_put), get)
                }
            };
            fields.push(FieldGen {
                go_name,
                go_type,
                put,
                get,
                id,
                key,
                resolved: resolved.clone(),
                simple,
                optional,
                must_understand,
                external,
            });
        }
    }

    let ty = escape_go_ident(&qualify(scope, &s.name.text));
    let _ = writeln!(
        out,
        "\n// {ty} is generated from the IDL struct of the same name."
    );
    let _ = writeln!(out, "type {ty} struct {{");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "\t", &s.annotations, PlacementKind::BeginDeclaration);
    for f in &fields {
        // #A20: `@external` is wire-neutral; surface it as a doc marker so the
        // annotation is not silently lost from the generated source.
        if f.external {
            let _ = writeln!(out, "\t// @external");
        }
        // An `@optional` member carries a companion presence flag (XTypes 1.3
        // §7.4.5.1.4: uint8 present-flag then the value if present).
        if f.optional {
            let _ = writeln!(out, "\t{}Present bool", f.go_name);
        }
        let _ = writeln!(out, "\t{} {}", f.go_name, f.go_type);
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "\t", &s.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}}");

    // marshalInto writes the struct into an existing writer (nested composites
    // call this so alignment stays stream-relative). @final: fields inline;
    // @appendable: a DHEADER-framed body (uint32 length + body bytes).
    let _ = writeln!(out, "\nfunc (v {ty}) marshalInto(w *Writer) {{");
    // Writes one field into writer `wv`, honoring `@optional` for the
    // final/appendable inline shapes: a Boolean presence flag (one octet,
    // XTypes 1.3 §7.4.5.1.4) then the value only if present.
    let write_put = |out: &mut String, f: &FieldGen, wv: &str| {
        let put = f.put.replace("$w", wv);
        if f.optional {
            let _ = writeln!(
                out,
                "\t{wv}.PutBool(v.{name}Present); if v.{name}Present {{ {put} }}",
                name = f.go_name
            );
        } else {
            let _ = writeln!(out, "\t{put}");
        }
    };
    match ext {
        ExtensibilityKind::Final => {
            for f in &fields {
                write_put(out, f, "w");
            }
        }
        ExtensibilityKind::Appendable => {
            let _ = writeln!(out, "\tbody := NewWriter(w.Endian)");
            for f in &fields {
                write_put(out, f, "body");
            }
            let _ = writeln!(out, "\tw.PutU32(uint32(len(body.Buf)))");
            let _ = writeln!(out, "\tw.PutBytes(body.Buf)");
        }
        // @mutable (XTypes §7.4.3.4.2): a DHEADER-framed member list; each member
        // is an EMHEADER (must-understand bit 31 when @must_understand — #A17,
        // compact length code per byte width — #A19) then its value. An
        // `@optional` member is simply OMITTED from the member list when absent
        // (no presence flag — the missing EMHEADER is the signal, §7.4.3.4.2).
        ExtensibilityKind::Mutable => {
            let _ = writeln!(out, "\tbody := NewWriter(w.Endian)");
            for f in &fields {
                if f.optional {
                    let _ = writeln!(out, "\tif v.{}Present {{", f.go_name);
                }
                write_mutable_member_encode(out, "\t", "body", f.id, f.must_understand, &f.put);
                if f.optional {
                    let _ = writeln!(out, "\t}}");
                }
            }
            let _ = writeln!(out, "\tw.PutU32(uint32(len(body.Buf)))");
            let _ = writeln!(out, "\tw.PutBytes(body.Buf)");
        }
    }
    let _ = writeln!(out, "}}");

    // MarshalXCDR encodes the struct standalone.
    let _ = writeln!(
        out,
        "\n// MarshalXCDR encodes {ty} as XCDR2 for the given wire byte order."
    );
    let _ = writeln!(
        out,
        "func (v {ty}) MarshalXCDR(endian Endianness) []byte {{"
    );
    let _ = writeln!(out, "\tw := NewWriter(endian)");
    let _ = writeln!(out, "\tv.marshalInto(w)");
    let _ = writeln!(out, "\treturn w.Bytes()");
    let _ = writeln!(out, "}}");

    // unmarshalFrom decodes the struct from a Reader (the inverse of marshalInto).
    // Reads one field, honoring `@optional` for the final/appendable inline
    // shapes: a Boolean presence flag then the value only if present.
    let write_get = |out: &mut String, f: &FieldGen| {
        if f.optional {
            let _ = writeln!(
                out,
                "\tv.{name}Present = r.GetBool(); if v.{name}Present {{ {get} }}",
                name = f.go_name,
                get = f.get
            );
        } else {
            let _ = writeln!(out, "\t{}", f.get);
        }
    };
    let _ = writeln!(out, "\nfunc unmarshalFrom{ty}(r *Reader) {ty} {{");
    let _ = writeln!(out, "\tvar v {ty}");
    match ext {
        ExtensibilityKind::Final => {
            for f in &fields {
                write_get(out, f);
            }
        }
        ExtensibilityKind::Appendable => {
            let _ = writeln!(out, "\t_ = r.GetU32() // DHEADER");
            for f in &fields {
                write_get(out, f);
            }
        }
        // NOTE (honest scope): this positional @mutable decoder assumes every
        // member is present in id order. It rides the pre-existing naive
        // decoder — an `@optional` member that was OMITTED on encode is NOT
        // detected here (no EMHEADER-driven skip), so absent mutable-optional
        // members do NOT round-trip. A fully-present mutable value does: the
        // presence flag is set so re-encode reproduces the member.
        ExtensibilityKind::Mutable => {
            let _ = writeln!(out, "\t_ = r.GetU32() // DHEADER");
            for f in &fields {
                if f.optional {
                    let _ = writeln!(out, "\tv.{}Present = true", f.go_name);
                }
                write_mutable_member_decode(out, "\t", &f.get);
            }
        }
    }
    let _ = writeln!(out, "\treturn v");
    let _ = writeln!(out, "}}");

    // UnmarshalXCDR decodes {ty} from XCDR2 bytes for the given wire byte order.
    let _ = writeln!(
        out,
        "\nfunc UnmarshalXCDR{ty}(buf []byte, endian Endianness) {ty} {{"
    );
    let _ = writeln!(out, "\treturn unmarshalFrom{ty}(NewReader(buf, endian))");
    let _ = writeln!(out, "}}");

    // KeyHash (XTypes §7.6.8): @key members serialized PLAIN_CDR2-BE (member-id
    // order). ≤16-byte KeyHolder → those bytes zero-padded to 16; larger (or
    // dynamically sized) → MD5(bytes)[0..16] (step 5.2). The branch is static
    // per type, decided by the shared max-size analysis.
    let mut keys: Vec<&FieldGen> = fields.iter().filter(|f| f.key).collect();
    keys.sort_by_key(|f| f.id);
    if !keys.is_empty() {
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
        let _ = writeln!(out, "\n// KeyHash returns the 16-byte XTypes KeyHash.");
        let _ = writeln!(out, "func (v {ty}) KeyHash() [16]byte {{");
        let _ = writeln!(out, "\tkw := NewWriter(Big)");
        for f in &keys {
            // Bug A: a `@key` member whose (typedef-dealiased) type is itself a
            // struct must expand to ONLY that struct's own `@key` members (or
            // ALL its members if it declares none), in member-id order — not
            // the struct's full member set. `f.put` reuses the generic
            // per-field mapper, which is correct for normal (non-key) struct
            // encoding but always encodes the FULL member set, so it must not
            // be used here for a nested-struct key.
            let nested_struct = if f.simple {
                match &f.resolved {
                    TypeSpec::Scoped(sn) => structs.get(&resolve_scoped_name(sn)).copied(),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(sd) = nested_struct {
                emit_key_struct_member(
                    out,
                    sd,
                    &format!("v.{}", f.go_name),
                    enum_names,
                    struct_names,
                    typedefs,
                    structs,
                )?;
            } else {
                let _ = writeln!(out, "\t{}", f.put.replace("$w", "kw"));
            }
        }
        if use_md5 {
            let _ = writeln!(out, "\treturn md5.Sum(kw.Buf)");
        } else {
            let _ = writeln!(out, "\tvar out [16]byte");
            let _ = writeln!(out, "\tcopy(out[:], kw.Buf)");
            let _ = writeln!(out, "\treturn out");
        }
        let _ = writeln!(out, "}}");
    }
    Ok(())
}

/// Emits KeyHash key-writer puts for a nested-struct `@key` member: expands
/// to `sd`'s own `@key` members (or ALL members if it declares none —
/// XTypes 1.3 §7.6.8), in member-id order, recursing again if one of those
/// members is itself a nested struct. Mirrors `idl-rust`'s
/// `emit_key_field_write` (see `crates/idl-rust/src/struct_emit.rs`).
/// zerodds-lint: recursion-depth 16 (nested `@key` struct expansion; bounded
/// by the IDL's aggregate nesting depth).
fn emit_key_struct_member(
    out: &mut String,
    sd: &StructDef,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    structs: &HashMap<String, &StructDef>,
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
                return Err(IdlGoError::Unsupported(
                    "array @key field inside a nested-struct key".to_string(),
                ));
            }
            let field = exported(&d.name().text);
            let nested_expr = format!("{expr}.{field}");
            if let TypeSpec::Scoped(sn) = &resolved {
                let name = resolve_scoped_name(sn);
                if let Some(nested_sd) = structs.get(&name) {
                    emit_key_struct_member(
                        out,
                        nested_sd,
                        &nested_expr,
                        enum_names,
                        struct_names,
                        typedefs,
                        structs,
                    )?;
                    continue;
                }
            }
            let (_, put) = map_type(&resolved, &nested_expr, enum_names, struct_names, 0)?;
            let _ = writeln!(out, "\t{}", put.replace("$w", "kw"));
        }
    }
    Ok(())
}

/// Maps an IDL type to `(Go type, put statement)`. The put statement uses `$w`
/// as the writer placeholder and `expr` as the value expression.
///
/// zerodds-lint: recursion-depth 32 (via `map_sequence` for `sequence<T>`;
/// bounded by nested collection depth in the IDL).
/// A type is "primitive" for the map-DHEADER rule if it is fully descriptive on
/// the wire: an IDL primitive or an enum (i32). Strings/sequences/maps/structs
/// are not, and force a collection DHEADER on a map.
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

/// `true` if `name` resolves to a `bitset`/`bitmask` declaration (its wire form
/// is a single backing integer — fully descriptive, no collection DHEADER).
fn is_bit_name(name: &str) -> bool {
    BIT_NAMES.with(|b| b.borrow().contains(name))
}

/// Builds a map put: collect keys, sort ascending, then `u32 count` + key/value
/// pairs (DHEADER-framed unless the pair is primitive). `key_put` references
/// `zdK{depth}`; `val_put` references `<expr>[zdK{depth}]`; both use `$w`. The
/// `depth` suffix keeps a `map<K, map<K2,V>>`'s inner key/keys/sub-writer temps
/// distinct from the outer's, so the inner value expression is not shadowed
/// (#A21/P9 encode side).
fn build_map_put(
    expr: &str,
    key_type: &str,
    key_put: &str,
    val_put: &str,
    prim: bool,
    bound: Option<i64>,
    depth: usize,
) -> String {
    let bound_check = match bound {
        // B1 follow-up (#22 decode-side parity template applied to encode too
        // where idl-go had no enforcement at all): mirror the check pattern
        // used by every other backend this session — XTypes 1.3 §7.4.3.
        Some(bv) => format!(
            "if len({expr}) > {bv} {{ panic(\"encoded map length exceeds its IDL bound ({bv})\") }}; "
        ),
        None => String::new(),
    };
    let prelude = format!(
        "{bound_check}zdKeys{depth} := make([]{key_type}, 0, len({expr})); \
         for zdK{depth} := range {expr} {{ zdKeys{depth} = append(zdKeys{depth}, zdK{depth}) }}; \
         sort.Slice(zdKeys{depth}, func(zi, zj int) bool {{ return zdKeys{depth}[zi] < zdKeys{depth}[zj] }});"
    );
    if prim {
        format!(
            "{{ {prelude} $w.PutU32(uint32(len(zdKeys{depth}))); \
             for _, zdK{depth} := range zdKeys{depth} {{ {key_put}; {val_put} }} }}"
        )
    } else {
        let kp = key_put.replace("$w", &format!("zdSub{depth}"));
        let vp = val_put.replace("$w", &format!("zdSub{depth}"));
        format!(
            "{{ {prelude} zdSub{depth} := NewWriter($w.Endian); \
             zdSub{depth}.PutU32(uint32(len(zdKeys{depth}))); \
             for _, zdK{depth} := range zdKeys{depth} {{ {kp}; {vp} }}; \
             $w.PutU32(uint32(len(zdSub{depth}.Buf))); $w.PutBytes(zdSub{depth}.Buf) }}"
        )
    }
}

/// Maps an IDL type to `(Go type, put statement)`. The put statement uses `$w`
/// as the writer placeholder and `expr` as the value expression. `depth` is the
/// dynamic-collection nesting level, so nested `sequence`/`map` temporaries do
/// not collide (#A21/P9).
/// zerodds-lint: recursion-depth 32 (via `map_sequence` for `sequence<T>`;
/// bounded by nested collection depth in the IDL).
fn map_type(
    t: &TypeSpec,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    depth: usize,
) -> Result<(String, String)> {
    match t {
        TypeSpec::Primitive(p) => map_primitive(*p, expr),
        // Bounded narrow `string<N>` (DDS-XTypes §7.4.3): Go strings are byte
        // sequences, so `len(s)` is already the CDR wire length — reject
        // over-bound on encode like strict vendors do.
        TypeSpec::String(st) if !st.wide => {
            let put = match st.bound.as_ref().and_then(array_size) {
                Some(bv) => format!(
                    "if len({expr}) > {bv} {{ panic(\"encoded string length exceeds its IDL bound ({bv})\") }}; $w.PutString({expr})"
                ),
                None => format!("$w.PutString({expr})"),
            };
            Ok(("string".to_string(), put))
        }
        // wstring: u32 octet-length (2·units, no BOM) + UTF-16 code units.
        // Bounded `wstring<N>`: N is a UTF-16 unit count (wstringUnitLen).
        TypeSpec::String(st) => {
            let put = match st.bound.as_ref().and_then(array_size) {
                Some(bv) => format!(
                    "if wstringUnitLen({expr}) > {bv} {{ panic(\"encoded wstring length exceeds its IDL bound ({bv})\") }}; $w.PutWString({expr})"
                ),
                None => format!("$w.PutWString({expr})"),
            };
            Ok(("string".to_string(), put))
        }
        TypeSpec::Sequence(seq) => map_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            expr,
            enum_names,
            struct_names,
            depth,
        ),
        // A map: entries sorted ascending by key (matches the BTreeMap reference),
        // `u32 count` + key/value pairs. A primitive key/value pair carries no
        // collection DHEADER; otherwise the count+pairs are DHEADER-framed.
        TypeSpec::Map(m) => {
            let (key_type, key_put) = map_type(
                &m.key,
                &format!("zdK{depth}"),
                enum_names,
                struct_names,
                depth + 1,
            )?;
            let (val_type, val_put) = map_type(
                &m.value,
                &format!("{expr}[zdK{depth}]"),
                enum_names,
                struct_names,
                depth + 1,
            )?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let go_type = format!("map[{key_type}]{val_type}");
            Ok((
                go_type,
                build_map_put(
                    expr,
                    &key_type,
                    &key_put,
                    &val_put,
                    prim,
                    m.bound.as_ref().and_then(array_size),
                    depth,
                ),
            ))
        }
        // A `fixed<P,S>` decimal: packed BCD, `(P+2)/2` raw octets, no length
        // prefix and no alignment (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5). The
        // Go field holds the BCD bytes directly; `zdFixedEnc` builds them from a
        // decimal string.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let _ = fixed_ps(f)?; // validate P/S resolve at codegen time
            Ok(("[]byte".to_string(), format!("$w.PutBytes({expr})")))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Enum member: signed integer whose width follows @bit_bound
                // (XTypes 1.3 §7.4.5.1); 1/2/4 octets. Go value is int32-based;
                // the narrow cast truncates the low octets (two's complement).
                let put = match enum_wire_width(&name) {
                    1 => format!("$w.PutU8(uint8({expr}))"),
                    2 => format!("$w.PutU16(uint16({expr}))"),
                    _ => format!("$w.PutU32(uint32({expr}))"),
                };
                Ok((exported(&name), put))
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                // Nested struct / bit-container member: marshal into the writer.
                Ok((escape_go_ident(&name), format!("{expr}.marshalInto($w)")))
            } else {
                Err(IdlGoError::Unsupported(format!("scoped type {name}")))
            }
        }
        other => Err(IdlGoError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_primitive(p: PrimitiveType, expr: &str) -> Result<(String, String)> {
    let (go_type, put) = match p {
        PrimitiveType::Octet => ("byte", format!("$w.PutU8({expr})")),
        PrimitiveType::Boolean => ("bool", format!("$w.PutBool({expr})")),
        PrimitiveType::Char => ("byte", format!("$w.PutU8({expr})")),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => ("float32", format!("$w.PutF32({expr})")),
        PrimitiveType::Floating(FloatingType::Double) => ("float64", format!("$w.PutF64({expr})")),
        // long double: IEEE binary128, widened from the float64 value.
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            ("float64", format!("$w.PutLongDouble({expr})"))
        }
        // wchar: a wchar32 (UTF-32 code point, 4 bytes) — the established
        // ZeroDDS reference wire (`endpoints/golden-gen` `write_u32`, the go
        // endpoint core, and every thin backend). #A1 (a narrower 2-byte
        // UTF-16 `wchar`) is a cross-backend wire change that must move the
        // golden reference and all backends together; done in isolation here it
        // would break go's byte-identity vs the golden. See the honest-scope note.
        PrimitiveType::WideChar => ("rune", format!("$w.PutU32(uint32({expr}))")),
    };
    Ok((go_type.to_string(), put))
}

fn map_integer(i: IntegerType, expr: &str) -> Result<(String, String)> {
    let (go_type, put) = match i {
        IntegerType::Int8 => ("int8", format!("$w.PutU8(byte({expr}))")),
        IntegerType::UInt8 => ("uint8", format!("$w.PutU8({expr})")),
        IntegerType::Short | IntegerType::Int16 => ("int16", format!("$w.PutU16(uint16({expr}))")),
        IntegerType::UShort | IntegerType::UInt16 => ("uint16", format!("$w.PutU16({expr})")),
        IntegerType::Long | IntegerType::Int32 => ("int32", format!("$w.PutU32(uint32({expr}))")),
        IntegerType::ULong | IntegerType::UInt32 => ("uint32", format!("$w.PutU32({expr})")),
        IntegerType::LongLong | IntegerType::Int64 => {
            ("int64", format!("$w.PutU64(uint64({expr}))"))
        }
        IntegerType::ULongLong | IntegerType::UInt64 => ("uint64", format!("$w.PutU64({expr})")),
    };
    Ok((go_type.to_string(), put))
}

/// zerodds-lint: recursion-depth 32 (calls `map_type` for the element type;
/// bounded by nested collection depth in the IDL).
fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    depth: usize,
) -> Result<(String, String)> {
    // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = encode error,
    // like every other backend this session. Checked once up front — shared
    // by all three wire shapes below (byte-slice fast path, struct-element
    // DHEADER path, primitive-element path).
    let bound_check = match bound.and_then(array_size) {
        Some(bv) => format!(
            "if len({expr}) > {bv} {{ panic(\"encoded sequence length exceeds its IDL bound ({bv})\") }}; "
        ),
        None => String::new(),
    };
    // sequence<octet> / sequence<uint8> → a byte slice with the fast path.
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok((
            "[]byte".to_string(),
            format!("{bound_check}$w.PutSeqU8({expr})"),
        ));
    }
    // sequence<struct> → a collection DHEADER (uint32 body length) wrapping
    // uint32 count + each element (XTypes 1.3 §7.4.3.5.3, non-primitive elem).
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let put = format!(
                "{{ {bound_check}sub := NewWriter($w.Endian); sub.PutU32(uint32(len({expr}))); \
                 for _, e := range {expr} {{ e.marshalInto(sub) }}; \
                 $w.PutU32(uint32(len(sub.Buf))); $w.PutBytes(sub.Buf) }}"
            );
            return Ok((format!("[]{}", escape_go_ident(&name)), put));
        }
    }
    // sequence<primitive> → u32 count + per-element encode. `for _, e := range`
    // rebinds `e` at each nesting level, so `sequence<sequence<T>>` needs no
    // suffix for `e`; a nested `map` element gets a deeper `depth` for its own
    // temporaries (#A21/P9).
    let (elem_go, elem_put) = map_type(elem, "e", enum_names, struct_names, depth + 1)?;
    let put = format!(
        "{bound_check}$w.PutU32(uint32(len({expr}))); for _, e := range {expr} {{ {elem_put} }}"
    );
    Ok((format!("[]{elem_go}"), put))
}

/// The inverse of [`map_type`]: a Go statement that reads a value from `r` and
/// assigns it to `target` (the decode side of the wire mapping). `depth` is the
/// dynamic-collection nesting level, so nested `sequence`/`map` loop and
/// temporary names do not collide/shadow (#A21/P9).
/// zerodds-lint: recursion-depth 32
fn map_get(
    t: &TypeSpec,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    depth: usize,
) -> Result<String> {
    match t {
        TypeSpec::Primitive(p) => Ok(map_get_primitive(*p, target)),
        // B1 follow-up (#22 decode-side parity): mirror the encode-side bound
        // check on decode too — XTypes 1.3 §7.4.3 requires enforcement on
        // BOTH sides; `r.GetString()`/`r.GetWString()` only ever validated
        // the wire's remaining bytes, never the IDL-declared bound.
        TypeSpec::String(st) if !st.wide => Ok(match st.bound.as_ref().and_then(array_size) {
            Some(bv) => format!(
                "{target} = r.GetString(); if len({target}) > {bv} {{ panic(\"decoded string length exceeds its IDL bound ({bv})\") }}"
            ),
            None => format!("{target} = r.GetString()"),
        }),
        TypeSpec::String(st) => Ok(match st.bound.as_ref().and_then(array_size) {
            Some(bv) => format!(
                "{target} = r.GetWString(); if wstringUnitLen({target}) > {bv} {{ panic(\"decoded wstring length exceeds its IDL bound ({bv})\") }}"
            ),
            None => format!("{target} = r.GetWString()"),
        }),
        TypeSpec::Sequence(seq) => map_get_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            target,
            enum_names,
            struct_names,
            depth,
        ),
        TypeSpec::Map(m) => map_get_map(m, target, enum_names, struct_names, depth),
        // `fixed<P,S>`: read the statically-known `(P+2)/2` BCD octets (copied
        // out of the reader's buffer so the value owns its bytes).
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let (p, _) = fixed_ps(f)?;
            let n = (p + 2) / 2;
            Ok(format!(
                "{target} = append([]byte(nil), r.GetBytesN({n})...)"
            ))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Read the @bit_bound-wide signed holder and sign-extend to the
                // enum's int32 domain (XTypes 1.3 §7.4.5.1).
                let get = match enum_wire_width(&name) {
                    1 => format!("{target} = {}(int32(int8(r.GetU8())))", exported(&name)),
                    2 => format!("{target} = {}(int32(int16(r.GetU16())))", exported(&name)),
                    _ => format!("{target} = {}(int32(r.GetU32()))", exported(&name)),
                };
                Ok(get)
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                Ok(format!(
                    "{target} = unmarshalFrom{}(r)",
                    escape_go_ident(&name)
                ))
            } else {
                Err(IdlGoError::Unsupported(format!("scoped type {name}")))
            }
        }
        other => Err(IdlGoError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType, target: &str) -> String {
    match p {
        PrimitiveType::Octet => format!("{target} = r.GetU8()"),
        PrimitiveType::Boolean => format!("{target} = r.GetBool()"),
        PrimitiveType::Char => format!("{target} = r.GetU8()"),
        PrimitiveType::Integer(i) => map_get_integer(i, target),
        PrimitiveType::Floating(FloatingType::Float) => format!("{target} = r.GetF32()"),
        PrimitiveType::Floating(FloatingType::Double) => format!("{target} = r.GetF64()"),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            format!("{target} = r.GetLongDouble()")
        }
        // wchar: a wchar32 (UTF-32 code point, 4 bytes) — matches the golden
        // reference; see the encode-side note (#A1 deferred to a coordinated
        // cross-backend wire change).
        PrimitiveType::WideChar => format!("{target} = rune(r.GetU32())"),
    }
}

fn map_get_integer(i: IntegerType, target: &str) -> String {
    match i {
        IntegerType::UInt8 => format!("{target} = r.GetU8()"),
        IntegerType::Int8 => format!("{target} = int8(r.GetU8())"),
        IntegerType::UShort | IntegerType::UInt16 => format!("{target} = r.GetU16()"),
        IntegerType::Short | IntegerType::Int16 => format!("{target} = int16(r.GetU16())"),
        IntegerType::ULong | IntegerType::UInt32 => format!("{target} = r.GetU32()"),
        IntegerType::Long | IntegerType::Int32 => format!("{target} = int32(r.GetU32())"),
        IntegerType::ULongLong | IntegerType::UInt64 => format!("{target} = r.GetU64()"),
        IntegerType::LongLong | IntegerType::Int64 => format!("{target} = int64(r.GetU64())"),
    }
}

/// zerodds-lint: recursion-depth 32
fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    depth: usize,
) -> Result<String> {
    // B1 follow-up (#22 decode-side parity): mirror the encode-side bound
    // check — XTypes 1.3 §7.4.3.
    let bv = bound.and_then(array_size);
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok(match bv {
            Some(bv) => format!(
                "{target} = r.GetSeqU8(); if len({target}) > {bv} {{ panic(\"decoded sequence length exceeds its IDL bound ({bv})\") }}"
            ),
            None => format!("{target} = r.GetSeqU8()"),
        });
    }
    // #A21/P9: suffix the count/loop-index with `depth` and read the element into
    // `{target}[zi{depth}]`, so a `sequence<sequence<T>>` decode does not shadow
    // the outer index (the former literal `i`/`zdn` panicked / corrupted).
    let idx = format!("zi{depth}");
    let cnt = format!("zdn{depth}");
    let (elem_go, _) = map_type(elem, "e", enum_names, struct_names, depth)?;
    let elem_get = map_get(
        elem,
        &format!("{target}[{idx}]"),
        enum_names,
        struct_names,
        depth + 1,
    )?;
    // sequence<struct> is DHEADER-framed; sequence<primitive> is not.
    let dheader = if matches!(elem, TypeSpec::Scoped(sn)
        if struct_names.contains(&resolve_scoped_name(sn)))
    {
        "_ = r.GetU32(); "
    } else {
        ""
    };
    let bound_check = match bv {
        Some(bv) => format!(
            "if {cnt} > {bv} {{ panic(\"decoded sequence length exceeds its IDL bound ({bv})\") }}; "
        ),
        None => String::new(),
    };
    Ok(format!(
        "{{ {dheader}{cnt} := int(r.GetU32()); {bound_check}{target} = make([]{elem_go}, {cnt}); \
         for {idx} := 0; {idx} < {cnt}; {idx}++ {{ {elem_get} }} }}"
    ))
}

/// zerodds-lint: recursion-depth 32
fn map_get_map(
    m: &zerodds_idl::ast::types::MapType,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    depth: usize,
) -> Result<String> {
    // #A21/P9: suffix every temporary (`zk`/`zv`/`zdn`/`zi`) with `depth` so a
    // `map<K, map<K2,V>>` decode's inner key/value do not shadow the outer's
    // (the former literal `zk`/`zv` produced `zv[zk] = zv`, a Go type error).
    let idx = format!("zi{depth}");
    let cnt = format!("zdn{depth}");
    let zk = format!("zk{depth}");
    let zv = format!("zv{depth}");
    let (key_go, _) = map_type(&m.key, "e", enum_names, struct_names, depth)?;
    let (val_go, _) = map_type(&m.value, "e", enum_names, struct_names, depth)?;
    let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
    let key_get = map_get(&m.key, &zk, enum_names, struct_names, depth + 1)?;
    let val_get = map_get(&m.value, &zv, enum_names, struct_names, depth + 1)?;
    let dheader = if prim { "" } else { "_ = r.GetU32(); " };
    // B1 follow-up (#22 decode-side parity): mirror the encode-side bound
    // check — XTypes 1.3 §7.4.3.
    let bound_check = match m.bound.as_ref().and_then(array_size) {
        Some(bv) => format!(
            "if {cnt} > {bv} {{ panic(\"decoded map length exceeds its IDL bound ({bv})\") }}; "
        ),
        None => String::new(),
    };
    Ok(format!(
        "{{ {dheader}{cnt} := int(r.GetU32()); {bound_check}{target} = make(map[{key_go}]{val_go}, {cnt}); \
         for {idx} := 0; {idx} < {cnt}; {idx}++ {{ var {zk} {key_go}; var {zv} {val_go}; {key_get}; {val_get}; \
         {target}[{zk}] = {zv} }} }}"
    ))
}

/// The inverse of [`build_array_put`]: nested index loops reading each element.
/// Array dimensions use `i{k}`; the element's own collection temporaries start
/// at collection-depth 0 with distinct `zi`/`zdn`/`zk`/`zv` names, so they never
/// collide with the `i{k}` array indices.
fn build_array_get(
    field: &str,
    sizes: &[i64],
    elem: &TypeSpec,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    let idx: String = (0..sizes.len()).map(|k| format!("[i{k}]")).collect();
    let mut body = map_get(
        elem,
        &format!("v.{field}{idx}"),
        enum_names,
        struct_names,
        0,
    )?;
    for k in (0..sizes.len()).rev() {
        body = format!(
            "for i{k} := 0; i{k} < {}; i{k}++ {{\n\t\t{body}\n\t}}",
            sizes[k]
        );
    }
    Ok(body)
}
