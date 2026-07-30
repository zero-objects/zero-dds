// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → Julia emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! Julia source file: a `Writer` (byte-identical to `endpoints/julia`) plus, per
//! IDL `struct`, a Julia struct with a `marshal_xcdr(v, endian)` function.
//! `@final`, `@appendable`, and `@mutable` are supported (structs and unions);
//! only constructs with non-literal collection bounds raise
//! [`IdlJuliaError::Unsupported`].

use std::fmt::Write as _;

use std::collections::{HashMap, HashSet};

use zerodds_idl::ast::types::{
    Annotation, AttrDecl, BinaryOp, BitmaskDecl, BitsetDecl, CaseLabel, ConstDecl, ConstExpr,
    ConstType, ConstrTypeDecl, Declarator, Definition, EnumDef, Export, FixedPtType, FloatingType,
    IntegerType, InterfaceDcl, InterfaceDef, Literal, LiteralKind, Member, OpDecl, ParamAttribute,
    PrimitiveType, ScopedName, SequenceType, Specification, StructDcl, StructDef, SwitchTypeSpec,
    TypeDecl, TypeSpec, UnaryOp, UnionDcl, UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, PlacementKind, enum_bit_bound, enum_wire_octets,
    lower_annotations, lower_single,
};

use crate::error::{IdlJuliaError, Result};
use crate::keywords::escape_julia_ident;

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
    /// reference to one of these maps to a Julia holder whose wire form is a
    /// single backing integer (`marshal_into!`/`read_<name>`) — no collection
    /// DHEADER, so it is fully-descriptive (primitive) for the sequence/map
    /// framing rules (XTypes 1.3 §7.4.7).
    static BIT_NAMES: std::cell::RefCell<HashSet<String>> =
        std::cell::RefCell::new(HashSet::new());

    /// Set whenever a `fixed<P,S>` member is emitted, so the BCD prelude helper
    /// (`zd_fixed_enc`) is appended exactly once, and only when needed.
    static USED_FIXED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };

    /// Flattened qualified enum name → signed wire holder width in OCTETS
    /// (1/2/4), from `@bit_bound` (XTypes 1.3 §7.3.1.2.1.9 + §7.4.5.1) via the
    /// shared [`enum_wire_octets`]. Populated once per run; read at the single
    /// enum encode/decode site so a `@bit_bound(8)`/`@bit_bound(16)` enum
    /// narrows to 1/2 bytes instead of the former fixed 4.
    static ENUM_WIDTHS: std::cell::RefCell<HashMap<String, u32>> =
        std::cell::RefCell::new(HashMap::new());

    /// Every named `const` value expression, keyed by simple name, populated by
    /// [`register_const_values`] at the start of each run. A named collection
    /// bound (`sequence<octet, MAX>`, `char[LEN]`) or `case` label resolves
    /// through this map, mirroring idl-rust's `CONST_VALUES` / idl-zig — without
    /// it `sequence<octet, MAX>` degraded silently to unbounded and `char[LEN]`
    /// to `Unsupported`.
    static CONST_VALUES: std::cell::RefCell<HashMap<String, ConstExpr>> =
        std::cell::RefCell::new(HashMap::new());

    /// Every enumerator's integer value, keyed by simple name, so a named
    /// enumerator used as a bound or a `case` label folds to its integer value
    /// (mirrors idl-rust's `ENUM_LITERAL_VALUES`).
    static ENUM_LITERAL_VALUES: std::cell::RefCell<HashMap<String, i64>> =
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

/// Julia codegen language aliases matched by `@verbatim(language="...")`
/// (case-insensitive; the spec wildcard `"*"` always matches — see
/// [`Lowered::verbatims_for_language`]).
const JULIA_LANG_ALIASES: &[&str] = &["julia", "jl"];

