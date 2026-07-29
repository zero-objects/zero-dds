// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → Zig emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! Zig source file: a shared XCDR2 `Writer` (byte-identical to `endpoints/zig`)
//! plus, per IDL `struct`, a Zig struct with a `marshalXCDR(endian, allocator)`
//! method. `@final` and `@appendable` are supported; other extensibilities and
//! constructs raise [`IdlZigError::Unsupported`].

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

use crate::error::{IdlZigError, Result};
use crate::keywords::escape_zig_ident;

/// Options for the Zig backend.
#[derive(Debug, Clone, Default)]
pub struct ZigGenOptions {}

/// The shared XCDR2 wire `Writer`, byte-identical to `endpoints/zig`.
const WIRE_PRELUDE: &str = r#"const std = @import("std");

pub const Endian = enum { little, big };

pub const Writer = struct {
    buf: std.ArrayList(u8),
    endian: Endian,

    pub fn init(alloc: std.mem.Allocator, endian: Endian) Writer {
        return .{ .buf = std.ArrayList(u8).init(alloc), .endian = endian };
    }
    pub fn deinit(self: *Writer) void {
        self.buf.deinit();
    }
    fn alignTo(self: *Writer, a: usize) !void {
        const cap: usize = if (a > 4) 4 else a;
        while (self.buf.items.len % cap != 0) try self.buf.append(0);
    }
    fn putLE(self: *Writer, a: usize, le: []const u8) !void {
        try self.alignTo(a);
        if (self.endian == .big) {
            var i: usize = le.len;
            while (i > 0) {
                i -= 1;
                try self.buf.append(le[i]);
            }
        } else {
            try self.buf.appendSlice(le);
        }
    }
    pub fn putU8(self: *Writer, v: u8) !void {
        try self.buf.append(v);
    }
    pub fn putBool(self: *Writer, v: bool) !void {
        try self.buf.append(if (v) 1 else 0);
    }
    pub fn putU16(self: *Writer, v: u16) !void {
        try self.putLE(2, &[_]u8{ @truncate(v), @truncate(v >> 8) });
    }
    pub fn putU32(self: *Writer, v: u32) !void {
        try self.putLE(4, &[_]u8{ @truncate(v), @truncate(v >> 8), @truncate(v >> 16), @truncate(v >> 24) });
    }
    pub fn putU64(self: *Writer, v: u64) !void {
        const le = [_]u8{
            @truncate(v),       @truncate(v >> 8),  @truncate(v >> 16), @truncate(v >> 24),
            @truncate(v >> 32), @truncate(v >> 40), @truncate(v >> 48), @truncate(v >> 56),
        };
        try self.putLE(4, &le);
    }
    pub fn putF32(self: *Writer, v: f32) !void {
        const bits: u32 = @bitCast(v);
        try self.putU32(bits);
    }
    pub fn putF64(self: *Writer, v: f64) !void {
        const bits: u64 = @bitCast(v);
        try self.putU64(bits);
    }
    pub fn putBytes(self: *Writer, b: []const u8) !void {
        try self.buf.appendSlice(b);
    }
    pub fn putString(self: *Writer, s: []const u8) !void {
        try self.putU32(@intCast(s.len + 1));
        try self.putBytes(s);
        try self.putU8(0);
    }
    pub fn putSeqU8(self: *Writer, b: []const u8) !void {
        try self.putU32(@intCast(b.len));
        try self.putBytes(b);
    }
    pub fn putWString(self: *Writer, s: []const u8) !void {
        var units = std.ArrayList(u16).init(self.buf.allocator);
        defer units.deinit();
        const view = try std.unicode.Utf8View.init(s);
        var it = view.iterator();
        while (it.nextCodepoint()) |cp| {
            if (cp <= 0xFFFF) {
                try units.append(@intCast(cp));
            } else {
                const rr = cp - 0x10000;
                try units.append(@intCast(0xD800 + (rr >> 10)));
                try units.append(@intCast(0xDC00 + (rr & 0x3FF)));
            }
        }
        try self.putU32(@intCast(units.items.len * 2));
        for (units.items) |u| try self.putU16(u);
    }
    pub fn putLongDouble(self: *Writer, v: f64) !void {
        const bits: u64 = @bitCast(v);
        const sign = bits >> 63;
        const exp = (bits >> 52) & 0x7FF;
        const mant = bits & 0xFFFFFFFFFFFFF;
        var hi: u64 = sign << 63;
        var lo: u64 = 0;
        if (!(exp == 0 and mant == 0)) {
            hi = (sign << 63) | ((exp - 1023 + 16383) << 48) | (mant >> 4);
            lo = (mant & 0xF) << 60;
        }
        var le: [16]u8 = undefined;
        var i: usize = 0;
        while (i < 8) : (i += 1) {
            le[i] = @truncate(lo >> @intCast(8 * i));
            le[8 + i] = @truncate(hi >> @intCast(8 * i));
        }
        try self.putLE(4, &le);
    }
    pub fn bytes(self: *Writer) []const u8 {
        return self.buf.items;
    }
};

