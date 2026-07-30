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

use crate::error::{IdlSwiftError, Result};
use crate::keywords::escape_swift_ident;

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
    /// reference to one of these maps to a Swift holder struct whose wire form
    /// is a single backing integer (`marshalInto`/`unmarshalFrom`) — no
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

/// Swift codegen language aliases matched by `@verbatim(language="...")`
/// (case-insensitive; the spec wildcard `"*"` always matches — see
/// [`Lowered::verbatims_for_language`]).
const SWIFT_LANG_ALIASES: &[&str] = &["swift"];

/// Emits every `@verbatim` block from `anns` whose language matches the Swift
/// codegen and whose `placement` equals `placement`, each line prefixed with
/// `indent`. Source order preserved; text spliced unmodified (no wire impact —
/// XTypes 1.3 §7.2.2.4.8 / IDL 4.2 §8.3.5.1). Mirrors `idl-rust`'s
/// `verbatim::emit_verbatim_at`.
fn emit_verbatim_at(out: &mut String, indent: &str, anns: &[Annotation], placement: PlacementKind) {
    let Ok(lowered) = lower_annotations(anns) else {
        return;
    };
    for v in lowered.verbatims_for_language(SWIFT_LANG_ALIASES) {
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

/// Injective flattened name for a declaration `simple` in module `scope`, via
/// the shared [`zerodds_idl::naming::encode_scoped`] encoding. The scope
/// separator (`_s`) and a literal underscore in a name (`_u`) are distinct, so
/// `A::B_C` and `A_B::C` no longer collapse to `A_B_C` (unlike the old
/// `join("_")`, which was NOT collision-free). Two same-simple-name types in
/// different modules become distinct types (`a_sReading`/`b_sReading`, #21).
fn qualify(scope: &[String], simple: &str) -> String {
    zerodds_idl::naming::encode_scoped(scope, simple)
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
            // #A39: interface-nested type declarations are promoted to the top
            // level under the interface's own scope segment, so their paths must
            // be registered for reference resolution (`interface I { struct C; }`
            // → `I_C`). Non-type exports (ops/attrs) carry no wire type.
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

/// Flattens a full type path (module components + simple name) exactly as the
/// definition site does via [`qualify`]/[`zerodds_idl::naming::encode_scoped`],
/// so a reference resolves to the same identifier the declaration emitted.
fn flatten_path(path: &[String]) -> String {
    match path.split_last() {
        Some((simple, scope)) => zerodds_idl::naming::encode_scoped(scope, simple),
        None => String::new(),
    }
}

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

/// BCD codec for `fixed<P,S>`. Appended once when any `fixed` member is emitted.
/// Builds the packed-BCD octet sequence (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5)
/// from a decimal string: an optional leading pad nibble (so the nibble count
/// is even), `P` digit nibbles most-significant first, then the sign nibble
/// (`0xC` positive, `0xD` negative). Byte count `(P+2)/2`, no length prefix.
const FIXED_PRELUDE: &str = r#"
func zdFixedEnc(_ s: String, _ P: Int, _ S: Int) -> [UInt8] {
    var sign = true
    let chars = Array(s)
    var i = 0
    if !chars.isEmpty && (chars[0] == "-" || chars[0] == "+") { sign = chars[0] != "-"; i = 1 }
    let rest = Array(chars[i...])
    var dot = rest.count
    for (k, c) in rest.enumerated() { if c == "." { dot = k; break } }
    let ip = Array(rest[0..<dot])
    let fp = dot < rest.count ? Array(rest[(dot + 1)...]) : []
    var db: [Character] = []
    let intNeeded = P - S
    if ip.count < intNeeded { for _ in ip.count..<intNeeded { db.append("0") } }
    db.append(contentsOf: ip)
    db.append(contentsOf: fp)
    if fp.count < S { for _ in fp.count..<S { db.append("0") } }
    var nib: [UInt8] = []
    if (P + 1) % 2 == 1 { nib.append(0) }
    for c in db { nib.append(UInt8(c.wholeNumberValue ?? 0)) }
    nib.append(sign ? 0x0C : 0x0D)
    var outb: [UInt8] = []
    var k = 0
    while k < nib.count { outb.append((nib[k] << 4) | nib[k + 1]); k += 2 }
    return outb
}
"#;

/// Generates a self-contained Swift module from the IDL AST: the shared XCDR2
/// wire `Writer`/`Reader` prelude followed by every generated type.
///
/// # Errors
/// Returns [`IdlSwiftError::Unsupported`] for constructs the Swift backend does
/// not yet emit (e.g. non-literal array/sequence bounds).
pub fn generate_swift_module(spec: &Specification, opts: &SwiftGenOptions) -> Result<String> {
    generate(spec, opts, true)
}

/// Generates a Swift **fragment** for the given IDL AST: the generated types
/// only, WITHOUT the shared wire prelude (`Writer`/`Reader`/`Endianness`/
/// `XcdrBoundError`). Use this for every file but the first in a multi-file
/// compose so the prelude is defined exactly once across the Swift module
/// (#C-swift — the whole-prelude was previously emitted per file, so a second
/// generated file re-declared `Writer`/`Reader`/`Endianness` and the module
/// failed to build with duplicate-definition errors).
///
/// # Errors
/// As [`generate_swift_module`].
pub fn generate_swift_fragment(spec: &Specification, opts: &SwiftGenOptions) -> Result<String> {
    generate(spec, opts, false)
}

fn generate(spec: &Specification, _opts: &SwiftGenOptions, emit_prelude: bool) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Code generated by zerodds-idlc (Swift backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "// SPDX-License-Identifier: Apache-2.0\n");
    // #C-swift: the shared wire prelude is emitted only by the prelude-carrying
    // file. A fragment relies on another file in the same Swift module owning
    // `Writer`/`Reader`/`Endianness`/`XcdrBoundError`.
    if emit_prelude {
        out.push_str(WIRE_PRELUDE);
    }

    // Register every named type's fully-qualified path so reference sites can
    // resolve a `ScopedName` against its enclosing scope (#21 cross-module,
    // #A39 interface-nested).
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
    // #A39: interface-nested type declarations, promoted to the top level under
    // the interface's own scope segment (Swift has no nested-in-interface type),
    // so their DDS data types survive instead of being dropped with the body.
    let iface_types = flatten_iface_types(&spec.definitions);

    // `bitset`/`bitmask` logical names, published to `BIT_NAMES` so a reference
    // site resolves them to the integer-backed holder (no collection DHEADER).
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

    // typedef qualified-name → aliased type-spec (wire-transparent) and struct
    // qualified-name → def (nested-struct `@key` KeyHash expansion). Interface-
    // nested typedefs/structs are folded in too (#A39).
    let mut typedefs = collect_typedefs(spec);
    let mut struct_defs = collect_struct_defs(spec);
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
                struct_defs.insert(qualify(scope, &s.name.text), s);
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
                &typedefs,
                &struct_defs,
                &enum_defs,
            )?,
            // #A5/P1 — a top-level `const` was silently dropped by the former
            // catch-all arm; emit it as a Swift file-level constant.
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
            &typedefs,
            &struct_defs,
            &enum_defs,
        )?;
    }

    // §7.2.2.4.8 — `@verbatim(placement=END_FILE)` from all top-level defs.
    for def in &spec.definitions {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::EndFile);
    }

    // The BCD codec prelude is appended once if any `fixed<P,S>` was emitted.
    // Only the prelude-carrying file owns the shared helper (#C-swift).
    if emit_prelude && USED_FIXED.with(std::cell::Cell::get) {
        out.push_str(FIXED_PRELUDE);
    }

    // CryptoKit is needed only for the KeyHash MD5 branch; import it on demand.
    if out.contains("Insecure.MD5") {
        out = format!("import CryptoKit\n{out}");
    }
    Ok(out)
}

