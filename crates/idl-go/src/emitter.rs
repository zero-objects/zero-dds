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
    CaseLabel, ConstExpr, ConstrTypeDecl, Declarator, Definition, EnumDef, FloatingType,
    IntegerType, Literal, LiteralKind, Member, PrimitiveType, SequenceType, Specification,
    StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnaryOp, UnionDcl, UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, lower_annotations, lower_single,
};

use crate::error::{IdlGoError, Result};
use crate::keywords::escape_go_ident;

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
const WIRE_PRELUDE: &str = r#"import "math"

// Endianness is the wire byte order (the XCDR encapsulation flag, not the host).
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

/// Generates a self-contained Go module from the IDL AST.
///
/// # Errors
/// Returns [`IdlGoError::Unsupported`] for constructs the Go backend does not
/// yet emit (unions, nested struct members, maps, `long double`, `@mutable`, …).
pub fn generate_go_module(spec: &Specification, opts: &GoGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Code generated by zerodds-idlc (Go backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "// SPDX-License-Identifier: Apache-2.0");
    let _ = writeln!(out, "package {}\n", opts.package_name);
    out.push_str(WIRE_PRELUDE);

    // `module X { ... }` content is promoted into the same flat, top-level
    // definition list (see `flatten_module_defs`) so it is no longer
    // silently dropped (swarm59 #21b).
    let flat = flatten_module_defs(&spec.definitions);

    // Named enums referenced by struct members: an enum member is a 32-bit
    // signed integer on the wire (XTypes 1.3 §7.4.5.1), byte-identical to the
    // int32/uint32 path.
    let enum_names: HashSet<String> = flat
        .iter()
        .filter_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                Some(e.name.text.clone())
            }
            _ => None,
        })
        .collect();

    // Named structs referenced as members (nested composites).
    let struct_names: HashSet<String> = flat
        .iter()
        .filter_map(|d| match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                Some(s.name.text.clone())
            }
            _ => None,
        })
        .collect();

    // typedef name → aliased type-spec (wire-transparent; resolved before mapping).
    let typedefs = collect_typedefs(spec);
    // struct name → def, so a nested-struct `@key` member's own `@key` subset
    // can be resolved for KeyHash emission (Bug A) and for the static
    // MD5-vs-zero-pad branch decision (Bug B) — mirrors `collect_typedefs`.
    let structs = collect_structs(spec);

    for def in &flat {
        match def {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => emit_enum(&mut out, e),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                emit_struct(&mut out, s, &enum_names, &struct_names, &typedefs, &structs)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                emit_union(&mut out, u, &enum_names, &struct_names, &typedefs)?;
            }
            _ => {}
        }
    }
    // On-demand imports (Go rejects unused ones): `sort` for a map put's
    // `sort.Slice`, `crypto/md5` for the KeyHash MD5 branch.
    let mut extra: Vec<&str> = Vec::new();
    if out.contains("sort.Slice") {
        extra.push("\t\"sort\"\n");
    }
    if out.contains("md5.Sum(") {
        extra.push("\t\"crypto/md5\"\n");
    }
    if !extra.is_empty() {
        let block = format!("import (\n\t\"math\"\n{})\n", extra.concat());
        out = out.replacen("import \"math\"\n", &block, 1);
    }
    Ok(out)
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