/// Emits every `@verbatim` block from `anns` whose language matches the Julia
/// codegen and whose `placement` equals `placement`, each line prefixed with
/// `indent`. Source order preserved; text spliced unmodified (no wire impact —
/// XTypes 1.3 §7.2.2.4.8 / IDL 4.2 §8.3.5.1). Mirrors `idl-d`'s
/// `emit_verbatim_at`.
fn emit_verbatim_at(out: &mut String, indent: &str, anns: &[Annotation], placement: PlacementKind) {
    let Ok(lowered) = lower_annotations(anns) else {
        return;
    };
    for v in lowered.verbatims_for_language(JULIA_LANG_ALIASES) {
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
/// `END_FILE`) and per-declaration `@verbatim` placement. Mirrors `idl-d`'s
/// `def_annotations`.
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

/// `true` if `name` resolves to a `bitset`/`bitmask` declaration (its wire form
/// is a single backing integer — fully descriptive, no collection DHEADER).
fn is_bit_name(name: &str) -> bool {
    BIT_NAMES.with(|b| b.borrow().contains(name))
}

/// Collision-free flattened name for a declaration `simple` in module `scope`:
/// [`flatten_path`] of `scope + simple`, or the bare `simple` at global scope
/// (so every existing top-level golden is unchanged). Two same-simple-name
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
/// single Julia identifier. Each segment's own underscores are doubled and the
/// segments joined by a single underscore, so `module A_B { struct C }`
/// (`["A_B","C"]` → `A__B_C`) never collides with `module A { module B {
/// struct C }}` (`["A","B","C"]` → `A_B_C`) — the previous `join("_")` mapped
/// both to `A_B_C` (#A35, non-injective flatten → duplicate top-level symbol).
/// A single (global-scope) segment is returned verbatim so every existing
/// top-level golden is unchanged, and any segment without underscores (the
/// common case) is passed through untouched.
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
            // Interface-nested type declarations are promoted to the top level
            // under the interface's own scope segment (#A39), so their
            // reference paths resolve the same way module-nested ones do.
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                scope.push(iface.name.text.clone());
                for ex in &iface.exports {
                    if let Export::Type(td) = ex {
                        register_type_paths(
                            std::slice::from_ref(&Definition::Type(td.clone())),
                            scope,
                        );
                    }
                }
                scope.pop();
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

/// Options for the Julia backend.
#[derive(Debug, Clone, Default)]
pub struct JuliaGenOptions {}

/// The shared XCDR2 `Writer`, byte-identical to `endpoints/julia`.
const WIRE_PRELUDE: &str = r#"@enum Endian LE BE

mutable struct Writer
    buf::Vector{UInt8}
    endian::Endian
    # Wire representation: false = XCDR2 (max alignment 4, DHEADER-framed
    # appendable/mutable + non-primitive collections); true = XCDR1 / classic
    # CDR (max alignment 8, no DHEADER, PL_CDR1 @mutable). A nested composite
    # called via `marshal_into!(v, w)` inherits this flag, so the mode
    # propagates down the stream automatically.
    xcdr1::Bool
end
Writer(endian::Endian) = Writer(UInt8[], endian, false)

function align!(w::Writer, a::Int)
    cap = min(a, w.xcdr1 ? 8 : 4)
    pad = mod(cap - mod(length(w.buf), cap), cap)
    for _ in 1:pad
        push!(w.buf, 0x00)
    end
end

function emit!(w::Writer, a::Int, le::Vector{UInt8})
    align!(w, a)
    append!(w.buf, w.endian == BE ? reverse(le) : le)
end

le_bytes(v::Unsigned, n::Int) = UInt8[UInt8((v >> (8 * (i - 1))) & 0xff) for i in 1:n]

put_u8!(w::Writer, v) = push!(w.buf, UInt8(v & 0xff))
put_bool!(w::Writer, v::Bool) = put_u8!(w, v ? 1 : 0)
put_u16!(w::Writer, v) = emit!(w, 2, le_bytes(UInt16(v), 2))
put_u32!(w::Writer, v) = emit!(w, 4, le_bytes(UInt32(v), 4))
put_u64!(w::Writer, v) = emit!(w, 8, le_bytes(UInt64(v), 8))
put_f32!(w::Writer, v) = emit!(w, 4, le_bytes(reinterpret(UInt32, Float32(v)), 4))
put_f64!(w::Writer, v) = emit!(w, 8, le_bytes(reinterpret(UInt64, Float64(v)), 8))
put_bytes!(w::Writer, b) = append!(w.buf, b)

function put_string!(w::Writer, s::AbstractString)
    put_u32!(w, sizeof(s) + 1)
    append!(w.buf, codeunits(s))
    put_u8!(w, 0)
end

function put_seq_u8!(w::Writer, b::Vector{UInt8})
    put_u32!(w, length(b))
    append!(w.buf, b)
end

function put_wstring!(w::Writer, s::AbstractString)
    units = UInt16[]
    for c in s
        cp = UInt32(c)
        if cp <= 0xFFFF
            push!(units, UInt16(cp))
        else
            rr = cp - 0x10000
            push!(units, UInt16(0xD800 + (rr >> 10)))
            push!(units, UInt16(0xDC00 + (rr & 0x3FF)))
        end
    end
    put_u32!(w, length(units) * 2)
    for u in units
        put_u16!(w, u)
    end
end

function put_long_double!(w::Writer, v::Float64)
    bits = reinterpret(UInt64, v)
    sign = bits >> 63
    exp = (bits >> 52) & 0x7FF
    mant = bits & 0xFFFFFFFFFFFFF
    hi = sign << 63
    lo = UInt64(0)
    if !(exp == 0 && mant == 0)
        hi = (sign << 63) | ((exp - 1023 + 16383) << 48) | (mant >> 4)
        lo = (mant & 0xF) << 60
    end
    le = UInt8[]
    for i in 0:7
        push!(le, UInt8((lo >> (8 * i)) & 0xff))
    end
    for i in 0:7
        push!(le, UInt8((hi >> (8 * i)) & 0xff))
    end
    emit!(w, 8, le)
end

bytes(w::Writer) = w.buf

mutable struct Reader
    buf::Vector{UInt8}
    pos::Int
    endian::Endian
    # See `Writer.xcdr1`: false = XCDR2, true = XCDR1 / classic CDR.
    xcdr1::Bool
end
Reader(buf::Vector{UInt8}, endian::Endian) = Reader(buf, 1, endian, false)

function ralign!(r::Reader, a::Int)
    cap = min(a, r.xcdr1 ? 8 : 4)
    while mod(r.pos - 1, cap) != 0
        r.pos += 1
    end
end

function get_le(r::Reader, a::Int, n::Int)::UInt64
    ralign!(r, a)
    v = UInt64(0)
    if r.endian == BE
        for i in 0:n-1
            v = (v << 8) | UInt64(r.buf[r.pos + i])
        end
    else
        for i in n-1:-1:0
            v = (v << 8) | UInt64(r.buf[r.pos + i])
        end
    end
    r.pos += n
    v
end

get_u8!(r::Reader)::UInt8 = begin b = r.buf[r.pos]; r.pos += 1; b end
get_bool!(r::Reader)::Bool = get_u8!(r) != 0
get_u16!(r::Reader)::UInt16 = UInt16(get_le(r, 2, 2))
get_u32!(r::Reader)::UInt32 = UInt32(get_le(r, 4, 4))
get_u64!(r::Reader)::UInt64 = get_le(r, 8, 8)
get_f32!(r::Reader)::Float32 = reinterpret(Float32, get_u32!(r))
get_f64!(r::Reader)::Float64 = reinterpret(Float64, get_u64!(r))

function get_bytes_n!(r::Reader, n::Int)::Vector{UInt8}
    b = r.buf[r.pos:r.pos + n - 1]
    r.pos += n
    b
end

function get_string!(r::Reader)::String
    n = Int(get_u32!(r))
    s = String(r.buf[r.pos:r.pos + n - 2])
    r.pos += n
    s
end

function get_seq_u8!(r::Reader)::Vector{UInt8}
    n = Int(get_u32!(r))
    get_bytes_n!(r, n)
end

function get_wstring!(r::Reader)::String
    n = div(Int(get_u32!(r)), 2)
    units = UInt16[]
    for _ in 1:n
        push!(units, get_u16!(r))
    end
    io = IOBuffer()
    i = 1
    while i <= n
        u = UInt32(units[i])
        if u >= 0xD800 && u <= 0xDBFF && i + 1 <= n
            lo = UInt32(units[i + 1])
            print(io, Char(0x10000 + ((u - 0xD800) << 10) + (lo - 0xDC00)))
            i += 2
        else
            print(io, Char(u))
            i += 1
        end
    end
    String(take!(io))
end

function get_long_double!(r::Reader)::Float64
    ralign!(r, 8)
    le = get_bytes_n!(r, 16)
    if r.endian == BE
        reverse!(le)
    end
    lo = UInt64(0)
    hi = UInt64(0)
    for i in 0:7
        lo |= UInt64(le[i + 1]) << (8 * i)
        hi |= UInt64(le[i + 9]) << (8 * i)
    end
    sign = hi >> 63
    exp = (hi >> 48) & 0x7FFF
    mant = ((hi & 0xFFFFFFFFFFFF) << 4) | (lo >> 60)
    bits = (exp == 0 && mant == 0) ? (sign << 63) : ((sign << 63) | ((exp - 16383 + 1023) << 52) | mant)
    reinterpret(Float64, bits)
end

# PL_CDR1 (@mutable, XCDR1) member: `[PID][len][body][pad-to-4]`. The PID length
# carries the UNPADDED body length; member ids >= 0x3F00 or bodies over 0xFFFF
# use the extended header (PID_EXTENDED, 32-bit id + length). Matches
# `zerodds_cdr::xcdr1::encode_pl_cdr1_member`.
function write_pl_cdr1_member!(w::Writer, id::UInt32, body::Vector{UInt8})
    if id >= 0x3F00 || length(body) > 0xFFFF
        put_u16!(w, 0x3F01) # PID_EXTENDED
        put_u16!(w, 8)
        put_u32!(w, id)
        put_u32!(w, UInt32(length(body)))
    else
        put_u16!(w, UInt16(id))
        put_u16!(w, UInt16(length(body)))
    end
    put_bytes!(w, body)
    pad = mod(4 - mod(length(body), 4), 4)
    for _ in 1:pad
        put_u8!(w, 0)
    end
    nothing
end

# PL_CDR1 sentinel terminator (PID_LIST_END = 0x3F02, length 0).
write_pl_cdr1_sentinel!(w::Writer) = (put_u16!(w, 0x3F02); put_u16!(w, 0); nothing)

# Reads one PL_CDR1 (@mutable, XCDR1) member. Returns `nothing` at the sentinel
# (PID_LIST_END). The RTPS MUST_UNDERSTAND / impl-specific flag bits (top two of
# the 16-bit PID) are stripped before comparing against the reserved PIDs.
# Mirrors `zerodds_cdr::xcdr1::read_pl_cdr1_member`.
function read_pl_cdr1_member!(r::Reader)
    pid = get_u16!(r) & 0x3FFF
    len = get_u16!(r)
    if pid == 0x3F02 # PID_LIST_END
        return nothing
    end
    if pid == 0x3F01 # PID_EXTENDED
        member_id = get_u32!(r)
        body_len = Int(get_u32!(r))
    else
        member_id = UInt32(pid)
        body_len = Int(len)
    end
    body = get_bytes_n!(r, body_len)
    pad = mod(4 - mod(body_len, 4), 4)
    for _ in 1:pad
        if r.pos <= length(r.buf)
            r.pos += 1
        end
    end
    (member_id, body)
end
"#;

/// Generates a self-contained Julia module from the IDL AST: the shared wire
/// `Writer`/`Reader` prelude followed by every generated type.
///
/// # Errors
/// Returns [`IdlJuliaError::Unsupported`] for constructs the Julia backend does
/// not yet emit (e.g. non-literal array/sequence bounds).
pub fn generate_julia_module(spec: &Specification, _opts: &JuliaGenOptions) -> Result<String> {
    generate(spec, true)
}

/// Generates a Julia **fragment** for the given IDL AST: the generated types
/// only, WITHOUT the shared wire prelude (`Writer`/`Reader`/`Endian` and the
/// `put_`/`get_` helpers). Use this for every file but the first in a
/// multi-file compose so the prelude is defined exactly once across the merged
/// source (#C-julia — the whole prelude was previously emitted per file, so a
/// second generated file re-declared `Writer`/`Reader`/`Endian` and the merge
/// failed to load).
///
/// # Errors
/// As [`generate_julia_module`].
pub fn generate_julia_fragment(spec: &Specification, _opts: &JuliaGenOptions) -> Result<String> {
    generate(spec, false)
}

fn generate(spec: &Specification, emit_prelude: bool) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Code generated by zerodds-idlc (Julia backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "# SPDX-License-Identifier: Apache-2.0\n");
    if emit_prelude {
        out.push_str(WIRE_PRELUDE);
    }

    // Register every named type's fully-qualified path so reference sites can
    // resolve a `ScopedName` against its enclosing scope (#21 cross-module,
    // #A39 interface-nested).
    TYPE_PATHS.with(|t| t.borrow_mut().clear());
    register_type_paths(&spec.definitions, &mut Vec::new());
    // Register every `const` value and enumerator so a named collection bound
    // (`sequence<octet, MAX>`, `char[LEN]`) or `case` label resolves through
    // `eval_const_int` (§7.4.1.4.4), mirroring idl-rust / idl-zig.
    CONST_VALUES.with(|m| m.borrow_mut().clear());
    ENUM_LITERAL_VALUES.with(|m| m.borrow_mut().clear());
    register_const_values(&spec.definitions);
    USED_FIXED.with(|f| f.set(false));

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_FILE)` from all top-level defs
    // (source order), emitted after the wire prelude, before any type.
    for def in &spec.definitions {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::BeginFile);
    }

    // `module X { ... }` content is promoted to the top level, each definition
    // paired with its module scope path (see `flatten_module_defs`).
    let flat = flatten_module_defs(&spec.definitions);
    // #A39: interface-nested type declarations are promoted to the top level
    // under the interface's own scope segment, so their DDS data types survive
    // instead of being silently dropped with the interface body.
    let iface_types = flatten_iface_types(&spec.definitions);

    // Every named type (module-level + interface-nested) paired with its scope.
    let all_types: Vec<(Vec<String>, &TypeDecl)> = flat
        .iter()
        .filter_map(|(scope, d)| match d {
            Definition::Type(td) => Some((scope.clone(), td)),
            _ => None,
        })
        .chain(iface_types.iter().map(|(s, td)| (s.clone(), *td)))
        .collect();

    // Named enums keyed by their flattened module-qualified name. An enum
    // member is a 32-bit signed integer on the wire (XTypes 1.3 §7.4.5.1).
    let enum_names: HashSet<String> = all_types
        .iter()
        .filter_map(|(scope, d)| match d {
            TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => Some(qualify(scope, &e.name.text)),
            _ => None,
        })
        .collect();

    // Qualified-name -> EnumDef, so a union with an enum discriminator can
    // resolve `case ENUMERATOR:` labels to their integer discriminant (#P4).
    let enum_defs: HashMap<String, &EnumDef> = all_types
        .iter()
        .filter_map(|(scope, d)| match d {
            TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => Some((qualify(scope, &e.name.text), e)),
            _ => None,
        })
        .collect();
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

    let struct_names: HashSet<String> = all_types
        .iter()
        .filter_map(|(scope, d)| match d {
            TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
                Some(qualify(scope, &s.name.text))
            }
            _ => None,
        })
        .collect();

    // Qualified-name -> StructDef, so a nested-struct `@key` member's own
    // `@key` subset (and `keyhash::uses_md5`'s static max-size analysis) can be
    // resolved — and so a `struct D : Base` can pull in the base's members
    // (#A10) — mirrors `struct_names` above, keeping the full def.
    let structs: HashMap<String, &StructDef> = all_types
        .iter()
        .filter_map(|(scope, d)| match d {
            TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
                Some((qualify(scope, &s.name.text), s))
            }
            _ => None,
        })
        .collect();

    // `bitset`/`bitmask` logical names, published to `BIT_NAMES` so a reference
    // site resolves them to the integer-backed holder (no collection DHEADER).
    let bit_names: HashSet<String> = all_types
        .iter()
        .filter_map(|(scope, d)| match d {
            TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => Some(qualify(scope, &b.name.text)),
            TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => Some(qualify(scope, &b.name.text)),
            _ => None,
        })
        .collect();
    BIT_NAMES.with(|b| *b.borrow_mut() = bit_names);

    let typedefs = collect_typedefs(spec);

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
            // catch-all arm; emit it as a Julia module-level constant.
            Definition::Const(c) => emit_const(&mut out, c, scope),
            // #11 — interface operations/attributes (previously dropped) emit as
            // native Julia `<Iface>_Client`/`<Iface>_Handler` abstract types plus
            // per-operation generic-function declarations, mirroring the idl-ts /
            // idl-swift interface surface. The interface's nested TYPES are still
            // emitted separately (#A39, `iface_types`).
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                emit_interface_surface(
                    &mut out,
                    iface,
                    scope,
                    &enum_names,
                    &struct_names,
                    &typedefs,
                );
            }
            _ => {}
        }
        // §7.2.2.4.8 — text directly after the annotated declaration.
        emit_verbatim_at(&mut out, "", anns, PlacementKind::AfterDeclaration);
    }

    // #A39: interface-nested types, emitted after the module-level defs.
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
        out.push_str(JULIA_FIXED);
    }
    // Self-contained MD5 (RFC 1321) for the KeyHash MD5 branch; appended only
    // when used (Julia resolves the call at run time, so order is fine).
    if out.contains("zd_md5(") {
        out.push_str(JULIA_MD5);
    }
    Ok(out)
}

/// Emits a single `TypeDecl` (module-level or interface-nested #A39) into `out`.
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

/// Emits an IDL `const` as a Julia module-level constant (#A5/P1). A `const` of
/// any type used to vanish through the top-level catch-all arm. The value is
/// rendered from the `ConstExpr` (Boolean literals normalized to `true`/`false`,
/// any wide `L"…"`/`L'…'` prefix stripped, float/fixed suffixes dropped) so the
/// output is always valid Julia. Values Julia cannot express as a compile-time
/// constant from the bare token (an enum-typed / scoped reference) are skipped
/// rather than emitting ill-formed source.
fn emit_const(out: &mut String, c: &ConstDecl, scope: &[String]) {
    let Some(val) = const_value_to_julia(&c.value) else {
        return;
    };
    let name = escape_julia_ident(&qualify(scope, &c.name.text));
    match const_julia_type(&c.type_) {
        Some(ty) => {
            let _ = writeln!(out, "\nconst {name} = {ty}({val})");
        }
        None => {
            let _ = writeln!(out, "\nconst {name} = {val}");
        }
    }
}

/// Julia numeric type for a `const` declaration whose value should be pinned to
/// that width (`None` = emit the value verbatim: string / char / boolean /
/// fixed / enum-typed, where the literal already carries the right Julia type
/// or no compile-time type exists).
fn const_julia_type(ct: &ConstType) -> Option<&'static str> {
    match ct {
        ConstType::Integer(i) => Some(julia_int_type(*i)),
        ConstType::Octet => Some("UInt8"),
        ConstType::Floating(FloatingType::Float) => Some("Float32"),
        ConstType::Floating(FloatingType::Double | FloatingType::LongDouble) => Some("Float64"),
        // A string / char / boolean literal already renders as its Julia type;
        // a `fixed` decimal is rendered as a string; an enum-/scoped-typed const
        // keeps the bare (skipped-or-verbatim) form.
        ConstType::Char
        | ConstType::WideChar
        | ConstType::Boolean
        | ConstType::String { .. }
        | ConstType::Fixed
        | ConstType::Scoped(_) => None,
    }
}

/// The Julia integer type for an IDL integer type.
fn julia_int_type(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Int8 => "Int8",
        IntegerType::UInt8 => "UInt8",
        IntegerType::Short | IntegerType::Int16 => "Int16",
        IntegerType::UShort | IntegerType::UInt16 => "UInt16",
        IntegerType::Long | IntegerType::Int32 => "Int32",
        IntegerType::ULong | IntegerType::UInt32 => "UInt32",
        IntegerType::LongLong | IntegerType::Int64 => "Int64",
        IntegerType::ULongLong | IntegerType::UInt64 => "UInt64",
    }
}

/// Renders a `ConstExpr` as a Julia constant expression, or `None` for a form
/// the Julia backend does not express (an enum-valued / const-alias scoped
/// reference — the bare last segment is not reconstructable here).
/// zerodds-lint: recursion-depth 32 (const expression tree; bounded by the IDL
/// grammar's expression nesting).
fn const_value_to_julia(e: &ConstExpr) -> Option<String> {
    match e {
        ConstExpr::Literal(l) => const_literal_to_julia(l),
        ConstExpr::Scoped(_) => None,
        ConstExpr::Unary { op, operand, .. } => {
            let v = const_value_to_julia(operand)?;
            let o = match op {
                UnaryOp::Plus => "+",
                UnaryOp::Minus => "-",
                UnaryOp::BitNot => "~",
            };
            Some(format!("{o}{v}"))
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let l = const_value_to_julia(lhs)?;
            let r = const_value_to_julia(rhs)?;
            let o = match op {
                BinaryOp::Or => "|",
                BinaryOp::Xor => "⊻",
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

/// Renders a single literal as valid Julia source.
fn const_literal_to_julia(l: &Literal) -> Option<String> {
    let raw = l.raw.trim();
    Some(match l.kind {
        // Julia accepts decimal / `0x` / `0o` / `0b` integer literals as-is.
        LiteralKind::Integer => raw.to_string(),
        // Strip a trailing IDL float/fixed suffix (`d`/`f`/`l`) Julia rejects.
        LiteralKind::Floating => raw
            .trim_end_matches(['d', 'D', 'f', 'F', 'l', 'L'])
            .to_string(),
        // A `fixed` decimal has no native Julia type — render as a string.
        LiteralKind::Fixed => format!(
            "\"{}\"",
            raw.trim_end_matches(['d', 'D']).replace('"', "\\\"")
        ),
        // Normalize the IDL boolean keyword to Julia's `true`/`false` (never
        // emit a bare `TRUE`/`FALSE` token, which is not a Julia value — #A13).
        LiteralKind::Boolean => {
            if raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // Narrow string/char literals pass through; wide literals drop the `L`
        // prefix (`L"x"`/`L'x'` is not valid Julia).
        LiteralKind::String | LiteralKind::Char => raw.to_string(),
        LiteralKind::WideString | LiteralKind::WideChar => {
            raw.strip_prefix('L').unwrap_or(raw).to_string()
        }
    })
}

/// RFC 1321 MD5 over a byte vector, returning the 16-byte digest. Self-contained
/// (Julia's Base has no MD5); byte-identical to `zerodds_foundation::md5`.
const JULIA_MD5: &str = r#"
const ZD_MD5_S = UInt32[7,12,17,22,7,12,17,22,7,12,17,22,7,12,17,22,
    5,9,14,20,5,9,14,20,5,9,14,20,5,9,14,20,
    4,11,16,23,4,11,16,23,4,11,16,23,4,11,16,23,
    6,10,15,21,6,10,15,21,6,10,15,21,6,10,15,21]
const ZD_MD5_K = UInt32[
    0xd76aa478,0xe8c7b756,0x242070db,0xc1bdceee,0xf57c0faf,0x4787c62a,0xa8304613,0xfd469501,
    0x698098d8,0x8b44f7af,0xffff5bb1,0x895cd7be,0x6b901122,0xfd987193,0xa679438e,0x49b40821,
    0xf61e2562,0xc040b340,0x265e5a51,0xe9b6c7aa,0xd62f105d,0x02441453,0xd8a1e681,0xe7d3fbc8,
    0x21e1cde6,0xc33707d6,0xf4d50d87,0x455a14ed,0xa9e3e905,0xfcefa3f8,0x676f02d9,0x8d2a4c8a,
    0xfffa3942,0x8771f681,0x6d9d6122,0xfde5380c,0xa4beea44,0x4bdecfa9,0xf6bb4b60,0xbebfbc70,
    0x289b7ec6,0xeaa127fa,0xd4ef3085,0x04881d05,0xd9d4d039,0xe6db99e5,0x1fa27cf8,0xc4ac5665,
    0xf4292244,0x432aff97,0xab9423a7,0xfc93a039,0x655b59c3,0x8f0ccc92,0xffeff47d,0x85845dd1,
    0x6fa87e4f,0xfe2ce6e0,0xa3014314,0x4e0811a1,0xf7537e82,0xbd3af235,0x2ad7d2bb,0xeb86d391]

function zd_md5(data::Vector{UInt8})::Vector{UInt8}
    a0 = 0x67452301; b0 = 0xefcdab89; c0 = 0x98badcfe; d0 = 0x10325476
    msg = copy(data)
    bitlen = UInt64(length(data)) * 8
    push!(msg, 0x80)
    while mod(length(msg), 64) != 56
        push!(msg, 0x00)
    end
    for i in 0:7
        push!(msg, UInt8((bitlen >> (8 * i)) & 0xff))
    end
    for off in 0:64:length(msg)-1
        M = UInt32[]
        for j in 0:15
            w = UInt32(msg[off+j*4+1]) | (UInt32(msg[off+j*4+2]) << 8) |
                (UInt32(msg[off+j*4+3]) << 16) | (UInt32(msg[off+j*4+4]) << 24)
            push!(M, w)
        end
        A = a0; B = b0; C = c0; D = d0
        for i in 0:63
            local F::UInt32
            local g::Int
            if i < 16
                F = (B & C) | (~B & D); g = i
            elseif i < 32
                F = (D & B) | (~D & C); g = mod(5 * i + 1, 16)
            elseif i < 48
                F = B ⊻ C ⊻ D; g = mod(3 * i + 5, 16)
            else
                F = C ⊻ (B | ~D); g = mod(7 * i, 16)
            end
            F = F + A + ZD_MD5_K[i+1] + M[g+1]
            A = D
            D = C
            C = B
            sh = ZD_MD5_S[i+1]
            B = B + ((F << sh) | (F >> (UInt32(32) - sh)))
        end
        a0 += A; b0 += B; c0 += C; d0 += D
    end
    out = UInt8[]
    for v in (a0, b0, c0, d0)
        for i in 0:3
            push!(out, UInt8((v >> (8 * i)) & 0xff))
        end
    end
    out
end
"#;

/// BCD codec for `fixed<P,S>`. Appended once when any `fixed` member is emitted.
/// Builds the packed-BCD octet sequence (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5)
/// from a decimal string: an optional leading pad nibble (so the nibble count
/// is even), `P` digit nibbles most-significant first, then the sign nibble
/// (`0xC` positive, `0xD` negative). Byte count `(P+2)/2`, no length prefix.
/// Byte-identical to `idl-d`'s `zdFixedEnc`.
const JULIA_FIXED: &str = r#"
function zd_fixed_enc(s::AbstractString, P::Int, S::Int)::Vector{UInt8}
    sign = true
    i = 1
    if !isempty(s) && (s[1] == '-' || s[1] == '+')
        sign = s[1] != '-'
        i = 2
    end
    rest = s[i:end]
    dotpos = findfirst(==('.'), rest)
    ip = dotpos === nothing ? rest : rest[1:dotpos-1]
    fp = dotpos === nothing ? "" : rest[dotpos+1:end]
    db = Char[]
    intNeeded = P - S
    for _ in length(ip):intNeeded-1
        push!(db, '0')
    end
    append!(db, collect(ip))
    append!(db, collect(fp))
    for _ in length(fp):S-1
        push!(db, '0')
    end
    nib = UInt8[]
    if (P + 1) % 2 == 1
        push!(nib, 0x00)
    end
    for c in db
        push!(nib, UInt8(c) - UInt8('0'))
    end
    push!(nib, sign ? 0x0c : 0x0d)
    outb = UInt8[]
    k = 1
    while k <= length(nib)
        push!(outb, UInt8((nib[k] << 4) | nib[k+1]))
        k += 2
    end
    outb
end
"#;

/// Backing-integer storage for a bit container of `total_bits` bits: XTypes 1.3
/// §7.4.7 — the smallest holder that fits (`≤8`→UInt8, `≤16`→UInt16, `≤32`→
/// UInt32, else UInt64). Returns `(Julia type, put-fn, get-fn)`.
fn bit_storage(total_bits: usize) -> (&'static str, &'static str, &'static str) {
    match total_bits {
        0..=8 => ("UInt8", "put_u8!", "get_u8!"),
        9..=16 => ("UInt16", "put_u16!", "get_u16!"),
        17..=32 => ("UInt32", "put_u32!", "get_u32!"),
        _ => ("UInt64", "put_u64!", "get_u64!"),
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

/// Resolves a `fixed<P,S>`'s digit count `P` and scale `S` to codegen-time
/// integers.
///
/// # Errors
/// [`IdlJuliaError::Unsupported`] if either is not a resolvable non-negative
/// integer literal.
fn fixed_ps(f: &FixedPtType) -> Result<(i64, i64)> {
    let p = array_size(&f.digits)
        .filter(|v| *v > 0)
        .ok_or_else(|| IdlJuliaError::Unsupported("non-integer fixed digit count".to_string()))?;
    let s = array_size(&f.scale)
        .filter(|v| *v >= 0)
        .ok_or_else(|| IdlJuliaError::Unsupported("non-integer fixed scale".to_string()))?;
    Ok((p, s))
}

/// Emits an IDL `bitset` as a Julia `mutable struct` holder over its backing
/// integer, with a bit-accessor per named bitfield and an XCDR2
/// marshal/unmarshal that writes the backing integer (XTypes 1.3 §7.4.7 — wire
/// = backing int). The holder is mutable so the bit setters mutate `storage`
/// in place; nested in an immutable aggregate it still works as a field value.
///
/// # Errors
/// [`IdlJuliaError::Unsupported`] if a bitfield width is not a codegen-time
/// integer.
fn emit_bitset(out: &mut String, b: &BitsetDecl, scope: &[String]) -> Result<()> {
    let mut widths: Vec<usize> = Vec::with_capacity(b.bitfields.len());
    for bf in &b.bitfields {
        let w = array_size(&bf.spec.width)
            .filter(|w| *w >= 0)
            .ok_or_else(|| {
                IdlJuliaError::Unsupported(format!(
                    "non-integer bitfield width in bitset {}",
                    b.name.text
                ))
            })? as usize;
        widths.push(w);
    }
    let total: usize = widths.iter().sum();
    let (storage, put, get) = bit_storage(total);
    let ty = escape_julia_ident(&qualify(scope, &b.name.text));

    let _ = writeln!(out, "\nmutable struct {ty}");
    let _ = writeln!(out, "    storage::{storage}");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::BeginDeclaration);
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "end");
    let mut offset: usize = 0;
    for (bf, width) in b.bitfields.iter().zip(&widths) {
        if let Some(name) = &bf.name {
            let field = escape_julia_ident(&name.text);
            if *width == 1 {
                let _ = writeln!(
                    out,
                    "{field}(v::{ty})::Bool = ((v.storage >> {offset}) & 1) != 0"
                );
                let _ = writeln!(
                    out,
                    "set_{field}!(v::{ty}, x::Bool) = (m = {storage}(1) << {offset}; x ? (v.storage |= m) : (v.storage &= ~m); v)"
                );
            } else {
                let mask: u128 = if *width >= 128 {
                    u128::MAX
                } else {
                    (1u128 << *width) - 1
                };
                let _ = writeln!(
                    out,
                    "{field}(v::{ty})::{storage} = {storage}((v.storage >> {offset}) & {mask})"
                );
                let _ = writeln!(
                    out,
                    "set_{field}!(v::{ty}, x::{storage}) = (m = {storage}({mask}) << {offset}; v.storage = {storage}((v.storage & ~m) | ((x & {mask}) << {offset})); v)"
                );
            }
        }
        offset += width;
    }
    emit_bit_marshal(out, &ty, storage, put, get);
    Ok(())
}

/// Emits an IDL `bitmask` as a Julia `mutable struct` holder over its
/// `@bit_bound` backing integer (default 32), with an OR-able manifest constant
/// per bit value and an XCDR2 marshal/unmarshal writing the backing integer
/// (XTypes 1.3 §7.4.7). Constants are module-level and type-qualified
/// (`<Type>_<NAME>`) since Julia has no declaration-scoped constants.
fn emit_bitmask(out: &mut String, b: &BitmaskDecl, scope: &[String]) {
    let (storage, put, get) = bit_storage(bitmask_bit_bound(&b.annotations) as usize);
    let ty = escape_julia_ident(&qualify(scope, &b.name.text));

    let _ = writeln!(out, "\nmutable struct {ty}");
    let _ = writeln!(out, "    storage::{storage}");
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::BeginDeclaration);
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "end");
    for (idx, v) in b.values.iter().enumerate() {
        let pos = bit_position(&v.annotations).unwrap_or(idx as u32);
        let cname = escape_julia_ident(&v.name.text.to_uppercase());
        let _ = writeln!(out, "const {ty}_{cname} = {storage}(1) << {pos}");
    }
    emit_bit_marshal(out, &ty, storage, put, get);
}

/// Shared marshal/unmarshal footer for a bit-container holder `ty` whose wire
/// form is the single backing integer `storage` (put/get functions `put`/`get`).
fn emit_bit_marshal(out: &mut String, ty: &str, storage: &str, put: &str, get: &str) {
    let _ = writeln!(out, "\nfunction marshal_into!(v::{ty}, w::Writer)");
    let _ = writeln!(out, "    {put}(w, v.storage)");
    let _ = writeln!(out, "    nothing");
    let _ = writeln!(out, "end");
    let _ = writeln!(
        out,
        "\nfunction marshal_xcdr(v::{ty}, endian::Endian)::Vector{{UInt8}}"
    );
    let _ = writeln!(out, "    w = Writer(endian)");
    let _ = writeln!(out, "    marshal_into!(v, w)");
    let _ = writeln!(out, "    bytes(w)");
    let _ = writeln!(out, "end");
    // XCDR1 entry point: a bit container's wire form is the single backing
    // integer (no framing), so the bytes are identical for a top-level holder;
    // the entry point exists for codegen-contract parity with idl-rust.
    let _ = writeln!(
        out,
        "\nfunction marshal_xcdr1(v::{ty}, endian::Endian)::Vector{{UInt8}}"
    );
    let _ = writeln!(out, "    w = Writer(endian)");
    let _ = writeln!(out, "    w.xcdr1 = true");
    let _ = writeln!(out, "    marshal_into!(v, w)");
    let _ = writeln!(out, "    bytes(w)");
    let _ = writeln!(out, "end");
    let _ = writeln!(out, "\nfunction read_{ty}(r::Reader)::{ty}");
    let _ = writeln!(out, "    {ty}({storage}({get}(r)))");
    let _ = writeln!(out, "end");
    let _ = writeln!(
        out,
        "\nunmarshal_xcdr_{ty}(buf::Vector{{UInt8}}, endian::Endian)::{ty} = read_{ty}(Reader(buf, endian))"
    );
    let _ = writeln!(
        out,
        "function unmarshal_xcdr1_{ty}(buf::Vector{{UInt8}}, endian::Endian)::{ty}\n    r = Reader(buf, endian)\n    r.xcdr1 = true\n    read_{ty}(r)\nend"
    );
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

/// Emits an IDL `enum` as a Julia `@enum` (Int32-backed) with explicit values.
fn emit_enum(out: &mut String, e: &EnumDef, scope: &[String]) {
    let values = enumerator_values(e);
    let ty = escape_julia_ident(&qualify(scope, &e.name.text));
    let mut line = format!("\n@enum {ty}");
    for (en, value) in e.enumerators.iter().zip(&values) {
        let name = escape_julia_ident(&en.name.text);
        line.push_str(&format!(" {name}={value}"));
    }
    let _ = writeln!(out, "{line}");
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
/// `enclosing_module… + interface_name` (#A39). Julia has no nested-type
/// construct, so these are promoted to the top level under the interface's own
/// name segment (so two interfaces in one module do not collide). Without this
/// the whole interface body — including its DDS data types — was silently
/// dropped.
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

/// Collects `typedef` aliases (simple declarators) as qualified-name -> aliased
/// type-spec. A typedef is wire-transparent, so members are resolved to the
/// underlying type before mapping (`typedef long Score; Score s;` → `long`).
fn collect_typedefs(spec: &Specification) -> HashMap<String, TypeSpec> {
    let mut m = HashMap::new();
    let module_typedefs = flatten_module_defs(&spec.definitions)
        .into_iter()
        .filter_map(|(scope, def)| match def {
            Definition::Type(td @ TypeDecl::Typedef(_)) => Some((scope, td)),
            _ => None,
        });
    // #A39: interface-nested typedefs are promoted under the interface scope.
    let iface_typedefs = flatten_iface_types(&spec.definitions)
        .into_iter()
        .filter(|(_, td)| matches!(td, TypeDecl::Typedef(_)));
    for (scope, td) in module_typedefs.chain(iface_typedefs) {
        if let TypeDecl::Typedef(td) = td {
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

/// Evaluates a fixed-array bound / collection bound / `fixed<P,S>` digit count
/// to its integer size. Resolves literals, unary signs, named `const`s, named
/// enumerators and folded binary arithmetic via [`eval_const_int`] (§7.4.1.4.4)
/// — so `sequence<octet, MAX>` and `char[LEN]` no longer degrade to unbounded /
/// `Unsupported` when the bound names a constant.
/// zerodds-lint: recursion-depth 32
fn array_size(e: &ConstExpr) -> Option<i64> {
    thread_local! {
        static EMPTY: HashMap<String, i64> = HashMap::new();
    }
    EMPTY.with(|empty| eval_const_int(e, empty, 0))
}

/// Registers every `const` value expression and every enumerator value in the
/// spec into [`CONST_VALUES`] / [`ENUM_LITERAL_VALUES`], keyed by simple name,
/// so [`eval_const_int`] can resolve a named bound (`sequence<octet, MAX>`) or a
/// named `case` label (§7.4.1.4.4 const_expr). Recurses into modules and
/// interface bodies.
/// zerodds-lint: recursion-depth 32 (module/interface nesting; bounded by grammar).
fn register_const_values(defs: &[Definition]) {
    for def in defs {
        match def {
            Definition::Const(c) => {
                CONST_VALUES.with(|m| {
                    m.borrow_mut().insert(c.name.text.clone(), c.value.clone());
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                for (en, val) in e.enumerators.iter().zip(enumerator_values(e)) {
                    ENUM_LITERAL_VALUES.with(|m| {
                        m.borrow_mut().insert(en.name.text.clone(), i64::from(val));
                    });
                }
            }
            Definition::Module(m) => register_const_values(&m.definitions),
            Definition::Interface(InterfaceDcl::Def(iface)) => {
                for ex in &iface.exports {
                    match ex {
                        Export::Const(c) => {
                            CONST_VALUES.with(|m| {
                                m.borrow_mut().insert(c.name.text.clone(), c.value.clone());
                            });
                        }
                        Export::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                            for (en, val) in e.enumerators.iter().zip(enumerator_values(e)) {
                                ENUM_LITERAL_VALUES.with(|m| {
                                    m.borrow_mut().insert(en.name.text.clone(), i64::from(val));
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

/// Evaluates a constant expression to a signed integer, resolving named
/// constants (via [`CONST_VALUES`]), enumerator names (via [`ENUM_LITERAL_VALUES`]
/// or `locals`) and folding IDL arithmetic/bitwise operators (§7.4.1.4.4).
/// Mirrors idl-rust's `eval_const_i128` / idl-zig's `eval_const_int` so a bound
/// or `case` label evaluates to the SAME integer in every backend. `locals`
/// supplies the switch enum's enumerators (union path); `depth` bounds
/// const-reference chains.
/// zerodds-lint: recursion-depth 32 (const-reference chain; explicitly bounded).
fn eval_const_int(e: &ConstExpr, locals: &HashMap<String, i64>, depth: u32) -> Option<i64> {
    if depth > 32 {
        return None;
    }
    match e {
        ConstExpr::Literal(Literal { kind, raw, .. }) => match kind {
            LiteralKind::Integer => parse_int(raw),
            LiteralKind::Char | LiteralKind::WideChar => char_literal_value(raw),
            LiteralKind::Boolean => Some(i64::from(raw.trim().eq_ignore_ascii_case("true"))),
            _ => None,
        },
        // A named enumerator or constant, resolved by its simple (last) segment:
        // the switch enum's enumerators first (union `case` path), then the
        // spec-wide enumerator set, then a named `const` (recursively evaluated).
        ConstExpr::Scoped(sn) => {
            let last = sn.parts.last()?.text.clone();
            if let Some(v) = locals.get(&last) {
                return Some(*v);
            }
            if let Some(v) = ENUM_LITERAL_VALUES.with(|m| m.borrow().get(&last).copied()) {
                return Some(v);
            }
            let value = CONST_VALUES.with(|m| m.borrow().get(&last).cloned())?;
            eval_const_int(&value, locals, depth + 1)
        }
        ConstExpr::Unary { op, operand, .. } => {
            let v = eval_const_int(operand, locals, depth + 1)?;
            match op {
                UnaryOp::Plus => Some(v),
                UnaryOp::Minus => v.checked_neg(),
                UnaryOp::BitNot => Some(!v),
            }
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let a = eval_const_int(lhs, locals, depth + 1)?;
            let b = eval_const_int(rhs, locals, depth + 1)?;
            match op {
                BinaryOp::Or => Some(a | b),
                BinaryOp::Xor => Some(a ^ b),
                BinaryOp::And => Some(a & b),
                BinaryOp::Shl => u32::try_from(b).ok().map(|s| a << s),
                BinaryOp::Shr => u32::try_from(b).ok().map(|s| a >> s),
                BinaryOp::Add => a.checked_add(b),
                BinaryOp::Sub => a.checked_sub(b),
                BinaryOp::Mul => a.checked_mul(b),
                BinaryOp::Div => a.checked_div(b),
                BinaryOp::Mod => a.checked_rem(b),
            }
        }
    }
}

/// Renders an IDL bound `ConstExpr` (always an integer literal, possibly
/// unary-signed) as a Julia integer literal for embedding in a generated
/// bound-check condition.
fn const_expr_to_julia(e: &ConstExpr) -> String {
    array_size(e).map_or_else(|| "0".to_string(), |v| v.to_string())
}

/// B1 follow-up (#22 decode-side parity is the companion fix; this is the
/// encode-side half idl-julia was entirely missing): wraps `put` — the
/// statement that actually writes a bounded `string<N>`/`wstring<N>`/
/// `sequence<T,N>`/`map<K,V,N>` value to the wire — in a `begin...end` block
/// that first rejects when `len_expr` exceeds `bound_expr`, mirroring the
/// encode-side checks already proven in idl-cpp/idl-csharp/idl-java (throw/
/// raise before writing, not a wire-only check). Julia has no established
/// house exception for generated code (this backend never raised on invalid
/// input before), so this uses the idiomatic `ArgumentError` — the same role
/// `System.ArgumentException` (csharp) / `IllegalArgumentException` (java)
/// play in their respective ecosystems.
fn bound_check_wrap(len_expr: &str, bound_expr: &str, message: &str, put: &str) -> String {
    format!(
        "begin\n    if {len_expr} > {bound_expr}\n        throw(ArgumentError(\"{message} ({bound_expr})\"))\n    end\n    {put}\nend"
    )
}

/// Decode-side mirror of [`bound_check_wrap`]: runs `get` (which assigns
/// `target`), then rejects if `len_expr` (evaluated on the now-decoded
/// value) exceeds `bound_expr`. Checked post-decode, not pre-read — the
/// value already exists in memory by then, same design idl-rust's
/// `emit_decode_bound_checks` documents (XTypes 1.3 §7.4.3: both sides).
fn decode_bound_check_wrap(get: &str, len_expr: &str, bound_expr: &str, message: &str) -> String {
    format!(
        "begin\n    {get}\n    if {len_expr} > {bound_expr}\n        throw(ArgumentError(\"{message} ({bound_expr})\"))\n    end\nend"
    )
}

/// Wraps a per-element put (`$elem`) in nested row-major `for` loops over a
/// fixed array `v.<field>[zdi0][zdi1]…` (Julia is 1-based: `1:N`).
fn build_array_put(field: &str, sizes: &[i64], elem_put: &str) -> String {
    let idx: String = (0..sizes.len()).map(|k| format!("[zdi{k}]")).collect();
    let mut body = elem_put.replace("$elem", &format!("v.{field}{idx}"));
    for k in (0..sizes.len()).rev() {
        body = format!("for zdi{k} in 1:{}\n{body}\nend", sizes[k]);
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

/// Evaluates a `char`/`wchar` literal (`'A'`, `L'x'`, `'\n'`) to its code point,
/// so a `case 'A':` union label resolves to the discriminant 65 (#A12).
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
    eval_const_int(e, enum_vals, 0)
}

/// Renders a union case label `l` as a Julia value comparable to the
/// discriminator field: `true`/`false` for a `boolean` switch, `Char(n)` for a
/// `char` switch, the enum constructor for an enum switch, else the bare
/// integer. This keeps `v.disc == <label>` well-typed — a `Char` never equals a
/// bare integer in Julia, and an `@enum` value never equals a bare integer, so
/// the non-integer discriminators (#A11/A12/A13) need the typed form.
fn render_disc_label(sw: &SwitchTypeSpec, disc_type: &str, l: i64) -> String {
    match sw {
        SwitchTypeSpec::Boolean => if l != 0 { "true" } else { "false" }.to_string(),
        SwitchTypeSpec::Char => format!("Char({l})"),
        SwitchTypeSpec::Scoped(_) => format!("{disc_type}(Int32({l}))"),
        _ => l.to_string(),
    }
}

/// Writes one `@mutable` member into writer `wv`: its EMHEADER (must-understand
/// bit 31 when `mu` — #A17 | LC4, bits 30-28 = 0b100, #A19 unchanged | member
/// id) then NEXTINT (member body length) then the body via `put` (`$w` → a
/// per-member sub-writer). Shared by the `@mutable` union framing (discriminator
/// = id 0, each branch = its 1-based id — #A16).
fn julia_mut_member_encode(out: &mut String, indent: &str, wv: &str, id: u32, mu: bool, put: &str) {
    let mu_bit = if mu { 0x8000_0000_u32 } else { 0 };
    let emh = mu_bit | 0x4000_0000 | (id & 0x0FFF_FFFF);
    let _ = writeln!(out, "{indent}put_u32!({wv}, 0x{emh:08x})");
    let _ = writeln!(out, "{indent}zdMem = Writer({wv}.endian)");
    let _ = writeln!(out, "{indent}{}", put.replace("$w", "zdMem"));
    let _ = writeln!(out, "{indent}zdMB = bytes(zdMem)");
    let _ = writeln!(out, "{indent}put_u32!({wv}, length(zdMB))");
    let _ = writeln!(out, "{indent}put_bytes!({wv}, zdMB)");
}

/// Reads one `@mutable` member: its EMHEADER + NEXTINT (LC4) then the value via
/// `get` (`$r` → `r`). Positional — it relies on members arriving in id order.
fn julia_mut_member_decode(out: &mut String, indent: &str, get: &str) {
    let _ = writeln!(out, "{indent}get_u32!(r)");
    let _ = writeln!(out, "{indent}get_u32!(r)");
    let _ = writeln!(out, "{indent}{}", get.replace("$r", "r"));
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
    zero: String,
}

/// Collects a struct's effective members base-first (#A10/P3): the base
/// struct's members (recursively) precede the derived struct's own, so the
/// generated Julia type and its wire form carry the inherited fields — matching
/// cpp/csharp/java. Without this a `struct D : Base` dropped every inherited
/// field from both the type and the wire.
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

    struct FieldGen {
        name: String,
        julia_type: String,
        put: String,
        get: String,
        id: u32,
        key: bool,
        // `Some((type_spec, expr))` for a Simple (non-array) declarator, so a
        // `@key` field can be re-mapped through `map_key_type` instead of
        // reusing `put` (which, for a struct-typed member, is the full
        // `marshal_into!` call shared with normal, non-key encoding). `None`
        // for an array declarator — `map_key_type` expects a scalar
        // TypeSpec/expr pair and would otherwise encode the array's ELEMENT
        // type once against the whole array value (wrong KeyHash: scalar-
        // encoding a list). Array key fields reuse `put` unchanged instead —
        // it already emits the correct row-major, no-length-prefix element
        // encoding (mirrors `idl-lua`'s `key_type: Option<..>` guard).
        key_type: Option<(TypeSpec, String)>,
        // `@optional`: a companion `Bool` presence flag precedes the value on
        // the wire (XTypes 1.3 §7.4.5.1.4).
        optional: bool,
        // `@must_understand`: sets the EMHEADER must-understand bit (bit 31)
        // for this member in the `@mutable` framing (#A17).
        must_understand: bool,
        // `@non_serialized`: kept in the Julia struct, off every wire form.
        non_serialized: bool,
    }
    let mut fields: Vec<FieldGen> = Vec::new();
    // Container-level `@autoid(HASH)` (XTypes 1.3 §7.3.1.2.1.1). When set, every
    // member with no explicit `@id`/`@hashid` takes a name-hashed member id
    // instead of a sequential one. Resolved through the shared frontend so the
    // ids match idl-rust/idl-cpp and the TypeObject (P0-3 member-id derivation).
    let container_hash = zerodds_idl::semantics::member_id::container_autoid_hash(&s.annotations);
    // Sequential fallback counter (`@autoid(SEQUENTIAL)`): advances ONLY for
    // members that take the positional default — an explicit `@id`, `@hashid`,
    // or `@autoid(HASH)` id does not consume a slot, matching the canonical
    // `resolve_member_ids` in `zerodds-types`.
    let mut next_seq: u32 = 0;
    // #A10/P3: the base struct's members (recursively) precede the derived
    // struct's own, so the generated type and its wire form carry every
    // inherited field — matching cpp/csharp/java. Without this a `struct D :
    // Base` dropped every inherited field from both the type and the wire.
    let mut all_members: Vec<&Member> = Vec::new();
    collect_base_members(s, structs, &mut all_members);
    for m in &all_members {
        let m = *m;
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
        // P0-5 (#2): a `@non_serialized` member keeps its Julia field but is off
        // the wire and does NOT consume a sequential id slot (ids compact).
        let non_serialized =
            zerodds_idl::semantics::annotations::member_is_non_serialized(&m.annotations);
        for d in &m.declarators {
            // Raw IDL member name (never the escaped Julia identifier): the wire
            // member-id hash is over the source spelling (XTypes §7.3.1.2.1.4).
            let raw_name = d.name().text.clone();
            let name = escape_julia_ident(&raw_name);
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
            let (julia_type, put, get, key_type) = match d {
                Declarator::Simple(_) => {
                    let expr = format!("v.{name}");
                    let (t, p) = map_type(&resolved, &expr, enum_names, struct_names)?;
                    let g = map_get(&resolved, &name, enum_names, struct_names)?;
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
                            IdlJuliaError::Unsupported(format!(
                                "non-literal array size on `{name}`"
                            ))
                        })?;
                    let (elem_type, elem_put) =
                        map_type(&resolved, "$elem", enum_names, struct_names)?;
                    let julia_type = sizes
                        .iter()
                        .fold(elem_type.clone(), |inner, _| format!("Vector{{{inner}}}"));
                    let put = build_array_put(&name, &sizes, &elem_put);
                    let idx: String = (0..sizes.len()).map(|k| format!("[zdi{k}]")).collect();
                    let elem_get =
                        map_get(&resolved, &format!("{name}{idx}"), enum_names, struct_names)?;
                    let get = build_array_get(&name, &sizes, &elem_type, &elem_get);
                    (julia_type, put, get, None)
                }
            };
            fields.push(FieldGen {
                name,
                julia_type,
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

    let ty = escape_julia_ident(&qualify(scope, &s.name.text));
    let _ = writeln!(out, "\nstruct {ty}");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "    ", &s.annotations, PlacementKind::BeginDeclaration);
    for f in &fields {
        // An `@optional` member carries a companion presence flag (§7.4.5.1.4:
        // Bool present-flag then the value, only present if the flag is set).
        if f.optional {
            let _ = writeln!(out, "    {}_present::Bool", f.name);
        }
        let _ = writeln!(out, "    {}::{}", f.name, f.julia_type);
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "    ", &s.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "end");

    // Emits the inline positional member puts into writer var `wv` (shared by
    // @final and both XCDR1/XCDR2 @appendable branches). Alignment (max 4 XCDR2 /
    // max 8 XCDR1) is a property of the writer mode, so one body serves both.
    let emit_inline_puts = |out: &mut String, wv: &str| {
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            if f.optional {
                // Bool presence flag then the value if present (§7.4.5.1.4).
                let _ = writeln!(out, "    put_u8!({wv}, v.{}_present ? 1 : 0)", f.name);
                let _ = writeln!(out, "    if v.{}_present", f.name);
                let _ = writeln!(out, "        {}", f.put.replace("$w", wv));
                let _ = writeln!(out, "    end");
            } else {
                let _ = writeln!(out, "    {}", f.put.replace("$w", wv));
            }
        }
    };

    // marshal_into! writes into an existing writer (nested composites call this
    // so alignment stays stream-relative). @final: inline; @appendable: DHEADER
    // (XCDR2) or inline (XCDR1); @mutable: PL_CDR2 EMHEADER list (XCDR2) or
    // PL_CDR1 PID list (XCDR1). The writer's `xcdr1` flag selects the branch at
    // run time so a single generated type serves both wire representations.
    let _ = writeln!(out, "\nfunction marshal_into!(v::{ty}, w::Writer)");
    if ext == ExtensibilityKind::Mutable {
        // XCDR1 classic CDR: @mutable is PL_CDR1 — a `[PID][len]` member list
        // with no outer DHEADER, each member body built member-relative in an
        // XCDR1 sub-writer, terminated by the sentinel.
        let _ = writeln!(out, "    if w.xcdr1");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            if f.optional {
                let _ = writeln!(out, "    if v.{}_present", f.name);
            }
            let _ = writeln!(out, "        zdMem = Writer(w.endian)");
            let _ = writeln!(out, "        zdMem.xcdr1 = true");
            let _ = writeln!(out, "        {}", f.put.replace("$w", "zdMem"));
            let _ = writeln!(
                out,
                "        write_pl_cdr1_member!(w, UInt32(0x{:08x}), bytes(zdMem))",
                f.id & 0x0FFF_FFFF
            );
            if f.optional {
                let _ = writeln!(out, "    end");
            }
        }
        let _ = writeln!(out, "        write_pl_cdr1_sentinel!(w)");
        let _ = writeln!(out, "        return nothing");
        let _ = writeln!(out, "    end");
        // XCDR2: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "    body = Writer(w.endian)");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            // An `@optional` member is omitted from the member list when absent
            // (XTypes 1.3 §7.4.3.4.2): guard its EMHEADER+body on the flag.
            if f.optional {
                let _ = writeln!(out, "    if v.{}_present", f.name);
            }
            // EMHEADER: must-understand bit 31 (#A17) | LC4 (bits 30-28 =
            // 0b100, #A19 unchanged — the shared byte-identity stand) | id.
            let mu_bit = if f.must_understand {
                0x8000_0000_u32
            } else {
                0
            };
            let emh = mu_bit | 0x4000_0000 | (f.id & 0x0FFF_FFFF);
            let _ = writeln!(out, "    put_u32!(body, 0x{emh:08x})");
            let _ = writeln!(out, "    zdMem = Writer(w.endian)");
            let _ = writeln!(out, "    {}", f.put.replace("$w", "zdMem"));
            let _ = writeln!(out, "    zdMB = bytes(zdMem)");
            let _ = writeln!(out, "    put_u32!(body, length(zdMB))");
            let _ = writeln!(out, "    put_bytes!(body, zdMB)");
            if f.optional {
                let _ = writeln!(out, "    end");
            }
        }
        let _ = writeln!(out, "    zdBB = bytes(body)");
        let _ = writeln!(out, "    put_u32!(w, length(zdBB))");
        let _ = writeln!(out, "    put_bytes!(w, zdBB)");
    } else if ext == ExtensibilityKind::Appendable {
        // XCDR1: inline (no DHEADER). XCDR2: length-prefixed member block.
        let _ = writeln!(out, "    if w.xcdr1");
        emit_inline_puts(out, "w");
        let _ = writeln!(out, "        return nothing");
        let _ = writeln!(out, "    end");
        let _ = writeln!(out, "    body = Writer(w.endian)");
        emit_inline_puts(out, "body");
        let _ = writeln!(out, "    bb = bytes(body)");
        let _ = writeln!(out, "    put_u32!(w, length(bb))");
        let _ = writeln!(out, "    put_bytes!(w, bb)");
    } else {
        emit_inline_puts(out, "w");
    }
    let _ = writeln!(out, "    nothing");
    let _ = writeln!(out, "end");

    let _ = writeln!(
        out,
        "\nfunction marshal_xcdr(v::{ty}, endian::Endian)::Vector{{UInt8}}"
    );
    let _ = writeln!(out, "    w = Writer(endian)");
    let _ = writeln!(out, "    marshal_into!(v, w)");
    let _ = writeln!(out, "    bytes(w)");
    let _ = writeln!(out, "end");
    // XCDR1 (classic CDR) entry point: same member logic, max-alignment-8 writer,
    // no DHEADER, PL_CDR1 @mutable framing (parity with idl-rust's `encode_xcdr1`).
    let _ = writeln!(
        out,
        "\nfunction marshal_xcdr1(v::{ty}, endian::Endian)::Vector{{UInt8}}"
    );
    let _ = writeln!(out, "    w = Writer(endian)");
    let _ = writeln!(out, "    w.xcdr1 = true");
    let _ = writeln!(out, "    marshal_into!(v, w)");
    let _ = writeln!(out, "    bytes(w)");
    let _ = writeln!(out, "end");
    let mut zdkeys: Vec<&FieldGen> = fields
        .iter()
        .filter(|f| f.key && !f.non_serialized)
        .collect();
    zdkeys.sort_by_key(|f| f.id);
    if !zdkeys.is_empty() {
        // #A10: inherited `@key` members participate in the KeyHash too, so
        // the max-size (MD5) analysis runs over the base-first member set.
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
        let _ = writeln!(out, "\nfunction key_hash(v::{ty})::Vector{{UInt8}}");
        let _ = writeln!(out, "    kw = Writer(BE)");
        for put in &key_puts {
            let _ = writeln!(out, "    {}", put.replace("$w", "kw"));
        }
        let _ = writeln!(out, "    b = bytes(kw)");
        if use_md5 {
            // KeyHolder max size > 16 → MD5(bytes)[0..16] (XTypes §7.6.8.4).
            let _ = writeln!(out, "    zd_md5(b)");
        } else {
            let _ = writeln!(out, "    outk = zeros(UInt8, 16)");
            let _ = writeln!(out, "    for i in 1:min(16, length(b)) outk[i] = b[i] end");
            let _ = writeln!(out, "    outk");
        }
        let _ = writeln!(out, "end");
    }

    let args = fields
        .iter()
        .flat_map(|f| {
            if f.optional {
                vec![format!("{}_present", f.name), f.name.clone()]
            } else {
                vec![f.name.clone()]
            }
        })
        .collect::<Vec<_>>()
        .join(", ");

    // Decode (inverse of marshal_into!). Julia structs are immutable, so each
    // field is read into a local and the struct is constructed positionally.
    // @final reads inline; @appendable skips the DHEADER (XCDR2 only); @mutable
    // is a PL_CDR2 EMHEADER list (XCDR2) or a PL_CDR1 PID list (XCDR1).
    let _ = writeln!(out, "\nfunction read_{ty}(r::Reader)::{ty}");
    if ext == ExtensibilityKind::Mutable {
        // XCDR1: PL_CDR1 PID-keyed member list (no outer DHEADER). Collect each
        // member body, then decode it from its own member-relative XCDR1 reader.
        // An absent id leaves the field at its default (and clears the presence
        // flag) — the correct @optional / omitted-member behaviour.
        let _ = writeln!(out, "    if r.xcdr1");
        let _ = writeln!(out, "        zd_endian = r.endian");
        let _ = writeln!(out, "        zd_pl = Dict{{UInt32, Vector{{UInt8}}}}()");
        let _ = writeln!(out, "        while true");
        let _ = writeln!(out, "            zdm = read_pl_cdr1_member!(r)");
        let _ = writeln!(out, "            zdm === nothing && break");
        let _ = writeln!(out, "            zd_pl[zdm[1]] = zdm[2]");
        let _ = writeln!(out, "        end");
        for f in &fields {
            let zero = zero_value(&f.julia_type, enum_names);
            if !f.non_serialized && f.optional {
                let _ = writeln!(out, "        {}_present = false", f.name);
            }
            let _ = writeln!(out, "        {} = {zero}", f.name);
        }
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            let id28 = f.id & 0x0FFF_FFFF;
            if f.optional {
                let _ = writeln!(
                    out,
                    "        {}_present = haskey(zd_pl, UInt32(0x{id28:08x}))",
                    f.name
                );
            }
            let _ = writeln!(out, "        if haskey(zd_pl, UInt32(0x{id28:08x}))");
            let _ = writeln!(
                out,
                "            zdr = Reader(zd_pl[UInt32(0x{id28:08x})], zd_endian)"
            );
            let _ = writeln!(out, "            zdr.xcdr1 = true");
            let _ = writeln!(out, "            {}", f.get.replace("$r", "zdr"));
            let _ = writeln!(out, "        end");
        }
        let _ = writeln!(out, "        return {ty}({args})");
        let _ = writeln!(out, "    end");
        // XCDR2: DHEADER + EMHEADER-framed positional decode.
        let _ = writeln!(out, "    get_u32!(r)");
        for f in &fields {
            if f.non_serialized {
                // Off the wire: bind the local to the type's zero value so the
                // positional constructor below still receives it (P0-5, #2).
                let zero = zero_value(&f.julia_type, enum_names);
                let _ = writeln!(out, "    {} = {zero}", f.name);
                continue;
            }
            // Mutable-optional decode rides the naive decoder: it assumes every
            // member is present in declaration order and does NOT reconcile an
            // omitted `@optional` member against its EMHEADER id (XTypes 1.3
            // §7.4.3.4.2). Round-trip of an all-present value is exact; an
            // absent optional is not reconstructed. Full parity is deferred.
            if f.optional {
                let _ = writeln!(out, "    {}_present = true", f.name);
            }
            let _ = writeln!(out, "    get_u32!(r)");
            let _ = writeln!(out, "    get_u32!(r)");
            let _ = writeln!(out, "    {}", f.get.replace("$r", "r"));
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            // XCDR2 frames the appendable member block with a DHEADER; XCDR1
            // classic CDR has none.
            let _ = writeln!(out, "    if !r.xcdr1");
            let _ = writeln!(out, "        get_u32!(r)");
            let _ = writeln!(out, "    end");
        }
        for f in &fields {
            if f.non_serialized {
                // Off the wire: bind the local to the type's zero value (P0-5, #2).
                let zero = zero_value(&f.julia_type, enum_names);
                let _ = writeln!(out, "    {} = {zero}", f.name);
                continue;
            }
            if f.optional {
                // Bool presence flag then the value only if present; the value
                // local is zero-initialised for the absent case (§7.4.5.1.4).
                let zero = zero_value(&f.julia_type, enum_names);
                let _ = writeln!(out, "    {}_present = get_bool!(r)", f.name);
                let _ = writeln!(out, "    {} = {zero}", f.name);
                let _ = writeln!(out, "    if {}_present", f.name);
                let _ = writeln!(out, "        {}", f.get.replace("$r", "r"));
                let _ = writeln!(out, "    end");
            } else {
                let _ = writeln!(out, "    {}", f.get.replace("$r", "r"));
            }
        }
    }
    let _ = writeln!(out, "    {ty}({args})");
    let _ = writeln!(out, "end");
    let _ = writeln!(
        out,
        "\nunmarshal_xcdr_{ty}(buf::Vector{{UInt8}}, endian::Endian)::{ty} = read_{ty}(Reader(buf, endian))"
    );
    // XCDR1 (classic CDR) decode entry point (parity with `decode_xcdr1`).
    let _ = writeln!(
        out,
        "function unmarshal_xcdr1_{ty}(buf::Vector{{UInt8}}, endian::Endian)::{ty}\n    r = Reader(buf, endian)\n    r.xcdr1 = true\n    read_{ty}(r)\nend"
    );
    Ok(())
}

