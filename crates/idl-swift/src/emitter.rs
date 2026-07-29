// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → Swift emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! Swift source file: a `Writer` (byte-identical to `endpoints/swift`) plus, per
//! IDL `struct`, a Swift struct with a `marshalXCDR(_ endian)` method. `@final`
//! and `@appendable` are supported; other extensibilities and constructs raise
//! [`IdlSwiftError::Unsupported`].

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

use crate::error::{IdlSwiftError, Result};
use crate::keywords::escape_swift_ident;

/// Options for the Swift backend.
#[derive(Debug, Clone, Default)]
pub struct SwiftGenOptions {}

/// The shared XCDR2 `Writer`, byte-identical to `endpoints/swift`.
const WIRE_PRELUDE: &str = r#"public enum Endianness { case little, big }

public struct Writer {
    public private(set) var buf: [UInt8] = []
    let endian: Endianness
    public init(_ endian: Endianness) { self.endian = endian }

    mutating func align(_ a: Int) {
        let cap = a > 4 ? 4 : a
        while buf.count % cap != 0 { buf.append(0) }
    }
    mutating func putLE(_ a: Int, _ le: [UInt8]) {
        align(a)
        buf.append(contentsOf: endian == .big ? le.reversed() : le)
    }
    public mutating func putU8(_ v: UInt8) { buf.append(v) }
    public mutating func putBool(_ v: Bool) { buf.append(v ? 1 : 0) }
    public mutating func putU16(_ v: UInt16) {
        putLE(2, [UInt8(v & 0xff), UInt8((v >> 8) & 0xff)])
    }
    public mutating func putU32(_ v: UInt32) {
        putLE(4, [UInt8(v & 0xff), UInt8((v >> 8) & 0xff), UInt8((v >> 16) & 0xff), UInt8((v >> 24) & 0xff)])
    }
    public mutating func putU64(_ v: UInt64) {
        var le = [UInt8]()
        for i in 0..<8 { le.append(UInt8((v >> (8 * UInt64(i))) & 0xff)) }
        putLE(4, le)
    }
    public mutating func putF32(_ v: Float) { putU32(v.bitPattern) }
    public mutating func putF64(_ v: Double) { putU64(v.bitPattern) }
    public mutating func putBytes(_ b: [UInt8]) { buf.append(contentsOf: b) }
    public mutating func putString(_ s: String) {
        let bytes = Array(s.utf8)
        putU32(UInt32(bytes.count + 1))
        putBytes(bytes)
        putU8(0)
    }
    public mutating func putSeqU8(_ b: [UInt8]) { putU32(UInt32(b.count)); putBytes(b) }
    public mutating func putWString(_ s: String) {
        let units = Array(s.utf16)
        putU32(UInt32(units.count * 2))
        for u in units { putU16(u) }
    }
    public mutating func putLongDouble(_ v: Double) {
        let bits = v.bitPattern
        let sign = bits >> 63
        let exp = (bits >> 52) & 0x7FF
        let mant = bits & 0xFFFFFFFFFFFFF
        var hi = sign << 63
        var lo: UInt64 = 0
        if !(exp == 0 && mant == 0) {
            hi = (sign << 63) | ((exp - 1023 + 16383) << 48) | (mant >> 4)
            lo = (mant & 0xF) << 60
        }
        var le = [UInt8](repeating: 0, count: 16)
        for i in 0..<8 {
            le[i] = UInt8((lo >> (8 * UInt64(i))) & 0xff)
            le[8 + i] = UInt8((hi >> (8 * UInt64(i))) & 0xff)
        }
        putLE(4, le)
    }
    public func bytes() -> [UInt8] { buf }
}

public struct Reader {
    let buf: [UInt8]
    var pos: Int = 0
    let endian: Endianness
    public init(_ buf: [UInt8], _ endian: Endianness) { self.buf = buf; self.endian = endian }

