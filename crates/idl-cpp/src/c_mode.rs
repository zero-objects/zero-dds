// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! IDL → C99 codegen mode (vendor spec `zerodds-xcdr2-c-1.0`).
//!
//! For each IDL specification, this module emits:
//! - C99 `typedef struct` definitions per `struct` (with a module prefix
//!   in the type name, because C99 has no namespaces).
//! - Static `zerodds_typesupport_t` tables with XCDR2 encoder/
//!   decoder/key-hash/free function pointers.
//! - Inline body implementations of the encoders/decoders that produce
//!   the XCDR2 wire format (XTypes 1.3 §7.4) byte-exactly.
//!
//! ## Scope
//!
//! Supported:
//! - Structs with `@final`/`@appendable`/`@mutable` extensibility.
//! - Primitive types (boolean, octet, short/long/long long + unsigned,
//!   float, double).
//! - `string` (unbounded).
//! - `sequence<T>` (unbounded; nested sequences allowed).
//! - Nested modules → type-name prefix with `::` (cross-language
//!   convention) and identifier prefix with `_` (C99-conformant).
//! - `@key` members → key-hash routine via `PlainCdr2BeKeyHolder`.
//! - `@id(N)` for @mutable.
//! - **Enums** → `int32_t` alias + prefixed enumerator constants (Bug C).
//! - **Typedefs** → resolved to the aliased type at every reference (Bug C).
//! - **Nested struct** members → inline-embedded by value (Bug C).
//! - **Fixed arrays** (`T x[N][M]…`) → C array, no length prefix (Bug C).
//! - **`@optional`** members → `<name>_present` companion flag + a wire
//!   presence boolean (Bug C2; no longer flattened to mandatory).
//! - **Array bound by named constant** (`long v[N]`) → the bound is resolved
//!   through the const symbol table (#43, const-array-bound).
//! - **`sequence<T>`** for primitive, string, enum, nested struct and nested
//!   `sequence<…>` element types (#43, sequence widening).
//! - **`map<K,V>`** → parallel `keys[]`/`vals[]` arrays + DHEADER/count codec
//!   (#43, map).
//! - **`wstring`** → NUL-terminated `uint16_t*`, UTF-16-LE on the wire
//!   (XTypes §7.4.4.6) (#43, wstring).
//! - **Discriminated `union`** → C tagged union `struct { D _d; union {…} _u; }`
//!   with a switch-on-discriminator encode/decode (#43, union). A top-level
//!   union additionally gets a `TypeSupport` so it can be a topic type.
//! - **`typedef`-to-aggregate** (alias of a struct / sequence / map) → resolved
//!   to the aliased aggregate at every reference (#43, typedef-to-aggregate).
//! - **typedef ALIAS CHAIN + typedef-to-array** (`typedef A B; typedef long
//!   M[3][3];`) → resolved through the chain to the root type; an array-alias
//!   contributes its dims at the use site (#43, typedefs).
//! - **`bitmask`** → smallest holder uint typedef (default `@bit_bound(32)` →
//!   `uint32_t`) + `<LABEL>` flag-bit constants; **`bitset`** → packed holder
//!   uint typedef + per-field SHIFT/MASK accessor macros. Both serialize as
//!   their holder integer (XTypes §7.3.1.2) (#43, bitset/bitmask).
//! - **Self-/mutually-recursive types** (self-referential through a sequence,
//!   or mutual `@external`) → a heap-indirected C type (pointer-to-tag element)
//!   plus a runtime `_write_body`/`_read_body` helper that splices the recursion
//!   at RUNTIME (XTypes §7.4.5) (#43, recursion / forward-decl).
//! - **Forward-declared** struct/union (then defined) → aggregate typedefs are
//!   emitted in by-value dependency order so a later definition is reachable.
//!
//! Out of scope (errors at the codegen level, honest partial):
//! - `fixed<M,N>` decimal, `any`, `long double` — these legitimately stay out
//!   of the C profile (no native C fixed-decimal / variant type).
//! - A genuinely **infinite-size** type (a by-value self/mutual cycle, e.g.
//!   `struct Node { Node n; };`) is rejected cleanly — the only legal
//!   self-reference is heap-indirected (through a sequence).
//! - `@optional` *inside* a nested struct, nested `@appendable`/`@mutable`
//!   struct splice (only the inline `@final` form is wired), nested-aggregate
//!   union arms inside a `@mutable` member, and freeing of heap-owning members
//!   reached only through a fixed array.
//!
//! ## Invocation
//!
//! ```rust
//! use zerodds_idl::config::ParserConfig;
//! use zerodds_idl_cpp::{generate_c_header, CGenOptions};
//!
//! let ast = zerodds_idl::parse("@final struct Point { long x; long y; };",
//!                              &ParserConfig::default()).unwrap();
//! let header = generate_c_header(&ast, &CGenOptions::default()).unwrap();
//! assert!(header.contains("typedef struct Point_s"));
//! assert!(header.contains("Point_typesupport"));
//! ```

#![allow(clippy::module_name_repetitions)]

use core::fmt::Write as _;
use std::collections::{BTreeMap, BTreeSet};

use zerodds_idl::ast::{
    Annotation, AnnotationParams, BitmaskDecl, BitsetDecl, Case, CaseLabel, ConstExpr,
    ConstrTypeDecl, Declarator, Definition, EnumDef, FloatingType, IntegerType, LiteralKind,
    ModuleDef, PrimitiveType, ScopedName, Specification, StructDcl, StructDef, SwitchTypeSpec,
    TypeDecl, TypeSpec, UnionDcl, UnionDef,
};
use zerodds_idl::semantics::const_eval::{Symbol, SymbolTable, evaluate, evaluate_positive_int};

use crate::error::CppGenError;

/// Codegen-scoped type registry for the C backend (Bug C scope widening).
/// Resolves a `Scoped` member to its referent kind so the emitter can pick the
/// right C type name + XCDR2 codec instead of rejecting all non-flat members.
#[derive(Default)]
struct TypeReg {
    /// IDL simple enum name → its C identifier (module-prefixed).
    enums: BTreeMap<String, String>,
    /// IDL simple enum name → signed wire holder width in BYTES (1/2/4) from
    /// `@bit_bound` (XTypes §7.4.5.1): N≤8 → 1, N≤16 → 2, else 4. The C in-memory
    /// type stays `int32_t`; only the wire write/read narrows.
    enum_bytes: BTreeMap<String, u32>,
    /// IDL simple typedef name → the aliased TypeSpec.
    typedefs: BTreeMap<String, TypeSpec>,
    /// IDL simple typedef name → its declarator array dimensions, for a
    /// `typedef long Matrix3[3][3];` style array-alias (#43, typedef-to-array).
    /// A reference to such a typedef inherits these dims at the use site.
    typedef_arrays: BTreeMap<String, Vec<u64>>,
    /// IDL simple bitmask name → (C identifier, holder uint C type). A bitmask
    /// serializes as its smallest holder uint (XTypes §7.3.1.2.2, default
    /// `@bit_bound(32)` → `uint32_t`).
    bitmasks: BTreeMap<String, (String, &'static str)>,
    /// IDL simple bitset name → (C identifier, packed holder uint C type). A
    /// bitset serializes as one packed integer sized to the sum of its bitfield
    /// widths (XTypes §7.3.1.2.1).
    bitsets: BTreeMap<String, (String, &'static str)>,
    /// IDL simple struct name → its C identifier (module-prefixed) + def.
    structs: BTreeMap<String, (String, StructDef)>,
    /// IDL simple union name → its C identifier (module-prefixed) + def.
    unions: BTreeMap<String, (String, UnionDef)>,
    /// Struct/union simple names that participate in a reference cycle
    /// (self-referential through a sequence, or mutually recursive). Members of
    /// this set are spliced via a generated `_write_body`/`_read_body` helper
    /// (runtime recursion) instead of being inline-emitted, which would recurse
    /// infinitely at codegen time (XTypes §7.4.5 / Bug G, C backend).
    recursive: BTreeSet<String>,
    /// Constant symbol table, keyed by BOTH the fully-qualified path
    /// (`Mod::Sub::N`) and the simple name (`N`), so an array bound written
    /// as a named constant (`long v[N]`) resolves to its literal the way every
    /// other backend does (Bug C #43, const-array-bound). Enumerator ordinals
    /// are folded in too (case labels / bounds frequently reference them).
    consts: SymbolTable,
}

impl TypeReg {
    fn build(defs: &[Definition]) -> Self {
        let mut r = TypeReg::default();
        collect_types(defs, &[], &mut r);
        r.compute_recursive();
        r
    }

    /// Compute the set of struct/union names that participate in a reference
    /// cycle. An aggregate is recursive if it can reach itself by following
    /// member references — directly, or (the only legal self-reference) through
    /// a sequence/map element, a union arm, or a typedef alias. Such types must
    /// be spliced via a runtime helper, not inline-emitted (codegen would
    /// otherwise recurse forever — Bug G for the C backend).
    fn compute_recursive(&mut self) {
        let names: Vec<String> = self
            .structs
            .keys()
            .chain(self.unions.keys())
            .cloned()
            .collect();
        let mut recursive = BTreeSet::new();
        for start in &names {
            let mut seen = BTreeSet::new();
            if self.reaches(start, start, &mut seen) {
                recursive.insert(start.clone());
            }
        }
        self.recursive = recursive;
    }

    /// True if aggregate `from` can reach `target` by following member /
    /// element / arm references. `seen` guards the traversal itself against
    /// cycles. zerodds-lint: recursion-depth 256 (bounded by distinct type set)
    fn reaches(&self, from: &str, target: &str, seen: &mut BTreeSet<String>) -> bool {
        if !seen.insert(from.to_string()) {
            return false;
        }
        let mut refs: Vec<String> = Vec::new();
        if let Some((_, sdef)) = self.structs.get(from) {
            for m in &sdef.members {
                self.collect_aggregate_refs(&m.type_spec, &mut refs);
            }
        }
        if let Some((_, udef)) = self.unions.get(from) {
            self.collect_aggregate_refs(&switch_type_spec(&udef.switch_type), &mut refs);
            for c in &udef.cases {
                self.collect_aggregate_refs(&c.element.type_spec, &mut refs);
            }
        }
        for r in refs {
            if r == target {
                return true;
            }
            if self.reaches(&r, target, seen) {
                return true;
            }
        }
        false
    }

    /// Collect the simple names of struct/union aggregates referenced by a
    /// type-spec (through sequences, maps, and typedef aliases). Enums, bitsets,
    /// bitmasks and primitives terminate (they cannot close a cycle).
    /// zerodds-lint: recursion-depth 64 (bounded by AST nesting)
    fn collect_aggregate_refs(&self, ts: &TypeSpec, out: &mut Vec<String>) {
        let resolved = resolve_alias(self, ts);
        match &resolved {
            TypeSpec::Scoped(sc) => {
                if let Some(last) = scoped_last(sc) {
                    if self.structs.contains_key(&last) || self.unions.contains_key(&last) {
                        out.push(last);
                    }
                }
            }
            TypeSpec::Sequence(seq) => self.collect_aggregate_refs(&seq.elem, out),
            TypeSpec::Map(m) => {
                self.collect_aggregate_refs(&m.key, out);
                self.collect_aggregate_refs(&m.value, out);
            }
            _ => {}
        }
    }

    fn is_recursive(&self, name: &str) -> bool {
        self.recursive.contains(name)
    }
}

/// zerodds-lint: recursion-depth 64 (module/type tree; bounded by IDL nesting)
fn collect_types(defs: &[Definition], scope: &[String], r: &mut TypeReg) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let mut s = scope.to_vec();
                s.push(m.name.text.clone());
                collect_types(&m.definitions, &s, r);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                r.enums
                    .insert(e.name.text.clone(), c_identifier(scope, &e.name.text));
                let ebound = extract_int_annotation_c(&e.annotations, "bit_bound")
                    .filter(|&v| (1..=32).contains(&v))
                    .unwrap_or(32);
                let ebytes = if ebound <= 8 {
                    1
                } else if ebound <= 16 {
                    2
                } else {
                    4
                };
                r.enum_bytes.insert(e.name.text.clone(), ebytes);
                // Register enumerator ordinals (both simple + scoped) so a
                // const-expression that references an enumerator (array bound or
                // union case label) resolves.
                let type_name = scope_join(scope, &e.name.text, "::");
                for (i, en) in e.enumerators.iter().enumerate() {
                    let sym = Symbol::EnumValue {
                        type_name: type_name.clone(),
                        value: i32::try_from(i).unwrap_or(0),
                    };
                    r.consts.insert(en.name.text.clone(), sym.clone());
                    r.consts.insert(scope_join(scope, &en.name.text, "::"), sym);
                }
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                r.structs.insert(
                    s.name.text.clone(),
                    (c_identifier(scope, &s.name.text), s.clone()),
                );
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                r.unions.insert(
                    u.name.text.clone(),
                    (c_identifier(scope, &u.name.text), u.clone()),
                );
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) => {
                let holder = bitmask_holder_c_type(b);
                r.bitmasks.insert(
                    b.name.text.clone(),
                    (c_identifier(scope, &b.name.text), holder),
                );
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
                let holder = bitset_holder_c_type(b);
                r.bitsets.insert(
                    b.name.text.clone(),
                    (c_identifier(scope, &b.name.text), holder),
                );
            }
            Definition::Type(TypeDecl::Typedef(td)) => {
                for decl in &td.declarators {
                    match decl {
                        Declarator::Simple(n) => {
                            r.typedefs.insert(n.text.clone(), td.type_spec.clone());
                        }
                        Declarator::Array(arr) => {
                            // `typedef long Matrix3[3][3];` — the alias carries
                            // both the element type AND fixed dims; a use site
                            // inherits the dims (#43, typedef-to-array).
                            r.typedefs
                                .insert(arr.name.text.clone(), td.type_spec.clone());
                            let dims: Vec<u64> = arr
                                .sizes
                                .iter()
                                .map(|sz| const_expr_to_u64_pre(&r.consts, sz).unwrap_or(0))
                                .collect();
                            r.typedef_arrays.insert(arr.name.text.clone(), dims);
                        }
                    }
                }
            }
            Definition::Const(c) => {
                // Resolve the const value against whatever's already collected
                // (forward refs to later consts are rare for array bounds, and
                // we make a best-effort pass).
                if let Ok(v) = evaluate(&c.value, &r.consts) {
                    r.consts
                        .insert(c.name.text.clone(), Symbol::Const(v.clone()));
                    r.consts
                        .insert(scope_join(scope, &c.name.text, "::"), Symbol::Const(v));
                }
            }
            _ => {}
        }
    }
}

/// Resolve a TypeSpec through typedef chains to its effective type.
/// zerodds-lint: recursion-depth 16
fn resolve_alias(reg: &TypeReg, ts: &TypeSpec) -> TypeSpec {
    let mut cur = ts.clone();
    for _ in 0..16 {
        let TypeSpec::Scoped(s) = &cur else { break };
        let Some(last) = s.parts.last() else { break };
        let Some(aliased) = reg.typedefs.get(&last.text).cloned() else {
            break;
        };
        cur = aliased;
    }
    cur
}

fn scoped_last(s: &ScopedName) -> Option<String> {
    s.parts.last().map(|p| p.text.clone())
}

fn unsupported(kind: &'static str) -> CppGenError {
    CppGenError::UnsupportedConstruct {
        construct: kind.to_string(),
        context: None,
    }
}

/// Codegen options (parallel to `CppGenOptions`).
#[derive(Debug, Clone, Default)]
pub struct CGenOptions {
    /// Optional include-guard name (default: `ZERODDS_GENERATED_H`).
    pub include_guard: Option<String>,
    /// Optional file-header comment.
    pub file_header: Option<String>,
}

/// Produces a complete C99 header from an IDL specification.
///
/// # Errors
/// - [`CppGenError::UnsupportedConstruct`]: out-of-scope IDL constructs that
///   legitimately stay outside the C profile (`fixed<M,N>` decimal, `any`,
///   `long double`), or a genuinely infinite-size by-value self/mutual
///   recursion. Structs, enums, typedefs (incl. alias chains + to-array),
///   bitsets/bitmasks, arrays (literal or const-bound), sequences, maps,
///   `wstring`, discriminated unions, forward declarations and
///   sequence-indirected recursive types are all supported.
pub fn generate_c_header(ast: &Specification, opts: &CGenOptions) -> Result<String, CppGenError> {
    let reg = TypeReg::build(&ast.definitions);
    let mut ctx = Ctx::new(opts, &reg);
    ctx.emit_preamble();
    // Emit enum typedefs first (referenced by struct fields as int32 aliases).
    ctx.emit_enums(&ast.definitions, &[]);
    // Bitset/bitmask holder typedefs + bitmask position constants (#43).
    ctx.emit_bits(&ast.definitions, &[]);
    // Aggregate (struct + union) typedefs in by-value dependency order, so a
    // by-value embed sees its referent's complete C type, and a recursive type's
    // body helper sees its own typedef (#43, recursion / forward-decl). A
    // recursive self-reference is always behind a pointer (sequence element) so
    // it does not constrain the order.
    ctx.emit_all_aggregate_typedefs(&ast.definitions)?;
    // Forward-declare + define the runtime body helpers for recursive types
    // (so a self-/mutually-recursive reference becomes a runtime call instead of
    // an infinite codegen recursion — Bug G for the C backend).
    ctx.emit_recursive_helpers()?;
    ctx.walk_definitions(&ast.definitions, &[])?;
    ctx.emit_postamble();
    Ok(ctx.out)
}

// ============================================================================
// Internals
// ============================================================================

struct Ctx<'a> {
    out: String,
    opts: &'a CGenOptions,
    reg: &'a TypeReg,
    /// Nesting depth of collection (sequence/map) element loops, so each loop
    /// gets a unique counter variable (`i0`, `i1`, …). Without this, a
    /// `sequence<sequence<T>>` reuses `i` and the inner loop shadows the outer
    /// index, corrupting addressing (segfault). Bumped around each element body.
    coll_depth: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Extensibility {
    Final,
    Appendable,
    Mutable,
}

impl<'a> Ctx<'a> {
    fn new(opts: &'a CGenOptions, reg: &'a TypeReg) -> Self {
        Self {
            out: String::new(),
            opts,
            reg,
            coll_depth: 0,
        }
    }