pub const Reader = struct {
    buf: []const u8,
    pos: usize,
    endian: Endian,
    alloc: std.mem.Allocator,

    pub fn init(buf: []const u8, endian: Endian, alloc: std.mem.Allocator) Reader {
        return .{ .buf = buf, .pos = 0, .endian = endian, .alloc = alloc };
    }
    fn ralign(self: *Reader, a: usize) void {
        const cap: usize = if (a > 4) 4 else a;
        while (self.pos % cap != 0) self.pos += 1;
    }
    fn getLE(self: *Reader, a: usize, n: usize) u64 {
        self.ralign(a);
        var v: u64 = 0;
        if (self.endian == .big) {
            var i: usize = 0;
            while (i < n) : (i += 1) v = (v << 8) | self.buf[self.pos + i];
        } else {
            var i: usize = n;
            while (i > 0) {
                i -= 1;
                v = (v << 8) | self.buf[self.pos + i];
            }
        }
        self.pos += n;
        return v;
    }
    pub fn getU8(self: *Reader) u8 {
        const b = self.buf[self.pos];
        self.pos += 1;
        return b;
    }
    pub fn getBool(self: *Reader) bool {
        return self.getU8() != 0;
    }
    pub fn getU16(self: *Reader) u16 {
        return @intCast(self.getLE(2, 2));
    }
    pub fn getU32(self: *Reader) u32 {
        return @intCast(self.getLE(4, 4));
    }
    pub fn getU64(self: *Reader) u64 {
        return self.getLE(4, 8);
    }
    pub fn getF32(self: *Reader) f32 {
        return @bitCast(self.getU32());
    }
    pub fn getF64(self: *Reader) f64 {
        return @bitCast(self.getU64());
    }
    pub fn getBytesN(self: *Reader, n: usize) []const u8 {
        const s = self.buf[self.pos .. self.pos + n];
        self.pos += n;
        return s;
    }
    pub fn getString(self: *Reader) ![]const u8 {
        const n = self.getU32();
        const s = self.buf[self.pos .. self.pos + n - 1];
        self.pos += n;
        return try self.alloc.dupe(u8, s);
    }
    pub fn getSeqU8(self: *Reader) ![]const u8 {
        const n = self.getU32();
        return try self.alloc.dupe(u8, self.getBytesN(n));
    }
    pub fn getWString(self: *Reader) ![]const u8 {
        const n = self.getU32() / 2;
        const units = try self.alloc.alloc(u16, n);
        defer self.alloc.free(units);
        var k: usize = 0;
        while (k < n) : (k += 1) units[k] = self.getU16();
        var out = std.ArrayList(u8).init(self.alloc);
        var i: usize = 0;
        while (i < n) {
            const u = units[i];
            var cp: u21 = undefined;
            if (u >= 0xD800 and u <= 0xDBFF and i + 1 < n) {
                const lo = units[i + 1];
                cp = @intCast(0x10000 + ((@as(u32, u) - 0xD800) << 10) + (@as(u32, lo) - 0xDC00));
                i += 2;
            } else {
                cp = @intCast(u);
                i += 1;
            }
            var b: [4]u8 = undefined;
            const len = try std.unicode.utf8Encode(cp, &b);
            try out.appendSlice(b[0..len]);
        }
        return try out.toOwnedSlice();
    }
    pub fn getLongDouble(self: *Reader) f64 {
        self.ralign(4);
        var le: [16]u8 = undefined;
        @memcpy(&le, self.getBytesN(16));
        if (self.endian == .big) std.mem.reverse(u8, &le);
        var lo: u64 = 0;
        var hi: u64 = 0;
        var i: usize = 0;
        while (i < 8) : (i += 1) {
            lo |= @as(u64, le[i]) << @intCast(8 * i);
            hi |= @as(u64, le[8 + i]) << @intCast(8 * i);
        }
        const sign = hi >> 63;
        const exp = (hi >> 48) & 0x7FFF;
        const mant = ((hi & 0xFFFFFFFFFFFF) << 4) | (lo >> 60);
        const bits: u64 = if (exp == 0 and mant == 0) (sign << 63) else ((sign << 63) | ((exp - 16383 + 1023) << 52) | mant);
        return @bitCast(bits);
    }
};

/// UTF-16 code-unit count of a UTF-8 slice (surrogate-pair aware: a non-BMP
/// codepoint is 2 units), matching the unit count `Writer.putWString`/
/// `Reader.getWString` themselves write/read on the wire. Moderate fix
/// (deep review of #22 decode-bounds-cross-backend): the previous
/// `wstring<N>` bound checks used `.len` directly on the `[]const u8` UTF-8
/// value — the UTF-8 BYTE length, not the UTF-16 unit count DDS-XTypes 1.3
/// §7.4.3 actually bounds. For any non-ASCII text the two diverge (e.g. a
/// 3-character CJK string is 9 UTF-8 bytes but 3 UTF-16 units), so the old
/// check was simply wrong, not just imprecise at the margins.
fn wstringUnitLen(s: []const u8) !usize {
    var count: usize = 0;
    const view = try std.unicode.Utf8View.init(s);
    var it = view.iterator();
    while (it.nextCodepoint()) |cp| {
        count += if (cp <= 0xFFFF) @as(usize, 1) else @as(usize, 2);
    }
    return count;
}
"#;

