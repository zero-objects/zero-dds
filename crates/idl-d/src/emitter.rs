// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → D emitter. Walks the `zerodds-idl` AST and emits a self-contained D
//! source file: a shared XCDR2 `Writer` (byte-identical to `endpoints/d`) plus,
//! per IDL `struct`, a D struct with a `marshalXCDR(endian)` method. `@final`
//! and `@appendable` are supported; other extensibilities and constructs raise
//! [`IdlDError::Unsupported`].

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

use crate::error::{IdlDError, Result};
use crate::keywords::escape_d_ident;

/// Options for the D backend.
#[derive(Debug, Clone, Default)]
pub struct DGenOptions {}

/// The shared XCDR2 wire `Writer`, byte-identical to `endpoints/d`.
const WIRE_PRELUDE: &str = r#"enum Endian { LE, BE }

struct Writer {
    ubyte[] buf;
    Endian endian;

    this(Endian e) { endian = e; }

    void alignTo(int a) {
        int cap = a < 4 ? a : 4;
        int pad = (cap - (cast(int) buf.length % cap)) % cap;
        foreach (_; 0 .. pad) buf ~= 0;
    }
    void put(int a, ubyte[] le) {
        alignTo(a);
        if (endian == Endian.BE)
            foreach_reverse (b; le) buf ~= b;
        else
            foreach (b; le) buf ~= b;
    }
    void putU8(int v) { buf ~= cast(ubyte)(v & 0xff); }
    void putBool(bool v) { putU8(v ? 1 : 0); }
    void putU16(int v) { put(2, [cast(ubyte) v, cast(ubyte)(v >> 8)]); }
    void putU32(uint v) {
        put(4, [cast(ubyte) v, cast(ubyte)(v >> 8), cast(ubyte)(v >> 16), cast(ubyte)(v >> 24)]);
    }
    void putU64(ulong v) {
        ubyte[] b;
        foreach (i; 0 .. 8) b ~= cast(ubyte)(v >> (8 * i));
        put(4, b);
    }
    void putF32(float v) {
        uint bits = *cast(uint*)&v;
        put(4, [cast(ubyte) bits, cast(ubyte)(bits >> 8), cast(ubyte)(bits >> 16), cast(ubyte)(bits >> 24)]);
    }
    void putF64(double v) {
        ulong bits = *cast(ulong*)&v;
        putU64(bits);
    }
    void putBytes(ubyte[] data) { foreach (x; data) buf ~= x; }
    void putString(string s) {
        putU32(cast(uint)(s.length + 1));
        foreach (c; s) buf ~= cast(ubyte) c;
        putU8(0);
    }
    void putSeqU8(ubyte[] b) {
        putU32(cast(uint) b.length);
        foreach (x; b) buf ~= x;
    }
    void putWString(string s) {
        ushort[] units;
        foreach (dchar r; s) {
            if (r <= 0xFFFF) units ~= cast(ushort) r;
            else { uint rr = r - 0x10000; units ~= cast(ushort)(0xD800 + (rr >> 10)); units ~= cast(ushort)(0xDC00 + (rr & 0x3FF)); }
        }
        putU32(cast(uint)(units.length * 2));
        foreach (u; units) putU16(u);
    }
    void putLongDouble(double v) {
        ulong bits = *cast(ulong*)&v;
        ulong sign = bits >> 63;
        ulong exp = (bits >> 52) & 0x7FF;
        ulong mant = bits & 0xFFFFFFFFFFFFF;
        ulong hi = sign << 63;
        ulong lo = 0;
        if (!(exp == 0 && mant == 0)) { hi = (sign << 63) | ((exp - 1023 + 16383) << 48) | (mant >> 4); lo = (mant & 0xF) << 60; }
        ubyte[] le = new ubyte[16];
        foreach (i; 0 .. 8) { le[i] = cast(ubyte)(lo >> (8*i)); le[8+i] = cast(ubyte)(hi >> (8*i)); }
        put(4, le);
    }
    ubyte[] bytes() { return buf; }
}

struct Reader {
    ubyte[] buf;
    size_t pos;
    Endian endian;

    this(ubyte[] b, Endian e) { buf = b; endian = e; }