/// Dispatches a single `TypeDecl` (module-level or interface-nested) to the
/// matching emitter. Shared by the module-level and interface-nested passes so
/// #A39 promoted types go through the same code paths.
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
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => emit_bitset(out, b, scope)?,
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => emit_bitmask(out, b, scope),
        _ => {}
    }
    Ok(())
}

/// Recursively descends into `Definition::Interface` bodies, returning every
/// interface-nested `Export::Type` declaration paired with the scope path
/// `enclosing_module… + interface_name` (#A39). Swift has no nested-in-interface
/// type, so these are promoted to the top level under the interface's own name
/// segment (so two interfaces in one module do not collide).
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

/// Emits an IDL `const` as a Swift file-level constant (#A5/P1). A `const` of
/// any type used to vanish through the top-level catch-all arm. The value is
/// rendered from the `ConstExpr` (Boolean literals normalized to `true`/`false`,
/// `char`/`wchar` literals to their code point since a `char` const maps to a
/// Swift `UInt8`/`UInt32`, and any wide `L"…"`/`L'…'` prefix stripped, so the
/// output is always valid Swift). Values Swift cannot express as a compile-time
/// constant (an enum-typed / scoped reference) are skipped rather than emitting
/// ill-formed source.
fn emit_const(out: &mut String, c: &ConstDecl, scope: &[String]) {
    let Some(val) = const_expr_to_swift(&c.value) else {
        return;
    };
    let Some(ty) = const_swift_type(&c.type_) else {
        return;
    };
    let name = escape_swift_ident(&qualify(scope, &c.name.text));
    let _ = writeln!(out, "\npublic let {name}: {ty} = {val}");
}

/// Swift type for a `const` declaration (`None` = a form the Swift backend does
/// not express as a compile-time constant, e.g. an enum-valued scoped const).
fn const_swift_type(ct: &ConstType) -> Option<&'static str> {
    Some(match ct {
        ConstType::Integer(i) => swift_int_type(*i),
        ConstType::Floating(FloatingType::Float) => "Float",
        ConstType::Floating(FloatingType::Double | FloatingType::LongDouble) => "Double",
        // A `char`/`octet` const holds a byte; a `wchar` const a UTF-16-wide
        // code unit (this backend's `wchar` wire is 4-byte — see `map_primitive`).
        ConstType::Char | ConstType::Octet => "UInt8",
        ConstType::WideChar => "UInt32",
        ConstType::Boolean => "Bool",
        ConstType::String { .. } => "String",
        // A `fixed` const has no native Swift compile-time type — render its
        // decimal as a `String` (mirrors the `fixed<P,S>` member holder).
        ConstType::Fixed => "String",
        // An enum-typed / scoped const value cannot be reconstructed from the
        // bare enumerator name; skip.
        ConstType::Scoped(_) => return None,
    })
}

/// The Swift integer type for an IDL integer type.
fn swift_int_type(i: IntegerType) -> &'static str {
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