/// Generates a self-contained Zig module from the IDL AST.
///
/// # Errors
/// Returns [`IdlZigError::Unsupported`] for constructs the Zig backend does not
/// yet emit (unions, nested-struct members, maps, `long double`, `@mutable`, …).
pub fn generate_zig_module(spec: &Specification, _opts: &ZigGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Code generated by zerodds-idlc (Zig backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "// SPDX-License-Identifier: Apache-2.0");
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

/// Emits an IDL `enum` as a Zig `enum(i32)` with explicit enumerator values.
fn emit_enum(out: &mut String, e: &EnumDef) {
    let values = enumerator_values(e);
    let ty = escape_zig_ident(&e.name.text);
    let _ = writeln!(
        out,
        "
pub const {ty} = enum(i32) {{"
    );
    for (en, value) in e.enumerators.iter().zip(&values) {
        let _ = writeln!(out, "    {} = {value},", escape_zig_ident(&en.name.text));
    }
    let _ = writeln!(out, "}};");
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

/// Builds a map put: copy the `[]const KV` slice, sort ascending by key, then
/// `u32 count` + key/value pairs (DHEADER-framed unless the pair is primitive).
fn build_map_put(
    field: &str,
    kv: &str,
    key_put: &str,
    val_put: &str,
    prim: bool,
    bound: Option<&ConstExpr>,
) -> String {
    // Bounded `map<K, V, N>` (XTypes 1.3 §7.4.3): reject an over-bound value
    // before it reaches the wire.
    let bound_check = bound
        .map(|b| {
            let bv = array_size(b).unwrap_or(i64::MAX);
            format!("if (self.{field}.len > {bv}) return error.BoundExceeded; ")
        })
        .unwrap_or_default();
    let head = format!(
        "{bound_check}const zdTmp = $w.buf.allocator.alloc({kv}, self.{field}.len) catch unreachable; \
         defer $w.buf.allocator.free(zdTmp); \
         @memcpy(zdTmp, self.{field}); \
         std.mem.sort({kv}, zdTmp, {{}}, struct {{ \
         fn lt(_: void, za: {kv}, zb: {kv}) bool {{ return za.k < zb.k; }} }}.lt);"
    );
    if prim {
        format!(
            "{{ {head} try $w.putU32(@intCast(zdTmp.len)); \
             for (zdTmp) |zdE| {{ {key_put} {val_put} }} }}"
        )
    } else {
        let kp = key_put.replace("$w", "zdSub");
        let vp = val_put.replace("$w", "zdSub");
        format!(
            "{{ {head} var zdSub = Writer.init($w.buf.allocator, $w.endian); \
             defer zdSub.deinit(); try zdSub.putU32(@intCast(zdTmp.len)); \
             for (zdTmp) |zdE| {{ {kp} {vp} }} \
             const zdBB = zdSub.bytes(); try $w.putU32(@intCast(zdBB.len)); \
             try $w.putBytes(zdBB); }}"
        )
    }
}

/// Wraps a per-element put (`$elem`) in nested row-major `while` loops over a
/// fixed array `self.<field>[i0][i1]…`.
fn build_array_put(field: &str, sizes: &[i64], elem_put: &str) -> String {
    // `zdi{k}` (not `i{k}`) — Zig reserves `i0`, `i1`, … as N-bit integer types.
    let idx: String = (0..sizes.len()).map(|k| format!("[zdi{k}]")).collect();
    let mut body = elem_put.replace("$elem", &format!("self.{field}{idx}"));
    for k in (0..sizes.len()).rev() {
        body = format!(
            "{{ var zdi{k}: usize = 0; while (zdi{k} < {}) : (zdi{k} += 1) {{ {body} }} }}",
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
    struct_defs: &HashMap<String, &StructDef>,
) -> Result<()> {
    let ext = extensibility(s);

    struct FieldGen {
        zig_name: String,
        zig_type: String,
        put: String,
        get: String,
        id: u32,
        key: bool,
        resolved_type: TypeSpec,
        array_sizes: Option<Vec<i64>>,
    }
    // KV structs for map members, emitted before the containing struct.
    let mut pre = String::new();
    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    for m in &s.members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        for d in &m.declarators {
            let raw_name = d.name().text.clone();
            let zig_name = escape_zig_ident(&raw_name);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let mut array_sizes: Option<Vec<i64>> = None;
            let (zig_type, put, get) = match (&resolved, d) {
                // A map: a `[]const KV` slice; marshal copies, sorts by key, then
                // `u32 count` + pairs (DHEADER-framed for a non-primitive pair).
                (TypeSpec::Map(mp), Declarator::Simple(_)) => {
                    let (key_type, key_put) = map_type(&mp.key, "zdE.k", enum_names, struct_names)?;
                    let (val_type, val_put) =
                        map_type(&mp.value, "zdE.v", enum_names, struct_names)?;
                    // Composite identifier — raw substrings never collide with a
                    // keyword token, so the raw (unescaped) names are used here.
                    let kv = format!("{}_{raw_name}_KV", s.name.text);
                    let _ = writeln!(
                        pre,
                        "\npub const {kv} = struct {{ k: {key_type}, v: {val_type} }};"
                    );
                    let prim =
                        is_primitive(&mp.key, enum_names) && is_primitive(&mp.value, enum_names);
                    let key_get = map_get(&mp.key, "zdList[zdI].k", enum_names, struct_names)?;
                    let val_get = map_get(&mp.value, "zdList[zdI].v", enum_names, struct_names)?;
                    let dh = if prim { "" } else { "_ = r.getU32(); " };
                    // B1 follow-up (#22 decode-side parity): mirror the
                    // encode-side bound check (build_map_put) — XTypes 1.3
                    // §7.4.3.
                    let map_bound_check = mp
                        .bound
                        .as_ref()
                        .map(|b| {
                            let bv = array_size(b).unwrap_or(i64::MAX);
                            format!("if (zdN > {bv}) return error.BoundExceeded; ")
                        })
                        .unwrap_or_default();
                    let get = format!(
                        "{{ {dh}const zdN = r.getU32(); {map_bound_check}const zdList = try r.alloc.alloc({kv}, zdN); var zdI: usize = 0; while (zdI < zdN) : (zdI += 1) {{ {key_get} {val_get} }} v.{zig_name} = zdList; }}"
                    );
                    (
                        format!("[]const {kv}"),
                        build_map_put(&zig_name, &kv, &key_put, &val_put, prim, mp.bound.as_ref()),
                        get,
                    )
                }
                (_, Declarator::Simple(_)) => {
                    let (t, p) = map_type(
                        &resolved,
                        &format!("self.{zig_name}"),
                        enum_names,
                        struct_names,
                    )?;
                    let g = map_get(
                        &resolved,
                        &format!("v.{zig_name}"),
                        enum_names,
                        struct_names,
                    )?;
                    (t, p, g)
                }
                // Fixed array: elements inline, row-major, no length prefix.
                (_, Declarator::Array(ad)) => {
                    let sizes = ad
                        .sizes
                        .iter()
                        .map(array_size)
                        .collect::<Option<Vec<i64>>>()
                        .ok_or_else(|| {
                            IdlZigError::Unsupported(format!(
                                "non-literal array size on `{raw_name}`"
                            ))
                        })?;
                    let (elem_type, elem_put) =
                        map_type(&resolved, "$elem", enum_names, struct_names)?;
                    let zig_type =
                        sizes.iter().map(|n| format!("[{n}]")).collect::<String>() + &elem_type;
                    let put = build_array_put(&zig_name, &sizes, &elem_put);
                    let elem_get = map_get(&resolved, "$L", enum_names, struct_names)?;
                    let get = build_array_get(&format!("v.{zig_name}"), &sizes, &elem_get);
                    array_sizes = Some(sizes);
                    (zig_type, put, get)
                }
            };
            fields.push(FieldGen {
                zig_name,
                zig_type,
                put,
                get,
                id,
                key,
                resolved_type: resolved.clone(),
                array_sizes,
            });
        }
    }

    out.push_str(&pre);
    let ty = escape_zig_ident(&s.name.text);
    let _ = writeln!(out, "\npub const {ty} = struct {{");
    for f in &fields {
        let _ = writeln!(out, "    {}: {},", f.zig_name, f.zig_type);
    }

    // marshalInto writes into an existing writer (nested composites call this so
    // alignment stays stream-relative). @final: inline; @appendable: DHEADER.
    let _ = writeln!(
        out,
        "\n    pub fn marshalInto(self: {ty}, w: *Writer) !void {{"
    );
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (LC4 =
        // member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(
            out,
            "        var body_s = Writer.init(w.buf.allocator, w.endian);"
        );
        let _ = writeln!(out, "        defer body_s.deinit();");
        let _ = writeln!(out, "        const body = &body_s;");
        for f in &fields {
            let emh = 0x4000_0000_u32 | f.id;
            let _ = writeln!(out, "        try body.putU32(0x{emh:08x});");
            let _ = writeln!(out, "        {{");
            let _ = writeln!(
                out,
                "            var zdMem_s = Writer.init(w.buf.allocator, w.endian);"
            );
            let _ = writeln!(out, "            defer zdMem_s.deinit();");
            let _ = writeln!(out, "            const zdMem = &zdMem_s;");
            let _ = writeln!(out, "            {}", f.put.replace("$w", "zdMem"));
            let _ = writeln!(
                out,
                "            try body.putU32(@intCast(zdMem.bytes().len));"
            );
            let _ = writeln!(out, "            try body.putBytes(zdMem.bytes());");
            let _ = writeln!(out, "        }}");
        }
        let _ = writeln!(out, "        try w.putU32(@intCast(body.bytes().len));");
        let _ = writeln!(out, "        try w.putBytes(body.bytes());");
    } else {
        let wv = if ext == ExtensibilityKind::Final {
            "w"
        } else {
            let _ = writeln!(
                out,
                "        var body_s = Writer.init(w.buf.allocator, w.endian);"
            );
            let _ = writeln!(out, "        defer body_s.deinit();");
            let _ = writeln!(out, "        const body = &body_s;");
            "body"
        };
        for f in &fields {
            let _ = writeln!(out, "        {}", f.put.replace("$w", wv));
        }
        if ext != ExtensibilityKind::Final {
            let _ = writeln!(out, "        try w.putU32(@intCast(body.bytes().len));");
            let _ = writeln!(out, "        try w.putBytes(body.bytes());");
        }
    }
    let _ = writeln!(out, "    }}");

    let _ = writeln!(
        out,
        "\n    pub fn marshalXCDR(self: {ty}, endian: Endian, alloc: std.mem.Allocator) ![]u8 {{"
    );
    let _ = writeln!(out, "        var w = Writer.init(alloc, endian);");
    let _ = writeln!(out, "        errdefer w.deinit();");
    let _ = writeln!(out, "        try self.marshalInto(&w);");
    let _ = writeln!(out, "        return try w.buf.toOwnedSlice();");
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
        let _ = writeln!(
            out,
            "\n    pub fn keyHash(self: {ty}, alloc: std.mem.Allocator) ![16]u8 {{"
        );
        let _ = writeln!(out, "        var kw = Writer.init(alloc, .big);");
        let _ = writeln!(out, "        defer kw.deinit();");
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
                        &format!("self.{}", f.zig_name),
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
                        build_array_put(&f.zig_name, sizes, &elem_put)
                    }
                }
            } else {
                f.put.clone()
            };
            let _ = writeln!(out, "        {}", put.replace("$w", "kw"));
        }
        let _ = writeln!(out, "        const b = kw.bytes();");
        if use_md5 {
            // KeyHolder max size > 16 → MD5(bytes)[0..16] (XTypes §7.6.8.4).
            let _ = writeln!(out, "        var outk: [16]u8 = undefined;");
            let _ = writeln!(out, "        std.crypto.hash.Md5.hash(b, &outk, .{{}});");
            let _ = writeln!(out, "        return outk;");
        } else {
            let _ = writeln!(out, "        var outk = [_]u8{{0}} ** 16;");
            let _ = writeln!(out, "        const n = @min(16, b.len);");
            let _ = writeln!(out, "        @memcpy(outk[0..n], b[0..n]);");
            let _ = writeln!(out, "        return outk;");
        }
        let _ = writeln!(out, "    }}");
    }

    // Decode (inverse of marshalInto). Fills `v: {ty}` in place; strings/seqs/
    // maps allocate via the reader's allocator (so reads use `try`). @final reads
    // inline, @appendable skips the DHEADER, @mutable skips DHEADER then per member
    // EMHEADER + NEXTINT (members in declaration order).
    let _ = writeln!(out, "\n    pub fn readFrom(r: *Reader) !{ty} {{");
    let _ = writeln!(out, "        var v: {ty} = undefined;");
    if ext == ExtensibilityKind::Mutable {
        let _ = writeln!(out, "        _ = r.getU32();");
        for f in &fields {
            let _ = writeln!(out, "        _ = r.getU32();");
            let _ = writeln!(out, "        _ = r.getU32();");
            let _ = writeln!(out, "        {}", f.get.replace("$r", "r"));
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "        _ = r.getU32();");
        }
        for f in &fields {
            let _ = writeln!(out, "        {}", f.get.replace("$r", "r"));
        }
    }
    let _ = writeln!(out, "        return v;");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(
        out,
        "\n    pub fn unmarshalXCDR(buf: []const u8, endian: Endian, alloc: std.mem.Allocator) !{ty} {{"
    );
    let _ = writeln!(out, "        var r = Reader.init(buf, endian, alloc);");
    let _ = writeln!(out, "        return try {ty}.readFrom(&r);");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "}};");
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
        return Err(IdlZigError::Unsupported(format!(
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
        "v.disc",
        enum_names,
        struct_names,
    )?;
    let mut cases: Vec<UnionCase> = Vec::new();
    for c in &u.cases {
        let field = escape_zig_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ty, put) = map_type(
            &resolved,
            &format!("self.{field}"),
            enum_names,
            struct_names,
        )?;
        let get = map_get(&resolved, &format!("v.{field}"), enum_names, struct_names)?;
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => labels.push(array_size(e).ok_or_else(|| {
                    IdlZigError::Unsupported(format!(
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

    let ty = escape_zig_ident(&u.name.text);
    let _ = writeln!(out, "\npub const {ty} = struct {{");
    let _ = writeln!(out, "    disc: {disc_type},");
    for c in &cases {
        let _ = writeln!(out, "    {}: {},", c.field, c.ty);
    }
    let _ = writeln!(
        out,
        "\n    pub fn marshalInto(self: {ty}, w: *Writer) !void {{"
    );
    let wv = if ext == ExtensibilityKind::Final {
        "w"
    } else {
        let _ = writeln!(
            out,
            "        var body_s = Writer.init(w.buf.allocator, w.endian);"
        );
        let _ = writeln!(out, "        defer body_s.deinit();");
        let _ = writeln!(out, "        const body = &body_s;");
        "body"
    };
    let _ = writeln!(out, "        {}", disc_put.replace("$w", wv));
    let _ = writeln!(out, "        switch (self.disc) {{");
    for c in &cases {
        // Block form (`=> { put; }`): the generated put already ends in `;`.
        if c.is_default {
            let _ = writeln!(
                out,
                "            else => {{ {} }},",
                c.put.replace("$w", wv)
            );
        } else {
            let lbl = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                out,
                "            {lbl} => {{ {} }},",
                c.put.replace("$w", wv)
            );
        }
    }
    if !has_default {
        let _ = writeln!(out, "            else => {{}},");
    }
    let _ = writeln!(out, "        }}");
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "        try w.putU32(@intCast(body.bytes().len));");
        let _ = writeln!(out, "        try w.putBytes(body.bytes());");
    }
    let _ = writeln!(out, "    }}");
    let _ = writeln!(
        out,
        "\n    pub fn marshalXCDR(self: {ty}, endian: Endian, alloc: std.mem.Allocator) ![]u8 {{"
    );
    let _ = writeln!(out, "        var w = Writer.init(alloc, endian);");
    let _ = writeln!(out, "        errdefer w.deinit();");
    let _ = writeln!(out, "        try self.marshalInto(&w);");
    let _ = writeln!(out, "        return try w.buf.toOwnedSlice();");
    let _ = writeln!(out, "    }}");

    // Decode: read the discriminator, then read only the selected member
    // (@appendable skips the leading DHEADER). Unread members stay undefined.
    let _ = writeln!(out, "\n    pub fn readFrom(r: *Reader) !{ty} {{");
    let _ = writeln!(out, "        var v: {ty} = undefined;");
    if ext == ExtensibilityKind::Appendable {
        let _ = writeln!(out, "        _ = r.getU32();");
    }
    let _ = writeln!(out, "        {}", disc_get.replace("$r", "r"));
    let _ = writeln!(out, "        switch (v.disc) {{");
    for c in &cases {
        if c.is_default {
            let _ = writeln!(
                out,
                "            else => {{ {} }},",
                c.get.replace("$r", "r")
            );
        } else {
            let lbl = c
                .labels
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                out,
                "            {lbl} => {{ {} }},",
                c.get.replace("$r", "r")
            );
        }
    }
    if !has_default {
        let _ = writeln!(out, "            else => {{}},");
    }
    let _ = writeln!(out, "        }}");
    let _ = writeln!(out, "        return v;");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(
        out,
        "\n    pub fn unmarshalXCDR(buf: []const u8, endian: Endian, alloc: std.mem.Allocator) !{ty} {{"
    );
    let _ = writeln!(out, "        var r = Reader.init(buf, endian, alloc);");
    let _ = writeln!(out, "        return try {ty}.readFrom(&r);");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "}};");
    Ok(())
}

/// Maps an IDL type to `(Zig type, put statement)`. The put uses `$w` as the
/// writer placeholder and `expr` as the value expression.
fn map_type(
    t: &TypeSpec,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<(String, String)> {
    match t {
        TypeSpec::Primitive(p) => map_primitive(*p, expr),
        // Bounded `string<N>` / `wstring<N>` (XTypes 1.3 §7.4.3): reject an
        // over-bound value before it reaches the wire — the Zig backend has
        // no bound-aware string type (both map to `[]const u8`), so this is
        // the only place the IDL bound is ever enforced.
        TypeSpec::String(st) if !st.wide => Ok((
            "[]const u8".to_string(),
            match &st.bound {
                Some(b) => {
                    let bv = array_size(b).unwrap_or(i64::MAX);
                    format!(
                        "{{ if ({expr}.len > {bv}) return error.BoundExceeded; try $w.putString({expr}); }}"
                    )
                }
                None => format!("try $w.putString({expr});"),
            },
        )),
        // Moderate fix (deep review of #22 decode-bounds-cross-backend): the
        // bound is in UTF-16 code units — count them via `wstringUnitLen`
        // (surrogate-pair aware), NOT `{expr}.len` (the UTF-8 BYTE length of
        // the `[]const u8` value, which diverges from the unit count for
        // any non-ASCII text).
        TypeSpec::String(st) => Ok((
            "[]const u8".to_string(),
            match &st.bound {
                Some(b) => {
                    let bv = array_size(b).unwrap_or(i64::MAX);
                    format!(
                        "{{ if (try wstringUnitLen({expr}) > {bv}) return error.BoundExceeded; try $w.putWString({expr}); }}"
                    )
                }
                None => format!("try $w.putWString({expr});"),
            },
        )),
        TypeSpec::Sequence(seq) => map_sequence(&seq.elem, seq.bound.as_ref(), expr, struct_names),
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                Ok((
                    escape_zig_ident(&name),
                    format!("try $w.putU32(@bitCast(@intFromEnum({expr})));"),
                ))
            } else if struct_names.contains(&name) {
                Ok((
                    escape_zig_ident(&name),
                    format!("try {expr}.marshalInto($w);"),
                ))
            } else {
                Err(IdlZigError::Unsupported(format!("scoped type {name}")))
            }
        }
        other => Err(IdlZigError::Unsupported(format!("type {other:?}"))),
    }
}

