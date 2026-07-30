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

use crate::error::{IdlDError, Result};
use crate::keywords::escape_d_ident;

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
    /// reference to one of these maps to a D holder struct whose wire form is a
    /// single backing integer (`marshalInto`/`unmarshalFrom<name>`) — no
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

/// D codegen language aliases matched by `@verbatim(language="...")`
/// (case-insensitive; the spec wildcard `"*"` always matches — see
/// [`Lowered::verbatims_for_language`]).
const D_LANG_ALIASES: &[&str] = &["d", "dlang"];

/// Emits every `@verbatim` block from `anns` whose language matches the D
/// codegen and whose `placement` equals `placement`, each line prefixed with
/// `indent`. Source order preserved; text spliced unmodified (no wire impact —
/// XTypes 1.3 §7.2.2.4.8 / IDL 4.2 §8.3.5.1). Mirrors `idl-rust`'s
/// `verbatim::emit_verbatim_at`.
fn emit_verbatim_at(out: &mut String, indent: &str, anns: &[Annotation], placement: PlacementKind) {
    let Ok(lowered) = lower_annotations(anns) else {
        return;
    };
    for v in lowered.verbatims_for_language(D_LANG_ALIASES) {
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
/// single D identifier. Each segment's own underscores are doubled and the
/// segments joined by a single underscore, so `module A_B { struct C }`
/// (`["A_B","C"]` → `A__B_C`) never collides with `module A { module B {
/// struct C }}` (`["A","B","C"]` → `A_B_C`) — the previous `join("_")` mapped
/// both to `A_B_C` (#A35, non-injective flatten). Injectivity holds because IDL
/// identifiers may not start with `_`: a segment boundary is always the single
/// `_` immediately preceding a letter, so the even run of a segment's doubled
/// trailing underscores is unambiguously separable. A single (global-scope)
/// segment is returned verbatim so every existing top-level golden is unchanged,
/// and any segment without underscores (the common case) is passed through
/// untouched — mirrors `idl-go`/`idl-python`'s `flatten_path`.
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
            // Interface-nested types are promoted to the top level under the
            // interface's own scope segment (#A39/F39), so their reference paths
            // must be registered the same way a module-nested type's is.
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
/// interface-nested — #A39/F39).
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

/// BCD codec for `fixed<P,S>`. Appended once when any `fixed` member is emitted.
/// Builds the packed-BCD octet sequence (CORBA/GIOP §9.3.2.7 ≡ XCDR2 §7.4.4.5)
/// from a decimal string: an optional leading pad nibble (so the nibble count
/// is even), `P` digit nibbles most-significant first, then the sign nibble
/// (`0xC` positive, `0xD` negative). Byte count `(P+2)/2`, no length prefix.
const FIXED_PRELUDE: &str = r#"
ubyte[] zdFixedEnc(string s, uint P, uint S) {
    bool sign = true;
    size_t i = 0;
    if (s.length > 0 && (s[0] == '-' || s[0] == '+')) { sign = s[0] != '-'; i = 1; }
    string rest = s[i .. $];
    size_t dot = rest.length;
    foreach (k, c; rest) { if (c == '.') { dot = k; break; } }
    string ip = rest[0 .. dot];
    string fp = dot < rest.length ? rest[dot + 1 .. $] : "";
    char[] db;
    size_t intNeeded = P - S;
    if (ip.length < intNeeded) foreach (zj; ip.length .. intNeeded) db ~= '0';
    db ~= ip;
    db ~= fp;
    if (fp.length < S) foreach (zj; fp.length .. S) db ~= '0';
    ubyte[] nib;
    if ((P + 1) % 2 == 1) nib ~= cast(ubyte) 0;
    foreach (c; db) nib ~= cast(ubyte)(c - '0');
    nib ~= cast(ubyte)(sign ? 0x0C : 0x0D);
    ubyte[] outb;
    for (size_t k = 0; k < nib.length; k += 2) outb ~= cast(ubyte)((nib[k] << 4) | nib[k + 1]);
    return outb;
}
"#;

/// Generates a self-contained D module from the IDL AST.
///
/// # Errors
/// Returns [`IdlDError::Unsupported`] for constructs the D backend does not yet
/// emit (e.g. `@mutable` unions and non-literal array/sequence bounds).
pub fn generate_d_module(spec: &Specification, _opts: &DGenOptions) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Code generated by zerodds-idlc (D backend). DO NOT EDIT."
    );
    let _ = writeln!(out, "// SPDX-License-Identifier: Apache-2.0\n");
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
    // Interface-nested type declarations (#A39/F39): promoted to the top level
    // under the interface's own scope segment, so their DDS data types survive
    // instead of being silently dropped with the interface body.
    let iface_types = flatten_iface_types(&spec.definitions);

    // Every named `TypeDecl` (module-level + interface-nested) paired with its
    // scope, for the name-set collection below.
    let type_decls: Vec<(&Vec<String>, &TypeDecl)> = flat
        .iter()
        .filter_map(|(s, d)| match d {
            Definition::Type(td) => Some((s, td)),
            _ => None,
        })
        .chain(iface_types.iter().map(|(s, td)| (s, *td)))
        .collect();

    // Named enums/structs keyed by their flattened module-qualified name. An
    // enum member is a 32-bit signed integer on the wire (XTypes 1.3 §7.4.5.1).
    // `enum_defs` (name → def) lets a union's `case ENUMERATOR:` label resolve
    // to its integer discriminant (#P4).
    let mut enum_names: HashSet<String> = HashSet::new();
    let mut struct_names: HashSet<String> = HashSet::new();
    let mut bit_names: HashSet<String> = HashSet::new();
    let mut enum_defs: HashMap<String, &EnumDef> = HashMap::new();
    for (scope, td) in &type_decls {
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
        emit_verbatim_at(&mut out, "", anns, PlacementKind::BeforeDeclaration);
        match def {
            Definition::Type(td) => emit_type_decl(
                &mut out,
                td,
                scope,
                &enum_names,
                &struct_names,
                &enum_defs,
                &typedefs,
                &structs,
            )?,
            // #A5/F5: a top-level `const` used to vanish through this catch-all
            // arm. Emit it as a D manifest constant (wire-neutral convenience).
            Definition::Const(c) => emit_const(&mut out, c, scope),
            _ => {}
        }
        // §7.2.2.4.8 — text directly after the annotated declaration.
        emit_verbatim_at(&mut out, "", anns, PlacementKind::AfterDeclaration);
    }

    // #A39/F39: interface-nested types, emitted after the module-level defs.
    for (scope, td) in &iface_types {
        emit_type_decl(
            &mut out,
            td,
            scope,
            &enum_names,
            &struct_names,
            &enum_defs,
            &typedefs,
            &structs,
        )?;
    }

    // §7.2.2.4.8 — `@verbatim(placement=END_FILE)` from all top-level defs.
    for def in &spec.definitions {
        emit_verbatim_at(&mut out, "", def_annotations(def), PlacementKind::EndFile);
    }

    // The BCD codec prelude is appended once if any `fixed<P,S>` was emitted.
    if USED_FIXED.with(std::cell::Cell::get) {
        out.push_str(FIXED_PRELUDE);
    }
    Ok(out)
}