/// Renders a `ConstExpr` as a Swift constant expression, or `None` for a form
/// the Swift backend does not express (an enum-valued scoped reference).
/// zerodds-lint: recursion-depth 32 (const expression tree; bounded by the IDL
/// grammar's expression nesting).
fn const_expr_to_swift(e: &ConstExpr) -> Option<String> {
    match e {
        ConstExpr::Literal(l) => const_literal_to_swift(l),
        // An enum-valued or const-alias scoped reference cannot be rendered from
        // the bare last segment; skip (wire-neutral — the const is a codegen
        // convenience, and a wrong Swift identifier would break the build).
        ConstExpr::Scoped(_) => None,
        ConstExpr::Unary { op, operand, .. } => {
            let v = const_expr_to_swift(operand)?;
            let o = match op {
                UnaryOp::Plus => "+",
                UnaryOp::Minus => "-",
                UnaryOp::BitNot => "~",
            };
            Some(format!("{o}{v}"))
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let l = const_expr_to_swift(lhs)?;
            let r = const_expr_to_swift(rhs)?;
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

/// Renders a single literal as valid Swift source.
fn const_literal_to_swift(l: &Literal) -> Option<String> {
    let raw = l.raw.trim();
    Some(match l.kind {
        // Swift accepts decimal / `0x` / `0o` / `0b` integer literals as-is.
        LiteralKind::Integer => raw.to_string(),
        // Strip a trailing IDL float/fixed suffix (`d`/`f`/`l`) Swift rejects.
        LiteralKind::Floating => raw
            .trim_end_matches(['d', 'D', 'f', 'F', 'l', 'L'])
            .to_string(),
        // A `fixed` decimal has no native Swift type — render as a string.
        LiteralKind::Fixed => format!(
            "\"{}\"",
            raw.trim_end_matches(['d', 'D']).replace('"', "\\\"")
        ),
        // Normalize the IDL boolean keyword to Swift's `true`/`false` (never
        // emit a bare `TRUE`/`FALSE` token — #A13).
        LiteralKind::Boolean => {
            if raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // A `char`/`wchar` const maps to a Swift integer type, so render the
        // code point rather than a `'A'`/`L'A'` literal (invalid as an integer).
        LiteralKind::Char | LiteralKind::WideChar => char_literal_value(raw)?.to_string(),
        // A narrow string passes through; a wide literal drops the `L` prefix
        // (`L"x"` is not valid Swift — #A7 family).
        LiteralKind::String => raw.to_string(),
        LiteralKind::WideString => raw.strip_prefix('L').unwrap_or(raw).to_string(),
    })
}

/// Evaluates a `char`/`wchar` literal (`'A'`, `L'x'`, `'\n'`) to its code point.
/// Used by both the `const` renderer and the union label evaluator (#A12) so a
/// `case 'A':` and a `const char C = 'A';` both resolve to the value 65.
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

/// Emits an IDL `enum` as a Swift `Int32`-raw enum.
fn emit_enum(out: &mut String, e: &EnumDef, scope: &[String]) {
    let values = enumerator_values(e);
    let ty = escape_swift_ident(&qualify(scope, &e.name.text));
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

/// Backing-integer storage for a bit container of `total_bits` bits: XTypes 1.3
/// §7.4.7 — the smallest holder that fits (`≤8`→UInt8, `≤16`→UInt16, `≤32`→
/// UInt32, else UInt64). Returns `(Swift type, put-method, get-method)`.
fn bit_storage(total_bits: usize) -> (&'static str, &'static str, &'static str) {
    match total_bits {
        0..=8 => ("UInt8", "putU8", "getU8"),
        9..=16 => ("UInt16", "putU16", "getU16"),
        17..=32 => ("UInt32", "putU32", "getU32"),
        _ => ("UInt64", "putU64", "getU64"),
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

/// Emits an IDL `bitset` as a Swift holder struct over its backing integer, with
/// a bit-accessor pair per named bitfield and an XCDR2 marshal/unmarshal that
/// writes the backing integer (XTypes 1.3 §7.4.7 — wire = backing int). The
/// method surface (`marshalInto`/`marshalXCDR`/`unmarshalFrom`/`unmarshalXCDR`)
/// matches a plain struct, so a reference site reuses the struct code paths.
///
/// # Errors
/// [`IdlSwiftError::Unsupported`] if a bitfield width is not a codegen-time
/// integer.
fn emit_bitset(out: &mut String, b: &BitsetDecl, scope: &[String]) -> Result<()> {
    let mut widths: Vec<usize> = Vec::with_capacity(b.bitfields.len());
    for bf in &b.bitfields {
        let w = array_size(&bf.spec.width)
            .filter(|w| *w >= 0)
            .ok_or_else(|| {
                IdlSwiftError::Unsupported(format!(
                    "non-integer bitfield width in bitset {}",
                    b.name.text
                ))
            })? as usize;
        widths.push(w);
    }
    let total: usize = widths.iter().sum();
    let (sty, put, get) = bit_storage(total);
    let ty = escape_swift_ident(&qualify(scope, &b.name.text));

    let _ = writeln!(out, "\npublic struct {ty} {{");
    let _ = writeln!(out, "    public var storage: {sty} = 0");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::BeginDeclaration);
    let mut offset: usize = 0;
    for (bf, width) in b.bitfields.iter().zip(&widths) {
        if let Some(name) = &bf.name {
            let field = escape_swift_ident(&name.text);
            if *width == 1 {
                let _ = writeln!(
                    out,
                    "    public func {field}() -> Bool {{ return ((storage >> {offset}) & 1) != 0 }}"
                );
                let _ = writeln!(
                    out,
                    "    public mutating func set_{field}(_ v: Bool) {{ let m = {sty}(1) << {offset}; if v {{ storage |= m }} else {{ storage &= ~m }} }}"
                );
            } else {
                let mask: u128 = if *width >= 64 {
                    u128::from(u64::MAX)
                } else {
                    (1u128 << *width) - 1
                };
                let _ = writeln!(
                    out,
                    "    public func {field}() -> {sty} {{ return {sty}((storage >> {offset}) & {mask}) }}"
                );
                let _ = writeln!(
                    out,
                    "    public mutating func set_{field}(_ v: {sty}) {{ let m = {sty}({mask}) << {offset}; storage = (storage & ~m) | ((v & {sty}({mask})) << {offset}) }}"
                );
            }
        }
        offset += width;
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::EndDeclaration);
    emit_bit_wire(out, &ty, put, get);
    Ok(())
}

/// Emits an IDL `bitmask` as a Swift holder struct over its `@bit_bound` backing
/// integer (default 32), with an OR-able manifest constant per bit value and an
/// XCDR2 marshal/unmarshal writing the backing integer (XTypes 1.3 §7.4.7).
fn emit_bitmask(out: &mut String, b: &BitmaskDecl, scope: &[String]) {
    let (sty, put, get) = bit_storage(bitmask_bit_bound(&b.annotations) as usize);
    let ty = escape_swift_ident(&qualify(scope, &b.name.text));

    let _ = writeln!(out, "\npublic struct {ty} {{");
    let _ = writeln!(out, "    public var storage: {sty} = 0");
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::BeginDeclaration);
    for (idx, v) in b.values.iter().enumerate() {
        let pos = bit_position(&v.annotations).unwrap_or(idx as u32);
        let cname = escape_swift_ident(&v.name.text.to_uppercase());
        let _ = writeln!(
            out,
            "    public static let {cname}: {sty} = {sty}(1) << {pos}"
        );
    }
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::EndDeclaration);
    emit_bit_wire(out, &ty, put, get);
}

/// Shared marshal/unmarshal surface for a bitset/bitmask holder `ty` (wire =
/// backing integer via `put`/`get`, no framing). `throws` for uniformity with
/// the struct method surface (a scoped reference calls these with `try`).
fn emit_bit_wire(out: &mut String, ty: &str, put: &str, get: &str) {
    let _ = writeln!(
        out,
        "\n    public func marshalInto(_ w: inout Writer) throws {{ w.{put}(storage) }}"
    );
    let _ = writeln!(
        out,
        "    public func marshalXCDR(_ endian: Endianness) throws -> [UInt8] {{ var w = Writer(endian); try marshalInto(&w); return w.bytes() }}"
    );
    let _ = writeln!(
        out,
        "    public static func unmarshalFrom(_ r: inout Reader) throws -> {ty} {{ return {ty}(storage: r.{get}()) }}"
    );
    let _ = writeln!(
        out,
        "    public static func unmarshalXCDR(_ buf: [UInt8], _ endian: Endianness) throws -> {ty} {{ var r = Reader(buf, endian); return try unmarshalFrom(&r) }}"
    );
    let _ = writeln!(out, "}}");
}

/// Resolves a `fixed<P,S>`'s digit count `P` and scale `S` to codegen-time
/// integers.
///
/// # Errors
/// [`IdlSwiftError::Unsupported`] if either is not a resolvable non-negative
/// integer literal.
fn fixed_ps(f: &FixedPtType) -> Result<(i64, i64)> {
    let p = array_size(&f.digits)
        .filter(|v| *v > 0)
        .ok_or_else(|| IdlSwiftError::Unsupported("non-integer fixed digit count".to_string()))?;
    let s = array_size(&f.scale)
        .filter(|v| *v >= 0)
        .ok_or_else(|| IdlSwiftError::Unsupported("non-integer fixed scale".to_string()))?;
    Ok((p, s))
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

/// Collects a struct's effective members base-first (#A10/P3): the base struct's
/// members (recursively) precede the derived struct's own, so the generated
/// Swift type and its wire form carry the inherited fields — matching
/// cpp/csharp/java/go. Without this a `struct D : Base` dropped every inherited
/// field from both the type and the wire.
///
/// zerodds-lint: recursion-depth 16 (struct inheritance chain; bounded by the
/// IDL aggregate nesting depth).
fn collect_base_members<'a>(
    s: &'a StructDef,
    struct_defs: &HashMap<String, &'a StructDef>,
    out: &mut Vec<&'a Member>,
) {
    if let Some(base) = &s.base {
        if let Some(bs) = struct_defs.get(&resolve_scoped_name(base)) {
            collect_base_members(bs, struct_defs, out);
        }
    }
    for m in &s.members {
        out.push(m);
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
        name: String,
        swift_type: String,
        put: String,
        get: String,
        id: u32,
        key: bool,
        resolved_type: TypeSpec,
        array_sizes: Option<Vec<i64>>,
        optional: bool,
        must_understand: bool,
        default_value: String,
        non_serialized: bool,
    }
    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
    // #A10/P3: base-first effective member list — the base struct's members
    // (recursively) precede the derived struct's own, so the generated Swift
    // type and its wire form carry the inherited fields (matching
    // cpp/csharp/java/go). Without this a `struct D : Base` dropped every
    // inherited field from both the type and the wire.
    let mut all_members: Vec<&Member> = Vec::new();
    collect_base_members(s, struct_defs, &mut all_members);
    for m in all_members.iter().copied() {
        let resolved = resolve_typedef(&m.type_spec, typedefs);
        let lowered = lower_annotations(&m.annotations).ok();
        let explicit_id = lowered.as_ref().and_then(|l| l.explicit_id());
        let key = lowered.as_ref().is_some_and(|l| l.has_key());
        let must_understand = lowered.as_ref().is_some_and(|l| {
            l.builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::MustUnderstand))
        });
        let optional = lowered.as_ref().is_some_and(|l| {
            l.builtins
                .iter()
                .any(|a| matches!(a, BuiltinAnnotation::Optional))
        });
        // An `@optional` member needs a decode-side default for the absent
        // branch (Swift has no default local init). A struct/bitset/bitmask-
        // typed value has no trivial zero literal, so reject it loudly rather
        // than emit code that would not compile (honest scope; primitive /
        // string / enum / sequence / array / map optionals round-trip).
        if optional {
            if let TypeSpec::Scoped(sn) = &resolved {
                let n = resolve_scoped_name(sn);
                if struct_names.contains(&n) || is_bit_name(&n) {
                    return Err(IdlSwiftError::Unsupported(format!(
                        "@optional struct/bitset-typed member `{}` (no Swift default)",
                        m.declarators
                            .first()
                            .map_or("?", |d| d.name().text.as_str())
                    )));
                }
            }
        }
        // P0-5 (#2): a `@non_serialized` member keeps its Swift field but is off
        // the wire and does NOT consume a sequential id slot (ids compact).
        let non_serialized =
            zerodds_idl::semantics::annotations::member_is_non_serialized(&m.annotations);
        for d in &m.declarators {
            let name = escape_swift_ident(&d.name().text);
            let id = if non_serialized {
                0
            } else {
                let assigned = explicit_id.unwrap_or(next_id);
                next_id = assigned + 1;
                assigned
            };
            let mut array_sizes: Option<Vec<i64>> = None;
            let (swift_type, put, get) = match d {
                Declarator::Simple(_) => {
                    let (t, p) = map_type(&resolved, &name, enum_names, struct_names, 0)?;
                    let g = map_get(&resolved, &name, enum_names, struct_names, 0)?;
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
                        map_type(&resolved, "$elem", enum_names, struct_names, 0)?;
                    let swift_type = sizes
                        .iter()
                        .fold(elem_type.clone(), |inner, _| format!("[{inner}]"));
                    let put = build_array_put(&name, &sizes, &elem_put);
                    let elem_get = map_get(&resolved, "zdE", enum_names, struct_names, 0)?;
                    let get = build_array_get(&name, &sizes, &elem_type, &elem_get);
                    array_sizes = Some(sizes);
                    (swift_type, put, get)
                }
            };
            let default_value = zero_value(&swift_type, enum_names);
            fields.push(FieldGen {
                name,
                swift_type,
                put,
                get,
                id,
                key,
                resolved_type: resolved.clone(),
                array_sizes,
                optional,
                must_understand,
                default_value,
                non_serialized,
            });
        }
    }

    let ty = escape_swift_ident(&qualify(scope, &s.name.text));
    let _ = writeln!(out, "\npublic struct {ty} {{");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "    ", &s.annotations, PlacementKind::BeginDeclaration);
    for f in &fields {
        // An `@optional` member carries a companion presence flag (XTypes 1.3
        // §7.4.5.1.4: uint8 present-flag then the value if present). Both the
        // flag and the value default so the value may be omitted at construction
        // (`T(a:)` leaves `b` absent).
        if f.optional {
            let _ = writeln!(out, "    public var {}_present: Bool = false", f.name);
            let _ = writeln!(
                out,
                "    public var {}: {} = {}",
                f.name, f.swift_type, f.default_value
            );
        } else {
            let _ = writeln!(out, "    public var {}: {}", f.name, f.swift_type);
        }
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
        // member id, plus the must-understand bit 31 when @must_understand —
        // #A17) + NEXTINT (body length) + body (XTypes §7.4.3.4.2). The LC4
        // length code stays (the byte-identical shared-reference form — #A19 is
        // a separate coordinated cross-backend change).
        let _ = writeln!(out, "        var body = Writer(w.endian)");
        for f in &fields {
            if f.non_serialized {
                continue;
            }
            // An `@optional` member is omitted from the member list when absent
            // (XTypes 1.3 §7.4.3.4.2): guard its EMHEADER+body on the flag.
            if f.optional {
                let _ = writeln!(out, "        if {}_present {{", f.name);
            }
            let mu_bit = if f.must_understand {
                0x8000_0000_u32
            } else {
                0
            };
            let emh = mu_bit | 0x4000_0000_u32 | (f.id & 0x0FFF_FFFF);
            let _ = writeln!(out, "        body.putU32(0x{emh:08x})");
            let _ = writeln!(out, "        do {{");
            let _ = writeln!(out, "            var zdMem = Writer(w.endian)");
            let _ = writeln!(out, "            {}", f.put.replace("$w", "zdMem"));
            let _ = writeln!(out, "            let zdMB = zdMem.bytes()");
            let _ = writeln!(out, "            body.putU32(UInt32(zdMB.count))");
            let _ = writeln!(out, "            body.putBytes(zdMB)");
            let _ = writeln!(out, "        }}");
            if f.optional {
                let _ = writeln!(out, "        }}");
            }
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
            if f.non_serialized {
                continue;
            }
            let put = f.put.replace("$w", wv);
            if f.optional {
                // uint8 presence flag then the value if present (§7.4.5.1.4).
                let _ = writeln!(
                    out,
                    "        {wv}.putU8({name}_present ? 1 : 0)\n        if {name}_present {{ {put} }}",
                    name = f.name
                );
            } else {
                let _ = writeln!(out, "        {put}");
            }
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
        // @mutable + @optional: an absent member is omitted from the wire member
        // list, which the naive positional decoder cannot detect. Decode
        // therefore assumes presence (rides the naive decoder — documented
        // limitation); present-only values round-trip.
        let _ = writeln!(out, "        _ = r.getU32() // DHEADER");
        for f in &fields {
            if f.non_serialized {
                // Off the wire: bind the in-memory field to its default so the
                // memberwise init below still receives it (P0-5, #2).
                let _ = writeln!(
                    out,
                    "        let {}: {} = {}",
                    f.name, f.swift_type, f.default_value
                );
                continue;
            }
            if f.optional {
                let _ = writeln!(out, "        var {}_present: Bool = true", f.name);
            }
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
            if f.non_serialized {
                // Off the wire: bind the in-memory field to its default (P0-5, #2).
                let _ = writeln!(
                    out,
                    "        let {}: {} = {}",
                    f.name, f.swift_type, f.default_value
                );
                continue;
            }
            if f.optional {
                // uint8 presence flag then the value if present (§7.4.5.1.4);
                // the value takes a zero default in the absent branch.
                let _ = writeln!(out, "        let {}_present: Bool = r.getBool()", f.name);
                let _ = writeln!(
                    out,
                    "        var {name}: {ty} = {def}",
                    name = f.name,
                    ty = f.swift_type,
                    def = f.default_value
                );
                let _ = writeln!(out, "        if {}_present {{ {} }}", f.name, f.get);
            } else {
                let _ = writeln!(out, "        var {}: {}", f.name, f.swift_type);
                let _ = writeln!(out, "        {}", f.get);
            }
        }
    }
    let args = fields
        .iter()
        .map(|f| {
            if f.optional {
                format!("{n}_present: {n}_present, {n}: {n}", n = f.name)
            } else {
                format!("{n}: {n}", n = f.name)
            }
        })
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

    let mut zdkeys: Vec<&FieldGen> = fields
        .iter()
        .filter(|f| f.key && !f.non_serialized)
        .collect();
    zdkeys.sort_by_key(|f| f.id);
    if !zdkeys.is_empty() {
        // #A10: the effective (base-first) member list, so an inherited `@key`
        // member is part of the KeyHash too.
        let key_members: Vec<&Member> = all_members
            .iter()
            .copied()
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
                if struct_defs.contains_key(&resolve_scoped_name(sn)));
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
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "    ", &s.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}}");
    Ok(())
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

/// Writes one `@mutable` member into writer `wv` (`"body"`): its EMHEADER (LC4
/// length code | member id, plus the must-understand bit 31 when `mu` — #A17)
/// then the value as a NEXTINT-prefixed body. The LC4 length code stays — the
/// byte-identical shared-reference form (#A19 is a separate coordinated change).
fn write_mutable_member_encode_swift(
    out: &mut String,
    indent: &str,
    wv: &str,
    id: u32,
    mu: bool,
    put: &str,
) {
    let mu_bit = if mu { 0x8000_0000_u32 } else { 0 };
    let emh = mu_bit | 0x4000_0000_u32 | (id & 0x0FFF_FFFF);
    let _ = writeln!(out, "{indent}{wv}.putU32(0x{emh:08x})");
    let _ = writeln!(out, "{indent}do {{");
    let _ = writeln!(out, "{indent}    var zdMem = Writer({wv}.endian)");
    let _ = writeln!(out, "{indent}    {}", put.replace("$w", "zdMem"));
    let _ = writeln!(out, "{indent}    let zdMB = zdMem.bytes()");
    let _ = writeln!(out, "{indent}    {wv}.putU32(UInt32(zdMB.count))");
    let _ = writeln!(out, "{indent}    {wv}.putBytes(zdMB)");
    let _ = writeln!(out, "{indent}}}");
}

/// Reads one `@mutable` member: its EMHEADER + NEXTINT (LC4) then the value via
/// `get`. Positional — it relies on members arriving in id order.
fn write_mutable_member_decode_swift(out: &mut String, indent: &str, get: &str) {
    let _ = writeln!(out, "{indent}_ = r.getU32() // EMHEADER");
    let _ = writeln!(out, "{indent}_ = r.getU32() // NEXTINT");
    let _ = writeln!(out, "{indent}{get}");
}

/// Emits an IDL `union` as a discriminated holder + a `marshalInto` that puts
/// the discriminator then dispatches on it to the selected member (XCDR2
/// §7.4.3.5.4). `@final`: inline; `@appendable`: DHEADER-framed body; `@mutable`:
/// an EMHEADER-framed member list (discriminator = member id 0, each branch its
/// 1-based id — #A16).
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
        "disc",
        enum_names,
        struct_names,
        0,
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
    // A boolean discriminator switches on Swift `true`/`false` (`disc` is a
    // `Bool`); an enum discriminator switches on its `Int32` `rawValue` (a Swift
    // `enum` cannot be matched against integer case labels). Every other
    // discriminator (integer/char) matches integer labels directly.
    let disc_is_bool = matches!(u.switch_type, SwitchTypeSpec::Boolean);
    let disc_is_enum = matches!(&u.switch_type, SwitchTypeSpec::Scoped(sn)
        if enum_names.contains(&resolve_scoped_name(sn)));

    let mut cases: Vec<UnionCase> = Vec::new();
    for c in &u.cases {
        let field = escape_swift_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ty, put) = map_type(&resolved, &field, enum_names, struct_names, 0)?;
        let get = map_get(
            &resolved,
            &format!("v.{field}"),
            enum_names,
            struct_names,
            0,
        )?;
        let zero = zero_value(&ty, enum_names);
        let mut labels = Vec::new();
        let mut is_default = false;
        for l in &c.labels {
            match l {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(e) => {
                    labels.push(eval_union_label(e, &enum_vals).ok_or_else(|| {
                        IdlSwiftError::Unsupported(format!(
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
    let has_default = cases.iter().any(|c| c.is_default);

    // Renders a case's labels: a boolean discriminator to `true`/`false`, every
    // other to the integer discriminant.
    let render_labels = |labels: &[i64]| -> String {
        labels
            .iter()
            .map(|&v| {
                if disc_is_bool {
                    (v != 0).to_string()
                } else {
                    v.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    // The value switched on: an enum discriminator dispatches on its `rawValue`.
    let switch_on = |disc_expr: &str| -> String {
        if disc_is_enum {
            format!("{disc_expr}.rawValue")
        } else {
            disc_expr.to_string()
        }
    };

    let ty = escape_swift_ident(&qualify(scope, &u.name.text));
    let _ = writeln!(out, "\npublic struct {ty} {{");
    emit_verbatim_at(out, "    ", &u.annotations, PlacementKind::BeginDeclaration);
    let _ = writeln!(out, "    public var disc: {disc_type}");
    for c in &cases {
        let _ = writeln!(out, "    public var {}: {}", c.field, c.ty);
    }
    let _ = writeln!(
        out,
        "\n    public func marshalInto(_ w: inout Writer) throws {{"
    );
    if ext == ExtensibilityKind::Mutable {
        // #A16: EMHEADER-framed member list — discriminator is member id 0, each
        // branch its 1-based id, wrapped in the struct's DHEADER.
        let _ = writeln!(out, "        var body = Writer(w.endian)");
        write_mutable_member_encode_swift(out, "        ", "body", 0, false, &disc_put);
        let _ = writeln!(out, "        switch {} {{", switch_on("disc"));
        for (i, c) in cases.iter().enumerate() {
            if c.is_default {
                let _ = writeln!(out, "        default:");
            } else {
                let _ = writeln!(out, "        case {}:", render_labels(&c.labels));
            }
            let id = u32::try_from(i + 1).unwrap_or(0);
            write_mutable_member_encode_swift(out, "            ", "body", id, false, &c.put);
        }
        if !has_default {
            let _ = writeln!(out, "        default: break");
        }
        let _ = writeln!(out, "        }}");
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
        let _ = writeln!(out, "        {}", disc_put.replace("$w", wv));
        let _ = writeln!(out, "        switch {} {{", switch_on("disc"));
        for c in &cases {
            if c.is_default {
                let _ = writeln!(out, "        default:");
            } else {
                let _ = writeln!(out, "        case {}:", render_labels(&c.labels));
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
    // only the selected member (@appendable skips the leading DHEADER; @mutable
    // skips the DHEADER then reads the disc + selected branch positionally).
    let disc_get = map_get(
        &switch_typespec(&u.switch_type),
        "zdDisc",
        enum_names,
        struct_names,
        0,
    )?;
    let _ = writeln!(
        out,
        "\n    public static func unmarshalFrom(_ r: inout Reader) throws -> {ty} {{"
    );
    if ext != ExtensibilityKind::Final {
        let _ = writeln!(out, "        _ = r.getU32() // DHEADER");
    }
    let _ = writeln!(out, "        var zdDisc: {disc_type}");
    if ext == ExtensibilityKind::Mutable {
        write_mutable_member_decode_swift(out, "        ", &disc_get);
    } else {
        let _ = writeln!(out, "        {disc_get}");
    }
    let zeros = cases
        .iter()
        .map(|c| format!("{}: {}", c.field, c.zero))
        .collect::<Vec<_>>()
        .join(", ");
    let sep = if cases.is_empty() { "" } else { ", " };
    let _ = writeln!(out, "        var v = {ty}(disc: zdDisc{sep}{zeros})");
    let _ = writeln!(out, "        switch {} {{", switch_on("zdDisc"));
    for c in &cases {
        if c.is_default {
            let _ = writeln!(out, "        default:");
        } else {
            let _ = writeln!(out, "        case {}:", render_labels(&c.labels));
        }
        if ext == ExtensibilityKind::Mutable {
            write_mutable_member_decode_swift(out, "            ", &c.get);
        } else {
            let _ = writeln!(out, "            {}", c.get);
        }
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
    emit_verbatim_at(out, "    ", &u.annotations, PlacementKind::EndDeclaration);
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

/// Builds a map put: `u32 count` + key/value pairs sorted ascending by key
/// (DHEADER-framed unless the key/value pair is primitive). Every temporary is
/// suffixed with the collection nesting `depth` and the non-primitive form is
/// wrapped in a `do { … }` block so sibling and nested maps never reuse a name
/// (#A21/A22 — a bare `zdSub`/`zdK` collided for two maps in one struct and
/// mis-bound the inner value in `map<K, map<K2,V>>`).
fn build_map_put(expr: &str, key_put: &str, val_put: &str, prim: bool, depth: usize) -> String {
    let k = format!("zdK{depth}");
    if prim {
        format!(
            "$w.putU32(UInt32({expr}.count))\n        for {k} in {expr}.keys.sorted() {{ {key_put}; {val_put} }}"
        )
    } else {
        let sub = format!("zdSub{depth}");
        let bb = format!("zdBB{depth}");
        let kp = key_put.replace("$w", &sub);
        let vp = val_put.replace("$w", &sub);
        format!(
            "do {{ var {sub} = Writer($w.endian)\n        {sub}.putU32(UInt32({expr}.count))\n        for {k} in {expr}.keys.sorted() {{ {kp}; {vp} }}\n        let {bb} = {sub}.bytes()\n        $w.putU32(UInt32({bb}.count)); $w.putBytes({bb}) }}"
        )
    }
}

/// zerodds-lint: recursion-depth 32
fn map_type(
    t: &TypeSpec,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    depth: usize,
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
        TypeSpec::Sequence(seq) => map_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            expr,
            enum_names,
            struct_names,
            depth,
        ),
        // A `fixed<P,S>` decimal: packed BCD, `(P+2)/2` raw octets, no length
        // prefix and no alignment (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5). The
        // Swift field holds the BCD bytes directly; `zdFixedEnc` builds them
        // from a decimal string.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let _ = fixed_ps(f)?; // validate P/S resolve at codegen time
            Ok(("[UInt8]".to_string(), format!("$w.putBytes({expr})")))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Enum holder width follows @bit_bound (XTypes 1.3 §7.4.5.1);
                // rawValue stays Int32, the narrow cast truncates the low octets.
                let put = match enum_wire_width(&name) {
                    1 => format!("$w.putU8(UInt8(truncatingIfNeeded: {expr}.rawValue))"),
                    2 => format!("$w.putU16(UInt16(truncatingIfNeeded: {expr}.rawValue))"),
                    _ => format!("$w.putU32(UInt32(bitPattern: {expr}.rawValue))"),
                };
                Ok((escape_swift_ident(&name), put))
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                Ok((
                    escape_swift_ident(&name),
                    format!("try {expr}.marshalInto(&$w)"),
                ))
            } else {
                Err(IdlSwiftError::Unsupported(format!("scoped type {name}")))
            }
        }
        // A map: entries sorted ascending by key, `u32 count` + key/value pairs
        // (no DHEADER for a primitive pair; DHEADER-framed otherwise). The key
        // loop variable and the nested key/value encoders are suffixed with the
        // nesting `depth` (#A21/A22).
        TypeSpec::Map(m) => {
            let k = format!("zdK{depth}");
            let (key_type, key_put) = map_type(&m.key, &k, enum_names, struct_names, depth + 1)?;
            let (val_type, val_put) = map_type(
                &m.value,
                &format!("{expr}[{k}]!"),
                enum_names,
                struct_names,
                depth + 1,
            )?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let bound_check = m.bound.as_ref().and_then(array_size).map(|bv| {
                format!("if {expr}.count > {bv} {{ throw XcdrBoundError(\"bounded map length exceeds its IDL bound ({bv})\") }}\n        ")
            }).unwrap_or_default();
            Ok((
                format!("[{key_type}: {val_type}]"),
                format!(
                    "{bound_check}{}",
                    build_map_put(expr, &key_put, &val_put, prim, depth)
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
    let (_, put) = map_type(&resolved, expr, enum_names, struct_names, 0)?;
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    depth: usize,
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
    // sequence<struct> → collection DHEADER + count + each element. A bitset/
    // bitmask is fully descriptive (backing int), so it takes the arbitrary
    // fallback below, not this DHEADER-framed struct branch. Temporaries carry
    // the nesting `depth` (#A21/A22 — nested/sibling `sequence<struct>`).
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let (sub, e, bb) = (
                format!("zdSub{depth}"),
                format!("zdElem{depth}"),
                format!("zdBB{depth}"),
            );
            let put = format!(
                "{bound_check}do {{ var {sub} = Writer($w.endian); {sub}.putU32(UInt32({expr}.count)); for {e} in {expr} {{ try {e}.marshalInto(&{sub}) }}; let {bb} = {sub}.bytes(); $w.putU32(UInt32({bb}.count)); $w.putBytes({bb}) }}"
            );
            return Ok((format!("[{}]", escape_swift_ident(&name)), put));
        }
    }
    // sequence<arbitrary> → u32 count + per-element encode, no collection
    // DHEADER (the element type is fully descriptive on the wire for the
    // primitive / enum / bitset / bitmask cases). Mirrors the `idl-go` /
    // `idl-d` fallback (`$w` survives to the field-level replace). The loop
    // variable and nested encoder are suffixed with `depth` (#A21/A22).
    let e = format!("zdElem{depth}");
    let (elem_ty, elem_put) = map_type(elem, &e, enum_names, struct_names, depth + 1)?;
    let put = format!(
        "{bound_check}$w.putU32(UInt32({expr}.count))\n        for {e} in {expr} {{ {elem_put} }}"
    );
    Ok((format!("[{elem_ty}]"), put))
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
    depth: usize,
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
        TypeSpec::Sequence(seq) => map_get_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            target,
            enum_names,
            struct_names,
            depth,
        ),
        // `fixed<P,S>`: read the statically-known `(P+2)/2` BCD octets.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let (p, _) = fixed_ps(f)?;
            let n = (p + 2) / 2;
            Ok(format!("{target} = r.getBytesN({n})"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                let esc = escape_swift_ident(&name);
                // Read the @bit_bound-wide holder and sign-extend to Int32.
                let get = match enum_wire_width(&name) {
                    1 => format!("{target} = {esc}(rawValue: Int32(Int8(bitPattern: r.getU8())))!"),
                    2 => {
                        format!("{target} = {esc}(rawValue: Int32(Int16(bitPattern: r.getU16())))!")
                    }
                    _ => format!("{target} = {esc}(rawValue: Int32(bitPattern: r.getU32()))!"),
                };
                Ok(get)
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                let esc = escape_swift_ident(&name);
                Ok(format!("{target} = try {esc}.unmarshalFrom(&r)"))
            } else {
                Err(IdlSwiftError::Unsupported(format!("scoped type {name}")))
            }
        }
        // A map decode: `zdN`/`zdK`/`zdV` are suffixed with the nesting `depth`
        // so `map<K, map<K2,V>>` does not reuse `zdV`/`zdK` (which previously
        // produced `zdV[zdK] = zdV`, a type error — #A21/A22).
        TypeSpec::Map(m) => {
            let (n, k, v) = (
                format!("zdN{depth}"),
                format!("zdK{depth}"),
                format!("zdV{depth}"),
            );
            let (key_type, _) = map_type(&m.key, &k, enum_names, struct_names, depth + 1)?;
            let (val_type, _) = map_type(&m.value, &v, enum_names, struct_names, depth + 1)?;
            let key_get = map_get(&m.key, &k, enum_names, struct_names, depth + 1)?;
            let val_get = map_get(&m.value, &v, enum_names, struct_names, depth + 1)?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            let dh = if prim { "" } else { "_ = r.getU32()\n        " };
            let bound_check = m.bound.as_ref().and_then(array_size).map(|bv| {
                format!("if {n} > {bv} {{ throw XcdrBoundError(\"decoded map length exceeds its IDL bound ({bv})\") }}; ")
            }).unwrap_or_default();
            Ok(format!(
                "{dh}do {{ let {n} = Int(r.getU32()); {bound_check}{target} = [:]\n        for _ in 0..<{n} {{ var {k}: {key_type}; {key_get}; var {v}: {val_type}; {val_get}; {target}[{k}] = {v} }} }}"
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_get_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    target: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    depth: usize,
) -> Result<String> {
    let bv = bound.and_then(array_size);
    // Length and element temporaries are suffixed with the nesting `depth`, so a
    // nested `sequence<sequence<T>>` decode never has the inner element temp
    // shadow the outer's append target (#A21/A22 — the former shared `zdE`
    // produced `zdE.append(zdE)` on `Int32`).
    let (n, e) = (format!("zdN{depth}"), format!("zdE{depth}"));
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
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let bound_check = bv
                .map(|bv| {
                    format!(
                        "if {n} > {bv} {{ throw XcdrBoundError(\"decoded sequence length exceeds its IDL bound ({bv})\") }}; "
                    )
                })
                .unwrap_or_default();
            let esc = escape_swift_ident(&name);
            return Ok(format!(
                "_ = r.getU32()\n        do {{ let {n} = Int(r.getU32()); {bound_check}{target} = []\n        for _ in 0..<{n} {{ {target}.append(try {esc}.unmarshalFrom(&r)) }} }}"
            ));
        }
    }
    // sequence<arbitrary> → u32 count + per-element decode, no DHEADER (inverse
    // of the `map_sequence` arbitrary fallback).
    let (elem_ty, _) = map_type(elem, &e, enum_names, struct_names, depth + 1)?;
    let elem_get = map_get(elem, &e, enum_names, struct_names, depth + 1)?;
    let bound_check = bv
        .map(|bv| {
            format!(
                "if {n} > {bv} {{ throw XcdrBoundError(\"decoded sequence length exceeds its IDL bound ({bv})\") }}; "
            )
        })
        .unwrap_or_default();
    Ok(format!(
        "do {{ let {n} = Int(r.getU32()); {bound_check}{target} = []\n        for _ in 0..<{n} {{ var {e}: {elem_ty}; {elem_get}; {target}.append({e}) }} }}"
    ))
}