    void ralign(int a) { int cap = a < 4 ? a : 4; while (pos % cap != 0) pos++; }
    ulong getLE(int a, int n) {
        ralign(a);
        ulong v = 0;
        if (endian == Endian.BE) { foreach (i; 0 .. n) v = (v << 8) | buf[pos + i]; }
        else { foreach_reverse (i; 0 .. n) v = (v << 8) | buf[pos + i]; }
        pos += n;
        return v;
    }
    ubyte getU8() { return buf[pos++]; }
    bool getBool() { return getU8() != 0; }
    ushort getU16() { return cast(ushort) getLE(2, 2); }
    uint getU32() { return cast(uint) getLE(4, 4); }
    ulong getU64() { return getLE(4, 8); }
    float getF32() { uint b = getU32(); return *cast(float*)&b; }
    double getF64() { ulong b = getU64(); return *cast(double*)&b; }
    ubyte[] getBytesN(size_t n) { auto r = buf[pos .. pos + n]; pos += n; return r; }
    string getString() { size_t n = getU32(); auto b = getBytesN(n); return n > 0 ? cast(string) b[0 .. n - 1] : ""; }
    ubyte[] getSeqU8() { size_t n = getU32(); return getBytesN(n).dup; }
    dstring getWStringD() {
        size_t n = getU32() / 2;
        ushort[] units;
        foreach (i; 0 .. n) units ~= getU16();
        dchar[] outc;
        for (size_t i = 0; i < n; i++) {
            ushort u = units[i];
            if (u >= 0xD800 && u <= 0xDBFF && i + 1 < n) {
                outc ~= cast(dchar)(0x10000 + ((u - 0xD800) << 10) + (units[i + 1] - 0xDC00));
                i++;
            } else outc ~= cast(dchar) u;
        }
        return cast(dstring) outc;
    }
    import std.conv : to;
    string getWString() { return to!string(getWStringD()); }
    double getLongDouble() {
        ralign(4);
        ubyte[] le = getBytesN(16).dup;
        if (endian == Endian.BE) { foreach (i; 0 .. 8) { auto t = le[i]; le[i] = le[15 - i]; le[15 - i] = t; } }
        ulong lo = 0, hi = 0;
        foreach (i; 0 .. 8) { lo |= cast(ulong) le[i] << (8 * i); hi |= cast(ulong) le[8 + i] << (8 * i); }
        ulong sign = hi >> 63;
        ulong exp = (hi >> 48) & 0x7FFF;
        ulong mant = ((hi & 0xFFFFFFFFFFFF) << 4) | (lo >> 60);
        ulong bits = (exp == 0 && mant == 0) ? (sign << 63) : ((sign << 63) | ((exp - 16383 + 1023) << 52) | mant);
        return *cast(double*)&bits;
    }
}
"#;

/// Generates a self-contained D module from the IDL AST.
///
/// # Errors
/// Returns [`IdlDError::Unsupported`] for constructs the D backend does not yet
/// emit (unions, nested-struct members, maps, `long double`, `@mutable`, …).
pub fn generate_d_module(spec: &Specification, _opts: &DGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Code generated by zerodds-idlc (D backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "// SPDX-License-Identifier: Apache-2.0\n");
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

    let typedefs = collect_typedefs(spec);
    // struct name → def, so a nested-struct `@key` member's own `@key`
    // subset can be resolved for keyHash emission (Bug A) and for the
    // static MD5-vs-zero-pad branch decision (Bug B) — mirrors
    // `collect_typedefs`.
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
    Ok(out)
}