    mutating func ralign(_ a: Int) { let cap = a > 4 ? 4 : a; while pos % cap != 0 { pos += 1 } }
    mutating func getLE(_ a: Int, _ n: Int) -> UInt64 {
        ralign(a)
        var v: UInt64 = 0
        if endian == .big {
            for i in 0..<n { v = (v << 8) | UInt64(buf[pos + i]) }
        } else {
            for i in stride(from: n - 1, through: 0, by: -1) { v = (v << 8) | UInt64(buf[pos + i]) }
        }
        pos += n
        return v
    }
    public mutating func getU8() -> UInt8 { let v = buf[pos]; pos += 1; return v }
    public mutating func getBool() -> Bool { return getU8() != 0 }
    public mutating func getU16() -> UInt16 { return UInt16(getLE(2, 2)) }
    public mutating func getU32() -> UInt32 { return UInt32(getLE(4, 4)) }
    public mutating func getU64() -> UInt64 { return getLE(4, 8) }
    public mutating func getF32() -> Float { return Float(bitPattern: getU32()) }
    public mutating func getF64() -> Double { return Double(bitPattern: getU64()) }
    public mutating func getBytesN(_ n: Int) -> [UInt8] { let b = Array(buf[pos..<pos + n]); pos += n; return b }
    public mutating func getString() -> String {
        let n = Int(getU32())
        let b = getBytesN(n)
        return n > 0 ? String(decoding: b[0..<n - 1], as: UTF8.self) : ""
    }
    public mutating func getSeqU8() -> [UInt8] { return getBytesN(Int(getU32())) }
    public mutating func getWString() -> String {
        let n = Int(getU32()) / 2
        var units = [UInt16]()
        for _ in 0..<n { units.append(getU16()) }
        return String(decoding: units, as: UTF16.self)
    }
    public mutating func getLongDouble() -> Double {
        ralign(4)
        var le = getBytesN(16)
        if endian == .big { le.reverse() }
        var lo: UInt64 = 0
        var hi: UInt64 = 0
        for i in 0..<8 {
            lo |= UInt64(le[i]) << (8 * UInt64(i))
            hi |= UInt64(le[8 + i]) << (8 * UInt64(i))
        }
        let sign = hi >> 63
        let exp = (hi >> 48) & 0x7FFF
        let mant = ((hi & 0xFFFFFFFFFFFF) << 4) | (lo >> 60)
        let bits = (exp == 0 && mant == 0) ? (sign << 63) : ((sign << 63) | ((exp - 16383 + 1023) << 52) | mant)
        return Double(bitPattern: bits)
    }
}

/// A DDS-XTypes §7.4.3 bounded-collection violation: an encode value, or a
/// decoded wire value (which may originate from an untrusted peer), longer
/// than its IDL `<N>` bound. A catchable `Error` so callers can reject an
/// over-bound peer-supplied value without aborting the process.
public struct XcdrBoundError: Error, CustomStringConvertible {
    public let message: String
    public init(_ message: String) { self.message = message }
    public var description: String { message }
}
"#;

