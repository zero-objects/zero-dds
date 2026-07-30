// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL4 → Zig emitter. Walks the `zerodds-idl` AST and emits a self-contained
//! Zig source file: a shared XCDR2 `Writer` (byte-identical to `endpoints/zig`)
//! plus, per IDL `struct`, a Zig struct with a `marshalXCDR(endian, allocator)`
//! method. `@final` and `@appendable` are supported; other extensibilities and
//! constructs raise [`IdlZigError::Unsupported`].

use std::cell::Cell;
use std::fmt::Write as _;

use std::collections::{HashMap, HashSet};

use zerodds_idl::ast::types::{
    Annotation, BitValue, BitmaskDecl, BitsetDecl, CaseLabel, ConstDecl, ConstExpr, ConstType,
    ConstrTypeDecl, Declarator, Definition, EnumDef, Export, FloatingType, IntegerType,
    InterfaceDcl, Literal, LiteralKind, Member, PrimitiveType, ScopedName, SequenceType,
    Specification, StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnaryOp, UnionDcl,
    UnionDef,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, PlacementKind, enum_bit_bound, enum_wire_octets,
    lower_annotations, lower_single,
};

use crate::error::{IdlZigError, Result};
use crate::keywords::escape_zig_ident;

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

    /// Monotonic counter handing out per-run-unique numeric suffixes for the
    /// temporaries a nested `map<K, map<…>>` value arm emits (#A28), so an
    /// inner map's loop/scratch variables never shadow an outer map's (Zig
    /// forbids local shadowing). Reset at the start of [`generate_zig_module`].
    static FRESH: Cell<u64> = const { Cell::new(0) };

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

/// Returns a per-run-unique id used to suffix nested-map temporaries (#A28).
fn fresh_id() -> u64 {
    FRESH.with(|c| {
        let v = c.get();
        c.set(v + 1);
        v
    })
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
/// single Zig identifier (#A35). Each segment's own underscores are doubled
/// and the segments joined by a single underscore, so `module A_B { struct C }`
/// (→ `A__B_C`) and `module A { module B { struct C } }` (→ `A_B_C`) can no
/// longer collide. A single (unqualified) segment passes through untouched, so
/// every existing global-scope golden is unchanged.
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

/// Registers the fully-qualified path of a single `TypeDecl` (module-level or
/// interface-nested).
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
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(m)) => push_type_path(scope, &m.name.text),
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

/// Options for the Zig backend.
#[derive(Debug, Clone, Default)]
pub struct ZigGenOptions {}

/// Zig codegen language aliases for `@verbatim(language="...")`. The spec
/// wildcard `"*"` always matches (handled by `verbatims_for_language`).
const ZIG_LANG_ALIASES: &[&str] = &["zig"];

/// Emits every `@verbatim` block from `anns` whose language matches the Zig
/// codegen and whose placement equals `placement`, each line prefixed with
/// `indent` (XTypes 1.3 §7.2.2.4.8 / IDL 4.2 §8.3.5.1). Unparseable
/// annotation lists are silently skipped (no wire impact).
fn emit_verbatim_at(out: &mut String, indent: &str, anns: &[Annotation], placement: PlacementKind) {
    let Ok(lowered) = lower_annotations(anns) else {
        return;
    };
    for v in lowered.verbatims_for_language(ZIG_LANG_ALIASES) {
        if v.placement != placement {
            continue;
        }
        for line in v.text.lines() {
            let _ = writeln!(out, "{indent}{line}");
        }
    }
}

/// Returns the annotation list carried by a top-level declaration (for
/// `@verbatim` placement). Non-annotatable / unsupported variants yield `&[]`.
fn def_annotations(d: &Definition) -> &[Annotation] {
    match d {
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
            &s.annotations
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
            &u.annotations
        }
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => &e.annotations,
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => &b.annotations,
        Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(m))) => &m.annotations,
        Definition::Type(TypeDecl::Typedef(t)) => &t.annotations,
        Definition::Const(c) => &c.annotations,
        _ => &[],
    }
}

/// Backing-integer Zig type and its byte width for a bitset/bitmask holder of
/// `total_bits` bits (XTypes 1.3 §7.4.7 / §7.3.1.2.1.1): ≤8 → u8, ≤16 → u16,
/// ≤32 → u32, else u64.
fn bitset_storage(total_bits: usize) -> (&'static str, u32) {
    match total_bits {
        0..=8 => ("u8", 1),
        9..=16 => ("u16", 2),
        17..=32 => ("u32", 4),
        _ => ("u64", 8),
    }
}

/// `Writer`/`Reader` method suffix for a backing integer of `width` bytes.
fn storage_put(width: u32) -> &'static str {
    match width {
        1 => "putU8",
        2 => "putU16",
        4 => "putU32",
        _ => "putU64",
    }
}
fn storage_get(width: u32) -> &'static str {
    match width {
        1 => "getU8",
        2 => "getU16",
        4 => "getU32",
        _ => "getU64",
    }
}

/// `@bit_bound(n)` holder width for a bitmask, default 32 (XTypes §7.3.1.2.1.1).
fn bitmask_bit_bound(anns: &[Annotation]) -> u16 {
    anns.iter()
        .find_map(|a| match lower_single(a) {
            Ok(Some(BuiltinAnnotation::BitBound(n))) => Some(n),
            _ => None,
        })
        .unwrap_or(32)
}

/// Explicit `@position(n)` on a bitmask value, if present.
fn bitvalue_position(v: &BitValue) -> Option<u32> {
    v.annotations.iter().find_map(|a| match lower_single(a) {
        Ok(Some(BuiltinAnnotation::Position(n))) => Some(n),
        _ => None,
    })
}