/// Emits an IDL `union` as a discriminated holder + a `marshalInto` that puts
/// the discriminator then dispatches on it to the selected member (XCDR2
/// §7.4.3.5.4). `@final`: inline; `@appendable`: DHEADER-framed body.
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
    let (disc_type, disc_put) = map_type(
        &switch_typespec(&u.switch_type),
        "v.disc",
        enum_names,
        struct_names,
    )?;
    let disc_get = map_get(
        &switch_typespec(&u.switch_type),
        "zdDisc",
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

    let mut cases: Vec<UnionCase> = Vec::new();
    for c in &u.cases {
        let field = escape_julia_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ty, put) = map_type(&resolved, &format!("v.{field}"), enum_names, struct_names)?;
        let get = map_get(&resolved, &field, enum_names, struct_names)?;
        let zero = zero_value(&ty, enum_names);
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => {
                    labels.push(eval_union_label(e, &enum_vals).ok_or_else(|| {
                        IdlJuliaError::Unsupported(format!(
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
            ty,
            put,
            get,
            zero,
        });
    }

    // Renders a case's `if`/`elseif`/`else` header against `disc_expr`, with
    // labels typed to the discriminator (#A11/A12/A13); `default` → `else`.
    let case_cond = |disc_expr: &str, i: usize, c: &UnionCase| -> String {
        if c.is_default {
            "    else".to_string()
        } else {
            let kw = if i == 0 { "if" } else { "elseif" };
            let cond = c
                .labels
                .iter()
                .map(|&l| {
                    format!(
                        "{disc_expr} == {}",
                        render_disc_label(&u.switch_type, &disc_type, l)
                    )
                })
                .collect::<Vec<_>>()
                .join(" || ");
            format!("    {kw} {cond}")
        }
    };

    let ty = escape_julia_ident(&qualify(scope, &u.name.text));
    let _ = writeln!(out, "\nstruct {ty}");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "    ", &u.annotations, PlacementKind::BeginDeclaration);
    let _ = writeln!(out, "    disc::{disc_type}");
    for c in &cases {
        let _ = writeln!(out, "    {}::{}", c.field, c.ty);
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "    ", &u.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "end");
    // Inline disc + selected-branch puts into writer var `wv` (shared by @final
    // and both @appendable branches; alignment handled by the writer mode).
    let emit_union_inline = |out: &mut String, wv: &str| {
        let _ = writeln!(out, "    {}", disc_put.replace("$w", wv));
        for (i, c) in cases.iter().enumerate() {
            let _ = writeln!(out, "{}", case_cond("v.disc", i, c));
            let _ = writeln!(out, "        {}", c.put.replace("$w", wv));
        }
        if !cases.is_empty() {
            let _ = writeln!(out, "    end");
        }
    };

    let _ = writeln!(out, "\nfunction marshal_into!(v::{ty}, w::Writer)");
    if ext == ExtensibilityKind::Mutable {
        // XCDR1 PL_CDR1: disc = member id 0, selected branch = member id
        // (case-index + 1), no outer DHEADER, terminated by the sentinel.
        let _ = writeln!(out, "    if w.xcdr1");
        let _ = writeln!(out, "        zdMem = Writer(w.endian)");
        let _ = writeln!(out, "        zdMem.xcdr1 = true");
        let _ = writeln!(out, "        {}", disc_put.replace("$w", "zdMem"));
        let _ = writeln!(
            out,
            "        write_pl_cdr1_member!(w, UInt32(0), bytes(zdMem))"
        );
        for (i, c) in cases.iter().enumerate() {
            let _ = writeln!(out, "{}", case_cond("v.disc", i, c));
            let id = u32::try_from(i + 1).unwrap_or(0);
            let _ = writeln!(out, "        zdMem = Writer(w.endian)");
            let _ = writeln!(out, "        zdMem.xcdr1 = true");
            let _ = writeln!(out, "        {}", c.put.replace("$w", "zdMem"));
            let _ = writeln!(
                out,
                "        write_pl_cdr1_member!(w, UInt32({id}), bytes(zdMem))"
            );
        }
        if !cases.is_empty() {
            let _ = writeln!(out, "    end");
        }
        let _ = writeln!(out, "        write_pl_cdr1_sentinel!(w)");
        let _ = writeln!(out, "        return nothing");
        let _ = writeln!(out, "    end");
        // #A16: DHEADER-framed EMHEADER member list — the discriminator is
        // member id 0, each branch its 1-based id (XTypes §7.4.3.4.2). LC4
        // framing (#A19 unchanged — the shared byte-identity stand).
        let _ = writeln!(out, "    body = Writer(w.endian)");
        julia_mut_member_encode(out, "    ", "body", 0, false, &disc_put);
        for (i, c) in cases.iter().enumerate() {
            let _ = writeln!(out, "{}", case_cond("v.disc", i, c));
            let id = u32::try_from(i + 1).unwrap_or(0);
            julia_mut_member_encode(out, "        ", "body", id, false, &c.put);
        }
        if !cases.is_empty() {
            let _ = writeln!(out, "    end");
        }
        let _ = writeln!(out, "    zdBB = bytes(body)");
        let _ = writeln!(out, "    put_u32!(w, length(zdBB))");
        let _ = writeln!(out, "    put_bytes!(w, zdBB)");
    } else if ext == ExtensibilityKind::Appendable {
        // XCDR1: inline (no DHEADER). XCDR2: length-prefixed member block.
        let _ = writeln!(out, "    if w.xcdr1");
        emit_union_inline(out, "w");
        let _ = writeln!(out, "        return nothing");
        let _ = writeln!(out, "    end");
        let _ = writeln!(out, "    body = Writer(w.endian)");
        emit_union_inline(out, "body");
        let _ = writeln!(out, "    bb = bytes(body)");
        let _ = writeln!(out, "    put_u32!(w, length(bb))");
        let _ = writeln!(out, "    put_bytes!(w, bb)");
    } else {
        emit_union_inline(out, "w");
    }
    let _ = writeln!(out, "    nothing");
    let _ = writeln!(out, "end");
    let _ = writeln!(
        out,
        "\nfunction marshal_xcdr(v::{ty}, endian::Endian)::Vector{{UInt8}}"
    );
    let _ = writeln!(out, "    w = Writer(endian)");
    let _ = writeln!(out, "    marshal_into!(v, w)");
    let _ = writeln!(out, "    bytes(w)");
    let _ = writeln!(out, "end");
    // XCDR1 (classic CDR) entry point (see the struct emitter).
    let _ = writeln!(
        out,
        "\nfunction marshal_xcdr1(v::{ty}, endian::Endian)::Vector{{UInt8}}"
    );
    let _ = writeln!(out, "    w = Writer(endian)");
    let _ = writeln!(out, "    w.xcdr1 = true");
    let _ = writeln!(out, "    marshal_into!(v, w)");
    let _ = writeln!(out, "    bytes(w)");
    let _ = writeln!(out, "end");

    // Decode: read the discriminator, zero-fill the case members, then read only
    // the selected member. @appendable skips the leading DHEADER; @mutable skips
    // the DHEADER then reads the discriminator EMHEADER + NEXTINT and, per branch,
    // the selected member's EMHEADER + NEXTINT (positional — a fully-present
    // union round-trips). The immutable holder is constructed positionally.
    let args = cases
        .iter()
        .map(|c| c.field.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let sep = if cases.is_empty() { "" } else { ", " };
    let disc_zero = zero_value(&disc_type, enum_names);
    let _ = writeln!(out, "\nfunction read_{ty}(r::Reader)::{ty}");
    let mutable = ext == ExtensibilityKind::Mutable;
    if mutable {
        // XCDR1: PL_CDR1 PID-keyed member list. Disc = member id 0, selected
        // branch = member id (case-index + 1); each decoded from its own
        // member-relative XCDR1 reader.
        let _ = writeln!(out, "    if r.xcdr1");
        let _ = writeln!(out, "        zd_endian = r.endian");
        let _ = writeln!(out, "        zd_pl = Dict{{UInt32, Vector{{UInt8}}}}()");
        let _ = writeln!(out, "        while true");
        let _ = writeln!(out, "            zdm = read_pl_cdr1_member!(r)");
        let _ = writeln!(out, "            zdm === nothing && break");
        let _ = writeln!(out, "            zd_pl[zdm[1]] = zdm[2]");
        let _ = writeln!(out, "        end");
        let _ = writeln!(out, "        zdDisc = {disc_zero}");
        let _ = writeln!(out, "        if haskey(zd_pl, UInt32(0))");
        let _ = writeln!(out, "            zdr = Reader(zd_pl[UInt32(0)], zd_endian)");
        let _ = writeln!(out, "            zdr.xcdr1 = true");
        let _ = writeln!(out, "            {}", disc_get.replace("$r", "zdr"));
        let _ = writeln!(out, "        end");
        for c in &cases {
            let _ = writeln!(out, "        {} = {}", c.field, c.zero);
        }
        for (i, c) in cases.iter().enumerate() {
            let _ = writeln!(out, "{}", case_cond("zdDisc", i, c));
            let id = u32::try_from(i + 1).unwrap_or(0);
            let _ = writeln!(out, "        if haskey(zd_pl, UInt32({id}))");
            let _ = writeln!(
                out,
                "            zdr = Reader(zd_pl[UInt32({id})], zd_endian)"
            );
            let _ = writeln!(out, "            zdr.xcdr1 = true");
            let _ = writeln!(out, "            {}", c.get.replace("$r", "zdr"));
            let _ = writeln!(out, "        end");
        }
        if !cases.is_empty() {
            let _ = writeln!(out, "    end");
        }
        let _ = writeln!(out, "        return {ty}(zdDisc{sep}{args})");
        let _ = writeln!(out, "    end");
        // XCDR2: DHEADER + EMHEADER-framed positional decode.
        let _ = writeln!(out, "    get_u32!(r)");
        julia_mut_member_decode(out, "    ", &disc_get);
    } else {
        if ext == ExtensibilityKind::Appendable {
            // XCDR2 frames the appendable body with a DHEADER; XCDR1 has none.
            let _ = writeln!(out, "    if !r.xcdr1");
            let _ = writeln!(out, "        get_u32!(r)");
            let _ = writeln!(out, "    end");
        }
        let _ = writeln!(out, "    {}", disc_get.replace("$r", "r"));
    }
    for c in &cases {
        let _ = writeln!(out, "    {} = {}", c.field, c.zero);
    }
    for (i, c) in cases.iter().enumerate() {
        let _ = writeln!(out, "{}", case_cond("zdDisc", i, c));
        if mutable {
            julia_mut_member_decode(out, "        ", &c.get);
        } else {
            let _ = writeln!(out, "        {}", c.get.replace("$r", "r"));
        }
    }
    if !cases.is_empty() {
        let _ = writeln!(out, "    end");
    }
    let _ = writeln!(out, "    {ty}(zdDisc{sep}{args})");
    let _ = writeln!(out, "end");
    let _ = writeln!(
        out,
        "\nunmarshal_xcdr_{ty}(buf::Vector{{UInt8}}, endian::Endian)::{ty} = read_{ty}(Reader(buf, endian))"
    );
    // XCDR1 (classic CDR) decode entry point.
    let _ = writeln!(
        out,
        "function unmarshal_xcdr1_{ty}(buf::Vector{{UInt8}}, endian::Endian)::{ty}\n    r = Reader(buf, endian)\n    r.xcdr1 = true\n    read_{ty}(r)\nend"
    );
    Ok(())
}

/// Emits the `<Iface>_Client` / `<Iface>_Handler` surface for an interface's
/// operations and attributes (#11). Operations carry no wire form, so these are
/// pure native-Julia service contracts mirroring the idl-ts / idl-swift
/// Client/Handler surface: an abstract type per role plus a generic-function
/// declaration per operation / attribute accessor (`function op end`), the
/// resolved parameter/return types documented in a preceding comment. Julia
/// dispatches on the concrete Client/Handler subtype the user defines; NO wire
/// runtime is invented — there is no Julia `zerodds-rpc` runtime, so a
/// requester/replier wrapper would be fictional (honest limit, not deferral).
/// The interface's nested data TYPES round-trip separately (#A39).
fn emit_interface_surface(
    out: &mut String,
    iface: &InterfaceDef,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
) {
    // Param/return type references resolve against the interface's own scope so
    // an interface-nested type maps to the promoted `I_C` name (#A39).
    let mut inner_scope = scope.to_vec();
    inner_scope.push(iface.name.text.clone());
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = inner_scope);

    let base = escape_julia_ident(&qualify(scope, &iface.name.text));
    // A single base → a Julia abstract-type supertype (`<: Base_Client`); Julia
    // has no multiple abstract inheritance, so extra bases are documented below.
    let supertype = |suffix: &str| -> String {
        match iface.bases.first() {
            Some(first) => format!(
                " <: {}{suffix}",
                escape_julia_ident(&resolve_scoped_name(first))
            ),
            None => String::new(),
        }
    };
    for suffix in ["_Client", "_Handler"] {
        let _ = writeln!(
            out,
            "\nabstract type {base}{suffix}{} end",
            supertype(suffix)
        );
    }
    if iface.bases.len() > 1 {
        let extra = iface.bases[1..]
            .iter()
            .map(resolve_scoped_name)
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            out,
            "# note: additional interface bases not expressible as a Julia supertype: {extra}"
        );
    }

    for ex in &iface.exports {
        match ex {
            Export::Op(op) => emit_op_surface(out, op, &base, enum_names, struct_names, typedefs),
            Export::Attr(attr) => {
                emit_attr_surface(out, attr, &base, enum_names, struct_names, typedefs);
            }
            _ => {}
        }
    }
}

/// Renders one interface operation as a documented generic-function declaration.
/// `in`/`inout` params become call arguments; `out`/`inout` params (plus the
/// return value) fold into a return `Tuple` when more than one, a bare type for
/// exactly one, `Nothing` for `void`. An operation whose signature references a
/// type the Julia backend cannot map is emitted as a `#` placeholder rather than
/// aborting the whole module.
fn emit_op_surface(
    out: &mut String,
    op: &OpDecl,
    base: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
) {
    let map = |t: &TypeSpec| -> Option<String> {
        let resolved = resolve_typedef(t, typedefs);
        map_type(&resolved, "_", enum_names, struct_names)
            .ok()
            .map(|(ty, _)| ty)
    };
    let mut params: Vec<String> = Vec::new();
    let mut out_entries: Vec<String> = Vec::new();
    for p in &op.params {
        let Some(ty) = map(&p.type_spec) else {
            let _ = writeln!(
                out,
                "# unsupported operation `{}` (type not representable in Julia)",
                op.name.text
            );
            return;
        };
        let pname = escape_julia_ident(&p.name.text);
        match p.attribute {
            ParamAttribute::In => params.push(format!("{pname}::{ty}")),
            ParamAttribute::InOut => {
                params.push(format!("{pname}::{ty}"));
                out_entries.push(ty);
            }
            ParamAttribute::Out => out_entries.push(ty),
        }
    }
    let mut rets: Vec<String> = Vec::new();
    if let Some(rt) = &op.return_type {
        let Some(ty) = map(rt) else {
            let _ = writeln!(
                out,
                "# unsupported operation `{}` (return type not representable in Julia)",
                op.name.text
            );
            return;
        };
        rets.push(ty);
    }
    rets.extend(out_entries);
    let ret_clause = match rets.len() {
        0 => "::Nothing".to_string(),
        1 => format!("::{}", rets[0]),
        _ => format!("::Tuple{{{}}}", rets.join(", ")),
    };
    let name = escape_julia_ident(&op.name.text);
    let _ = writeln!(
        out,
        "# {base}_Client / {base}_Handler operation: {name}({}){ret_clause}",
        params.join(", ")
    );
    let _ = writeln!(out, "function {name} end");
}

/// Renders the getter (and, unless `readonly`, setter) generic functions for an
/// IDL interface attribute.
fn emit_attr_surface(
    out: &mut String,
    attr: &AttrDecl,
    base: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
) {
    let resolved = resolve_typedef(&attr.type_spec, typedefs);
    let Ok((ty, _)) = map_type(&resolved, "_", enum_names, struct_names) else {
        let _ = writeln!(
            out,
            "# unsupported attribute `{}` (type not representable in Julia)",
            attr.name.text
        );
        return;
    };
    let name = escape_julia_ident(&attr.name.text);
    let _ = writeln!(
        out,
        "# {base}_Client / {base}_Handler attribute getter: get_{name}()::{ty}"
    );
    let _ = writeln!(out, "function get_{name} end");
    if !attr.readonly {
        let _ = writeln!(
            out,
            "# {base}_Client / {base}_Handler attribute setter: set_{name}(value::{ty})::Nothing"
        );
        let _ = writeln!(out, "function set_{name} end");
    }
}

/// Maps an IDL type to `(Julia type, put statement)`. The put uses `$w` as the
/// writer placeholder and `expr` as the value expression.
/// A type is "primitive" for the map-DHEADER rule if it is fully descriptive on
/// the wire: an IDL primitive or an enum (i32). Others force a collection DHEADER.
fn is_primitive(t: &TypeSpec, enum_names: &HashSet<String>) -> bool {
    match t {
        TypeSpec::Primitive(_) => true,
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            enum_names.contains(&name) || is_bit_name(&name)
        }
        _ => false,
    }
}