/// Generates a self-contained Swift module from the IDL AST.
///
/// # Errors
/// Returns [`IdlSwiftError::Unsupported`] for constructs the Swift backend does
/// not yet emit (unions, nested-struct members, maps, `long double`, `@mutable`,
/// …).
pub fn generate_swift_module(spec: &Specification, _opts: &SwiftGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Code generated by zerodds-idlc (Swift backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "// SPDX-License-Identifier: Apache-2.0\n");
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
    // CryptoKit is needed only for the KeyHash MD5 branch; import it on demand.
    if out.contains("Insecure.MD5") {
        out = format!("import CryptoKit\n{out}");
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

/// Emits an IDL `enum` as a Swift `Int32`-raw enum.
fn emit_enum(out: &mut String, e: &EnumDef) {
    let values = enumerator_values(e);
    let ty = escape_swift_ident(&e.name.text);
    let _ = writeln!(
        out,
        "
public enum {ty}: Int32 {{"
    );
    for (en, value) in e.enumerators.iter().zip(&values) {
        let _ = writeln!(
            out,
            "    case {} = {value}",
            escape_swift_ident(&en.name.text)
        );
    }
    let _ = writeln!(out, "}}");
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

/// Wraps a per-element put (`$elem`) in nested row-major `for … in 0..<N` loops
/// over a fixed array `<field>[zdi0][zdi1]…` (Swift accesses properties bare).
fn build_array_put(field: &str, sizes: &[i64], elem_put: &str) -> String {
    let idx: String = (0..sizes.len()).map(|k| format!("[zdi{k}]")).collect();
    let mut body = elem_put.replace("$elem", &format!("{field}{idx}"));
    for k in (0..sizes.len()).rev() {
        body = format!("for zdi{k} in 0..<{} {{ {body} }}", sizes[k]);
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
    typedefs: &HashMap<String, TypeSpec>,
    struct_defs: &HashMap<String, &StructDef>,
) -> Result<()> {
    let ext = extensibility(s);

    struct FieldGen {
        name: String,
        swift_type: String,
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
            let name = escape_swift_ident(&d.name().text);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let mut array_sizes: Option<Vec<i64>> = None;
            let (swift_type, put, get) = match d {
                Declarator::Simple(_) => {
                    let (t, p) = map_type(&resolved, &name, enum_names, struct_names)?;
                    let g = map_get(&resolved, &name, enum_names, struct_names)?;
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
                            IdlSwiftError::Unsupported(format!(
                                "non-literal array size on `{name}`"
                            ))
                        })?;
                    let (elem_type, elem_put) =
                        map_type(&resolved, "$elem", enum_names, struct_names)?;
                    let swift_type = sizes
                        .iter()
                        .fold(elem_type.clone(), |inner, _| format!("[{inner}]"));
                    let put = build_array_put(&name, &sizes, &elem_put);
                    let elem_get = map_get(&resolved, "zdE", enum_names, struct_names)?;
                    let get = build_array_get(&name, &sizes, &elem_type, &elem_get);
                    array_sizes = Some(sizes);
                    (swift_type, put, get)
                }
            };
            fields.push(FieldGen {
                name,
                swift_type,
                put,
                get,
                id,
                key,
                resolved_type: resolved.clone(),
                array_sizes,
            });
        }
    }

    let ty = escape_swift_ident(&s.name.text);
    let _ = writeln!(out, "\npublic struct {ty} {{");
    for f in &fields {
        let _ = writeln!(out, "    public var {}: {}", f.name, f.swift_type);
    }

    // marshalInto writes into an existing writer (nested composites call this so
    // alignment stays stream-relative). @final: inline; @appendable: DHEADER.
    // `throws`: a bounded member's encode-side check (XTypes §7.4.3) throws
    // `XcdrBoundError` on an over-bound value (B1 blocker fix — was
    // `fatalError`, an uncatchable process abort).
    let _ = writeln!(
        out,
        "\n    public func marshalInto(_ w: inout Writer) throws {{"
    );
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "        var body = Writer(w.endian)");
        for f in &fields {
            let emh = 0x4000_0000_u32 | f.id;
            let _ = writeln!(out, "        body.putU32(0x{emh:08x})");
            let _ = writeln!(out, "        do {{");
            let _ = writeln!(out, "            var zdMem = Writer(w.endian)");
            let _ = writeln!(out, "            {}", f.put.replace("$w", "zdMem"));
            let _ = writeln!(out, "            let zdMB = zdMem.bytes()");
            let _ = writeln!(out, "            body.putU32(UInt32(zdMB.count))");
            let _ = writeln!(out, "            body.putBytes(zdMB)");
            let _ = writeln!(out, "        }}");
        }
        let _ = writeln!(out, "        let zdBB = body.bytes()");
        let _ = writeln!(out, "        w.putU32(UInt32(zdBB.count))");
        let _ = writeln!(out, "        w.putBytes(zdBB)");
    } else {
        let wv = if ext == ExtensibilityKind::Final {
            "w"
        } else {
            let _ = writeln!(out, "        var body = Writer(w.endian)");
            "body"
        };
        for f in &fields {
            let _ = writeln!(out, "        {}", f.put.replace("$w", wv));
        }
        if ext != ExtensibilityKind::Final {
            let _ = writeln!(out, "        let bb = body.bytes()");
            let _ = writeln!(out, "        w.putU32(UInt32(bb.count))");
            let _ = writeln!(out, "        w.putBytes(bb)");
        }
    }
    let _ = writeln!(out, "    }}");

    let _ = writeln!(
        out,
        "\n    public func marshalXCDR(_ endian: Endianness) throws -> [UInt8] {{"
    );
    let _ = writeln!(out, "        var w = Writer(endian)");
    let _ = writeln!(out, "        try self.marshalInto(&w)");
    let _ = writeln!(out, "        return w.bytes()");
    let _ = writeln!(out, "    }}");

    // Decode (inverse of marshalInto). @final: inline reads; @appendable: skip
    // the DHEADER then read inline; @mutable: skip DHEADER, then per member skip
    // EMHEADER + NEXTINT and read (members in declaration order).
    // `throws`: a bounded member's decode-side check throws `XcdrBoundError`
    // on an over-bound *wire* value — untrusted-peer input, so a catchable
    // error, not `fatalError` (B1 blocker fix).
    let _ = writeln!(
        out,
        "\n    public static func unmarshalFrom(_ r: inout Reader) throws -> {ty} {{"
    );
    if ext == ExtensibilityKind::Mutable {
        let _ = writeln!(out, "        _ = r.getU32() // DHEADER");
        for f in &fields {
            let _ = writeln!(out, "        var {}: {}", f.name, f.swift_type);
            let _ = writeln!(out, "        _ = r.getU32() // EMHEADER");
            let _ = writeln!(out, "        _ = r.getU32() // NEXTINT");
            let _ = writeln!(out, "        {}", f.get);
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "        _ = r.getU32() // DHEADER");
        }
        for f in &fields {
            let _ = writeln!(out, "        var {}: {}", f.name, f.swift_type);
            let _ = writeln!(out, "        {}", f.get);
        }
    }
    let args = fields
        .iter()
        .map(|f| format!("{n}: {n}", n = f.name))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "        return {ty}({args})");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(
        out,
        "\n    public static func unmarshalXCDR(_ buf: [UInt8], _ endian: Endianness) throws -> {ty} {{"
    );
    let _ = writeln!(out, "        var r = Reader(buf, endian)");
    let _ = writeln!(out, "        return try unmarshalFrom(&r)");
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
        let use_md5 = zerodds_idl::keyhash::uses_md5(&key_members, struct_defs, typedefs);
        // `throws`: a @key member's put statement can itself throw (bounded
        // string/wstring/sequence/map key, or a nested-struct key's
        // marshalInto) — see marshalInto above.
        let _ = writeln!(out, "\n    public func keyHash() throws -> [UInt8] {{");
        let _ = writeln!(out, "        var kw = Writer(.big)");
        for f in &zdkeys {
            // Bug A: a struct-typed `@key` member must expand to that
            // struct's own `@key` subset (or ALL its members if it declares
            // none — XTypes 1.3 §7.6.8), not the full member set that
            // `f.put` (shared with normal, non-key encoding) would emit via
            // `marshalInto`. Non-struct key fields are unaffected: `key_put`
            // falls back to the same `map_type` put used for `f.put`.
            let is_struct_key = matches!(&f.resolved_type, TypeSpec::Scoped(sn)
                if struct_defs.contains_key(&sn.parts.last().map(|p| p.text.clone()).unwrap_or_default()));
            let put = if is_struct_key {
                match &f.array_sizes {
                    None => key_put(
                        &f.name,
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
            let _ = writeln!(out, "        {}", put.replace("$w", "kw"));
        }
        let _ = writeln!(out, "        let b = kw.bytes()");
        if use_md5 {
            // KeyHolder max size > 16 → MD5(bytes)[0..16] (XTypes §7.6.8.4).
            let _ = writeln!(
                out,
                "        return Array(Insecure.MD5.hash(data: Data(b)))"
            );
        } else {
            let _ = writeln!(out, "        var outk = [UInt8](repeating: 0, count: 16)");
            let _ = writeln!(
                out,
                "        for i in 0..<min(16, b.count) {{ outk[i] = b[i] }}"
            );
            let _ = writeln!(out, "        return outk");
        }
        let _ = writeln!(out, "    }}");
    }
    let _ = writeln!(out, "}}");
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
        return Err(IdlSwiftError::Unsupported(format!(
            "@mutable union {} (EMHEADER framing not yet emitted)",
            u.name.text
        )));
    }
    let (disc_type, disc_put) = map_type(
        &switch_typespec(&u.switch_type),
        "disc",
        enum_names,
        struct_names,
    )?;
    let mut cases: Vec<UnionCase> = Vec::new();
    for c in &u.cases {
        let field = escape_swift_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ty, put) = map_type(&resolved, &field, enum_names, struct_names)?;
        let get = map_get(&resolved, &format!("v.{field}"), enum_names, struct_names)?;
        let zero = zero_value(&ty, enum_names);
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => labels.push(array_size(e).ok_or_else(|| {
                    IdlSwiftError::Unsupported(format!(
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
    let has_default = cases.iter().any(|c| c.is_default);

    let ty = escape_swift_ident(&u.name.text);
    let _ = writeln!(out, "\npublic struct {ty} {{");
    let _ = writeln!(out, "    public var disc: {disc_type}");
    for c in &cases {
        let _ = writeln!(out, "    public var {}: {}", c.field, c.ty);
    }
    let _ = writeln!(
        out,
        "\n    public func marshalInto(_ w: inout Writer) throws {{"
    );
    let wv = if ext == ExtensibilityKind::Final {
        "w"
    } else {
        let _ = writeln!(out, "        var body = Writer(w.endian)");
        "body"
    };
    let _ = writeln!(out, "        {}", disc_put.replace("$w", wv));
    let _ = writeln!(out, "        switch disc {{");
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "        default:");
        } else {
            let lbl = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "        case {lbl}:");
        }
        let _ = writeln!(out, "            {}", c.put.replace("$w", wv));
    }
    if !has_default {
        let _ = writeln!(out, "        default: break");
    }
    let _ = writeln!(out, "        }}");
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "        let bb = body.bytes()");
        let _ = writeln!(out, "        w.putU32(UInt32(bb.count))");
        let _ = writeln!(out, "        w.putBytes(bb)");
    }
    let _ = writeln!(out, "    }}");
    let _ = writeln!(
        out,
        "\n    public func marshalXCDR(_ endian: Endianness) throws -> [UInt8] {{"
    );
    let _ = writeln!(out, "        var w = Writer(endian)");
    let _ = writeln!(out, "        try self.marshalInto(&w)");
    let _ = writeln!(out, "        return w.bytes()");
    let _ = writeln!(out, "    }}");

    // Decode: read the discriminator, build the holder zero-filled, then read
    // only the selected member (@appendable skips the leading DHEADER).
    let disc_get = map_get(
        &switch_typespec(&u.switch_type),
        "zdDisc",
        enum_names,
        struct_names,
    )?;
    let _ = writeln!(
        out,
        "\n    public static func unmarshalFrom(_ r: inout Reader) throws -> {ty} {{"
    );
    if ext == ExtensibilityKind::Appendable {
        let _ = writeln!(out, "        _ = r.getU32() // DHEADER");
    }
    let _ = writeln!(out, "        var zdDisc: {disc_type}");
    let _ = writeln!(out, "        {disc_get}");
    let zeros = cases
        .iter()
        .map(|c| format!("{}: {}", c.field, c.zero))
        .collect::<Vec<_>>()
        .join(", ");
    let sep = if cases.is_empty() { "" } else { ", " };
    let _ = writeln!(out, "        var v = {ty}(disc: zdDisc{sep}{zeros})");
    let _ = writeln!(out, "        switch zdDisc {{");
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "        default:");
        } else {
            let lbl = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "        case {lbl}:");
        }
        let _ = writeln!(out, "            {}", c.get);
    }
    if !has_default {
        let _ = writeln!(out, "        default: break");
    }
    let _ = writeln!(out, "        }}");
    let _ = writeln!(out, "        return v");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(
        out,
        "\n    public static func unmarshalXCDR(_ buf: [UInt8], _ endian: Endianness) throws -> {ty} {{"
    );
    let _ = writeln!(out, "        var r = Reader(buf, endian)");
    let _ = writeln!(out, "        return try unmarshalFrom(&r)");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "}}");
    Ok(())
}