/// Emits a single `TypeDecl` (module-level or interface-nested — #A39/F39).
/// A `typedef` is wire-transparent and needs no D declaration (references are
/// dealiased before mapping), so it produces no output.
#[allow(clippy::too_many_arguments)]
fn emit_type_decl(
    out: &mut String,
    td: &TypeDecl,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    enum_defs: &HashMap<String, &EnumDef>,
    typedefs: &HashMap<String, TypeSpec>,
    structs: &HashMap<String, &StructDef>,
) -> Result<()> {
    match td {
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => emit_enum(out, e, scope),
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            emit_struct(out, s, scope, enum_names, struct_names, typedefs, structs)?;
        }
        TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u))) => {
            emit_union(out, u, scope, enum_names, struct_names, enum_defs, typedefs)?;
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => emit_bitset(out, b, scope)?,
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => emit_bitmask(out, b, scope),
        _ => {}
    }
    Ok(())
}

/// Interface-nested type declarations (`Export::Type`), promoted to the top
/// level under the interface's own scope segment (#A39/F39). Descends into
/// modules; interfaces do not nest, so no recursion into interface bodies.
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

/// Emits an IDL `const` as a D manifest constant (`enum <ty> <name> = <val>;`)
/// (#A5/F5/P1). Values the D type system cannot express as a compile-time
/// constant (an enum-typed / scoped reference) are skipped rather than emit a
/// broken identifier — the constant is a codegen convenience with no wire form.
fn emit_const(out: &mut String, c: &ConstDecl, scope: &[String]) {
    let Some(ty) = const_d_type(&c.type_) else {
        return;
    };
    let Some(val) = const_expr_to_d(&c.value) else {
        return;
    };
    let name = escape_d_ident(&qualify(scope, &c.name.text));
    let _ = writeln!(out, "\nenum {ty} {name} = {val};");
}

/// D type for a `const` declaration. `None` = a scoped/enum-typed const whose
/// value cannot be rendered from the bare enumerator name; skip it.
fn const_d_type(ct: &ConstType) -> Option<&'static str> {
    Some(match ct {
        ConstType::Integer(i) => d_int_type(*i),
        ConstType::Floating(FloatingType::Float) => "float",
        ConstType::Floating(FloatingType::Double | FloatingType::LongDouble) => "double",
        ConstType::Char => "char",
        ConstType::WideChar => "wchar",
        ConstType::Octet => "ubyte",
        ConstType::Boolean => "bool",
        ConstType::String { wide: false } => "string",
        ConstType::String { wide: true } => "wstring",
        // A `fixed` const has no native D compile-time type; render its decimal
        // as a string constant.
        ConstType::Fixed => "string",
        ConstType::Scoped(_) => return None,
    })
}

/// The D integer type for an IDL integer type.
fn d_int_type(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Int8 => "byte",
        IntegerType::UInt8 => "ubyte",
        IntegerType::Short | IntegerType::Int16 => "short",
        IntegerType::UShort | IntegerType::UInt16 => "ushort",
        IntegerType::Long | IntegerType::Int32 => "int",
        IntegerType::ULong | IntegerType::UInt32 => "uint",
        IntegerType::LongLong | IntegerType::Int64 => "long",
        IntegerType::ULongLong | IntegerType::UInt64 => "ulong",
    }
}