/// Builds a map put: `u32 count` + key/value pairs sorted ascending by key
/// (DHEADER-framed unless the key/value pair is primitive).
fn build_map_put(expr: &str, key_put: &str, val_put: &str, prim: bool) -> String {
    if prim {
        format!(
            "begin\n    put_u32!($w, length({expr}))\n    for zdK in sort(collect(keys({expr})))\n        {key_put}\n        {val_put}\n    end\nend"
        )
    } else {
        let kp = key_put.replace("$w", "zdSub");
        let vp = val_put.replace("$w", "zdSub");
        // XCDR2: collection DHEADER over the sub-writer body. XCDR1 classic CDR:
        // no DHEADER — entries written stream-relative into `$w`.
        format!(
            "if $w.xcdr1\n    put_u32!($w, length({expr}))\n    for zdK in sort(collect(keys({expr})))\n        {key_put}\n        {val_put}\n    end\nelse\n    begin\n    zdSub = Writer($w.endian)\n    put_u32!(zdSub, length({expr}))\n    for zdK in sort(collect(keys({expr})))\n        {kp}\n        {vp}\n    end\n    zdBB = bytes(zdSub)\n    put_u32!($w, length(zdBB))\n    put_bytes!($w, zdBB)\n    end\nend"
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
        // Bounded `wstring<N>` (DDS-XTypes §7.4.3): N counts UTF-16 code
        // units (surrogate pairs = 2), matching the unit-count `put_wstring!`
        // itself writes below — reject over-bound before writing.
        TypeSpec::String(st) if st.wide => {
            let put = format!("put_wstring!($w, {expr})");
            Ok((
                "String".to_string(),
                match &st.bound {
                    Some(b) => bound_check_wrap(
                        &format!("sum(c -> (UInt32(c) <= 0xFFFF ? 1 : 2), {expr}; init=0)"),
                        &const_expr_to_julia(b),
                        "bounded wstring length exceeds its IDL bound",
                        &put,
                    ),
                    None => put,
                },
            ))
        }
        // Bounded narrow `string<N>` (DDS-XTypes §7.4.3): N counts UTF-8
        // bytes (`sizeof` on a Julia `String`, matching what `put_string!`
        // writes as the wire length below, minus the NUL terminator it adds).
        TypeSpec::String(st) if !st.wide => {
            let put = format!("put_string!($w, {expr})");
            Ok((
                "String".to_string(),
                match &st.bound {
                    Some(b) => bound_check_wrap(
                        &format!("sizeof({expr})"),
                        &const_expr_to_julia(b),
                        "bounded string length exceeds its IDL bound",
                        &put,
                    ),
                    None => put,
                },
            ))
        }
        TypeSpec::Sequence(seq) => {
            let (ty, put) = map_sequence(&seq.elem, expr, enum_names, struct_names)?;
            let put = match &seq.bound {
                Some(b) => bound_check_wrap(
                    &format!("length({expr})"),
                    &const_expr_to_julia(b),
                    "bounded sequence length exceeds its IDL bound",
                    &put,
                ),
                None => put,
            };
            Ok((ty, put))
        }
        // A `fixed<P,S>` decimal: packed BCD, `(P+2)/2` raw octets, no length
        // prefix and no alignment (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5). The
        // Julia field holds the BCD bytes directly; `zd_fixed_enc` builds them
        // from a decimal string.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let _ = fixed_ps(f)?; // validate P/S resolve at codegen time
            Ok((
                "Vector{UInt8}".to_string(),
                format!("put_bytes!($w, {expr})"),
            ))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Enum holder width follows @bit_bound (XTypes 1.3 §7.4.5.1);
                // `% Int8`/`% Int16` truncates modularly to the narrow signed
                // holder, then reinterpret to the unsigned put helper.
                let put = match enum_wire_width(&name) {
                    1 => format!("put_u8!($w, reinterpret(UInt8, Integer({expr}) % Int8))"),
                    2 => format!("put_u16!($w, reinterpret(UInt16, Integer({expr}) % Int16))"),
                    _ => format!("put_u32!($w, reinterpret(UInt32, Int32(Integer({expr}))))"),
                };
                Ok((escape_julia_ident(&name), put))
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                Ok((
                    escape_julia_ident(&name),
                    format!("marshal_into!({expr}, $w)"),
                ))
            } else {
                Err(IdlJuliaError::Unsupported(format!("scoped type {name}")))
            }
        }
        // A map: entries sorted ascending by key, `u32 count` + key/value pairs
        // (no DHEADER for a primitive pair; DHEADER-framed otherwise).
        TypeSpec::Map(m) => {
            let (key_type, key_put) = map_type(&m.key, "zdK", enum_names, struct_names)?;
            let (val_type, val_put) =
                map_type(&m.value, &format!("{expr}[zdK]"), enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let put = build_map_put(expr, &key_put, &val_put, prim);
            let put = match &m.bound {
                Some(b) => bound_check_wrap(
                    &format!("length({expr})"),
                    &const_expr_to_julia(b),
                    "bounded map length exceeds its IDL bound",
                    &put,
                ),
                None => put,
            };
            Ok((format!("Dict{{{key_type}, {val_type}}}"), put))
        }
        other => Err(IdlJuliaError::Unsupported(format!("type {other:?}"))),
    }
}