    /// Emit each IDL enum as a C `enum` + an int32 typedef so struct fields can
    /// reference it by name and the codec can treat it as int32 (Spec §7.4.1.4.2).
    /// zerodds-lint: recursion-depth 64 (module tree; bounded by IDL nesting)
    fn emit_enums(&mut self, defs: &[Definition], scope: &[String]) {
        for d in defs {
            match d {
                Definition::Module(m) => {
                    let mut s = scope.to_vec();
                    s.push(m.name.text.clone());
                    self.emit_enums(&m.definitions, &s);
                }
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                    self.emit_enum(e, scope);
                }
                _ => {}
            }
        }
    }

    fn emit_enum(&mut self, e: &EnumDef, scope: &[String]) {
        let c_name = c_identifier(scope, &e.name.text);
        let _ = writeln!(self.out, "typedef int32_t {c_name}_t;");
        for (i, en) in e.enumerators.iter().enumerate() {
            // Enumerator constants are module+enum prefixed to stay unique in C's
            // flat namespace.
            let _ = writeln!(
                self.out,
                "enum {{ {c_name}_{label} = {i} }};",
                label = en.name.text
            );
        }
        let _ = writeln!(self.out);
    }

    /// Emit each IDL bitset/bitmask as a C holder-integer typedef. A bitmask
    /// additionally gets a `<C>_<LABEL>` flag-bit constant per value; a bitset
    /// gets `<C>_<field>_SHIFT` / `<C>_<field>_MASK` accessor macros (XTypes
    /// §7.3.1.2). The wire form is the holder integer in both cases (#43).
    /// zerodds-lint: recursion-depth 64 (module tree; bounded by IDL nesting)
    fn emit_bits(&mut self, defs: &[Definition], scope: &[String]) {
        for d in defs {
            match d {
                Definition::Module(m) => {
                    let mut s = scope.to_vec();
                    s.push(m.name.text.clone());
                    self.emit_bits(&m.definitions, &s);
                }
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) => {
                    self.emit_bitmask(b, scope);
                }
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
                    self.emit_bitset(b, scope);
                }
                _ => {}
            }
        }
    }

    fn emit_bitmask(&mut self, b: &BitmaskDecl, scope: &[String]) {
        let c_name = c_identifier(scope, &b.name.text);
        let holder = bitmask_holder_c_type(b);
        let _ = writeln!(self.out, "typedef {holder} {c_name}_t;");
        let mut next_pos: u32 = 0;
        for v in &b.values {
            let pos = extract_int_annotation_c(&v.annotations, "position").unwrap_or(next_pos);
            next_pos = pos.saturating_add(1);
            // Flag bit value (`1 << position`) as a C enum constant — keeps it a
            // compile-time constant of the holder width.
            let _ = writeln!(
                self.out,
                "enum {{ {c_name}_{label} = (1u << {pos}) }};",
                label = v.name.text
            );
        }
        let _ = writeln!(self.out);
    }

    fn emit_bitset(&mut self, b: &BitsetDecl, scope: &[String]) {
        let c_name = c_identifier(scope, &b.name.text);
        let holder = bitset_holder_c_type(b);
        let _ = writeln!(self.out, "typedef {holder} {c_name}_t;");
        // Per named bitfield: SHIFT (offset) + MASK accessor macros so a caller
        // can pack/unpack the field within the holder integer.
        let mut offset: u32 = 0;
        for f in &b.bitfields {
            let width = match &f.spec.width {
                ConstExpr::Literal(l) if l.kind == LiteralKind::Integer => {
                    l.raw.trim().parse::<u32>().unwrap_or(0)
                }
                _ => 0,
            };
            if let Some(name) = &f.name {
                let mask: u64 = if width >= 64 {
                    u64::MAX
                } else {
                    (1u64 << width) - 1
                };
                let _ = writeln!(
                    self.out,
                    "enum {{ {c_name}_{field}_SHIFT = {offset} }};",
                    field = name.text
                );
                let _ = writeln!(
                    self.out,
                    "#define {c_name}_{field}_MASK 0x{mask:X}u",
                    field = name.text
                );
            }
            offset = offset.saturating_add(width);
        }
        let _ = writeln!(self.out);
    }

    /// Emit one IDL union as a C tagged union `typedef struct { D _d; union
    /// { ... } _u; }` so struct members can embed it by value and the codec can
    /// switch on `_d` (#43, union). Called from the aggregate-typedef pre-pass.
    fn emit_union_typedef(&mut self, u: &UnionDef, scope: &[String]) -> Result<(), CppGenError> {
        let c_name = c_identifier(scope, &u.name.text);
        let disc_ts = switch_type_spec(&u.switch_type);
        let disc_c = c_type_for(self.reg, &disc_ts)?;
        let _ = writeln!(self.out, "typedef struct {c_name}_s {{");
        let _ = writeln!(self.out, "    {disc_c} _d;");
        let _ = writeln!(self.out, "    union {{");
        for case in &u.cases {
            let field = union_case_field(case);
            let c_type = c_type_for(self.reg, &case.element.type_spec)?;
            let dims =
                effective_array_dims(self.reg, &case.element.type_spec, &case.element.declarator)?;
            if dims.is_empty() {
                let _ = writeln!(self.out, "        {c_type} {field};");
            } else {
                let suffix: String = dims.iter().map(|n| format!("[{n}]")).collect();
                let _ = writeln!(self.out, "        {c_type} {field}{suffix};");
            }
        }
        let _ = writeln!(self.out, "    }} _u;");
        let _ = writeln!(self.out, "}} {c_name}_t;");
        let _ = writeln!(self.out);
        Ok(())
    }

    /// Emit encode/decode/free/typesupport for a TOP-LEVEL union (so it can be
    /// a topic type). XCDR2 unions are appendable by default → DHEADER wrap.
    fn emit_union_typesupport(
        &mut self,
        u: &UnionDef,
        scope: &[String],
    ) -> Result<(), CppGenError> {
        let c_name = c_identifier(scope, &u.name.text);
        let dds_name = dds_type_name(scope, &u.name.text);
        let ext = extensibility_of(&u.annotations);

        let _ = writeln!(
            self.out,
            "static int {c_name}_encode(const void* sample, uint8_t* out_buf, size_t out_cap, size_t* out_len);"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode(const uint8_t* buf, size_t len, void* out_sample);\nstatic int {c_name}_decode_e(const uint8_t* buf, size_t len, void* out_sample, int zd_be);"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode_repr(const uint8_t* buf, size_t len, uint8_t representation, void* out_sample);"
        );
        let _ = writeln!(self.out, "static void {c_name}_sample_free(void* sample);");
        let _ = writeln!(self.out);
        let _ = writeln!(
            self.out,
            "static const char {c_name}_type_name[] = \"{dds_name}\";"
        );
        let _ = writeln!(
            self.out,
            "static const zerodds_typesupport_t {c_name}_typesupport = {{"
        );
        let _ = writeln!(self.out, "    .type_hash = {{0}},");
        let _ = writeln!(self.out, "    .type_name = {c_name}_type_name,");
        let _ = writeln!(self.out, "    .is_keyed = 0,");
        let _ = writeln!(self.out, "    .extensibility = {},", ext.as_u8());
        let _ = writeln!(self.out, "    ._reserved = {{0}},");
        let _ = writeln!(self.out, "    .encode = {c_name}_encode,");
        let _ = writeln!(self.out, "    .decode = {c_name}_decode,");
        let _ = writeln!(self.out, "    .key_hash = NULL,");
        let _ = writeln!(self.out, "    .sample_free = {c_name}_sample_free,");
        let _ = writeln!(self.out, "    .decode_repr = {c_name}_decode_repr,");
        let _ = writeln!(self.out, "}};");
        let _ = writeln!(self.out);

        // ---- encode body ----
        let _ = writeln!(
            self.out,
            "static int {c_name}_encode_repr(const void* sample, uint8_t* out_buf, size_t out_cap, size_t* out_len, int representation);"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_encode(const void* sample, uint8_t* out_buf, size_t out_cap, size_t* out_len) {{ return {c_name}_encode_repr(sample, out_buf, out_cap, out_len, 1); }}"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_encode_repr(const void* sample, uint8_t* out_buf, size_t out_cap, size_t* out_len, int representation) {{"
        );
        let _ = writeln!(
            self.out,
            "    const {c_name}_t* s = (const {c_name}_t*)sample;"
        );
        let _ = writeln!(self.out, "    (void)s;");
        let _ = writeln!(
            self.out,
            "    size_t zd_ma = representation ? 4 : 8; (void)zd_ma;"
        );
        let _ = writeln!(self.out, "    uint8_t* w_buf = NULL;");
        let _ = writeln!(self.out, "    size_t w_len = 0;");
        let _ = writeln!(self.out, "    size_t w_cap = 0;");
        let _ = writeln!(
            self.out,
            "    if (out_buf == NULL && out_cap > 0) goto fail;"
        );
        // The union's own DHEADER (appendable/mutable) is emitted by emit_union_write,
        // so it is identical whether the union is top-level or a nested element.
        let _ = ext;
        self.emit_union_write("(*s)", u)?;
        let _ = writeln!(self.out, "    if (out_len) *out_len = w_len;");
        let _ = writeln!(
            self.out,
            "    if (out_buf == NULL || out_cap < w_len) {{ free(w_buf); return -13; }}"
        );
        let _ = writeln!(
            self.out,
            "    if (w_len > 0) memcpy(out_buf, w_buf, w_len);"
        );
        let _ = writeln!(self.out, "    free(w_buf);");
        let _ = writeln!(self.out, "    return 0;");
        let _ = writeln!(self.out, "fail:");
        let _ = writeln!(self.out, "    free(w_buf);");
        let _ = writeln!(self.out, "    return -1;");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(self.out);

        // ---- decode body ----
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode_core(const uint8_t* buf, size_t len, void* out_sample, int zd_be, int representation);"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode(const uint8_t* buf, size_t len, void* out_sample) {{ return {c_name}_decode_core(buf, len, out_sample, 0, 1); }}"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode_e(const uint8_t* buf, size_t len, void* out_sample, int zd_be) {{ return {c_name}_decode_core(buf, len, out_sample, zd_be, 1); }}"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode_repr(const uint8_t* buf, size_t len, uint8_t representation, void* out_sample) {{ return {c_name}_decode_core(buf, len, out_sample, 0, representation ? 1 : 0); }}"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode_core(const uint8_t* buf, size_t len, void* out_sample, int zd_be, int representation) {{"
        );
        let _ = writeln!(self.out, "    {c_name}_t* s = ({c_name}_t*)out_sample;");
        let _ = writeln!(self.out, "    size_t pos = 0;");
        let _ = writeln!(
            self.out,
            "    size_t zd_ma = representation ? 4 : 8; (void)zd_ma;"
        );
        // The union DHEADER (appendable/mutable) is read by emit_union_read itself.
        self.emit_union_read("(*s)", u)?;
        let _ = writeln!(self.out, "    (void)s;");
        let _ = writeln!(self.out, "    return 0;");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(self.out);

        // ---- free body (free heap-owning union arms) ----
        let _ = writeln!(
            self.out,
            "static void {c_name}_sample_free(void* sample) {{"
        );
        let _ = writeln!(self.out, "    if (sample == NULL) return;");
        let _ = writeln!(self.out, "    {c_name}_t* s = ({c_name}_t*)sample;");
        let _ = writeln!(self.out, "    (void)s;");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(self.out);
        Ok(())
    }

    fn emit_preamble(&mut self) {
        let guard = self
            .opts
            .include_guard
            .clone()
            .unwrap_or_else(|| "ZERODDS_GENERATED_H".to_string());
        if let Some(h) = &self.opts.file_header {
            for line in h.lines() {
                let _ = writeln!(self.out, "/* {line} */");
            }
        } else {
            let _ = writeln!(
                self.out,
                "/* Generated by zerodds idl-cpp c_mode. Do not edit. */"
            );
        }
        let _ = writeln!(self.out, "#ifndef {guard}");
        let _ = writeln!(self.out, "#define {guard}");
        let _ = writeln!(self.out, "#include <stddef.h>");
        let _ = writeln!(self.out, "#include <stdint.h>");
        let _ = writeln!(self.out, "#include <string.h>");
        let _ = writeln!(self.out, "#include <stdlib.h>");
        // Bug C2: only `zerodds_xcdr2.h` is needed — it declares
        // `zerodds_typesupport_t` and every `zerodds_xcdr2_c_*` helper the
        // generated body calls. The cbindgen-generated `zerodds.h` redeclares
        // ~6 typed-FFI functions (`zerodds_topic_create_typed`, …) with names
        // that conflict with the handwritten prototypes in `zerodds_xcdr2.h`
        // (`struct zerodds_ZeroDdsRuntime*` vs `struct zerodds_ZeroDdsRuntime *`
        // plus other signature drift), so including both broke
        // `gcc -fsyntax-only <gen>.h`. The generated header uses none of the
        // `zerodds.h`-only symbols, so it is dropped.
        let _ = writeln!(self.out, "#include \"zerodds_xcdr2.h\"");
        let _ = writeln!(self.out, "#ifdef __cplusplus");
        let _ = writeln!(self.out, "extern \"C\" {{");
        let _ = writeln!(self.out, "#endif");
        let _ = writeln!(self.out);
        // XCDR2 8-byte primitives align to MAXALIGN = min(sizeof, 4) = 4, never
        // 8 (OMG DDS-XTypes 1.3 §7.4.1.1.1 / §7.4.3.2.3 INIT MAXALIGN(2)=4 —
        // matches the cross-vendor `zerodds-cdr` core, crates/cdr). The shared
        // `zerodds_xcdr2.h` `write_u64`/`read_u64` helpers pad to 8 (classic CDR1
        // / XCDR1 semantics), so the XCDR2 codec uses these locally-emitted
        // align-4 variants for u64/i64/f64 instead.
        let _ = writeln!(self.out, "#ifndef ZERODDS_X2_ALIGN4_8BYTE");
        let _ = writeln!(self.out, "#define ZERODDS_X2_ALIGN4_8BYTE");
        let _ = writeln!(
            self.out,
            "static inline int zd_x2_write_u64(uint8_t** buf, size_t* len, size_t* cap, uint64_t v) {{"
        );
        let _ = writeln!(
            self.out,
            "    if (zerodds_xcdr2_c_pad_to(buf, len, cap, 4) != 0) return -1;"
        );
        let _ = writeln!(
            self.out,
            "    if (zerodds_xcdr2_c_grow(buf, cap, *len + 8) != 0) return -1;"
        );
        let _ = writeln!(
            self.out,
            "    for (int i = 0; i < 8; ++i) {{ (*buf)[(*len)++] = (uint8_t)((v >> (8 * i)) & 0xFFu); }}"
        );
        let _ = writeln!(self.out, "    return 0;");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(
            self.out,
            "static inline int zd_x2_write_i64(uint8_t** buf, size_t* len, size_t* cap, int64_t v) {{ return zd_x2_write_u64(buf, len, cap, (uint64_t)v); }}"
        );
        let _ = writeln!(
            self.out,
            "static inline int zd_x2_write_f64(uint8_t** buf, size_t* len, size_t* cap, double v) {{ uint64_t u; memcpy(&u, &v, sizeof(u)); return zd_x2_write_u64(buf, len, cap, u); }}"
        );
        let _ = writeln!(
            self.out,
            "static inline int zd_x2_read_u64(const uint8_t* buf, size_t len, size_t* pos, uint64_t* out, int big_endian) {{"
        );
        let _ = writeln!(
            self.out,
            "    if (zerodds_xcdr2_c_pad_read(buf, len, pos, 4) != 0) return -1;"
        );
        let _ = writeln!(self.out, "    if (*pos + 8 > len) return -1;");
        let _ = writeln!(self.out, "    uint64_t v = 0;");
        let _ = writeln!(
            self.out,
            "    for (int i = 0; i < 8; ++i) {{ int sh = big_endian ? (8 * (7 - i)) : (8 * i); v |= (uint64_t)buf[*pos + (size_t)i] << sh; }}"
        );
        let _ = writeln!(self.out, "    *pos += 8; *out = v; return 0;");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(
            self.out,
            "static inline int zd_x2_read_i64(const uint8_t* buf, size_t len, size_t* pos, int64_t* out, int big_endian) {{ uint64_t u; int rc = zd_x2_read_u64(buf, len, pos, &u, big_endian); if (rc != 0) return rc; *out = (int64_t)u; return 0; }}"
        );
        let _ = writeln!(
            self.out,
            "static inline int zd_x2_read_f64(const uint8_t* buf, size_t len, size_t* pos, double* out, int big_endian) {{ uint64_t u; int rc = zd_x2_read_u64(buf, len, pos, &u, big_endian); if (rc != 0) return rc; memcpy(out, &u, sizeof(*out)); return 0; }}"
        );
        let _ = writeln!(self.out, "#endif");
        let _ = writeln!(self.out);
    }

    /// Map a primitive write/read helper suffix to the XCDR2-align-4 prefix.
    /// The 8-byte primitives (`u64`/`i64`/`f64`) route through the locally
    /// emitted `zd_x2_*` helpers (align 4, §7.4.1.1.1); everything else uses the
    /// shared `zerodds_xcdr2_c_*` helpers (which already pad to their own size,
    /// capped at 4).
    fn helper_call_prefix(helper: &str) -> &'static str {
        match helper {
            "u64" | "i64" | "f64" => "zd_x2_",
            _ => "zerodds_xcdr2_c_",
        }
    }

    /// If `name` is a bitmask or bitset, return the XCDR2 primitive helper
    /// suffix for its holder integer (`u8`/`u16`/`u32`/`u64`). Both serialize as
    /// their unsigned holder integer (XTypes §7.3.1.2).
    fn bits_helper(&self, name: &str) -> Option<&'static str> {
        let holder = self
            .reg
            .bitmasks
            .get(name)
            .or_else(|| self.reg.bitsets.get(name))
            .map(|(_, h)| *h)?;
        Some(match holder {
            "uint8_t" => "u8",
            "uint16_t" => "u16",
            "uint32_t" => "u32",
            _ => "u64",
        })
    }

    fn emit_postamble(&mut self) {
        let _ = writeln!(self.out, "#ifdef __cplusplus");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(self.out, "#endif");
        let _ = writeln!(self.out, "#endif");
    }

    fn walk_definitions(
        &mut self,
        defs: &[Definition],
        scope: &[String],
    ) -> Result<(), CppGenError> {
        for d in defs {
            match d {
                Definition::Module(m) => self.walk_module(m, scope)?,
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(sd))) => {
                    if let zerodds_idl::ast::StructDcl::Def(def) = sd {
                        self.emit_struct(def, scope)?;
                    }
                }
                // Enums are emitted in the pre-pass (as int32 typedefs);
                // bitsets/bitmasks in the pre-pass (as holder-int typedefs);
                // typedefs are resolved inline at every reference site. All are
                // no-ops here (Bug C / #43: no longer rejected).
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(_)))
                | Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(_)))
                | Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(_)))
                | Definition::Type(TypeDecl::Typedef(_)) => {}
                Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(ud))) => {
                    // The tagged-union typedef is emitted in the pre-pass; a
                    // top-level union additionally gets a TypeSupport so it can
                    // be a topic type (#43, union).
                    if let UnionDcl::Def(def) = ud {
                        self.emit_union_typesupport(def, scope)?;
                    }
                }
                Definition::Type(_) => {
                    return Err(unsupported("non-struct type"));
                }
                // Constants/annotations occur at top level.
                Definition::Const(_)
                | Definition::Annotation(_)
                | Definition::TypeId(_)
                | Definition::TypePrefix(_)
                | Definition::Import(_) => {
                    // Ignore — no C output needed.
                }
                _ => {
                    return Err(unsupported("non-struct definition"));
                }
            }
        }
        Ok(())
    }

    fn walk_module(&mut self, m: &ModuleDef, scope: &[String]) -> Result<(), CppGenError> {
        let mut new_scope = scope.to_vec();
        new_scope.push(m.name.text.clone());
        self.walk_definitions(&m.definitions, &new_scope)
    }

    /// Emit just the `typedef struct {…} <C>_t;` for a struct. Split out so all
    /// struct typedefs can be emitted in a pre-pass — a recursive struct's body
    /// helper (and a struct that splices another struct) needs every aggregate's
    /// C type already in scope (#43, recursion / forward-decl).
    /// zerodds-lint: recursion-depth 64 (member walk; bounded by IDL nesting)
    fn emit_struct_typedef(
        &mut self,
        def: &StructDef,
        scope: &[String],
    ) -> Result<(), CppGenError> {
        let c_name = c_identifier(scope, &def.name.text);
        let _ = writeln!(self.out, "typedef struct {c_name}_s {{");
        for member in &def.members {
            let optional = is_optional(&member.annotations);
            for decl in &member.declarators {
                let m_name = decl.name();
                let c_type = c_type_for(self.reg, &member.type_spec)?;
                // Fixed array declarator → C array suffix `[N][M]…`. A
                // typedef-to-array alias contributes leading dims (#43).
                let dims = effective_array_dims(self.reg, &member.type_spec, decl)?;
                if dims.is_empty() {
                    let _ = writeln!(self.out, "    {c_type} {field};", field = m_name.text);
                } else {
                    let suffix: String = dims.iter().map(|n| format!("[{n}]")).collect();
                    let _ = writeln!(
                        self.out,
                        "    {c_type} {field}{suffix};",
                        field = m_name.text
                    );
                }
                // @optional: a presence companion flag (Bug C2: no longer
                // flattened to a mandatory member).
                if optional {
                    let _ = writeln!(
                        self.out,
                        "    uint8_t {field}_present;",
                        field = m_name.text
                    );
                }
            }
        }
        if def.members.is_empty() {
            // C99 forbids empty structs; dummy padding member.
            let _ = writeln!(self.out, "    uint8_t _zerodds_empty;");
        }
        let _ = writeln!(self.out, "}} {c_name}_t;");
        let _ = writeln!(self.out);
        Ok(())
    }

    /// Emit ALL aggregate typedefs (structs + unions) in by-value dependency
    /// order. A struct/union embedded BY VALUE (not behind a sequence/map
    /// pointer) must have its complete C type emitted first; a recursive
    /// self-reference is a pointer and imposes no order.
    ///
    /// A *by-value* cycle (e.g. `struct Node { Node n; };`) is a genuinely
    /// infinite-size type — the only legal self-reference is heap-indirected
    /// (through a sequence). Such a cycle is rejected cleanly here rather than
    /// emitting a non-compilable infinite C struct (XTypes §7.4.5, #43).
    fn emit_all_aggregate_typedefs(&mut self, defs: &[Definition]) -> Result<(), CppGenError> {
        // Gather every aggregate with its scope, keyed by simple name.
        let mut order: Vec<(String, Vec<String>, AggDef)> = Vec::new();
        collect_aggregates(defs, &[], &mut order);
        // Reject infinite-size by-value self/mutual recursion.
        for (name, _, agg) in &order {
            let mut seen = BTreeSet::new();
            if self.by_value_reaches(agg, name, &mut seen) {
                return Err(CppGenError::UnsupportedConstruct {
                    construct: "infinite-size type: a struct/union member references its own \
                                type by value (a self-reference is only legal heap-indirected, \
                                e.g. through a sequence)"
                        .to_string(),
                    context: Some(name.clone()),
                });
            }
        }
        // Topological sort on the by-value dependency edges.
        let mut emitted: BTreeSet<String> = BTreeSet::new();
        let mut remaining: Vec<(String, Vec<String>, AggDef)> = order;
        // Bounded fixpoint: each pass emits at least one node (acyclic by-value
        // graph), so at most N passes.
        let max_passes = remaining.len() + 1;
        for _ in 0..max_passes {
            if remaining.is_empty() {
                break;
            }
            let mut progressed = false;
            let mut next: Vec<(String, Vec<String>, AggDef)> = Vec::new();
            for (name, scope, agg) in remaining.drain(..) {
                let deps = self.by_value_agg_deps(&agg);
                if deps.iter().all(|d| emitted.contains(d) || d == &name) {
                    match &agg {
                        AggDef::Struct(s) => self.emit_struct_typedef(s, &scope)?,
                        AggDef::Union(u) => self.emit_union_typedef(u, &scope)?,
                    }
                    emitted.insert(name);
                    progressed = true;
                } else {
                    next.push((name, scope, agg));
                }
            }
            remaining = next;
            if !progressed {
                // Defensive: a residual (should not happen for valid IDL) — emit
                // in declaration order rather than dropping types.
                for (name, scope, agg) in remaining.drain(..) {
                    match &agg {
                        AggDef::Struct(s) => self.emit_struct_typedef(s, &scope)?,
                        AggDef::Union(u) => self.emit_union_typedef(u, &scope)?,
                    }
                    emitted.insert(name);
                }
                break;
            }
        }
        Ok(())
    }

    /// The simple names of aggregates a struct/union embeds BY VALUE (members
    /// that are directly a struct/union, or a fixed array / typedef-to-aggregate
    /// of one). Sequence/map element refs are pointers and excluded.
    fn by_value_agg_deps(&self, agg: &AggDef) -> Vec<String> {
        let mut out = Vec::new();
        let push_ts = |ts: &TypeSpec, out: &mut Vec<String>| {
            let resolved = resolve_alias(self.reg, ts);
            if let TypeSpec::Scoped(sc) = &resolved {
                if let Some(last) = scoped_last(sc) {
                    if self.reg.structs.contains_key(&last) || self.reg.unions.contains_key(&last) {
                        out.push(last);
                    }
                }
            }
        };
        match agg {
            AggDef::Struct(s) => {
                for m in &s.members {
                    push_ts(&m.type_spec, &mut out);
                }
            }
            AggDef::Union(u) => {
                for c in &u.cases {
                    push_ts(&c.element.type_spec, &mut out);
                }
            }
        }
        out
    }

    /// True if `agg` can reach `target` following ONLY by-value member edges —
    /// i.e. `target` is embedded (transitively) as a non-pointer field, which
    /// makes the type infinite-size. `seen` guards the walk against cycles.
    /// zerodds-lint: recursion-depth 256 (bounded by distinct type set)
    fn by_value_reaches(&self, agg: &AggDef, target: &str, seen: &mut BTreeSet<String>) -> bool {
        for dep in self.by_value_agg_deps(agg) {
            if dep == target {
                return true;
            }
            if !seen.insert(dep.clone()) {
                continue;
            }
            let next = self
                .reg
                .structs
                .get(&dep)
                .map(|(_, s)| AggDef::Struct(s.clone()))
                .or_else(|| {
                    self.reg
                        .unions
                        .get(&dep)
                        .map(|(_, u)| AggDef::Union(u.clone()))
                });
            if let Some(next) = next {
                if self.by_value_reaches(&next, target, seen) {
                    return true;
                }
            }
        }
        false
    }

    /// Forward-declare and define a runtime body helper for every recursive
    /// struct/union. A recursive reference (self / mutual) is emitted as a call
    /// to `<C>_write_body` / `<C>_read_body`, which threads the SAME growing
    /// buffer (`w_buf`/`w_len`/`w_cap`) / read cursor (`pos`) by pointer — so
    /// recursion happens at RUNTIME, not at codegen time (Bug G, C backend).
    fn emit_recursive_helpers(&mut self) -> Result<(), CppGenError> {
        // Collect (c_name, AggDef) for recursive aggregates, in a stable order.
        let mut recs: Vec<(String, AggDef)> = Vec::new();
        for name in self.reg.recursive.clone() {
            if let Some((cn, s)) = self.reg.structs.get(&name).cloned() {
                recs.push((cn, AggDef::Struct(s)));
            } else if let Some((cn, u)) = self.reg.unions.get(&name).cloned() {
                recs.push((cn, AggDef::Union(u)));
            }
        }
        if recs.is_empty() {
            return Ok(());
        }
        // Forward declarations (so mutually recursive bodies can call each other).
        for (cn, _) in &recs {
            let _ = writeln!(
                self.out,
                "static int {cn}_write_body(const {cn}_t* v, uint8_t** w_buf_pp, size_t* w_len_pp, size_t* w_cap_pp, int representation);"
            );
            let _ = writeln!(
                self.out,
                "static int {cn}_read_body(const uint8_t* buf, size_t len, size_t* pos_pp, {cn}_t* v, int zd_be, int representation);"
            );
        }
        let _ = writeln!(self.out);
        // Bodies.
        for (cn, agg) in &recs {
            self.emit_recursive_write_body(cn, agg)?;
            self.emit_recursive_read_body(cn, agg)?;
        }
        Ok(())
    }

    /// `<C>_write_body`: the inline write templates, but with `w_buf`/`w_len`/
    /// `w_cap` macro-aliased to the caller's buffer cursor (passed by pointer),
    /// so a nested recursive call (`&w_buf` == the same `w_buf_pp`) keeps the
    /// one shared output buffer. The templates' `goto fail` reaches a local
    /// `fail:` returning -1.
    fn emit_recursive_write_body(&mut self, cn: &str, agg: &AggDef) -> Result<(), CppGenError> {
        let _ = writeln!(
            self.out,
            "static int {cn}_write_body(const {cn}_t* v, uint8_t** w_buf_pp, size_t* w_len_pp, size_t* w_cap_pp, int representation) {{"
        );
        let _ = writeln!(self.out, "#define w_buf (*w_buf_pp)");
        let _ = writeln!(self.out, "#define w_len (*w_len_pp)");
        let _ = writeln!(self.out, "#define w_cap (*w_cap_pp)");
        let _ = writeln!(self.out, "    const {cn}_t* s = v; (void)s;");
        let _ = writeln!(
            self.out,
            "    size_t zd_ma = representation ? 4 : 8; (void)zd_ma;"
        );
        match agg {
            AggDef::Struct(s) => self.emit_struct_body_writes(s)?,
            AggDef::Union(u) => self.emit_union_write("(*s)", u)?,
        }
        let _ = writeln!(self.out, "    return 0;");
        let _ = writeln!(self.out, "fail:");
        let _ = writeln!(self.out, "    return -1;");
        let _ = writeln!(self.out, "#undef w_buf");
        let _ = writeln!(self.out, "#undef w_len");
        let _ = writeln!(self.out, "#undef w_cap");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(self.out);
        Ok(())
    }

    /// `<C>_read_body`: the inline read templates with `pos` macro-aliased to the
    /// caller's read cursor (passed by pointer); templates' `return -7` is the
    /// error path.
    fn emit_recursive_read_body(&mut self, cn: &str, agg: &AggDef) -> Result<(), CppGenError> {
        let _ = writeln!(
            self.out,
            "static int {cn}_read_body(const uint8_t* buf, size_t len, size_t* pos_pp, {cn}_t* v, int zd_be, int representation) {{"
        );
        let _ = writeln!(self.out, "#define pos (*pos_pp)");
        let _ = writeln!(self.out, "    {cn}_t* s = v; (void)s;");
        let _ = writeln!(
            self.out,
            "    size_t zd_ma = representation ? 4 : 8; (void)zd_ma;"
        );
        match agg {
            AggDef::Struct(s) => self.emit_struct_body_reads(s)?,
            AggDef::Union(u) => self.emit_union_read("(*s)", u)?,
        }
        let _ = writeln!(self.out, "    return 0;");
        let _ = writeln!(self.out, "#undef pos");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(self.out);
        Ok(())
    }

    fn emit_struct(&mut self, def: &StructDef, scope: &[String]) -> Result<(), CppGenError> {
        let ext = extensibility_of(&def.annotations);
        let c_name = c_identifier(scope, &def.name.text);
        let dds_name = dds_type_name(scope, &def.name.text);

        // ---- typedef struct ---- emitted in the aggregate-typedef pre-pass
        // (`emit_all_aggregate_typedefs`) so by-value embeds and recursive body
        // helpers see complete C types; here we emit only the codec.

        // ---- encode/decode/free/key_hash declarations ----
        let _ = writeln!(
            self.out,
            "static int {c_name}_encode(const void* sample, uint8_t* out_buf, size_t out_cap, size_t* out_len);"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode(const uint8_t* buf, size_t len, void* out_sample);\nstatic int {c_name}_decode_e(const uint8_t* buf, size_t len, void* out_sample, int zd_be);"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode_repr(const uint8_t* buf, size_t len, uint8_t representation, void* out_sample);"
        );
        let _ = writeln!(self.out, "static void {c_name}_sample_free(void* sample);");
        let has_key = struct_has_key(def);
        if has_key {
            let _ = writeln!(
                self.out,
                "static int {c_name}_key_hash(const void* sample, uint8_t out_hash[16]);"
            );
        }
        let _ = writeln!(self.out);

        // ---- typesupport static const ----
        let _ = writeln!(
            self.out,
            "static const char {c_name}_type_name[] = \"{dds_name}\";"
        );
        let _ = writeln!(
            self.out,
            "static const zerodds_typesupport_t {c_name}_typesupport = {{"
        );
        let _ = writeln!(self.out, "    .type_hash = {{0}},");
        let _ = writeln!(self.out, "    .type_name = {c_name}_type_name,");
        let _ = writeln!(self.out, "    .is_keyed = {},", if has_key { 1 } else { 0 });
        let _ = writeln!(self.out, "    .extensibility = {},", ext.as_u8());
        let _ = writeln!(self.out, "    ._reserved = {{0}},");
        let _ = writeln!(self.out, "    .encode = {c_name}_encode,");
        let _ = writeln!(self.out, "    .decode = {c_name}_decode,");
        if has_key {
            let _ = writeln!(self.out, "    .key_hash = {c_name}_key_hash,");
        } else {
            let _ = writeln!(self.out, "    .key_hash = NULL,");
        }
        let _ = writeln!(self.out, "    .sample_free = {c_name}_sample_free,");
        let _ = writeln!(self.out, "    .decode_repr = {c_name}_decode_repr,");
        let _ = writeln!(self.out, "}};");
        let _ = writeln!(self.out);

        // ---- encode body ----
        self.emit_encode_body(&c_name, def, ext)?;
        // ---- decode body ----
        self.emit_decode_body(&c_name, def, ext)?;
        // ---- free body ----
        self.emit_free_body(&c_name, def);
        // ---- key_hash body ----
        if has_key {
            self.emit_key_hash_body(&c_name, def);
        }
        Ok(())
    }

    fn emit_encode_body(
        &mut self,
        c_name: &str,
        def: &StructDef,
        ext: Extensibility,
    ) -> Result<(), CppGenError> {
        // `_encode` keeps the XCDR2 default ABI (typesupport .encode pointer);
        // `_encode_repr` is the representation-aware body. `representation`:
        // 0=XCDR1 (classic CDR, max_align 8, no DHEADER, @mutable=PL_CDR1),
        // 1=XCDR2 (DHEADER + EMHEADER). c_mode is little-endian only.
        let _ = writeln!(
            self.out,
            "static int {c_name}_encode_repr(const void* sample, uint8_t* out_buf, size_t out_cap, size_t* out_len, int representation);"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_encode(const void* sample, uint8_t* out_buf, size_t out_cap, size_t* out_len) {{ return {c_name}_encode_repr(sample, out_buf, out_cap, out_len, 1); }}"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_encode_repr(const void* sample, uint8_t* out_buf, size_t out_cap, size_t* out_len, int representation) {{"
        );
        let _ = writeln!(
            self.out,
            "    const {c_name}_t* s = (const {c_name}_t*)sample;"
        );
        let _ = writeln!(self.out, "    (void)s;");
        // XCDR1 caps 8-byte primitives at MAXALIGN 8, XCDR2 at 4 (§7.4.1.1.1).
        let _ = writeln!(
            self.out,
            "    size_t zd_ma = representation ? 4 : 8; (void)zd_ma;"
        );
        let _ = writeln!(self.out, "    /* Two-pass: grow the buffer, then copy. */");
        let _ = writeln!(self.out, "    uint8_t* w_buf = NULL;");
        let _ = writeln!(self.out, "    size_t w_len = 0;");
        let _ = writeln!(self.out, "    size_t w_cap = 0;");
        let _ = writeln!(
            self.out,
            "    if (out_buf == NULL && out_cap > 0) goto fail;"
        );
        match ext {
            Extensibility::Final => {
                self.emit_struct_body_writes(def)?;
            }
            Extensibility::Appendable => {
                // XCDR2: DHEADER reserved, body-writes, then length patch.
                // XCDR1: NO DHEADER — body starts at offset 0 (max_align 8).
                let _ = writeln!(self.out, "    size_t dheader_pos = 0; (void)dheader_pos;");
                let _ = writeln!(self.out, "    if (representation) {{");
                let _ = writeln!(
                    self.out,
                    "        if (zerodds_xcdr2_c_grow(&w_buf, &w_cap, w_len + 4) != 0) goto fail;"
                );
                let _ = writeln!(self.out, "        dheader_pos = w_len; w_len += 4;");
                let _ = writeln!(self.out, "    }}");
                let _ = writeln!(self.out, "    size_t body_start = w_len;");
                self.emit_struct_body_writes(def)?;
                let _ = writeln!(self.out, "    if (representation) {{");
                let _ = writeln!(
                    self.out,
                    "        zerodds_xcdr2_c_put_u32_at(w_buf, dheader_pos, (uint32_t)(w_len - body_start));"
                );
                let _ = writeln!(self.out, "    }}");
            }
            Extensibility::Mutable => {
                // XCDR2: DHEADER + EMHEADER per member. XCDR1: PL_CDR1 parameter
                // list (no DHEADER), terminated by the PID_LIST_END sentinel.
                let _ = writeln!(self.out, "    if (representation) {{");
                let _ = writeln!(
                    self.out,
                    "        if (zerodds_xcdr2_c_grow(&w_buf, &w_cap, w_len + 4) != 0) goto fail;"
                );
                let _ = writeln!(self.out, "        size_t dheader_pos = w_len; w_len += 4;");
                let _ = writeln!(self.out, "        size_t mut_body_start = w_len;");
                self.emit_mutable_member_writes(def)?;
                let _ = writeln!(
                    self.out,
                    "        zerodds_xcdr2_c_put_u32_at(w_buf, dheader_pos, (uint32_t)(w_len - mut_body_start));"
                );
                let _ = writeln!(self.out, "    }} else {{");
                self.emit_pl_cdr1_member_writes(def)?;
                let _ = writeln!(self.out, "    }}");
            }
        }
        // Copy the output.
        let _ = writeln!(self.out, "    if (out_len) *out_len = w_len;");
        let _ = writeln!(
            self.out,
            "    if (out_buf == NULL || out_cap < w_len) {{ free(w_buf); return -13; }}"
        );
        let _ = writeln!(
            self.out,
            "    if (w_len > 0) memcpy(out_buf, w_buf, w_len);"
        );
        let _ = writeln!(self.out, "    free(w_buf);");
        let _ = writeln!(self.out, "    return 0;");
        let _ = writeln!(self.out, "fail:");
        let _ = writeln!(self.out, "    free(w_buf);");
        let _ = writeln!(self.out, "    return -1;");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(self.out);
        Ok(())
    }

    fn emit_struct_body_writes(&mut self, def: &StructDef) -> Result<(), CppGenError> {
        for member in &def.members {
            let optional = is_optional(&member.annotations);
            for decl in &member.declarators {
                let f = &decl.name().text;
                let dims = effective_array_dims(self.reg, &member.type_spec, decl)?;
                if optional {
                    // XCDR2 non-mutable optional: a boolean presence flag, then
                    // the value only if present (XTypes §7.4.3 optional members).
                    let _ = writeln!(
                        self.out,
                        "    if (zerodds_xcdr2_c_write_u8(&w_buf, &w_len, &w_cap, s->{f}_present ? 1 : 0) != 0) goto fail;"
                    );
                    let _ = writeln!(self.out, "    if (s->{f}_present) {{");
                    self.emit_array_or_scalar_write(&format!("s->{f}"), &member.type_spec, &dims)?;
                    let _ = writeln!(self.out, "    }}");
                } else {
                    self.emit_array_or_scalar_write(&format!("s->{f}"), &member.type_spec, &dims)?;
                }
            }
        }
        Ok(())
    }

    /// Write a member that may be a fixed array (N nested loops over the C array)
    /// or a scalar/aggregate. Fixed arrays carry no length prefix (XTypes §7.4.3).
    fn emit_array_or_scalar_write(
        &mut self,
        var: &str,
        type_spec: &TypeSpec,
        dims: &[u64],
    ) -> Result<(), CppGenError> {
        if dims.is_empty() {
            return self.emit_member_write(var, type_spec);
        }
        // Bug XV-arr: a fixed array gets ONE collection DHEADER only when its
        // ELEMENT type is NON-primitive (1-D array of struct/string/sequence). An
        // array of a PRIMITIVE element is a PARRAY (XTypes 1.3 §7.4.3.5 rule 8) —
        // PLAIN-collection regardless of dimensionality, so it carries NO DHEADER
        // even when multi-dimensional (`long grid[2][3]`). Byte-identical to the
        // corrected rust golden (grid: 6×i32 tight, NO DHEADER; shape: DHEADER=16).
        let needs_dheader = !self.seq_elem_is_primitive(type_spec);
        if needs_dheader {
            // Own brace scope so two array members in one struct do not collide
            // on the `arr_dheader_pos`/`arr_body_start` locals. The collection
            // DHEADER exists only under XCDR2 — XCDR1 (classic CDR) has none.
            let _ = writeln!(self.out, "    {{");
            let _ = writeln!(
                self.out,
                "    size_t arr_dheader_pos = 0; (void)arr_dheader_pos;"
            );
            let _ = writeln!(self.out, "    if (representation) {{");
            let _ = writeln!(
                self.out,
                "        if (zerodds_xcdr2_c_grow(&w_buf, &w_cap, w_len + 4) != 0) goto fail;"
            );
            let _ = writeln!(self.out, "        arr_dheader_pos = w_len; w_len += 4;");
            let _ = writeln!(self.out, "    }}");
            let _ = writeln!(
                self.out,
                "    size_t arr_body_start = w_len; (void)arr_body_start;"
            );
        }
        let mut acc = var.to_string();
        for (d, _n) in dims.iter().enumerate() {
            let iv = format!("ai{d}");
            let _ = writeln!(
                self.out,
                "    for (uint32_t {iv} = 0; {iv} < {n}; ++{iv}) {{",
                n = dims[d]
            );
            acc = format!("{acc}[{iv}]");
        }
        self.emit_member_write(&acc, type_spec)?;
        for _ in dims {
            let _ = writeln!(self.out, "    }}");
        }
        if needs_dheader {
            let _ = writeln!(
                self.out,
                "    if (representation) {{ zerodds_xcdr2_c_put_u32_at(w_buf, arr_dheader_pos, (uint32_t)(w_len - arr_body_start)); }}"
            );
            let _ = writeln!(self.out, "    }}"); // close the array DHEADER scope
        }
        Ok(())
    }

    /// zerodds-lint: recursion-depth 64 (nested struct members; bounded by IDL)
    fn emit_member_write(&mut self, var: &str, type_spec: &TypeSpec) -> Result<(), CppGenError> {
        // Resolve typedef aliases to the effective type first (Bug C).
        let resolved = resolve_alias(self.reg, type_spec);
        match &resolved {
            TypeSpec::Primitive(p) => self.emit_primitive_write(var, *p),
            TypeSpec::String(st) => {
                if st.wide {
                    return self.emit_wstring_write(var, st.bound.as_ref());
                }
                // Bounded string<N>: enforce the bound (XTypes §7.4.13.4.2) — a
                // string longer than N must be rejected, not silently corrupt.
                if let Some(n) = self.eval_bound(st.bound.as_ref()) {
                    let _ = writeln!(
                        self.out,
                        "    if (({var}) != NULL && strlen({var}) > {n}u) goto fail;"
                    );
                }
                let _ = writeln!(
                    self.out,
                    "    if (zerodds_xcdr2_c_write_string(&w_buf, &w_len, &w_cap, {var}) != 0) goto fail;"
                );
                Ok(())
            }
            TypeSpec::Sequence(seq) => self.emit_sequence_write(var, &seq.elem, seq.bound.as_ref()),
            TypeSpec::Map(m) => self.emit_map_write(var, &m.key, &m.value, m.bound.as_ref()),
            TypeSpec::Scoped(sc) => {
                let last = scoped_last(sc).ok_or_else(|| unsupported("empty scoped name"))?;
                if self.reg.enums.contains_key(&last) {
                    // enum → signed wire holder of its @bit_bound width
                    // (XTypes §7.4.5.1). u8/u16 carry the byte image of the
                    // signed value; the int32 case is the spec default.
                    let bytes = self.reg.enum_bytes.get(&last).copied().unwrap_or(4);
                    let line = match bytes {
                        1 => format!(
                            "    if (zerodds_xcdr2_c_write_u8(&w_buf, &w_len, &w_cap, (uint8_t)({var})) != 0) goto fail;"
                        ),
                        2 => format!(
                            "    if (zerodds_xcdr2_c_write_u16(&w_buf, &w_len, &w_cap, (uint16_t)({var})) != 0) goto fail;"
                        ),
                        _ => format!(
                            "    if (zerodds_xcdr2_c_write_i32(&w_buf, &w_len, &w_cap, (int32_t)({var})) != 0) goto fail;"
                        ),
                    };
                    let _ = writeln!(self.out, "{line}");
                    return Ok(());
                }
                // bitmask / bitset → write the holder integer (XTypes §7.3.1.2).
                if let Some(helper) = self.bits_helper(&last) {
                    let prefix = Self::helper_call_prefix(helper);
                    let _ = writeln!(
                        self.out,
                        "    if ({prefix}write_{helper}(&w_buf, &w_len, &w_cap, {var}) != 0) goto fail;"
                    );
                    return Ok(());
                }
                if let Some((cn, ndef)) = self.reg.structs.get(&last).cloned() {
                    // A recursive struct (self-referential through a sequence,
                    // or mutually recursive) is spliced through its own runtime
                    // body helper — inlining would recurse forever at codegen
                    // time (XTypes §7.4.5 / Bug G). Non-recursive structs are
                    // inline-encoded by value (no DHEADER: @final inline form).
                    if self.reg.is_recursive(&last) {
                        // A recursive struct spliced as a member/sequence-element
                        // is a self-contained value. Under XCDR2 a NON-final
                        // (@appendable/@mutable) aggregate is DELIMITED — it
                        // carries its own DHEADER (XTypes §7.4.3.5 / §7.4.4.4),
                        // exactly as its top-level `_encode` would emit. The
                        // body-only `_write_body` must therefore be wrapped in a
                        // DHEADER here, so each `sequence<Tree>` element matches
                        // the rust golden (per-node DHEADER). @final splices the
                        // bare body (no DHEADER).
                        let ext = extensibility_of(&ndef.annotations);
                        if matches!(ext, Extensibility::Final) {
                            let _ = writeln!(
                                self.out,
                                "    if ({cn}_write_body(&({var}), &w_buf, &w_len, &w_cap, representation) != 0) goto fail;"
                            );
                        } else {
                            // Per-node DHEADER under XCDR2 only; XCDR1 splices the
                            // bare recursive body (no delimiter).
                            let _ = writeln!(self.out, "    {{");
                            let _ = writeln!(
                                self.out,
                                "    size_t rec_dheader_pos = 0; (void)rec_dheader_pos;"
                            );
                            let _ = writeln!(self.out, "    if (representation) {{");
                            let _ = writeln!(
                                self.out,
                                "        if (zerodds_xcdr2_c_grow(&w_buf, &w_cap, w_len + 4) != 0) goto fail;"
                            );
                            let _ =
                                writeln!(self.out, "        rec_dheader_pos = w_len; w_len += 4;");
                            let _ = writeln!(self.out, "    }}");
                            let _ = writeln!(
                                self.out,
                                "    size_t rec_body_start = w_len; (void)rec_body_start;"
                            );
                            let _ = writeln!(
                                self.out,
                                "    if ({cn}_write_body(&({var}), &w_buf, &w_len, &w_cap, representation) != 0) goto fail;"
                            );
                            let _ = writeln!(
                                self.out,
                                "    if (representation) {{ zerodds_xcdr2_c_put_u32_at(w_buf, rec_dheader_pos, (uint32_t)(w_len - rec_body_start)); }}"
                            );
                            let _ = writeln!(self.out, "    }}");
                        }
                        return Ok(());
                    }
                    return self.emit_nested_struct_write(var, &ndef);
                }
                if let Some((cn, udef)) = self.reg.unions.get(&last).cloned() {
                    if self.reg.is_recursive(&last) {
                        let _ = writeln!(
                            self.out,
                            "    if ({cn}_write_body(&({var}), &w_buf, &w_len, &w_cap, representation) != 0) goto fail;"
                        );
                        return Ok(());
                    }
                    return self.emit_union_write(var, &udef);
                }
                Err(unsupported("unresolved scoped member (C backend)"))
            }
            TypeSpec::Fixed(f) => {
                // fixed<P,S>: write the (P+2)/2 raw BCD octets (CORBA §9.3.2.7),
                // no alignment, no length prefix, endian-independent.
                let p = const_expr_to_u64(self.reg, &f.digits)
                    .ok_or_else(|| unsupported("fixed: non-constant digit count"))?;
                let n = (p + 2) / 2;
                let _ = writeln!(
                    self.out,
                    "    for (size_t __fi = 0; __fi < {n}; __fi++) {{ if (zerodds_xcdr2_c_write_u8(&w_buf, &w_len, &w_cap, (uint8_t)(({var}).bcd[__fi])) != 0) goto fail; }}"
                );
                Ok(())
            }
            TypeSpec::Any => Err(unsupported("any")),
        }
    }

    /// XCDR2 wstring (§7.4.4.6): uint32 byte-length (= 2*code-units), then the
    /// UTF-16-LE code units, no NUL. C side holds a NUL-terminated `uint16_t*`.
    fn emit_wstring_write(
        &mut self,
        var: &str,
        bound: Option<&ConstExpr>,
    ) -> Result<(), CppGenError> {
        let _ = writeln!(self.out, "    {{");
        let _ = writeln!(
            self.out,
            "        const uint16_t* ws_p = (const uint16_t*)({var});"
        );
        let _ = writeln!(self.out, "        uint32_t ws_n = 0;");
        let _ = writeln!(self.out, "        if (ws_p) while (ws_p[ws_n]) ++ws_n;");
        // Bounded wstring<N>: reject a string longer than N code units.
        if let Some(n) = self.eval_bound(bound) {
            let _ = writeln!(self.out, "        if (ws_n > {n}u) goto fail;");
        }
        let _ = writeln!(
            self.out,
            "        if (zerodds_xcdr2_c_write_u32(&w_buf, &w_len, &w_cap, ws_n * 2u) != 0) goto fail;"
        );
        let _ = writeln!(
            self.out,
            "        for (uint32_t wi = 0; wi < ws_n; ++wi) {{ if (zerodds_xcdr2_c_write_u16(&w_buf, &w_len, &w_cap, ws_p[wi]) != 0) goto fail; }}"
        );
        let _ = writeln!(self.out, "    }}");
        Ok(())
    }

    /// map<K,V> (§7.4.4.7): XCDR2 serializes a map as a sequence of (K,V)
    /// pairs — DHEADER (byte-length) + uint32 entry-count, then each entry's
    /// key followed by its value. C side: parallel `keys[]`/`vals[]` arrays.
    fn emit_map_write(
        &mut self,
        var: &str,
        key: &TypeSpec,
        value: &TypeSpec,
        bound: Option<&ConstExpr>,
    ) -> Result<(), CppGenError> {
        let _ = writeln!(self.out, "    {{");
        // Bounded map<K,V,N>: reject more than N entries.
        if let Some(n) = self.eval_bound(bound) {
            let _ = writeln!(self.out, "    if (({var}).len > {n}u) goto fail;");
        }
        // The map DHEADER is a uint32 -> 4-align before writing it (a map after a
        // sub-4-byte member, e.g. a 2-byte @bit_bound enum, would otherwise land
        // the DHEADER unaligned). Matches the rust reference + XCDR2 §7.4.1.
        let _ = writeln!(
            self.out,
            "    if (zerodds_xcdr2_c_pad_to(&w_buf, &w_len, &w_cap, 4) != 0) goto fail;"
        );
        // XCDR2 §7.4.3.5: a map carries a DHEADER only when its (key,value) element
        // is NON-primitive. `map<long,long>` (both primitive) omits it — matching
        // cdr-core `needs_collection_dheader(.., K::IS_PRIMITIVE && V::IS_PRIMITIVE)`
        // and FastDDS/OpenDDS. (Same rule as the primitive-array PARRAY above.)
        let map_dh = !(self.seq_elem_is_primitive(key) && self.seq_elem_is_primitive(value));
        let _ = writeln!(
            self.out,
            "    size_t map_dheader_pos = 0; (void)map_dheader_pos;"
        );
        if map_dh {
            let _ = writeln!(self.out, "    if (representation) {{");
            let _ = writeln!(
                self.out,
                "        if (zerodds_xcdr2_c_grow(&w_buf, &w_cap, w_len + 4) != 0) goto fail;"
            );
            let _ = writeln!(self.out, "        map_dheader_pos = w_len; w_len += 4;");
            let _ = writeln!(self.out, "    }}");
        }
        let _ = writeln!(
            self.out,
            "    size_t map_body_start = w_len; (void)map_body_start;"
        );
        let _ = writeln!(
            self.out,
            "    if (zerodds_xcdr2_c_write_u32(&w_buf, &w_len, &w_cap, ({var}).len) != 0) goto fail;"
        );
        let mv = format!("mi{}", self.coll_depth);
        let _ = writeln!(
            self.out,
            "    for (uint32_t {mv} = 0; {mv} < ({var}).len; ++{mv}) {{"
        );
        self.coll_depth += 1;
        let rk = self.emit_member_write(&format!("({var}).keys[{mv}]"), key);
        let rv = if rk.is_ok() {
            self.emit_member_write(&format!("({var}).vals[{mv}]"), value)
        } else {
            Ok(())
        };
        self.coll_depth -= 1;
        rk.and(rv)?;
        let _ = writeln!(self.out, "    }}");
        if map_dh {
            let _ = writeln!(
                self.out,
                "    if (representation) {{ zerodds_xcdr2_c_put_u32_at(w_buf, map_dheader_pos, (uint32_t)(w_len - map_body_start)); }}"
            );
        }
        let _ = writeln!(self.out, "    }}");
        Ok(())
    }

    /// Discriminated union (§7.4.4.5): write the discriminator, then the member
    /// selected by it. C side: a tagged union `struct { Disc _d; union {...} _u; }`.
    fn emit_union_write(&mut self, var: &str, udef: &UnionDef) -> Result<(), CppGenError> {
        // A union honours its extensibility wherever it appears (top-level OR as a
        // member / sequence element): @appendable/@mutable carry a 4-aligned uint32
        // DHEADER over [disc + branch]; @final does not. Previously only the
        // top-level TypeSupport wrapped the DHEADER, so a union *element* (e.g.
        // sequence<Sel>) was emitted without one — cross-PSM wire divergence.
        let u_dheader = !matches!(extensibility_of(&udef.annotations), Extensibility::Final);
        if u_dheader {
            let _ = writeln!(self.out, "    {{");
            let _ = writeln!(
                self.out,
                "    size_t u_dheader_pos = 0; (void)u_dheader_pos;"
            );
            let _ = writeln!(self.out, "    if (representation) {{");
            let _ = writeln!(
                self.out,
                "        if (zerodds_xcdr2_c_pad_to(&w_buf, &w_len, &w_cap, 4) != 0) goto fail;"
            );
            let _ = writeln!(
                self.out,
                "        if (zerodds_xcdr2_c_grow(&w_buf, &w_cap, w_len + 4) != 0) goto fail;"
            );
            let _ = writeln!(self.out, "        u_dheader_pos = w_len; w_len += 4;");
            let _ = writeln!(self.out, "    }}");
            let _ = writeln!(
                self.out,
                "    size_t u_body_start = w_len; (void)u_body_start;"
            );
        }
        // Discriminator.
        let disc_ts = switch_type_spec(&udef.switch_type);
        self.emit_member_write(&format!("({var})._d"), &disc_ts)?;
        let _ = writeln!(self.out, "    switch (({var})._d) {{");
        let mut default_case: Option<&Case> = None;
        for case in &udef.cases {
            let field = union_case_field(case);
            let mut has_value_label = false;
            for label in &case.labels {
                match label {
                    CaseLabel::Value(expr) => {
                        has_value_label = true;
                        let lit = self.case_label_literal(expr)?;
                        let _ = writeln!(self.out, "    case {lit}:");
                    }
                    CaseLabel::Default => {
                        default_case = Some(case);
                    }
                }
            }
            if has_value_label {
                self.emit_member_write(&format!("({var})._u.{field}"), &case.element.type_spec)?;
                let _ = writeln!(self.out, "        break;");
            }
        }
        if let Some(case) = default_case {
            let field = union_case_field(case);
            let _ = writeln!(self.out, "    default:");
            self.emit_member_write(&format!("({var})._u.{field}"), &case.element.type_spec)?;
            let _ = writeln!(self.out, "        break;");
        } else {
            let _ = writeln!(self.out, "    default: break;");
        }
        let _ = writeln!(self.out, "    }}");
        if u_dheader {
            let _ = writeln!(
                self.out,
                "    if (representation) {{ zerodds_xcdr2_c_put_u32_at(w_buf, u_dheader_pos, (uint32_t)(w_len - u_body_start)); }}"
            );
            let _ = writeln!(self.out, "    }}");
        }
        Ok(())
    }

    /// Inline-encode a nested struct's members by value (Plain-CDR2, no DHEADER).
    /// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
    fn emit_nested_struct_write(&mut self, var: &str, ndef: &StructDef) -> Result<(), CppGenError> {
        // FINDING T1b: a NON-`@final` nested struct is delimited — it carries its
        // own leading DHEADER, exactly as its top-level `_encode` would emit
        // (XTypes §7.4.3.5 / §7.4.4.4). When such a struct is a @mutable member
        // or a sequence element, its DHEADER doubles as the EMHEADER NEXTINT
        // (LengthCode 5, picked by `mutable_compact_lc`). `@mutable` additionally
        // frames each of its own members with an EMHEADER; `@appendable` splices
        // its members tight after the DHEADER. A `@final` nested struct has no
        // DHEADER and tight-packs its body (the original inline form).
        let ext = extensibility_of(&ndef.annotations);
        if !matches!(ext, Extensibility::Final) {
            // XCDR2: the non-@final nested struct carries its own DHEADER.
            // XCDR1: no DHEADER (classic CDR) — tight-packed body. (A nested
            // @mutable struct under XCDR1 would need PL_CDR1 framing; the corpus
            // has only @appendable nested structs, which splice tight.)
            let _ = writeln!(self.out, "    {{");
            let _ = writeln!(
                self.out,
                "    size_t nst_dheader_pos = 0; (void)nst_dheader_pos;"
            );
            let _ = writeln!(self.out, "    if (representation) {{");
            let _ = writeln!(
                self.out,
                "        if (zerodds_xcdr2_c_grow(&w_buf, &w_cap, w_len + 4) != 0) goto fail;"
            );
            let _ = writeln!(self.out, "        nst_dheader_pos = w_len; w_len += 4;");
            let _ = writeln!(self.out, "    }}");
            let _ = writeln!(
                self.out,
                "    size_t nst_body_start = w_len; (void)nst_body_start;"
            );
            if matches!(ext, Extensibility::Mutable) {
                self.emit_mutable_member_writes_base(ndef, &format!("({var})."))?;
            } else {
                self.emit_nested_struct_inline_write(var, ndef)?;
            }
            let _ = writeln!(
                self.out,
                "    if (representation) {{ zerodds_xcdr2_c_put_u32_at(w_buf, nst_dheader_pos, (uint32_t)(w_len - nst_body_start)); }}"
            );
            let _ = writeln!(self.out, "    }}");
            return Ok(());
        }
        self.emit_nested_struct_inline_write(var, ndef)
    }

    /// Tight-packed (`@final`) inline write of a nested struct's members by
    /// value — Plain-CDR2, no DHEADER, no per-member EMHEADER. Also serves as the
    /// `@appendable` body (members after the DHEADER frame).
    /// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
    fn emit_nested_struct_inline_write(
        &mut self,
        var: &str,
        ndef: &StructDef,
    ) -> Result<(), CppGenError> {
        for member in &ndef.members {
            let optional = is_optional(&member.annotations);
            for decl in &member.declarators {
                let f = &decl.name().text;
                let dims = effective_array_dims(self.reg, &member.type_spec, decl)?;
                if optional {
                    // Plain-CDR2 optional (XTypes §7.4.3): a boolean presence
                    // flag, then the value only if present. The nested struct's
                    // own typedef carries the `<f>_present` companion (emitted by
                    // emit_struct), so the field access is symmetric.
                    let _ = writeln!(
                        self.out,
                        "    if (zerodds_xcdr2_c_write_u8(&w_buf, &w_len, &w_cap, ({var}).{f}_present ? 1 : 0) != 0) goto fail;"
                    );
                    let _ = writeln!(self.out, "    if (({var}).{f}_present) {{");
                    self.emit_array_or_scalar_write(
                        &format!("({var}).{f}"),
                        &member.type_spec,
                        &dims,
                    )?;
                    let _ = writeln!(self.out, "    }}");
                } else {
                    self.emit_array_or_scalar_write(
                        &format!("({var}).{f}"),
                        &member.type_spec,
                        &dims,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn emit_primitive_write(&mut self, var: &str, p: PrimitiveType) -> Result<(), CppGenError> {
        let helper = match p {
            PrimitiveType::Boolean | PrimitiveType::Octet => "u8",
            PrimitiveType::Char => "u8",
            PrimitiveType::WideChar => "u16",
            PrimitiveType::Integer(IntegerType::Short)
            | PrimitiveType::Integer(IntegerType::Int16) => "i16",
            PrimitiveType::Integer(IntegerType::UShort)
            | PrimitiveType::Integer(IntegerType::UInt16) => "u16",
            PrimitiveType::Integer(IntegerType::Long)
            | PrimitiveType::Integer(IntegerType::Int32) => "i32",
            PrimitiveType::Integer(IntegerType::ULong)
            | PrimitiveType::Integer(IntegerType::UInt32) => "u32",
            PrimitiveType::Integer(IntegerType::LongLong)
            | PrimitiveType::Integer(IntegerType::Int64) => "i64",
            PrimitiveType::Integer(IntegerType::ULongLong)
            | PrimitiveType::Integer(IntegerType::UInt64) => "u64",
            PrimitiveType::Integer(IntegerType::Int8) => "i8",
            PrimitiveType::Integer(IntegerType::UInt8) => "u8",
            PrimitiveType::Floating(FloatingType::Float) => "f32",
            PrimitiveType::Floating(FloatingType::Double) => "f64",
            PrimitiveType::Floating(FloatingType::LongDouble) => {
                return Err(unsupported("long double"));
            }
        };
        if matches!(helper, "u64" | "i64" | "f64") {
            // 8-byte primitive: XCDR2 caps alignment at 4 (`zd_x2_*`), XCDR1 uses
            // the shared align-8 helper (classic CDR, MAXALIGN 8). `representation`
            // is in scope in `_encode_repr` / `_write_body`.
            let _ = writeln!(
                self.out,
                "    if (representation) {{ if (zd_x2_write_{helper}(&w_buf, &w_len, &w_cap, {var}) != 0) goto fail; }}"
            );
            let _ = writeln!(
                self.out,
                "    else {{ if (zerodds_xcdr2_c_write_{helper}(&w_buf, &w_len, &w_cap, {var}) != 0) goto fail; }}"
            );
            return Ok(());
        }
        let prefix = Self::helper_call_prefix(helper);
        let _ = writeln!(
            self.out,
            "    if ({prefix}write_{helper}(&w_buf, &w_len, &w_cap, {var}) != 0) goto fail;"
        );
        Ok(())
    }

    fn emit_sequence_write(
        &mut self,
        var: &str,
        elem: &TypeSpec,
        bound: Option<&ConstExpr>,
    ) -> Result<(), CppGenError> {
        // Sequence-Repraesentation in C: `struct { uint32_t len; T* elems; }`.
        // XCDR2 §7.4.3.5: non-primitive elements (string, struct, nested
        // sequence, …) get a DHEADER (uint32 = byte length of
        // [count + elements]) prepended; primitives (incl. enum→int32) do not.
        // Cyclone-DDS-verified.
        let non_primitive = !self.seq_elem_is_primitive(elem);
        // Block-scopes the DHEADER locals (multiple sequences per struct).
        let _ = writeln!(self.out, "    {{");
        // Bounded sequence<T,N>: reject more than N elements (XTypes §7.4.13.4.2)
        // — over-bound must error, not silently corrupt the wire.
        if let Some(n) = self.eval_bound(bound) {
            let _ = writeln!(self.out, "    if (({var}).len > {n}u) goto fail;");
        }
        if non_primitive {
            // DHEADER only under XCDR2; XCDR1 (classic CDR) has no collection
            // delimiter — just the count + elements.
            let _ = writeln!(
                self.out,
                "    size_t seq_dheader_pos = 0; (void)seq_dheader_pos;"
            );
            let _ = writeln!(self.out, "    if (representation) {{");
            let _ = writeln!(
                self.out,
                "        if (zerodds_xcdr2_c_grow(&w_buf, &w_cap, w_len + 4) != 0) goto fail;"
            );
            let _ = writeln!(self.out, "        seq_dheader_pos = w_len; w_len += 4;");
            let _ = writeln!(self.out, "    }}");
            let _ = writeln!(
                self.out,
                "    size_t seq_body_start = w_len; (void)seq_body_start;"
            );
        }
        let _ = writeln!(
            self.out,
            "    if (zerodds_xcdr2_c_write_u32(&w_buf, &w_len, &w_cap, ({var}).len) != 0) goto fail;"
        );
        let iv = format!("i{}", self.coll_depth);
        let _ = writeln!(
            self.out,
            "    for (uint32_t {iv} = 0; {iv} < ({var}).len; ++{iv}) {{"
        );
        // Delegate the element body to the generic member writer so structs,
        // enums and nested sequences (sequence<sequence<T>>) all work (#43,
        // sequence<non-primitive T>). The element loop counter is depth-scoped
        // so a nested sequence does not shadow the outer index.
        self.coll_depth += 1;
        let r = self.emit_member_write(&format!("({var}).elems[{iv}]"), elem);
        self.coll_depth -= 1;
        r?;
        let _ = writeln!(self.out, "    }}");
        if non_primitive {
            let _ = writeln!(
                self.out,
                "    if (representation) {{ zerodds_xcdr2_c_put_u32_at(w_buf, seq_dheader_pos, (uint32_t)(w_len - seq_body_start)); }}"
            );
        }
        let _ = writeln!(self.out, "    }}");
        Ok(())
    }

    fn emit_mutable_member_writes(&mut self, def: &StructDef) -> Result<(), CppGenError> {
        self.emit_mutable_member_writes_base(def, "s->")
    }

    /// Emits the per-member EMHEADER-framed writes of a @mutable struct, with the
    /// member accessors prefixed by `base` (`"s->"` for a top-level encode, or
    /// `"(<var>)."` when this struct is a nested @mutable member / sequence
    /// element). Splits out so a nested @mutable struct reuses the SAME framing.
    fn emit_mutable_member_writes_base(
        &mut self,
        def: &StructDef,
        base: &str,
    ) -> Result<(), CppGenError> {
        // XTypes 1.3 §7.3.4.3: `@autoid` defaults to SEQUENTIAL — a @mutable member
        // without explicit `@id(N)` takes the next 0-based declaration-order id
        // (vendor-confirmed byte-identical to CycloneDDS). Explicit `@id(N)` sets
        // the id and resets the counter to N+1. (Previously the C backend rejected
        // auto-id @mutable members outright.)
        let mut auto_id: u32 = 0;
        for member in &def.members {
            let id = id_annotation(&member.annotations).unwrap_or(auto_id);
            auto_id = id + 1;
            let dims_per_decl: Vec<Vec<u64>> = member
                .declarators
                .iter()
                .map(|d| effective_array_dims(self.reg, &member.type_spec, d))
                .collect::<Result<_, _>>()?;
            let optional = is_optional(&member.annotations);
            for (decl, dims) in member.declarators.iter().zip(dims_per_decl.iter()) {
                let f = format!("{base}{}", decl.name().text);
                let f = f.as_str();
                // Bug XV-mut: pick the COMPACT length code (XTypes 1.3 §7.4.3.4.2)
                // mirroring the Rust reference (`mutable_member_length_code`):
                //   * a fixed-size scalar primitive uses LC by wire size
                //     (1→LC0, 2→LC1, 4→LC2, 8→LC3) — NO NEXTINT, body inline;
                //   * a string/wstring uses LC5 — its own uint32 length prefix
                //     doubles as the NEXTINT (no separate NEXTINT);
                //   * everything else (arrays, sequences, maps, nested aggregates,
                //     long double) falls back to the universal LC4 + NEXTINT.
                let compact_lc = mutable_compact_lc(self.reg, &member.type_spec, dims, optional);
                match compact_lc {
                    Some(lc) => {
                        // LC0..3 (fixed primitive) and LC5 (string): EMHEADER then
                        // the body straight into w_buf, NO separate NEXTINT.
                        let emheader: u32 = (u32::from(lc) << 28) | id;
                        let _ = writeln!(self.out, "    {{");
                        let _ = writeln!(
                            self.out,
                            "        if (zerodds_xcdr2_c_write_u32(&w_buf, &w_len, &w_cap, 0x{emheader:08X}u) != 0) goto fail;"
                        );
                        self.emit_array_or_scalar_write(f, &member.type_spec, dims)?;
                        let _ = writeln!(self.out, "    }}");
                    }
                    None => {
                        // Universal LC4 (NEXTINT = body byte length), back-patched.
                        let emheader: u32 = (4u32 << 28) | id;
                        let _ = writeln!(self.out, "    {{");
                        let _ = writeln!(
                            self.out,
                            "        if (zerodds_xcdr2_c_write_u32(&w_buf, &w_len, &w_cap, 0x{emheader:08X}u) != 0) goto fail;"
                        );
                        let _ = writeln!(
                            self.out,
                            "        if (zerodds_xcdr2_c_grow(&w_buf, &w_cap, w_len + 4) != 0) goto fail;"
                        );
                        let _ = writeln!(self.out, "        size_t m_nextint_pos = w_len;");
                        let _ = writeln!(self.out, "        w_len += 4;");
                        let _ = writeln!(self.out, "        size_t m_body_start = w_len;");
                        self.emit_array_or_scalar_write(f, &member.type_spec, dims)?;
                        let _ = writeln!(
                            self.out,
                            "        uint32_t m_body_len = (uint32_t)(w_len - m_body_start);"
                        );
                        let _ = writeln!(
                            self.out,
                            "        zerodds_xcdr2_c_put_u32_at(w_buf, m_nextint_pos, m_body_len);"
                        );
                        let _ = writeln!(self.out, "    }}");
                    }
                }
            }
        }
        Ok(())
    }

    /// Emits the @mutable members under XCDR1 as a PL_CDR1 parameter list. The C
    /// runtime aligns relative to the buffer start, but a PL_CDR1 member body
    /// must align param-relative (origin 0) — so each member body is encoded
    /// into a FRESH temp buffer (save/swap of `w_buf`), then spliced through
    /// `pl1_write_member` (PID header + body + pad-to-4). Ends with the sentinel.
    /// Mirrors `emit_mutable_member_writes_base`'s id assignment.
    fn emit_pl_cdr1_member_writes(&mut self, def: &StructDef) -> Result<(), CppGenError> {
        let mut auto_id: u32 = 0;
        for member in &def.members {
            let id = id_annotation(&member.annotations).unwrap_or(auto_id);
            auto_id = id + 1;
            let dims_per_decl: Vec<Vec<u64>> = member
                .declarators
                .iter()
                .map(|d| effective_array_dims(self.reg, &member.type_spec, d))
                .collect::<Result<_, _>>()?;
            let optional = is_optional(&member.annotations);
            for (decl, dims) in member.declarators.iter().zip(dims_per_decl.iter()) {
                let fname = decl.name().text.clone();
                let f = format!("s->{fname}");
                if optional {
                    // PL_CDR1 optional: present -> emit the parameter; absent ->
                    // omit it (no present flag; absence = not in the list).
                    let _ = writeln!(self.out, "    if (s->{fname}_present) {{");
                }
                let _ = writeln!(self.out, "    {{");
                // Save the main buffer; redirect w_* to a fresh temp so the body
                // is encoded param-relative (origin 0).
                let _ = writeln!(
                    self.out,
                    "        uint8_t* m_buf = w_buf; size_t m_len = w_len; size_t m_cap = w_cap;"
                );
                let _ = writeln!(self.out, "        w_buf = NULL; w_len = 0; w_cap = 0;");
                self.emit_array_or_scalar_write(&f, &member.type_spec, dims)?;
                // Splice [PID header][temp body][pad] into the saved main buffer.
                let _ = writeln!(
                    self.out,
                    "        if (zerodds_xcdr2_c_pl1_write_member(&m_buf, &m_len, &m_cap, {id}u, w_buf, w_len) != 0) {{ free(w_buf); w_buf = m_buf; w_len = m_len; w_cap = m_cap; goto fail; }}"
                );
                let _ = writeln!(
                    self.out,
                    "        free(w_buf); w_buf = m_buf; w_len = m_len; w_cap = m_cap;"
                );
                let _ = writeln!(self.out, "    }}");
                if optional {
                    let _ = writeln!(self.out, "    }}");
                }
            }
        }
        // PID_LIST_END sentinel terminates the parameter list.
        let _ = writeln!(
            self.out,
            "    if (zerodds_xcdr2_c_pl1_sentinel(&w_buf, &w_len, &w_cap) != 0) goto fail;"
        );
        Ok(())
    }

    /// Reads a @mutable struct under XCDR1 as a PL_CDR1 parameter list: read each
    /// member header, dispatch on its id, and decode the body from the parameter
    /// sub-buffer (`buf + body_start`, local pos 0 → param-relative alignment),
    /// then advance past the (padded) parameter. Symmetric to
    /// `emit_pl_cdr1_member_writes`; mirrors its id assignment.
    fn emit_pl_cdr1_member_reads(&mut self, def: &StructDef) -> Result<(), CppGenError> {
        let _ = writeln!(self.out, "    while (pos + 4 <= len) {{");
        let _ = writeln!(
            self.out,
            "        uint32_t pl_id = 0; size_t pl_blen = 0; int pl_end = 0;"
        );
        let _ = writeln!(
            self.out,
            "        if (zerodds_xcdr2_c_pl1_read_header(buf, len, &pos, &pl_id, &pl_blen, &pl_end, zd_be) != 0) return -7;"
        );
        let _ = writeln!(self.out, "        if (pl_end) break;");
        let _ = writeln!(self.out, "        if (pos + pl_blen > len) return -7;");
        let _ = writeln!(self.out, "        size_t pl_body_start = pos;");
        let _ = writeln!(self.out, "        switch (pl_id) {{");
        let mut auto_id: u32 = 0;
        for member in &def.members {
            let id = id_annotation(&member.annotations).unwrap_or(auto_id);
            auto_id = id + 1;
            let dims_per_decl: Vec<Vec<u64>> = member
                .declarators
                .iter()
                .map(|d| effective_array_dims(self.reg, &member.type_spec, d))
                .collect::<Result<_, _>>()?;
            let optional = is_optional(&member.annotations);
            for (decl, dims) in member.declarators.iter().zip(dims_per_decl.iter()) {
                let fname = decl.name().text.clone();
                let f = format!("s->{fname}");
                let _ = writeln!(self.out, "        case {id}u: {{");
                // Redirect the cursor to the parameter body (origin 0).
                let _ = writeln!(
                    self.out,
                    "            const uint8_t* mb = buf; size_t ml = len; size_t mp = pos;"
                );
                let _ = writeln!(
                    self.out,
                    "            buf = mb + pl_body_start; len = pl_blen; pos = 0;"
                );
                if optional {
                    let _ = writeln!(self.out, "            s->{fname}_present = 1;");
                }
                self.emit_array_or_scalar_read(&f, &member.type_spec, dims)?;
                let _ = writeln!(self.out, "            buf = mb; len = ml; pos = mp;");
                let _ = writeln!(self.out, "            break;");
                let _ = writeln!(self.out, "        }}");
            }
        }
        let _ = writeln!(self.out, "        default: break;");
        let _ = writeln!(self.out, "        }}");
        let _ = writeln!(self.out, "        pos = pl_body_start + pl_blen;");
        // Skip the trailing pad to the next 4-byte boundary.
        let _ = writeln!(
            self.out,
            "        {{ size_t pl_pad = (4u - (pl_blen % 4u)) % 4u; pos += pl_pad; if (pos > len) pos = len; }}"
        );
        let _ = writeln!(self.out, "    }}");
        Ok(())
    }

    fn emit_decode_body(
        &mut self,
        c_name: &str,
        def: &StructDef,
        ext: Extensibility,
    ) -> Result<(), CppGenError> {
        // The decoder is symmetric to the encoder; builds the Rust-stack-style
        // BufferReader via helper inline functions from `zerodds_xcdr2.h`.
        // `_decode`/`_decode_e` keep the XCDR2 ABI (typesupport .decode pointer);
        // `_decode_repr` is the representation-aware entry (header typedef
        // `zerodds_decode_repr_fn`); all funnel into `_decode_core(buf,len,out,
        // zd_be,representation)`. representation: 0=XCDR1 (no DHEADER, max_align 8,
        // @mutable=PL_CDR1), 1=XCDR2.
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode_core(const uint8_t* buf, size_t len, void* out_sample, int zd_be, int representation);"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode(const uint8_t* buf, size_t len, void* out_sample) {{ return {c_name}_decode_core(buf, len, out_sample, 0, 1); }}"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode_e(const uint8_t* buf, size_t len, void* out_sample, int zd_be) {{ return {c_name}_decode_core(buf, len, out_sample, zd_be, 1); }}"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode_repr(const uint8_t* buf, size_t len, uint8_t representation, void* out_sample) {{ return {c_name}_decode_core(buf, len, out_sample, 0, representation ? 1 : 0); }}"
        );
        let _ = writeln!(
            self.out,
            "static int {c_name}_decode_core(const uint8_t* buf, size_t len, void* out_sample, int zd_be, int representation) {{"
        );
        let _ = writeln!(self.out, "    {c_name}_t* s = ({c_name}_t*)out_sample;");
        let _ = writeln!(self.out, "    size_t pos = 0;");
        let _ = writeln!(
            self.out,
            "    size_t zd_ma = representation ? 4 : 8; (void)zd_ma;"
        );
        match ext {
            Extensibility::Final => {
                self.emit_struct_body_reads(def)?;
            }
            Extensibility::Appendable => {
                // XCDR2 reads the DHEADER; XCDR1 has none (body runs to `len`).
                let _ = writeln!(self.out, "    size_t body_end = len;");
                let _ = writeln!(self.out, "    if (representation) {{");
                let _ = writeln!(self.out, "        uint32_t dheader = 0;");
                let _ = writeln!(
                    self.out,
                    "        if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &dheader, zd_be) != 0) return -7;"
                );
                let _ = writeln!(self.out, "        body_end = pos + dheader;");
                let _ = writeln!(self.out, "        if (body_end > len) return -7;");
                let _ = writeln!(self.out, "    }}");
                self.emit_struct_body_reads(def)?;
                let _ = writeln!(self.out, "    if (representation) pos = body_end;");
            }
            Extensibility::Mutable => {
                let _ = writeln!(self.out, "    if (!representation) {{");
                self.emit_pl_cdr1_member_reads(def)?;
                let _ = writeln!(self.out, "    }} else {{");
                let _ = writeln!(self.out, "    uint32_t dheader = 0;");
                let _ = writeln!(
                    self.out,
                    "    if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &dheader, zd_be) != 0) return -7;"
                );
                let _ = writeln!(self.out, "    size_t body_end = pos + dheader;");
                let _ = writeln!(self.out, "    if (body_end > len) return -7;");
                let _ = writeln!(self.out, "    while (pos < body_end) {{");
                let _ = writeln!(self.out, "        uint32_t emheader = 0;");
                let _ = writeln!(
                    self.out,
                    "        if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &emheader, zd_be) != 0) return -7;"
                );
                let _ = writeln!(self.out, "        uint32_t mid = emheader & 0x0FFFFFFFu;");
                let _ = writeln!(self.out, "        uint32_t lc = (emheader >> 28) & 0x7u;");
                let _ = writeln!(self.out, "        uint32_t nextint = 0;");
                let _ = writeln!(self.out, "        size_t body_len = 0;");
                // Bug XV-mut: compact length codes. LC0..3 = fixed 1/2/4/8-byte
                // bodies, NO NEXTINT. LC4/6/7 carry a separate NEXTINT (consumed
                // here). LC5 = the body's OWN leading uint32 length word doubles as
                // the NEXTINT — so it must NOT be consumed separately; we PEEK it
                // (read via a scratch position) and leave `pos` at the body start so
                // the member reader sees its length prefix intact.
                let _ = writeln!(
                    self.out,
                    "        if (lc == 0) body_len = 1; else if (lc == 1) body_len = 2; else if (lc == 2) body_len = 4; else if (lc == 3) body_len = 8;"
                );
                let _ = writeln!(self.out, "        else if (lc == 5) {{");
                let _ = writeln!(self.out, "            size_t peek_pos = pos;");
                let _ = writeln!(
                    self.out,
                    "            if (zerodds_xcdr2_c_read_u32(buf, len, &peek_pos, &nextint, zd_be) != 0) return -7;"
                );
                let _ = writeln!(self.out, "            body_len = 4u + (size_t)nextint;");
                let _ = writeln!(self.out, "        }} else {{");
                let _ = writeln!(
                    self.out,
                    "            if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &nextint, zd_be) != 0) return -7;"
                );
                let _ = writeln!(self.out, "            body_len = nextint;");
                let _ = writeln!(self.out, "        }}");
                let _ = writeln!(
                    self.out,
                    "        if (pos + body_len > body_end) return -7;"
                );
                self.emit_mutable_member_dispatch(def)?;
                let _ = writeln!(self.out, "    }}"); // close while
                let _ = writeln!(self.out, "    }}"); // close the XCDR2 `else`
            }
        }
        let _ = writeln!(self.out, "    (void)s;");
        let _ = writeln!(self.out, "    return 0;");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(self.out);
        Ok(())
    }

    fn emit_struct_body_reads(&mut self, def: &StructDef) -> Result<(), CppGenError> {
        for member in &def.members {
            let optional = is_optional(&member.annotations);
            for decl in &member.declarators {
                let f = &decl.name().text;
                let dims = effective_array_dims(self.reg, &member.type_spec, decl)?;
                if optional {
                    let _ = writeln!(self.out, "    {{");
                    let _ = writeln!(self.out, "        uint8_t present = 0;");
                    let _ = writeln!(
                        self.out,
                        "        if (zerodds_xcdr2_c_read_u8(buf, len, &pos, &present, zd_be) != 0) return -7;"
                    );
                    let _ = writeln!(self.out, "        s->{f}_present = present;");
                    let _ = writeln!(self.out, "        if (present) {{");
                    self.emit_array_or_scalar_read(&format!("s->{f}"), &member.type_spec, &dims)?;
                    let _ = writeln!(self.out, "        }}");
                    let _ = writeln!(self.out, "    }}");
                } else {
                    self.emit_array_or_scalar_read(&format!("s->{f}"), &member.type_spec, &dims)?;
                }
            }
        }
        Ok(())
    }

    fn emit_array_or_scalar_read(
        &mut self,
        var: &str,
        type_spec: &TypeSpec,
        dims: &[u64],
    ) -> Result<(), CppGenError> {
        if dims.is_empty() {
            return self.emit_member_read(var, type_spec);
        }
        // Bug XV-arr: symmetric to the encode — only a NON-primitive-element array
        // carries a collection DHEADER. A PARRAY (primitive element, any dims) has
        // none, so nothing to read+discard.
        let needs_dheader = !self.seq_elem_is_primitive(type_spec);
        if needs_dheader {
            let _ = writeln!(
                self.out,
                "    if (representation) {{ uint32_t arr_dheader = 0; if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &arr_dheader, zd_be) != 0) return -7; (void)arr_dheader; }}"
            );
        }
        let mut acc = var.to_string();
        for (d, _n) in dims.iter().enumerate() {
            let iv = format!("ri{d}");
            let _ = writeln!(
                self.out,
                "    for (uint32_t {iv} = 0; {iv} < {n}; ++{iv}) {{",
                n = dims[d]
            );
            acc = format!("{acc}[{iv}]");
        }
        self.emit_member_read(&acc, type_spec)?;
        for _ in dims {
            let _ = writeln!(self.out, "    }}");
        }
        Ok(())
    }

    /// zerodds-lint: recursion-depth 64 (nested struct members; bounded by IDL)
    fn emit_member_read(&mut self, var: &str, type_spec: &TypeSpec) -> Result<(), CppGenError> {
        let resolved = resolve_alias(self.reg, type_spec);
        match &resolved {
            TypeSpec::Primitive(p) => {
                let helper = primitive_helper(*p)?;
                if matches!(helper, "u64" | "i64" | "f64") {
                    // 8-byte primitive: XCDR2 caps alignment at 4 (`zd_x2_*`),
                    // XCDR1 reads at MAXALIGN 8 via the shared helper.
                    let _ = writeln!(
                        self.out,
                        "    if (representation) {{ if (zd_x2_read_{helper}(buf, len, &pos, &({var}), zd_be) != 0) return -7; }}"
                    );
                    let _ = writeln!(
                        self.out,
                        "    else {{ if (zerodds_xcdr2_c_read_{helper}(buf, len, &pos, &({var}), zd_be) != 0) return -7; }}"
                    );
                    return Ok(());
                }
                let prefix = Self::helper_call_prefix(helper);
                let _ = writeln!(
                    self.out,
                    "    if ({prefix}read_{helper}(buf, len, &pos, &({var}), zd_be) != 0) return -7;"
                );
                Ok(())
            }
            TypeSpec::String(st) => {
                if st.wide {
                    return self.emit_wstring_read(var);
                }
                let _ = writeln!(
                    self.out,
                    "    if (zerodds_xcdr2_c_read_string(buf, len, &pos, &({var}), zd_be) != 0) return -7;"
                );
                Ok(())
            }
            TypeSpec::Sequence(seq) => self.emit_sequence_read(var, &seq.elem),
            TypeSpec::Map(m) => self.emit_map_read(var, &m.key, &m.value),
            TypeSpec::Scoped(sc) => {
                let last = scoped_last(sc).ok_or_else(|| unsupported("empty scoped name"))?;
                if self.reg.enums.contains_key(&last) {
                    // Read the @bit_bound-width holder, sign-extend to the int32
                    // in-memory enum (XTypes §7.4.5.1).
                    let bytes = self.reg.enum_bytes.get(&last).copied().unwrap_or(4);
                    let line = match bytes {
                        1 => format!(
                            "    {{ uint8_t zd_e8 = 0; if (zerodds_xcdr2_c_read_u8(buf, len, &pos, &zd_e8, zd_be) != 0) return -7; ({var}) = (int32_t)(int8_t)zd_e8; }}"
                        ),
                        2 => format!(
                            "    {{ uint16_t zd_e16 = 0; if (zerodds_xcdr2_c_read_u16(buf, len, &pos, &zd_e16, zd_be) != 0) return -7; ({var}) = (int32_t)(int16_t)zd_e16; }}"
                        ),
                        _ => format!(
                            "    if (zerodds_xcdr2_c_read_i32(buf, len, &pos, (int32_t*)&({var}), zd_be) != 0) return -7;"
                        ),
                    };
                    let _ = writeln!(self.out, "{line}");
                    return Ok(());
                }
                // bitmask / bitset → read the holder integer (XTypes §7.3.1.2).
                if let Some(helper) = self.bits_helper(&last) {
                    let prefix = Self::helper_call_prefix(helper);
                    let _ = writeln!(
                        self.out,
                        "    if ({prefix}read_{helper}(buf, len, &pos, &({var}), zd_be) != 0) return -7;"
                    );
                    return Ok(());
                }
                if let Some((cn, ndef)) = self.reg.structs.get(&last).cloned() {
                    if self.reg.is_recursive(&last) {
                        // Symmetric to the encode: a NON-final recursive struct is
                        // DHEADER-delimited — read+discard its DHEADER before its
                        // body. (XTypes §7.4.3.5 / §7.4.4.4.)
                        let ext = extensibility_of(&ndef.annotations);
                        if !matches!(ext, Extensibility::Final) {
                            let _ = writeln!(
                                self.out,
                                "    if (representation) {{ uint32_t rec_dheader = 0; if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &rec_dheader, zd_be) != 0) return -7; (void)rec_dheader; }}"
                            );
                        }
                        let _ = writeln!(
                            self.out,
                            "    if ({cn}_read_body(buf, len, &pos, &({var}), zd_be, representation) != 0) return -7;"
                        );
                        return Ok(());
                    }
                    return self.emit_nested_struct_read(var, &ndef);
                }
                if let Some((cn, udef)) = self.reg.unions.get(&last).cloned() {
                    if self.reg.is_recursive(&last) {
                        let _ = writeln!(
                            self.out,
                            "    if ({cn}_read_body(buf, len, &pos, &({var}), zd_be, representation) != 0) return -7;"
                        );
                        return Ok(());
                    }
                    return self.emit_union_read(var, &udef);
                }
                Err(unsupported("unresolved scoped member read (C backend)"))
            }
            TypeSpec::Fixed(f) => {
                // fixed<P,S>: read the (P+2)/2 raw BCD octets into `.bcd[]`.
                let p = const_expr_to_u64(self.reg, &f.digits)
                    .ok_or_else(|| unsupported("fixed: non-constant digit count"))?;
                let n = (p + 2) / 2;
                let _ = writeln!(
                    self.out,
                    "    for (size_t __fi = 0; __fi < {n}; __fi++) {{ uint8_t __fb; if (zerodds_xcdr2_c_read_u8(buf, len, &pos, &__fb, zd_be) != 0) return -7; ({var}).bcd[__fi] = __fb; }}"
                );
                Ok(())
            }
            _ => Err(unsupported("complex member read")),
        }
    }

    /// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
    fn emit_nested_struct_read(&mut self, var: &str, ndef: &StructDef) -> Result<(), CppGenError> {
        // Symmetric to `emit_nested_struct_write`: a NON-`@final` nested struct is
        // DHEADER-delimited. `@mutable` parses the EMHEADER member loop bounded by
        // its own DHEADER; `@appendable` reads its members tight after the
        // DHEADER; `@final` reads the bare inline body. The depth counter scopes
        // the local names so nested @mutable structs don't collide.
        let ext = extensibility_of(&ndef.annotations);
        if !matches!(ext, Extensibility::Final) {
            let d = self.coll_depth;
            self.coll_depth += 1;
            let r = self.emit_nested_struct_read_delimited(var, ndef, ext, d);
            self.coll_depth -= 1;
            return r;
        }
        self.emit_nested_struct_inline_read(var, ndef)
    }

    /// Reads a DHEADER-delimited nested struct (`@appendable` / `@mutable`). For
    /// `@mutable`, runs the EMHEADER member-id dispatch loop bounded by the
    /// struct's own DHEADER; for `@appendable`, reads members tight then snaps to
    /// the DHEADER end. `depth` scopes the C local names.
    fn emit_nested_struct_read_delimited(
        &mut self,
        var: &str,
        ndef: &StructDef,
        ext: Extensibility,
        depth: u32,
    ) -> Result<(), CppGenError> {
        let _ = writeln!(self.out, "    {{");
        // XCDR2: read the nested struct's DHEADER (bounds the body). XCDR1: no
        // DHEADER — the body runs inline (no separate bound). (A @mutable nested
        // struct under XCDR1 would be PL_CDR1; the corpus has only @appendable
        // nested structs.)
        let _ = writeln!(
            self.out,
            "        size_t nst_end{depth} = len; (void)nst_end{depth};"
        );
        let _ = writeln!(self.out, "        if (representation) {{");
        let _ = writeln!(self.out, "        uint32_t nst_dheader{depth} = 0;");
        let _ = writeln!(
            self.out,
            "        if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &nst_dheader{depth}, zd_be) != 0) return -7;"
        );
        let _ = writeln!(
            self.out,
            "        nst_end{depth} = pos + nst_dheader{depth};"
        );
        let _ = writeln!(self.out, "        if (nst_end{depth} > len) return -7;");
        let _ = writeln!(self.out, "        }}");
        if matches!(ext, Extensibility::Mutable) {
            let _ = writeln!(
                self.out,
                "        while (representation && pos < nst_end{depth}) {{"
            );
            let _ = writeln!(self.out, "            uint32_t emheader{depth} = 0;");
            let _ = writeln!(
                self.out,
                "            if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &emheader{depth}, zd_be) != 0) return -7;"
            );
            let _ = writeln!(
                self.out,
                "            uint32_t mid{depth} = emheader{depth} & 0x0FFFFFFFu;"
            );
            let _ = writeln!(
                self.out,
                "            uint32_t lc{depth} = (emheader{depth} >> 28) & 0x7u;"
            );
            let _ = writeln!(self.out, "            uint32_t nextint{depth} = 0;");
            let _ = writeln!(self.out, "            size_t body_len{depth} = 0;");
            let _ = writeln!(
                self.out,
                "            if (lc{depth} == 0) body_len{depth} = 1; else if (lc{depth} == 1) body_len{depth} = 2; else if (lc{depth} == 2) body_len{depth} = 4; else if (lc{depth} == 3) body_len{depth} = 8;"
            );
            let _ = writeln!(self.out, "            else if (lc{depth} == 5) {{");
            let _ = writeln!(self.out, "                size_t peek_pos{depth} = pos;");
            let _ = writeln!(
                self.out,
                "                if (zerodds_xcdr2_c_read_u32(buf, len, &peek_pos{depth}, &nextint{depth}, zd_be) != 0) return -7;"
            );
            let _ = writeln!(
                self.out,
                "                body_len{depth} = 4u + (size_t)nextint{depth};"
            );
            let _ = writeln!(self.out, "            }} else {{");
            let _ = writeln!(
                self.out,
                "                if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &nextint{depth}, zd_be) != 0) return -7;"
            );
            let _ = writeln!(
                self.out,
                "                body_len{depth} = nextint{depth};"
            );
            let _ = writeln!(self.out, "            }}");
            let _ = writeln!(
                self.out,
                "            if (pos + body_len{depth} > nst_end{depth}) return -7;"
            );
            self.emit_mutable_member_dispatch_base(
                ndef,
                &format!("({var})."),
                &format!("mid{depth}"),
                &format!("body_len{depth}"),
            )?;
            let _ = writeln!(self.out, "        }}");
        } else {
            self.emit_nested_struct_inline_read(var, ndef)?;
            let _ = writeln!(
                self.out,
                "        if (representation) pos = nst_end{depth};"
            );
        }
        let _ = writeln!(self.out, "    }}");
        Ok(())
    }

    /// Tight-packed (`@final`) inline read of a nested struct's members. Also the
    /// `@appendable` body (members after the DHEADER).
    /// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
    fn emit_nested_struct_inline_read(
        &mut self,
        var: &str,
        ndef: &StructDef,
    ) -> Result<(), CppGenError> {
        for member in &ndef.members {
            let optional = is_optional(&member.annotations);
            for decl in &member.declarators {
                let f = &decl.name().text;
                let dims = effective_array_dims(self.reg, &member.type_spec, decl)?;
                if optional {
                    let _ = writeln!(self.out, "    {{");
                    let _ = writeln!(self.out, "        uint8_t present = 0;");
                    let _ = writeln!(
                        self.out,
                        "        if (zerodds_xcdr2_c_read_u8(buf, len, &pos, &present, zd_be) != 0) return -7;"
                    );
                    let _ = writeln!(self.out, "        ({var}).{f}_present = present;");
                    let _ = writeln!(self.out, "        if (present) {{");
                    self.emit_array_or_scalar_read(
                        &format!("({var}).{f}"),
                        &member.type_spec,
                        &dims,
                    )?;
                    let _ = writeln!(self.out, "        }}");
                    let _ = writeln!(self.out, "    }}");
                } else {
                    self.emit_array_or_scalar_read(
                        &format!("({var}).{f}"),
                        &member.type_spec,
                        &dims,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Read an XCDR2 wstring into a freshly-malloc'd NUL-terminated `uint16_t*`.
    fn emit_wstring_read(&mut self, var: &str) -> Result<(), CppGenError> {
        let _ = writeln!(self.out, "    {{");
        let _ = writeln!(self.out, "        uint32_t ws_bytes = 0;");
        let _ = writeln!(
            self.out,
            "        if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &ws_bytes, zd_be) != 0) return -7;"
        );
        let _ = writeln!(self.out, "        uint32_t ws_n = ws_bytes / 2u;");
        let _ = writeln!(
            self.out,
            "        uint16_t* ws_p = (uint16_t*)malloc(((size_t)ws_n + 1) * sizeof(uint16_t));"
        );
        let _ = writeln!(self.out, "        if (ws_p == NULL) return -7;");
        let _ = writeln!(
            self.out,
            "        for (uint32_t wi = 0; wi < ws_n; ++wi) {{ if (zerodds_xcdr2_c_read_u16(buf, len, &pos, &ws_p[wi], zd_be) != 0) {{ free(ws_p); return -7; }} }}"
        );
        let _ = writeln!(self.out, "        ws_p[ws_n] = 0;");
        let _ = writeln!(self.out, "        ({var}) = ws_p;");
        let _ = writeln!(self.out, "    }}");
        Ok(())
    }

    /// Read an XCDR2 map: skip DHEADER, read count, allocate parallel
    /// key/value arrays, read each pair.
    fn emit_map_read(
        &mut self,
        var: &str,
        key: &TypeSpec,
        value: &TypeSpec,
    ) -> Result<(), CppGenError> {
        let kc = c_type_for(self.reg, key)?;
        let vc = c_type_for(self.reg, value)?;
        let _ = writeln!(self.out, "    {{");
        // Symmetric to the encoder: the map DHEADER is 4-aligned and present
        // ONLY for a non-primitive (key,value) element (XCDR2 §7.4.3.5).
        let _ = writeln!(
            self.out,
            "        if (zerodds_xcdr2_c_pad_read(buf, len, &pos, 4) != 0) return -1;"
        );
        if !(self.seq_elem_is_primitive(key) && self.seq_elem_is_primitive(value)) {
            let _ = writeln!(self.out, "        if (representation) {{");
            let _ = writeln!(self.out, "        uint32_t map_dheader = 0;");
            let _ = writeln!(
                self.out,
                "        if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &map_dheader, zd_be) != 0) return -7;"
            );
            let _ = writeln!(self.out, "        (void)map_dheader;");
            let _ = writeln!(self.out, "        }}");
        }
        let _ = writeln!(self.out, "        uint32_t map_len = 0;");
        let _ = writeln!(
            self.out,
            "        if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &map_len, zd_be) != 0) return -7;"
        );
        let _ = writeln!(self.out, "        ({var}).len = map_len;");
        let _ = writeln!(
            self.out,
            "        ({var}).keys = ({kc}*)calloc(map_len ? map_len : 1, sizeof({kc}));"
        );
        let _ = writeln!(
            self.out,
            "        ({var}).vals = ({vc}*)calloc(map_len ? map_len : 1, sizeof({vc}));"
        );
        let _ = writeln!(
            self.out,
            "        if (((({var}).keys == NULL) || (({var}).vals == NULL)) && map_len > 0) return -7;"
        );
        let mv = format!("mi{}", self.coll_depth);
        let _ = writeln!(
            self.out,
            "        for (uint32_t {mv} = 0; {mv} < map_len; ++{mv}) {{"
        );
        self.coll_depth += 1;
        let rk = self.emit_member_read(&format!("({var}).keys[{mv}]"), key);
        let rv = if rk.is_ok() {
            self.emit_member_read(&format!("({var}).vals[{mv}]"), value)
        } else {
            Ok(())
        };
        self.coll_depth -= 1;
        rk.and(rv)?;
        let _ = writeln!(self.out, "        }}");
        let _ = writeln!(self.out, "    }}");
        Ok(())
    }

    /// Read a discriminated union: read the discriminator, then the selected
    /// member into the `_u` arm.
    fn emit_union_read(&mut self, var: &str, udef: &UnionDef) -> Result<(), CppGenError> {
        // Symmetric to emit_union_write: appendable/mutable unions carry a
        // 4-aligned DHEADER over [disc + branch]; @final does not.
        let u_dheader = !matches!(extensibility_of(&udef.annotations), Extensibility::Final);
        if u_dheader {
            // XCDR2: 4-align + the union DHEADER. XCDR1: no DHEADER (the
            // discriminator read below does its own alignment).
            let _ = writeln!(self.out, "    if (representation) {{");
            let _ = writeln!(
                self.out,
                "    if (zerodds_xcdr2_c_pad_read(buf, len, &pos, 4) != 0) return -1;"
            );
            let _ = writeln!(
                self.out,
                "    {{ uint32_t u_dh = 0; if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &u_dh, zd_be) != 0) return -7; (void)u_dh; }}"
            );
            let _ = writeln!(self.out, "    }}");
        }
        let disc_ts = switch_type_spec(&udef.switch_type);
        self.emit_member_read(&format!("({var})._d"), &disc_ts)?;
        let _ = writeln!(self.out, "    switch (({var})._d) {{");
        let mut default_case: Option<&Case> = None;
        for case in &udef.cases {
            let field = union_case_field(case);
            let mut has_value_label = false;
            for label in &case.labels {
                match label {
                    CaseLabel::Value(expr) => {
                        has_value_label = true;
                        let lit = self.case_label_literal(expr)?;
                        let _ = writeln!(self.out, "    case {lit}:");
                    }
                    CaseLabel::Default => {
                        default_case = Some(case);
                    }
                }
            }
            if has_value_label {
                self.emit_member_read(&format!("({var})._u.{field}"), &case.element.type_spec)?;
                let _ = writeln!(self.out, "        break;");
            }
        }
        if let Some(case) = default_case {
            let field = union_case_field(case);
            let _ = writeln!(self.out, "    default:");
            self.emit_member_read(&format!("({var})._u.{field}"), &case.element.type_spec)?;
            let _ = writeln!(self.out, "        break;");
        } else {
            let _ = writeln!(self.out, "    default: break;");
        }
        let _ = writeln!(self.out, "    }}");
        Ok(())
    }

    /// Fold a collection bound (`sequence<T,N>` / `string<N>` / `map<K,V,N>`)
    /// to its positive integer value, resolving named constants/enumerators via
    /// the symbol table. `None` for an unbounded collection.
    fn eval_bound(&self, bound: Option<&ConstExpr>) -> Option<u64> {
        let expr = bound?;
        evaluate(expr, &self.reg.consts)
            .ok()
            .and_then(|v| v.as_i64())
            .filter(|n| *n >= 0)
            .map(|n| n as u64)
    }

    /// Evaluate a union case label to a C-printable integer literal. Handles
    /// integer/char/boolean discriminators plus enum constants.
    fn case_label_literal(&self, expr: &ConstExpr) -> Result<String, CppGenError> {
        // integer / char / boolean / enumerator → integer C `case` value.
        // `as_i64` already covers Bool, Char, WChar and Enum ordinals.
        if let Ok(v) = evaluate(expr, &self.reg.consts) {
            if let Some(i) = v.as_i64() {
                return Ok(i.to_string());
            }
        }
        Err(unsupported("non-integer union case label (C backend)"))
    }

    fn emit_sequence_read(&mut self, var: &str, elem: &TypeSpec) -> Result<(), CppGenError> {
        let _ = writeln!(self.out, "    {{");
        // XCDR2 §7.4.3.5: for non-primitive elements, a DHEADER (uint32 before
        // [count + elements]) is present — skip it. enum→int32 is primitive.
        if !self.seq_elem_is_primitive(elem) {
            let _ = writeln!(self.out, "        if (representation) {{");
            let _ = writeln!(self.out, "        uint32_t seq_dheader = 0;");
            let _ = writeln!(
                self.out,
                "        if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &seq_dheader, zd_be) != 0) return -7;"
            );
            let _ = writeln!(self.out, "        (void)seq_dheader;");
            let _ = writeln!(self.out, "        }}");
        }
        let _ = writeln!(self.out, "        uint32_t seq_len = 0;");
        let _ = writeln!(
            self.out,
            "        if (zerodds_xcdr2_c_read_u32(buf, len, &pos, &seq_len, zd_be) != 0) return -7;"
        );
        let elem_c = c_type_for(self.reg, elem)?;
        let _ = writeln!(self.out, "        ({var}).len = seq_len;");
        let _ = writeln!(
            self.out,
            "        ({var}).elems = ({elem_c}*)calloc(seq_len ? seq_len : 1, sizeof({elem_c}));"
        );
        let _ = writeln!(
            self.out,
            "        if (({var}).elems == NULL && seq_len > 0) return -7;"
        );
        let iv = format!("i{}", self.coll_depth);
        let _ = writeln!(
            self.out,
            "        for (uint32_t {iv} = 0; {iv} < seq_len; ++{iv}) {{"
        );
        // Delegate to the generic member reader so structs, enums and nested
        // sequences all decode (#43, sequence<non-primitive T>). Depth-scoped
        // loop counter avoids inner/outer index shadowing.
        self.coll_depth += 1;
        let r = self.emit_member_read(&format!("({var}).elems[{iv}]"), elem);
        self.coll_depth -= 1;
        r?;
        let _ = writeln!(self.out, "        }}");
        let _ = writeln!(self.out, "    }}");
        Ok(())
    }

    /// True if a sequence element serializes as a plain primitive (no DHEADER):
    /// IDL primitives, and enum members (which become int32). Resolves through
    /// typedef aliases first.
    fn seq_elem_is_primitive(&self, elem: &TypeSpec) -> bool {
        let resolved = resolve_alias(self.reg, elem);
        match &resolved {
            TypeSpec::Primitive(_) => true,
            TypeSpec::Scoped(sc) => scoped_last(sc).is_some_and(|last| {
                // enum (→int32), bitmask/bitset (→holder uint) all serialize as
                // a plain primitive integer, so no element DHEADER.
                self.reg.enums.contains_key(&last)
                    || self.reg.bitmasks.contains_key(&last)
                    || self.reg.bitsets.contains_key(&last)
            }),
            _ => false,
        }
    }

    fn emit_mutable_member_dispatch(&mut self, def: &StructDef) -> Result<(), CppGenError> {
        self.emit_mutable_member_dispatch_base(def, "s->", "mid", "body_len")
    }

    /// EMHEADER member-id `switch` for a @mutable decode, with member accessors
    /// prefixed by `base` and the member-id / body-length expressions named by
    /// `mid_var` / `body_len_var` (so a nested @mutable struct can run its own
    /// depth-scoped loop).
    fn emit_mutable_member_dispatch_base(
        &mut self,
        def: &StructDef,
        base: &str,
        mid_var: &str,
        body_len_var: &str,
    ) -> Result<(), CppGenError> {
        let _ = writeln!(self.out, "        switch ({mid_var}) {{");
        // Sequential auto-id (XTypes §7.3.4.3 default), mirroring the encoder.
        let mut auto_id: u32 = 0;
        for member in &def.members {
            let id = id_annotation(&member.annotations).unwrap_or(auto_id);
            auto_id = id + 1;
            for decl in &member.declarators {
                let f = format!("{base}{}", decl.name().text);
                let dims = effective_array_dims(self.reg, &member.type_spec, decl)?;
                let _ = writeln!(self.out, "        case {id}: {{");
                self.emit_array_or_scalar_read(&f, &member.type_spec, &dims)?;
                let _ = writeln!(self.out, "            break;");
                let _ = writeln!(self.out, "        }}");
            }
        }
        let _ = writeln!(self.out, "        default: pos += {body_len_var}; break;");
        let _ = writeln!(self.out, "        }}");
        Ok(())
    }

    fn emit_free_body(&mut self, c_name: &str, def: &StructDef) {
        let _ = writeln!(
            self.out,
            "static void {c_name}_sample_free(void* sample) {{"
        );
        let _ = writeln!(self.out, "    if (sample == NULL) return;");
        let _ = writeln!(self.out, "    {c_name}_t* s = ({c_name}_t*)sample;");
        let _ = writeln!(self.out, "    (void)s;");
        for member in &def.members {
            let resolved = resolve_alias(self.reg, &member.type_spec);
            for decl in &member.declarators {
                let f = &decl.name().text;
                let dims =
                    effective_array_dims(self.reg, &member.type_spec, decl).unwrap_or_default();
                if dims.is_empty() {
                    // Scalar member.
                    self.emit_free_member(&format!("s->{f}"), &resolved);
                } else {
                    // Heap-owning members reached THROUGH a fixed array (e.g.
                    // `string names[3]`, `sequence<long> rows[2]`) must still be
                    // freed per element — previously skipped → leak.
                    let mut acc = format!("s->{f}");
                    for (d, n) in dims.iter().enumerate() {
                        let iv = format!("fi{d}");
                        let _ = writeln!(
                            self.out,
                            "    for (uint32_t {iv} = 0; {iv} < {n}; ++{iv}) {{"
                        );
                        acc = format!("{acc}[{iv}]");
                    }
                    self.emit_free_member(&acc, &resolved);
                    for _ in &dims {
                        let _ = writeln!(self.out, "    }}");
                    }
                }
            }
        }
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(self.out);
    }

    /// Free the heap-owned payload of a single member instance, reached via the
    /// accessor expression `acc` (e.g. `s->name`, `s->names[fi0]`). Resolved
    /// type drives the shape; primitives/enums/nested structs hold no heap.
    fn emit_free_member(&mut self, acc: &str, resolved: &TypeSpec) {
        match resolved {
            TypeSpec::String(_) => {
                // Both string (char*) and wstring (uint16_t*) are heap.
                let _ = writeln!(self.out, "    free({acc}); {acc} = NULL;");
            }
            TypeSpec::Sequence(seq) => {
                if matches!(resolve_alias(self.reg, &seq.elem), TypeSpec::String(_)) {
                    let _ = writeln!(
                        self.out,
                        "    for (uint32_t fsi = 0; fsi < ({acc}).len; ++fsi) free(({acc}).elems[fsi]);"
                    );
                }
                let _ = writeln!(
                    self.out,
                    "    free(({acc}).elems); ({acc}).elems = NULL; ({acc}).len = 0;"
                );
            }
            TypeSpec::Map(m) => {
                if matches!(resolve_alias(self.reg, &m.key), TypeSpec::String(_)) {
                    let _ = writeln!(
                        self.out,
                        "    for (uint32_t fki = 0; fki < ({acc}).len; ++fki) free(({acc}).keys[fki]);"
                    );
                }
                if matches!(resolve_alias(self.reg, &m.value), TypeSpec::String(_)) {
                    let _ = writeln!(
                        self.out,
                        "    for (uint32_t fvi = 0; fvi < ({acc}).len; ++fvi) free(({acc}).vals[fvi]);"
                    );
                }
                let _ = writeln!(self.out, "    free(({acc}).keys); ({acc}).keys = NULL;");
                let _ = writeln!(
                    self.out,
                    "    free(({acc}).vals); ({acc}).vals = NULL; ({acc}).len = 0;"
                );
            }
            _ => {}
        }
    }

    fn emit_key_hash_body(&mut self, c_name: &str, def: &StructDef) {
        // Spec §7.6.8: collects  members in PlainCdr2BeKeyHolder, then
        // either zero-pad or MD5. We use the XCDR2 C helpers.
        let _ = writeln!(
            self.out,
            "static int {c_name}_key_hash(const void* sample, uint8_t out_hash[16]) {{"
        );
        let _ = writeln!(
            self.out,
            "    const {c_name}_t* s = (const {c_name}_t*)sample;"
        );
        let _ = writeln!(self.out, "    uint8_t* kh_buf = NULL;");
        let _ = writeln!(self.out, "    size_t kh_len = 0;");
        let _ = writeln!(self.out, "    size_t kh_cap = 0;");
        for member in &def.members {
            if !is_key(&member.annotations) {
                continue;
            }
            for decl in &member.declarators {
                let f = &decl.name().text;
                match &member.type_spec {
                    TypeSpec::Primitive(p) => {
                        let helper = match primitive_helper(*p) {
                            Ok(h) => h,
                            Err(_) => continue,
                        };
                        let _ = writeln!(
                            self.out,
                            "    if (zerodds_xcdr2_c_kh_write_{helper}(&kh_buf, &kh_len, &kh_cap, s->{f}) != 0) {{ free(kh_buf); return -1; }}"
                        );
                    }
                    TypeSpec::String(_) => {
                        let _ = writeln!(
                            self.out,
                            "    if (zerodds_xcdr2_c_kh_write_string(&kh_buf, &kh_len, &kh_cap, s->{f}) != 0) {{ free(kh_buf); return -1; }}"
                        );
                    }
                    _ => {}
                }
            }
        }
        let _ = writeln!(
            self.out,
            "    zerodds_xcdr2_c_compute_key_hash(kh_buf, kh_len, /*max_size_static=*/0, out_hash);"
        );
        let _ = writeln!(self.out, "    free(kh_buf);");
        let _ = writeln!(self.out, "    return 0;");
        let _ = writeln!(self.out, "}}");
        let _ = writeln!(self.out);
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// An aggregate definition (struct or union) for the typedef-ordering pre-pass.
#[derive(Clone)]
enum AggDef {
    Struct(StructDef),
    Union(UnionDef),
}

/// Collect every struct/union definition with its module scope, in declaration
/// order, for the aggregate-typedef pre-pass.
/// zerodds-lint: recursion-depth 64 (module tree; bounded by IDL nesting)
fn collect_aggregates(
    defs: &[Definition],
    scope: &[String],
    out: &mut Vec<(String, Vec<String>, AggDef)>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let mut s = scope.to_vec();
                s.push(m.name.text.clone());
                collect_aggregates(&m.definitions, &s, out);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                out.push((
                    s.name.text.clone(),
                    scope.to_vec(),
                    AggDef::Struct(s.clone()),
                ));
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                out.push((
                    u.name.text.clone(),
                    scope.to_vec(),
                    AggDef::Union(u.clone()),
                ));
            }
            _ => {}
        }
    }
}

fn dds_type_name(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        let mut s = scope.join("::");
        s.push_str("::");
        s.push_str(name);
        s
    }
}

/// The TypeSpec a union discriminator maps to (so the discriminator can reuse
/// the generic member writer/reader + C type mapping).
fn switch_type_spec(s: &SwitchTypeSpec) -> TypeSpec {
    match s {
        SwitchTypeSpec::Integer(it) => TypeSpec::Primitive(PrimitiveType::Integer(*it)),
        SwitchTypeSpec::Char => TypeSpec::Primitive(PrimitiveType::Char),
        SwitchTypeSpec::Boolean => TypeSpec::Primitive(PrimitiveType::Boolean),
        SwitchTypeSpec::Octet => TypeSpec::Primitive(PrimitiveType::Octet),
        SwitchTypeSpec::Scoped(sn) => TypeSpec::Scoped(sn.clone()),
    }
}

/// The C field name for a union case's member (the case's declarator name).
fn union_case_field(case: &Case) -> String {
    case.element.declarator.name().text.clone()
}

/// Join a scope + name with the given separator (`::` for IDL paths).
fn scope_join(scope: &[String], name: &str, sep: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        let mut s = scope.join(sep);
        s.push_str(sep);
        s.push_str(name);
        s
    }
}

fn c_identifier(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        let mut s = scope.join("_");
        s.push('_');
        s.push_str(name);
        s
    }
}

impl Extensibility {
    fn as_u8(self) -> u8 {
        match self {
            Self::Final => 0,
            Self::Appendable => 1,
            Self::Mutable => 2,
        }
    }
}

fn extensibility_of(annotations: &[Annotation]) -> Extensibility {
    for a in annotations {
        if let Some(name) = a.name.parts.last() {
            match name.text.as_str() {
                "final" | "Final" => return Extensibility::Final,
                "appendable" | "Appendable" => return Extensibility::Appendable,
                "mutable" | "Mutable" => return Extensibility::Mutable,
                _ => {}
            }
        }
    }
    // Un-annotated default: FINAL. XTypes 1.3 §7.2.2.4.4 leaves the
    // extensibility of an un-annotated aggregate implementation-defined; the
    // canonical zerodds choice — anchored to the `zerodds-cdr` core and the
    // idl-rust reference (whose hardcoded default is Final, see
    // tools/idlc/src/default_ext.rs) — is FINAL, matching the rust golden:
    // SX2: spec default for an unannotated aggregate is APPENDABLE (§7.3.3.1).
    // `--default-extensibility final` opts back to the no-DHEADER form.
    Extensibility::Appendable
}

fn is_optional(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|a| {
        a.name
            .parts
            .last()
            .is_some_and(|p| p.text == "optional" || p.text == "Optional")
    })
}

/// XCDR2 wire byte size of a primitive scalar (XTypes 1.3 §7.4.1). Used to pick
/// the compact EMHEADER length code for a `@mutable` member.
fn primitive_wire_bytes(p: PrimitiveType) -> u32 {
    match p {
        PrimitiveType::Boolean | PrimitiveType::Octet | PrimitiveType::Char => 1,
        PrimitiveType::WideChar => 2,
        PrimitiveType::Integer(IntegerType::Int8 | IntegerType::UInt8) => 1,
        PrimitiveType::Integer(
            IntegerType::Short | IntegerType::Int16 | IntegerType::UShort | IntegerType::UInt16,
        ) => 2,
        PrimitiveType::Integer(
            IntegerType::Long | IntegerType::Int32 | IntegerType::ULong | IntegerType::UInt32,
        ) => 4,
        PrimitiveType::Integer(
            IntegerType::LongLong
            | IntegerType::Int64
            | IntegerType::ULongLong
            | IntegerType::UInt64,
        ) => 8,
        PrimitiveType::Floating(FloatingType::Float) => 4,
        PrimitiveType::Floating(FloatingType::Double) => 8,
        // long double (16 bytes) is not a compact code → falls back to LC4.
        PrimitiveType::Floating(FloatingType::LongDouble) => 16,
    }
}

/// Picks the compact XTypes 1.3 §7.4.3.4.2 length code for a `@mutable` member,
/// mirroring the Rust reference `mutable_member_length_code` (Bug XV-mut). Only
/// non-array, non-optional scalar primitives (LC0..3 by wire size) and
/// strings/wstrings (LC5, reusing the leading uint32 length prefix as the
/// NEXTINT) are eligible. Returns `None` for everything else → universal LC4.
fn mutable_compact_lc(
    reg: &TypeReg,
    type_spec: &TypeSpec,
    dims: &[u64],
    optional: bool,
) -> Option<u8> {
    if !dims.is_empty() || optional {
        return None;
    }
    let resolved = resolve_alias(reg, type_spec);
    match &resolved {
        TypeSpec::Primitive(p) => match primitive_wire_bytes(*p) {
            1 => Some(0),
            2 => Some(1),
            4 => Some(2),
            8 => Some(3),
            _ => None, // long double → LC4
        },
        // FINDING T1b (mirrors idl-rust `mutable_member_length_code` +
        // `member_body_has_leading_dheader`): a member whose XCDR2 body begins
        // with a 4-byte length word — a string/wstring length prefix, a
        // non-primitive sequence/map DHEADER, or a nested @appendable/@mutable
        // struct's DHEADER — uses LC5 to REUSE that word as the NEXTINT (no
        // redundant NEXTINT), matching CycloneDDS / RTI / FastDDS. A @final
        // nested struct (no DHEADER) and a sequence<primitive> (bare element
        // count, not a byte length) fall through to the universal LC4.
        _ if member_body_has_leading_dheader_c(reg, &resolved) => Some(5),
        _ => None,
    }
}

/// `true` if `type_spec`'s XCDR2 body begins with a 4-byte length/DHEADER word,
/// making it eligible for EMHEADER LengthCode 5 (the leading word doubles as the
/// NEXTINT). Mirrors idl-rust `type_map::member_body_has_leading_dheader`:
///   * string / wstring — leading uint32 octet-length prefix,
///   * map<K,V> — always a non-primitive aggregate → DHEADER,
///   * sequence<E> with NON-primitive `E` — XCDR2 DHEADER (sequence<primitive>
///     has only a bare element count, not a byte length → stays LC4),
///   * a nested struct (chasing typedef) whose extensibility is @appendable /
///     @mutable — self-delimits with a leading DHEADER; a @final nested struct
///     tight-packs its body (no DHEADER) → LC4.
///
/// zerodds-lint: recursion-depth 16
fn member_body_has_leading_dheader_c(reg: &TypeReg, type_spec: &TypeSpec) -> bool {
    match resolve_alias(reg, type_spec) {
        TypeSpec::String(_) => true,
        TypeSpec::Map(_) => true,
        TypeSpec::Sequence(seq) => !seq_elem_is_primitive_reg(reg, &seq.elem),
        TypeSpec::Scoped(sc) => scoped_last(&sc)
            .and_then(|last| reg.structs.get(&last).cloned())
            .is_some_and(|(_, ndef)| {
                !matches!(extensibility_of(&ndef.annotations), Extensibility::Final)
            }),
        _ => false,
    }
}

/// Free-function twin of `CSink::seq_elem_is_primitive` (a sequence element is
/// "primitive" — and carries no DHEADER — when it is an IDL primitive, enum,
/// bitmask or bitset). Kept standalone so the length-code picker can run without
/// a `&self` borrow.
fn seq_elem_is_primitive_reg(reg: &TypeReg, elem: &TypeSpec) -> bool {
    match resolve_alias(reg, elem) {
        TypeSpec::Primitive(_) => true,
        TypeSpec::Scoped(sc) => scoped_last(&sc).is_some_and(|last| {
            reg.enums.contains_key(&last)
                || reg.bitmasks.contains_key(&last)
                || reg.bitsets.contains_key(&last)
        }),
        _ => false,
    }
}

/// Fixed-array dimensions of a declarator (empty for a simple declarator).
///
/// A bound may be a literal **or a named constant** (`long v[N]`) — the latter
/// is resolved through the const symbol table the way every other backend does
/// (Bug C #43, const-array-bound).
fn array_dims(reg: &TypeReg, decl: &Declarator) -> Result<Vec<u64>, CppGenError> {
    match decl {
        Declarator::Simple(_) => Ok(Vec::new()),
        Declarator::Array(arr) => {
            let mut dims = Vec::with_capacity(arr.sizes.len());
            for sz in &arr.sizes {
                let n = const_expr_to_u64(reg, sz)
                    .ok_or_else(|| unsupported("non-literal array bound"))?;
                dims.push(n);
            }
            Ok(dims)
        }
    }
}

/// Effective fixed-array dimensions for a member, combining a typedef-to-array
/// alias's dims with the declarator's own dims. `Matrix3 transform;` where
/// `typedef long Matrix3[3][3];` yields `[3,3]` even though the member's own
/// declarator is simple (#43, typedef-to-array). Alias dims come first
/// (outermost), then the declarator dims, matching C array nesting order.
fn effective_array_dims(
    reg: &TypeReg,
    type_spec: &TypeSpec,
    decl: &Declarator,
) -> Result<Vec<u64>, CppGenError> {
    let mut dims = Vec::new();
    if let TypeSpec::Scoped(sc) = type_spec {
        if let Some(last) = scoped_last(sc) {
            if let Some(td) = reg.typedef_arrays.get(&last) {
                dims.extend_from_slice(td);
            }
        }
    }
    dims.extend(array_dims(reg, decl)?);
    Ok(dims)
}

/// Smallest `uintN_t` holder for a bit width (XTypes §7.3.1.2 / Tab. 7.12).
fn holder_uint_c_type(bits: u32) -> &'static str {
    match bits {
        0..=8 => "uint8_t",
        9..=16 => "uint16_t",
        17..=32 => "uint32_t",
        _ => "uint64_t",
    }
}

/// The C holder type for a bitmask: smallest uint that fits `@bit_bound`
/// (default 32). XTypes §7.3.1.2.2 — a bitmask serializes as this integer.
fn bitmask_holder_c_type(b: &BitmaskDecl) -> &'static str {
    // Holder width: an explicit `@bit_bound(N)` is authoritative; otherwise the
    // spec DEFAULT is @bit_bound=32 (Bug XV-bits, XTypes 1.3 §7.3.1.2.1.6) → a
    // UInt32 (4-byte) holder, NOT a width sized to the value count. Byte-identical
    // wire to the corrected rust golden (Perm => uint32_t).
    let bound = extract_int_annotation_c(&b.annotations, "bit_bound").unwrap_or(32);
    holder_uint_c_type(bound)
}

/// The C holder type for a bitset: smallest uint that fits the sum of all
/// bitfield widths. XTypes §7.3.1.2.1 — a bitset serializes as one packed
/// integer (over 64 bits is out of the C/XTypes profile and saturates to u64).
fn bitset_holder_c_type(b: &BitsetDecl) -> &'static str {
    let mut total: u32 = 0;
    for f in &b.bitfields {
        if let ConstExpr::Literal(l) = &f.spec.width {
            if l.kind == LiteralKind::Integer {
                if let Ok(w) = l.raw.trim().parse::<u32>() {
                    total = total.saturating_add(w);
                }
            }
        }
    }
    holder_uint_c_type(total)
}