/// Emits an IDL `union` as a Go struct holding the discriminator + one field per
/// case member, plus a `marshalInto` that puts the discriminator then switches
/// on it to marshal the selected member (XCDR2 §7.4.3.5.4). `@final`: inline;
/// `@appendable`: a DHEADER-framed body.
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
        return Err(IdlGoError::Unsupported(format!(
            "@mutable union {} (EMHEADER framing not yet emitted)",
            u.name.text
        )));
    }

    let disc_ts = switch_typespec(&u.switch_type);
    let (disc_type, disc_put) = map_type(&disc_ts, "v.Disc", enum_names, struct_names)?;
    let disc_get = map_get(&disc_ts, "v.Disc", enum_names, struct_names)?;

    let mut cases: Vec<UnionCase> = Vec::new();
    for c in &u.cases {
        let field = exported(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (go_type, put) = map_type(&resolved, &format!("v.{field}"), enum_names, struct_names)?;
        let get = map_get(&resolved, &format!("v.{field}"), enum_names, struct_names)?;
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => labels.push(array_size(e).ok_or_else(|| {
                    IdlGoError::Unsupported(format!("non-integer union label in `{}`", u.name.text))
                })?),
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

    let ty = escape_go_ident(&u.name.text);
    let _ = writeln!(
        out,
        "\n// {ty} is generated from the IDL union of the same name."
    );
    let _ = writeln!(out, "type {ty} struct {{");
    let _ = writeln!(out, "\tDisc {disc_type}");
    for c in &cases {
        let _ = writeln!(out, "\t{} {}", c.field, c.go_type);
    }
    let _ = writeln!(out, "}}");

    let _ = writeln!(out, "\nfunc (v {ty}) marshalInto(w *Writer) {{");
    let wv = if ext == ExtensibilityKind::Final {
        "w"
    } else {
        let _ = writeln!(out, "\tbody := NewWriter(w.Endian)");
        "body"
    };
    let _ = writeln!(out, "\t{}", disc_put.replace("$w", wv));
    let _ = writeln!(out, "\tswitch v.Disc {{");
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "\tdefault:");
        } else {
            let labels = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "\tcase {labels}:");
        }
        let _ = writeln!(out, "\t\t{}", c.put.replace("$w", wv));
    }
    let _ = writeln!(out, "\t}}");
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "\tw.PutU32(uint32(len(body.Buf)))");
        let _ = writeln!(out, "\tw.PutBytes(body.Buf)");
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
    // member (the inverse of marshalInto).
    let _ = writeln!(out, "\nfunc unmarshalFrom{ty}(r *Reader) {ty} {{");
    let _ = writeln!(out, "\tvar v {ty}");
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "\t_ = r.GetU32() // DHEADER");
    }
    let _ = writeln!(out, "\t{disc_get}");
    let _ = writeln!(out, "\tswitch v.Disc {{");
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "\tdefault:");
        } else {
            let labels = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "\tcase {labels}:");
        }
        let _ = writeln!(out, "\t\t{}", c.get);
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
fn emit_enum(out: &mut String, e: &EnumDef) {
    let values = enumerator_values(e);
    let ty = exported(&e.name.text);
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

/// Extensibility of a struct (defaults to `@appendable`, the post-SX2 default).
fn extensibility(s: &StructDef) -> ExtensibilityKind {
    lower_annotations(&s.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable)
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

/// Recursively descends into `Definition::Module`, returning every
/// non-module definition (struct/enum/union/typedef/…) in document order.
/// The IDL AST builder already merges a reopened `module M {} ... module
/// M {}` into one AST node (`crates/idl/src/ast/builder.rs`); this promotes
/// a module's members into the same flat namespace this backend already
/// uses for type-reference resolution (`sn.parts.last()` in `map_type`/
/// `map_get` below) — module content is no longer silently dropped
/// (swarm59 #21b), it is simply not namespaced: two same-named types in
/// different modules collide, exactly as two same-named top-level types
/// would. Go has no nested-type-declaration construct that this backend's
/// existing single-flat-namespace reference resolution could target, so
/// this is the correctness-preserving fix rather than a per-module prefix.
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

/// Collects `typedef` aliases (simple declarators) as name → aliased type-spec.
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
/// §7.6.8) for KeyHash emission and for the static max-size (MD5 vs.
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
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    structs: &HashMap<String, &StructDef>,
) -> Result<()> {
    let ext = extensibility(s);

    struct FieldGen {
        go_name: String,
        go_type: String,
        put: String, // statement operating on writer `$w`, referencing v.<go_name>
        get: String, // decode statement reading from `r` into v.<go_name>
        id: u32,     // XTypes member id (@id(n) or sequential)
        key: bool,   // @key member
        resolved: TypeSpec, // typedef-dealiased type of this field
        simple: bool, // true for a `Declarator::Simple` (not a fixed array)
    }
    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    for m in &s.members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        for d in &m.declarators {
            let go_name = exported(&d.name().text);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let simple = matches!(d, Declarator::Simple(_));
            let (go_type, put, get) = match d {
                Declarator::Simple(_) => {
                    let (t, put) =
                        map_type(&resolved, &format!("v.{go_name}"), enum_names, struct_names)?;
                    let get =
                        map_get(&resolved, &format!("v.{go_name}"), enum_names, struct_names)?;
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
                        map_type(&resolved, "$elem", enum_names, struct_names)?;
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
            });
        }
    }

    let ty = escape_go_ident(&s.name.text);
    let _ = writeln!(
        out,
        "\n// {ty} is generated from the IDL struct of the same name."
    );
    let _ = writeln!(out, "type {ty} struct {{");
    for f in &fields {
        let _ = writeln!(out, "\t{} {}", f.go_name, f.go_type);
    }
    let _ = writeln!(out, "}}");

    // marshalInto writes the struct into an existing writer (nested composites
    // call this so alignment stays stream-relative). @final: fields inline;
    // @appendable: a DHEADER-framed body (uint32 length + body bytes).
    let _ = writeln!(out, "\nfunc (v {ty}) marshalInto(w *Writer) {{");
    match ext {
        ExtensibilityKind::Final => {
            for f in &fields {
                let _ = writeln!(out, "\t{}", f.put.replace("$w", "w"));
            }
        }
        ExtensibilityKind::Appendable => {
            let _ = writeln!(out, "\tbody := NewWriter(w.Endian)");
            for f in &fields {
                let _ = writeln!(out, "\t{}", f.put.replace("$w", "body"));
            }
            let _ = writeln!(out, "\tw.PutU32(uint32(len(body.Buf)))");
            let _ = writeln!(out, "\tw.PutBytes(body.Buf)");
        }
        // @mutable (XTypes §7.4.3.4.2): a DHEADER-framed member list; each member
        // is an EMHEADER (LC4 = member id) + NEXTINT (body length) + body.
        ExtensibilityKind::Mutable => {
            let _ = writeln!(out, "\tbody := NewWriter(w.Endian)");
            for f in &fields {
                let emh = 0x4000_0000_u32 | f.id;
                let _ = writeln!(out, "\tbody.PutU32(0x{emh:08x})");
                let _ = writeln!(out, "\t{{");
                let _ = writeln!(out, "\t\tmem := NewWriter(w.Endian)");
                let _ = writeln!(out, "\t\t{}", f.put.replace("$w", "mem"));
                let _ = writeln!(out, "\t\tbody.PutU32(uint32(len(mem.Buf)))");
                let _ = writeln!(out, "\t\tbody.PutBytes(mem.Buf)");
                let _ = writeln!(out, "\t}}");
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
    let _ = writeln!(out, "\nfunc unmarshalFrom{ty}(r *Reader) {ty} {{");
    let _ = writeln!(out, "\tvar v {ty}");
    match ext {
        ExtensibilityKind::Final => {
            for f in &fields {
                let _ = writeln!(out, "\t{}", f.get);
            }
        }
        ExtensibilityKind::Appendable => {
            let _ = writeln!(out, "\t_ = r.GetU32() // DHEADER");
            for f in &fields {
                let _ = writeln!(out, "\t{}", f.get);
            }
        }
        ExtensibilityKind::Mutable => {
            let _ = writeln!(out, "\t_ = r.GetU32() // DHEADER");
            for f in &fields {
                let _ = writeln!(out, "\t_ = r.GetU32() // EMHEADER");
                let _ = writeln!(out, "\t_ = r.GetU32() // NEXTINT");
                let _ = writeln!(out, "\t{}", f.get);
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
                    TypeSpec::Scoped(sn) => {
                        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
                        structs.get(&name).copied()
                    }
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
                let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
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
            let (_, put) = map_type(&resolved, &nested_expr, enum_names, struct_names)?;
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
            enum_names.contains(&sn.parts.last().map(|p| p.text.clone()).unwrap_or_default())
        }
        _ => false,
    }
}

/// Builds a map put: collect keys, sort ascending, then `u32 count` + key/value
/// pairs (DHEADER-framed unless the pair is primitive). `key_put` references
/// `zdK`; `val_put` references `<expr>[zdK]`; both use `$w`.
fn build_map_put(
    expr: &str,
    key_type: &str,
    key_put: &str,
    val_put: &str,
    prim: bool,
    bound: Option<i64>,
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
        "{bound_check}zdKeys := make([]{key_type}, 0, len({expr})); \
         for zdK := range {expr} {{ zdKeys = append(zdKeys, zdK) }}; \
         sort.Slice(zdKeys, func(zi, zj int) bool {{ return zdKeys[zi] < zdKeys[zj] }});"
    );
    if prim {
        format!(
            "{{ {prelude} $w.PutU32(uint32(len(zdKeys))); \
             for _, zdK := range zdKeys {{ {key_put}; {val_put} }} }}"
        )
    } else {
        let kp = key_put.replace("$w", "zdSub");
        let vp = val_put.replace("$w", "zdSub");
        format!(
            "{{ {prelude} zdSub := NewWriter($w.Endian); \
             zdSub.PutU32(uint32(len(zdKeys))); \
             for _, zdK := range zdKeys {{ {kp}; {vp} }}; \
             $w.PutU32(uint32(len(zdSub.Buf))); $w.PutBytes(zdSub.Buf) }}"
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
        ),
        // A map: entries sorted ascending by key (matches the BTreeMap reference),
        // `u32 count` + key/value pairs. A primitive key/value pair carries no
        // collection DHEADER; otherwise the count+pairs are DHEADER-framed.
        TypeSpec::Map(m) => {
            let (key_type, key_put) = map_type(&m.key, "zdK", enum_names, struct_names)?;
            let (val_type, val_put) =
                map_type(&m.value, &format!("{expr}[zdK]"), enum_names, struct_names)?;
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
                ),
            ))
        }
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                // Enum member: 32-bit signed integer on the wire.
                Ok((exported(&name), format!("$w.PutU32(uint32({expr}))")))
            } else if struct_names.contains(&name) {
                // Nested struct member: marshal into the same writer.
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
        // wchar: a wchar32 (UTF-32 code point).
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
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let put = format!(
                "{{ {bound_check}sub := NewWriter($w.Endian); sub.PutU32(uint32(len({expr}))); \
                 for _, e := range {expr} {{ e.marshalInto(sub) }}; \
                 $w.PutU32(uint32(len(sub.Buf))); $w.PutBytes(sub.Buf) }}"
            );
            return Ok((format!("[]{}", escape_go_ident(&name)), put));
        }
    }
    // sequence<primitive> → u32 count + per-element encode.
    let (elem_go, elem_put) = map_type(elem, "e", enum_names, struct_names)?;
    let put = format!(
        "{bound_check}$w.PutU32(uint32(len({expr}))); for _, e := range {expr} {{ {elem_put} }}"
    );
    Ok((format!("[]{elem_go}"), put))
}