/// Builds a KeyHash-writer statement (using the `$w` placeholder like
/// `map_type`'s put strings; each statement already ends in `;`, matching
/// `map_type`'s convention) for one `@key` member value. Reuses the shared
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
                        return Err(IdlZigError::Unsupported(
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
            return Ok(stmts.join(" "));
        }
    }
    let (_, put) = map_type(&resolved, expr, enum_names, struct_names)?;
    Ok(put)
}

fn map_primitive(p: PrimitiveType, expr: &str) -> Result<(String, String)> {
    let (ty, put) = match p {
        PrimitiveType::Octet => ("u8", format!("try $w.putU8({expr});")),
        PrimitiveType::Boolean => ("bool", format!("try $w.putBool({expr});")),
        PrimitiveType::Char => ("u8", format!("try $w.putU8({expr});")),
        PrimitiveType::Integer(i) => return map_integer(i, expr),
        PrimitiveType::Floating(FloatingType::Float) => ("f32", format!("try $w.putF32({expr});")),
        PrimitiveType::Floating(FloatingType::Double) => ("f64", format!("try $w.putF64({expr});")),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            ("f64", format!("try $w.putLongDouble({expr});"))
        }
        PrimitiveType::WideChar => ("u32", format!("try $w.putU32({expr});")),
    };
    Ok((ty.to_string(), put))
}