/// Maps an IDL type to `(Swift type, put statement)`. The put uses `$w` as the
/// writer placeholder and `expr` as the struct field name.
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
            "$w.putU32(UInt32({expr}.count))\n        for zdK in {expr}.keys.sorted() {{ {key_put}; {val_put} }}"
        )
    } else {
        let kp = key_put.replace("$w", "zdSub");
        let vp = val_put.replace("$w", "zdSub");
        format!(
            "var zdSub = Writer($w.endian)\n        zdSub.putU32(UInt32({expr}.count))\n        for zdK in {expr}.keys.sorted() {{ {kp}; {vp} }}\n        let zdBB = zdSub.bytes()\n        $w.putU32(UInt32(zdBB.count)); $w.putBytes(zdBB)"
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
            let put = match &st.bound {
                // B1 blocker fix (deep review of #22 decode-bounds-cross-backend):
                // XTypes 1.3 §7.4.3 bounded string<N>, UTF-8 byte length (matches
                // the CDR wire), mirroring the check idiom already landed in
                // idl-cpp/idl-csharp/idl-java. `marshalInto`/`unmarshalFrom` are
                // now `throws` Swift functions (uniformly, across every emitted
                // type) so a bound violation raises a catchable `XcdrBoundError`
                // instead of the previous `fatalError` process abort — a
                // decoded over-bound wire value is untrusted-peer input, not a
                // programmer invariant break.
                Some(b) if array_size(b).is_some() => {
                    let bv = array_size(b).unwrap_or_default();
                    format!(
                        "if {expr}.utf8.count > {bv} {{ throw XcdrBoundError(\"bounded string length exceeds its IDL bound ({bv})\") }}\n        $w.putString({expr})"
                    )
                }
                _ => format!("$w.putString({expr})"),
            };
            Ok(("String".to_string(), put))
        }
        TypeSpec::String(st) => {
            let put = match &st.bound {
                Some(b) if array_size(b).is_some() => {
                    let bv = array_size(b).unwrap_or_default();
                    format!(
                        "if {expr}.utf16.count > {bv} {{ throw XcdrBoundError(\"bounded wstring length exceeds its IDL bound ({bv})\") }}\n        $w.putWString({expr})"
                    )
                }
                _ => format!("$w.putWString({expr})"),
            };
            Ok(("String".to_string(), put))
        }
        TypeSpec::Sequence(seq) => map_sequence(&seq.elem, seq.bound.as_ref(), expr, struct_names),
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                Ok((
                    escape_swift_ident(&name),
                    format!("$w.putU32(UInt32(bitPattern: {expr}.rawValue))"),
                ))
            } else if struct_names.contains(&name) {
                Ok((
                    escape_swift_ident(&name),
                    format!("try {expr}.marshalInto(&$w)"),
                ))
            } else {
                Err(IdlSwiftError::Unsupported(format!("scoped type {name}")))
            }
        }
        // A map: entries sorted ascending by key, `u32 count` + key/value pairs
        // (no DHEADER for a primitive pair; DHEADER-framed otherwise).
        TypeSpec::Map(m) => {
            let (key_type, key_put) = map_type(&m.key, "zdK", enum_names, struct_names)?;
            let (val_type, val_put) =
                map_type(&m.value, &format!("{expr}[zdK]!"), enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let bound_check = m.bound.as_ref().and_then(array_size).map(|bv| {
                format!("if {expr}.count > {bv} {{ throw XcdrBoundError(\"bounded map length exceeds its IDL bound ({bv})\") }}\n        ")
            }).unwrap_or_default();
            Ok((
                format!("[{key_type}: {val_type}]"),
                format!(
                    "{bound_check}{}",
                    build_map_put(expr, &key_put, &val_put, prim)
                ),
            ))
        }
        other => Err(IdlSwiftError::Unsupported(format!("type {other:?}"))),
    }
}