/// Maps a `@key` member's type to zero or more `KeyHash`-body put statements
/// (each using the `$w` writer placeholder, consistent with [`map_type`]'s
/// `put`).
///
/// Unlike [`map_type`] — shared with normal (non-key) member encoding, where a
/// struct-typed member always emits the struct's FULL `marshal_into!` — a
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
            // `compute_key_holder`/`encode_key_holder` (P0-3).
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
                        return Err(IdlJuliaError::Unsupported(
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
        PrimitiveType::Octet => ("UInt8", format!("put_u8!($w, {expr})")),
        PrimitiveType::Boolean => ("Bool", format!("put_bool!($w, {expr})")),
        PrimitiveType::Char => ("Char", format!("put_u8!($w, UInt8({expr}))")),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => {
            ("Float32", format!("put_f32!($w, {expr})"))
        }
        PrimitiveType::Floating(FloatingType::Double) => {
            ("Float64", format!("put_f64!($w, {expr})"))
        }
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            ("Float64", format!("put_long_double!($w, {expr})"))
        }
        PrimitiveType::WideChar => ("UInt32", format!("put_u32!($w, {expr})")),
    };
    Ok((ty.to_string(), put))
}

fn map_integer(i: IntegerType, expr: &str) -> Result<(String, String)> {
    // Signed IDL integers reinterpret to the unsigned wire.
    let (ty, put) = match i {
        IntegerType::UInt8 => ("UInt8", format!("put_u8!($w, {expr})")),
        IntegerType::Int8 => ("Int8", format!("put_u8!($w, reinterpret(UInt8, {expr}))")),
        IntegerType::UShort | IntegerType::UInt16 => ("UInt16", format!("put_u16!($w, {expr})")),
        IntegerType::Short | IntegerType::Int16 => (
            "Int16",
            format!("put_u16!($w, reinterpret(UInt16, {expr}))"),
        ),
        IntegerType::ULong | IntegerType::UInt32 => ("UInt32", format!("put_u32!($w, {expr})")),
        IntegerType::Long | IntegerType::Int32 => (
            "Int32",
            format!("put_u32!($w, reinterpret(UInt32, {expr}))"),
        ),
        IntegerType::ULongLong | IntegerType::UInt64 => ("UInt64", format!("put_u64!($w, {expr})")),
        IntegerType::LongLong | IntegerType::Int64 => (
            "Int64",
            format!("put_u64!($w, reinterpret(UInt64, {expr}))"),
        ),
    };
    Ok((ty.to_string(), put))
}

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_sequence(
    elem: &TypeSpec,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok((
            "Vector{UInt8}".to_string(),
            format!("put_seq_u8!($w, {expr})"),
        ));
    }
    // sequence<struct> → collection DHEADER + count + each element.
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let esc = escape_julia_ident(&name);
            // XCDR2: a collection DHEADER (byte length) frames the count +
            // elements, built member-relative in a sub-writer. XCDR1 classic CDR:
            // no DHEADER — count + elements are written stream-relative into `$w`
            // so each nested struct aligns against the real stream position.
            let put = format!(
                "if $w.xcdr1\n    put_u32!($w, length({expr}))\n    for e in {expr}; marshal_into!(e, $w); end\nelse\n    begin sub = Writer($w.endian); put_u32!(sub, length({expr}));                  for e in {expr}; marshal_into!(e, sub); end;                  bb = bytes(sub); put_u32!($w, length(bb)); put_bytes!($w, bb) end\nend"
            );
            return Ok((format!("Vector{{{esc}}}"), put));
        }
    }
    // sequence<arbitrary> → u32 count + per-element encode (no collection
    // DHEADER; the element type is fully descriptive on the wire for the
    // primitive / enum / bitset / bitmask cases handled here). Mirrors the
    // `idl-go`/`idl-d` fallback. Both the count and the per-element put write
    // to the same `$w`, so no sub-writer/DHEADER is needed.
    let (elem_ty, elem_put) = map_type(elem, "zdElem", enum_names, struct_names)?;
    let put = format!(
        "begin\n    put_u32!($w, length({expr}))\n    for zdElem in {expr}\n        {elem_put}\n    end\nend"
    );
    Ok((format!("Vector{{{elem_ty}}}"), put))
}