fn extensibility(s: &StructDef) -> ExtensibilityKind {
    lower_annotations(&s.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable)
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

/// Emits an IDL `enum` as a D `int`-based `enum` (its members are enum-scoped).
fn emit_enum(out: &mut String, e: &EnumDef) {
    let values = enumerator_values(e);
    let ty = escape_d_ident(&e.name.text);
    let _ = writeln!(out, "\nenum {ty} : int {{");
    for (en, value) in e.enumerators.iter().zip(&values) {
        let name = escape_d_ident(&en.name.text);
        let _ = writeln!(out, "    {name} = {value},");
    }
    let _ = writeln!(out, "}}");
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

/// Collects top-level `struct` definitions as name → def, so a nested-struct
/// `@key` member can be expanded into its own `@key` subset (XTypes 1.3
/// §7.6.8) for keyHash emission and for the static max-size (MD5 vs.
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
/// fixed array `<field>[zdi0][zdi1]…` (D marshalInto accesses fields bare).
fn build_array_put(field: &str, sizes: &[i64], elem_put: &str) -> String {
    let idx: String = (0..sizes.len()).map(|k| format!("[zdi{k}]")).collect();
    let mut body = elem_put.replace("$elem", &format!("{field}{idx}"));
    for k in (0..sizes.len()).rev() {
        body = format!(
            "for (size_t zdi{k} = 0; zdi{k} < {}; zdi{k}++) {{ {body} }}",
            sizes[k]
        );
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
    structs: &HashMap<String, &StructDef>,
) -> Result<()> {
    let ext = extensibility(s);

    struct FieldGen {
        d_name: String,
        d_type: String,
        put: String,
        get: String,
        id: u32,
        key: bool,
        resolved: TypeSpec, // typedef-dealiased type of this field
        simple: bool,       // true for a `Declarator::Simple` (not a fixed array)
    }
    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    for m in &s.members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        for d in &m.declarators {
            let d_name = escape_d_ident(&d.name().text);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let simple = matches!(d, Declarator::Simple(_));
            let (d_type, put, get) = match d {
                Declarator::Simple(_) => {
                    let (t, put) = map_type(&resolved, &d_name, enum_names, struct_names)?;
                    let get = map_get(&resolved, &format!("v.{d_name}"), enum_names, struct_names)?;
                    (t, put, get)
                }
                // Fixed array: elements inline, row-major, no length prefix.
                Declarator::Array(ad) => {
                    let sizes = ad
                        .sizes
                        .iter()
                        .map(array_size)
                        .collect::<Option<Vec<i64>>>()
                        .ok_or_else(|| {
                            IdlDError::Unsupported(format!("non-literal array size on `{d_name}`"))
                        })?;
                    let (elem_type, elem_put) =
                        map_type(&resolved, "$elem", enum_names, struct_names)?;
                    let d_type =
                        elem_type + &sizes.iter().map(|n| format!("[{n}]")).collect::<String>();
                    let get = build_array_get(
                        &format!("v.{d_name}"),
                        &sizes,
                        &resolved,
                        enum_names,
                        struct_names,
                    )?;
                    (d_type, build_array_put(&d_name, &sizes, &elem_put), get)
                }
            };
            fields.push(FieldGen {
                d_name,
                d_type,
                put,
                get,
                id,
                key,
                resolved: resolved.clone(),
                simple,
            });
        }
    }

    let ty = escape_d_ident(&s.name.text);
    let _ = writeln!(out, "\nstruct {ty} {{");
    for f in &fields {
        let _ = writeln!(out, "    {} {};", f.d_type, f.d_name);
    }

    // marshalInto writes into an existing writer (nested composites call this so
    // alignment stays stream-relative). @final: fields inline; @appendable: a
    // DHEADER-framed body.
    let _ = writeln!(out, "\n    void marshalInto(ref Writer w) {{");
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "        auto zdBody = Writer(w.endian);");
        for f in &fields {
            let emh = 0x4000_0000_u32 | f.id;
            let _ = writeln!(out, "        zdBody.putU32(0x{emh:08x}u);");
            let _ = writeln!(out, "        {{");
            let _ = writeln!(out, "            auto zdMem = Writer(w.endian);");
            let _ = writeln!(out, "            {}", f.put.replace("$w", "zdMem"));
            let _ = writeln!(
                out,
                "            zdBody.putU32(cast(uint) zdMem.bytes().length);"
            );
            let _ = writeln!(out, "            zdBody.putBytes(zdMem.bytes());");
            let _ = writeln!(out, "        }}");
        }
        let _ = writeln!(out, "        w.putU32(cast(uint) zdBody.bytes().length);");
        let _ = writeln!(out, "        w.putBytes(zdBody.bytes());");
    } else {
        let wv = if ext == ExtensibilityKind::Final {
            "w"
        } else {
            let _ = writeln!(out, "        auto zdBody = Writer(w.endian);");
            "zdBody"
        };
        for f in &fields {
            let _ = writeln!(out, "        {}", f.put.replace("$w", wv));
        }
        if ext != ExtensibilityKind::Final {
            let _ = writeln!(out, "        w.putU32(cast(uint) zdBody.bytes().length);");
            let _ = writeln!(out, "        w.putBytes(zdBody.bytes());");
        }
    }
    let _ = writeln!(out, "    }}");

    let _ = writeln!(out, "\n    ubyte[] marshalXCDR(Endian endian) {{");
    let _ = writeln!(out, "        auto w = Writer(endian);");
    let _ = writeln!(out, "        this.marshalInto(w);");
    let _ = writeln!(out, "        return w.bytes();");
    let _ = writeln!(out, "    }}");
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
        let _ = writeln!(out, "\n    ubyte[16] keyHash() {{");
        let _ = writeln!(out, "        auto kw = Writer(Endian.BE);");
        for f in &zdkeys {
            // Bug A: a `@key` member whose (typedef-dealiased) type is
            // itself a struct must expand to ONLY that struct's own `@key`
            // members (or ALL its members if it declares none), in
            // member-id order — not the struct's full member set. `f.put`
            // reuses the generic per-field mapper, which is correct for
            // normal (non-key) struct encoding but always encodes the FULL
            // member set, so it must not be used here for a nested-struct
            // key.
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
                    &f.d_name,
                    enum_names,
                    struct_names,
                    typedefs,
                    structs,
                )?;
            } else {
                let _ = writeln!(out, "        {}", f.put.replace("$w", "kw"));
            }
        }
        let _ = writeln!(out, "        auto b = kw.bytes();");
        if use_md5 {
            // KeyHolder max size > 16 → MD5(bytes)[0..16] (XTypes §7.6.8.4).
            let _ = writeln!(out, "        import std.digest.md : md5Of;");
            let _ = writeln!(out, "        return md5Of(b);");
        } else {
            let _ = writeln!(out, "        ubyte[16] outk;");
            let _ = writeln!(out, "        foreach (i, x; b) if (i < 16) outk[i] = x;");
            let _ = writeln!(out, "        return outk;");
        }
        let _ = writeln!(out, "    }}");
    }
    let _ = writeln!(out, "}}");

    // unmarshalFrom / UnmarshalXCDR: the decode side (inverse of marshalInto).
    let _ = writeln!(out, "\n{ty} unmarshalFrom{ty}(ref Reader r) {{");
    let _ = writeln!(out, "    {ty} v;");
    match ext {
        ExtensibilityKind::Final => {
            for f in &fields {
                let _ = writeln!(out, "    {}", f.get);
            }
        }
        ExtensibilityKind::Appendable => {
            let _ = writeln!(out, "    r.getU32(); // DHEADER");
            for f in &fields {
                let _ = writeln!(out, "    {}", f.get);
            }
        }
        ExtensibilityKind::Mutable => {
            let _ = writeln!(out, "    r.getU32(); // DHEADER");
            for f in &fields {
                let _ = writeln!(out, "    r.getU32(); // EMHEADER");
                let _ = writeln!(out, "    r.getU32(); // NEXTINT");
                let _ = writeln!(out, "    {}", f.get);
            }
        }
    }
    let _ = writeln!(out, "    return v;");
    let _ = writeln!(out, "}}");
    let _ = writeln!(
        out,
        "\n{ty} UnmarshalXCDR{ty}(ubyte[] buf, Endian endian) {{"
    );
    let _ = writeln!(out, "    auto r = Reader(buf, endian);");
    let _ = writeln!(out, "    return unmarshalFrom{ty}(r);");
    let _ = writeln!(out, "}}");
    Ok(())
}