/// The inverse of [`map_type`]: a Go statement that reads a value from `r` and
/// assigns it to `target` (the decode side of the wire mapping).
/// zerodds-lint: recursion-depth 32
fn map_get(
    t: &TypeSpec,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
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
        ),
        TypeSpec::Map(m) => map_get_map(m, target, enum_names, struct_names),
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                Ok(format!("{target} = {}(int32(r.GetU32()))", exported(&name)))
            } else if struct_names.contains(&name) {
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
    let (elem_go, _) = map_type(elem, "e", enum_names, struct_names)?;
    let elem_get = map_get(elem, &format!("{target}[i]"), enum_names, struct_names)?;
    // sequence<struct> is DHEADER-framed; sequence<primitive> is not.
    let dheader = if matches!(elem, TypeSpec::Scoped(sn)
        if struct_names.contains(&sn.parts.last().map(|p| p.text.clone()).unwrap_or_default()))
    {
        "_ = r.GetU32(); "
    } else {
        ""
    };
    let bound_check = match bv {
        Some(bv) => format!(
            "if zdn > {bv} {{ panic(\"decoded sequence length exceeds its IDL bound ({bv})\") }}; "
        ),
        None => String::new(),
    };
    Ok(format!(
        "{{ {dheader}zdn := int(r.GetU32()); {bound_check}{target} = make([]{elem_go}, zdn); \
         for i := 0; i < zdn; i++ {{ {elem_get} }} }}"
    ))
}