/// Renders a `ConstExpr` as a D constant expression, or `None` for a form the
/// D backend does not express (an enum-valued scoped reference).
/// zerodds-lint: recursion-depth 32 (const expression tree; bounded by the IDL
/// grammar's expression nesting).
fn const_expr_to_d(e: &ConstExpr) -> Option<String> {
    match e {
        ConstExpr::Literal(l) => const_literal_to_d(l),
        // An enum-valued or const-alias scoped reference cannot be rendered from
        // the bare last segment; skip (wire-neutral — a wrong D identifier would
        // break the build).
        ConstExpr::Scoped(_) => None,
        ConstExpr::Unary { op, operand, .. } => {
            let v = const_expr_to_d(operand)?;
            let o = match op {
                UnaryOp::Plus => "+",
                UnaryOp::Minus => "-",
                UnaryOp::BitNot => "~",
            };
            Some(format!("{o}{v}"))
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let l = const_expr_to_d(lhs)?;
            let r = const_expr_to_d(rhs)?;
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

/// Renders a literal as valid D source: booleans normalized to `true`/`false`
/// (never a bare `TRUE`/`FALSE`), any wide `L"…"`/`L'…'` prefix stripped, and a
/// `fixed`/float suffix removed.
fn const_literal_to_d(l: &Literal) -> Option<String> {
    let raw = l.raw.trim();
    Some(match l.kind {
        // D accepts decimal / `0x` / `0b` integer literals as-is.
        LiteralKind::Integer => raw.to_string(),
        // Strip a trailing IDL float/fixed suffix (`d`/`f`/`l`) D rejects.
        LiteralKind::Floating => raw
            .trim_end_matches(['d', 'D', 'f', 'F', 'l', 'L'])
            .to_string(),
        // A `fixed` decimal has no native D type — render as a string.
        LiteralKind::Fixed => format!(
            "\"{}\"",
            raw.trim_end_matches(['d', 'D']).replace('"', "\\\"")
        ),
        // Normalize the IDL boolean keyword to D's `true`/`false` (#A13).
        LiteralKind::Boolean => {
            if raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        // Narrow string/char literals pass through; wide literals drop the
        // `L` prefix (`L"x"`/`L'x'` is not valid D — a D string literal is
        // polysemous and adapts to `wstring`).
        LiteralKind::String | LiteralKind::Char => raw.to_string(),
        LiteralKind::WideString | LiteralKind::WideChar => {
            raw.strip_prefix('L').unwrap_or(raw).to_string()
        }
    })
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
fn emit_enum(out: &mut String, e: &EnumDef, scope: &[String]) {
    let values = enumerator_values(e);
    let ty = escape_d_ident(&qualify(scope, &e.name.text));
    let _ = writeln!(out, "\nenum {ty} : int {{");
    for (en, value) in e.enumerators.iter().zip(&values) {
        let name = escape_d_ident(&en.name.text);
        let _ = writeln!(out, "    {name} = {value},");
    }
    let _ = writeln!(out, "}}");
}

/// Backing-integer storage for a bit container of `total_bits` bits: XTypes 1.3
/// §7.4.7 — the smallest holder that fits (`≤8`→u8, `≤16`→u16, `≤32`→u32, else
/// u64). Returns `(D type, put-method, get-method)`.
fn bit_storage(total_bits: usize) -> (&'static str, &'static str, &'static str) {
    match total_bits {
        0..=8 => ("ubyte", "putU8", "getU8"),
        9..=16 => ("ushort", "putU16", "getU16"),
        17..=32 => ("uint", "putU32", "getU32"),
        _ => ("ulong", "putU64", "getU64"),
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

/// Emits an IDL `bitset` as a D holder struct over its backing integer, with a
/// bit-accessor per named bitfield and an XCDR2 marshal/unmarshal that writes
/// the backing integer (XTypes 1.3 §7.4.7 — wire = backing int).
///
/// # Errors
/// [`IdlDError::Unsupported`] if a bitfield width is not a codegen-time integer.
fn emit_bitset(out: &mut String, b: &BitsetDecl, scope: &[String]) -> Result<()> {
    let mut widths: Vec<usize> = Vec::with_capacity(b.bitfields.len());
    for bf in &b.bitfields {
        let w = array_size(&bf.spec.width)
            .filter(|w| *w >= 0)
            .ok_or_else(|| {
                IdlDError::Unsupported(format!(
                    "non-integer bitfield width in bitset {}",
                    b.name.text
                ))
            })? as usize;
        widths.push(w);
    }
    let total: usize = widths.iter().sum();
    let (storage, put, get) = bit_storage(total);
    let ty = escape_d_ident(&qualify(scope, &b.name.text));

    let _ = writeln!(out, "\nstruct {ty} {{");
    let _ = writeln!(out, "    {storage} storage;");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::BeginDeclaration);
    let mut offset: usize = 0;
    for (bf, width) in b.bitfields.iter().zip(&widths) {
        if let Some(name) = &bf.name {
            let field = escape_d_ident(&name.text);
            if *width == 1 {
                let _ = writeln!(
                    out,
                    "    bool {field}() {{ return ((storage >> {offset}) & 1) != 0; }}"
                );
                let _ = writeln!(
                    out,
                    "    void set_{field}(bool v) {{ auto m = cast({storage})(1) << {offset}; if (v) storage |= m; else storage &= cast({storage}) ~m; }}"
                );
            } else {
                let mask: u128 = if *width >= 128 {
                    u128::MAX
                } else {
                    (1u128 << *width) - 1
                };
                let _ = writeln!(
                    out,
                    "    {storage} {field}() {{ return cast({storage})((storage >> {offset}) & {mask}); }}"
                );
                let _ = writeln!(
                    out,
                    "    void set_{field}({storage} v) {{ auto m = cast({storage})({mask}) << {offset}; storage = cast({storage})((storage & cast({storage}) ~m) | ((v & {mask}) << {offset})); }}"
                );
            }
        }
        offset += width;
    }
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(
        out,
        "\n    void marshalInto(ref Writer w) {{ w.{put}(storage); }}"
    );
    let _ = writeln!(
        out,
        "    ubyte[] marshalXCDR(Endian endian) {{ auto w = Writer(endian); marshalInto(w); return w.bytes(); }}"
    );
    let _ = writeln!(out, "}}");
    let _ = writeln!(out, "\n{ty} unmarshalFrom{ty}(ref Reader r) {{");
    let _ = writeln!(
        out,
        "    {ty} v; v.storage = cast({storage}) r.{get}(); return v;"
    );
    let _ = writeln!(out, "}}");
    let _ = writeln!(
        out,
        "\n{ty} UnmarshalXCDR{ty}(ubyte[] buf, Endian endian) {{"
    );
    let _ = writeln!(
        out,
        "    auto r = Reader(buf, endian); return unmarshalFrom{ty}(r);"
    );
    let _ = writeln!(out, "}}");
    Ok(())
}

/// Emits an IDL `bitmask` as a D holder struct over its `@bit_bound` backing
/// integer (default 32), with an OR-able manifest constant per bit value and an
/// XCDR2 marshal/unmarshal writing the backing integer (XTypes 1.3 §7.4.7).
fn emit_bitmask(out: &mut String, b: &BitmaskDecl, scope: &[String]) {
    let (storage, put, get) = bit_storage(bitmask_bit_bound(&b.annotations) as usize);
    let ty = escape_d_ident(&qualify(scope, &b.name.text));

    let _ = writeln!(out, "\nstruct {ty} {{");
    let _ = writeln!(out, "    {storage} storage;");
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::BeginDeclaration);
    for (idx, v) in b.values.iter().enumerate() {
        let pos = bit_position(&v.annotations).unwrap_or(idx as u32);
        let cname = escape_d_ident(&v.name.text.to_uppercase());
        let _ = writeln!(
            out,
            "    enum {storage} {cname} = cast({storage})(1) << {pos};"
        );
    }
    emit_verbatim_at(out, "    ", &b.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(
        out,
        "\n    void marshalInto(ref Writer w) {{ w.{put}(storage); }}"
    );
    let _ = writeln!(
        out,
        "    ubyte[] marshalXCDR(Endian endian) {{ auto w = Writer(endian); marshalInto(w); return w.bytes(); }}"
    );
    let _ = writeln!(out, "}}");
    let _ = writeln!(out, "\n{ty} unmarshalFrom{ty}(ref Reader r) {{");
    let _ = writeln!(
        out,
        "    {ty} v; v.storage = cast({storage}) r.{get}(); return v;"
    );
    let _ = writeln!(out, "}}");
    let _ = writeln!(
        out,
        "\n{ty} UnmarshalXCDR{ty}(ubyte[] buf, Endian endian) {{"
    );
    let _ = writeln!(
        out,
        "    auto r = Reader(buf, endian); return unmarshalFrom{ty}(r);"
    );
    let _ = writeln!(out, "}}");
}

/// Resolves a `fixed<P,S>`'s digit count `P` and scale `S` to codegen-time
/// integers.
///
/// # Errors
/// [`IdlDError::Unsupported`] if either is not a resolvable non-negative
/// integer literal.
fn fixed_ps(f: &FixedPtType) -> Result<(i64, i64)> {
    let p = array_size(&f.digits)
        .filter(|v| *v > 0)
        .ok_or_else(|| IdlDError::Unsupported("non-integer fixed digit count".to_string()))?;
    let s = array_size(&f.scale)
        .filter(|v| *v >= 0)
        .ok_or_else(|| IdlDError::Unsupported("non-integer fixed scale".to_string()))?;
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

/// Collects `typedef` aliases (simple declarators) as name -> aliased type-spec.
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
/// §7.6.8) for keyHash emission and for the static max-size (MD5 vs.
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

/// Collects a struct's effective members base-first (#A10/F10/P3): the base
/// struct's members (recursively) precede the derived struct's own, so the
/// generated D struct and its wire form carry the inherited fields — matching
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
        d_name: String,
        d_type: String,
        put: String,
        get: String,
        id: u32,
        key: bool,
        resolved: TypeSpec,    // typedef-dealiased type of this field
        simple: bool,          // true for a `Declarator::Simple` (not a fixed array)
        optional: bool,        // `@optional`: uint8 presence flag then value
        must_understand: bool, // `@must_understand`: EMHEADER bit 31 (@mutable)
    }
    // #A10/F10/P3: base-first effective member list (inherited members precede
    // the derived struct's own, in the type and on the wire).
    let mut all_members: Vec<&Member> = Vec::new();
    collect_base_members(s, structs, &mut all_members);

    let mut fields: Vec<FieldGen> = Vec::new();
    let mut next_id: u32 = 0;
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
        for d in &m.declarators {
            let d_name = escape_d_ident(&d.name().text);
            let id = explicit_id.unwrap_or(next_id);
            next_id = id + 1;
            let simple = matches!(d, Declarator::Simple(_));
            let (d_type, put, get) = match d {
                Declarator::Simple(_) => {
                    let (t, put) = map_type(&resolved, &d_name, enum_names, struct_names, 0)?;
                    let get = map_get(
                        &resolved,
                        &format!("v.{d_name}"),
                        enum_names,
                        struct_names,
                        0,
                    )?;
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
                        map_type(&resolved, "$elem", enum_names, struct_names, 0)?;
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
                optional,
                must_understand,
            });
        }
    }

    let ty = escape_d_ident(&qualify(scope, &s.name.text));
    let _ = writeln!(out, "\nstruct {ty} {{");
    // §7.2.2.4.8 — text as the first element inside the declaration.
    emit_verbatim_at(out, "    ", &s.annotations, PlacementKind::BeginDeclaration);
    for f in &fields {
        // An `@optional` member carries a companion presence flag (XTypes 1.3
        // §7.4.5.1.4: uint8 present-flag then the value if present).
        if f.optional {
            let _ = writeln!(out, "    bool {}_present;", f.d_name);
        }
        let _ = writeln!(out, "    {} {};", f.d_type, f.d_name);
    }

    // marshalInto writes into an existing writer (nested composites call this so
    // alignment stays stream-relative). @final: fields inline; @appendable: a
    // DHEADER-framed body.
    let _ = writeln!(out, "\n    void marshalInto(ref Writer w) {{");
    if ext == ExtensibilityKind::Mutable {
        // @mutable: DHEADER-framed member list; each member = EMHEADER (M-bit |
        // LC4 = member id) + NEXTINT (body length) + body (XTypes §7.4.3.4.2).
        let _ = writeln!(out, "        auto zdBody = Writer(w.endian);");
        for f in &fields {
            // An `@optional` member is omitted from the member list when absent
            // (XTypes 1.3 §7.4.3.4.2): guard its EMHEADER+body on the flag.
            if f.optional {
                let _ = writeln!(out, "        if ({}_present) {{", f.d_name);
            }
            // #A17/F17: the must-understand bit (EMHEADER bit 31) is set for a
            // `@must_understand` member so a reader that skips an unknown member
            // still knows it was mandatory. LC4 (bits 30-28) is left as-is
            // (A19 is a coordinated cross-backend change, out of scope here).
            let mu_bit = if f.must_understand {
                0x8000_0000_u32
            } else {
                0
            };
            let emh = mu_bit | 0x4000_0000 | (f.id & 0x0FFF_FFFF);
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
            if f.optional {
                let _ = writeln!(out, "        }}");
            }
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
            let put = f.put.replace("$w", wv);
            if f.optional {
                // uint8 presence flag then the value if present (§7.4.5.1.4).
                let _ = writeln!(
                    out,
                    "        {wv}.putU8({name}_present ? 1 : 0); if ({name}_present) {{ {put} }}",
                    name = f.d_name
                );
            } else {
                let _ = writeln!(out, "        {put}");
            }
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
        // #A10/F10: the KeyHash max-size decision runs over the effective
        // (base-first) member set, so an inherited `@key` member is counted.
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
    // §7.2.2.4.8 — text as the last element inside the declaration.
    emit_verbatim_at(out, "    ", &s.annotations, PlacementKind::EndDeclaration);
    let _ = writeln!(out, "}}");

    // unmarshalFrom / UnmarshalXCDR: the decode side (inverse of marshalInto).
    // An `@optional` member reads its uint8 presence flag, then the value only
    // if present (§7.4.5.1.4).
    let write_get = |out: &mut String, f: &FieldGen| {
        if f.optional {
            let _ = writeln!(
                out,
                "    v.{name}_present = r.getBool(); if (v.{name}_present) {{ {get} }}",
                name = f.d_name,
                get = f.get
            );
        } else {
            let _ = writeln!(out, "    {}", f.get);
        }
    };
    let _ = writeln!(out, "\n{ty} unmarshalFrom{ty}(ref Reader r) {{");
    let _ = writeln!(out, "    {ty} v;");
    match ext {
        ExtensibilityKind::Final => {
            for f in &fields {
                write_get(out, f);
            }
        }
        ExtensibilityKind::Appendable => {
            let _ = writeln!(out, "    r.getU32(); // DHEADER");
            for f in &fields {
                write_get(out, f);
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
            let _ = writeln!(out, "        {}", put.replace("$w", "kw"));
        }
    }
    Ok(())
}

/// Resolves a union case label to its integer discriminant (#A11/A12/A13 /
/// F11/F12/F13). Beyond integer literals it handles `char`/`wchar` literals
/// (codepoint), boolean literals (`TRUE`/`FALSE` → 1/0), and `case ENUMERATOR:`
/// labels (the enumerator's value, via `enum_vals` name → value of the switch
/// enum). Mirrors `idl-go`'s `eval_union_label`.
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

/// Decodes an IDL `char`/`wchar` literal (`'A'`, `'\n'`, `'\x41'`, `L'A'`) to
/// its codepoint. Mirrors `idl-go`'s `char_literal_value`.
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
            'x' => return i64::from_str_radix(it.as_str(), 16).ok(),
            _ => return None,
        };
        Some(v)
    } else {
        Some(i64::from(u32::from(c)))
    }
}

/// Emits an IDL `union` as a discriminated holder + a `marshalInto` that puts
/// the discriminator then dispatches on it to the selected member (XCDR2
/// §7.4.3.5.4). `@final`: inline; `@appendable`: a DHEADER-framed body;
/// `@mutable`: an EMHEADER-framed member list (discriminator = member id 0,
/// each branch = its 1-based id — #A16/F14), wrapped in the struct's DHEADER.
#[allow(clippy::too_many_arguments)]
fn emit_union(
    out: &mut String,
    u: &UnionDef,
    scope: &[String],
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    enum_defs: &HashMap<String, &EnumDef>,
    typedefs: &HashMap<String, TypeSpec>,
) -> Result<()> {
    // Member references resolve against this union's module scope.
    CURRENT_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
    let ext = lower_annotations(&u.annotations)
        .ok()
        .and_then(|l| l.extensibility())
        .unwrap_or(ExtensibilityKind::Appendable);
    let disc_ts = switch_typespec(&u.switch_type);
    let (disc_type, disc_put) = map_type(&disc_ts, "disc", enum_names, struct_names, 0)?;
    let disc_get = map_get(&disc_ts, "v.disc", enum_names, struct_names, 0)?;

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
        let field = escape_d_ident(&c.element.declarator.name().text);
        let resolved = resolve_typedef(&c.element.type_spec, typedefs);
        let (ty, put) = map_type(&resolved, &field, enum_names, struct_names, 0)?;
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
                        IdlDError::Unsupported(format!(
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

    // A boolean discriminator switches on `true`/`false`, not integers (`disc`
    // is a D `bool`); every other discriminator is an integer/enum/char that
    // compares against the integer label directly (#A13/F13).
    let disc_is_bool = matches!(u.switch_type, SwitchTypeSpec::Boolean);
    let render_label = |l: i64| -> String {
        if disc_is_bool {
            (l != 0).to_string()
        } else {
            l.to_string()
        }
    };

    let ty = escape_d_ident(&qualify(scope, &u.name.text));
    let _ = writeln!(out, "\nstruct {ty} {{");
    emit_verbatim_at(out, "    ", &u.annotations, PlacementKind::BeginDeclaration);
    let _ = writeln!(out, "    {disc_type} disc;");
    for c in &cases {
        let _ = writeln!(out, "    {} {};", c.ty, c.field);
    }
    let _ = writeln!(out, "\n    void marshalInto(ref Writer w) {{");
    if ext == ExtensibilityKind::Mutable {
        // #A16/F14: EMHEADER-framed member list — discriminator is member id 0,
        // each branch its 1-based id, wrapped in the struct's DHEADER.
        let _ = writeln!(out, "        auto zdBody = Writer(w.endian);");
        write_mutable_member(out, "        ", 0, &disc_put);
        for (i, c) in cases.iter().enumerate() {
            let (kw, cond) = branch_head(i, c, &render_label, "disc");
            let _ = writeln!(out, "        {kw}{cond} {{");
            let id = u32::try_from(i + 1).unwrap_or(0);
            write_mutable_member(out, "            ", id, &c.put);
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
        let _ = writeln!(out, "        {}", disc_put.replace("$w", wv));
        for (i, c) in cases.iter().enumerate() {
            let (kw, cond) = branch_head(i, c, &render_label, "disc");
            let _ = writeln!(out, "        {kw}{cond} {{");
            let _ = writeln!(out, "            {}", c.put.replace("$w", wv));
            let _ = writeln!(out, "        }}");
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
    emit_verbatim_at(out, "    ", &u.annotations, PlacementKind::EndDeclaration);
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
    if ext == ExtensibilityKind::Mutable {
        // The mutable path is positional: read the discriminator's EMHEADER +
        // NEXTINT + value, switch, then read the one selected branch's frame.
        let _ = writeln!(out, "    r.getU32(); // EMHEADER");
        let _ = writeln!(out, "    r.getU32(); // NEXTINT");
        let _ = writeln!(out, "    {disc_get}");
        for (i, c) in cases.iter().enumerate() {
            let (kw, cond) = branch_head(i, c, &render_label, "v.disc");
            let _ = writeln!(out, "    {kw}{cond} {{");
            let _ = writeln!(out, "        r.getU32(); // EMHEADER");
            let _ = writeln!(out, "        r.getU32(); // NEXTINT");
            let _ = writeln!(out, "        {}", c.get);
            let _ = writeln!(out, "    }}");
        }
    } else {
        let _ = writeln!(out, "    {disc_get}");
        for (i, c) in cases.iter().enumerate() {
            let (kw, cond) = branch_head(i, c, &render_label, "v.disc");
            let _ = writeln!(out, "    {kw}{cond} {{ {} }}", c.get);
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

/// Builds a union branch head: `("if"|"else if", " (cond)")` for a value case,
/// or `("else", "")` for the `default` case. `disc` is the discriminator
/// expression (`disc` on encode, `v.disc` on decode).
fn branch_head(
    i: usize,
    c: &UnionCase,
    render_label: &impl Fn(i64) -> String,
    disc: &str,
) -> (&'static str, String) {
    if c.is_default {
        ("else", String::new())
    } else {
        let kw = if i == 0 { "if" } else { "else if" };
        let cond = c
            .labels
            .iter()
            .map(|&l| format!("{disc} == {}", render_label(l)))
            .collect::<Vec<_>>()
            .join(" || ");
        (kw, format!(" ({cond})"))
    }
}

/// Emits one `@mutable` member frame into `zdBody`: the EMHEADER
/// (`LC4 | member id`), then NEXTINT (body length), then the body. `put` is the
/// member's put with `$w` bound to a fresh member writer. The must-understand
/// bit is not set for union members (the discriminator and the single selected
/// branch are always present). LC4 is left as-is (A19 is out of scope).
fn write_mutable_member(out: &mut String, indent: &str, id: u32, put: &str) {
    let emh = 0x4000_0000_u32 | (id & 0x0FFF_FFFF);
    let _ = writeln!(out, "{indent}zdBody.putU32(0x{emh:08x}u);");
    let _ = writeln!(out, "{indent}{{");
    let _ = writeln!(out, "{indent}    auto zdMem = Writer(w.endian);");
    let _ = writeln!(out, "{indent}    {}", put.replace("$w", "zdMem"));
    let _ = writeln!(
        out,
        "{indent}    zdBody.putU32(cast(uint) zdMem.bytes().length);"
    );
    let _ = writeln!(out, "{indent}    zdBody.putBytes(zdMem.bytes());");
    let _ = writeln!(out, "{indent}}}");
}

/// Maps an IDL type to `(D type, put statement)`. The put uses `$w` as the
/// writer placeholder and `expr` as the field name (a struct member).
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

/// Per-nesting-depth suffix for a generated temporary name (#A22/F22): empty at
/// the outermost collection (depth 0, so every existing non-nested golden is
/// unchanged), then `1`, `2`, … one level deeper. Distinct suffixes keep a
/// `sequence<sequence<…>>` / nested-`map` from re-declaring (D-shadowing) the
/// same loop counter / length / key / value name, and stop the inner loop from
/// indexing with the outer loop's variable.
fn depth_suffix(depth: usize) -> String {
    if depth == 0 {
        String::new()
    } else {
        depth.to_string()
    }
}

/// Builds a map put: `u32 count` + key/value pairs sorted ascending by key
/// (DHEADER-framed unless the key/value pair is primitive). A local `import`
/// pulls in `sort` only where a map is emitted. `depth` suffixes the temporary
/// names so a nested map/sequence value does not shadow this level's (#A22/F22).
fn build_map_put(
    expr: &str,
    key_put: &str,
    val_put: &str,
    prim: bool,
    bound: Option<&ConstExpr>,
    depth: usize,
) -> String {
    let s = depth_suffix(depth);
    let head = format!(
        "import std.algorithm.sorting : sort;\n        auto zdKeys{s} = {expr}.keys;\n        zdKeys{s}.sort();"
    );
    // Bounded `map<K,V,N>` (DDS-XTypes §7.4.3): over-bound = encode error.
    let bound_check = match bound.and_then(bound_literal) {
        Some(bv) => format!(
            "\n        if (zdKeys{s}.length > {bv}) throw new Exception(\"bounded map length exceeds its IDL bound ({bv})\");"
        ),
        None => String::new(),
    };
    if prim {
        format!(
            "{{\n        {head}{bound_check}\n        $w.putU32(cast(uint) zdKeys{s}.length);\n        foreach (zdK{s}; zdKeys{s}) {{ {key_put} {val_put} }}\n        }}"
        )
    } else {
        let kp = key_put.replace("$w", &format!("zdSub{s}"));
        let vp = val_put.replace("$w", &format!("zdSub{s}"));
        format!(
            "{{\n        {head}{bound_check}\n        auto zdSub{s} = Writer($w.endian);\n        zdSub{s}.putU32(cast(uint) zdKeys{s}.length);\n        foreach (zdK{s}; zdKeys{s}) {{ {kp} {vp} }}\n        auto zdBB{s} = zdSub{s}.bytes();\n        $w.putU32(cast(uint) zdBB{s}.length);\n        $w.putBytes(zdBB{s});\n        }}"
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
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
        // D field holds the BCD bytes directly; `zdFixedEnc` builds them from a
        // decimal string.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let _ = fixed_ps(f)?; // validate P/S resolve at codegen time
            Ok(("ubyte[]".to_string(), format!("$w.putBytes({expr});")))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            if enum_names.contains(&name) {
                // Enum holder width follows @bit_bound (XTypes 1.3 §7.4.5.1);
                // putU8/putU16 mask internally, so a plain cast(int) suffices.
                let put = match enum_wire_width(&name) {
                    1 => format!("$w.putU8(cast(int) {expr});"),
                    2 => format!("$w.putU16(cast(int) {expr});"),
                    _ => format!("$w.putU32(cast(uint) {expr});"),
                };
                Ok((escape_d_ident(&name), put))
            } else if struct_names.contains(&name) || is_bit_name(&name) {
                Ok((escape_d_ident(&name), format!("{expr}.marshalInto($w);")))
            } else {
                Err(IdlDError::Unsupported(format!("scoped type {name}")))
            }
        }
        // A map: entries sorted ascending by key, `u32 count` + key/value pairs
        // (no DHEADER for a primitive pair; DHEADER-framed otherwise). The key
        // variable and this level's temporaries carry the depth suffix; the
        // key/value themselves recurse one level deeper (#A22/F22).
        TypeSpec::Map(m) => {
            let s = depth_suffix(depth);
            let kvar = format!("zdK{s}");
            let (key_type, key_put) = map_type(&m.key, &kvar, enum_names, struct_names, depth + 1)?;
            let (val_type, val_put) = map_type(
                &m.value,
                &format!("{expr}[{kvar}]"),
                enum_names,
                struct_names,
                depth + 1,
            )?;
            let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
            Ok((
                format!("{val_type}[{key_type}]"),
                build_map_put(expr, &key_put, &val_put, prim, m.bound.as_ref(), depth),
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

/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn map_sequence(
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
    enum_names: &HashSet<String>,
    struct_names: &HashSet<String>,
    depth: usize,
) -> Result<(String, String)> {
    let s = depth_suffix(depth);
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
    // sequence<struct> → collection DHEADER + count + each element. Temporaries
    // carry the depth suffix so a nested sequence-of-sequence-of-struct does not
    // shadow the outer level's `zdElem`/`zdSub` (#A22/F22).
    if let TypeSpec::Scoped(sn) = elem {
        let name = resolve_scoped_name(sn);
        if struct_names.contains(&name) {
            let put = format!(
                "{{ {bound_check}auto zdSub{s} = Writer($w.endian); zdSub{s}.putU32(cast(uint) {expr}.length); foreach (zdElem{s}; {expr}) zdElem{s}.marshalInto(zdSub{s}); $w.putU32(cast(uint) zdSub{s}.bytes().length); $w.putBytes(zdSub{s}.bytes()); }}"
            );
            return Ok((format!("{}[]", escape_d_ident(&name)), put));
        }
    }
    // sequence<arbitrary> → u32 count + per-element encode (no collection
    // DHEADER; the element type is fully descriptive on the wire for the
    // primitive / enum / bitset / bitmask cases handled here). Mirrors the
    // `idl-go` fallback (`idl-go/src/emitter.rs`). The element recurses one
    // level deeper so its own temporaries never collide with `zdElem{s}`.
    let elem_var = format!("zdElem{s}");
    let (elem_ty, elem_put) = map_type(elem, &elem_var, enum_names, struct_names, depth + 1)?;
    let put = format!(
        "{{ {bound_check}$w.putU32(cast(uint) {expr}.length); foreach ({elem_var}; {expr}) {{ {elem_put} }} }}"
    );
    Ok((format!("{elem_ty}[]"), put))
}

/// The inverse of [`map_type`]: a D statement reading from `r` into `target`.
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
        TypeSpec::Sequence(seq) => map_get_sequence(
            &seq.elem,
            seq.bound.as_ref(),
            target,
            enum_names,
            struct_names,
            depth,
        ),
        TypeSpec::Map(m) => map_get_map(m, target, enum_names, struct_names, depth),
        // `fixed<P,S>`: read the statically-known `(P+2)/2` BCD octets.
        TypeSpec::Fixed(f) => {
            USED_FIXED.with(|u| u.set(true));
            let (p, _) = fixed_ps(f)?;
            let n = (p + 2) / 2;
            Ok(format!("{target} = r.getBytesN({n}).dup;"))
        }
        TypeSpec::Scoped(sn) => {
            let name = resolve_scoped_name(sn);
            let esc = escape_d_ident(&name);
            if enum_names.contains(&name) {
                // Read the @bit_bound-wide holder and sign-extend to the enum's
                // int domain via a signed cast (XTypes 1.3 §7.4.5.1).
                let get = match enum_wire_width(&name) {
                    1 => format!("{target} = cast({esc})(cast(int) cast(byte) r.getU8());"),
                    2 => format!("{target} = cast({esc})(cast(int) cast(short) r.getU16());"),
                    _ => format!("{target} = cast({esc}) r.getU32();"),
                };
                Ok(get)
            } else if struct_names.contains(&name) || is_bit_name(&name) {
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
    depth: usize,
) -> Result<String> {
    let s = depth_suffix(depth);
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
    let (elem_ty, _) = map_type(elem, "e", enum_names, struct_names, depth + 1)?;
    // The loop counter `i{s}` and length `zdn{s}` are depth-suffixed, and the
    // element is indexed with THIS level's counter, so a nested sequence uses a
    // distinct counter and never re-indexes with the outer one (#A22/F22).
    let elem_get = map_get(
        elem,
        &format!("{target}[i{s}]"),
        enum_names,
        struct_names,
        depth + 1,
    )?;
    let dheader = if matches!(elem, TypeSpec::Scoped(sn)
        if struct_names.contains(&resolve_scoped_name(sn)))
    {
        "r.getU32(); "
    } else {
        ""
    };
    let bound_check = match &bv {
        Some(bv) => format!(
            "if (zdn{s} > {bv}) throw new Exception(\"decoded sequence length exceeds its IDL bound ({bv})\"); "
        ),
        None => String::new(),
    };
    Ok(format!(
        "{{ {dheader}size_t zdn{s} = r.getU32(); {bound_check}{target} = new {elem_ty}[zdn{s}]; \
         foreach (i{s}; 0 .. zdn{s}) {{ {elem_get} }} }}"
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
    let s = depth_suffix(depth);
    let (key_ty, _) = map_type(&m.key, "zk", enum_names, struct_names, depth + 1)?;
    let (val_ty, _) = map_type(&m.value, "zv", enum_names, struct_names, depth + 1)?;
    let prim = is_primitive(&m.key, enum_names) && is_primitive(&m.value, enum_names);
    // This level's key/value holders (`zk{s}`/`zv{s}`), counter (`i{s}`) and
    // length (`zdn{s}`) are depth-suffixed; the key/value decoders recurse one
    // level deeper so a nested map/sequence value never shadows them (#A22/F22).
    let key_get = map_get(
        &m.key,
        &format!("zk{s}"),
        enum_names,
        struct_names,
        depth + 1,
    )?;
    let val_get = map_get(
        &m.value,
        &format!("zv{s}"),
        enum_names,
        struct_names,
        depth + 1,
    )?;
    let dheader = if prim { "" } else { "r.getU32(); " };
    // Bounded `map<K,V,N>`: mirror the encode-side check. §7.4.3.
    let bound_check = match m.bound.as_ref().and_then(bound_literal) {
        Some(bv) => format!(
            "if (zdn{s} > {bv}) throw new Exception(\"decoded map length exceeds its IDL bound ({bv})\"); "
        ),
        None => String::new(),
    };
    Ok(format!(
        "{{ {dheader}size_t zdn{s} = r.getU32(); {bound_check}foreach (i{s}; 0 .. zdn{s}) {{ {key_ty} zk{s}; {val_ty} zv{s}; \
         {key_get} {val_get} {target}[zk{s}] = zv{s}; }} }}"
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
    let mut body = map_get(elem, &format!("{field}{idx}"), enum_names, struct_names, 0)?;
    for k in (0..sizes.len()).rev() {
        body = format!(
            "for (size_t zdi{k} = 0; zdi{k} < {}; zdi{k}++) {{ {body} }}",
            sizes[k]
        );
    }
    Ok(body)
}