/// Emits keyHash key-writer puts for a nested-struct `@key` member: expands
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
                return Err(IdlDError::Unsupported(
                    "array @key field inside a nested-struct key".to_string(),
                ));
            }
            let field = d.name().text.clone();
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
            let _ = writeln!(out, "        {}", put.replace("$w", "kw"));
        }
    }
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
        return Err(IdlDError::Unsupported(format!(
            "@mutable union {} (EMHEADER framing not yet emitted)",
            u.name.text
        )));
    }
    let disc_ts = switch_typespec(&u.switch_type);
    let (disc_type, disc_put) = map_type(&disc_ts, "disc", enum_names, struct_names)?;
    let disc_get = map_get(&disc_ts, "v.disc", enum_names, struct_names)?;
    let mut cases: Vec<UnionCase> = Vec::new();
    for c in &u.cases {
        let field = escape_d_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ty, put) = map_type(&resolved, &field, enum_names, struct_names)?;
        let get = map_get(&resolved, &format!("v.{field}"), enum_names, struct_names)?;
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => labels.push(array_size(e).ok_or_else(|| {
                    IdlDError::Unsupported(format!("non-integer union label in `{}`", u.name.text))
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

    let ty = escape_d_ident(&u.name.text);
    let _ = writeln!(out, "\nstruct {ty} {{");
    let _ = writeln!(out, "    {disc_type} disc;");
    for c in &cases {
        let _ = writeln!(out, "    {} {};", c.ty, c.field);
    }
    let _ = writeln!(out, "\n    void marshalInto(ref Writer w) {{");
    let wv = if ext == ExtensibilityKind::Final {
        "w"
    } else {
        let _ = writeln!(out, "        auto zdBody = Writer(w.endian);");
        "zdBody"
    };
    let _ = writeln!(out, "        {}", disc_put.replace("$w", wv));
    for (i, c) in cases.iter().enumerate() {
        if c.is_default {
            let _ = writeln!(out, "        else {{");
        } else {
            let kw = if i == 0 { "if" } else { "else if" };
            let cond = c
                .labels
                .iter()
                .map(|l| format!("disc == {l}"))
                .collect::<Vec<_>>()
                .join(" || ");
            let _ = writeln!(out, "        {kw} ({cond}) {{");
        }
        let _ = writeln!(out, "            {}", c.put.replace("$w", wv));
        let _ = writeln!(out, "        }}");
    }
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "        w.putU32(cast(uint) zdBody.bytes().length);");
        let _ = writeln!(out, "        w.putBytes(zdBody.bytes());");
    }
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "\n    ubyte[] marshalXCDR(Endian endian) {{");
    let _ = writeln!(out, "        auto w = Writer(endian);");
    let _ = writeln!(out, "        this.marshalInto(w);");
    let _ = writeln!(out, "        return w.bytes();");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "}}");

    let _ = writeln!(
        out,
        "
{ty} unmarshalFrom{ty}(ref Reader r) {{"
    );
    let _ = writeln!(out, "    {ty} v;");
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "    r.getU32(); // DHEADER");
    }
    let _ = writeln!(out, "    {disc_get}");
    for (i, c) in cases.iter().enumerate() {
        if c.is_default {
            let _ = writeln!(out, "    else {{ {} }}", c.get);
        } else {
            let kw = if i == 0 { "if" } else { "else if" };
            let cond = c
                .labels
                .iter()
                .map(|l| format!("v.disc == {l}"))
                .collect::<Vec<_>>()
                .join(" || ");
            let _ = writeln!(out, "    {kw} ({cond}) {{ {} }}", c.get);
        }
    }
    let _ = writeln!(out, "    return v;");
    let _ = writeln!(out, "}}");
    let _ = writeln!(
        out,
        "
{ty} UnmarshalXCDR{ty}(ubyte[] buf, Endian endian) {{"
    );
    let _ = writeln!(out, "    auto r = Reader(buf, endian);");
    let _ = writeln!(out, "    return unmarshalFrom{ty}(r);");
    let _ = writeln!(out, "}}");
    Ok(())
}

/// Maps an IDL type to `(D type, put statement)`. The put uses `$w` as the
/// writer placeholder and `expr` as the field name (a struct member).
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
/// (DHEADER-framed unless the key/value pair is primitive). A local `import`
/// pulls in `sort` only where a map is emitted.
fn build_map_put(
    expr: &str,
    key_put: &str,
    val_put: &str,
    prim: bool,
    bound: Option<&ConstExpr>,
) -> String {
    let head = format!(
        "import std.algorithm.sorting : sort;\n        auto zdKeys = {expr}.keys;\n        zdKeys.sort();"
    );
    // Bounded `map<K,V,N>` (DDS-XTypes §7.4.3): over-bound = encode error.
    let bound_check = match bound.and_then(bound_literal) {
        Some(bv) => format!(
            "\n        if (zdKeys.length > {bv}) throw new Exception(\"bounded map length exceeds its IDL bound ({bv})\");"
        ),
        None => String::new(),
    };
    if prim {
        format!(
            "{{\n        {head}{bound_check}\n        $w.putU32(cast(uint) zdKeys.length);\n        foreach (zdK; zdKeys) {{ {key_put} {val_put} }}\n        }}"
        )
    } else {
        let kp = key_put.replace("$w", "zdSub");
        let vp = val_put.replace("$w", "zdSub");
        format!(
            "{{\n        {head}{bound_check}\n        auto zdSub = Writer($w.endian);\n        zdSub.putU32(cast(uint) zdKeys.length);\n        foreach (zdK; zdKeys) {{ {kp} {vp} }}\n        auto zdBB = zdSub.bytes();\n        $w.putU32(cast(uint) zdBB.length);\n        $w.putBytes(zdBB);\n        }}"
        )
    }
}

/// zerodds-lint: recursion-depth 32
/// Renders a `ConstExpr` bound as D source text when it resolves to an
/// integer literal (mirrors [`array_size`]'s resolution scope — a named
/// constant bound is not folded here, matching the existing array-bound
/// convention in this crate). `None` means "bound not resolvable at codegen
/// time"; callers skip emitting the check in that case rather than fail
/// codegen, the same fallback `array_size` callers already use.
fn bound_literal(e: &ConstExpr) -> Option<String> {
    array_size(e).map(|n| n.to_string())
}

fn map_type(
    t: &TypeSpec,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    match t {
        TypeSpec::Primitive(p) => map_primitive(*p, expr),
        TypeSpec::String(st) if !st.wide => {
            // Bounded `string<N>` (DDS-XTypes §7.4.3): D `string` is UTF-8, so
            // `.length` is the byte count — matches the CDR narrow-string wire
            // length exactly. Reject over-bound before writing (strict vendors
            // reject on the wire too).
            let put = match st.bound.as_ref().and_then(bound_literal) {
                Some(bv) => format!(
                    "if ({expr}.length > {bv}) throw new Exception(\"bounded string length exceeds its IDL bound ({bv})\"); $w.putString({expr});"
                ),
                None => format!("$w.putString({expr});"),
            };
            Ok(("string".to_string(), put))
        }
        // wstring: u32 octet-length (2·units, no BOM) + UTF-16 code units.
        // Bounded `wstring<N>`: the bound counts UTF-16 code units (XTypes
        // §7.4.3), not the UTF-8 D `string`'s `.length` — use
        // `std.utf.codeLength!wchar` to count without allocating a UTF-16 copy.
        TypeSpec::String(st) => {
            let put = match st.bound.as_ref().and_then(bound_literal) {
                Some(bv) => format!(
                    "{{ import std.utf : codeLength; if (codeLength!wchar({expr}) > {bv}) throw new Exception(\"bounded wstring length exceeds its IDL bound ({bv})\"); }} $w.putWString({expr});"
                ),
                None => format!("$w.putWString({expr});"),
            };
            Ok(("string".to_string(), put))
        }
        TypeSpec::Sequence(seq) => {
            map_sequence(&seq.elem, seq.bound.as_ref(), expr, enum_names, struct_names)
        }
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                Ok((
                    escape_d_ident(&name),
                    format!("$w.putU32(cast(uint) {expr});"),
                ))
            } else if struct_names.contains(&name) {
                Ok((escape_d_ident(&name), format!("{expr}.marshalInto($w);")))
            } else {
                Err(IdlDError::Unsupported(format!("scoped type {name}")))
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
                format!("{val_type}[{key_type}]"),
                build_map_put(expr, &key_put, &val_put, prim, m.bound.as_ref()),
            ))
        }
        other => Err(IdlDError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_primitive(p: PrimitiveType, expr: &str) -> Result<(String, String)> {
    let (ty, put) = match p {
        PrimitiveType::Octet => ("ubyte", format!("$w.putU8({expr});")),
        PrimitiveType::Boolean => ("bool", format!("$w.putBool({expr});")),
        PrimitiveType::Char => ("char", format!("$w.putU8({expr});")),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => ("float", format!("$w.putF32({expr});")),
        PrimitiveType::Floating(FloatingType::Double) => ("double", format!("$w.putF64({expr});")),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            ("double", format!("$w.putLongDouble({expr});"))
        }
        PrimitiveType::WideChar => ("uint", format!("$w.putU32({expr});")),
    };
    Ok((ty.to_string(), put))
}