/// zerodds-lint: recursion-depth 32
fn map_get_map(
    m: &zerodds_idl::ast::types::MapType,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    let (key_go, _) = map_type(&m.key, "zk", enum_names, struct_names)?;
    let (val_go, _) = map_type(&m.value, "zv", enum_names, struct_names)?;
    let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
    let key_get = map_get(&m.key, "zk", enum_names, struct_names)?;
    let val_get = map_get(&m.value, "zv", enum_names, struct_names)?;
    let dheader = if prim { "" } else { "_ = r.GetU32(); " };
    // B1 follow-up (#22 decode-side parity): mirror the encode-side bound
    // check — XTypes 1.3 §7.4.3.
    let bound_check = match m.bound.as_ref().and_then(array_size) {
        Some(bv) => format!(
            "if zdn > {bv} {{ panic(\"decoded map length exceeds its IDL bound ({bv})\") }}; "
        ),
        None => String::new(),
    };
    Ok(format!(
        "{{ {dheader}zdn := int(r.GetU32()); {bound_check}{target} = make(map[{key_go}]{val_go}, zdn); \
         for i := 0; i < zdn; i++ {{ var zk {key_go}; var zv {val_go}; {key_get}; {val_get}; \
         {target}[zk] = zv }} }}"
    ))
}

/// The inverse of [`build_array_put`]: nested index loops reading each element.
fn build_array_get(
    field: &str,
    sizes: &[i64],
    elem: &TypeSpec,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    let idx: String = (0..sizes.len()).map(|k| format!("[i{k}]")).collect();
    let mut body = map_get(elem, &format!("v.{field}{idx}"), enum_names, struct_names)?;
    for k in (0..sizes.len()).rev() {
        body = format!(
            "for i{k} := 0; i{k} < {}; i{k}++ {{\n\t\t{body}\n\t}}",
            sizes[k]
        );
    }
    Ok(body)
}