fn map_integer(i: IntegerType, expr: &str) -> Result<(String, String)> {
    // Signed IDL integers reinterpret to the unsigned wire via @bitCast.
    let (ty, put) = match i {
        IntegerType::UInt8 => ("u8", format!("try $w.putU8({expr});")),
        IntegerType::Int8 => ("i8", format!("try $w.putU8(@bitCast({expr}));")),
        IntegerType::UShort | IntegerType::UInt16 => ("u16", format!("try $w.putU16({expr});")),
        IntegerType::Short | IntegerType::Int16 => {
            ("i16", format!("try $w.putU16(@bitCast({expr}));"))
        }
        IntegerType::ULong | IntegerType::UInt32 => ("u32", format!("try $w.putU32({expr});")),
        IntegerType::Long | IntegerType::Int32 => {
            ("i32", format!("try $w.putU32(@bitCast({expr}));"))
        }
        IntegerType::ULongLong | IntegerType::UInt64 => ("u64", format!("try $w.putU64({expr});")),
        IntegerType::LongLong | IntegerType::Int64 => {
            ("i64", format!("try $w.putU64(@bitCast({expr}));"))
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
    // Bounded `sequence<T, N>` (XTypes 1.3 §7.4.3): reject an over-bound
    // value before it reaches the wire.
    let bound_check = bound
        .map(|b| {
            let bv = array_size(b).unwrap_or(i64::MAX);
            format!("if ({expr}.len > {bv}) return error.BoundExceeded; ")
        })
        .unwrap_or_default();
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok((
            "[]const u8".to_string(),
            format!("{{ {bound_check}try $w.putSeqU8({expr}); }}"),
        ));
    }
    // sequence<struct> → collection DHEADER + count + each element.
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            let put = format!(
                "{{ {bound_check}var subw = Writer.init($w.buf.allocator, $w.endian); defer subw.deinit();                  const sub = &subw; try sub.putU32(@intCast({expr}.len));                  for ({expr}) |elem| try elem.marshalInto(sub);                  try $w.putU32(@intCast(sub.bytes().len)); try $w.putBytes(sub.bytes()); }}"
            );
            return Ok((format!("[]const {}", escape_zig_ident(&name)), put));
        }
    }
    Err(IdlZigError::Unsupported(
        "sequence of non-octet elements".to_string(),
    ))
}