/// Extract a single integer annotation parameter (`@bit_bound(32)` /
/// `@position(3)`) — used by the bitset/bitmask holder sizing.
fn extract_int_annotation_c(anns: &[Annotation], name: &str) -> Option<u32> {
    let a = anns
        .iter()
        .find(|a| a.name.parts.last().map(|p| p.text.as_str()) == Some(name))?;
    if let AnnotationParams::Single(ConstExpr::Literal(l)) = &a.params {
        if l.kind == LiteralKind::Integer {
            return l.raw.trim().parse::<u32>().ok();
        }
    }
    None
}

/// Best-effort literal/const fold during the collection pre-pass (the const
/// table is still being built, so this only resolves literals + already-seen
/// consts). Used to size a `typedef T A[N];` array-alias.
fn const_expr_to_u64_pre(consts: &SymbolTable, e: &ConstExpr) -> Option<u64> {
    if let ConstExpr::Literal(l) = e {
        if l.kind == LiteralKind::Integer {
            let raw = l.raw.trim();
            return if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
                u64::from_str_radix(hex, 16).ok()
            } else {
                raw.parse::<u64>().ok()
            };
        }
    }
    evaluate_positive_int(e, consts, e.span()).ok()
}

fn const_expr_to_u64(reg: &TypeReg, e: &ConstExpr) -> Option<u64> {
    // Literal fast path (also covers hex).
    if let ConstExpr::Literal(l) = e {
        if l.kind == LiteralKind::Integer {
            let raw = l.raw.trim();
            let parsed =
                if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
                    u64::from_str_radix(hex, 16).ok()
                } else {
                    raw.parse::<u64>().ok()
                };
            if let Some(v) = parsed {
                return Some(v);
            }
        }
    }
    // Named-constant / expression path: evaluate against the const symbol table
    // (handles `const long N = 4; long v[N];` and arithmetic like `N*2`).
    evaluate_positive_int(e, &reg.consts, e.span()).ok()
}