fn map_integer(i: IntegerType, expr: &str) -> Result<(String, String)> {
    let (ty, put) = match i {
        IntegerType::UInt8 => ("ubyte", format!("$w.putU8({expr});")),
        IntegerType::Int8 => ("byte", format!("$w.putU8({expr});")),
        IntegerType::UShort | IntegerType::UInt16 => ("ushort", format!("$w.putU16({expr});")),
        IntegerType::Short | IntegerType::Int16 => ("short", format!("$w.putU16({expr});")),
        IntegerType::ULong | IntegerType::UInt32 => ("uint", format!("$w.putU32({expr});")),
        IntegerType::Long | IntegerType::Int32 => ("int", format!("$w.putU32(cast(uint) {expr});")),
        IntegerType::ULongLong | IntegerType::UInt64 => ("ulong", format!("$w.putU64({expr});")),
        IntegerType::LongLong | IntegerType::Int64 => {
            ("long", format!("$w.putU64(cast(ulong) {expr});"))
        }
    };
    Ok((ty.to_string(), put))
}

fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    _enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = encode error.
    let bound_check = match bound.and_then(bound_literal) {
        Some(bv) => format!(
            "if ({expr}.length > {bv}) throw new Exception(\"bounded sequence length exceeds its IDL bound ({bv})\"); "
        ),
        None => String::new(),
    };
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok((
            "ubyte[]".to_string(),
            format!("{bound_check}$w.putSeqU8({expr});"),
        ));
    }
    // sequence<struct> → collection DHEADER + count + each element.
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let put = format!(
                "{{ {bound_check}auto zdSub = Writer($w.endian); zdSub.putU32(cast(uint) {expr}.length); foreach (zdElem; {expr}) zdElem.marshalInto(zdSub); $w.putU32(cast(uint) zdSub.bytes().length); $w.putBytes(zdSub.bytes()); }}"
            );
            return Ok((format!("{}[]", escape_d_ident(&name)), put));
        }
    }
    Err(IdlDError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}