/// Emits an IDL `bitset` as a Zig struct wrapping a packed backing integer
/// (XTypes 1.3 §7.4.7). Per named bitfield: a getter (single-bit → `bool`,
/// multi-bit → the backing int) and a `set_*` mutator. The wire form is the
/// backing integer (`marshalInto` writes it, `readFrom` reads it), so a bitset
/// member serializes exactly like the storage int on every vendor.
fn emit_bitset(out: &mut String, b: &BitsetDecl, scope: &[String]) {
    let total: usize = b
        .bitfields
        .iter()
        .map(|bf| array_size(&bf.spec.width).unwrap_or(0).max(0) as usize)
        .sum();
    let (sty, width) = bitset_storage(total);
    let ty = escape_zig_ident(&qualify(scope, &b.name.text));
    let _ = writeln!(out, "\npub const {ty} = struct {{");
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::BeginDeclaration);
    let _ = writeln!(out, "    storage: {sty} = 0,");
    let mut offset: usize = 0;
    for bf in &b.bitfields {
        let w = array_size(&bf.spec.width).unwrap_or(0).max(0) as usize;
        if let Some(name) = &bf.name {
            let getter = escape_zig_ident(&name.text);
            let setter = escape_zig_ident(&format!("set_{}", name.text));
            if w == 1 {
                let _ = writeln!(
                    out,
                    "    pub fn {getter}(self: {ty}) bool {{ return ((self.storage >> {offset}) & 1) != 0; }}"
                );
                let _ = writeln!(
                    out,
                    "    pub fn {setter}(self: *{ty}, value: bool) void {{ const zdMask: {sty} = @as({sty}, 1) << {offset}; if (value) {{ self.storage |= zdMask; }} else {{ self.storage &= ~zdMask; }} }}"
                );
            } else {
                let mask: u128 = if w >= 128 {
                    u128::MAX
                } else {
                    (1u128 << w) - 1
                };
                let _ = writeln!(
                    out,
                    "    pub fn {getter}(self: {ty}) {sty} {{ return (self.storage >> {offset}) & @as({sty}, {mask}); }}"
                );
                let _ = writeln!(
                    out,
                    "    pub fn {setter}(self: *{ty}, value: {sty}) void {{ const zdMask: {sty} = @as({sty}, {mask}) << {offset}; self.storage = (self.storage & ~zdMask) | ((value & @as({sty}, {mask})) << {offset}); }}"
                );
            }
        }
        offset += w;
    }
    emit_holder_wire(out, &ty, width);
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}};");
}

/// Emits an IDL `bitmask` as a Zig struct wrapping a backing integer whose
/// width is the `@bit_bound` (default 32, XTypes §7.3.1.2.1.1 — NOT the count
/// of declared bits). Each value becomes an OR-able `const` at its bit
/// position (explicit `@position` or declaration index). Wire = backing int.
fn emit_bitmask(out: &mut String, m: &BitmaskDecl, scope: &[String]) {
    let (sty, width) = bitset_storage(bitmask_bit_bound(&m.annotations) as usize);
    let ty = escape_zig_ident(&qualify(scope, &m.name.text));
    let _ = writeln!(out, "\npub const {ty} = struct {{");
    emit_verbatim_at(out, "    ", &m.annotations, PlacementKind::BeginDeclaration);
    let _ = writeln!(out, "    storage: {sty} = 0,");
    for (idx, val) in m.values.iter().enumerate() {
        let pos = bitvalue_position(val).unwrap_or(idx as u32);
        let cname = escape_zig_ident(&val.name.text.to_uppercase());
        let _ = writeln!(
            out,
            "    pub const {cname}: {ty} = .{{ .storage = @as({sty}, 1) << {pos} }};"
        );
    }
    let _ = writeln!(
        out,
        "    pub fn bits(self: {ty}) {sty} {{ return self.storage; }}"
    );
    let _ = writeln!(
        out,
        "    pub fn contains(self: {ty}, other: {ty}) bool {{ return (self.storage & other.storage) == other.storage; }}"
    );
    emit_holder_wire(out, &ty, width);
    emit_verbatim_at(out, "    ", &m.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}};");
}