// ---- decode (inverse of the put path): a `Reader` wire-core (allocator-backed)
// in the prelude, plus `map_get` — the inverse of `map_type` — emitting a
// statement that reads one value from `r` into the lvalue `target`. Reads that
// allocate use `try`, so the read fns return `!Ty`. Roundtrip-verified.

/// Reads a fixed array: nested row-major `while` loops assigning into the value
/// array `{target}[zdi0][zdi1]…` (inverse of [`build_array_put`]).
fn build_array_get(target: &str, sizes: &[i64], elem_get: &str) -> String {
    let idx: String = (0..sizes.len()).map(|k| format!("[zdi{k}]")).collect();
    let mut body = elem_get.replace("$L", &format!("{target}{idx}"));
    for k in (0..sizes.len()).rev() {
        body = format!(
            "{{ var zdi{k}: usize = 0; while (zdi{k} < {}) : (zdi{k} += 1) {{ {body} }} }}",
            sizes[k]
        );
    }
    body
}

/// Emits a statement reading one value of IDL type `t` from `r` into `target`.
fn map_get(
    t: &TypeSpec,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
) -> Result<String> {
    match t {
        TypeSpec::Primitive(p) => map_get_primitive(*p, target),
        // B1 follow-up (#22 decode-side parity): mirror the encode-side
        // bound check — XTypes 1.3 §7.4.3 requires the IDL bound enforced
        // on decode too, not just whatever the wire happens to contain.
        //
        // Documented tradeoff (moderate item, deep review of #22): this
        // checks AFTER `r.getString`/`r.getWString` has already materialized
        // the value, not before — the same order idl-rust's own reference
        // decoder uses (`struct_emit.rs::emit_decode_bound_checks`: checked
        // post-decode since the value already exists by then). A single
        // primitive-collection read's cost is bounded by the wire's own
        // remaining buffer, not by the attacker-declared length, so there is
        // no separate amplification to guard against by checking earlier.
        // Contrast the struct-element branch in `map_get_sequence` below,
        // which checks BEFORE its per-element decode loop runs.
        TypeSpec::String(st) if !st.wide => Ok(match &st.bound {
            Some(b) => {
                let bv = array_size(b).unwrap_or(i64::MAX);
                format!(
                    "{{ const zdS = try r.getString(); if (zdS.len > {bv}) return error.BoundExceeded; {target} = zdS; }}"
                )
            }
            None => format!("{target} = try r.getString();"),
        }),
        // Moderate fix: count true UTF-16 units via `wstringUnitLen`, NOT
        // `zdS.len` (the decoded UTF-8 BYTE length) — see the encode-side
        // comment above and `wstringUnitLen`'s doc comment in WIRE_PRELUDE.
        TypeSpec::String(st) => Ok(match &st.bound {
            Some(b) => {
                let bv = array_size(b).unwrap_or(i64::MAX);
                format!(
                    "{{ const zdS = try r.getWString(); if (try wstringUnitLen(zdS) > {bv}) return error.BoundExceeded; {target} = zdS; }}"
                )
            }
            None => format!("{target} = try r.getWString();"),
        }),
        TypeSpec::Sequence(seq) => {
            map_get_sequence(&seq.elem, seq.bound.as_ref(), target, struct_names)
        }
        TypeSpec::Scoped(sn) => {
            let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
            if enum_names.contains(&name) {
                Ok(format!(
                    "{target} = @enumFromInt(@as(i32, @bitCast(r.getU32())));"
                ))
            } else if struct_names.contains(&name) {
                Ok(format!(
                    "{target} = try {}.readFrom(r);",
                    escape_zig_ident(&name)
                ))
            } else {
                Err(IdlZigError::Unsupported(format!("scoped type {name}")))
            }
        }
        other => Err(IdlZigError::Unsupported(format!("type {other:?}"))),
    }
}