/// The inverse of [`map_type`]: a D statement reading from `r` into `target`.
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
        // check (`map_type`) on decode too — XTypes 1.3 §7.4.3 requires the
        // IDL bound enforced on BOTH sides, not just the wire's own
        // remaining-buffer validation.
        TypeSpec::String(st) if !st.wide => {
            let check = match st.bound.as_ref().and_then(bound_literal) {
                Some(bv) => format!(
                    " if ({target}.length > {bv}) throw new Exception(\"decoded string length exceeds its IDL bound ({bv})\");"
                ),
                None => String::new(),
            };
            Ok(format!("{target} = r.getString();{check}"))
        }
        TypeSpec::String(st) => {
            let check = match st.bound.as_ref().and_then(bound_literal) {
                Some(bv) => format!(
                    " {{ import std.utf : codeLength; if (codeLength!wchar({target}) > {bv}) throw new Exception(\"decoded wstring length exceeds its IDL bound ({bv})\"); }}"
                ),
                None => String::new(),
            };
            Ok(format!("{target} = r.getWString();{check}"))
        }
        TypeSpec::Sequence(seq) => {
            map_get_sequence(&seq.elem, seq.bound.as_ref(), target, enum_names, struct_names)
        }
        TypeSpec::Map(m) => map_get_map(m, target, enum_names, struct_names),
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            let esc = escape_d_ident(&name);
            if enum_names.contains(&name) {
                Ok(format!("{target} = cast({esc}) r.getU32();"))
            } else if struct_names.contains(&name) {
                Ok(format!("{target} = unmarshalFrom{esc}(r);"))
            } else {
                Err(IdlDError::Unsupported(format!("scoped type {name}")))
            }
        }
        other => Err(IdlDError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType, target: &str) -> String {
    match p {
        PrimitiveType::Octet => format!("{target} = r.getU8();"),
        PrimitiveType::Boolean => format!("{target} = r.getBool();"),
        PrimitiveType::Char => format!("{target} = cast(char) r.getU8();"),
        PrimitiveType::Integer(i) => map_get_integer(i, target),
        PrimitiveType::Floating(FloatingType::Float) => format!("{target} = r.getF32();"),
        PrimitiveType::Floating(FloatingType::Double) => format!("{target} = r.getF64();"),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            format!("{target} = r.getLongDouble();")
        }
        PrimitiveType::WideChar => format!("{target} = r.getU32();"),
    }
}