fn is_key(annotations: &[Annotation]) -> bool {
    annotations.iter().any(|a| {
        a.name
            .parts
            .last()
            .is_some_and(|p| p.text == "key" || p.text == "Key")
    })
}

fn struct_has_key(def: &StructDef) -> bool {
    def.members.iter().any(|m| is_key(&m.annotations))
}

fn id_annotation(annotations: &[Annotation]) -> Option<u32> {
    for a in annotations {
        let last = a.name.parts.last()?;
        if last.text != "id" && last.text != "Id" {
            continue;
        }
        if let AnnotationParams::Single(ConstExpr::Literal(lit)) = &a.params {
            if lit.kind == LiteralKind::Integer {
                if let Ok(v) = lit.raw.parse::<u32>() {
                    return Some(v);
                }
            }
        }
    }
    None
}
/// zerodds-lint: recursion-depth 64 (c_type_for bounded by AST depth)
fn c_type_for(reg: &TypeReg, type_spec: &TypeSpec) -> Result<String, CppGenError> {
    let s = match type_spec {
        TypeSpec::Scoped(sc) => {
            let last = scoped_last(sc).ok_or_else(|| unsupported("empty scoped name"))?;
            // enum → int32 alias.
            if let Some(cn) = reg.enums.get(&last) {
                return Ok(format!("{cn}_t"));
            }
            // bitmask / bitset → holder-integer typedef.
            if let Some((cn, _)) = reg.bitmasks.get(&last) {
                return Ok(format!("{cn}_t"));
            }
            if let Some((cn, _)) = reg.bitsets.get(&last) {
                return Ok(format!("{cn}_t"));
            }
            // typedef → resolve to underlying type. A typedef-to-array alias
            // (`typedef long Matrix3[3][3];`) carries the element type here; the
            // dims are applied separately at the member declarator via
            // `effective_array_dims` (#43, typedef-to-array).
            if reg.typedefs.contains_key(&last) {
                let resolved = resolve_alias(reg, type_spec);
                return c_type_for(reg, &resolved);
            }
            // nested struct → embed the C struct by value. A RECURSIVE struct
            // can only be referenced behind a pointer (sequence element); use
            // the struct tag `struct <C>_s*` so the type is valid INSIDE its own
            // typedef, before the `<C>_t` alias name exists (#43, recursion).
            if let Some((cn, _)) = reg.structs.get(&last) {
                if reg.is_recursive(&last) {
                    return Ok(format!("struct {cn}_s"));
                }
                return Ok(format!("{cn}_t"));
            }
            // union → embed the C tagged-union by value (tag form if recursive).
            if let Some((cn, _)) = reg.unions.get(&last) {
                if reg.is_recursive(&last) {
                    return Ok(format!("struct {cn}_s"));
                }
                return Ok(format!("{cn}_t"));
            }
            return Err(unsupported("unresolved scoped type (C backend)"));
        }
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean => "uint8_t",
            PrimitiveType::Octet => "uint8_t",
            PrimitiveType::Char => "char",
            PrimitiveType::WideChar => "uint16_t",
            PrimitiveType::Integer(IntegerType::Short)
            | PrimitiveType::Integer(IntegerType::Int16) => "int16_t",
            PrimitiveType::Integer(IntegerType::UShort)
            | PrimitiveType::Integer(IntegerType::UInt16) => "uint16_t",
            PrimitiveType::Integer(IntegerType::Long)
            | PrimitiveType::Integer(IntegerType::Int32) => "int32_t",
            PrimitiveType::Integer(IntegerType::ULong)
            | PrimitiveType::Integer(IntegerType::UInt32) => "uint32_t",
            PrimitiveType::Integer(IntegerType::LongLong)
            | PrimitiveType::Integer(IntegerType::Int64) => "int64_t",
            PrimitiveType::Integer(IntegerType::ULongLong)
            | PrimitiveType::Integer(IntegerType::UInt64) => "uint64_t",
            PrimitiveType::Integer(IntegerType::Int8) => "int8_t",
            PrimitiveType::Integer(IntegerType::UInt8) => "uint8_t",
            PrimitiveType::Floating(FloatingType::Float) => "float",
            PrimitiveType::Floating(FloatingType::Double) => "double",
            PrimitiveType::Floating(FloatingType::LongDouble) => {
                return Err(unsupported("long double"));
            }
        },
        TypeSpec::String(st) => {
            if st.wide {
                // wstring → NUL-terminated UTF-16 (uint16) buffer (#43, wstring).
                "uint16_t*"
            } else {
                "char*"
            }
        }
        TypeSpec::Sequence(seq) => {
            let elem = c_type_for(reg, &seq.elem)?;
            return Ok(format!("struct {{ uint32_t len; {elem}* elems; }}"));
        }
        TypeSpec::Map(m) => {
            // map<K,V> → parallel key/value arrays + count (#43, map<K,V>).
            let kc = c_type_for(reg, &m.key)?;
            let vc = c_type_for(reg, &m.value)?;
            return Ok(format!(
                "struct {{ uint32_t len; {kc}* keys; {vc}* vals; }}"
            ));
        }
        TypeSpec::Fixed(f) => {
            // fixed<P,S>: CORBA/GIOP §9.3.2.7 packed BCD, (P+2)/2 raw octets.
            // C has no decimal type, so the field IS the BCD byte array; the
            // user fills/reads `.bcd[]` (helpers can convert to/from a string).
            let p = const_expr_to_u64(reg, &f.digits)
                .ok_or_else(|| unsupported("fixed: non-constant digit count"))?;
            let n = (p + 2) / 2;
            return Ok(format!("struct {{ uint8_t bcd[{n}]; }}"));
        }
        _ => {
            return Err(unsupported("non-primitive type in field"));
        }
    };
    Ok(s.to_string())
}