/// Builds a KeyHash-writer statement (using the `$w` placeholder like
/// `map_type`'s put strings) for one `@key` member value. Reuses the shared
/// per-field `map_type` put for primitive/string/enum/sequence/typedef
/// members — safe, since normal and key encoding agree there. For a
/// struct-typed key member (Bug A: nested-struct `@key` member must not
/// include non-key fields, XTypes 1.3 §7.6.8), does NOT call the struct's
/// full `marshalInto`; instead expands to the struct's own `@key` members
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
                        return Err(IdlSwiftError::Unsupported(
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
        PrimitiveType::Octet | PrimitiveType::Char => ("UInt8", format!("$w.putU8({expr})")),
        PrimitiveType::Boolean => ("Bool", format!("$w.putBool({expr})")),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => ("Float", format!("$w.putF32({expr})")),
        PrimitiveType::Floating(FloatingType::Double) => ("Double", format!("$w.putF64({expr})")),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            ("Double", format!("$w.putLongDouble({expr})"))
        }
        PrimitiveType::WideChar => ("UInt32", format!("$w.putU32({expr})")),
    };
    Ok((ty.to_string(), put))
}

fn map_integer(i: IntegerType, expr: &str) -> Result<(String, String)> {
    // Signed IDL integers reinterpret to the unsigned wire via `bitPattern`.
    let (ty, put) = match i {
        IntegerType::UInt8 => ("UInt8", format!("$w.putU8({expr})")),
        IntegerType::Int8 => ("Int8", format!("$w.putU8(UInt8(bitPattern: {expr}))")),
        IntegerType::UShort | IntegerType::UInt16 => ("UInt16", format!("$w.putU16({expr})")),
        IntegerType::Short | IntegerType::Int16 => {
            ("Int16", format!("$w.putU16(UInt16(bitPattern: {expr}))"))
        }
        IntegerType::ULong | IntegerType::UInt32 => ("UInt32", format!("$w.putU32({expr})")),
        IntegerType::Long | IntegerType::Int32 => {
            ("Int32", format!("$w.putU32(UInt32(bitPattern: {expr}))"))
        }
        IntegerType::ULongLong | IntegerType::UInt64 => ("UInt64", format!("$w.putU64({expr})")),
        IntegerType::LongLong | IntegerType::Int64 => {
            ("Int64", format!("$w.putU64(UInt64(bitPattern: {expr}))"))
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
    // B1 blocker fix (deep review of #22 decode-bounds-cross-backend): XTypes
    // 1.3 §7.4.3 `sequence<T, N>`. `throw XcdrBoundError` — a catchable error
    // via the now-`throws` marshal functions — replaces the previous
    // `fatalError` process abort.
    let bound_check = bound.and_then(array_size).map(|bv| {
        format!("if {expr}.count > {bv} {{ throw XcdrBoundError(\"bounded sequence length exceeds its IDL bound ({bv})\") }}\n        ")
    }).unwrap_or_default();
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok((
            "[UInt8]".to_string(),
            format!("{bound_check}$w.putSeqU8({expr})"),
        ));
    }
    // sequence<struct> → collection DHEADER + count + each element.
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let put = format!(
                "{bound_check}do {{ var sub = Writer($w.endian); sub.putU32(UInt32({expr}.count));                  for e in {expr} {{ try e.marshalInto(&sub) }}; let bb = sub.bytes();                  $w.putU32(UInt32(bb.count)); $w.putBytes(bb) }}"
            );
            return Ok((format!("[{}]", escape_swift_ident(&name)), put));
        }
    }
    Err(IdlSwiftError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}

