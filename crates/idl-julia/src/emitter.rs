// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → Julia emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! Julia source file: a `Writer` (byte-identical to `endpoints/julia`) plus, per
//! IDL `struct`, a Julia struct with a `marshal_xcdr(v, endian)` function.
//! `@final` and `@appendable` are supported; other extensibilities and
//! constructs raise [`IdlJuliaError::Unsupported`].

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

use crate::error::{IdlJuliaError, Result};
use crate::keywords::escape_julia_ident;

/// Options for the Julia backend.
#[derive(Debug, Clone, Default)]
pub struct JuliaGenOptions {}

/// The shared XCDR2 `Writer`, byte-identical to `endpoints/julia`.
const WIRE_PRELUDE: &str = r#"@enum Endian LE BE

mutable struct Writer
    buf::Vector{UInt8}
    endian::Endian
end
Writer(endian::Endian) = Writer(UInt8[], endian)

function align!(w::Writer, a::Int)
    cap = min(a, 4)
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
put_u64!(w::Writer, v) = emit!(w, 4, le_bytes(UInt64(v), 8))
put_f32!(w::Writer, v) = emit!(w, 4, le_bytes(reinterpret(UInt32, Float32(v)), 4))
put_f64!(w::Writer, v) = emit!(w, 4, le_bytes(reinterpret(UInt64, Float64(v)), 8))
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
    emit!(w, 4, le)
end

bytes(w::Writer) = w.buf

mutable struct Reader
    buf::Vector{UInt8}
    pos::Int
    endian::Endian
end
Reader(buf::Vector{UInt8}, endian::Endian) = Reader(buf, 1, endian)

function ralign!(r::Reader, a::Int)
    cap = min(a, 4)
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
get_u64!(r::Reader)::UInt64 = get_le(r, 4, 8)
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
    ralign!(r, 4)
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
"#;