// ---- decode (inverse of the put path): a `Reader` wire-core in the prelude,
// plus `map_get` — the inverse of `map_type` — emitting a statement that reads
// one value from `$r` into the lvalue `target`. Julia structs are immutable, so
// each field is read into a local and the struct is constructed positionally.

/// A zero/empty value for a Julia type, used to fill unread union members.
fn zero_value(t: &str, enum_names: &HashSet<String>) -> String {
    // `t` may already be keyword-escaped (trailing `_`, see
    // `escape_julia_ident`) while `enum_names` holds the raw IDL names —
    // strip a defensive escape suffix before the membership check so a
    // keyword-colliding enum type name still resolves.
    let unescaped = t
        .strip_suffix('_')
        .filter(|u| crate::keywords::is_reserved(u));
    if t.starts_with("Vector{") || t.starts_with("Dict{") {
        format!("{t}()")
    } else if t == "String" {
        "\"\"".to_string()
    } else if t == "Char" {
        "Char(0)".to_string()
    } else if enum_names.contains(t) || unescaped.is_some_and(|u| enum_names.contains(u)) {
        format!("{t}(Int32(0))")
    } else {
        format!("zero({t})")
    }
}

/// Nests `elem` in `dims` layers of `Vector{…}`.
fn julia_vec(elem: &str, dims: usize) -> String {
    (0..dims).fold(elem.to_string(), |inner, _| format!("Vector{{{inner}}}"))
}