fn map_get_primitive(p: PrimitiveType, target: &str) -> Result<String> {
    let s = match p {
        PrimitiveType::Octet | PrimitiveType::Char => format!("{target} = r.getU8();"),
        PrimitiveType::Boolean => format!("{target} = r.getBool();"),
        PrimitiveType::Integer(i) => return map_get_integer(i, target),
        PrimitiveType::Floating(FloatingType::Float) => format!("{target} = r.getF32();"),
        PrimitiveType::Floating(FloatingType::Double) => format!("{target} = r.getF64();"),
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            format!("{target} = r.getLongDouble();")
        }
        PrimitiveType::WideChar => format!("{target} = r.getU32();"),
    };
    Ok(s)
}

fn map_get_integer(i: IntegerType, target: &str) -> Result<String> {
    let s = match i {
        IntegerType::UInt8 => format!("{target} = r.getU8();"),
        IntegerType::Int8 => format!("{target} = @bitCast(r.getU8());"),
        IntegerType::UShort | IntegerType::UInt16 => format!("{target} = r.getU16();"),
        IntegerType::Short | IntegerType::Int16 => format!("{target} = @bitCast(r.getU16());"),
        IntegerType::ULong | IntegerType::UInt32 => format!("{target} = r.getU32();"),
        IntegerType::Long | IntegerType::Int32 => format!("{target} = @bitCast(r.getU32());"),
        IntegerType::ULongLong | IntegerType::UInt64 => format!("{target} = r.getU64();"),
        IntegerType::LongLong | IntegerType::Int64 => format!("{target} = @bitCast(r.getU64());"),
    };
    Ok(s)
}

fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    target: &str,
    struct_names: &HashSet<String>,
) -> Result<String> {
    let bv = bound.map(|b| array_size(b).unwrap_or(i64::MAX));
    // Documented tradeoff (moderate item, deep review of #22): this octet
    // sequence checks AFTER `r.getSeqU8` has already allocated+copied the
    // bytes — a single bounded-by-remaining-buffer read, not a decode loop
    // (see the narrow string/wstring comment in `map_get` above for why that
    // is fine). The struct-element branch below is different — checked
    // BEFORE its per-element decode loop / `r.alloc.alloc` call.
    if let TypeSpec::Primitive(PrimitiveType::Octet | PrimitiveType::Integer(IntegerType::UInt8)) =
        elem
    {
        return Ok(match bv {
            Some(bv) => format!(
                "{{ const zdS = try r.getSeqU8(); if (zdS.len > {bv}) return error.BoundExceeded; {target} = zdS; }}"
            ),
            None => format!("{target} = try r.getSeqU8();"),
        });
    }
    if let TypeSpec::Scoped(sn) = elem {
        let name = sn.parts.last().map(|p| p.text.clone()).unwrap_or_default();
        if struct_names.contains(&name) {
            // Checked BEFORE `r.alloc.alloc({esc}, zdN)`/the decode loop —
            // an attacker-supplied huge `zdN` must not drive an oversized
            // allocation or decode loop before the bound is ever checked.
            let bound_check = bv
                .map(|bv| format!("if (zdN > {bv}) return error.BoundExceeded; "))
                .unwrap_or_default();
            let esc = escape_zig_ident(&name);
            return Ok(format!(
                "{{ _ = r.getU32(); const zdN = r.getU32(); {bound_check}const zdList = try r.alloc.alloc({esc}, zdN); var zdI: usize = 0; while (zdI < zdN) : (zdI += 1) zdList[zdI] = try {esc}.readFrom(r); {target} = zdList; }}"
            ));
        }
    }
    Err(IdlZigError::Unsupported(
        "sequence of non-octet elements".to_string(),
    ))
}