// ---- decode (inverse of the put path): a `Reader` wire-core in the prelude,
// plus `map_get` — the inverse of `map_type` — emitting a statement that reads
// one value from `r` and assigns it into `target`. Roundtrip-verified against
// the goldens: `marshal(unmarshal(golden)) == golden` for LE and BE.

/// A zero/empty value for a Swift type, used to construct a union holder before
/// the selected member is overwritten (mirrors Go/D zero-init on decode).
fn zero_value(swift_type: &str, enum_names: &HashSet<String>) -> String {
    if swift_type.starts_with('[') && swift_type.contains(": ") {
        "[:]".to_string()
    } else if swift_type.starts_with('[') {
        "[]".to_string()
    } else if swift_type == "String" {
        "\"\"".to_string()
    } else if swift_type == "Bool" {
        "false".to_string()
    } else if enum_names
        .iter()
        .any(|n| escape_swift_ident(n) == swift_type)
    {
        format!("{swift_type}(rawValue: 0)!")
    } else {
        "0".to_string()
    }
}

/// Builds the read for a fixed array: nested row-major `for` loops appending
/// each element (inverse of [`build_array_put`]). `elem_get` targets `zdE`.
fn build_array_get(target: &str, sizes: &[i64], elem_type: &str, elem_get: &str) -> String {
    /// zerodds-lint: recursion-depth 32
    fn rec(lval: &str, sizes: &[i64], depth: usize, elem_type: &str, elem_get: &str) -> String {
        let s = sizes[depth];
        if depth + 1 == sizes.len() {
            format!(
                "{lval} = []\n        for _ in 0..<{s} {{ var zdE: {elem_type}\n        {elem_get}\n        {lval}.append(zdE) }}"
            )
        } else {
            let inner_type: String =
                (depth + 1..sizes.len()).fold(elem_type.to_string(), |t, _| format!("[{t}]"));
            let inner = rec(
                &format!("zdRow{depth}"),
                sizes,
                depth + 1,
                elem_type,
                elem_get,
            );
            format!(
                "{lval} = []\n        for _ in 0..<{s} {{ var zdRow{depth}: {inner_type}\n        {inner}\n        {lval}.append(zdRow{depth}) }}"
            )
        }
    }
    rec(target, sizes, 0, elem_type, elem_get)
}