/// Reads a fixed array: pre-allocated nested `Vector`s + row-major loops filling
/// each element (inverse of [`build_array_put`]). `elem_get` targets the fully
/// indexed lvalue `{target}[zdi0][zdi1]…`.
fn build_array_get(target: &str, sizes: &[i64], elem_type: &str, elem_get: &str) -> String {
    /// zerodds-lint: recursion-depth 32
    fn rec(target: &str, sizes: &[i64], depth: usize, elem_type: &str, elem_get: &str) -> String {
        let idx: String = (0..depth).map(|k| format!("[zdi{k}]")).collect();
        let lval = format!("{target}{idx}");
        let s = sizes[depth];
        let this_elem = julia_vec(elem_type, sizes.len() - depth - 1);
        let prealloc = format!("{lval} = Vector{{{this_elem}}}(undef, {s})");
        if depth + 1 == sizes.len() {
            format!("{prealloc}\nfor zdi{depth} in 1:{s}\n{elem_get}\nend")
        } else {
            let inner = rec(target, sizes, depth + 1, elem_type, elem_get);
            format!("{prealloc}\nfor zdi{depth} in 1:{s}\n{inner}\nend")
        }
    }
    rec(target, sizes, 0, elem_type, elem_get)
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
        // B1 follow-up (#22 decode-side parity): the wire's remaining-byte
        // check `get_wstring!`/`get_string!` already do is NOT the same as
        // the IDL-declared bound N (XTypes 1.3 §7.4.3 requires both sides
        // enforce it) — a bounded field previously decoded an arbitrarily
        // large well-formed payload without complaint. Checked post-decode
        // (value already exists in memory by then), mirroring the "not
        // pre-encode" design idl-rust's emit_decode_bound_checks documents.
        TypeSpec::String(st) if st.wide => {
            let get = format!("{target} = get_wstring!($r)");
            Ok(match &st.bound {
                Some(b) => decode_bound_check_wrap(
                    &get,
                    &format!("sum(c -> (UInt32(c) <= 0xFFFF ? 1 : 2), {target}; init=0)"),
                    &const_expr_to_julia(b),
                    "decoded wstring length exceeds its IDL bound",
                ),
                None => get,
            })
        }
        TypeSpec::String(st) if !st.wide => {
            let get = format!("{target} = get_string!($r)");
            Ok(match &st.bound {
                Some(b) => decode_bound_check_wrap(
                    &get,
                    &format!("sizeof({target})"),
                    &const_expr_to_julia(b),
                    "decoded string length exceeds its IDL bound",
                ),
                None => get,
            })
        }
        TypeSpec::Sequence(seq) => map_get_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            target,
            enum_names,
            struct_names,
        ),
        // `fixed<P,S>`: read the statically-known `(P+2)/2` BCD octets.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let (p, _) = fixed_ps(f)?;
            let n = (p + 2) / 2;
            Ok(format!("{target} = get_bytes_n!($r, {n})"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                let esc = escape_julia_ident(&name);
                // Read the @bit_bound-wide holder and sign-extend to Int32.
                let get = match enum_wire_width(&name) {
                    1 => format!("{target} = {esc}(Int32(reinterpret(Int8, get_u8!($r))))"),
                    2 => format!("{target} = {esc}(Int32(reinterpret(Int16, get_u16!($r))))"),
                    _ => format!("{target} = {esc}(reinterpret(Int32, get_u32!($r)))"),
                };
                Ok(get)
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                let esc = escape_julia_ident(&name);
                Ok(format!("{target} = read_{esc}($r)"))
            } else {
                Err(IdlJuliaError::Unsupported(format!("scoped type {name}")))
            }
        }
        TypeSpec::Map(m) => {
            let (key_type, _) = map_type(&m.key, "zdK", enum_names, struct_names)?;
            let (val_type, _) = map_type(&m.value, "zdV", enum_names, struct_names)?;
            let key_get = map_get(&m.key, "zdK", enum_names, struct_names)?;
            let val_get = map_get(&m.value, "zdV", enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            // XCDR2 frames a non-primitive map with a collection DHEADER; XCDR1
            // classic CDR has none.
            let dh = if prim {
                ""
            } else {
                "if !$r.xcdr1\n        get_u32!($r)\n    end\n    "
            };
            // B1 follow-up (#22 decode-side parity): check the declared
            // count against the IDL bound before allocating/looping.
            let bound_check = match &m.bound {
                Some(b) => format!(
                    "if zdN > {bv}\n        throw(ArgumentError(\"decoded map length exceeds its IDL bound ({bv})\"))\n    end\n    ",
                    bv = const_expr_to_julia(b)
                ),
                None => String::new(),
            };
            Ok(format!(
                "begin\n    {dh}zdN = Int(get_u32!($r))\n    {bound_check}{target} = Dict{{{key_type}, {val_type}}}()\n    for _ in 1:zdN\n        {key_get}\n        {val_get}\n        {target}[zdK] = zdV\n    end\nend"
            ))
        }
        other => Err(IdlJuliaError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType, target: &str) -> Result<String> {
    let s = match p {
        PrimitiveType::Octet => format!("{target} = get_u8!($r)"),
        PrimitiveType::Char => format!("{target} = Char(get_u8!($r))"),
        PrimitiveType::Boolean => format!("{target} = get_bool!($r)"),
        PrimitiveType::Integer(i) => return map_get_integer(i, target),
        PrimitiveType::Floating(FloatingType::Float) => format!("{target} = get_f32!($r)"),
        PrimitiveType::Floating(FloatingType::Double) => format!("{target} = get_f64!($r)"),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            format!("{target} = get_long_double!($r)")
        }
        PrimitiveType::WideChar => format!("{target} = get_u32!($r)"),
    };
    Ok(s)
}