fn map_get_integer(i: IntegerType, target: &str) -> String {
    match i {
        IntegerType::UInt8 => format!("{target} = r.getU8();"),
        IntegerType::Int8 => format!("{target} = cast(byte) r.getU8();"),
        IntegerType::UShort | IntegerType::UInt16 => format!("{target} = r.getU16();"),
        IntegerType::Short | IntegerType::Int16 => format!("{target} = cast(short) r.getU16();"),
        IntegerType::ULong | IntegerType::UInt32 => format!("{target} = r.getU32();"),
        IntegerType::Long | IntegerType::Int32 => format!("{target} = cast(int) r.getU32();"),
        IntegerType::ULongLong | IntegerType::UInt64 => format!("{target} = r.getU64();"),
        IntegerType::LongLong | IntegerType::Int64 => format!("{target} = cast(long) r.getU64();"),
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
    // Bounded `sequence<T, N>`: mirror the encode-side check. §7.4.3.
    let bv = bound.and_then(bound_literal);
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok(match &bv {
            Some(bv) => format!(
                "{target} = r.getSeqU8(); if ({target}.length > {bv}) throw new Exception(\"decoded sequence length exceeds its IDL bound ({bv})\");"
            ),
            None => format!("{target} = r.getSeqU8();"),
        });
    }
    let (elem_ty, _) = map_type(elem, "e", enum_names, struct_names)?;
    let elem_get = map_get(elem, &format!("{target}[i]"), enum_names, struct_names)?;
    let dheader = if matches!(elem, TypeSpec::Scoped(sn)
        if struct_names.contains(&sn.parts.last().map(|p| p.text.clone()).unwrap_or_default()))
    {
        "r.getU32(); "
    } else {
        ""
    };
    let bound_check = match &bv {
        Some(bv) => format!(
            "if (zdn > {bv}) throw new Exception(\"decoded sequence length exceeds its IDL bound ({bv})\"); "
        ),
        None => String::new(),
    };
    Ok(format!(
        "{{ {dheader}size_t zdn = r.getU32(); {bound_check}{target} = new {elem_ty}[zdn]; \
         foreach (i; 0 .. zdn) {{ {elem_get} }} }}"
    ))
}