/// Emits a statement reading one value of IDL type `t` from `r` into `target`.
/// zerodds-lint: recursion-depth 32
fn map_get(
    t: &TypeSpec,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    match t {
        TypeSpec::Primitive(p) => map_get_primitive(*p, target),
        TypeSpec::String(st) if !st.wide => match st.bound.as_ref().and_then(array_size) {
            Some(bv) => Ok(format!(
                "{target} = r.getString()\n        if {target}.utf8.count > {bv} {{ throw XcdrBoundError(\"decoded string length exceeds its IDL bound ({bv})\") }}"
            )),
            None => Ok(format!("{target} = r.getString()")),
        },
        TypeSpec::String(st) => match st.bound.as_ref().and_then(array_size) {
            Some(bv) => Ok(format!(
                "{target} = r.getWString()\n        if {target}.utf16.count > {bv} {{ throw XcdrBoundError(\"decoded wstring length exceeds its IDL bound ({bv})\") }}"
            )),
            None => Ok(format!("{target} = r.getWString()")),
        },
        TypeSpec::Sequence(seq) => {
            map_get_sequence(&seq.elem, seq.bound.as_ref(), target, struct_names)
        }
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                let esc = escape_swift_ident(&name);
                Ok(format!(
                    "{target} = {esc}(rawValue: Int32(bitPattern: r.getU32()))!"
                ))
            } else if struct_names.contains(&name) {
                let esc = escape_swift_ident(&name);
                Ok(format!("{target} = try {esc}.unmarshalFrom(&r)"))
            } else {
                Err(IdlSwiftError::Unsupported(format!("scoped type {name}")))
            }
        }
        TypeSpec::Map(m) => {
            let (key_type, _) = map_type(&m.key, "zdK", enum_names, struct_names)?;
            let (val_type, _) = map_type(&m.value, "zdV", enum_names, struct_names)?;
            let key_get = map_get(&m.key, "zdK", enum_names, struct_names)?;
            let val_get = map_get(&m.value, "zdV", enum_names, struct_names)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let dh = if prim { "" } else { "_ = r.getU32()\n        " };
            let bound_check = m.bound.as_ref().and_then(array_size).map(|bv| {
                format!("if zdN > {bv} {{ throw XcdrBoundError(\"decoded map length exceeds its IDL bound ({bv})\") }}; ")
            }).unwrap_or_default();
            Ok(format!(
                "{dh}do {{ let zdN = Int(r.getU32()); {bound_check}{target} = [:]\n        for _ in 0..<zdN {{ var zdK: {key_type}; {key_get}; var zdV: {val_type}; {val_get}; {target}[zdK] = zdV }} }}"
            ))
        }
        other => Err(IdlSwiftError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType, target: &str) -> Result<String> {
    let s = match p {
        PrimitiveType::Octet | PrimitiveType::Char => format!("{target} = r.getU8()"),
        PrimitiveType::Boolean => format!("{target} = r.getBool()"),
        PrimitiveType::Integer(i) => return map_get_integer(i, target),
        PrimitiveType::Floating(FloatingType::Float) => format!("{target} = r.getF32()"),
        PrimitiveType::Floating(FloatingType::Double) => format!("{target} = r.getF64()"),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            format!("{target} = r.getLongDouble()")
        }
        PrimitiveType::WideChar => format!("{target} = r.getU32()"),
    };
    Ok(s)
}