/// Emits the shared `marshalInto`/`marshalXCDR`/`readFrom`/`unmarshalXCDR`
/// wire methods for a bitset/bitmask holder over backing integer `sty`.
fn emit_holder_wire(out: &mut String, ty: &str, width: u32) {
    let put = storage_put(width);
    let get = storage_get(width);
    let _ = writeln!(
        out,
        "    pub fn marshalInto(self: {ty}, w: *Writer) !void {{ try w.{put}(self.storage); }}"
    );
    let _ = writeln!(
        out,
        "    pub fn marshalXCDR(self: {ty}, endian: Endian, alloc: std.mem.Allocator) ![]u8 {{ var w = Writer.init(alloc, endian); errdefer w.deinit(); try self.marshalInto(&w); return try w.buf.toOwnedSlice(); }}"
    );
    let _ = writeln!(
        out,
        "    pub fn readFrom(r: *Reader) !{ty} {{ return .{{ .storage = r.{get}() }}; }}"
    );
    let _ = writeln!(
        out,
        "    pub fn unmarshalXCDR(buf: []const u8, endian: Endian, alloc: std.mem.Allocator) !{ty} {{ var r = Reader.init(buf, endian, alloc); return try {ty}.readFrom(&r); }}"
    );
}

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
    pub fn putFixed(self: *Writer, dec: []const u8, comptime zdP: usize, comptime zdS: usize) !void {
        var positive = true;
        var text = dec;
        if (text.len > 0 and text[0] == '-') {
            positive = false;
            text = text[1..];
        } else if (text.len > 0 and text[0] == '+') {
            text = text[1..];
        }
        var int_part = text;
        var frac_part: []const u8 = &[_]u8{};
        if (std.mem.indexOfScalar(u8, text, '.')) |dot| {
            int_part = text[0..dot];
            frac_part = text[dot + 1 ..];
        }
        const int_needed = zdP - zdS;
        if (int_part.len > int_needed) return error.FixedOverflow;
        if (frac_part.len > zdS) return error.FixedOverflow;
        var nibbles: [zdP + 2]u8 = undefined;
        var n: usize = 0;
        if ((zdP + 1) % 2 == 1) {
            nibbles[n] = 0;
            n += 1;
        }
        var pad: usize = int_needed - int_part.len;
        while (pad > 0) : (pad -= 1) {
            nibbles[n] = 0;
            n += 1;
        }
        for (int_part) |c| {
            if (c < '0' or c > '9') return error.FixedInvalid;
            nibbles[n] = c - '0';
            n += 1;
        }
        for (frac_part) |c| {
            if (c < '0' or c > '9') return error.FixedInvalid;
            nibbles[n] = c - '0';
            n += 1;
        }
        var fpad: usize = zdS - frac_part.len;
        while (fpad > 0) : (fpad -= 1) {
            nibbles[n] = 0;
            n += 1;
        }
        nibbles[n] = if (positive) 0x0C else 0x0D;
        n += 1;
        var k: usize = 0;
        while (k < n) : (k += 2) {
            try self.buf.append((nibbles[k] << 4) | nibbles[k + 1]);
        }
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
    pub fn getFixed(self: *Reader, comptime zdP: usize, comptime zdS: usize) ![]const u8 {
        const nbytes = (zdP + 2) / 2;
        const raw = self.getBytesN(nbytes);
        var nibs: [nbytes * 2]u8 = undefined;
        var ni: usize = 0;
        for (raw) |b| {
            nibs[ni] = (b >> 4) & 0x0F;
            nibs[ni + 1] = b & 0x0F;
            ni += 2;
        }
        const has_pad = (zdP + 1) % 2 == 1;
        const start: usize = if (has_pad) 1 else 0;
        const negative = nibs[start + zdP] == 0x0D;
        var out = std.ArrayList(u8).init(self.alloc);
        errdefer out.deinit();
        if (negative) try out.append('-');
        const int_digits = zdP - zdS;
        var i: usize = 0;
        while (i < zdP) : (i += 1) {
            if (zdS > 0 and i == int_digits) try out.append('.');
            try out.append(@as(u8, '0') +% nibs[start + i]);
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
/// yet emit (e.g. `@mutable` unions and non-literal array/sequence bounds).
pub fn generate_zig_module(spec: &Specification, _opts: &ZigGenOptions) -> Result<String> {
    // Fresh nested-map temporaries start numbering from zero each run (#A28).
    FRESH.with(|c| c.set(0));

    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Code generated by zerodds-idlc (Zig backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "// SPDX-License-Identifier: Apache-2.0");
    out.push_str(WIRE_PRELUDE);

    // Register every named type's fully-qualified path so reference sites can
    // resolve a `ScopedName` against its enclosing scope (#21 cross-module;
    // #A39 interface-nested types are registered under the interface scope).
    TYPE_PATHS.with(|t| t.borrow_mut().clear());
    register_type_paths(&spec.definitions, &mut Vec::new());

    // `module X { ... }` content is promoted to the top level, each definition
    // paired with its module scope path (see `flatten_module_defs`).
    let flat = flatten_module_defs(&spec.definitions);
    // #A39: interface bodies are not silently discarded — their nested
    // `Export::Type` declarations are promoted to top-level types under the
    // interface's own scope segment.
    let iface_types = flatten_iface_types(&spec.definitions);

    // Named-type sets drive reference resolution; they must span both the
    // module-level defs and the promoted interface-nested types.
    let mut enum_names: HashSet<String> = HashSet::new();
    let mut struct_names: HashSet<String> = HashSet::new();
    for (scope, d) in &flat {
        if let Definition::Type(td) = d {
            collect_type_names(td, scope, &mut enum_names, &mut struct_names);
        }
    }
    for (scope, td) in &iface_types {
        collect_type_names(td, scope, &mut enum_names, &mut struct_names);
    }

    let mut typedefs = collect_typedefs(spec);
    let mut struct_defs = collect_struct_defs(spec);
    let mut enum_defs = collect_enum_defs(spec);
    // Fold the interface-nested typedefs/structs/enums into the resolution maps
    // so a reference to an interface-scoped type resolves like any other (#A39).
    for (scope, td) in &iface_types {
        match td {
            TypeDecl::Typedef(t) => {
                for d in &t.declarators {
                    if let Declarator::Simple(name) = d {
                        typedefs.insert(qualify(scope, &name.text), t.type_spec.clone());
                    }
                }
            }
            TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
                struct_defs.insert(qualify(scope, &s.name.text), s);
            }
            TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => {
                enum_defs.insert(qualify(scope, &e.name.text), e);
            }
            _ => {}
        }
    }
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

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_FILE)` from every declaration,
    // emitted before any type output.
    for (_, def) in &flat {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::BeginFile);
    }

    for (scope, def) in &flat {
        // §7.2.2.4.8 — text placed directly before the annotated declaration.
        emit_verbatim_at(
            &mut out,
            "",
            def_annotations(def),
            PlacementKind::BeforeDeclaration,
        );
        match def {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                emit_enum(&mut out, e, scope);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                emit_struct(
                    &mut out,
                    s,
                    scope,
                    &enum_names,
                    &struct_names,
                    &typedefs,
                    &struct_defs,
                )?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                emit_union(
                    &mut out,
                    u,
                    scope,
                    &enum_names,
                    &struct_names,
                    &typedefs,
                    &enum_defs,
                )?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
                emit_bitset(&mut out, b, scope);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(m))) => {
                emit_bitmask(&mut out, m, scope);
            }
            // #A5/P1: a top-level `const` was silently dropped by the former
            // catch-all arm; emit it as a Zig `pub const`.
            Definition::Const(c) => emit_const(&mut out, c, scope),
            _ => {}
        }
        // §7.2.2.4.8 — text placed directly after the annotated declaration.
        emit_verbatim_at(
            &mut out,
            "",
            def_annotations(def),
            PlacementKind::AfterDeclaration,
        );
    }

    // #A39: interface-nested types, emitted after the module-level defs.
    for (scope, td) in &iface_types {
        emit_type_decl(
            &mut out,
            td,
            scope,
            &enum_names,
            &struct_names,
            &typedefs,
            &struct_defs,
            &enum_defs,
        )?;
    }

    // §7.2.2.4.8 — `@verbatim(placement=END_FILE)` from every declaration,
    // emitted after all type output.
    for (_, def) in &flat {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::EndFile);
    }
    Ok(out)
}

/// Emits a single `TypeDecl` (used for interface-nested types promoted to the
/// top level — #A39). Mirrors the module-level dispatch in
/// [`generate_zig_module`].
#[allow(clippy::too_many_arguments)]
fn emit_type_decl(
    out: &mut String,
    td: &TypeDecl,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    struct_defs: &HashMap<String, &StructDef>,
    enum_defs: &HashMap<String, &EnumDef>,
) -> Result<()> {
    match td {
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => emit_enum(out, e, scope),
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            emit_struct(
                out,
                s,
                scope,
                enum_names,
                struct_names,
                typedefs,
                struct_defs,
            )?;
        }
        TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u))) => {
            emit_union(out, u, scope, enum_names, struct_names, typedefs, enum_defs)?;
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => emit_bitset(out, b, scope),
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(m)) => emit_bitmask(out, m, scope),
        _ => {}
    }
    Ok(())
}

/// Collects the qualified names of the enum / struct-shaped (struct, bitset,
/// bitmask) types a `TypeDecl` declares, into the reference-resolution sets.
fn collect_type_names(
    td: &TypeDecl,
    scope: &[String],
    enum_names: &mut HashSet<String>,
    struct_names: &mut HashSet<String>,
) {
    match td {
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => {
            enum_names.insert(qualify(scope, &e.name.text));
        }
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            struct_names.insert(qualify(scope, &s.name.text));
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => {
            struct_names.insert(qualify(scope, &b.name.text));
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(m)) => {
            struct_names.insert(qualify(scope, &m.name.text));
        }
        _ => {}
    }
}

/// Recursively descends into `Definition::Interface` bodies, returning every
/// interface-nested `Export::Type` declaration paired with the scope path
/// `enclosing_module… + interface_name` (#A39). Zig has no nested-type
/// construct, so these are promoted to the top level under the interface's own
/// name segment (two interfaces in one module therefore never collide).
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

/// Renders an IDL `const` as a Zig `pub const NAME: TYPE = VALUE;` (#A5/P1).
/// Enum-/scoped-valued consts are skipped (their bare enumerator name cannot be
/// reconstructed without ambiguity, and a wrong identifier would break the
/// build) — the const is a codegen convenience with no wire effect.
fn emit_const(out: &mut String, c: &ConstDecl, scope: &[String]) {
    let Some(val) = const_expr_to_zig(&c.value) else {
        return;
    };
    let name = escape_zig_ident(&qualify(scope, &c.name.text));
    match const_zig_type(&c.type_) {
        Some(ty) => {
            let _ = writeln!(out, "\npub const {name}: {ty} = {val};");
        }
        None => {
            let _ = writeln!(out, "\npub const {name} = {val};");
        }
    }
}

/// Zig type for a `const` declaration (`None` = let Zig infer from the value).
fn const_zig_type(ct: &ConstType) -> Option<&'static str> {
    Some(match ct {
        ConstType::Integer(i) => zig_int_type(*i),
        ConstType::Floating(FloatingType::Float) => "f32",
        ConstType::Floating(FloatingType::Double | FloatingType::LongDouble) => "f64",
        ConstType::Char | ConstType::Octet => "u8",
        ConstType::WideChar => "u32",
        ConstType::Boolean => "bool",
        ConstType::String { .. } => "[]const u8",
        // A `fixed` const has no native Zig compile-time type; its decimal is
        // rendered as a string constant.
        ConstType::Fixed => "[]const u8",
        // An enum-typed / scoped const value cannot be reconstructed from the
        // bare enumerator name; infer (and the value renderer skips it anyway).
        ConstType::Scoped(_) => return None,
    })
}

/// The Zig integer type for an IDL integer type.
fn zig_int_type(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Int8 => "i8",
        IntegerType::UInt8 => "u8",
        IntegerType::Short | IntegerType::Int16 => "i16",
        IntegerType::UShort | IntegerType::UInt16 => "u16",
        IntegerType::Long | IntegerType::Int32 => "i32",
        IntegerType::ULong | IntegerType::UInt32 => "u32",
        IntegerType::LongLong | IntegerType::Int64 => "i64",
        IntegerType::ULongLong | IntegerType::UInt64 => "u64",
    }
}

/// Renders a `ConstExpr` as a Zig constant expression, or `None` for a form the
/// Zig backend does not express (an enum-valued scoped reference).
/// zerodds-lint: recursion-depth 32 (const expression tree; bounded by the IDL
/// grammar's expression nesting).
fn const_expr_to_zig(e: &ConstExpr) -> Option<String> {
    match e {
        ConstExpr::Literal(l) => const_literal_to_zig(l),
        // An enum-valued or const-alias scoped reference cannot be rendered from
        // the bare last segment; skip (wire-neutral).
        ConstExpr::Scoped(_) => None,
        ConstExpr::Unary { op, operand, .. } => {
            let v = const_expr_to_zig(operand)?;
            let o = match op {
                UnaryOp::Plus => "+",
                UnaryOp::Minus => "-",
                UnaryOp::BitNot => "~",
            };
            Some(format!("{o}{v}"))
        }
        // Zig has no C-style compile-time expression grammar identical to IDL's
        // for every operator; a binary const expression is skipped rather than
        // risk emitting a non-compiling token sequence (wire-neutral).
        ConstExpr::Binary { .. } => None,
    }
}

/// Renders a single IDL literal as a Zig constant token.
fn const_literal_to_zig(l: &Literal) -> Option<String> {
    let raw = l.raw.trim();
    Some(match l.kind {
        // Re-emit integers in decimal so an IDL octal (`0755`) or hex is never
        // reinterpreted under Zig's own literal grammar; fall back to the raw
        // text only if it does not parse as a plain integer.
        LiteralKind::Integer => parse_int(raw).map_or_else(|| raw.to_string(), |v| v.to_string()),
        // Strip a trailing IDL float/fixed suffix (`d`/`f`/`l`) Zig rejects.
        LiteralKind::Floating => raw
            .trim_end_matches(['d', 'D', 'f', 'F', 'l', 'L'])
            .to_string(),
        // A `fixed` decimal has no native Zig type — render as a string.
        LiteralKind::Fixed => format!(
            "\"{}\"",
            raw.trim_end_matches(['d', 'D']).replace('"', "\\\"")
        ),
        // Normalize the IDL boolean keyword to Zig's `true`/`false` (never a
        // bare `TRUE`/`FALSE` token — #A13).
        LiteralKind::Boolean => {
            if raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // Narrow string/char literals pass through (Zig shares C escapes and
        // treats `'A'` as a comptime int assignable to `u8`); wide literals
        // drop the `L` prefix (`L"x"`/`L'x'` is not valid Zig).
        LiteralKind::String | LiteralKind::Char => raw.to_string(),
        LiteralKind::WideString | LiteralKind::WideChar => {
            raw.strip_prefix('L').unwrap_or(raw).to_string()
        }
    })
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
fn emit_enum(out: &mut String, e: &EnumDef, scope: &[String]) {
    let values = enumerator_values(e);
    let ty = escape_zig_ident(&qualify(scope, &e.name.text));
    let _ = writeln!(
        out,
        "
pub const {ty} = enum(i32) {{"
    );
    emit_verbatim_at(out, "    ", &e.annotations, PlacementKind::BeginDeclaration);
    for (en, value) in e.enumerators.iter().zip(&values) {
        let _ = writeln!(out, "    {} = {value},", escape_zig_ident(&en.name.text));
    }
    emit_verbatim_at(out, "    ", &e.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}};");
}

fn extensibility(s: &StructDef) -> ExtensibilityKind {
    lower_annotations(&s.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable)
}

/// Recursively descends into `Definition::Module`, returning every non-module
/// definition (struct/enum/union/typedef/…) paired with its module scope path,
/// in document order. A module's members are promoted to the top level; the
/// scope path is carried so the definition and reference sites flatten each name
/// to `scope_simple` ([`qualify`] / [`resolve_scoped_name`]). Two same-simple-name
/// types in different modules therefore become distinct types, not a collision
/// (#21).
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

/// Collects `struct` definitions as qualified-name -> `StructDef`, so a
/// nested-struct `@key` member can be expanded to its own `@key` subset
/// (Bug A) and `keyhash::uses_md5` can resolve a struct-typed `@key` member's
/// size instead of unconditionally forcing the MD5 branch (Bug B).
fn collect_struct_defs(spec: &Specification) -> HashMap<String, &StructDef> {
    let mut m = HashMap::new();
    for (scope, def) in flatten_module_defs(&spec.definitions) {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) = def {
            m.insert(qualify(&scope, &s.name.text), s);
        }
    }
    m
}

/// Collects `enum` definitions as qualified-name -> `EnumDef`, so a union whose
/// discriminator is an enum can resolve `case ENUMERATOR:` labels to their
/// integer discriminant (#A11/P4).
fn collect_enum_defs(spec: &Specification) -> HashMap<String, &EnumDef> {
    let mut m = HashMap::new();
    for (scope, def) in flatten_module_defs(&spec.definitions) {
        if let Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) = def {
            m.insert(qualify(&scope, &e.name.text), e);
        }
    }
    m
}

/// Collects a struct's effective members base-first (#A10/P3): the base
/// struct's members (recursively) precede the derived struct's own, so the
/// generated Zig type and its wire form carry the inherited fields — matching
/// cpp/csharp/java (`resolve_wire_members`) and the go backend. Without this a
/// `struct D : Base` dropped every inherited field from both the type and the
/// wire.
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

/// Evaluates a `char`/`wchar` literal (`'A'`, `L'x'`, `'\n'`) to its code point
/// (#A12). Used by the union label evaluator so a `case 'A':` resolves to 65.
fn char_literal_value(raw: &str) -> Option<i64> {
    let s = raw.trim().strip_prefix('L').unwrap_or(raw.trim());
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
/// enumerators (via `enum_vals`, name -> value of the switch enum), `char`
/// code points, and `TRUE`/`FALSE`.
/// zerodds-lint: recursion-depth 32 (label expression tree; bounded by the IDL
/// grammar's expression nesting).
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

/// A type is "primitive" for the map-DHEADER rule if it is fully descriptive on
/// the wire: an IDL primitive or an enum (i32). Others force a collection DHEADER.
fn is_primitive(t: &TypeSpec, enum_names: &HashSet<String>) -> bool {
    match t {
        TypeSpec::Primitive(_) => true,
        TypeSpec::Scoped(sn) => enum_names.contains(&resolve_scoped_name(sn)),
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
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    typedefs: &HashMap<String, TypeSpec>,
    struct_defs: &HashMap<String, &StructDef>,
) -> Result<()> {
    // Member references resolve against this struct's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = extensibility(s);

    struct FieldGen {
        zig_name: String,
        zig_type: String,
        put: String,
        get: String,
        id: u32,
        key: bool,
        must_understand: bool,
        optional: bool,
        // For an `@optional` member: the value-only put/get (no presence flag)
        // and the unwrapped value type, used by the `@mutable` path where a
        // present member is signalled by its EMHEADER rather than a body byte
        // (#A18). Empty for a non-optional member.
        opt_inner_put: String,
        opt_inner_get: String,
        opt_inner_type: String,
        resolved_type: TypeSpec,
        array_sizes: Option<Vec<i64>>,
        non_serialized: bool,
    }
    // KV structs for map members, emitted before the containing struct.
    let mut pre = String::new();
    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    // #A10/P3: base-first effective member list (inherited members precede the
    // derived struct's own), so inherited fields survive in both the type and
    // the wire.
    let mut effective_members: Vec<&Member> = Vec::new();
    collect_base_members(s, struct_defs, &mut effective_members);
    for m in effective_members {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        let optional = lowered.as_ref().is_some_and(|l| {
            l.builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::Optional))
        });
        // #A17: `@must_understand` sets EMHEADER bit 31 in the `@mutable` path.
        let must_understand = lowered.as_ref().is_some_and(|l| {
            l.builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::MustUnderstand))
        });
        // P0-5 (#2): a `@non_serialized` member keeps its Zig field but is off
        // the wire and does NOT consume a sequential id slot (ids compact).
        let non_serialized =
            zerodds_idl::semantics::annotations::member_is_non_serialized(&m.annotations);
        for d in &m.declarators {
            let raw_name = d.name().text.clone();
            let zig_name = escape_zig_ident(&raw_name);
            let id = if non_serialized {
                0
            } else {
                let assigned = explicit_id.unwrap_or(next_id);
                next_id = assigned + 1;
                assigned
            };
            let mut array_sizes: Option<Vec<i64>> = None;
            let mut opt_inner_put = String::new();
            let mut opt_inner_get = String::new();
            let mut opt_inner_type = String::new();
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
                // `@optional` member (XTypes 1.3 §7.4.3.5): a `?T` field, encoded
                // as a `uint8` presence flag (0/1) followed by the value when
                // present — byte-for-byte the `Option<T>` form the Rust backend
                // emits. `@optional` never co-occurs with an array declarator in
                // legal IDL, so it is handled only on the simple-declarator path.
                (_, Declarator::Simple(_)) if optional => {
                    let (t, base_put) = map_type(&resolved, "zdOptV", enum_names, struct_names)?;
                    let base_get = map_get(&resolved, "zdOptTmp", enum_names, struct_names)?;
                    let put = format!(
                        "if (self.{zig_name}) |zdOptV| {{ try $w.putU8(1); {base_put} }} else {{ try $w.putU8(0); }}"
                    );
                    let get = format!(
                        "{{ if (r.getU8() != 0) {{ var zdOptTmp: {t} = undefined; {base_get} v.{zig_name} = zdOptTmp; }} else {{ v.{zig_name} = null; }} }}"
                    );
                    // Preserve the value-only forms for the `@mutable` path (#A18).
                    opt_inner_put = base_put;
                    opt_inner_get = base_get;
                    opt_inner_type = t.clone();
                    (format!("?{t}"), put, get)
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
                must_understand,
                optional,
                opt_inner_put,
                opt_inner_get,
                opt_inner_type,
                resolved_type: resolved.clone(),
                array_sizes,
                non_serialized,
            });
        }
    }

    out.push_str(&pre);
    let ty = escape_zig_ident(&qualify(scope, &s.name.text));
    let _ = writeln!(out, "\npub const {ty} = struct {{");
    emit_verbatim_at(out, "    ", &s.annotations, PlacementKind::BeginDeclaration);
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
        // member id, must-understand bit 31 when @must_understand — #A17) +
        // NEXTINT (body length) + body (XTypes §7.4.3.4.2). An `@optional`
        // member is simply OMITTED from the member list when absent — the
        // missing EMHEADER is the presence signal, so no presence byte goes
        // into the body (#A18), unlike the @final/@appendable inline shape.
        let _ = writeln!(
            out,
            "        var body_s = Writer.init(w.buf.allocator, w.endian);"
        );
        let _ = writeln!(out, "        defer body_s.deinit();");
        let _ = writeln!(out, "        const body = &body_s;");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            let mu = if f.must_understand {
                0x8000_0000_u32
            } else {
                0
            };
            let emh = mu | 0x4000_0000 | f.id;
            // Value-only put: for an optional member use the presence-flag-free
            // form (guarded below), else the normal put.
            let put = if f.optional {
                f.opt_inner_put.clone()
            } else {
                f.put.clone()
            };
            let indent = if f.optional {
                "            "
            } else {
                "        "
            };
            if f.optional {
                let _ = writeln!(out, "        if (self.{}) |zdOptV| {{", f.zig_name);
            }
            let _ = writeln!(out, "{indent}try body.putU32(0x{emh:08x});");
            let _ = writeln!(out, "{indent}{{");
            let _ = writeln!(
                out,
                "{indent}    var zdMem_s = Writer.init(w.buf.allocator, w.endian);"
            );
            let _ = writeln!(out, "{indent}    defer zdMem_s.deinit();");
            let _ = writeln!(out, "{indent}    const zdMem = &zdMem_s;");
            let _ = writeln!(out, "{indent}    {}", put.replace("$w", "zdMem"));
            let _ = writeln!(
                out,
                "{indent}    try body.putU32(@intCast(zdMem.bytes().len));"
            );
            let _ = writeln!(out, "{indent}    try body.putBytes(zdMem.bytes());");
            let _ = writeln!(out, "{indent}}}");
            if f.optional {
                let _ = writeln!(out, "        }}");
            }
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
            if f.non_serialized {
                continue;
            }
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
    let mut zdkeys: Vec<&FieldGen> = fields
        .iter()
        .filter(|f| f.key && !f.non_serialized)
        .collect();
    zdkeys.sort_by_key(|f| f.id);
    if !zdkeys.is_empty() {
        // #A10: inherited `@key` members count too — filter the base-first
        // effective member list, not just this struct's own members.
        let mut eff: Vec<&Member> = Vec::new();
        collect_base_members(s, struct_defs, &mut eff);
        let key_members: Vec<&Member> = eff
            .into_iter()
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
                if struct_defs.contains_key(&resolve_scoped_name(sn)));
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
        // Positional @mutable decoder: assumes every member present in id order
        // (rides the existing naive decoder). An `@optional` member reads its
        // value straight from the body (no presence byte — #A18) and is stored
        // non-null; a member OMITTED on encode does NOT round-trip here, matching
        // the go backend's documented scope. A fully-present value round-trips.
        let _ = writeln!(out, "        _ = r.getU32();");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            let _ = writeln!(out, "        _ = r.getU32();");
            let _ = writeln!(out, "        _ = r.getU32();");
            if f.optional {
                let _ = writeln!(
                    out,
                    "        {{ var zdOptTmp: {} = undefined; {} v.{} = zdOptTmp; }}",
                    f.opt_inner_type,
                    f.opt_inner_get.replace("$r", "r"),
                    f.zig_name
                );
            } else {
                let _ = writeln!(out, "        {}", f.get.replace("$r", "r"));
            }
        }
    } else {
        if ext == ExtensibilityKind::Appendable {
            let _ = writeln!(out, "        _ = r.getU32();");
        }
        for f in &fields {
            if f.non_serialized {
                continue;
            }
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
    emit_verbatim_at(out, "    ", &s.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}};");
    Ok(())
}

/// Emits an IDL `union` as a discriminated holder + a `marshalInto` that puts
/// the discriminator then dispatches on it to the selected member (XCDR2
/// §7.4.3.5.4). `@final`: inline; `@appendable`: DHEADER-framed body; `@mutable`
/// (#A16): an EMHEADER-framed member list (discriminator = member id 0, each
/// branch = its 1-based id). Non-integer discriminators resolve via
/// [`eval_union_label`] — enum enumerators (#A11), `char` code points (#A12),
/// and `TRUE`/`FALSE` (#A13).
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
    let (disc_type, disc_put) = map_type(&disc_ts, "self.disc", enum_names, struct_names)?;
    let disc_get = map_get(&disc_ts, "v.disc", enum_names, struct_names)?;

    // #P4: when the discriminator is an enum, build enumerator-name → value so
    // `case ENUMERATOR:` labels resolve to their integer discriminant.
    let enum_vals: HashMap<String, i64> = match &u.switch_type {
        SwitchTypeSpec::Scoped(sn) => enum_defs
            .get(&resolve_scoped_name(sn))
            .map(|e| {
                e.enumerators
                    .iter()
                    .zip(enumerator_values(e))
                    .map(|(en, val)| (en.name.text.clone(), i64::from(val)))
                    .collect()
            })
            .unwrap_or_default(),
        _ => HashMap::new(),
    };
    // Zig switch scrutinee: an enum-typed discriminator switches on its integer
    // value (`@intFromEnum`) so integer case labels apply; a bool switches on
    // `true`/`false`; every other discriminator is an integer/char.
    let disc_is_enum = matches!(&u.switch_type, SwitchTypeSpec::Scoped(sn)
        if enum_names.contains(&resolve_scoped_name(sn)));
    let disc_is_bool = matches!(u.switch_type, SwitchTypeSpec::Boolean);
    let scrut_enc = if disc_is_enum {
        "@intFromEnum(self.disc)"
    } else {
        "self.disc"
    };
    let scrut_dec = if disc_is_enum {
        "@intFromEnum(v.disc)"
    } else {
        "v.disc"
    };
    let render_label = |v: i64| -> String {
        if disc_is_bool {
            if v == 0 { "false" } else { "true" }.to_string()
        } else {
            v.to_string()
        }
    };

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
                CaseLabel::Value(e) => {
                    labels.push(eval_union_label(e, &enum_vals).ok_or_else(|| {
                        IdlZigError::Unsupported(format!(
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
        });
    }
    let has_default = cases.iter().any(|c| c.is_default);
    // A bool discriminator with both `true` and `false` branches is exhaustive:
    // Zig rejects an `else` prong on an exhaustive switch, so suppress it.
    let bool_exhaustive = disc_is_bool
        && cases.iter().any(|c| c.labels.contains(&1))
        && cases.iter().any(|c| c.labels.contains(&0));
    let need_else = !has_default && !bool_exhaustive;
    let labels_of = |c: &UnionCase| -> String {
        c.labels
            .iter()
            .map(|v| render_label(*v))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let ty = escape_zig_ident(&qualify(scope, &u.name.text));
    let _ = writeln!(out, "\npub const {ty} = struct {{");
    emit_verbatim_at(out, "    ", &u.annotations, PlacementKind::BeginDeclaration);
    let _ = writeln!(out, "    disc: {disc_type},");
    for c in &cases {
        let _ = writeln!(out, "    {}: {},", c.field, c.ty);
    }
    let _ = writeln!(
        out,
        "\n    pub fn marshalInto(self: {ty}, w: *Writer) !void {{"
    );
    if ext == ExtensibilityKind::Mutable {
        // #A16: EMHEADER-framed member list. Discriminator = member id 0; the
        // selected branch = its 1-based case index.
        let _ = writeln!(
            out,
            "        var body_s = Writer.init(w.buf.allocator, w.endian);"
        );
        let _ = writeln!(out, "        defer body_s.deinit();");
        let _ = writeln!(out, "        const body = &body_s;");
        emit_union_mutable_member(out, "        ", 0, &disc_put);
        let _ = writeln!(out, "        switch ({scrut_enc}) {{");
        for (i, c) in cases.iter().enumerate() {
            let id = u32::try_from(i + 1).unwrap_or(0);
            let head = if c.is_default {
                "            else =>".to_string()
            } else {
                format!("            {} =>", labels_of(c))
            };
            let _ = writeln!(out, "{head} {{");
            emit_union_mutable_member(out, "                ", id, &c.put);
            let _ = writeln!(out, "            }},");
        }
        if need_else {
            let _ = writeln!(out, "            else => {{}},");
        }
        let _ = writeln!(out, "        }}");
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
        let _ = writeln!(out, "        {}", disc_put.replace("$w", wv));
        let _ = writeln!(out, "        switch ({scrut_enc}) {{");
        for c in &cases {
            // Block form (`=> { put; }`): the generated put already ends in `;`.
            if c.is_default {
                let _ = writeln!(
                    out,
                    "            else => {{ {} }},",
                    c.put.replace("$w", wv)
                );
            } else {
                let _ = writeln!(
                    out,
                    "            {} => {{ {} }},",
                    labels_of(c),
                    c.put.replace("$w", wv)
                );
            }
        }
        if need_else {
            let _ = writeln!(out, "            else => {{}},");
        }
        let _ = writeln!(out, "        }}");
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

    // Decode: read the discriminator, then read only the selected member.
    // @appendable skips the leading DHEADER; @mutable skips DHEADER, reads the
    // discriminator's EMHEADER+NEXTINT, switches, then reads the selected
    // branch's EMHEADER+NEXTINT + value. Unread members stay undefined.
    let _ = writeln!(out, "\n    pub fn readFrom(r: *Reader) !{ty} {{");
    let _ = writeln!(out, "        var v: {ty} = undefined;");
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "        _ = r.getU32();");
    }
    if ext == ExtensibilityKind::Mutable {
        let _ = writeln!(out, "        _ = r.getU32();");
        let _ = writeln!(out, "        _ = r.getU32();");
    }
    let _ = writeln!(out, "        {}", disc_get.replace("$r", "r"));
    let _ = writeln!(out, "        switch ({scrut_dec}) {{");
    for c in &cases {
        let body = if ext == ExtensibilityKind::Mutable {
            format!(
                "_ = r.getU32(); _ = r.getU32(); {}",
                c.get.replace("$r", "r")
            )
        } else {
            c.get.replace("$r", "r")
        };
        if c.is_default {
            let _ = writeln!(out, "            else => {{ {body} }},");
        } else {
            let _ = writeln!(out, "            {} => {{ {body} }},", labels_of(c));
        }
    }
    if need_else {
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
    emit_verbatim_at(out, "    ", &u.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}};");
    Ok(())
}

/// Writes one `@mutable` union member into the `body` writer: its EMHEADER (LC4
/// | member id) + NEXTINT (length) + value (#A16). `put` uses the `$w`
/// placeholder (rewritten to the per-member scratch writer here).
fn emit_union_mutable_member(out: &mut String, indent: &str, id: u32, put: &str) {
    let emh = 0x4000_0000_u32 | id;
    let _ = writeln!(out, "{indent}try body.putU32(0x{emh:08x});");
    let _ = writeln!(out, "{indent}{{");
    let _ = writeln!(
        out,
        "{indent}    var zdMem_s = Writer.init(body.buf.allocator, body.endian);"
    );
    let _ = writeln!(out, "{indent}    defer zdMem_s.deinit();");
    let _ = writeln!(out, "{indent}    const zdMem = &zdMem_s;");
    let _ = writeln!(out, "{indent}    {}", put.replace("$w", "zdMem"));
    let _ = writeln!(
        out,
        "{indent}    try body.putU32(@intCast(zdMem.bytes().len));"
    );
    let _ = writeln!(out, "{indent}    try body.putBytes(zdMem.bytes());");
    let _ = writeln!(out, "{indent}}}");
}

/// Maps an IDL type to `(Zig type, put statement)`. The put uses `$w` as the
/// writer placeholder and `expr` as the value expression.
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
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
        TypeSpec::Sequence(seq) => map_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            expr,
            enum_names,
            struct_names,
        ),
        // `fixed<P,S>` (XCDR2 §7.4.4.5 / CORBA §9.3.2.7): a decimal STRING field
        // encoded as `(P+2)/2` packed-BCD octets — no length prefix, no
        // alignment (`putFixed`/`getFixed` in the prelude carry the codec).
        TypeSpec::Fixed(f) => {
            let p = array_size(&f.digits).unwrap_or(0);
            let s = array_size(&f.scale).unwrap_or(0);
            Ok((
                "[]const u8".to_string(),
                format!("try $w.putFixed({expr}, {p}, {s});"),
            ))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Enum holder width follows @bit_bound (XTypes 1.3 §7.4.5.1);
                // the Zig enum tag is i32, narrowed by @truncate for 1/2 octets.
                let put = match enum_wire_width(&name) {
                    1 => {
                        format!("try $w.putU8(@bitCast(@as(i8, @truncate(@intFromEnum({expr})))));")
                    }
                    2 => format!(
                        "try $w.putU16(@bitCast(@as(i16, @truncate(@intFromEnum({expr})))));"
                    ),
                    _ => format!("try $w.putU32(@bitCast(@intFromEnum({expr})));"),
                };
                Ok((escape_zig_ident(&name), put))
            } else if struct_names.contains(&name) {
                Ok((
                    escape_zig_ident(&name),
                    format!("try {expr}.marshalInto($w);"),
                ))
            } else {
                Err(IdlZigError::Unsupported(format!("scoped type {name}")))
            }
        }
        // Nested `map<K, V>` as a member/sequence-element/map-value (#A28): a
        // `[]const struct{ k, v }` slice, encoded like the top-level map handler
        // — copy, sort ascending by key, then `u32 count` + pairs. A nested map
        // value forces the DHEADER-framed (non-primitive) shape. Every temporary
        // carries a per-run-unique suffix (`fresh_id`) so an inner map never
        // shadows an outer one (Zig forbids local shadowing).
        TypeSpec::Map(mp) => {
            let n = fresh_id();
            let (key_type, key_put) =
                map_type(&mp.key, &format!("zdKvE{n}.k"), enum_names, struct_names)?;
            let (val_type, val_put) =
                map_type(&mp.value, &format!("zdKvE{n}.v"), enum_names, struct_names)?;
            let kv_ty = format!("struct {{ k: {key_type}, v: {val_type} }}");
            let prim = is_primitive(&mp.key, enum_names) && is_primitive(&mp.value, enum_names);
            let bound_check = mp
                .bound
                .as_ref()
                .map(|b| {
                    let bv = array_size(b).unwrap_or(i64::MAX);
                    format!("if ({expr}.len > {bv}) return error.BoundExceeded; ")
                })
                .unwrap_or_default();
            let head = format!(
                "const zdKvT{n} = @typeInfo(@TypeOf({expr})).Pointer.child; \
                 const zdKvS{n} = $w.buf.allocator.alloc(zdKvT{n}, {expr}.len) catch unreachable; \
                 defer $w.buf.allocator.free(zdKvS{n}); @memcpy(zdKvS{n}, {expr}); \
                 std.mem.sort(zdKvT{n}, zdKvS{n}, {{}}, struct {{ \
                 fn lt(_: void, za: zdKvT{n}, zb: zdKvT{n}) bool {{ return za.k < zb.k; }} }}.lt);"
            );
            let put = if prim {
                format!(
                    "{{ {bound_check}{head} try $w.putU32(@intCast(zdKvS{n}.len)); \
                     for (zdKvS{n}) |zdKvE{n}| {{ {key_put} {val_put} }} }}"
                )
            } else {
                let kp = key_put.replace("$w", &format!("zdKvSub{n}"));
                let vp = val_put.replace("$w", &format!("zdKvSub{n}"));
                format!(
                    "{{ {bound_check}{head} var zdKvSub{n} = Writer.init($w.buf.allocator, $w.endian); \
                     defer zdKvSub{n}.deinit(); try zdKvSub{n}.putU32(@intCast(zdKvS{n}.len)); \
                     for (zdKvS{n}) |zdKvE{n}| {{ {kp} {vp} }} \
                     const zdKvBB{n} = zdKvSub{n}.bytes(); try $w.putU32(@intCast(zdKvBB{n}.len)); \
                     try $w.putBytes(zdKvBB{n}); }}"
                )
            };
            Ok((format!("[]const {kv_ty}"), put))
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
        let name = resolve_scoped_name(sn);
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    enum_names: &HashSet<String>,
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
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let put = format!(
                "{{ {bound_check}var subw = Writer.init($w.buf.allocator, $w.endian); defer subw.deinit();                  const sub = &subw; try sub.putU32(@intCast({expr}.len));                  for ({expr}) |elem| try elem.marshalInto(sub);                  try $w.putU32(@intCast(sub.bytes().len)); try $w.putBytes(sub.bytes()); }}"
            );
            return Ok((format!("[]const {}", escape_zig_ident(&name)), put));
        }
    }
    // sequence<primitive | enum | string | fixed | nested-sequence> → a `u32`
    // element count followed by each element encoded inline (no per-element
    // DHEADER). Thin→thin parity with the Go backend (`idl-go` map_sequence
    // fallback): the element reuses the full `map_type` put.
    let (elem_zig, elem_put) = map_type(elem, "zdSeqE", enum_names, struct_names)?;
    let put = format!(
        "{{ {bound_check}try $w.putU32(@intCast({expr}.len)); for ({expr}) |zdSeqE| {{ {elem_put} }} }}"
    );
    Ok((format!("[]const {elem_zig}"), put))
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
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
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
        TypeSpec::Sequence(seq) => map_get_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            target,
            enum_names,
            struct_names,
        ),
        // `fixed<P,S>` decode: read `(P+2)/2` packed-BCD octets back into the
        // decimal string (inverse of `putFixed` — see the encode-side arm).
        TypeSpec::Fixed(f) => {
            let p = array_size(&f.digits).unwrap_or(0);
            let s = array_size(&f.scale).unwrap_or(0);
            Ok(format!("{target} = try r.getFixed({p}, {s});"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Read the @bit_bound-wide holder and sign-extend to the i32 tag.
                let get = match enum_wire_width(&name) {
                    1 => {
                        format!("{target} = @enumFromInt(@as(i32, @as(i8, @bitCast(r.getU8()))));")
                    }
                    2 => format!(
                        "{target} = @enumFromInt(@as(i32, @as(i16, @bitCast(r.getU16()))));"
                    ),
                    _ => format!("{target} = @enumFromInt(@as(i32, @bitCast(r.getU32())));"),
                };
                Ok(get)
            } else if struct_names.contains(&name) {
                Ok(format!(
                    "{target} = try {}.readFrom(r);",
                    escape_zig_ident(&name)
                ))
            } else {
                Err(IdlZigError::Unsupported(format!("scoped type {name}")))
            }
        }
        // Nested `map<K, V>` decode (#A28, inverse of the map arm in `map_type`):
        // an optional collection DHEADER (non-primitive pair) + `u32 count` +
        // per-entry key/value decode into an allocated `[]struct{ k, v }`.
        // Unique `{n}` suffixes keep nested `while` loops from colliding.
        TypeSpec::Map(mp) => {
            let n = fresh_id();
            let (key_type, _) = map_type(&mp.key, "", enum_names, struct_names)?;
            let (val_type, _) = map_type(&mp.value, "", enum_names, struct_names)?;
            let kv_ty = format!("struct {{ k: {key_type}, v: {val_type} }}");
            let prim = is_primitive(&mp.key, enum_names) && is_primitive(&mp.value, enum_names);
            let dh = if prim { "" } else { "_ = r.getU32(); " };
            let bound_check = mp
                .bound
                .as_ref()
                .map(|b| {
                    let bv = array_size(b).unwrap_or(i64::MAX);
                    format!("if (zdKvN{n} > {bv}) return error.BoundExceeded; ")
                })
                .unwrap_or_default();
            let key_get = map_get(
                &mp.key,
                &format!("zdKvL{n}[zdKvI{n}].k"),
                enum_names,
                struct_names,
            )?;
            let val_get = map_get(
                &mp.value,
                &format!("zdKvL{n}[zdKvI{n}].v"),
                enum_names,
                struct_names,
            )?;
            Ok(format!(
                "{{ {dh}const zdKvN{n} = r.getU32(); {bound_check}const zdKvL{n} = try r.alloc.alloc({kv_ty}, zdKvN{n}); \
                 var zdKvI{n}: usize = 0; while (zdKvI{n} < zdKvN{n}) : (zdKvI{n} += 1) {{ {key_get} {val_get} }} {target} = zdKvL{n}; }}"
            ))
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    target: &str,
    enum_names: &HashSet<String>,
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
        let name = resolve_scoped_name(sn);
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
    // sequence<primitive | enum | string | fixed | nested-sequence> → `u32`
    // count + inline per-element decode (inverse of the `map_sequence`
    // fallback; no per-element DHEADER). The bound is checked BEFORE the
    // `alloc`/decode loop so an attacker-declared huge count cannot drive an
    // oversized allocation.
    let (elem_zig, _) = map_type(elem, "", enum_names, struct_names)?;
    let elem_get = map_get(elem, "zdList[zdI]", enum_names, struct_names)?;
    let bound_check = bv
        .map(|bv| format!("if (zdN > {bv}) return error.BoundExceeded; "))
        .unwrap_or_default();
    Ok(format!(
        "{{ const zdN = r.getU32(); {bound_check}const zdList = try r.alloc.alloc({elem_zig}, zdN); var zdI: usize = 0; while (zdI < zdN) : (zdI += 1) {{ {elem_get} }} {target} = zdList; }}"
    ))
}