/// Generates a self-contained Julia module from the IDL AST.
///
/// # Errors
/// Returns [`IdlJuliaError::Unsupported`] for constructs the Julia backend does
/// not yet emit (unions, nested-struct members, maps, `long double`, `@mutable`,
/// …).
pub fn generate_julia_module(spec: &Specification, _opts: &JuliaGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Code generated by zerodds-idlc (Julia backend). DO NOT EDIT."
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
    // Self-contained MD5 (RFC 1321) for the KeyHash MD5 branch; appended only
    // when used (Julia resolves the call at run time, so order is fine).
    if out.contains("zd_md5(") {
        out.push_str(JULIA_MD5);
    }
    Ok(out)
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
fn emit_enum(out: &mut String, e: &EnumDef) {
    let values = enumerator_values(e);
    let ty = escape_julia_ident(&e.name.text);
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
    }
    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    for m in &s.members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        for d in &m.declarators {
            let name = escape_julia_ident(&d.name().text);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
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
            });
        }
    }

    let ty = escape_julia_ident(&s.name.text);
    let _ = writeln!(out, "\nstruct {ty}");
    for f in &fields {
        let _ = writeln!(out, "    {}::{}", f.name, f.julia_type);
    }
    let _ = writeln!(out, "end");

    // marshal_into! writes into an existing writer (nested composites call this
    // so alignment stays stream-relative). @final: inline; @appendable: DHEADER.
    let _ = writeln!(out, "\nfunction marshal_into!(v::{ty}, w::Writer)");
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "    body = Writer(w.endian)");
        for f in &fields {
            let emh = 0x4000_0000_u32 | f.id;
            let _ = writeln!(out, "    put_u32!(body, 0x{emh:08x})");
            let _ = writeln!(out, "    zdMem = Writer(w.endian)");
            let _ = writeln!(out, "    {}", f.put.replace("$w", "zdMem"));
            let _ = writeln!(out, "    zdMB = bytes(zdMem)");
            let _ = writeln!(out, "    put_u32!(body, length(zdMB))");
            let _ = writeln!(out, "    put_bytes!(body, zdMB)");
        }
        let _ = writeln!(out, "    zdBB = bytes(body)");
        let _ = writeln!(out, "    put_u32!(w, length(zdBB))");
        let _ = writeln!(out, "    put_bytes!(w, zdBB)");
    } else {
        let wv = if ext == ExtensibilityKind::Final {
            "w"
        } else {
            let _ = writeln!(out, "    body = Writer(w.endian)");
            "body"
        };
        for f in &fields {
            let _ = writeln!(out, "    {}", f.put.replace("$w", wv));
        }
        if ext != ExtensibilityKind::Final {
            let _ = writeln!(out, "    bb = bytes(body)");
            let _ = writeln!(out, "    put_u32!(w, length(bb))");
            let _ = writeln!(out, "    put_bytes!(w, bb)");
        }
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

    // Decode (inverse of marshal_into!). Julia structs are immutable, so each
    // field is read into a local and the struct is constructed positionally.
    // @final reads inline, @appendable skips the DHEADER, @mutable skips DHEADER
    // then per member EMHEADER + NEXTINT (members in declaration order).
    let _ = writeln!(out, "\nfunction read_{ty}(r::Reader)::{ty}");
    if ext == ExtensibilityKind::Mutable {
        let _ = writeln!(out, "    get_u32!(r)");
        for f in &fields {
            let _ = writeln!(out, "    get_u32!(r)");
            let _ = writeln!(out, "    get_u32!(r)");
            let _ = writeln!(out, "    {}", f.get.replace("$r", "r"));
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "    get_u32!(r)");
        }
        for f in &fields {
            let _ = writeln!(out, "    {}", f.get.replace("$r", "r"));
        }
    }
    let args = fields
        .iter()
        .map(|f| f.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "    {ty}({args})");
    let _ = writeln!(out, "end");
    let _ = writeln!(
        out,
        "\nunmarshal_xcdr_{ty}(buf::Vector{{UInt8}}, endian::Endian)::{ty} = read_{ty}(Reader(buf, endian))"
    );
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
        return Err(IdlJuliaError::Unsupported(format!(
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
    let disc_get = map_get(
        &switch_typespec(&u.switch_type),
        "zdDisc",
        enum_names,
        struct_names,
    )?;
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
                CaseLabel::Value(e) => labels.push(array_size(e).ok_or_else(|| {
                    IdlJuliaError::Unsupported(format!(
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
            zero,
        });
    }

    let ty = escape_julia_ident(&u.name.text);
    let _ = writeln!(out, "\nstruct {ty}");
    let _ = writeln!(out, "    disc::{disc_type}");
    for c in &cases {
        let _ = writeln!(out, "    {}::{}", c.field, c.ty);
    }
    let _ = writeln!(out, "end");
    let _ = writeln!(out, "\nfunction marshal_into!(v::{ty}, w::Writer)");
    let wv = if ext == ExtensibilityKind::Final {
        "w"
    } else {
        let _ = writeln!(out, "    body = Writer(w.endian)");
        "body"
    };
    let _ = writeln!(out, "    {}", disc_put.replace("$w", wv));
    for (i, c) in cases.iter().enumerate() {
        if c.is_default {
            let _ = writeln!(out, "    else");
        } else {
            let kw = if i == 0 { "if" } else { "elseif" };
            let cond = c
                .labels
                .iter()
                .map(|l| format!("v.disc == {l}"))
                .collect::<Vec<_>>()
                .join(" || ");
            let _ = writeln!(out, "    {kw} {cond}");
        }
        let _ = writeln!(out, "        {}", c.put.replace("$w", wv));
    }
    let _ = writeln!(out, "    end");
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "    bb = bytes(body)");
        let _ = writeln!(out, "    put_u32!(w, length(bb))");
        let _ = writeln!(out, "    put_bytes!(w, bb)");
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

    // Decode: read the discriminator, zero-fill the case members, then read only
    // the selected member (@appendable skips the leading DHEADER). The immutable
    // holder is constructed positionally.
    let _ = writeln!(out, "\nfunction read_{ty}(r::Reader)::{ty}");
    if ext == ExtensibilityKind::Appendable {
        let _ = writeln!(out, "    get_u32!(r)");
    }
    let _ = writeln!(out, "    {}", disc_get.replace("$r", "r"));
    for c in &cases {
        let _ = writeln!(out, "    {} = {}", c.field, c.zero);
    }
    for (i, c) in cases.iter().enumerate() {
        if c.is_default {
            let _ = writeln!(out, "    else");
        } else {
            let kw = if i == 0 { "if" } else { "elseif" };
            let cond = c
                .labels
                .iter()
                .map(|l| format!("zdDisc == {l}"))
                .collect::<Vec<_>>()
                .join(" || ");
            let _ = writeln!(out, "    {kw} {cond}");
        }
        let _ = writeln!(out, "        {}", c.get.replace("$r", "r"));
    }
    if !cases.is_empty() {
        let _ = writeln!(out, "    end");
    }
    let args = cases
        .iter()
        .map(|c| c.field.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let sep = if cases.is_empty() { "" } else { ", " };
    let _ = writeln!(out, "    {ty}(zdDisc{sep}{args})");
    let _ = writeln!(out, "end");
    let _ = writeln!(
        out,
        "\nunmarshal_xcdr_{ty}(buf::Vector{{UInt8}}, endian::Endian)::{ty} = read_{ty}(Reader(buf, endian))"
    );
    Ok(())
}

/// Maps an IDL type to `(Julia type, put statement)`. The put uses `$w` as the
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
        format!(
            "begin\n    zdSub = Writer($w.endian)\n    put_u32!(zdSub, length({expr}))\n    for zdK in sort(collect(keys({expr})))\n        {kp}\n        {vp}\n    end\n    zdBB = bytes(zdSub)\n    put_u32!($w, length(zdBB))\n    put_bytes!($w, zdBB)\nend"
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
            let (ty, put) = map_sequence(&seq.elem, expr, struct_names)?;
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
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                Ok((
                    escape_julia_ident(&name),
                    format!("put_u32!($w, reinterpret(UInt32, Int32(Integer({expr}))))"),
                ))
            } else if struct_names.contains(&name) {
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

fn map_sequence(
    elem: &TypeSpec,
    expr: &str,
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
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let esc = escape_julia_ident(&name);
            let put = format!(
                "begin sub = Writer($w.endian); put_u32!(sub, length({expr}));                  for e in {expr}; marshal_into!(e, sub); end;                  bb = bytes(sub); put_u32!($w, length(bb)); put_bytes!($w, bb) end"
            );
            return Ok((format!("Vector{{{esc}}}"), put));
        }
    }
    Err(IdlJuliaError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
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
        TypeSpec::Sequence(seq) => {
            map_get_sequence(&seq.elem, seq.bound.as_ref(), target, struct_names)
        }
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                let esc = escape_julia_ident(&name);
                Ok(format!(
                    "{target} = {esc}(reinterpret(Int32, get_u32!($r)))"
                ))
            } else if struct_names.contains(&name) {
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
            let dh = if prim { "" } else { "get_u32!($r)\n    " };
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

fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    target: &str,
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
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
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
            return Ok(format!(
                "begin\n    get_u32!($r)\n    zdN = Int(get_u32!($r))\n    {bound_check}{target} = Vector{{{esc}}}(undef, zdN)\n    for zdI in 1:zdN\n        {target}[zdI] = read_{esc}($r)\n    end\nend"
            ));
        }
    }
    Err(IdlJuliaError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}