fn map_get_integer(i: IntegerType, target: &str) -> Result<String> {
    let s = match i {
        IntegerType::UInt8 => format!("{target} = r.getU8()"),
        IntegerType::Int8 => format!("{target} = Int8(bitPattern: r.getU8())"),
        IntegerType::UShort | IntegerType::UInt16 => format!("{target} = r.getU16()"),
        IntegerType::Short | IntegerType::Int16 => {
            format!("{target} = Int16(bitPattern: r.getU16())")
        }
        IntegerType::ULong | IntegerType::UInt32 => format!("{target} = r.getU32()"),
        IntegerType::Long | IntegerType::Int32 => {
            format!("{target} = Int32(bitPattern: r.getU32())")
        }
        IntegerType::ULongLong | IntegerType::UInt64 => format!("{target} = r.getU64()"),
        IntegerType::LongLong | IntegerType::Int64 => {
            format!("{target} = Int64(bitPattern: r.getU64())")
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
    let bv = bound.and_then(array_size);
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok(match bv {
            Some(bv) => format!(
                "{target} = r.getSeqU8()\n        if {target}.count > {bv} {{ throw XcdrBoundError(\"decoded sequence length exceeds its IDL bound ({bv})\") }}"
            ),
            None => format!("{target} = r.getSeqU8()"),
        });
    }
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let bound_check = bv
                .map(|bv| {
                    format!(
                        "if zdN > {bv} {{ throw XcdrBoundError(\"decoded sequence length exceeds its IDL bound ({bv})\") }}; "
                    )
                })
                .unwrap_or_default();
            let esc = escape_swift_ident(&name);
            return Ok(format!(
                "_ = r.getU32()\n        do {{ let zdN = Int(r.getU32()); {bound_check}{target} = []\n        for _ in 0..<zdN {{ {target}.append(try {esc}.unmarshalFrom(&r)) }} }}"
            ));
        }
    }
    Err(IdlSwiftError::Unsupported(
        "sequence of non-struct, non-octet elements".to_string(),
    ))
}