/// zerodds-lint: recursion-depth 32
fn map_get_map(
    m: &zerodds_idl::ast::types::MapType,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    let (key_ty, _) = map_type(&m.key, "zk", enum_names, struct_names)?;
    let (val_ty, _) = map_type(&m.value, "zv", enum_names, struct_names)?;
    let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
    let key_get = map_get(&m.key, "zk", enum_names, struct_names)?;
    let val_get = map_get(&m.value, "zv", enum_names, struct_names)?;
    let dheader = if prim { "" } else { "r.getU32(); " };
    // Bounded `map<K,V,N>`: mirror the encode-side check. §7.4.3.
    let bound_check = match m.bound.as_ref().and_then(bound_literal) {
        Some(bv) => format!(
            "if (zdn > {bv}) throw new Exception(\"decoded map length exceeds its IDL bound ({bv})\"); "
        ),
        None => String::new(),
    };
    Ok(format!(
        "{{ {dheader}size_t zdn = r.getU32(); {bound_check}foreach (i; 0 .. zdn) {{ {key_ty} zk; {val_ty} zv; \
         {key_get} {val_get} {target}[zk] = zv; }} }}"
    ))
}

/// The inverse of `build_array_put`: nested loops reading each element.
fn build_array_get(
    field: &str,
    sizes: &[i64],
    elem: &TypeSpec,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    let idx: String = (0..sizes.len()).map(|k| format!("[zdi{k}]")).collect();
    let mut body = map_get(elem, &format!("{field}{idx}"), enum_names, struct_names)?;
    for k in (0..sizes.len()).rev() {
        body = format!(
            "for (size_t zdi{k} = 0; zdi{k} < {}; zdi{k}++) {{ {body} }}",
            sizes[k]
        );
    }
    Ok(body)
}