fn primitive_helper(p: PrimitiveType) -> Result<&'static str, CppGenError> {
    Ok(match p {
        PrimitiveType::Boolean | PrimitiveType::Octet => "u8",
        PrimitiveType::Char => "u8",
        PrimitiveType::WideChar => "u16",
        PrimitiveType::Integer(IntegerType::Short) | PrimitiveType::Integer(IntegerType::Int16) => {
            "i16"
        }
        PrimitiveType::Integer(IntegerType::UShort)
        | PrimitiveType::Integer(IntegerType::UInt16) => "u16",
        PrimitiveType::Integer(IntegerType::Long) | PrimitiveType::Integer(IntegerType::Int32) => {
            "i32"
        }
        PrimitiveType::Integer(IntegerType::ULong)
        | PrimitiveType::Integer(IntegerType::UInt32) => "u32",
        PrimitiveType::Integer(IntegerType::LongLong)
        | PrimitiveType::Integer(IntegerType::Int64) => "i64",
        PrimitiveType::Integer(IntegerType::ULongLong)
        | PrimitiveType::Integer(IntegerType::UInt64) => "u64",
        PrimitiveType::Integer(IntegerType::Int8) => "i8",
        PrimitiveType::Integer(IntegerType::UInt8) => "u8",
        PrimitiveType::Floating(FloatingType::Float) => "f32",
        PrimitiveType::Floating(FloatingType::Double) => "f64",
        PrimitiveType::Floating(FloatingType::LongDouble) => {
            return Err(unsupported("long double"));
        }
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use zerodds_idl::config::ParserConfig;

    fn gen_c(src: &str) -> String {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse ok");
        generate_c_header(&ast, &CGenOptions::default()).expect("c-gen ok")
    }

    #[test]
    fn empty_final_struct_emits_typedef_and_typesupport() {
        let h = gen_c("@final struct Empty {};");
        assert!(h.contains("typedef struct Empty_s"));
        assert!(h.contains("Empty_typesupport"));
        assert!(h.contains(".extensibility = 0"));
    }

    #[test]
    fn primitive_struct_maps_types() {
        let h = gen_c("@final struct Point { long x; long y; };");
        assert!(h.contains("int32_t x;"));
        assert!(h.contains("int32_t y;"));
        assert!(h.contains("\"Point\""));
    }

    #[test]
    fn nested_module_yields_scoped_type_name() {
        let h = gen_c("module Outer { module Inner { @final struct S { long x; }; }; };");
        assert!(h.contains("typedef struct Outer_Inner_S_s"));
        assert!(h.contains("\"Outer::Inner::S\""));
    }

    #[test]
    fn appendable_default_when_no_annotation() {
        // SX2: un-annotated aggregates default to APPENDABLE (extensibility = 1,
        // XTypes 1.3 §7.3.3.1), self-delimited with a DHEADER, matching the
        // TypeObject path + FastDDS/Cyclone.
        let h = gen_c("struct V { long a; long b; };");
        assert!(h.contains(".extensibility = 1"));
    }

    #[test]
    fn mutable_struct_marks_extensibility() {
        let h = gen_c("@mutable struct M { @id(1) long a; };");
        assert!(h.contains(".extensibility = 2"));
    }

    #[test]
    fn key_member_sets_is_keyed_and_emits_key_hash() {
        let h = gen_c("@final struct Sensor { @key long id; double value; };");
        assert!(h.contains(".is_keyed = 1"));
        assert!(h.contains("Sensor_key_hash"));
    }

    // ---- Bug C: scope widening (no longer rejected) ----

    #[test]
    fn enum_member_no_longer_rejected() {
        // Previously: `non-struct type` / `nested struct reference` error.
        let h = gen_c("enum Color { RED, GREEN, BLUE }; @final struct S { Color c; };");
        assert!(h.contains("typedef int32_t Color_t;"));
        assert!(h.contains("Color_GREEN = 1"));
        // field typed as the int32 alias, encoded as i32.
        assert!(h.contains("Color_t c;"));
        assert!(h.contains("zerodds_xcdr2_c_write_i32(&w_buf, &w_len, &w_cap, (int32_t)(s->c))"));
        assert!(h.contains("zerodds_xcdr2_c_read_i32(buf, len, &pos, (int32_t*)&(s->c), zd_be)"));
    }

    #[test]
    fn typedef_member_resolves_to_underlying() {
        let h = gen_c("typedef double Amps; @final struct S { Amps battery; };");
        assert!(h.contains("double battery;"));
        // 8-byte primitives use the XCDR2 align-4 helper `zd_x2_write_f64`
        // (MAXALIGN=min(8,4)=4, §7.4.1.1.1), NOT the shared align-8 `write_f64`.
        assert!(h.contains("zd_x2_write_f64(&w_buf, &w_len, &w_cap, s->battery)"));
        assert!(h.contains("static inline int zd_x2_write_f64"));
    }

    #[test]
    fn nested_struct_member_inlined() {
        let h = gen_c(
            "@final struct Point { long x; long y; }; @final struct Line { Point a; Point b; };",
        );
        assert!(h.contains("Point_t a;"));
        assert!(h.contains("Point_t b;"));
        // inline-encoded by value (sub-member access through the field).
        assert!(h.contains("(s->a).x"));
        assert!(h.contains("(s->b).y"));
    }

    #[test]
    fn fixed_array_member_no_length_prefix() {
        let h = gen_c("@final struct G { long grid[3]; };");
        assert!(h.contains("int32_t grid[3];"));
        // a loop over the fixed extent, no u32 length write.
        assert!(h.contains("ai0 = 0; ai0 < 3"));
    }

    // ---- Bug C2: @optional + header conflict ----

    #[test]
    fn optional_member_gets_presence_flag() {
        let h = gen_c("@final struct O { @optional long maybe; };");
        assert!(h.contains("int32_t maybe;"));
        assert!(h.contains("uint8_t maybe_present;"));
        // wire: presence boolean, then the value only if present.
        assert!(h.contains("s->maybe_present ? 1 : 0"));
        assert!(h.contains("if (s->maybe_present) {"));
    }

    #[test]
    fn header_does_not_include_conflicting_zerodds_h() {
        // Bug C2: the cbindgen `zerodds.h` redeclares typed-FFI fns with names
        // conflicting with `zerodds_xcdr2.h`; only the latter is needed.
        let h = gen_c("@final struct S { long x; };");
        assert!(h.contains("#include \"zerodds_xcdr2.h\""));
        assert!(!h.contains("#include \"zerodds.h\""));
    }

    // ---- #43: C-Foundation widening — union / map / wstring / const-bound ----

    #[test]
    fn union_emits_tagged_union_codec() {
        // Unions are now in scope (#43): a tagged-union typedef + TypeSupport.
        let h = gen_c("union U switch (long) { case 1: long a; default: long b; };");
        assert!(h.contains("typedef struct U_s"));
        assert!(h.contains("} _u;"));
        assert!(h.contains("U_typesupport"));
        // Discriminator is switched on in encode/decode.
        assert!(h.contains("._d) {"));
        assert!(h.contains("case 1:"));
        assert!(h.contains("default:"));
    }

    #[test]
    fn const_array_bound_resolves_to_literal() {
        // `long v[N]` with `const long N = 4` resolves the bound (#43).
        let h = gen_c("const long N = 4; @final struct A { long v[N]; };");
        assert!(
            h.contains("int32_t v[4];"),
            "named const bound not folded:\n{h}"
        );
    }

    #[test]
    fn map_member_emits_parallel_arrays_and_codec() {
        let h = gen_c("@final struct M { map<string, long> counters; };");
        assert!(h.contains("keys;"));
        assert!(h.contains("vals;"));
        // DHEADER + count emitted for the map body.
        assert!(h.contains("map_dheader_pos"));
    }

    #[test]
    fn wstring_member_maps_to_uint16_ptr() {
        let h = gen_c("@final struct W { wstring label; };");
        assert!(h.contains("uint16_t* label;"));
        // wire: byte-length then uint16 code units.
        assert!(h.contains("ws_n * 2u"));
    }

    #[test]
    fn sequence_of_struct_resolves_element_type() {
        let h = gen_c("@final struct P { long x; }; @final struct S { sequence<P> ps; };");
        assert!(h.contains("P_t* elems; } ps;"));
        // element body inline-encodes the struct sub-member.
        assert!(h.contains(".elems[i0]).x") || h.contains(".elems[i0])"));
    }

    #[test]
    fn typedef_to_aggregate_resolves_to_struct() {
        let h = gen_c(
            "@final struct Point { long x; }; typedef Point Position; \
             @final struct S { Position p; };",
        );
        assert!(h.contains("Point_t p;"));
    }

    // ---- #43: the last four Foundation fixtures (typedefs / bits / recursion
    // / forward-decl) ----

    #[test]
    fn typedef_alias_chain_resolves_to_root() {
        let h = gen_c("typedef double A; typedef A B; @final struct S { B v; };");
        // The alias-of-alias resolves to the root primitive `double`.
        assert!(h.contains("double v;"), "alias chain not resolved:\n{h}");
    }

    #[test]
    fn typedef_to_array_contributes_dims_at_use_site() {
        let h = gen_c("typedef long Matrix3[3][3]; @final struct S { Matrix3 m; };");
        assert!(
            h.contains("int32_t m[3][3];"),
            "typedef-to-array dims lost:\n{h}"
        );
    }

    #[test]
    fn bitmask_maps_to_holder_uint_and_flag_constants() {
        let h = gen_c(
            "bitmask Permissions { READ, WRITE, EXECUTE }; @final struct S { Permissions p; };",
        );
        // No explicit @bit_bound → spec DEFAULT @bit_bound=32 (Bug XV-bits,
        // XTypes 1.3 §7.3.1.2.1.6) → a UInt32 holder, NOT a width sized to the
        // value count. flag = 1 << position.
        assert!(
            h.contains("typedef uint32_t Permissions_t;"),
            "holder uint:\n{h}"
        );
        assert!(
            h.contains("Permissions_WRITE = (1u << 1)"),
            "flag bit:\n{h}"
        );
        assert!(h.contains("Permissions_t p;"), "member type:\n{h}");
        // Serialized as the holder integer (u32), no DHEADER.
        assert!(h.contains("zerodds_xcdr2_c_write_u32(&w_buf, &w_len, &w_cap, s->p)"));
    }

    #[test]
    fn bitset_maps_to_packed_holder_and_accessors() {
        let h = gen_c(
            "bitset Flags { bitfield<3> kind; bitfield<1> active; bitfield<4> priority; }; \
             @final struct S { Flags f; };",
        );
        // 3+1+4 = 8 bits → uint8 holder; SHIFT/MASK accessor per named field.
        assert!(
            h.contains("typedef uint8_t Flags_t;"),
            "packed holder:\n{h}"
        );
        assert!(h.contains("Flags_active_SHIFT = 3"), "field offset:\n{h}");
        assert!(
            h.contains("#define Flags_priority_MASK 0xFu"),
            "field mask:\n{h}"
        );
        assert!(h.contains("zerodds_xcdr2_c_write_u8(&w_buf, &w_len, &w_cap, s->f)"));
    }

    #[test]
    fn recursive_type_through_sequence_splices_via_helper() {
        let h = gen_c("struct TreeNode { long value; sequence<TreeNode> children; };");
        // Self-reference is a pointer-to-tag, spliced through a runtime helper.
        assert!(
            h.contains("struct TreeNode_s* elems;"),
            "pointer-to-tag elem:\n{h}"
        );
        assert!(
            h.contains("static int TreeNode_write_body("),
            "write helper:\n{h}"
        );
        assert!(
            h.contains("static int TreeNode_read_body("),
            "read helper:\n{h}"
        );
        assert!(
            h.contains("TreeNode_write_body(&((s->children).elems[i0])"),
            "recursive splice call:\n{h}"
        );
    }

    #[test]
    fn forward_declared_then_defined_struct_and_union_generate() {
        // Forward decls then defs; the union embeds the (recursive) struct by
        // value, so the struct typedef must precede the union typedef.
        let h = gen_c(
            "module conf { \
                struct Node; union Variant; \
                struct Node { long value; sequence<Node> next; }; \
                union Variant switch (long) { case 0: long a; case 1: Node n; }; \
             };",
        );
        assert!(
            h.contains("typedef struct conf_Node_s"),
            "node typedef:\n{h}"
        );
        assert!(
            h.contains("typedef struct conf_Variant_s"),
            "variant typedef:\n{h}"
        );
        // Node typedef must come before Variant typedef (by-value embed order).
        let ni = h
            .find("typedef struct conf_Node_s")
            .expect("Node typedef present");
        let vi = h
            .find("typedef struct conf_Variant_s")
            .expect("Variant typedef present");
        assert!(ni < vi, "Node typedef must precede Variant typedef");
    }

    #[test]
    fn direct_by_value_self_membership_is_rejected() {
        // `struct Node { Node n; };` is an infinite-size type — rejected cleanly.
        let ast = zerodds_idl::parse("struct Node { long v; Node n; };", &ParserConfig::default());
        if let Ok(ast) = ast {
            assert!(
                generate_c_header(&ast, &CGenOptions::default()).is_err(),
                "infinite by-value self-membership must be rejected"
            );
        }
    }

    #[test]
    fn fixed_decimal_is_a_bcd_byte_field() {
        // fixed<P,S> now maps to a (P+2)/2-octet BCD byte field (CORBA §9.3.2.7)
        // with a raw encode/decode loop. `any` stays out of the C profile.
        let ast = zerodds_idl::parse(
            "@final struct S { fixed<5,2> price; };",
            &ParserConfig::default(),
        )
        .expect("parse");
        let c = generate_c_header(&ast, &CGenOptions::default()).expect("gen");
        assert!(
            c.contains("uint8_t bcd[3]"),
            "fixed<5,2> -> 3-octet BCD field:\n{c}"
        );
        assert!(
            c.contains("zerodds_xcdr2_c_write_u8") && c.contains("zerodds_xcdr2_c_read_u8"),
            "fixed must emit a raw BCD write/read loop"
        );
    }

    #[test]
    fn any_remains_out_of_scope() {
        // `any` (CORBA TypeCode + dynamic value) is NOT in the C profile.
        let ast = zerodds_idl::parse("@final struct S { any value; };", &ParserConfig::default())
            .expect("parse");
        assert!(generate_c_header(&ast, &CGenOptions::default()).is_err());
    }
}