fn map_get_integer(i: IntegerType, target: &str) -> Result<String> {
    let s = match i {
        IntegerType::UInt8 => format!("{target} = get_u8!($r)"),
        IntegerType::Int8 => format!("{target} = reinterpret(Int8, get_u8!($r))"),
        IntegerType::UShort | IntegerType::UInt16 => format!("{target} = get_u16!($r)"),
        IntegerType::Short | IntegerType::Int16 => {
            format!("{target} = reinterpret(Int16, get_u16!($r))")
        }
        IntegerType::ULong | IntegerType::UInt32 => format!("{target} = get_u32!($r)"),
        IntegerType::Long | IntegerType::Int32 => {
            format!("{target} = reinterpret(Int32, get_u32!($r))")
        }
        IntegerType::ULongLong | IntegerType::UInt64 => format!("{target} = get_u64!($r)"),
        IntegerType::LongLong | IntegerType::Int64 => {
            format!("{target} = reinterpret(Int64, get_u64!($r))")
        }
    };
    Ok(s)
}

/// zerodds-lint: recursion-depth 32
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
        let get = format!("{target} = get_seq_u8!($r)");
        return Ok(match bound {
            Some(b) => decode_bound_check_wrap(
                &get,
                &format!("length({target})"),
                &const_expr_to_julia(b),
                "decoded sequence length exceeds its IDL bound",
            ),
            None => get,
        });
    }
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            // Moderate fix (deep review of #22 decode-bounds-cross-backend):
            // check the IDL bound right after reading the wire count `zdN` —
            // BEFORE allocating `Vector{esc}(undef, zdN)` or looping to
            // decode each element. Previously the bound check ran only after
            // the whole loop had already materialized `zdN` structs from the
            // wire (`decode_bound_check_wrap`'s post-decode design, which is
            // fine for a single primitive read but not for a per-element
            // decode loop): an attacker-supplied huge `zdN` drove an
            // oversized allocation and decode loop before the bound ever
            // rejected it — exactly the resource exhaustion the bound exists
            // to prevent. Mirrors ada/d/elixir/go, which all check before
            // the loop.
            let bound_check = match bound {
                Some(b) => {
                    let bv = const_expr_to_julia(b);
                    format!(
                        "if zdN > {bv}\n        throw(ArgumentError(\"decoded sequence length exceeds its IDL bound ({bv})\"))\n    end\n    "
                    )
                }
                None => String::new(),
            };
            let esc = escape_julia_ident(&name);
            // XCDR2 leads with a collection DHEADER; XCDR1 classic CDR does not.
            return Ok(format!(
                "begin\n    if !$r.xcdr1\n        get_u32!($r)\n    end\n    zdN = Int(get_u32!($r))\n    {bound_check}{target} = Vector{{{esc}}}(undef, zdN)\n    for zdI in 1:zdN\n        {target}[zdI] = read_{esc}($r)\n    end\nend"
            ));
        }
    }
    // sequence<arbitrary> → u32 count + per-element decode (no collection
    // DHEADER; mirrors the encode-side arbitrary fallback in `map_sequence`).
    // The bound is checked right after reading the wire count, before the
    // pre-allocation and decode loop (resource-exhaustion guard).
    let (elem_ty, _) = map_type(elem, "zdE", enum_names, struct_names)?;
    let elem_get = map_get(elem, &format!("{target}[zdI]"), enum_names, struct_names)?;
    let bound_check = match bound {
        Some(b) => {
            let bv = const_expr_to_julia(b);
            format!(
                "if zdN > {bv}\n        throw(ArgumentError(\"decoded sequence length exceeds its IDL bound ({bv})\"))\n    end\n    "
            )
        }
        None => String::new(),
    };
    Ok(format!(
        "begin\n    zdN = Int(get_u32!($r))\n    {bound_check}{target} = Vector{{{elem_ty}}}(undef, zdN)\n    for zdI in 1:zdN\n        {elem_get}\n    end\nend"
    ))
}
