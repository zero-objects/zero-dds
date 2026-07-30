// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AST walker that emits C++17 headers.
//!
//! Block-A: Header layout (`#pragma once`, includes, namespaces).
//! Block-B: Primitive mapping (delegated to [`crate::type_map`]).
//! Block-C: struct/enum/union/typedef/sequence/array/inheritance.
//! Block-D: Exception → `class X : public std::exception`.
//! Block-E: Time/Duration via DDS::Time_t / DDS::Duration_t.
//!
//! Emission is single-pass: all required standard includes are collected
//! via a pre-walk, then the body is emitted. This keeps the header
//! preamble deterministic (alphabetically sorted).

use core::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use zerodds_idl::ast::{
    Annotation, AnnotationParams, Case, CaseLabel, ConstExpr, ConstrTypeDecl, Declarator,
    Definition, EnumDef, ExceptDecl, Export, InterfaceDcl, InterfaceDef, Literal, LiteralKind,
    Member, ModuleDef, OpDecl, ParamAttribute, PrimitiveType, ScopedName, Specification,
    StateVisibility, StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, TypedefDecl,
    UnionDcl, UnionDef, ValueDef, ValueElement,
};

use zerodds_idl::semantics::ExtensibilityKind as ExtKind;
use zerodds_idl::semantics::annotations::PlacementKind;

use crate::bitset::{emit_bitmask, emit_bitset};
use crate::error::CppGenError;
use crate::type_map::{escape_cpp_ident, is_reserved, primitive_to_cpp};
use crate::verbatim::emit_verbatim_at;
use crate::{CppGenOptions, TIME_DURATION_TYPES};

/// Known header includes. Order is stably alphabetical.
#[derive(Debug, Default, Clone)]
struct Includes {
    headers: BTreeSet<&'static str>,
}

impl Includes {
    fn add(&mut self, h: &'static str) {
        self.headers.insert(h);
    }
}

/// Main entry point: generates the complete C++ header.
/// `true` if `ts` is (or, through sequence nesting, contains) a bounded
/// collection that the encoder enforces: a bounded `sequence<T, N>` or a bounded
/// narrow `string<N>`. Drives the conditional `<stdexcept>` include.
///
/// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
fn type_has_bounded_collection(ts: &TypeSpec) -> bool {
    match ts {
        TypeSpec::Sequence(s) => s.bound.is_some() || type_has_bounded_collection(&s.elem),
        // narrow `string<N>` AND wide `wstring<N>` both throw on over-bound.
        TypeSpec::String(s) => s.bound.is_some(),
        _ => false,
    }
}

/// `true` if any topic struct in `spec` has a member with a bounded collection.
fn spec_has_bounded_collection(spec: &Specification) -> bool {
    let mut structs: Vec<(String, &StructDef)> = Vec::new();
    collect_topic_structs(&spec.definitions, "", &mut structs);
    structs.iter().any(|(_, s)| {
        s.members
            .iter()
            .any(|m| type_has_bounded_collection(&m.type_spec))
    })
}

pub(crate) fn emit_header(
    spec: &Specification,
    opts: &CppGenOptions,
) -> Result<String, CppGenError> {
    // 1. Detect colliding inheritance cycles (before emission).
    detect_inheritance_cycles(spec)?;

    // 1b. Codegen-scoped type registry (enum/struct simple-names) so the XCDR2
    //     member encoder can classify a `Scoped` member as an enum.
    set_type_registry(spec);

    // 2. Walk pre-pass: collect includes.
    let mut includes = Includes::default();
    includes.add("<cstdint>"); // stdint.h is always present (uint8_t etc.).
    collect_includes(spec, &mut includes);
    // Bounded collections throw std::length_error on over-bound encode
    // (DDS-XTypes §7.4.3) — pull <stdexcept> only when needed, so headers
    // without bounded types stay byte-identical.
    if spec_has_bounded_collection(spec) {
        includes.add("<stdexcept>");
    }

    // 3. Output buffer.
    let mut out = String::new();
    write_header_preamble(&mut out, opts, &includes)?;

    // 4. Optional top-level namespace wrapping (`namespace_prefix`).
    let mut ctx = EmitCtx::new(opts);
    let outer_prefix: Option<&str> = opts.namespace_prefix.as_deref().filter(|p| !p.is_empty());
    if let Some(prefix) = outer_prefix {
        ctx.open_namespace(&mut out, prefix)?;
    }

    // 4b. §7.2.2.4.8 — `@verbatim(placement=BEGIN_FILE)` from all
    // top-level defs. The spec says nothing about ordering for multiple —
    // we use source order.
    for d in &spec.definitions {
        if let Some(anns) = top_level_annotations(d) {
            emit_verbatim_at(&mut out, "", anns, PlacementKind::BeginFile)?;
        }
    }

    // 4c. If the spec contains at least one top-level or module-nested
    //     `struct` definition, we also emit `dds/topic/TopicTraits.hpp`
    //     after the standard includes — that header provides
    //     `topic_type_support<T>` and ByteSeq/string defaults — as well as
    //     `dds/topic/xcdr2.hpp` and `dds/topic/xcdr2_md5.hpp` for the
    //     XCDR2 wire-encoder/MD5 helpers that the later-emitted
    //     specializations depend on.
    let mut probe_structs: Vec<(String, &StructDef)> = Vec::new();
    collect_topic_structs(&spec.definitions, "", &mut probe_structs);
    let mut have_unions = Vec::new();
    collect_topic_unions(&spec.definitions, "", &mut have_unions);
    if !probe_structs.is_empty() || !have_unions.is_empty() {
        // <array> is needed by key_hash(), <vector>/<string> by
        // encode/decode(). If the standard walks have not already pulled
        // them in, they are covered transitively via TopicTraits.hpp —
        // but we emit them explicitly here so the header remains
        // syntactically valid without the topic helpers.
        writeln!(&mut out, "#include \"dds/topic/TopicTraits.hpp\"").map_err(fmt_err)?;
        writeln!(&mut out, "#include \"dds/topic/xcdr2.hpp\"").map_err(fmt_err)?;
        writeln!(&mut out, "#include \"dds/topic/xcdr2_md5.hpp\"").map_err(fmt_err)?;
        // `dds::core::Fixed<P,S>` — runtime BCD type for `fixed` members
        // (header-only; harmless when no fixed member is present).
        writeln!(&mut out, "#include \"dds/core/Fixed.hpp\"").map_err(fmt_err)?;
        writeln!(&mut out).map_err(fmt_err)?;
    }

    // 5. Emit definitions.
    for d in &spec.definitions {
        emit_definition(&mut out, &mut ctx, d)?;
    }

    // 5b. §7.2.2.4.8 — `@verbatim(placement=END_FILE)` from all
    // top-level defs.
    for d in &spec.definitions {
        if let Some(anns) = top_level_annotations(d) {
            emit_verbatim_at(&mut out, "", anns, PlacementKind::EndFile)?;
        }
    }

    // 6. Optionally close the top-level namespace.
    if let Some(prefix) = outer_prefix {
        ctx.close_namespace(&mut out, prefix)?;
    }

    // 7. `topic_type_support<T>` specializations for all struct defs.
    //    Lives in the `::dds::topic` namespace (global scope), so emitted
    //    after the `outer_prefix` close with fully-qualified T.
    let mut probe_unions: Vec<(String, &UnionDef)> = Vec::new();
    collect_topic_unions(&spec.definitions, "", &mut probe_unions);
    if !probe_structs.is_empty() || !probe_unions.is_empty() {
        // #24 / F-TYPES-3: full-spec resolved NameMap (the SAME "Path A"
        // index `build_type_registry` builds) so each emitted
        // `type_object()` resolves typedef/enum/sequence/map/nested-struct/
        // union/array member types exactly as `idl-rust` does — a build
        // failure (cyclic type / not-yet-mappable `fixed`/`any`) degrades to
        // an empty map, i.e. affected structs emit an empty `type_object()`
        // (typed-create then falls back to the byte-oriented create), never a
        // codegen error.
        let type_names = zerodds_idl::semantics::build_type_registry(spec)
            .map(|lowered| lowered.names)
            .unwrap_or_default();
        emit_topic_type_support_specs(&mut out, opts, &probe_structs, &probe_unions, &type_names)?;
    }

    Ok(out)
}

/// Returns the annotation list of a top-level `Definition`, if the
/// variant carries one. Used for file-level verbatim emission.
fn top_level_annotations(d: &Definition) -> Option<&[Annotation]> {
    match d {
        Definition::Module(m) => Some(&m.annotations),
        Definition::Type(TypeDecl::Constr(c)) => match c {
            ConstrTypeDecl::Struct(StructDcl::Def(s)) => Some(&s.annotations),
            ConstrTypeDecl::Union(UnionDcl::Def(u)) => Some(&u.annotations),
            ConstrTypeDecl::Enum(e) => Some(&e.annotations),
            _ => None,
        },
        Definition::Type(TypeDecl::Typedef(t)) => Some(&t.annotations),
        Definition::Const(c) => Some(&c.annotations),
        Definition::Except(e) => Some(&e.annotations),
        _ => None,
    }
}

/// Writes `// Generated...`, `#pragma once`, and standard includes.
fn write_header_preamble(
    out: &mut String,
    opts: &CppGenOptions,
    includes: &Includes,
) -> Result<(), CppGenError> {
    writeln!(out, "// Generated by zerodds idl-cpp. Do not edit.").map_err(fmt_err)?;
    writeln!(out, "#pragma once").map_err(fmt_err)?;
    if let Some(guard) = opts.include_guard_prefix.as_deref() {
        if !guard.is_empty() {
            // Optional guard comment for tools that only accept classical
            // include guards. `#pragma once` remains primary.
            writeln!(out, "// guard-prefix: {guard}").map_err(fmt_err)?;
        }
    }
    writeln!(out).map_err(fmt_err)?;
    for h in &includes.headers {
        writeln!(out, "#include {h}").map_err(fmt_err)?;
    }
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// Walks the AST and collects required `<...>` includes.
fn collect_includes(spec: &Specification, inc: &mut Includes) {
    for d in &spec.definitions {
        collect_in_def(d, inc);
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_in_def(d: &Definition, inc: &mut Includes) {
    match d {
        Definition::Module(m) => {
            for sub in &m.definitions {
                collect_in_def(sub, inc);
            }
        }
        Definition::Type(td) => collect_in_typedecl(td, inc),
        Definition::Const(c) => {
            // A string const emits `constexpr std::string_view`/`std::wstring_view`.
            if matches!(&c.type_, zerodds_idl::ast::ConstType::String { .. }) {
                inc.add("<string_view>");
            }
        }
        Definition::Except(e) => {
            inc.add("<exception>");
            for m in &e.members {
                collect_in_typespec(&m.type_spec, inc);
                for decl in &m.declarators {
                    if matches!(decl, Declarator::Array(_)) {
                        inc.add("<array>");
                    }
                }
            }
        }
        Definition::Interface(_)
        | Definition::ValueBox(_)
        | Definition::ValueForward(_)
        | Definition::ValueDef(_)
        | Definition::TypeId(_)
        | Definition::TypePrefix(_)
        | Definition::Import(_)
        | Definition::Component(_)
        | Definition::Home(_)
        | Definition::Event(_)
        | Definition::Porttype(_)
        | Definition::Connector(_)
        | Definition::TemplateModule(_)
        | Definition::TemplateModuleInst(_)
        | Definition::Annotation(_)
        | Definition::VendorExtension(_) => {
            // Recognised as UnsupportedConstruct in emit_definition;
            // no includes to collect here.
        }
    }
}

fn collect_in_typedecl(td: &TypeDecl, inc: &mut Includes) {
    match td {
        TypeDecl::Constr(c) => match c {
            ConstrTypeDecl::Struct(StructDcl::Def(s)) => {
                // Field-order constructor uses std::move (<utility>) when the
                // struct has at least one member.
                if !s.members.is_empty() {
                    inc.add("<utility>");
                }
                for m in &s.members {
                    collect_in_typespec(&m.type_spec, inc);
                    for decl in &m.declarators {
                        if matches!(decl, Declarator::Array(_)) {
                            inc.add("<array>");
                        }
                    }
                    if has_optional_annotation(&m.annotations) {
                        inc.add("<optional>");
                    }
                    if has_shared_annotation(&m.annotations) {
                        inc.add("<memory>");
                    }
                }
            }
            ConstrTypeDecl::Struct(StructDcl::Forward(_)) => {}
            ConstrTypeDecl::Union(UnionDcl::Def(u)) => {
                inc.add("<variant>");
                for c in &u.cases {
                    collect_in_typespec(&c.element.type_spec, inc);
                    if matches!(c.element.declarator, Declarator::Array(_)) {
                        inc.add("<array>");
                    }
                }
            }
            ConstrTypeDecl::Union(UnionDcl::Forward(_)) => {}
            ConstrTypeDecl::Enum(_) | ConstrTypeDecl::Bitset(_) | ConstrTypeDecl::Bitmask(_) => {}
        },
        TypeDecl::Typedef(t) => {
            collect_in_typespec(&t.type_spec, inc);
            for decl in &t.declarators {
                if matches!(decl, Declarator::Array(_)) {
                    inc.add("<array>");
                }
            }
        }
        // `native X;` — opaque type, no includes.
        TypeDecl::Native(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_in_typespec(ts: &TypeSpec, inc: &mut Includes) {
    match ts {
        TypeSpec::Primitive(_) => {}
        TypeSpec::Scoped(_) => {}
        TypeSpec::Sequence(s) => {
            inc.add("<vector>");
            collect_in_typespec(&s.elem, inc);
        }
        TypeSpec::String(_) => {
            // Both `string` and `wstring` are defined in `<string>`.
            inc.add("<string>");
        }
        TypeSpec::Map(m) => {
            inc.add("<map>");
            // A user-defined (scoped) map key may be a struct, for which we
            // generate a `std::tie`-based `operator<` — that needs `<tuple>`.
            if matches!(m.key.as_ref(), TypeSpec::Scoped(_)) {
                inc.add("<tuple>");
            }
            collect_in_typespec(&m.key, inc);
            collect_in_typespec(&m.value, inc);
        }
        TypeSpec::Fixed(_) => {
            // §7.2.4.2.4 — `fixed<digits, scale>` -> `dds::core::Fixed`.
            // We emit a forward-declared wrapper class
            // (runtime implementation comes with the `dds-core` crate).
            inc.add("<cstdint>");
        }
        TypeSpec::Any => {
            // §7.3 — `any` -> `dds::core::Any`. Runtime in dds-core.
            inc.add("<cstdint>");
        }
    }
}

/// Emit context (indentation, output).
struct EmitCtx<'o> {
    opts: &'o CppGenOptions,
    indent_level: usize,
}

impl<'o> EmitCtx<'o> {
    fn new(opts: &'o CppGenOptions) -> Self {
        Self {
            opts,
            indent_level: 0,
        }
    }

    fn indent(&self) -> String {
        " ".repeat(self.indent_level * self.opts.indent_width)
    }

    fn open_namespace(&mut self, out: &mut String, name: &str) -> Result<(), CppGenError> {
        writeln!(out, "{}namespace {name} {{", self.indent()).map_err(fmt_err)?;
        self.indent_level += 1;
        Ok(())
    }

    fn close_namespace(&mut self, out: &mut String, name: &str) -> Result<(), CppGenError> {
        self.indent_level = self.indent_level.saturating_sub(1);
        writeln!(out, "{}}} // namespace {name}", self.indent()).map_err(fmt_err)?;
        Ok(())
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn emit_definition(
    out: &mut String,
    ctx: &mut EmitCtx<'_>,
    def: &Definition,
) -> Result<(), CppGenError> {
    match def {
        Definition::Module(m) => emit_module(out, ctx, m),
        Definition::Type(td) => emit_type_decl(out, ctx, td),
        Definition::Const(c) => emit_const_decl(out, ctx, c),
        Definition::Except(e) => emit_exception(out, ctx, e),
        Definition::Interface(InterfaceDcl::Def(iface)) => {
            // Spec idl4-cpp §7.4: IDL interface -> C++ pure-virtual class.
            // `@service` interfaces go on via the RPC codegen path
            // (see `crate::rpc`); here is the legacy CORBA stub path
            // for non-service interfaces.
            emit_interface_stub(out, ctx, iface)
        }
        Definition::Interface(InterfaceDcl::Forward(f)) => {
            let name = escape_cpp_ident(&f.name.text);
            writeln!(out, "{}class {name};", ctx.indent()).map_err(fmt_err)?;
            Ok(())
        }
        Definition::ValueDef(v) => emit_value_type(out, ctx, v),
        Definition::ValueBox(_) | Definition::ValueForward(_) => {
            // ValueBox + ValueForward are rare CORBA constructs;
            // the foundation leaves them as a no-op (spec §7.6.x allows
            // a forward decl with later resolution).
            Ok(())
        }
        Definition::TypeId(_)
        | Definition::TypePrefix(_)
        | Definition::Import(_)
        | Definition::Component(_)
        | Definition::Home(_)
        | Definition::Event(_)
        | Definition::Porttype(_)
        | Definition::Connector(_)
        | Definition::TemplateModule(_)
        | Definition::TemplateModuleInst(_) => Err(CppGenError::UnsupportedConstruct {
            construct: "corba/ccm/template construct".into(),
            context: None,
        }),
        Definition::Annotation(_) => {
            // §7.4.15 annotation defs do not become C++ code directly —
            // applications are emitted at the annotated members via
            // annotation bridges (e.g. @key).
            Ok(())
        }
        Definition::VendorExtension(v) => Err(CppGenError::UnsupportedConstruct {
            construct: format!("vendor-extension:{}", v.production_name),
            context: None,
        }),
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn emit_module(out: &mut String, ctx: &mut EmitCtx<'_>, m: &ModuleDef) -> Result<(), CppGenError> {
    let name = escape_cpp_ident(&m.name.text);
    ctx.open_namespace(out, &name)?;
    // Track the lexical scope (raw IDL module name) so member field-type
    // references inside this module resolve to the correct declaration (P0-2).
    let _sg = ScopeGuard(cur_scope());
    let mut inner = cur_scope();
    inner.push(m.name.text.clone());
    set_scope(&inner);
    for d in &m.definitions {
        emit_definition(out, ctx, d)?;
    }
    ctx.close_namespace(out, &name)?;
    Ok(())
}

fn emit_type_decl(
    out: &mut String,
    ctx: &mut EmitCtx<'_>,
    td: &TypeDecl,
) -> Result<(), CppGenError> {
    match td {
        TypeDecl::Constr(c) => match c {
            ConstrTypeDecl::Struct(StructDcl::Def(s)) => emit_struct(out, ctx, s),
            ConstrTypeDecl::Struct(StructDcl::Forward(f)) => {
                let name = escape_cpp_ident(&f.name.text);
                writeln!(out, "{}class {name};", ctx.indent()).map_err(fmt_err)?;
                Ok(())
            }
            ConstrTypeDecl::Union(UnionDcl::Def(u)) => emit_union(out, ctx, u),
            ConstrTypeDecl::Union(UnionDcl::Forward(f)) => {
                let name = escape_cpp_ident(&f.name.text);
                writeln!(out, "{}class {name};", ctx.indent()).map_err(fmt_err)?;
                Ok(())
            }
            ConstrTypeDecl::Enum(e) => emit_enum(out, ctx, e),
            ConstrTypeDecl::Bitset(b) => {
                // Name escaping happens inside `emit_bitset` (reserved-word →
                // trailing `_`), consistent with the abs_path registry entry.
                let ind = ctx.indent();
                let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);
                emit_bitset(out, &ind, &inner, b)
            }
            ConstrTypeDecl::Bitmask(b) => {
                let ind = ctx.indent();
                let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);
                emit_bitmask(out, &ind, &inner, b)
            }
        },
        TypeDecl::Typedef(t) => emit_typedef(out, ctx, t),
        // `native X;` — opaque, platform-specific type without a wire
        // representation; not emitted in the DataType codegen.
        TypeDecl::Native(_) => Ok(()),
    }
}

fn emit_struct(out: &mut String, ctx: &mut EmitCtx<'_>, s: &StructDef) -> Result<(), CppGenError> {
    let sname = escape_cpp_ident(&s.name.text);
    let ind = ctx.indent();

    // §7.2.2.4.8 — `@verbatim(placement=BEFORE_DECLARATION)` before the
    // class header line.
    emit_verbatim_at(out, &ind, &s.annotations, PlacementKind::BeforeDeclaration)?;

    // Class header with an optional inheritance clause.
    if let Some(base) = &s.base {
        let base_str = scoped_to_cpp(base);
        writeln!(out, "{ind}class {sname} : public {base_str} {{").map_err(fmt_err)?;
    } else {
        writeln!(out, "{ind}class {sname} {{").map_err(fmt_err)?;
    }
    writeln!(out, "{ind}public:").map_err(fmt_err)?;

    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_DECLARATION)` as the first
    // line inside the `public:` block.
    emit_verbatim_at(out, &inner, &s.annotations, PlacementKind::BeginDeclaration)?;

    // Default constructor.
    writeln!(out, "{inner}{sname}() = default;").map_err(fmt_err)?;
    writeln!(out, "{inner}~{sname}() = default;").map_err(fmt_err)?;

    // Field-order constructor — one parameter per member in declaration
    // order, member-initialised (parameters passed by value + std::move so
    // strings/sequences/nested aggregates bind cheaply; scalar move == copy).
    // Enables brace/aggregate-style init `T t{a, b};`. Skipped for the
    // zero-field case so it does not collide with the defaulted ctor.
    emit_field_order_ctor(out, ctx, s, &inner)?;
    writeln!(out).map_err(fmt_err)?;

    // Member fields as private storage.
    writeln!(out, "{ind}private:").map_err(fmt_err)?;
    for m in &s.members {
        emit_struct_member_field(out, ctx, m)?;
    }
    writeln!(out).map_err(fmt_err)?;

    // Reference-Pattern Getter (mutable + const).
    writeln!(out, "{ind}public:").map_err(fmt_err)?;
    for m in &s.members {
        emit_struct_member_accessors(out, ctx, m)?;
    }

    // §7.2.2.4.8 — `@verbatim(placement=END_DECLARATION)` as the last
    // line before `};`.
    emit_verbatim_at(out, &inner, &s.annotations, PlacementKind::EndDeclaration)?;

    writeln!(out, "{ind}}};").map_err(fmt_err)?;

    // §7.2.2.4.8 — `@verbatim(placement=AFTER_DECLARATION)` direkt
    // after the closing `};`.
    emit_verbatim_at(out, &ind, &s.annotations, PlacementKind::AfterDeclaration)?;

    // §7.4.2.9: a struct used as a `map` key needs `operator<` so
    // `std::map<Struct, _>` is well-formed. Emit a lexicographic ordering over
    // the members (declaration order) via `std::tie` of the const accessors.
    let is_map_key = MAP_KEY_STRUCTS.with(|set| set.borrow().contains(&s.name.text));
    if is_map_key {
        emit_map_key_ordering(out, &ind, s, &sname)?;
    }

    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// Emits a free lexicographic `operator<` for a struct used as a `std::map`
/// key. Compares the members in declaration order through the const accessors
/// (`std::tie(l.a(), l.b()) < std::tie(r.a(), r.b())`), which is a strict weak
/// ordering as long as every member type is itself `<`-comparable (primitives,
/// strings, vectors, arrays, optionals, and nested map-key structs all are).
fn emit_map_key_ordering(
    out: &mut String,
    ind: &str,
    s: &StructDef,
    sname: &str,
) -> Result<(), CppGenError> {
    let mut getters: Vec<String> = Vec::new();
    for m in &s.members {
        for decl in &m.declarators {
            getters.push(escape_cpp_ident(&decl.name().text));
        }
    }
    let lhs = getters
        .iter()
        .map(|g| format!("zd_l.{g}()"))
        .collect::<Vec<_>>()
        .join(", ");
    let rhs = getters
        .iter()
        .map(|g| format!("zd_r.{g}()"))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        out,
        "{ind}inline bool operator<(const {sname}& zd_l, const {sname}& zd_r) {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{ind}    return std::tie({lhs}) < std::tie({rhs});").map_err(fmt_err)?;
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    Ok(())
}

/// Private storage-field identifier for a member with the RAW IDL name
/// `raw_name`.
///
/// The convention is `<escaped_accessor>_`. When the member name is a reserved
/// C++ keyword its escaped accessor is `kw_`, so the naive `kw_` + `_` would be
/// `kw__` — and any identifier containing `__` is reserved to the implementation
/// (ISO/IEC 14882:2017 [lex.name]/3, guarded by the
/// `no_implementation_reserved_identifiers` test). For that keyword case the
/// storage suffix is `field` instead of a second `_` (`operator` ->
/// `operator_field`), keeping the field legal, `__`-free and distinct from the
/// `operator_()` accessor. Non-keyword names keep the exact pre-existing
/// `<name>_` form (so no non-reserved header changes), which is the sole reason
/// this decides on `is_reserved(raw_name)` rather than on a trailing `_`.
pub(crate) fn member_storage_ident(raw_name: &str) -> String {
    let escaped = escape_cpp_ident(raw_name);
    if is_reserved(raw_name) {
        format!("{escaped}field")
    } else {
        format!("{escaped}_")
    }
}

/// Storage type for a member declarator, applying `@shared` (-> shared_ptr)
/// and `@optional` (-> std::optional) wrapping exactly as the field/accessor
/// emitters do. Returned type matches the private `_`-suffixed member field.
fn member_storage_type(m: &Member, decl: &Declarator) -> Result<String, CppGenError> {
    let cpp_ty = type_for_declarator(&m.type_spec, decl)?;
    let core_ty = if has_shared_annotation(&m.annotations) {
        format!("std::shared_ptr<{cpp_ty}>")
    } else {
        cpp_ty
    };
    if has_optional_annotation(&m.annotations) {
        Ok(format!("std::optional<{core_ty}>"))
    } else {
        Ok(core_ty)
    }
}

/// Emit the field-order constructor: one parameter per member (in declaration
/// order, mirroring the member list exactly, including multi-declarator
/// members like `long a, b;`). Parameters are taken by value and moved into
/// the corresponding `_`-suffixed field. No constructor is emitted for a
/// zero-field struct (it would be ambiguous with the defaulted default ctor).
fn emit_field_order_ctor(
    out: &mut String,
    _ctx: &EmitCtx<'_>,
    s: &StructDef,
    inner: &str,
) -> Result<(), CppGenError> {
    // Flatten members -> (storage_type, raw_member_name) in declaration order.
    let mut fields: Vec<(String, String)> = Vec::new();
    for m in &s.members {
        for decl in &m.declarators {
            fields.push((member_storage_type(m, decl)?, decl.name().text.clone()));
        }
    }
    if fields.is_empty() {
        return Ok(());
    }

    // Parameter names are the escaped accessor identifiers (`operator_`); the
    // member-initialiser targets the `__`-free storage field.
    let params = fields
        .iter()
        .map(|(ty, raw)| format!("{ty} {}", escape_cpp_ident(raw)))
        .collect::<Vec<_>>()
        .join(", ");
    let inits = fields
        .iter()
        .map(|(_, raw)| {
            format!(
                "{}(std::move({}))",
                member_storage_ident(raw),
                escape_cpp_ident(raw)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    let sname = escape_cpp_ident(&s.name.text);
    writeln!(out, "{inner}{sname}({params})\n{inner}    : {inits} {{}}").map_err(fmt_err)?;
    Ok(())
}

fn emit_struct_member_field(
    out: &mut String,
    ctx: &EmitCtx<'_>,
    m: &Member,
) -> Result<(), CppGenError> {
    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);
    let optional = has_optional_annotation(&m.annotations);
    let shared = has_shared_annotation(&m.annotations);
    for decl in &m.declarators {
        let cpp_ty = type_for_declarator(&m.type_spec, decl)?;
        let field = member_storage_ident(&decl.name().text);
        let key_marker = if has_key_annotation(&m.annotations) {
            " // @key"
        } else {
            ""
        };
        // §8.1.5 `@shared` -> `std::shared_ptr<T>`. Combination with
        // `@optional` yields `std::optional<std::shared_ptr<T>>`.
        let core_ty = if shared {
            format!("std::shared_ptr<{cpp_ty}>")
        } else {
            cpp_ty
        };
        if optional {
            writeln!(out, "{inner}std::optional<{core_ty}> {field};{key_marker}")
                .map_err(fmt_err)?;
        } else {
            writeln!(out, "{inner}{core_ty} {field};{key_marker}").map_err(fmt_err)?;
        }
    }
    Ok(())
}

fn emit_struct_member_accessors(
    out: &mut String,
    ctx: &EmitCtx<'_>,
    m: &Member,
) -> Result<(), CppGenError> {
    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);
    let optional = has_optional_annotation(&m.annotations);
    let shared = has_shared_annotation(&m.annotations);
    for decl in &m.declarators {
        let cpp_ty = type_for_declarator(&m.type_spec, decl)?;
        // Escape reserved words so the accessor method name is a legal C++
        // token AND matches the escaped `_`-suffixed storage field.
        let name = escape_cpp_ident(&decl.name().text);
        let core_ty = if shared {
            format!("std::shared_ptr<{cpp_ty}>")
        } else {
            cpp_ty.clone()
        };
        let storage_ty = if optional {
            format!("std::optional<{core_ty}>")
        } else {
            core_ty
        };
        let field = member_storage_ident(&decl.name().text);
        writeln!(out, "{inner}{storage_ty}& {name}() {{ return {field}; }}").map_err(fmt_err)?;
        writeln!(
            out,
            "{inner}const {storage_ty}& {name}() const {{ return {field}; }}"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "{inner}void {name}(const {storage_ty}& value) {{ {field} = value; }}"
        )
        .map_err(fmt_err)?;
        // broad-audit P0-7: `@shared` (XTypes 1.3 §7.3.1.2.1.9) holds the member
        // in memory via a `std::shared_ptr<T>`, but the WIRE carries the referenced
        // value fully by value (byte-identical to the same member WITHOUT @shared).
        // These value-typed setter overloads let the generated decode assign the
        // decoded pointee directly (it is wrapped into a fresh `shared_ptr`), so the
        // decode paths reuse the ordinary `zd_v.name(<value>)` calls unchanged
        // instead of silently dropping the member. For `@shared @optional` the same
        // overload engages the outer `std::optional` (assigning a `shared_ptr` to
        // an `optional<shared_ptr<T>>` makes it present); absence still flows through
        // the primary `optional`-typed setter via `std::nullopt`.
        if shared {
            writeln!(
                out,
                "{inner}void {name}(const {cpp_ty}& value) {{ {field} = std::make_shared<{cpp_ty}>(value); }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{inner}void {name}({cpp_ty}&& value) {{ {field} = std::make_shared<{cpp_ty}>(std::move(value)); }}"
            )
            .map_err(fmt_err)?;
        }
    }
    Ok(())
}

fn emit_union(out: &mut String, ctx: &mut EmitCtx<'_>, u: &UnionDef) -> Result<(), CppGenError> {
    let uname = escape_cpp_ident(&u.name.text);
    let ind = ctx.indent();
    emit_verbatim_at(out, &ind, &u.annotations, PlacementKind::BeforeDeclaration)?;
    writeln!(out, "{ind}class {uname} {{").map_err(fmt_err)?;
    writeln!(out, "{ind}public:").map_err(fmt_err)?;
    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);
    emit_verbatim_at(out, &inner, &u.annotations, PlacementKind::BeginDeclaration)?;

    let disc_ty = switch_type_to_cpp(&u.switch_type)?;

    // Build the variant list from distinct element types.
    let mut variant_types: Vec<String> = Vec::new();
    for c in &u.cases {
        let cpp_ty = type_for_declarator(&c.element.type_spec, &c.element.declarator)?;
        if !variant_types.iter().any(|t| t == &cpp_ty) {
            variant_types.push(cpp_ty);
        }
    }
    let variant_str = if variant_types.is_empty() {
        "std::monostate".to_string()
    } else {
        variant_types.join(", ")
    };

    writeln!(
        out,
        "{inner}using value_type = std::variant<{variant_str}>;"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{uname}() = default;").map_err(fmt_err)?;
    writeln!(out, "{inner}~{uname}() = default;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Discriminator.
    writeln!(
        out,
        "{inner}{disc_ty} _d() const {{ return discriminator_; }}"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}void _d({disc_ty} d) {{ discriminator_ = d; }}").map_err(fmt_err)?;
    writeln!(out, "{inner}value_type& value() {{ return value_; }}").map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}const value_type& value() const {{ return value_; }}"
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Branch markers as comments (discriminator values).
    let mut has_default = false;
    for c in &u.cases {
        emit_union_case_comment(out, &inner, c, &mut has_default)?;
    }
    if !has_default {
        writeln!(out, "{inner}// no explicit 'default:' branch").map_err(fmt_err)?;
    }

    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "{ind}private:").map_err(fmt_err)?;
    writeln!(out, "{inner}{disc_ty} discriminator_{{}};").map_err(fmt_err)?;
    writeln!(out, "{inner}value_type value_{{}};").map_err(fmt_err)?;
    emit_verbatim_at(out, &inner, &u.annotations, PlacementKind::EndDeclaration)?;
    writeln!(out, "{ind}}};").map_err(fmt_err)?;
    emit_verbatim_at(out, &ind, &u.annotations, PlacementKind::AfterDeclaration)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_union_case_comment(
    out: &mut String,
    inner: &str,
    c: &Case,
    has_default: &mut bool,
) -> Result<(), CppGenError> {
    for label in &c.labels {
        match label {
            CaseLabel::Default => {
                *has_default = true;
                writeln!(
                    out,
                    "{inner}// case default -> {}",
                    declarator_name(&c.element.declarator)
                )
                .map_err(fmt_err)?;
            }
            CaseLabel::Value(expr) => {
                let val = const_expr_to_cpp(expr);
                writeln!(
                    out,
                    "{inner}// case {val} -> {}",
                    declarator_name(&c.element.declarator)
                )
                .map_err(fmt_err)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn declarator_name(d: &Declarator) -> &str {
    &d.name().text
}

fn emit_enum(out: &mut String, ctx: &mut EmitCtx<'_>, e: &EnumDef) -> Result<(), CppGenError> {
    let ename = escape_cpp_ident(&e.name.text);
    let ind = ctx.indent();
    emit_verbatim_at(out, &ind, &e.annotations, PlacementKind::BeforeDeclaration)?;
    writeln!(out, "{ind}enum class {ename} : int32_t {{").map_err(fmt_err)?;
    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);
    emit_verbatim_at(out, &inner, &e.annotations, PlacementKind::BeginDeclaration)?;
    // XTypes 1.3 §7.4.5.1: an enumerator's wire value is `@value(n)` when
    // present, else one past the previous enumerator (from 0). Emitting bare
    // names let C++ re-derive plain ordinals (0,1,2,…), silently diverging from
    // the TypeObject/peer wire value for any `@value` gap. Emit every value
    // explicitly so the C++ constant matches the wire.
    let mut next: i64 = 0;
    for en in &e.enumerators {
        let en_name = escape_cpp_ident(&en.name.text);
        let val = enumerator_value_annotation(&en.annotations).unwrap_or(next);
        writeln!(out, "{inner}{en_name} = {val},").map_err(fmt_err)?;
        next = val.wrapping_add(1);
    }
    emit_verbatim_at(out, &inner, &e.annotations, PlacementKind::EndDeclaration)?;
    writeln!(out, "{ind}}};").map_err(fmt_err)?;
    emit_verbatim_at(out, &ind, &e.annotations, PlacementKind::AfterDeclaration)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_interface_stub(
    out: &mut String,
    ctx: &mut EmitCtx<'_>,
    iface: &InterfaceDef,
) -> Result<(), CppGenError> {
    let name = escape_cpp_ident(&iface.name.text);
    let ind = ctx.indent();
    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);

    emit_verbatim_at(
        out,
        &ind,
        &iface.annotations,
        PlacementKind::BeforeDeclaration,
    )?;

    // Bases via public virtual inheritance (CORBA pattern for diamonds).
    if iface.bases.is_empty() {
        writeln!(out, "{ind}class {name} {{").map_err(fmt_err)?;
    } else {
        let bases: Vec<String> = iface
            .bases
            .iter()
            .map(|b| format!("public virtual {}", scoped_to_cpp(b)))
            .collect();
        writeln!(out, "{ind}class {name} : {} {{", bases.join(", ")).map_err(fmt_err)?;
    }
    writeln!(out, "{ind}public:").map_err(fmt_err)?;
    writeln!(out, "{inner}virtual ~{name}() = default;").map_err(fmt_err)?;

    for export in &iface.exports {
        match export {
            Export::Op(op) => emit_interface_op(out, &inner, op)?,
            Export::Attr(attr) => emit_interface_attr(out, &inner, attr)?,
            Export::Type(td) => emit_type_decl(out, ctx, td)?,
            Export::Const(c) => emit_const_decl(out, ctx, c)?,
            Export::Except(e) => emit_exception(out, ctx, e)?,
        }
    }

    writeln!(out, "{ind}}};").map_err(fmt_err)?;
    emit_verbatim_at(
        out,
        &ind,
        &iface.annotations,
        PlacementKind::AfterDeclaration,
    )?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_interface_op(out: &mut String, inner: &str, op: &OpDecl) -> Result<(), CppGenError> {
    let op_name = escape_cpp_ident(&op.name.text);
    let ret = match &op.return_type {
        None => "void".to_string(),
        Some(t) => typespec_to_cpp(t)?,
    };
    let params: Vec<String> = op
        .params
        .iter()
        .map(|p| -> Result<String, CppGenError> {
            let ty = typespec_to_cpp(&p.type_spec)?;
            // Spec §7.4.5: in -> const T& (or T for primitives),
            // out/inout -> T&. For the foundation: const T&/T& consistent.
            let qual = match p.attribute {
                ParamAttribute::In => format!("const {ty}&"),
                ParamAttribute::Out | ParamAttribute::InOut => format!("{ty}&"),
            };
            Ok(format!("{qual} {}", escape_cpp_ident(&p.name.text)))
        })
        .collect::<Result<_, _>>()?;
    let raises_comment = if op.raises.is_empty() {
        String::new()
    } else {
        let raises: Vec<String> = op.raises.iter().map(scoped_to_cpp).collect();
        format!(" /* throws {} */", raises.join(", "))
    };
    writeln!(
        out,
        "{inner}virtual {ret} {op_name}({}) = 0;{raises_comment}",
        params.join(", ")
    )
    .map_err(fmt_err)?;
    Ok(())
}

fn emit_interface_attr(
    out: &mut String,
    inner: &str,
    attr: &zerodds_idl::ast::AttrDecl,
) -> Result<(), CppGenError> {
    let attr_name = escape_cpp_ident(&attr.name.text);
    let ty = typespec_to_cpp(&attr.type_spec)?;
    // Getter (every attribute has one).
    writeln!(out, "{inner}virtual {ty} {attr_name}() const = 0;").map_err(fmt_err)?;
    // Setter only for non-readonly.
    if !attr.readonly {
        writeln!(
            out,
            "{inner}virtual void {attr_name}(const {ty}& value) = 0;"
        )
        .map_err(fmt_err)?;
    }
    Ok(())
}

fn emit_value_type(
    out: &mut String,
    ctx: &mut EmitCtx<'_>,
    v: &ValueDef,
) -> Result<(), CppGenError> {
    let name = escape_cpp_ident(&v.name.text);
    let ind = ctx.indent();
    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);

    emit_verbatim_at(out, &ind, &v.annotations, PlacementKind::BeforeDeclaration)?;

    // Spec idl4-cpp §7.6: valuetype -> C++ class with pure-virtual
    // public/protected accessors (state) + factory class.
    // Inheritance: public virtual for all bases + supports interfaces.
    let mut bases: Vec<String> = Vec::new();
    if let Some(inh) = &v.inheritance {
        for b in &inh.bases {
            bases.push(format!("public virtual {}", scoped_to_cpp(b)));
        }
        for s in &inh.supports {
            bases.push(format!("public virtual {}", scoped_to_cpp(s)));
        }
    }
    if bases.is_empty() {
        writeln!(out, "{ind}class {name} {{").map_err(fmt_err)?;
    } else {
        writeln!(out, "{ind}class {name} : {} {{", bases.join(", ")).map_err(fmt_err)?;
    }
    writeln!(out, "{ind}public:").map_err(fmt_err)?;
    writeln!(out, "{inner}virtual ~{name}() = default;").map_err(fmt_err)?;

    // Public state + methods.
    let mut has_protected = false;
    for el in &v.elements {
        match el {
            ValueElement::State(s) if matches!(s.visibility, StateVisibility::Public) => {
                let ty = typespec_to_cpp(&s.type_spec)?;
                for d in &s.declarators {
                    let n = escape_cpp_ident(&d.name().text);
                    writeln!(out, "{inner}virtual const {ty}& {n}() const = 0;")
                        .map_err(fmt_err)?;
                    writeln!(out, "{inner}virtual void {n}(const {ty}& value) = 0;")
                        .map_err(fmt_err)?;
                }
            }
            ValueElement::State(s) if matches!(s.visibility, StateVisibility::Private) => {
                has_protected = true;
            }
            ValueElement::Export(Export::Op(op)) => emit_interface_op(out, &inner, op)?,
            ValueElement::Export(Export::Attr(a)) => emit_interface_attr(out, &inner, a)?,
            _ => {}
        }
    }

    // Protected-State (private members in IDL -> protected in C++ per Spec).
    if has_protected {
        writeln!(out, "{ind}protected:").map_err(fmt_err)?;
        for el in &v.elements {
            if let ValueElement::State(s) = el {
                if matches!(s.visibility, StateVisibility::Private) {
                    let ty = typespec_to_cpp(&s.type_spec)?;
                    for d in &s.declarators {
                        let n = escape_cpp_ident(&d.name().text);
                        writeln!(out, "{inner}virtual const {ty}& {n}() const = 0;")
                            .map_err(fmt_err)?;
                        writeln!(out, "{inner}virtual void {n}(const {ty}& value) = 0;")
                            .map_err(fmt_err)?;
                    }
                }
            }
        }
    }

    writeln!(out, "{ind}}};").map_err(fmt_err)?;

    // Factory-Class pro factory-init (Spec §7.6: <ValueTypeName>_factory).
    let factories: Vec<&zerodds_idl::ast::InitDcl> = v
        .elements
        .iter()
        .filter_map(|e| {
            if let ValueElement::Init(i) = e {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    if !factories.is_empty() {
        writeln!(out, "{ind}class {name}_factory {{").map_err(fmt_err)?;
        writeln!(out, "{ind}public:").map_err(fmt_err)?;
        writeln!(out, "{inner}virtual ~{name}_factory() = default;").map_err(fmt_err)?;
        for f in &factories {
            let f_name = escape_cpp_ident(&f.name.text);
            let params: Vec<String> = f
                .params
                .iter()
                .map(|p| -> Result<String, CppGenError> {
                    let ty = typespec_to_cpp(&p.type_spec)?;
                    let qual = match p.attribute {
                        ParamAttribute::In => format!("const {ty}&"),
                        ParamAttribute::Out | ParamAttribute::InOut => format!("{ty}&"),
                    };
                    Ok(format!("{qual} {}", escape_cpp_ident(&p.name.text)))
                })
                .collect::<Result<_, _>>()?;
            writeln!(
                out,
                "{inner}virtual std::shared_ptr<{name}> {f_name}({}) = 0;",
                params.join(", ")
            )
            .map_err(fmt_err)?;
        }
        writeln!(out, "{ind}}};").map_err(fmt_err)?;
    }

    emit_verbatim_at(out, &ind, &v.annotations, PlacementKind::AfterDeclaration)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_typedef(
    out: &mut String,
    ctx: &mut EmitCtx<'_>,
    t: &TypedefDecl,
) -> Result<(), CppGenError> {
    let ind = ctx.indent();
    emit_verbatim_at(out, &ind, &t.annotations, PlacementKind::BeforeDeclaration)?;
    for decl in &t.declarators {
        let alias = escape_cpp_ident(&decl.name().text);
        let target_ty = type_for_declarator(&t.type_spec, decl)?;
        writeln!(out, "{ind}using {alias} = {target_ty};").map_err(fmt_err)?;
    }
    emit_verbatim_at(out, &ind, &t.annotations, PlacementKind::AfterDeclaration)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_const_decl(
    out: &mut String,
    ctx: &mut EmitCtx<'_>,
    c: &zerodds_idl::ast::ConstDecl,
) -> Result<(), CppGenError> {
    let cname = escape_cpp_ident(&c.name.text);
    let ind = ctx.indent();
    let cpp_ty = match &c.type_ {
        zerodds_idl::ast::ConstType::Integer(i) => crate::type_map::integer_to_cpp(*i).to_string(),
        zerodds_idl::ast::ConstType::Floating(f) => {
            crate::type_map::floating_to_cpp(*f).to_string()
        }
        zerodds_idl::ast::ConstType::Boolean => "bool".into(),
        zerodds_idl::ast::ConstType::Char => "char".into(),
        zerodds_idl::ast::ConstType::WideChar => "wchar_t".into(),
        zerodds_idl::ast::ConstType::Octet => "uint8_t".into(),
        // A namespace-scope `constexpr std::string`/`std::wstring` is ill-formed
        // (non-literal type, non-trivial destructor). `std::string_view` /
        // `std::wstring_view` ARE literal types, so `constexpr` is well-formed
        // and the constant still reads as a string (§7.2.3 string const →
        // constexpr string_view). The initializer string literal has static
        // storage duration, so the view is valid for the program lifetime.
        zerodds_idl::ast::ConstType::String { wide: false } => "std::string_view".into(),
        zerodds_idl::ast::ConstType::String { wide: true } => "std::wstring_view".into(),
        zerodds_idl::ast::ConstType::Scoped(s) => scoped_to_cpp(s),
        zerodds_idl::ast::ConstType::Fixed => {
            // §7.2.4.2.4 — fixed constant without a digits/scale annotation;
            // we emit it as an opaque wrapper (the caller annotates the type
            // via a separate `typedef fixed<D,S> Name;`).
            "::dds::core::Fixed<31, 0>".into()
        }
    };
    let val = const_expr_to_cpp(&c.value);
    writeln!(out, "{ind}constexpr {cpp_ty} {cname} = {val};").map_err(fmt_err)?;
    Ok(())
}

fn emit_exception(
    out: &mut String,
    ctx: &mut EmitCtx<'_>,
    e: &ExceptDecl,
) -> Result<(), CppGenError> {
    let ename = escape_cpp_ident(&e.name.text);
    let ind = ctx.indent();
    writeln!(out, "{ind}class {ename} : public std::exception {{").map_err(fmt_err)?;
    writeln!(out, "{ind}public:").map_err(fmt_err)?;
    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);
    writeln!(out, "{inner}{ename}() = default;").map_err(fmt_err)?;
    writeln!(out, "{inner}~{ename}() override = default;").map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}const char* what() const noexcept override {{ return \"{}\"; }}",
        e.name.text
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "{ind}private:").map_err(fmt_err)?;
    for m in &e.members {
        for decl in &m.declarators {
            let cpp_ty = type_for_declarator(&m.type_spec, decl)?;
            let field = member_storage_ident(&decl.name().text);
            writeln!(out, "{inner}{cpp_ty} {field};").map_err(fmt_err)?;
        }
    }
    writeln!(out, "{ind}}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// TypeSpec → C++-Type-Ausdruck
// ---------------------------------------------------------------------------

/// Returns the C++ type expression for a member (TypeSpec + Declarator).
/// Array declarators become `std::array<T, N>` (multidimensionally nested).
pub(crate) fn type_for_declarator(ts: &TypeSpec, decl: &Declarator) -> Result<String, CppGenError> {
    let base = typespec_to_cpp(ts)?;
    match decl {
        Declarator::Simple(_) => Ok(base),
        Declarator::Array(arr) => {
            // Wrap inside-out: arr.sizes[0] is the outermost dimension.
            // `int x[2][3]` → `std::array<std::array<int, 3>, 2>`.
            let mut out = base;
            for size in arr.sizes.iter().rev() {
                // Central const-eval: `data[CAP]` / `data[BASE*2]` resolve to
                // their integer dimension instead of the former literal-only
                // `const_expr_to_usize`, which dropped every non-literal to `0`
                // (`std::array<..., 0>`). Symbolic fallback keeps an in-scope
                // `constexpr` dimension compiling when it cannot be evaluated.
                let n = bound_to_cpp(size);
                out = format!("std::array<{out}, {n}>");
            }
            Ok(out)
        }
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
pub(crate) fn typespec_to_cpp(ts: &TypeSpec) -> Result<String, CppGenError> {
    match ts {
        // `long double` has a platform-dependent width and bit layout
        // (sizeof == 8 on MSVC, 16 x86-80-bit-extended on Linux/gcc, 16
        // IEEE-binary128 on AArch64) while the XCDR wire form is a fixed 16-byte
        // value. Native `long double` storage therefore either truncates the
        // wire (MSVC) or ships a non-portable bit pattern. Until a canonical
        // binary128 codec exists (blocked on Rust `f128`), reject it at codegen
        // — exactly like the C backend (`c_mode.rs`) — instead of emitting a
        // silently platform-divergent member.
        TypeSpec::Primitive(zerodds_idl::ast::PrimitiveType::Floating(
            zerodds_idl::ast::FloatingType::LongDouble,
        )) => Err(CppGenError::UnsupportedConstruct {
            construct:
                "long double (no portable 16-byte binary128 wire form; blocked on Rust f128)"
                    .to_string(),
            context: None,
        }),
        TypeSpec::Primitive(p) => Ok(primitive_to_cpp(*p).to_string()),
        TypeSpec::Scoped(s) => Ok(scoped_to_cpp(s)),
        TypeSpec::Sequence(s) => {
            let inner = typespec_to_cpp(&s.elem)?;
            Ok(format!("std::vector<{inner}>"))
        }
        TypeSpec::String(s) => {
            if s.wide {
                Ok("std::wstring".into())
            } else {
                Ok("std::string".into())
            }
        }
        TypeSpec::Map(m) => {
            let k = typespec_to_cpp(&m.key)?;
            let v = typespec_to_cpp(&m.value)?;
            Ok(format!("std::map<{k}, {v}>"))
        }
        TypeSpec::Fixed(f) => {
            // Spec idl4-cpp §7.2.4.2.4: fixed<digits, scale> ->
            // `omg::types::fixed<D, S>` (spec) resp. `dds::core::Fixed<D,S>`
            // (ZeroDDS-equivalent form). Digits/scale from the AST.
            let digits = const_expr_to_u32(&f.digits).unwrap_or(0);
            let scale = const_expr_to_u32(&f.scale).unwrap_or(0);
            Ok(format!("::dds::core::Fixed<{digits}, {scale}>"))
        }
        TypeSpec::Any => {
            // `any` is a CORBA type (TypeCode + dynamic value), absent from the
            // DDS-XTypes type system, and has NO XCDR wire codec in ZeroDDS yet.
            // Emitting a `::dds::core::Any` field (no such runtime type) produced
            // code that did not compile AND dropped the value on the wire. Reject
            // it cleanly at codegen — exactly like the C and Python backends —
            // instead of a cryptic downstream error. (Tracked: dedicated `any`
            // wire-codec follow-up; CORBA interfaces keep their own `any`.)
            Err(CppGenError::UnsupportedConstruct {
                construct: "any (no DDS-XTypes wire form / no TypeObject)".to_string(),
                context: None,
            })
        }
    }
}

pub(crate) fn switch_type_to_cpp(s: &SwitchTypeSpec) -> Result<String, CppGenError> {
    Ok(match s {
        SwitchTypeSpec::Integer(i) => crate::type_map::integer_to_cpp(*i).to_string(),
        SwitchTypeSpec::Char => "char".into(),
        SwitchTypeSpec::Boolean => "bool".into(),
        SwitchTypeSpec::Octet => "uint8_t".into(),
        SwitchTypeSpec::Scoped(s) => scoped_to_cpp(s),
    })
}

pub(crate) fn scoped_to_cpp(s: &ScopedName) -> String {
    // Mapping for Time/Duration (block E).
    if s.parts.len() == 1 {
        if let Some(mapped) = TIME_DURATION_TYPES
            .iter()
            .find(|(idl, _)| *idl == s.parts[0].text)
            .map(|(_, cpp)| *cpp)
        {
            return mapped.to_string();
        }
    }
    // Absolutely qualify known user types so member references resolve at any
    // scope — serializer helpers live at global scope, where a bare single-part
    // name (an intra-module IDL reference) would not resolve. The reference is
    // resolved to its FQN against the current lexical scope (P0-2), so two
    // same-named types in different modules map to their OWN absolute path.
    if let Some(fqn) = resolve_fqn(s) {
        if let Some(path) = TYPE_PATHS.with(|r| r.borrow().get(&fqn).cloned()) {
            return path;
        }
    }
    // Fallback for names not in TYPE_PATHS (unregistered base classes,
    // scoped enumerator references, cross-refs): escape each `::`-component so a
    // reserved-word module/type/enumerator segment stays a legal C++ token.
    let parts: Vec<String> = s.parts.iter().map(|p| escape_cpp_ident(&p.text)).collect();
    let joined = parts.join("::");
    if s.absolute {
        format!("::{joined}")
    } else {
        joined
    }
}

// ---------------------------------------------------------------------------
// ConstExpr → C++ literal string (best-effort for the foundation)
// ---------------------------------------------------------------------------

fn const_expr_to_u32(e: &ConstExpr) -> Option<u32> {
    if let ConstExpr::Literal(l) = e {
        if matches!(l.kind, LiteralKind::Integer) {
            return l.raw.parse::<u32>().ok();
        }
    }
    None
}

pub(crate) fn const_expr_to_cpp(e: &ConstExpr) -> String {
    match e {
        ConstExpr::Literal(l) => literal_to_cpp(l),
        ConstExpr::Scoped(s) => scoped_to_cpp(s),
        ConstExpr::Unary { op, operand, .. } => {
            let prefix = match op {
                zerodds_idl::ast::UnaryOp::Plus => "+",
                zerodds_idl::ast::UnaryOp::Minus => "-",
                zerodds_idl::ast::UnaryOp::BitNot => "~",
            };
            format!("{prefix}{}", const_expr_to_cpp(operand))
        }
        ConstExpr::Binary { op, lhs, rhs, .. } => {
            let opstr = match op {
                zerodds_idl::ast::BinaryOp::Or => "|",
                zerodds_idl::ast::BinaryOp::Xor => "^",
                zerodds_idl::ast::BinaryOp::And => "&",
                zerodds_idl::ast::BinaryOp::Shl => "<<",
                zerodds_idl::ast::BinaryOp::Shr => ">>",
                zerodds_idl::ast::BinaryOp::Add => "+",
                zerodds_idl::ast::BinaryOp::Sub => "-",
                zerodds_idl::ast::BinaryOp::Mul => "*",
                zerodds_idl::ast::BinaryOp::Div => "/",
                zerodds_idl::ast::BinaryOp::Mod => "%",
            };
            format!(
                "({} {opstr} {})",
                const_expr_to_cpp(lhs),
                const_expr_to_cpp(rhs)
            )
        }
    }
}

fn literal_to_cpp(l: &Literal) -> String {
    match l.kind {
        // IDL boolean literals are `TRUE`/`FALSE` (§7.2.6.4); C++ has no such
        // tokens — only lowercase `true`/`false`. Emitting the raw IDL spelling
        // produced non-compiling code (`constexpr bool F = TRUE;`). Normalise
        // case-insensitively; leave any unexpected spelling untouched.
        LiteralKind::Boolean => {
            if l.raw.eq_ignore_ascii_case("true") {
                "true".to_string()
            } else if l.raw.eq_ignore_ascii_case("false") {
                "false".to_string()
            } else {
                l.raw.clone()
            }
        }
        LiteralKind::Integer | LiteralKind::Floating => l.raw.clone(),
        LiteralKind::Char => l.raw.clone(),
        LiteralKind::WideChar => l.raw.clone(),
        LiteralKind::String => l.raw.clone(),
        LiteralKind::WideString => l.raw.clone(),
        LiteralKind::Fixed => l.raw.clone(),
    }
}

// ---------------------------------------------------------------------------
// Annotation-Helpers
// ---------------------------------------------------------------------------

fn has_key_annotation(anns: &[Annotation]) -> bool {
    has_named_annotation(anns, "key")
}

fn has_optional_annotation(anns: &[Annotation]) -> bool {
    has_named_annotation(anns, "optional")
}

fn has_shared_annotation(anns: &[Annotation]) -> bool {
    has_named_annotation(anns, "shared")
}

fn has_named_annotation(anns: &[Annotation], name: &str) -> bool {
    anns.iter().any(|a| {
        a.name.parts.last().is_some_and(|p| p.text == name)
            && matches!(a.params, AnnotationParams::None | AnnotationParams::Empty)
    })
}

/// Looks for a `@<name>(N)` and returns the uint32 value; otherwise None.
fn find_uint_annotation(anns: &[Annotation], name: &str) -> Option<u32> {
    for a in anns {
        if a.name.parts.last().is_some_and(|p| p.text == name) {
            if let AnnotationParams::Single(expr) = &a.params {
                if let Some(v) = const_expr_as_u32(expr) {
                    return Some(v);
                }
            }
        }
    }
    None
}
/// zerodds-lint: recursion-depth 64 (const_expr_as_u32 bounded by AST depth)
/// Attempts to interpret a ConstExpr as a positive u32 (only an integer
/// literal or unary plus on an integer literal).
fn const_expr_as_u32(e: &ConstExpr) -> Option<u32> {
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => parse_int_literal(raw).and_then(|v| u32::try_from(v).ok()),
        ConstExpr::Unary {
            op: zerodds_idl::ast::UnaryOp::Plus,
            operand,
            ..
        } => const_expr_as_u32(operand),
        _ => None,
    }
}

/// Returns the signed `@value(n)` of an enumerator (XTypes 1.3 §7.4.5.1), or
/// `None` when the enumerator carries no `@value`. The value may be negative
/// (the enum wire holder is a signed int32), so unlike `find_uint_annotation`
/// this accepts a leading unary minus.
fn enumerator_value_annotation(anns: &[Annotation]) -> Option<i64> {
    for a in anns {
        if a.name.parts.last().is_some_and(|p| p.text == "value") {
            if let AnnotationParams::Single(expr) = &a.params {
                if let Some(v) = const_expr_as_i64(expr) {
                    return Some(v);
                }
            }
        }
    }
    None
}

/// zerodds-lint: recursion-depth 64 (bounded by AST depth)
/// Interprets a ConstExpr as a signed i64 (integer literal, possibly with a
/// leading unary `+`/`-`). Used for enumerator `@value`.
fn const_expr_as_i64(e: &ConstExpr) -> Option<i64> {
    match e {
        ConstExpr::Literal(Literal {
            kind: LiteralKind::Integer,
            raw,
            ..
        }) => parse_int_literal(raw).and_then(|v| i64::try_from(v).ok()),
        ConstExpr::Unary {
            op: zerodds_idl::ast::UnaryOp::Plus,
            operand,
            ..
        } => const_expr_as_i64(operand),
        ConstExpr::Unary {
            op: zerodds_idl::ast::UnaryOp::Minus,
            operand,
            ..
        } => const_expr_as_i64(operand).map(|v| -v),
        _ => None,
    }
}

/// Parser for integer literals (decimal, hex `0x`, octal `0...`).
fn parse_int_literal(raw: &str) -> Option<u64> {
    let s = raw.trim_end_matches(|c: char| matches!(c, 'l' | 'L' | 'u' | 'U'));
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if s.len() > 1 && s.starts_with('0') {
        u64::from_str_radix(&s[1..], 8).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

/// Extensibility mode of a struct from its annotations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Extensibility {
    Final,
    Appendable,
    Mutable,
}

fn struct_extensibility(anns: &[Annotation]) -> Extensibility {
    // Broad-audit P0-4: read the effective extensibility through the ONE
    // central normalizer, which honors both the short forms
    // (`@final`/`@appendable`/`@mutable`) AND the long form
    // `@extensibility(FINAL|APPENDABLE|MUTABLE)` (XTypes 1.3 §7.3.3). Scanning
    // only the short forms here silently downgraded `@extensibility(MUTABLE)`
    // to the default and drifted the C++ wire (PL_CDR/EMHEADER vs DHEADER)
    // away from Rust/Java/Python.
    match zerodds_idl::semantics::extensibility_of(anns) {
        Some(ExtKind::Final) => Extensibility::Final,
        Some(ExtKind::Mutable) => Extensibility::Mutable,
        Some(ExtKind::Appendable) => Extensibility::Appendable,
        // Un-annotated default is APPENDABLE (XTypes 1.3 §7.3.3.1); an
        // un-annotated aggregate is left implementation-defined by §7.2.2.4.4
        // and `--default-extensibility` patches an explicit short form onto the
        // AST before emit, so this fallback is the spec default.
        None => Extensibility::Appendable,
    }
}

// ---------------------------------------------------------------------------
// Inheritance-Cycle-Detection (reine Self/Direct-Loops im Top-Level-Scope).
// ---------------------------------------------------------------------------

/// Walks the AST and collects `child → parent` edges (FQN strings).
///
/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_inheritance_edges(
    defs: &[Definition],
    parents: &mut std::collections::HashMap<String, String>,
    prefix: &str,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let new_prefix = if prefix.is_empty() {
                    m.name.text.clone()
                } else {
                    format!("{prefix}::{}", m.name.text)
                };
                collect_inheritance_edges(&m.definitions, parents, &new_prefix);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                let key = if prefix.is_empty() {
                    s.name.text.clone()
                } else {
                    format!("{prefix}::{}", s.name.text)
                };
                if let Some(b) = &s.base {
                    let base_str = b
                        .parts
                        .iter()
                        .map(|p| p.text.clone())
                        .collect::<Vec<_>>()
                        .join("::");
                    parents.insert(key, base_str);
                }
            }
            _ => {}
        }
    }
}

fn detect_inheritance_cycles(spec: &Specification) -> Result<(), CppGenError> {
    use std::collections::HashMap;

    let mut parents: HashMap<String, String> = HashMap::new();
    collect_inheritance_edges(&spec.definitions, &mut parents, "");

    // Cycle detection via a visited set per node.
    for start in parents.keys() {
        let mut current = start.clone();
        let mut visited: BTreeSet<String> = BTreeSet::new();
        visited.insert(current.clone());
        while let Some(p) = parents.get(&current) {
            // Match flexibly: full key or suffix match.
            let resolved = parents
                .keys()
                .find(|k| *k == p || k.ends_with(&format!("::{p}")))
                .cloned()
                .unwrap_or_else(|| p.clone());
            if visited.contains(&resolved) {
                return Err(CppGenError::InheritanceCycle {
                    type_name: short_name(&resolved),
                });
            }
            visited.insert(resolved.clone());
            // Direktes Self-Reference (Parent == Self):
            if resolved == current {
                return Err(CppGenError::InheritanceCycle {
                    type_name: short_name(&resolved),
                });
            }
            current = resolved;
            if !parents.contains_key(&current) {
                break;
            }
        }
    }
    Ok(())
}

fn short_name(s: &str) -> String {
    s.rsplit("::").next().unwrap_or(s).to_string()
}

// ---------------------------------------------------------------------------
// topic_type_support<T> — DDS-PSM-Cxx Topic-Trait-Spezialisierung
// ---------------------------------------------------------------------------
//
// Collects all top-level and module-nested structs and emits per struct
// a `dds::topic::topic_type_support<FQN>` specialization with type_name(),
// encode(), encode_be(), decode(), key_hash(), is_keyed() and extensibility().
//
// The wire format is full XCDR2 (XTypes 1.3 §7.4):
//   * Plain-CDR2 LE with alignment relative to the encapsulation start.
//   * `@final`           -> no DHEADER.
//   * `@appendable`(def) -> DHEADER (4 byte body-size) prefixed.
//   * `@mutable`         -> DHEADER + EMHEADER per member (PL_CDR2).
//   * `@key`             -> member goes into the key hash (MD5 over BE-Plain-CDR2).
//   * `@id(N)`           -> EMHEADER member-id.
//   * `@optional`        -> EMHEADER skip if absent (mutable);
//                            1-byte present-flag for final/appendable.
//   * `@must_understand` -> EMHEADER MU-Flag.
//
// Konformanz: docs/specs/zerodds-xcdr2-bindings-conformance-1.0.md (V-1..V-12).

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
fn collect_topic_structs<'a>(
    defs: &'a [Definition],
    prefix: &str,
    out: &mut Vec<(String, &'a StructDef)>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let np = if prefix.is_empty() {
                    m.name.text.clone()
                } else {
                    format!("{prefix}::{}", m.name.text)
                };
                collect_topic_structs(&m.definitions, &np, out);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                let fqn = if prefix.is_empty() {
                    s.name.text.clone()
                } else {
                    format!("{prefix}::{}", s.name.text)
                };
                out.push((fqn, s));
            }
            _ => {}
        }
    }
}

/// Collect all top-level + module-nested union definitions (FQN, def), so the
/// emitter can produce a `topic_type_support<Union>` specialization for each —
/// the union member splice path depends on it (Bug R3).
/// zerodds-lint: recursion-depth 64 (codegen AST walk; bounded by IDL nesting).
fn collect_topic_unions<'a>(
    defs: &'a [Definition],
    prefix: &str,
    out: &mut Vec<(String, &'a UnionDef)>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let np = if prefix.is_empty() {
                    m.name.text.clone()
                } else {
                    format!("{prefix}::{}", m.name.text)
                };
                collect_topic_unions(&m.definitions, &np, out);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                let fqn = if prefix.is_empty() {
                    u.name.text.clone()
                } else {
                    format!("{prefix}::{}", u.name.text)
                };
                out.push((fqn, u));
            }
            _ => {}
        }
    }
}

fn emit_topic_type_support_specs(
    out: &mut String,
    opts: &CppGenOptions,
    structs: &[(String, &StructDef)],
    unions: &[(String, &UnionDef)],
    type_names: &zerodds_idl::semantics::NameMap,
) -> Result<(), CppGenError> {
    writeln!(out).map_err(fmt_err)?;
    writeln!(
        out,
        "// DDS-PSM-Cxx topic_type_support<T> -- auto-generiert (XCDR2 Wire, XTypes 1.3 7.4)."
    )
    .map_err(fmt_err)?;
    writeln!(out, "namespace dds {{").map_err(fmt_err)?;
    writeln!(out, "namespace topic {{").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    let user_prefix = opts
        .namespace_prefix
        .as_deref()
        .filter(|p| !p.is_empty())
        .unwrap_or("");
    // Bug G2 (mutual recursion, e.g. @external Vertex<->Edge): emit ALL
    // specializations as declarations first (full class, method signatures
    // only), THEN all method bodies out-of-line. This way a body that
    // references another specialization (`topic_type_support<Edge>::encode`)
    // always sees a complete declaration of it, instead of an implicit
    // instantiation of a not-yet-defined template. Unions are emitted before
    // structs so a struct that splices a union sees the union's declaration.
    // Build the C++ FQN used as the `topic_type_support<...>` template argument
    // (and everywhere that references the type in method bodies). Each
    // `::`-component of the IDL FQN is reserved-word escaped so it matches the
    // escaped namespace/class the declaration emits. The RAW `fqn` is passed
    // separately as `type_name` (the DDS wire type name), which MUST stay the
    // original IDL FQN — escaping is name-local and must not move the wire.
    let cpp_fqn = |fqn: &str| -> String {
        let escaped = fqn
            .split("::")
            .map(escape_cpp_ident)
            .collect::<Vec<_>>()
            .join("::");
        if user_prefix.is_empty() {
            format!("::{escaped}")
        } else {
            format!("::{user_prefix}::{escaped}")
        }
    };

    // Phase 1: declarations.
    for (fqn, u) in unions {
        emit_union_topic_type_support(out, &cpp_fqn(fqn), fqn, u, TtsPhase::Decl)?;
    }
    for (fqn, s) in structs {
        let scope = fqn_module_scope(fqn);
        emit_topic_type_support_for(
            out,
            &cpp_fqn(fqn),
            fqn,
            s,
            &scope,
            type_names,
            TtsPhase::Decl,
        )?;
    }
    // Phase 2: out-of-line definitions.
    for (fqn, u) in unions {
        emit_union_topic_type_support(out, &cpp_fqn(fqn), fqn, u, TtsPhase::Def)?;
    }
    for (fqn, s) in structs {
        let scope = fqn_module_scope(fqn);
        emit_topic_type_support_for(
            out,
            &cpp_fqn(fqn),
            fqn,
            s,
            &scope,
            type_names,
            TtsPhase::Def,
        )?;
    }

    writeln!(out, "}} // namespace topic").map_err(fmt_err)?;
    writeln!(out, "}} // namespace dds").map_err(fmt_err)?;
    Ok(())
}

/// Two-phase emission for `topic_type_support<T>` specializations (Bug G2):
/// `Decl` emits the class with method *signatures only*; `Def` emits the method
/// bodies out-of-line. The split lets mutually recursive specializations see one
/// another's complete declaration before any body instantiates the other.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TtsPhase {
    Decl,
    Def,
}

/// broad-audit P0-7: rejects the `@shared` shapes this backend does not
/// serialize LOUDLY (a hard `UnsupportedConstruct`), never a silent skip that
/// would drop the referenced value from the wire. Plain `@shared` and
/// `@shared @optional` (both serialized by value, see `emit_shared_encode_ref`)
/// return `Ok(())`.
///
/// `@shared` (XTypes 1.3 §7.3.1.2.1.9) governs only the IN-MEMORY representation
/// (a shared reference / `std::shared_ptr<T>`); ON THE WIRE the referenced value
/// is serialized fully by value, byte-identical to the same member WITHOUT
/// `@shared`. A `@shared` ARRAY declarator (`std::shared_ptr<std::array<…>>`) is
/// the one shape not wired here — the array element path takes the getter
/// directly, which a shared_ptr indirection would break.
fn reject_unsupported_shared(m: &Member, decl: &Declarator) -> Result<(), CppGenError> {
    if !has_shared_annotation(&m.annotations) {
        return Ok(());
    }
    if matches!(decl, Declarator::Array(_)) {
        return Err(CppGenError::UnsupportedConstruct {
            construct: "@shared array member".to_string(),
            context: Some(escape_cpp_ident(&decl.name().text)),
        });
    }
    Ok(())
}

/// broad-audit P0-7 (encode side): emit a null-safe `const T&` reference to a
/// `@shared` member's pointee at `indent` and return the reference identifier for
/// use as the value-access expression. The referenced value is then serialized by
/// the ordinary `emit_value_write` / `emit_mutable_value_emit` path — byte-
/// identical to the same member without `@shared` (XTypes 1.3 §7.3.1.2.1.9: the
/// wire is by-value; `@shared` is in-memory sharing only). A null `shared_ptr`
/// serializes as a default-constructed value, matching a default non-`@shared`
/// member (the `static const` empty is a single per-emission instance).
fn emit_shared_encode_ref(
    out: &mut String,
    ts: &TypeSpec,
    accessor: &str,
    indent: &str,
) -> Result<String, CppGenError> {
    let cpp_ty = typespec_to_cpp(ts)?;
    let id = next_nest_id();
    let empty = format!("zd_shempty{id}");
    let rf = format!("zd_shref{id}");
    writeln!(out, "{indent}static const {cpp_ty} {empty}{{}};").map_err(fmt_err)?;
    writeln!(
        out,
        "{indent}const {cpp_ty}& {rf} = {accessor} ? *{accessor} : {empty};"
    )
    .map_err(fmt_err)?;
    Ok(rf)
}

/// Returns true if a type spec is understood by the XCDR2 codegen.
///
/// zerodds-lint: recursion-depth 64 (type-spec walk; bounded by IDL nesting)
fn typespec_supported(ts: &TypeSpec) -> bool {
    match ts {
        TypeSpec::Primitive(_) => true,
        // narrow `string` AND wide `wstring` (conformance §9.1, UTF-16 wire).
        TypeSpec::String(_) => true,
        // Sequence elements: primitives, narrow + wide string, enum (-> int32),
        // nested struct of ANY extensibility (@final → recursed inline;
        // @appendable/@mutable → 4-aligned splice per element, see emit loops),
        // nested sequence (recursed, own inner DHEADER) and map (recursed, own
        // DHEADER) are all wired.
        TypeSpec::Sequence(seq) => match &*seq.elem {
            TypeSpec::Primitive(_) => true,
            TypeSpec::String(_) => true,
            // enum (-> int32), nested struct, AND union (spliced via its own
            // DHEADER-framed TypeSupport, Bug R3) — a sequence<union> was
            // previously silently dropped from the wire (data loss).
            TypeSpec::Scoped(s) => {
                scoped_is_enum(s) || scoped_struct(s).is_some() || scoped_union(s).is_some()
            }
            TypeSpec::Sequence(_) => typespec_supported(&seq.elem),
            TypeSpec::Map(m) => typespec_supported(&m.key) && typespec_supported(&m.value),
            _ => false,
        },
        // A `Scoped` member resolving to an enum (→ int32) or to a directly-
        // encodable struct of ANY extensibility (@final → recursed inline;
        // @appendable/@mutable → spliced, see `scoped_struct`) is supported.
        // The Sequence-element arm above mirrors this (each non-final element is
        // 4-aligned + spliced/sub-decoded per its own DHEADER).
        TypeSpec::Scoped(s) => {
            scoped_is_enum(s)
                || scoped_struct(s).is_some()
                || scoped_union(s).is_some()
                || scoped_bitholder(s).is_some()
        }
        // map<K,V>: supported iff both key and value are themselves supported
        // (encode/decode recurse through emit_value_write/read per entry).
        TypeSpec::Map(m) => typespec_supported(&m.key) && typespec_supported(&m.value),
        // fixed<P,S>: raw BCD octets (CORBA §9.3.2.7 / XCDR2 §7.4.4.5), wired
        // via `::dds::core::Fixed<P,S>`. Not an XTypes type (no TypeObject) but
        // carried on the wire across every ZeroDDS binding.
        TypeSpec::Fixed(_) => true,
        _ => false,
    }
}

// Codegen-scoped type registry (thread-local = correct under concurrent header
// generation; rebuilt per `emit_header`). Holds the SIMPLE names of all enums
// and all structs, so a `Scoped` member can be classified as an enum WITHOUT a
// full name resolver: a name is treated as an enum only if it is a known enum
// simple-name AND NOT a known struct simple-name — never mis-classifying a
// struct (which would emit a broken int32 cast). Ambiguous/relative names fall
// back to "skip", i.e. no regression.
thread_local! {
    static ENUM_NAMES: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    static STRUCT_NAMES: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    static STRUCT_DEFS: RefCell<BTreeMap<String, StructDef>> = const { RefCell::new(BTreeMap::new()) };
    /// Simple union name set + union defs, so a `Scoped` member resolving to a
    /// union can be classified (Bug R3) and routed through the splice path (the
    /// union's own DHEADER-framed `topic_type_support<Union>::encode/decode`)
    /// instead of being silently dropped from the wire (data loss).
    static UNION_NAMES: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    static UNION_DEFS: RefCell<BTreeMap<String, UnionDef>> = const { RefCell::new(BTreeMap::new()) };
    /// Simple type name → fully-qualified absolute C++ path (`::mod::Sub::Name`).
    /// Serializer helpers live at global scope, so member type references must be
    /// absolutely qualified (e.g. `::nga::LinearVelocity2DType`) — a bare
    /// single-part name (an intra-module IDL reference) would not resolve there.
    static TYPE_PATHS: RefCell<BTreeMap<String, String>> = const { RefCell::new(BTreeMap::new()) };
    /// Simple typedef name → aliased type, for resolving typedef-to-primitive
    /// members (otherwise silently skipped → wire data loss).
    static TYPEDEF_MAP: RefCell<BTreeMap<String, TypeSpec>> = const { RefCell::new(BTreeMap::new()) };
    /// Simple bitmask/bitset name → holder width in BYTES (1/2/4/8). A
    /// bitmask/bitset member serializes its holder integer at this width; the
    /// width matches the cross-vendor cdr-core reference (Rust `bitset_storage_type`):
    /// bitmask = smallest int fitting the #values (or explicit `@bit_bound`),
    /// bitset = smallest int fitting the total bitfield width. Previously such
    /// members were silently skipped from the wire (data loss).
    static BITHOLDER_BYTES: RefCell<BTreeMap<String, u32>> = const { RefCell::new(BTreeMap::new()) };
    /// Simple names that are bitsets (a `struct{ uintN value; }`), as opposed to
    /// bitmasks (an `enum class : uintN`). Needed to pick `.value` vs a cast at
    /// the bitmask/bitset member encode/decode site.
    static BITSET_NAMES: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    /// Simple enum name → signed wire holder width in BYTES (1/2/4), from
    /// `@bit_bound` (XTypes §7.4.5.1). An enum member/element serializes at this
    /// width instead of a fixed int32.
    static ENUM_BYTES: RefCell<BTreeMap<String, u32>> = const { RefCell::new(BTreeMap::new()) };
    /// Simple struct names used as a `map` key — they receive a generated
    /// lexicographic `operator<` so `std::map<Struct, _>` compiles.
    static MAP_KEY_STRUCTS: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    // Monotonic counter for unique nested-struct decode temp-var names
    // (`zd_ns<N>`), so nested-nested decodes do not shadow each other.
    static NEST_CTR: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
    // Struct FQNs currently on the `scoped_struct` analysis stack.
    // Self-referential / mutually recursive types (XTypes §7.4.5, e.g.
    // `struct Node { sequence<Node> next; }`) would otherwise loop forever
    // through `scoped_struct` → `typespec_supported` → `scoped_struct` and
    // overflow the stack. A name already being visited is reported as
    // NOT inline-encodable, so the member uses the heap/splice path (its own
    // `topic_type_support` encode), which terminates at runtime on the data.
    static VISITING: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    /// FQN of every named type (enum/struct/union/typedef/bitmask/bitset). Used
    /// by `resolve_fqn` to resolve a member's `ScopedName` against the current
    /// lexical scope to the correct declaration (P0-2).
    static ALL_TYPES: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    /// Current lexical module scope during emission (outermost-first, raw IDL
    /// module names). Maintained by `emit_module` (type-decl walk) and set per
    /// serializer at the `topic_type_support` emit sites; a reference resolves
    /// its FQN bottom-up from here, exactly as `resolver.rs` does. Mirrors
    /// `idl-rust`'s `CURRENT_SCOPE`.
    static CUR_SCOPE: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// Central const/enum symbol table for the whole spec, built once per
    /// header by [`build_symbol_table`]. Every collection bound and fixed-array
    /// dimension is resolved through it (via [`resolve_bound`]) so a bound
    /// written as `CAP` or `BASE*2` yields its evaluated integer instead of a
    /// backend-local re-parse that dropped non-literals to `0` (audit P1
    /// "Const-Eval driftet").
    static CONST_SYMS: RefCell<zerodds_idl::semantics::SymbolTable> =
        RefCell::new(zerodds_idl::semantics::SymbolTable::new());
}

/// Resolves a collection-bound / array-dimension `ConstExpr` to a non-negative
/// integer through the central evaluator + the spec-wide [`CONST_SYMS`] table.
/// `None` when the expression is not a resolvable non-negative integer (e.g. a
/// forward reference or a genuinely symbolic value).
fn resolve_bound(expr: &ConstExpr) -> Option<u64> {
    CONST_SYMS.with(|c| zerodds_idl::semantics::eval_bound(expr, &c.borrow()))
}

/// Renders a bound/array-dimension expression as the C++ integer it evaluates
/// to (the central resolved value). Falls back to the symbolic C++ spelling
/// only when the expression does not resolve — the const is then relied upon to
/// be an in-scope `constexpr`. The literal fast path is value-identical, so
/// literal bounds keep their existing rendering.
fn bound_to_cpp(expr: &ConstExpr) -> String {
    resolve_bound(expr).map_or_else(|| const_expr_to_cpp(expr), |n| n.to_string())
}

/// FQN (`Mod::Sub::Name`) of a type named `name` declared in module path
/// `scope` — the raw IDL name used as every registry key (P0-2).
fn idl_fqn(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{}::{name}", scope.join("::"))
    }
}

/// Snapshot of the current lexical scope.
fn cur_scope() -> Vec<String> {
    CUR_SCOPE.with(|c| c.borrow().clone())
}

/// Sets the current lexical scope (raw IDL module names, outermost-first).
fn set_scope(scope: &[String]) {
    CUR_SCOPE.with(|c| *c.borrow_mut() = scope.to_vec());
}

/// Restores `CUR_SCOPE` on drop — used to switch into a nested type's own module
/// scope for the duration of an inline (@final) member-body recursion, so the
/// nested type's members resolve relative to WHERE the nested type is declared,
/// not the outer reference site.
struct ScopeGuard(Vec<String>);
impl Drop for ScopeGuard {
    fn drop(&mut self) {
        set_scope(&self.0);
    }
}

/// Switches `CUR_SCOPE` to the module scope of the type `sc` resolves to and
/// returns a guard that restores the previous scope on drop. A no-op switch when
/// `sc` does not resolve (fallback keeps the outer scope).
fn enter_ref_scope(sc: &ScopedName) -> ScopeGuard {
    let prev = cur_scope();
    if let Some(fqn) = resolve_fqn(sc) {
        set_scope(&fqn_module_scope(&fqn));
    }
    ScopeGuard(prev)
}

/// Resolves a member's `ScopedName` reference to the FQN of the declaration it
/// binds to, honouring the current lexical scope (`CUR_SCOPE`) exactly as
/// `resolver.rs` does: absolute names resolve from the root; relative names
/// search bottom-up from the current scope. As a safety net, when the scoped
/// search fails but the name (as a suffix) is UNIQUE across all declared types,
/// that unique declaration is used — this keeps behaviour identical to the prior
/// simple-name lookup for every non-colliding IDL while still disambiguating
/// same-named types by scope (P0-2).
fn resolve_fqn(s: &ScopedName) -> Option<String> {
    resolve_fqn_in(s, &cur_scope())
}

fn resolve_fqn_in(s: &ScopedName, scope: &[String]) -> Option<String> {
    let parts: Vec<String> = s.parts.iter().map(|p| p.text.clone()).collect();
    if parts.is_empty() {
        return None;
    }
    ALL_TYPES.with(|set| {
        let set = set.borrow();
        if s.absolute {
            let cand = parts.join("::");
            return if set.contains(&cand) {
                Some(cand)
            } else {
                None
            };
        }
        // Bottom-up: try `<scope>::<parts>`, then drop the innermost module and
        // retry, down to the root (§7.5.4).
        let mut base: Vec<String> = scope.to_vec();
        loop {
            let mut cand: Vec<String> = base.clone();
            cand.extend(parts.iter().cloned());
            let joined = cand.join("::");
            if set.contains(&joined) {
                return Some(joined);
            }
            if base.is_empty() {
                break;
            }
            base.pop();
        }
        // Fallback: unique suffix match (no scope info needed).
        let suffix = parts.join("::");
        let tail = format!("::{suffix}");
        let mut hit: Option<String> = None;
        for fqn in set.iter() {
            if *fqn == suffix || fqn.ends_with(&tail) {
                if hit.is_some() {
                    return None; // ambiguous — require real scope resolution
                }
                hit = Some(fqn.clone());
            }
        }
        hit
    })
}

fn next_nest_id() -> u32 {
    NEST_CTR.with(|c| {
        let v = c.get();
        c.set(v.wrapping_add(1));
        v
    })
}

fn set_type_registry(spec: &Specification) {
    let mut r = Registry::default();
    let mut scope: Vec<String> = Vec::new();
    collect_type_names(&spec.definitions, &mut scope, &mut r);
    // Intersect the collected map-key SIMPLE names with the simple names of all
    // declared structs — only a struct key lacks a built-in `operator<`
    // (primitives, strings, scoped enums all order natively). `r.structs` holds
    // FQNs, so compare on the FQN's simple-name tail. `MAP_KEY_STRUCTS` stays
    // keyed by simple name: it only gates whether a struct gets an `operator<`
    // (looked up by simple name at the declaration site).
    let struct_simple: BTreeSet<String> = r
        .structs
        .iter()
        .map(|fqn| fqn.rsplit("::").next().unwrap_or(fqn).to_string())
        .collect();
    let key_structs: BTreeSet<String> = r
        .map_key_names
        .iter()
        .filter(|n| struct_simple.contains(*n))
        .cloned()
        .collect();

    ENUM_NAMES.with(|c| *c.borrow_mut() = r.enums);
    STRUCT_NAMES.with(|c| *c.borrow_mut() = r.structs);
    STRUCT_DEFS.with(|c| *c.borrow_mut() = r.struct_defs);
    UNION_NAMES.with(|c| *c.borrow_mut() = r.unions);
    UNION_DEFS.with(|c| *c.borrow_mut() = r.union_defs);
    TYPE_PATHS.with(|c| *c.borrow_mut() = r.paths);
    TYPEDEF_MAP.with(|c| *c.borrow_mut() = r.typedefs);
    BITHOLDER_BYTES.with(|c| *c.borrow_mut() = r.bitholders);
    ENUM_BYTES.with(|c| *c.borrow_mut() = r.enum_bytes);
    BITSET_NAMES.with(|c| *c.borrow_mut() = r.bitsets);
    ALL_TYPES.with(|c| *c.borrow_mut() = r.all);
    MAP_KEY_STRUCTS.with(|c| *c.borrow_mut() = key_structs);
    // Central const/enum table for bound + array-dimension resolution.
    CONST_SYMS.with(|c| *c.borrow_mut() = zerodds_idl::semantics::build_symbol_table(spec));
    // Reset the lexical scope for this header's emission.
    set_scope(&[]);
}

#[derive(Default)]
struct Registry {
    /// FQN of EVERY named type (enum/struct/union/typedef/bitmask/bitset), so a
    /// reference's lexical scope can be resolved to the correct declaration even
    /// when two modules declare the same simple name (P0-2). Keyed identically to
    /// the per-category maps below.
    all: BTreeSet<String>,
    enums: BTreeSet<String>,
    structs: BTreeSet<String>,
    struct_defs: BTreeMap<String, StructDef>,
    unions: BTreeSet<String>,
    union_defs: BTreeMap<String, UnionDef>,
    paths: BTreeMap<String, String>,
    typedefs: BTreeMap<String, TypeSpec>,
    bitholders: BTreeMap<String, u32>,
    bitsets: BTreeSet<String>,
    /// Simple enum name → signed wire holder width in BYTES (1/2/4) selected by
    /// `@bit_bound` (XTypes 1.3 §7.4.5.1 / §7.3.1.2.1.2): N≤8 → 1, N≤16 → 2,
    /// else 4. Cyclone honours this; the prior fixed-int32 path dropped it.
    enum_bytes: BTreeMap<String, u32>,
    /// Simple type names that appear as a `map<Key, _>` key. A `std::map` key
    /// needs `operator<`; user structs have none by default, so a
    /// `map<Struct, _>` member failed to compile. Structs in this set get a
    /// generated lexicographic `operator<` (§7.4.2.9 map mapping).
    map_key_names: BTreeSet<String>,
}

/// The unsigned C++ holder type for a holder width in bytes.
fn holder_uint_for_bytes(bytes: u32) -> &'static str {
    match bytes {
        1 => "uint8_t",
        2 => "uint16_t",
        4 => "uint32_t",
        _ => "uint64_t",
    }
}

/// Holder width in BYTES (1/2/4/8) for a given holder *bit* width — mirrors the
/// cdr-core reference `bitset_storage_type` (u8/u16/u32/u64). Clamped to 8.
fn holder_bytes_for_bits(bits: u32) -> u32 {
    match bits {
        0..=8 => 1,
        9..=16 => 2,
        17..=32 => 4,
        _ => 8,
    }
}

/// Absolute C++ path `::a::b::Name` for a type named `name` in module `scope`.
///
/// Every path component is reserved-word escaped (`class` -> `class_`) so the
/// absolute reference resolves to the same escaped namespace/class the
/// declaration emits. This is the reference-site chokepoint: `TYPE_PATHS` is
/// built from here, so every `scoped_to_cpp` lookup (member-type refs,
/// base-class refs, nested-struct splices) inherits the escaping. Escaping is
/// name-local — it never touches the DDS wire type name (see `type_name`).
fn abs_path(scope: &[String], name: &str) -> String {
    let mut p = String::from("::");
    for s in scope {
        p.push_str(&escape_cpp_ident(s));
        p.push_str("::");
    }
    p.push_str(&escape_cpp_ident(name));
    p
}

/// zerodds-lint: recursion-depth 64 (module/type tree; bounded by IDL nesting)
fn collect_type_names(defs: &[Definition], scope: &mut Vec<String>, r: &mut Registry) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                scope.push(m.name.text.clone());
                collect_type_names(&m.definitions, scope, r);
                scope.pop();
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                // P0-2: every registry key is the fully-qualified IDL name
                // (`Mod::Sub::Type`), NOT the simple name — two same-named types in
                // different modules must NOT collide/overwrite. References resolve
                // their lexical scope to an FQN (see `resolve_fqn`).
                let fqn = idl_fqn(scope, &e.name.text);
                r.enums.insert(fqn.clone());
                r.all.insert(fqn.clone());
                r.paths.insert(fqn.clone(), abs_path(scope, &e.name.text));
                // @bit_bound → signed wire holder width (default 32 → 4 bytes).
                let bound = find_uint_annotation(&e.annotations, "bit_bound")
                    .filter(|&v| (1..=32).contains(&v))
                    .unwrap_or(32);
                let bytes = if bound <= 8 {
                    1
                } else if bound <= 16 {
                    2
                } else {
                    4
                };
                r.enum_bytes.insert(fqn, bytes);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                let fqn = idl_fqn(scope, &s.name.text);
                r.structs.insert(fqn.clone());
                r.all.insert(fqn.clone());
                r.struct_defs.insert(fqn.clone(), s.clone());
                r.paths.insert(fqn, abs_path(scope, &s.name.text));
                for m in &s.members {
                    collect_map_key_names(&m.type_spec, &mut r.map_key_names);
                }
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                // Bug R3: register unions so a union-typed struct member is
                // classified + spliced (not dropped from the wire).
                let fqn = idl_fqn(scope, &u.name.text);
                r.unions.insert(fqn.clone());
                r.all.insert(fqn.clone());
                r.union_defs.insert(fqn.clone(), u.clone());
                r.paths.insert(fqn, abs_path(scope, &u.name.text));
                for c in &u.cases {
                    collect_map_key_names(&c.element.type_spec, &mut r.map_key_names);
                }
            }
            Definition::Type(TypeDecl::Typedef(td)) => {
                for decl in &td.declarators {
                    if let Declarator::Simple(n) = decl {
                        let fqn = idl_fqn(scope, &n.text);
                        r.typedefs.insert(fqn.clone(), td.type_spec.clone());
                        r.all.insert(fqn);
                    }
                }
                collect_map_key_names(&td.type_spec, &mut r.map_key_names);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitmask(b))) => {
                // Holder bits = explicit `@bit_bound`, else the spec DEFAULT of 32
                // (Bug XV-bits): XTypes 1.3 §7.3.1.2.1.6 — a bitmask with no
                // `@bit_bound` defaults to @bit_bound=32 → a UInt32 (4-byte) holder,
                // NOT a width sized to the value count. Cross-vendor-validated
                // against the Rust reference (`bitmask_bit_bound`).
                let fqn = idl_fqn(scope, &b.name.text);
                let bits = find_uint_annotation(&b.annotations, "bit_bound").unwrap_or(32);
                r.bitholders
                    .insert(fqn.clone(), holder_bytes_for_bits(bits));
                r.all.insert(fqn.clone());
                r.paths.insert(fqn, abs_path(scope, &b.name.text));
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Bitset(b))) => {
                // Holder bits = sum of all bitfield widths (cdr-core ref).
                let fqn = idl_fqn(scope, &b.name.text);
                let total: u32 = b
                    .bitfields
                    .iter()
                    .filter_map(|f| const_expr_to_u32(&f.spec.width))
                    .sum();
                r.bitholders
                    .insert(fqn.clone(), holder_bytes_for_bits(total));
                r.bitsets.insert(fqn.clone());
                r.all.insert(fqn.clone());
                r.paths.insert(fqn, abs_path(scope, &b.name.text));
            }
            _ => {}
        }
    }
}

/// Records the simple name of every `map<Key, _>` key that is a scoped
/// (user-defined) type, recursing through sequences, nested maps and the map
/// value. A scoped key that turns out to be a struct gets a generated
/// `operator<` (see `set_type_registry`); enum/typedef/primitive keys need
/// none and are filtered out later.
/// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
fn collect_map_key_names(ts: &TypeSpec, out: &mut BTreeSet<String>) {
    match ts {
        TypeSpec::Sequence(s) => collect_map_key_names(&s.elem, out),
        TypeSpec::Map(m) => {
            if let TypeSpec::Scoped(key) = m.key.as_ref() {
                if let Some(last) = key.parts.last() {
                    out.insert(last.text.clone());
                }
            }
            collect_map_key_names(&m.key, out);
            collect_map_key_names(&m.value, out);
        }
        _ => {}
    }
}

/// Resolves a member's type through typedef chains to the effective type. A
/// typedef-to-primitive member would otherwise match neither the enum nor the
/// struct classifier and be silently skipped (XCDR2 wire data loss).
/// zerodds-lint: recursion-depth 16
fn resolve_typedef_spec(ts: &TypeSpec) -> TypeSpec {
    let mut cur = ts.clone();
    let mut scope = cur_scope();
    for _ in 0..16 {
        let TypeSpec::Scoped(s) = &cur else { break };
        // Resolve the alias name in ITS lexical scope; a typedef declared in
        // another module aliases a type resolved relative to that module.
        let Some(fqn) = resolve_fqn_in(s, &scope) else {
            break;
        };
        let Some(aliased) = TYPEDEF_MAP.with(|r| r.borrow().get(&fqn).cloned()) else {
            break;
        };
        cur = aliased;
        scope = fqn_module_scope(&fqn);
    }
    cur
}

/// Returns `m` with its type resolved through typedef chains, so the XCDR2
/// encoder/decoder dispatches on the effective type (a typedef-to-primitive is
/// no longer mis-classified and silently skipped). Cheap clone — members are small.
fn normalize_member(m: &Member) -> Member {
    let mut m2 = m.clone();
    m2.type_spec = resolve_typedef_spec(&m.type_spec);
    m2
}

/// Raw struct-def lookup by scoped name, with NO gate on whether every one
/// of the struct's members is codecable by the *general* XCDR2 encoder
/// (unlike `scoped_struct`, which requires that — see `all_encodable`). The
/// KeyHash-specific walker (`emit_key_value_write`) dealiases and expands
/// independently of the general encoder, so a nested struct that the general
/// encoder cannot fully encode (e.g. because one of ITS members is a typedef
/// alias) can still be found and expanded here.
fn struct_def_raw(s: &ScopedName) -> Option<StructDef> {
    let fqn = resolve_fqn(s)?;
    STRUCT_DEFS.with(|r| r.borrow().get(&fqn).cloned())
}

/// Sorts `members` into ascending member-id order (explicit `@id(N)`, else
/// positional index within `members`) — XTypes 1.3 §7.6.8.3.1.b / the same
/// convention `@mutable` EMHEADER member-id assignment uses elsewhere in this
/// file (see `find_uint_annotation(&m.annotations, "id")` at the `@mutable`
/// emit sites).
fn sort_members_by_id<'a>(members: &[&'a Member]) -> Vec<&'a Member> {
    let mut ordered: Vec<(u32, &Member)> = members
        .iter()
        .enumerate()
        .map(|(idx, m)| {
            (
                find_uint_annotation(&m.annotations, "id").unwrap_or(idx as u32),
                *m,
            )
        })
        .collect();
    ordered.sort_by_key(|(id, _)| *id);
    ordered.into_iter().map(|(_, m)| m).collect()
}

/// Raw IDL source spelling of a member's first declarator — the string the
/// `@autoid(HASH)` / `@hashid` member-id derivation hashes (P0-3). Never the
/// C++-escaped identifier: the NameHash is computed over the wire name.
fn member_raw_name(m: &Member) -> &str {
    m.declarators.first().map_or("", |d| d.name().text.as_str())
}

/// Resolved wire member-IDs of `s`, one per wire member (in
/// [`resolved_wire_members`] order), via the ONE central resolver in the
/// semantic layer (broad-audit P0-3:
/// `zerodds_idl::semantics::member_id::resolved_member_id`). The C++ backend no
/// longer re-derives `@autoid(HASH)` / `@hashid` with a positional counter — it
/// consumes the same NameHash derivation the TypeObject builder and idl-rust
/// use, so the EMHEADER (XCDR2) / PID (XCDR1) member ids for hashed members are
/// byte-identical to the member ids the TypeObject / descriptor carry.
///
/// The SEQUENTIAL fallback keeps the running-counter semantics the C++ wire
/// vectors are gated on (XTypes 1.3 §7.3.1.2.1 / Cyclone-confirmed): an explicit
/// `@id(n)` advances the auto-id counter to `n+1`, so the next un-annotated
/// member is `n+1` — not its positional index. The counter steps one slot per
/// declarator, so a later member keeps its id whether or not codegen emits it
/// (encode + decode consume this same list, staying in lockstep).
fn resolved_member_ids(s: &StructDef) -> Vec<u32> {
    let autoid_hash = zerodds_idl::semantics::member_id::container_autoid_hash(&s.annotations);
    let mut next: u32 = 0;
    resolved_wire_members(s)
        .iter()
        .map(|m| {
            let base_id = zerodds_idl::semantics::member_id::resolved_member_id(
                autoid_hash,
                &m.annotations,
                member_raw_name(m),
                next,
            );
            // Advance the auto-id counter past this member's declarator slots
            // (mirrors the historical `next_id = this_id + 1` per declarator).
            next = base_id + m.declarators.len() as u32;
            base_id
        })
        .collect()
}

/// KeyHash-specific value writer (XTypes 1.3 §7.6.8). Recurses through
/// typedef alias chains (dealiasing independently of the general encoder's
/// `typespec_supported`/`scoped_struct` gate — FINDING: a nested struct
/// whose own member was a typedef used to make the ENTIRE outer `@key`
/// member vanish from the KeyHash, worse than the over-inclusion bug below).
///
/// For a member whose (dealiased) type is a nested struct, expands to that
/// struct's own `@key` subset (or ALL its members if it declares none —
/// XTypes 1.3 §7.6.8: a keyless aggregate is keyed in full), in member-id
/// order, regardless of the struct's own extensibility: a KeyHolder is
/// always the FLAT concatenation of key bytes, never DHEADER-framed, even
/// when the general (non-key) encoder would splice an @appendable/@mutable
/// nested struct's own framed encode.
///
/// Falls back to the generic `emit_value_write` for primitives, strings,
/// enums, unions, bitholders — the investigation found those already
/// correct via the existing generic per-field encoder.
/// zerodds-lint: recursion-depth 16
fn emit_key_value_write(
    out: &mut String,
    ts: &TypeSpec,
    access: &str,
    endian: &str,
    origin: &str,
) -> Result<(), CppGenError> {
    if let TypeSpec::Scoped(s) = ts {
        let resolved = resolve_typedef_spec(ts);
        if resolved != *ts {
            return emit_key_value_write(out, &resolved, access, endian, origin);
        }
        if let Some(def) = struct_def_raw(s) {
            // Recurse into the nested struct's key members in ITS module scope
            // (P0-2): `struct_def_raw` resolved `s` in the outer scope above.
            let _sg = enter_ref_scope(s);
            let nested_keys: Vec<&Member> = def
                .members
                .iter()
                .filter(|m| has_key_annotation(&m.annotations))
                .collect();
            let effective: Vec<&Member> = if nested_keys.is_empty() {
                def.members.iter().collect()
            } else {
                nested_keys
            };
            for m in sort_members_by_id(&effective) {
                for decl in &m.declarators {
                    let field = &decl.name().text;
                    if matches!(decl, Declarator::Array(_)) {
                        // Loud codegen error, not a silent skip: dropping the
                        // field from the KeyHash would silently under-encode
                        // the KeyHolder (same class of bug this function
                        // exists to fix). Matches the other 12 backends'
                        // identical rejection of an array field inside a
                        // nested-struct @key (e.g. `idl-rust`'s
                        // `emit_key_field_write`, `idl-go`'s
                        // `emit_key_struct_member`).
                        return Err(CppGenError::UnsupportedConstruct {
                            construct: "array field inside a nested-struct @key".to_string(),
                            context: Some(field.clone()),
                        });
                    }
                    emit_key_value_write(
                        out,
                        &m.type_spec,
                        &format!("{access}.{}()", escape_cpp_ident(field)),
                        endian,
                        origin,
                    )?;
                }
            }
            return Ok(());
        }
    }
    emit_value_write(out, ts, access, endian, origin, "    ")
}

/// If `s` resolves to a union, return its [`UnionDef`]. A union member is wired
/// (Bug R3) via the union's own DHEADER-framed `topic_type_support<Union>`
/// encode/decode (splice path, identical to an `@appendable` nested struct).
fn scoped_union(s: &ScopedName) -> Option<UnionDef> {
    let fqn = resolve_fqn(s)?;
    UNION_DEFS.with(|r| r.borrow().get(&fqn).cloned())
}

/// If `s` names a registered bitmask or bitset, return its holder width in
/// BYTES (1/2/4/8). Such a member serializes its holder integer at this width
/// (cdr-core reference) — previously the whole member was skipped (wire data
/// loss). The struct exposes `enum class : uintN` (bitmask) or `struct{ uint64_t
/// value; }` (bitset); both narrow to the holder width on the wire.
fn scoped_bitholder(s: &ScopedName) -> Option<u32> {
    let fqn = resolve_fqn(s)?;
    BITHOLDER_BYTES.with(|r| r.borrow().get(&fqn).copied())
}

/// Signed wire holder width in BYTES (1/2/4) of an enum named by `s`, from its
/// `@bit_bound` (XTypes §7.4.5.1). Defaults to 4 for an unregistered name.
fn scoped_enum_bytes(s: &ScopedName) -> u32 {
    let Some(fqn) = resolve_fqn(s) else {
        return 4;
    };
    ENUM_BYTES
        .with(|r| r.borrow().get(&fqn).copied())
        .unwrap_or(4)
}

/// The signed C++ integer wire type for an enum holder of `bytes` width.
fn enum_wire_ctype(bytes: u32) -> &'static str {
    match bytes {
        1 => "int8_t",
        2 => "int16_t",
        _ => "int32_t",
    }
}

/// `true` if `s` names a registered bitset (struct holder), vs a bitmask
/// (enum-class holder). Drives `.value` access vs an enum-class cast.
fn scoped_is_bitset(s: &ScopedName) -> bool {
    let Some(fqn) = resolve_fqn(s) else {
        return false;
    };
    BITSET_NAMES.with(|r| r.borrow().contains(&fqn))
}

/// `true` if `s` (a member's scoped type name) unambiguously names an enum.
fn scoped_is_enum(s: &ScopedName) -> bool {
    let Some(fqn) = resolve_fqn(s) else {
        return false;
    };
    // With FQN keys a name binds to exactly one declaration; the `!is_struct`
    // guard is kept for safety against the fallback suffix match.
    let is_enum = ENUM_NAMES.with(|r| r.borrow().contains(&fqn));
    let is_struct = STRUCT_NAMES.with(|r| r.borrow().contains(&fqn));
    is_enum && !is_struct
}

/// If `s` resolves to a `@final` struct whose members are ALL directly encodable
/// (single Simple declarator + a type the XCDR2 member encoder handles), returns
/// the [`StructDef`] so the encoder can recurse into it inline (Plain-CDR2: no
/// DHEADER for `@final`, Spec §7.4.3.4.1). Appendable/mutable nested structs +
/// arrays/sub-structs inside the nested struct fall back to "not supported"
/// (whole member skipped — never a partial encode).
fn scoped_final_struct(s: &ScopedName) -> Option<StructDef> {
    match scoped_struct(s) {
        Some((def, Extensibility::Final)) => Some(def),
        _ => None,
    }
}

/// Like [`scoped_final_struct`] but for **any** extensibility: returns the
/// [`StructDef`] + its [`Extensibility`] when `s` resolves to a struct whose
/// members are all directly encodable. The member encoder picks the wire form
/// per extensibility — `@final` recurses inline (no DHEADER, alignment relative
/// to the outer origin), while `@appendable`/`@mutable` are **spliced** from the
/// nested type's own `topic_type_support<...>::encode`/`decode`. The latter is
/// byte-correct because the nested struct's own DHEADER (Plain-CDR2 for
/// appendable, the mutable scope for mutable) forces 4-alignment, and under
/// XCDR2 (`max_align == 4`) a 4-aligned splice point preserves every member's
/// relative alignment (Spec §7.4.3.4.2).
///
/// zerodds-lint: recursion-depth 64 (via typespec_supported; bounded by IDL nesting)
fn scoped_struct(s: &ScopedName) -> Option<(StructDef, Extensibility)> {
    let fqn = resolve_fqn(s)?;
    let def = STRUCT_DEFS.with(|r| r.borrow().get(&fqn).cloned())?;
    let ext = effective_extensibility(&fqn, &def.annotations);
    // Cycle guard: a recursive type (directly or mutually recursive, XTypes
    // §7.4.5 — e.g. `struct Node { sequence<Node> next; }`) would otherwise loop
    // forever through `scoped_struct` → `typespec_supported` → `scoped_struct`
    // and overflow the stack. When a name is already on the analysis stack,
    // report it as a supported struct WITHOUT re-walking its members. It is
    // `effective_extensibility`-coerced to non-`@final`, so every emit site
    // routes it through the splice path (its own `topic_type_support`
    // encode/decode, length-delimited by a DHEADER), which terminates at runtime
    // on the data — no inline expansion, no member dropped (no wire data loss).
    if VISITING.with(|v| v.borrow().contains(&fqn)) {
        return Some((def, ext));
    }
    VISITING.with(|v| {
        v.borrow_mut().insert(fqn.clone());
    });
    // The struct's members reference types relative to the struct's OWN module
    // scope, not the reference site's — switch `CUR_SCOPE` for the walk (P0-2).
    let prev = cur_scope();
    set_scope(&fqn_module_scope(&fqn));
    let all_encodable = def.members.iter().all(|m| {
        m.declarators.len() == 1
            && matches!(m.declarators.first(), Some(Declarator::Simple(_)))
            && typespec_supported(&m.type_spec)
    });
    set_scope(&prev);
    VISITING.with(|v| {
        v.borrow_mut().remove(&fqn);
    });
    if all_encodable {
        Some((def, ext))
    } else {
        None
    }
}

/// Fully-resolved wire member list for a struct: base-class members FIRST
/// (recursive, multi-level), then the struct's own members. XTypes inheritance
/// (§7.4.3.4.1) places base members before derived members on the wire; the
/// codec (encode/decode/key_hash) must serialize them in that order. The C++
/// class inherits base accessors (`class Derived : public Base`), so a base
/// member's `zd_v.<name>()` resolves through inheritance. Base resolution uses
/// the global `STRUCT_DEFS` registry keyed by simple name; a cycle guard bounds
/// pathological inheritance loops.
fn resolved_wire_members(s: &StructDef) -> Vec<Member> {
    let mut chain: Vec<StructDef> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut cur = s.base.clone();
    // Base classes resolve relative to each struct's own module scope; start from
    // the current lexical scope (set by the caller to `s`'s module).
    let mut scope = cur_scope();
    while let Some(bn) = cur {
        let Some(fqn) = resolve_fqn_in(&bn, &scope) else {
            break;
        };
        if !seen.insert(fqn.clone()) {
            break;
        }
        let Some(def) = STRUCT_DEFS.with(|r| r.borrow().get(&fqn).cloned()) else {
            break;
        };
        scope = fqn_module_scope(&fqn);
        cur = def.base.clone();
        chain.push(def);
    }
    // `chain` is [parent, grandparent, …]; reverse so the oldest ancestor's
    // members lead, then each descendant's, then the struct's own members.
    let mut out: Vec<Member> = Vec::new();
    for def in chain.into_iter().rev() {
        out.extend(def.members.iter().cloned());
    }
    out.extend(s.members.iter().cloned());
    out
}

/// Declared extensibility of a struct, coerced to `@appendable` when the struct
/// is recursive (directly or mutually). A recursive type cannot be inlined
/// (infinite expansion), so it must be serialized in a length-delimited form
/// (DHEADER) and spliced via its own `topic_type_support`. `@final` recursive
/// types are therefore promoted to `@appendable` *consistently* — at every
/// member/element splice site AND in the type's own standalone serializer — so
/// the DHEADER the splice-decode reads is exactly the one the standalone encode
/// wrote (XTypes §7.4.3.4.2 + §7.4.5). Non-recursive types keep their declared
/// extensibility, so this never changes a non-recursive wire format.
fn effective_extensibility(name: &str, anns: &[Annotation]) -> Extensibility {
    let declared = struct_extensibility(anns);
    if matches!(declared, Extensibility::Final) && struct_is_recursive(name) {
        Extensibility::Appendable
    } else {
        declared
    }
}

/// `true` if the struct named `root` can reach itself through the member-type
/// graph (directly, via a nested struct, or through sequence/array/map element
/// types). Its own visited set bounds the search, so it terminates even on
/// recursive types — unlike the `scoped_struct` encodability walk it guards.
fn struct_is_recursive(root: &str) -> bool {
    let Some(def) = STRUCT_DEFS.with(|r| r.borrow().get(root).cloned()) else {
        return false;
    };
    let mut visited = BTreeSet::new();
    visited.insert(root.to_string());
    let scope = fqn_module_scope(root);
    def.members
        .iter()
        .any(|m| type_reaches(root, &m.type_spec, &scope, &mut visited))
}

/// zerodds-lint: recursion-depth 64 (member-graph walk; bounded by `visited`)
/// `target` and the `visited` set are FQNs; `scope` is the module scope of the
/// struct whose members are currently walked, so a relative member reference
/// resolves to the correct declaration (P0-2).
fn type_reaches(
    target: &str,
    ts: &TypeSpec,
    scope: &[String],
    visited: &mut BTreeSet<String>,
) -> bool {
    // Resolve typedef aliases relative to the current scope.
    let prev = cur_scope();
    set_scope(scope);
    let resolved = resolve_typedef_spec(ts);
    set_scope(&prev);
    match resolved {
        TypeSpec::Scoped(s) => {
            let Some(fqn) = resolve_fqn_in(&s, scope) else {
                return false;
            };
            if fqn == target {
                return true;
            }
            if !visited.insert(fqn.clone()) {
                return false;
            }
            let Some(def) = STRUCT_DEFS.with(|r| r.borrow().get(&fqn).cloned()) else {
                return false;
            };
            let inner = fqn_module_scope(&fqn);
            def.members
                .iter()
                .any(|m| type_reaches(target, &m.type_spec, &inner, visited))
        }
        TypeSpec::Sequence(seq) => type_reaches(target, &seq.elem, scope, visited),
        TypeSpec::Map(m) => {
            type_reaches(target, &m.key, scope, visited)
                || type_reaches(target, &m.value, scope, visited)
        }
        _ => false,
    }
}

fn emit_topic_type_support_for(
    out: &mut String,
    cpp_fqn: &str,
    type_name: &str,
    s: &StructDef,
    scope: &[String],
    type_names: &zerodds_idl::semantics::NameMap,
    phase: TtsPhase,
) -> Result<(), CppGenError> {
    // Every member/base reference in this struct's serializer resolves relative
    // to the struct's own module scope (P0-2).
    let _sg = ScopeGuard(cur_scope());
    set_scope(scope);
    // Coerced ext: a `@final` recursive type is promoted to `@appendable` here
    // too, so its standalone serializer writes the same DHEADER the splice-decode
    // at every reference site reads back (see `effective_extensibility`). The
    // struct's FQN (== `type_name`) keys the recursion analysis.
    let ext = effective_extensibility(type_name, &s.annotations);
    let is_keyed = resolved_wire_members(s)
        .iter()
        .any(|m| has_key_annotation(&m.annotations));

    if phase == TtsPhase::Decl {
        writeln!(out, "template <>").map_err(fmt_err)?;
        writeln!(out, "struct topic_type_support<{cpp_fqn}> {{").map_err(fmt_err)?;
        writeln!(
            out,
            "    static const char* type_name() {{ return \"{type_name}\"; }}"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "    static constexpr bool is_keyed() {{ return {}; }}",
            if is_keyed { "true" } else { "false" }
        )
        .map_err(fmt_err)?;
        let ext_lit = match ext {
            Extensibility::Final => "FINAL",
            Extensibility::Appendable => "APPENDABLE",
            Extensibility::Mutable => "MUTABLE",
        };
        writeln!(
            out,
            "    static constexpr ::dds::topic::core::policy::DataRepresentationKind extensibility() {{ return ::dds::topic::core::policy::DataRepresentationKind::{ext_lit}; }}"
        )
        .map_err(fmt_err)?;
        // F-TYPES-3 / #24: serialized COMPLETE TypeObject accessor — the bytes
        // `zerodds::TypedWriter/Reader` hand to `zerodds_*_create_typed`.
        writeln!(out, "    static const uint8_t* type_object();").map_err(fmt_err)?;
        writeln!(out, "    static uintptr_t type_object_len();").map_err(fmt_err)?;
        // method signatures only.
        emit_encode_fn(out, cpp_fqn, s, ext, /*be=*/ false, TtsPhase::Decl)?;
        emit_encode_fn(out, cpp_fqn, s, ext, /*be=*/ true, TtsPhase::Decl)?;
        emit_decode_fn(out, cpp_fqn, s, ext, TtsPhase::Decl)?;
        emit_key_hash_fn(out, cpp_fqn, s, is_keyed, TtsPhase::Decl)?;
        writeln!(out, "}};").map_err(fmt_err)?;
        writeln!(out).map_err(fmt_err)?;
        return Ok(());
    }

    // Def phase: out-of-line bodies.
    // F-TYPES-3 / #24: TypeObject byte constant + accessors.
    emit_type_object_fns(out, cpp_fqn, s, scope, type_names)?;
    // encode (LE)
    emit_encode_fn(out, cpp_fqn, s, ext, /*be=*/ false, TtsPhase::Def)?;
    // encode_be (BE)
    emit_encode_fn(out, cpp_fqn, s, ext, /*be=*/ true, TtsPhase::Def)?;
    // decode (LE)
    emit_decode_fn(out, cpp_fqn, s, ext, TtsPhase::Def)?;
    // key_hash (BE Plain-CDR2 of @key members + MD5)
    emit_key_hash_fn(out, cpp_fqn, s, is_keyed, TtsPhase::Def)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// Module-path scope (outermost-first, type name dropped) of a fully-qualified
/// `Module::Inner::Type` name — the scope `map_type_spec_resolved` needs for
/// innermost-first relative member-type resolution (mirrors `idl-rust`'s
/// `CURRENT_SCOPE`).
fn fqn_module_scope(fqn: &str) -> Vec<String> {
    let mut parts: Vec<String> = fqn.split("::").map(|p| p.to_string()).collect();
    parts.pop(); // drop the type's own simple name
    parts
}

/// F-TYPES-3 / #24: emits `topic_type_support<T>::type_object()` /
/// `type_object_len()` out-of-line. The body carries the COMPLETE `TypeObject`
/// serialized (XCDR-LE) by the SHARED
/// `zerodds_idl::semantics::complete_struct_type_object_bytes` — the SAME source
/// `idl-rust`'s `TYPE_IDENTIFIER` codegen uses, so the two bindings emit
/// byte-identical bytes (and thus the identical `TypeIdentifier`).
///
/// A struct whose members cannot all be resolved (a `fixed`/`any` member, or a
/// scoped reference absent from the registry) emits an empty accessor
/// (`nullptr` / `0`); `zerodds::TypedWriter` then falls back to the
/// byte-oriented create — the pre-#24 behavior, never a codegen failure.
fn emit_type_object_fns(
    out: &mut String,
    cpp_fqn: &str,
    s: &StructDef,
    scope: &[String],
    type_names: &zerodds_idl::semantics::NameMap,
) -> Result<(), CppGenError> {
    let bytes =
        zerodds_idl::semantics::complete_struct_type_object_bytes(s, scope, type_names).ok();
    match bytes {
        Some(bytes) if !bytes.is_empty() => {
            writeln!(
                out,
                "inline const uint8_t* topic_type_support<{cpp_fqn}>::type_object() {{"
            )
            .map_err(fmt_err)?;
            write!(out, "    static const uint8_t ZD_TYPE_OBJECT[] = {{").map_err(fmt_err)?;
            for (i, b) in bytes.iter().enumerate() {
                if i % 12 == 0 {
                    write!(out, "\n        ").map_err(fmt_err)?;
                }
                write!(out, "0x{b:02x}, ").map_err(fmt_err)?;
            }
            writeln!(out, "\n    }};").map_err(fmt_err)?;
            writeln!(out, "    return ZD_TYPE_OBJECT;").map_err(fmt_err)?;
            writeln!(out, "}}").map_err(fmt_err)?;
            writeln!(
                out,
                "inline uintptr_t topic_type_support<{cpp_fqn}>::type_object_len() {{ return {}; }}",
                bytes.len()
            )
            .map_err(fmt_err)?;
        }
        _ => {
            writeln!(
                out,
                "inline const uint8_t* topic_type_support<{cpp_fqn}>::type_object() {{ return nullptr; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "inline uintptr_t topic_type_support<{cpp_fqn}>::type_object_len() {{ return 0; }}"
            )
            .map_err(fmt_err)?;
        }
    }
    Ok(())
}

/// Discriminator type of a union as a TypeSpec for the value-writer/reader.
/// Enum + integer switches wire as their primitive; char/octet/bool too.
fn switch_type_spec(s: &SwitchTypeSpec) -> TypeSpec {
    match s {
        SwitchTypeSpec::Integer(i) => TypeSpec::Primitive(PrimitiveType::Integer(*i)),
        SwitchTypeSpec::Char => TypeSpec::Primitive(PrimitiveType::Char),
        SwitchTypeSpec::Boolean => TypeSpec::Primitive(PrimitiveType::Boolean),
        SwitchTypeSpec::Octet => TypeSpec::Primitive(PrimitiveType::Octet),
        SwitchTypeSpec::Scoped(sc) => TypeSpec::Scoped(sc.clone()),
    }
}

/// Bug R3: a union is wired as an `@appendable`-style aggregate (XTypes §7.4.3):
/// DHEADER, then the discriminator value, then the selected branch's value. The
/// `topic_type_support<Union>` specialization splices exactly like an appendable
/// nested struct, so a union-typed struct member round-trips over the wire.
fn emit_union_topic_type_support(
    out: &mut String,
    cpp_fqn: &str,
    type_name: &str,
    u: &UnionDef,
    phase: TtsPhase,
) -> Result<(), CppGenError> {
    // The discriminator + every case type reference resolves relative to the
    // union's own module scope (P0-2). `type_name` is the union's raw IDL FQN.
    let _sg = ScopeGuard(cur_scope());
    set_scope(&fqn_module_scope(type_name));
    let disc_ts = switch_type_spec(&u.switch_type);
    let disc_cpp = switch_type_to_cpp(&u.switch_type)?;
    // A union honours its extensibility just like a struct (XTypes §7.4.4.5):
    // @final = `discriminator + selected branch` with NO DHEADER (rule (26),
    // vendor-confirmed 8 B byte-identical to CycloneDDS); @appendable/@mutable
    // prepend a DHEADER. Previously the top-level union TypeSupport hard-coded
    // APPENDABLE + an unconditional DHEADER, dropping the @final wire form.
    let ext = union_extensibility(&u.annotations);
    let has_dheader = !matches!(ext, Extensibility::Final);
    let kind = match ext {
        Extensibility::Final => "FINAL",
        Extensibility::Mutable => "MUTABLE",
        Extensibility::Appendable => "APPENDABLE",
    };

    if phase == TtsPhase::Decl {
        writeln!(out, "template <>").map_err(fmt_err)?;
        writeln!(out, "struct topic_type_support<{cpp_fqn}> {{").map_err(fmt_err)?;
        writeln!(
            out,
            "    static const char* type_name() {{ return \"{type_name}\"; }}"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "    static constexpr bool is_keyed() {{ return false; }}"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "    static constexpr ::dds::topic::core::policy::DataRepresentationKind extensibility() {{ return ::dds::topic::core::policy::DataRepresentationKind::{kind}; }}"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "    static std::vector<uint8_t> encode(const {cpp_fqn}& zd_v);"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "    static std::vector<uint8_t> encode(const {cpp_fqn}& zd_v, ::dds::topic::xcdr2::XcdrVersion zd_repr);"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "    static std::vector<uint8_t> encode_be(const {cpp_fqn}& zd_v);"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "    static std::vector<uint8_t> encode_be(const {cpp_fqn}& zd_v, ::dds::topic::xcdr2::XcdrVersion zd_repr);"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "    static {cpp_fqn} decode(const uint8_t* zd_buf, size_t zd_len, ::dds::topic::xcdr2::XcdrVersion zd_repr, bool zd_be = false);"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "    static std::array<uint8_t, 16> key_hash(const {cpp_fqn}& zd_v);"
        )
        .map_err(fmt_err)?;
        // F-TYPES-3 / #24: union TypeObject lowering is not part of the shared
        // struct builder yet, so the accessor is an empty stub — a union topic's
        // `zerodds::TypedWriter` falls back to the byte-oriented create. Present
        // for trait uniformity so `traits::type_object()` compiles for any T.
        writeln!(
            out,
            "    static const uint8_t* type_object() {{ return nullptr; }}"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "    static uintptr_t type_object_len() {{ return 0; }}"
        )
        .map_err(fmt_err)?;
        writeln!(out, "}};").map_err(fmt_err)?;
        writeln!(out).map_err(fmt_err)?;
        return Ok(());
    }

    // Def phase: out-of-line bodies. encode (LE delegator + repr-aware) + BE.
    for be in [false, true] {
        let endian = if be { "be" } else { "le" };
        let beb = if be { "true" } else { "false" };
        if be {
            writeln!(
                out,
                "inline std::vector<uint8_t> topic_type_support<{cpp_fqn}>::encode_be(const {cpp_fqn}& zd_v) {{"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "        return encode_be(zd_v, ::dds::topic::xcdr2::XcdrVersion::Xcdr2);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "    }}").map_err(fmt_err)?;
            writeln!(
                out,
                "inline std::vector<uint8_t> topic_type_support<{cpp_fqn}>::encode_be(const {cpp_fqn}& zd_v, ::dds::topic::xcdr2::XcdrVersion zd_repr) {{"
            )
            .map_err(fmt_err)?;
        } else {
            writeln!(
                out,
                "inline std::vector<uint8_t> topic_type_support<{cpp_fqn}>::encode(const {cpp_fqn}& zd_v) {{"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "        return encode(zd_v, ::dds::topic::xcdr2::XcdrVersion::Xcdr2);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "    }}").map_err(fmt_err)?;
            writeln!(
                out,
                "inline std::vector<uint8_t> topic_type_support<{cpp_fqn}>::encode(const {cpp_fqn}& zd_v, ::dds::topic::xcdr2::XcdrVersion zd_repr) {{"
            )
            .map_err(fmt_err)?;
        }
        writeln!(out, "        std::vector<uint8_t> zd_out;").map_err(fmt_err)?;
        writeln!(out, "        (void)zd_v;").map_err(fmt_err)?;
        // Both LE (`encode`) and BE (`encode_be`) carry `zd_repr` now, so the
        // alignment cap and DHEADER framing are identical apart from byte order.
        writeln!(
            out,
            "        const size_t zd_max_align = ::dds::topic::xcdr2::xcdr_max_align(zd_repr);"
        )
        .map_err(fmt_err)?;
        writeln!(out, "        (void)zd_max_align;").map_err(fmt_err)?;
        // @appendable/@mutable: DHEADER then origin at body start (XCDR2 only —
        // XCDR1 / classic CDR has no DHEADER). @final: no DHEADER in any repr.
        if has_dheader {
            writeln!(
                out,
                "        size_t zd_dh = ::dds::topic::xcdr2::DHEADER_NONE; (void)zd_dh;"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "        if (zd_repr != ::dds::topic::xcdr2::XcdrVersion::Xcdr1) {{ zd_dh = ::dds::topic::xcdr2::dheader_begin(zd_out); }}"
            )
            .map_err(fmt_err)?;
        }
        writeln!(out, "        const size_t zd_origin = zd_out.size();").map_err(fmt_err)?;
        writeln!(out, "        (void)zd_origin;").map_err(fmt_err)?;
        // discriminator.
        emit_value_write(out, &disc_ts, "zd_v._d()", endian, "zd_origin", "        ")?;
        // branch selection on the discriminator.
        emit_union_branch_switch(out, u, &disc_cpp, /*decode=*/ false, endian)?;
        if has_dheader {
            writeln!(
                out,
                "        ::dds::topic::xcdr2::dheader_end_r(zd_out, zd_dh, {beb}, zd_repr);"
            )
            .map_err(fmt_err)?;
        }
        writeln!(out, "        return zd_out;").map_err(fmt_err)?;
        writeln!(out, "    }}").map_err(fmt_err)?;
    }

    // decode.
    writeln!(
        out,
        "inline {cpp_fqn} topic_type_support<{cpp_fqn}>::decode(const uint8_t* zd_buf, size_t zd_len, ::dds::topic::xcdr2::XcdrVersion zd_repr, bool zd_be) {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        size_t zd_pos = 0;").map_err(fmt_err)?;
    writeln!(out, "        {cpp_fqn} zd_v;").map_err(fmt_err)?;
    writeln!(
        out,
        "        const size_t zd_max_align = ::dds::topic::xcdr2::xcdr_max_align(zd_repr);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "        (void)zd_buf; (void)zd_len; (void)zd_pos; (void)zd_max_align; (void)zd_be;"
    )
    .map_err(fmt_err)?;
    if has_dheader {
        // @appendable/@mutable union: a DHEADER under XCDR2, NONE under XCDR1.
        writeln!(out, "        size_t zd_end = zd_len; (void)zd_end;").map_err(fmt_err)?;
        writeln!(
            out,
            "        if (zd_repr != ::dds::topic::xcdr2::XcdrVersion::Xcdr1) {{ const auto zd_dh = ::dds::topic::xcdr2::dheader_read(zd_buf, zd_pos, zd_len, zd_be); zd_end = zd_pos + zd_dh; }}"
        )
        .map_err(fmt_err)?;
        writeln!(out, "        const size_t zd_origin = zd_pos;").map_err(fmt_err)?;
    } else {
        // @final: no DHEADER; body starts at the current position (0).
        writeln!(out, "        const size_t zd_origin = zd_pos;").map_err(fmt_err)?;
    }
    writeln!(out, "        {disc_cpp} zd_disc{{}};").map_err(fmt_err)?;
    emit_value_read(out, &disc_ts, "zd_disc =", "zd_origin", "        ", false)?;
    writeln!(out, "        zd_v._d(zd_disc);").map_err(fmt_err)?;
    emit_union_branch_switch(out, u, &disc_cpp, /*decode=*/ true, "le")?;
    if has_dheader {
        writeln!(
            out,
            "        if (zd_repr != ::dds::topic::xcdr2::XcdrVersion::Xcdr1 && zd_pos < zd_end) zd_pos = zd_end;"
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "        return zd_v;").map_err(fmt_err)?;
    writeln!(out, "    }}").map_err(fmt_err)?;

    // key_hash: unions are not keyed → zero hash.
    writeln!(
        out,
        "inline std::array<uint8_t, 16> topic_type_support<{cpp_fqn}>::key_hash(const {cpp_fqn}& zd_v) {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        (void)zd_v;").map_err(fmt_err)?;
    writeln!(
        out,
        "        return std::array<uint8_t, 16>{{{{0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0}}}};"
    )
    .map_err(fmt_err)?;
    writeln!(out, "    }}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// Render a union case label. For a scoped-enum discriminator, a bare label
/// (`K_A`) is an `enum class` enumerator and must be qualified by the enum type
/// (`::conf::Kind::K_A`); other labels (integers, chars, already-qualified
/// enumerators) pass through `const_expr_to_cpp`.
fn render_case_label(expr: &ConstExpr, enum_switch: bool, disc_cpp: &str) -> String {
    if enum_switch {
        if let ConstExpr::Scoped(s) = expr {
            if s.parts.len() == 1 {
                // Escape the bare enumerator so a reserved-word label
                // (`delete` -> `delete_`) matches the escaped `enum class`.
                return format!("{disc_cpp}::{}", escape_cpp_ident(&s.parts[0].text));
            }
        }
    }
    const_expr_to_cpp(expr)
}

/// Emit the discriminator `switch` that encodes/decodes the active union branch.
/// On encode it reads the branch from `std::get<T>(zd_v.value())`; on decode it
/// reads the branch type and assigns `zd_v.value() = <decoded>`.
fn emit_union_branch_switch(
    out: &mut String,
    u: &UnionDef,
    disc_cpp: &str,
    decode: bool,
    endian: &str,
) -> Result<(), CppGenError> {
    emit_union_branch_switch_at(out, u, disc_cpp, decode, endian, "zd_v", "zd_origin")
}

/// Like [`emit_union_branch_switch`] but parameterized on the C++ access
/// expression for the union value (`access`, e.g. `zd_v` standalone or
/// `zd_v.reading()` when inlined as a `@final` member) and the alignment origin
/// (`origin`, the outer struct origin when inlined). Used to inline a `@final`
/// union member (XTypes 1.3 §7.4.3.4.1 / rule (26) FUNION_TYPE: disc + selected
/// member, NO DHEADER) instead of splicing its own DHEADER-framed serializer.
fn emit_union_branch_switch_at(
    out: &mut String,
    u: &UnionDef,
    disc_cpp: &str,
    decode: bool,
    endian: &str,
    access: &str,
    origin: &str,
) -> Result<(), CppGenError> {
    // For a scoped-enum discriminator, an unqualified case label (`K_A`) names an
    // enumerator of an `enum class` and must be qualified (`::conf::Kind::K_A`).
    let enum_switch = matches!(&u.switch_type, SwitchTypeSpec::Scoped(s) if scoped_is_enum(s));
    writeln!(
        out,
        "        switch (static_cast<{disc_cpp}>({access}._d())) {{"
    )
    .map_err(fmt_err)?;
    let mut default_case: Option<&Case> = None;
    for c in &u.cases {
        let mut is_default = false;
        let mut had_label = false;
        for label in &c.labels {
            match label {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(expr) => {
                    let val = render_case_label(expr, enum_switch, disc_cpp);
                    writeln!(out, "        case static_cast<{disc_cpp}>({val}):")
                        .map_err(fmt_err)?;
                    had_label = true;
                }
            }
        }
        if is_default {
            default_case = Some(c);
            continue;
        }
        if !had_label {
            continue;
        }
        writeln!(out, "        {{").map_err(fmt_err)?;
        emit_union_branch_body(out, c, decode, endian, access, origin)?;
        writeln!(out, "            break;").map_err(fmt_err)?;
        writeln!(out, "        }}").map_err(fmt_err)?;
    }
    if let Some(c) = default_case {
        writeln!(out, "        default: {{").map_err(fmt_err)?;
        emit_union_branch_body(out, c, decode, endian, access, origin)?;
        writeln!(out, "            break;").map_err(fmt_err)?;
        writeln!(out, "        }}").map_err(fmt_err)?;
    } else {
        writeln!(out, "        default: break;").map_err(fmt_err)?;
    }
    writeln!(out, "        }}").map_err(fmt_err)?;
    Ok(())
}

/// Encode/decode one union branch's value. The branch value lives in the
/// union's `std::variant value_` keyed by the branch C++ type. `access` is the
/// union value expression and `origin` the alignment origin (see
/// [`emit_union_branch_switch_at`]).
fn emit_union_branch_body(
    out: &mut String,
    c: &Case,
    decode: bool,
    endian: &str,
    access: &str,
    origin: &str,
) -> Result<(), CppGenError> {
    let cpp_ty = type_for_declarator(&c.element.type_spec, &c.element.declarator)?;
    let ts = &c.element.type_spec;
    if decode {
        writeln!(out, "            {cpp_ty} zd_bv{{}};").map_err(fmt_err)?;
        emit_value_read(out, ts, "zd_bv =", origin, "            ", false)?;
        writeln!(out, "            {access}.value() = zd_bv;").map_err(fmt_err)?;
    } else {
        writeln!(
            out,
            "            const {cpp_ty}& zd_bv = std::get<{cpp_ty}>({access}.value());"
        )
        .map_err(fmt_err)?;
        emit_value_write(out, ts, "zd_bv", endian, origin, "            ")?;
    }
    Ok(())
}

fn emit_encode_fn(
    out: &mut String,
    cpp_fqn: &str,
    s: &StructDef,
    ext: Extensibility,
    be: bool,
    phase: TtsPhase,
) -> Result<(), CppGenError> {
    // Suffix for write helpers: write_le or write_be, write_string or write_string_be.
    let endian_suffix = if be { "be" } else { "le" };
    let beb = if be { "true" } else { "false" };

    // Decl phase (Bug G2): emit method signatures only, terminated by `;`.
    if phase == TtsPhase::Decl {
        if be {
            writeln!(
                out,
                "    static std::vector<uint8_t> encode_be(const {cpp_fqn}& zd_v);"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "    static std::vector<uint8_t> encode_be(const {cpp_fqn}& zd_v, \
                 ::dds::topic::xcdr2::XcdrVersion zd_repr);"
            )
            .map_err(fmt_err)?;
        } else {
            writeln!(
                out,
                "    static std::vector<uint8_t> encode(const {cpp_fqn}& zd_v);"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "    static std::vector<uint8_t> encode(const {cpp_fqn}& zd_v, \
                 ::dds::topic::xcdr2::XcdrVersion zd_repr);"
            )
            .map_err(fmt_err)?;
        }
        return Ok(());
    }

    // Def phase: out-of-line bodies (`inline ... topic_type_support<T>::...`).
    if be {
        // `encode_be(v)` keeps the XCDR2-BE default; `encode_be(v, repr)` is the
        // representation-aware BE encoder (XCDR1-BE = classic CDR, big-endian).
        writeln!(
            out,
            "inline std::vector<uint8_t> topic_type_support<{cpp_fqn}>::encode_be(const {cpp_fqn}& zd_v) {{"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "        return encode_be(zd_v, ::dds::topic::xcdr2::XcdrVersion::Xcdr2);"
        )
        .map_err(fmt_err)?;
        writeln!(out, "    }}").map_err(fmt_err)?;
        writeln!(
            out,
            "inline std::vector<uint8_t> topic_type_support<{cpp_fqn}>::encode_be(const {cpp_fqn}& zd_v, \
             ::dds::topic::xcdr2::XcdrVersion zd_repr) {{"
        )
        .map_err(fmt_err)?;
    } else {
        // XCDR2 default delegator + version-aware encode. XCDR2 caps
        // 8-byte primitive alignment to 4 (XTypes 1.3 §7.4.3.4.2)
        // — symmetric to `decode(.., XcdrVersion)`. XCDR2 is the
        // ZeroDDS system default (= dcps DEFAULT_OFFER [XCDR2], encap
        // 0x07/0x09/0x0b); for legacy XCDR1 call
        // `encode(zd_v, XcdrVersion::Xcdr1)` explicitly.
        writeln!(
            out,
            "inline std::vector<uint8_t> topic_type_support<{cpp_fqn}>::encode(const {cpp_fqn}& zd_v) {{"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "        return encode(zd_v, ::dds::topic::xcdr2::XcdrVersion::Xcdr2);"
        )
        .map_err(fmt_err)?;
        writeln!(out, "    }}").map_err(fmt_err)?;
        writeln!(
            out,
            "inline std::vector<uint8_t> topic_type_support<{cpp_fqn}>::encode(const {cpp_fqn}& zd_v, \
             ::dds::topic::xcdr2::XcdrVersion zd_repr) {{"
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "        std::vector<uint8_t> zd_out;").map_err(fmt_err)?;
    writeln!(out, "        (void)zd_v;").map_err(fmt_err)?;
    // Both LE (`encode`) and BE (`encode_be`) now carry the `zd_repr` param, so
    // the alignment cap (XCDR2 -> 4, XCDR1 -> 8) and the representation-aware
    // DHEADER/PL_CDR1 framing are identical apart from the byte order.
    writeln!(
        out,
        "        const size_t zd_max_align = ::dds::topic::xcdr2::xcdr_max_align(zd_repr);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        (void)zd_max_align;").map_err(fmt_err)?;

    match ext {
        Extensibility::Final => {
            // Plain-CDR2, no DHEADER, alignment relative to buffer start.
            // origin = 0.
            writeln!(out, "        const size_t zd_origin = 0;").map_err(fmt_err)?;
            writeln!(out, "        (void)zd_origin;").map_err(fmt_err)?;
            for m in &resolved_wire_members(s) {
                emit_plain_member_encode(out, m, endian_suffix, "zd_origin")?;
            }
        }
        Extensibility::Appendable => {
            // XCDR2: a DHEADER (4-byte body size) prefixes the members (origin
            // post-DHEADER, max_align 4). XCDR1 (classic CDR, either byte order):
            // NO DHEADER — plain positional members at origin = stream start with
            // max_align 8 (`zd_max_align` already reflects `zd_repr`). Identical
            // member emit; only the DHEADER framing differs.
            writeln!(
                out,
                "        const bool zd_x1 = (zd_repr == ::dds::topic::xcdr2::XcdrVersion::Xcdr1);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        size_t zd_dh = 0; (void)zd_dh;").map_err(fmt_err)?;
            writeln!(
                out,
                "        if (!zd_x1) {{ zd_dh = ::dds::topic::xcdr2::dheader_begin(zd_out); }}"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        const size_t zd_origin = zd_out.size();").map_err(fmt_err)?;
            writeln!(out, "        (void)zd_origin;").map_err(fmt_err)?;
            for m in &resolved_wire_members(s) {
                emit_plain_member_encode(out, m, endian_suffix, "zd_origin")?;
            }
            writeln!(
                out,
                "        if (!zd_x1) {{ ::dds::topic::xcdr2::dheader_end(zd_out, zd_dh, {beb}); }}"
            )
            .map_err(fmt_err)?;
        }
        Extensibility::Mutable => {
            // XCDR2: PL_CDR2 — outer DHEADER + a 32-bit EMHEADER per member.
            // XCDR1 (either byte order): PL_CDR1 — NO DHEADER, each member a
            // 16/32-bit PID parameter (UNPADDED length, body padded to 4),
            // terminated by the PID_LIST_END sentinel.
            writeln!(
                out,
                "        if (zd_repr == ::dds::topic::xcdr2::XcdrVersion::Xcdr1) {{"
            )
            .map_err(fmt_err)?;
            for (m, id) in resolved_wire_members(s).iter().zip(resolved_member_ids(s)) {
                emit_pl_cdr1_member_encode(out, m, id, endian_suffix)?;
            }
            writeln!(
                out,
                "            ::dds::topic::xcdr2::pl_cdr1_write_sentinel(zd_out, {beb});"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        }} else {{").map_err(fmt_err)?;
            writeln!(
                out,
                "        const auto zd_scope = ::dds::topic::xcdr2::mutable_begin(zd_out);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        const size_t zd_origin = zd_scope.origin;").map_err(fmt_err)?;
            writeln!(out, "        (void)zd_origin;").map_err(fmt_err)?;
            for (m, id) in resolved_wire_members(s).iter().zip(resolved_member_ids(s)) {
                emit_mutable_member_encode(out, m, endian_suffix, id)?;
            }
            writeln!(
                out,
                "        ::dds::topic::xcdr2::mutable_end(zd_out, zd_scope, {beb});"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        }}").map_err(fmt_err)?;
        }
    }

    writeln!(out, "        return zd_out;").map_err(fmt_err)?;
    writeln!(out, "    }}").map_err(fmt_err)?;
    Ok(())
}

/// Emit the XCDR array BODY (element bytes, WITHOUT any per-member EMHEADER/PID
/// framing) for one array declarator, reading from the C++ getter `access` and
/// aligning relative to `origin`. Shared by the @final/@appendable plain path
/// and the @mutable EMHEADER (XCDR2) / PL_CDR1 (XCDR1) paths (broad-audit P0-6).
/// Returns `false` for an array shape this backend does not yet emit (the caller
/// decides between a skip comment (plain) and a hard error (mutable)).
///
/// Layout (XTypes 1.3 §7.4.3.5): a primitive array (PARRAY) — 1-D or multi-dim —
/// is tight-packed with NO collection DHEADER; a 1-D leaf `string`/`wstring`
/// array likewise; an array of non-primitive elements (enum/struct/union, any
/// dims; string only for >=2-D) is one collection DHEADER wrapping the row-major
/// elements. `emit_value_write` picks the per-element form (final structs inline,
/// appendable/mutable structs + unions splice their own DHEADER-framed encode).
fn emit_array_body_encode(
    out: &mut String,
    type_spec: &TypeSpec,
    ndims: usize,
    access: &str,
    endian: &str,
    origin: &str,
    indent: &str,
) -> Result<bool, CppGenError> {
    let beb = if endian == "be" { "true" } else { "false" };
    let prim = matches!(type_spec, TypeSpec::Primitive(_));
    let leaf_1d = ndims == 1 && matches!(type_spec, TypeSpec::Primitive(_) | TypeSpec::String(_));
    if leaf_1d {
        writeln!(out, "{indent}for (const auto& zd_ae : {access}) {{").map_err(fmt_err)?;
        emit_value_write(out, type_spec, "zd_ae", endian, origin, indent)?;
        writeln!(out, "{indent}}}").map_err(fmt_err)?;
        Ok(true)
    } else if prim && ndims >= 2 {
        // Multi-dim primitive array = PARRAY (XTypes 1.3 §7.4.3.5 rule 8): NO
        // collection DHEADER, row-major elements tight-packed.
        let mut acc = access.to_string();
        let mut ind = indent.to_string();
        for d in 0..ndims {
            let lv = format!("zd_a{d}");
            writeln!(out, "{ind}for (const auto& {lv} : {acc}) {{").map_err(fmt_err)?;
            acc = lv;
            ind.push_str("    ");
        }
        emit_value_write(out, type_spec, &acc, endian, origin, &ind)?;
        for _ in 0..ndims {
            ind.truncate(ind.len() - 4);
            writeln!(out, "{ind}}}").map_err(fmt_err)?;
        }
        Ok(true)
    } else if matches!(type_spec, TypeSpec::Scoped(s) if scoped_is_enum(s) || scoped_struct(s).is_some() || scoped_union(s).is_some())
        || (matches!(type_spec, TypeSpec::String(_)) && ndims >= 2)
    {
        // Array of non-primitive elements: one collection DHEADER wrapping N
        // elements inline, row-major, NO count.
        writeln!(out, "{indent}{{").map_err(fmt_err)?;
        writeln!(
            out,
            "{indent}const auto zd_arr_dh = ::dds::topic::xcdr2::dheader_begin_r(zd_out, zd_repr);"
        )
        .map_err(fmt_err)?;
        let mut acc = access.to_string();
        let mut ind = indent.to_string();
        for d in 0..ndims {
            let lv = format!("zd_a{d}");
            writeln!(out, "{ind}for (const auto& {lv} : {acc}) {{").map_err(fmt_err)?;
            acc = lv;
            ind.push_str("    ");
        }
        emit_value_write(out, type_spec, &acc, endian, origin, &ind)?;
        for _ in 0..ndims {
            ind.truncate(ind.len() - 4);
            writeln!(out, "{ind}}}").map_err(fmt_err)?;
        }
        writeln!(
            out,
            "{indent}::dds::topic::xcdr2::dheader_end_r(zd_out, zd_arr_dh, {beb}, zd_repr);"
        )
        .map_err(fmt_err)?;
        writeln!(out, "{indent}}}").map_err(fmt_err)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Symmetric decode counterpart of [`emit_array_body_encode`]: read the XCDR
/// array BODY (no per-member framing) into `zd_v.<name>` via its getter/setter,
/// aligning relative to `origin`. Returns `false` for an unsupported shape.
fn emit_array_body_decode(
    out: &mut String,
    type_spec: &TypeSpec,
    ndims: usize,
    name: &str,
    origin: &str,
    indent: &str,
) -> Result<bool, CppGenError> {
    let inner = format!("{indent}    ");
    let prim = matches!(type_spec, TypeSpec::Primitive(_));
    let leaf_1d = ndims == 1 && matches!(type_spec, TypeSpec::Primitive(_) | TypeSpec::String(_));
    let prim_read_expr = || -> String {
        match type_spec {
            TypeSpec::Primitive(PrimitiveType::Boolean) => {
                "::dds::topic::xcdr2::read_bool(zd_buf, zd_pos, zd_len)".to_string()
            }
            TypeSpec::Primitive(PrimitiveType::Octet) => {
                "::dds::topic::xcdr2::read_u8(zd_buf, zd_pos, zd_len)".to_string()
            }
            TypeSpec::Primitive(p) => format!(
                "::dds::topic::xcdr2::read_le_origin<{}>(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be)",
                primitive_to_cpp(*p)
            ),
            TypeSpec::String(s) if s.wide => format!(
                "::dds::topic::xcdr2::read_wstring_origin(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be)"
            ),
            _ => format!(
                "::dds::topic::xcdr2::read_string_origin(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be)"
            ),
        }
    };
    if leaf_1d {
        let read_expr = prim_read_expr();
        writeln!(out, "{indent}{{").map_err(fmt_err)?;
        writeln!(out, "{inner}auto zd_arr = zd_v.{name}();").map_err(fmt_err)?;
        writeln!(
            out,
            "{inner}for (auto& zd_ae : zd_arr) {{ zd_ae = {read_expr}; }}"
        )
        .map_err(fmt_err)?;
        writeln!(out, "{inner}zd_v.{name}(zd_arr);").map_err(fmt_err)?;
        writeln!(out, "{indent}}}").map_err(fmt_err)?;
        Ok(true)
    } else if prim && ndims >= 2 {
        let read_expr = prim_read_expr();
        writeln!(out, "{indent}{{").map_err(fmt_err)?;
        writeln!(out, "{inner}auto zd_arr = zd_v.{name}();").map_err(fmt_err)?;
        let mut acc = String::from("zd_arr");
        let mut ind = inner.clone();
        for d in 0..ndims {
            let lv = format!("zd_a{d}");
            writeln!(out, "{ind}for (auto& {lv} : {acc}) {{").map_err(fmt_err)?;
            acc = lv;
            ind.push_str("    ");
        }
        writeln!(out, "{ind}{acc} = {read_expr};").map_err(fmt_err)?;
        for _ in 0..ndims {
            ind.truncate(ind.len() - 4);
            writeln!(out, "{ind}}}").map_err(fmt_err)?;
        }
        writeln!(out, "{inner}zd_v.{name}(zd_arr);").map_err(fmt_err)?;
        writeln!(out, "{indent}}}").map_err(fmt_err)?;
        Ok(true)
    } else if matches!(type_spec, TypeSpec::Scoped(s) if scoped_is_enum(s) || scoped_struct(s).is_some() || scoped_union(s).is_some())
        || (matches!(type_spec, TypeSpec::String(_)) && ndims >= 2)
    {
        writeln!(out, "{indent}{{").map_err(fmt_err)?;
        writeln!(out, "{indent}const auto zd_arr_dh = ::dds::topic::xcdr2::dheader_read_r(zd_buf, zd_pos, zd_len, zd_be, zd_repr); (void)zd_arr_dh;").map_err(fmt_err)?;
        writeln!(out, "{indent}auto zd_arr = zd_v.{name}();").map_err(fmt_err)?;
        let mut acc = String::from("zd_arr");
        let mut ind = indent.to_string();
        for d in 0..ndims {
            let lv = format!("zd_a{d}");
            writeln!(out, "{ind}for (auto& {lv} : {acc}) {{").map_err(fmt_err)?;
            acc = lv;
            ind.push_str("    ");
        }
        emit_value_read(out, type_spec, &format!("{acc} ="), origin, &ind, false)?;
        for _ in 0..ndims {
            ind.truncate(ind.len() - 4);
            writeln!(out, "{ind}}}").map_err(fmt_err)?;
        }
        writeln!(out, "{indent}zd_v.{name}(zd_arr);").map_err(fmt_err)?;
        writeln!(out, "{indent}}}").map_err(fmt_err)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Emit Plain-CDR2 (LE/BE) encoding for one member at the current
/// position; alignment relative to `origin`.
fn emit_plain_member_encode(
    out: &mut String,
    m: &Member,
    endian: &str,
    origin: &str,
) -> Result<(), CppGenError> {
    let m = &normalize_member(m);
    let is_optional = has_optional_annotation(&m.annotations);
    let shared = has_shared_annotation(&m.annotations);
    for decl in &m.declarators {
        // broad-audit P0-7: reject the unsupported @shared shapes loudly instead
        // of silently skipping the member (data loss).
        reject_unsupported_shared(m, decl)?;
        let name = escape_cpp_ident(&decl.name().text);
        // Fixed array member (any dims): emit the shared array body — 1-D leaf
        // primitives/strings tight-packed, multi-dim primitive PARRAY tight-
        // packed, non-primitive elements DHEADER-wrapped (XTypes 1.3 §7.4.3.5).
        if let Declarator::Array(arr) = decl {
            if !emit_array_body_encode(
                out,
                &m.type_spec,
                arr.sizes.len(),
                &format!("zd_v.{name}()"),
                endian,
                origin,
                "        ",
            )? {
                writeln!(
                    out,
                    "        // xcdr2: array member '{name}' (multi-dim string-1D-only / unsupported elem) not supported (skip)"
                )
                .map_err(fmt_err)?;
            }
            continue;
        }
        if !typespec_supported(&m.type_spec) {
            writeln!(
                out,
                "        // xcdr2: member '{name}' not supported (nested/enum/map; skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        if is_optional {
            // Final/appendable: 1-byte present-flag, then the value if present.
            writeln!(out, "        if (zd_v.{name}().has_value()) {{").map_err(fmt_err)?;
            writeln!(out, "            zd_out.push_back(uint8_t{{1}});").map_err(fmt_err)?;
            // broad-audit P0-7: `@shared @optional` — the present value is the
            // pointee of the inner shared_ptr (`*(*optional)`), serialized by value.
            if shared {
                let rf = emit_shared_encode_ref(
                    out,
                    &m.type_spec,
                    &format!("(*zd_v.{name}())"),
                    "            ",
                )?;
                emit_value_write(out, &m.type_spec, &rf, endian, origin, "        ")?;
            } else {
                emit_value_write(
                    out,
                    &m.type_spec,
                    &format!("(*zd_v.{name}())"),
                    endian,
                    origin,
                    "        ",
                )?;
            }
            writeln!(out, "        }} else {{").map_err(fmt_err)?;
            writeln!(out, "            zd_out.push_back(uint8_t{{0}});").map_err(fmt_err)?;
            writeln!(out, "        }}").map_err(fmt_err)?;
        } else if shared {
            // broad-audit P0-7: @shared member — serialize the referenced value BY
            // VALUE (deref the shared_ptr), byte-identical to the same non-@shared
            // member (XTypes 1.3 §7.3.1.2.1.9).
            let rf =
                emit_shared_encode_ref(out, &m.type_spec, &format!("zd_v.{name}()"), "        ")?;
            emit_value_write(out, &m.type_spec, &rf, endian, origin, "    ")?;
        } else {
            emit_value_write(
                out,
                &m.type_spec,
                &format!("zd_v.{name}()"),
                endian,
                origin,
                "    ",
            )?;
        }
    }
    Ok(())
}

/// Emit a single value write at `access` using LE or BE convention.
///
/// zerodds-lint: recursion-depth 64 (nested type emit; bounded by IDL nesting)
fn emit_value_write(
    out: &mut String,
    ts: &TypeSpec,
    access: &str,
    endian: &str,
    origin: &str,
    indent: &str,
) -> Result<(), CppGenError> {
    let pre = format!("{indent}    ");
    let beb = if endian == "be" { "true" } else { "false" };
    match ts {
        TypeSpec::Primitive(PrimitiveType::Boolean) => {
            writeln!(
                out,
                "{pre}::dds::topic::xcdr2::write_bool(zd_out, {access});"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(PrimitiveType::Octet) => {
            writeln!(out, "{pre}::dds::topic::xcdr2::write_u8(zd_out, {access});")
                .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(p) => {
            let cpp_ty = primitive_to_cpp(*p);
            if endian == "be" {
                // BE: representation-aware too (XCDR2 caps 8-byte align at 4).
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_be_origin<{cpp_ty}>(zd_out, {origin}, {access}, zd_max_align);"
                )
                .map_err(fmt_err)?;
            } else {
                // LE: representation-aware (XCDR2 deckelt 8-Byte-Align auf 4).
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_le_origin<{cpp_ty}>(zd_out, {origin}, {access}, zd_max_align);"
                )
                .map_err(fmt_err)?;
            }
        }
        TypeSpec::String(s) if !s.wide => {
            // Bounded narrow `string<N>` (DDS-XTypes §7.4.3): byte-length check
            // (std::string::size = bytes = CDR wire length).
            if let Some(b) = &s.bound {
                let bv = bound_to_cpp(b);
                writeln!(
                    out,
                    "{pre}if ({access}.size() > {bv}) throw std::length_error(\"bounded string length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            if endian == "be" {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_string_be(zd_out, {access});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_string_origin(zd_out, {origin}, {access}, zd_max_align);"
                )
                .map_err(fmt_err)?;
            }
        }
        TypeSpec::String(s) if s.wide => {
            // Bounded `wstring<N>` (DDS-XTypes §7.4.3): bound is in wide chars
            // (std::wstring::size). Wire = UTF-16 (conformance §9.1).
            if let Some(b) = &s.bound {
                let bv = bound_to_cpp(b);
                writeln!(
                    out,
                    "{pre}if ({access}.size() > {bv}) throw std::length_error(\"bounded wstring length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            if endian == "be" {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_wstring_be(zd_out, {access});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_wstring_origin(zd_out, {origin}, {access}, zd_max_align);"
                )
                .map_err(fmt_err)?;
            }
        }
        TypeSpec::Sequence(seq) => {
            // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = encode
            // error. The encode returns a vector (no Result channel), so this
            // throws — strict vendors (OpenDDS) reject on the wire likewise.
            if let Some(b) = &seq.bound {
                let bv = bound_to_cpp(b);
                writeln!(
                    out,
                    "{pre}if ({access}.size() > {bv}) throw std::length_error(\"bounded sequence length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            if matches!(&*seq.elem, TypeSpec::Primitive(PrimitiveType::Octet)) {
                // sequence<octet>: u32 length + raw byte block, no per-byte loop.
                if endian == "be" {
                    writeln!(out, "{pre}::dds::topic::xcdr2::write_be<uint32_t>(zd_out, static_cast<uint32_t>({access}.size()));").map_err(fmt_err)?;
                } else {
                    writeln!(out, "{pre}::dds::topic::xcdr2::write_le_origin<uint32_t>(zd_out, {origin}, static_cast<uint32_t>({access}.size()), zd_max_align);").map_err(fmt_err)?;
                }
                writeln!(
                    out,
                    "{pre}zd_out.insert(zd_out.end(), {access}.begin(), {access}.end());"
                )
                .map_err(fmt_err)?;
                return Ok(());
            }
            // XCDR2 §7.4.3.5: sequences with NON-primitive elements
            // (string, struct, …) get a DHEADER (uint32 = byte length of
            // [count + elements]) prepended; primitives do not.
            // Cyclone-DDS-verified (V-5 without, V-6 with).
            let seq_non_primitive = !matches!(&*seq.elem, TypeSpec::Primitive(_));
            if seq_non_primitive {
                writeln!(out, "{pre}{{").map_err(fmt_err)?;
                // The sequence DHEADER is a uint32 -> 4-align to origin before it
                // (same class of bug as the map DHEADER: a non-primitive sequence
                // after a sub-4-byte member would otherwise land it unaligned).
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::pad_to_from_origin(zd_out, {origin}, 4);"
                )
                .map_err(fmt_err)?;
                writeln!(
                    out,
                    "{pre}const auto zd_seq_dh = ::dds::topic::xcdr2::dheader_begin_r(zd_out, zd_repr);"
                )
                .map_err(fmt_err)?;
            }
            let count_call = if endian == "be" {
                format!(
                    "{pre}::dds::topic::xcdr2::write_be<uint32_t>(zd_out, static_cast<uint32_t>({access}.size()));"
                )
            } else {
                format!(
                    "{pre}::dds::topic::xcdr2::write_le_origin<uint32_t>(zd_out, {origin}, static_cast<uint32_t>({access}.size()), zd_max_align);"
                )
            };
            writeln!(out, "{count_call}").map_err(fmt_err)?;
            writeln!(out, "{pre}for (const auto& zd_e : {access}) {{").map_err(fmt_err)?;
            let elem_indent = format!("{pre}    ");
            match &*seq.elem {
                TypeSpec::Primitive(PrimitiveType::Boolean) => {
                    writeln!(
                        out,
                        "{elem_indent}::dds::topic::xcdr2::write_bool(zd_out, zd_e);"
                    )
                    .map_err(fmt_err)?;
                }
                TypeSpec::Primitive(PrimitiveType::Octet) => {
                    writeln!(
                        out,
                        "{elem_indent}::dds::topic::xcdr2::write_u8(zd_out, zd_e);"
                    )
                    .map_err(fmt_err)?;
                }
                TypeSpec::Primitive(p) => {
                    let cpp_ty = primitive_to_cpp(*p);
                    if endian == "be" {
                        writeln!(
                            out,
                            "{elem_indent}::dds::topic::xcdr2::write_be<{cpp_ty}>(zd_out, zd_e);"
                        )
                        .map_err(fmt_err)?;
                    } else {
                        writeln!(
                            out,
                            "{elem_indent}::dds::topic::xcdr2::write_le_origin<{cpp_ty}>(zd_out, {origin}, zd_e, zd_max_align);"
                        )
                        .map_err(fmt_err)?;
                    }
                }
                TypeSpec::String(s) if !s.wide => {
                    if endian == "be" {
                        writeln!(
                            out,
                            "{elem_indent}::dds::topic::xcdr2::write_string_be(zd_out, zd_e);"
                        )
                        .map_err(fmt_err)?;
                    } else {
                        writeln!(
                            out,
                            "{elem_indent}::dds::topic::xcdr2::write_string_origin(zd_out, {origin}, zd_e, zd_max_align);"
                        )
                        .map_err(fmt_err)?;
                    }
                }
                // wide string (wstring): recurse for the BOM/octet-length wire form.
                TypeSpec::String(_) => {
                    emit_value_write(out, &seq.elem, "zd_e", endian, origin, &elem_indent)?;
                }
                // enum (-> int32) and nested struct of ANY extensibility: recurse
                // through emit_value_write, identical to member-level encoding —
                // @final inlines (no DHEADER), @appendable/@mutable pad-to-4 +
                // splice the element's own [DHEADER+body] (XTypes §7.4.3.5).
                TypeSpec::Scoped(sc) if scoped_is_enum(sc) || scoped_struct(sc).is_some() => {
                    emit_value_write(out, &seq.elem, "zd_e", endian, origin, &elem_indent)?;
                }
                // union element (sequence<union>, Bug R3): recurse — emit_value_write
                // splices each element's own DHEADER-framed TypeSupport encode.
                TypeSpec::Scoped(sc) if scoped_union(sc).is_some() => {
                    emit_value_write(out, &seq.elem, "zd_e", endian, origin, &elem_indent)?;
                }
                // nested sequence (sequence<sequence<...>>): recurse — the inner
                // sequence emits its own DHEADER (XTypes §7.4.3.5).
                TypeSpec::Sequence(_) => {
                    emit_value_write(out, &seq.elem, "zd_e", endian, origin, &elem_indent)?;
                }
                // map element (sequence<map<K,V>>): recurse — the map emits its
                // own DHEADER.
                TypeSpec::Map(_) => {
                    emit_value_write(out, &seq.elem, "zd_e", endian, origin, &elem_indent)?;
                }
                _ => {
                    writeln!(
                        out,
                        "{elem_indent}// xcdr2: nested sequence-element not supported"
                    )
                    .map_err(fmt_err)?;
                }
            }
            writeln!(out, "{pre}}}").map_err(fmt_err)?;
            if seq_non_primitive {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::dheader_end_r(zd_out, zd_seq_dh, {beb}, zd_repr);"
                )
                .map_err(fmt_err)?;
                writeln!(out, "{pre}}}").map_err(fmt_err)?;
            }
        }
        // map<K,V> member (XTypes §7.4.4.6): a non-primitive collection -> DHEADER
        // (uint32 byte-len of [count + entries]); uint32 count; then each entry as
        // key.encode + value.encode in key-sorted order. std::map iterates in
        // ascending key order, matching the Rust BTreeMap reference encoder
        // (crates/cdr/src/composite.rs §7.4.4.6) byte-for-byte.
        TypeSpec::Map(m) => {
            if let Some(b) = &m.bound {
                let bv = bound_to_cpp(b);
                writeln!(
                    out,
                    "{pre}if ({access}.size() > {bv}) throw std::length_error(\"bounded map length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{pre}{{").map_err(fmt_err)?;
            // The map DHEADER is a uint32 and must be 4-aligned relative to the
            // aggregate origin BEFORE it is written — otherwise a map preceded by
            // a sub-4-byte member (e.g. a @bit_bound(16) enum) lands the DHEADER
            // at an unaligned offset. (Bug surfaced by the MapEnum cross-PSM probe;
            // rust/Cyclone pad here.)
            writeln!(
                out,
                "{pre}::dds::topic::xcdr2::pad_to_from_origin(zd_out, {origin}, 4);"
            )
            .map_err(fmt_err)?;
            let map_dh = !map_pair_is_primitive(&m.key, &m.value);
            if map_dh {
                writeln!(
                    out,
                    "{pre}const auto zd_map_dh = ::dds::topic::xcdr2::dheader_begin_r(zd_out, zd_repr);"
                )
                .map_err(fmt_err)?;
            }
            if endian == "be" {
                writeln!(out, "{pre}::dds::topic::xcdr2::write_be<uint32_t>(zd_out, static_cast<uint32_t>({access}.size()));").map_err(fmt_err)?;
            } else {
                writeln!(out, "{pre}::dds::topic::xcdr2::write_le_origin<uint32_t>(zd_out, {origin}, static_cast<uint32_t>({access}.size()), zd_max_align);").map_err(fmt_err)?;
            }
            writeln!(out, "{pre}for (const auto& zd_kv : {access}) {{").map_err(fmt_err)?;
            let kv_indent = format!("{pre}    ");
            emit_value_write(out, &m.key, "zd_kv.first", endian, origin, &kv_indent)?;
            emit_value_write(out, &m.value, "zd_kv.second", endian, origin, &kv_indent)?;
            writeln!(out, "{pre}}}").map_err(fmt_err)?;
            if map_dh {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::dheader_end_r(zd_out, zd_map_dh, {beb}, zd_repr);"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{pre}}}").map_err(fmt_err)?;
        }
        // bitmask / bitset member: serialize the holder integer at the holder
        // width (cdr-core reference: bitmask = #values, bitset = total bits).
        // A bitmask is `enum class : uintN` (cast to the holder), a bitset is
        // `struct{ uint64_t value; }` (use `.value`, narrowed). XTypes §7.4.x.
        TypeSpec::Scoped(s) if scoped_bitholder(s).is_some() => {
            let bytes = scoped_bitholder(s).unwrap_or(1);
            let holder = holder_uint_for_bytes(bytes);
            let is_bitset = scoped_is_bitset(s);
            let raw = if is_bitset {
                format!("static_cast<{holder}>({access}.value)")
            } else {
                format!("static_cast<{holder}>({access})")
            };
            if endian == "be" {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_be_origin<{holder}>(zd_out, {origin}, {raw});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_le_origin<{holder}>(zd_out, {origin}, {raw}, zd_max_align);"
                )
                .map_err(fmt_err)?;
            }
        }
        // enum member: encode as its int32 underlying type (Spec §7.4.1.4.2).
        TypeSpec::Scoped(s) if scoped_is_enum(s) => {
            // T2: the enum holder narrows to its @bit_bound width (§7.4.5.1).
            let ec = enum_wire_ctype(scoped_enum_bytes(s));
            if endian == "be" {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_be<{ec}>(zd_out, static_cast<{ec}>({access}));"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_le_origin<{ec}>(zd_out, {origin}, static_cast<{ec}>({access}), zd_max_align);"
                )
                .map_err(fmt_err)?;
            }
        }
        // nested struct member. @final: recurse, encoding each sub-member inline
        // (Plain-CDR2, no DHEADER, Spec §7.4.3.4.1). @appendable/@mutable: splice
        // the nested type's own encoding — its DHEADER forces 4-alignment, so a
        // 4-aligned splice point is byte-identical to standalone under XCDR2.
        TypeSpec::Scoped(sc) if scoped_struct(sc).is_some() => {
            let Some((def, ext)) = scoped_struct(sc) else {
                return Ok(());
            };
            // Resolve the nested type's C++ name in the OUTER scope, then switch
            // to the nested type's own module for its inline (@final) member loop
            // (P0-2). Appendable/mutable splice via the precomputed name.
            let cpp = scoped_to_cpp(sc);
            let _sg = enter_ref_scope(sc);
            match ext {
                Extensibility::Final => {
                    for sm in &def.members {
                        let sm_name = escape_cpp_ident(&sm.declarators[0].name().text);
                        emit_value_write(
                            out,
                            &sm.type_spec,
                            &format!("{access}.{sm_name}()"),
                            endian,
                            origin,
                            &pre,
                        )?;
                    }
                }
                Extensibility::Appendable | Extensibility::Mutable => {
                    let id = next_nest_id();
                    writeln!(out, "{pre}{{").map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{pre}    ::dds::topic::xcdr2::pad_to_from_origin(zd_out, {origin}, 4);"
                    )
                    .map_err(fmt_err)?;
                    if endian == "be" {
                        writeln!(
                            out,
                            "{pre}    auto zd_nsb{id} = ::dds::topic::topic_type_support<{cpp}>::encode_be({access}, zd_repr);"
                        )
                        .map_err(fmt_err)?;
                    } else {
                        writeln!(
                            out,
                            "{pre}    auto zd_nsb{id} = ::dds::topic::topic_type_support<{cpp}>::encode({access}, zd_repr);"
                        )
                        .map_err(fmt_err)?;
                    }
                    writeln!(
                        out,
                        "{pre}    zd_out.insert(zd_out.end(), zd_nsb{id}.begin(), zd_nsb{id}.end());"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "{pre}}}").map_err(fmt_err)?;
                }
            }
        }
        // Bug R3: union member — splice via its own DHEADER-framed
        // `topic_type_support<Union>::encode` (4-aligned, identical to an
        // appendable nested struct), so the active branch reaches the wire.
        TypeSpec::Scoped(sc) if scoped_union(sc).is_some() => {
            let Some(u) = scoped_union(sc) else {
                return Ok(());
            };
            // C++ name resolved in the OUTER scope; the @final inline body
            // resolves the union's own cases in the union's module scope (P0-2).
            let cpp = scoped_to_cpp(sc);
            let _sg = enter_ref_scope(sc);
            match union_extensibility(&u.annotations) {
                // @final union (rule (26) FUNION_TYPE): inline disc + selected
                // member, NO DHEADER (XTypes 1.3 §7.4.3.4.1). Identical to the
                // union's own standalone serializer body, but written into the
                // outer buffer at the outer origin so 8-byte members align to
                // min(8,4)=4 relative to the top-level DHEADER.
                Extensibility::Final => {
                    let disc_ts = switch_type_spec(&u.switch_type);
                    let disc_cpp = switch_type_to_cpp(&u.switch_type)?;
                    emit_value_write(
                        out,
                        &disc_ts,
                        &format!("{access}._d()"),
                        endian,
                        origin,
                        &pre,
                    )?;
                    emit_union_branch_switch_at(
                        out, &u, &disc_cpp, /*decode=*/ false, endian, access, origin,
                    )?;
                }
                // @appendable/@mutable union: splice its own DHEADER-framed
                // serializer (4-aligned splice point preserves member alignment
                // under XCDR2 max_align=4, §7.4.3.4.2).
                Extensibility::Appendable | Extensibility::Mutable => {
                    let id = next_nest_id();
                    writeln!(out, "{pre}{{").map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{pre}    ::dds::topic::xcdr2::pad_to_from_origin(zd_out, {origin}, 4);"
                    )
                    .map_err(fmt_err)?;
                    if endian == "be" {
                        writeln!(
                            out,
                            "{pre}    auto zd_nub{id} = ::dds::topic::topic_type_support<{cpp}>::encode_be({access}, zd_repr);"
                        )
                        .map_err(fmt_err)?;
                    } else {
                        writeln!(
                            out,
                            "{pre}    auto zd_nub{id} = ::dds::topic::topic_type_support<{cpp}>::encode({access}, zd_repr);"
                        )
                        .map_err(fmt_err)?;
                    }
                    writeln!(
                        out,
                        "{pre}    zd_out.insert(zd_out.end(), zd_nub{id}.begin(), zd_nub{id}.end());"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "{pre}}}").map_err(fmt_err)?;
                }
            }
        }
        TypeSpec::Fixed(_) => {
            // fixed<P,S>: raw BCD octets (CORBA §9.3.2.7), alignment 1, no
            // length prefix, endian-independent. `::dds::core::Fixed<P,S>`
            // stores exactly (P+2)/2 octets — splice them verbatim.
            writeln!(
                out,
                "{pre}{{ const auto& zd_bcd = {access}.bcd_bytes(); zd_out.insert(zd_out.end(), zd_bcd.begin(), zd_bcd.end()); }}"
            )
            .map_err(fmt_err)?;
        }
        _ => {
            writeln!(out, "{pre}// xcdr2: member type not supported (skip)").map_err(fmt_err)?;
        }
    }
    Ok(())
}

/// Extensibility of a union from its annotations. Mirrors
/// [`struct_extensibility`]: un-annotated → FINAL (the canonical zerodds /
/// idl-rust default), so a nested un-annotated union recurses inline with no
/// DHEADER (rule (26)).
fn union_extensibility(anns: &[Annotation]) -> Extensibility {
    struct_extensibility(anns)
}

/// Emit Mutable-EMHEADER + body for one member. `base_id` is the member's wire
/// id resolved centrally (broad-audit P0-3: `@id(N)` / `@hashid` / struct-level
/// `@autoid(HASH)`, else the sequential positional fallback — see
/// [`resolved_member_ids`]). Multiple declarators of one member step up from
/// `base_id`. Previously this backend used its own positional counter that
/// ignored `@autoid(HASH)` / `@hashid`, diverging from rust + the TypeObject.
fn emit_mutable_member_encode(
    out: &mut String,
    m: &Member,
    endian: &str,
    base_id: u32,
) -> Result<(), CppGenError> {
    let m = &normalize_member(m);
    let is_optional = has_optional_annotation(&m.annotations);
    let shared = has_shared_annotation(&m.annotations);
    let must_understand = has_named_annotation(&m.annotations, "must_understand");
    let mu_lit = if must_understand { "true" } else { "false" };
    let beb = if endian == "be" { "true" } else { "false" };

    for (idx, decl) in m.declarators.iter().enumerate() {
        // broad-audit P0-7: reject unsupported @shared shapes loudly.
        reject_unsupported_shared(m, decl)?;
        let name = escape_cpp_ident(&decl.name().text);
        // Central-resolved id; further declarators of the same member step up
        // from it. encode + decode compute the same id from the same wire-member
        // list + index, so they stay in lockstep across the skip paths below.
        let this_id = base_id + idx as u32;
        // broad-audit P0-6: an array member in an @mutable struct. XTypes 1.3
        // §7.4.3.4.2 — an array member is framed with the universal LC=4 EMHEADER
        // (a separately-serialized NEXTINT = total array-body byte length), the
        // same choice the Rust reference makes (`mutable_length_code_for` returns
        // None → Lc4 for `Declarator::Array`). The body is the identical PLAIN
        // array encoding (tight-packed primitives / DHEADER-wrapped non-primitive
        // elements), so the member reaches the wire — the previous silent skip
        // dropped it entirely.
        if let Declarator::Array(arr) = decl {
            let id_expr = format!("0x{this_id:x}u");
            writeln!(
                out,
                "        {{ const auto zd_sub = ::dds::topic::xcdr2::emheader_nextint_begin(zd_out, zd_origin, {id_expr}, {mu_lit}, {beb});"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "            {{ const auto zd_body_origin = zd_sub.body_start; (void)zd_body_origin;"
            )
            .map_err(fmt_err)?;
            let ok = emit_array_body_encode(
                out,
                &m.type_spec,
                arr.sizes.len(),
                &format!("zd_v.{name}()"),
                endian,
                "zd_body_origin",
                "            ",
            )?;
            if !ok {
                return Err(CppGenError::UnsupportedConstruct {
                    construct: "array member in @mutable struct".into(),
                    context: Some(name),
                });
            }
            writeln!(out, "            }}").map_err(fmt_err)?;
            writeln!(
                out,
                "            ::dds::topic::xcdr2::emheader_nextint_end(zd_out, zd_sub, {beb}); }}"
            )
            .map_err(fmt_err)?;
            continue;
        }
        if !matches!(decl, Declarator::Simple(_)) {
            writeln!(
                out,
                "        // xcdr2: non-array/non-simple member '{name}' not supported (skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        if !typespec_supported(&m.type_spec) {
            writeln!(
                out,
                "        // xcdr2: member '{name}' not supported (skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        let id_expr = format!("0x{this_id:x}u");
        if is_optional {
            // Mutable + optional: skip EMHEADER if absent.
            writeln!(out, "        if (zd_v.{name}().has_value()) {{").map_err(fmt_err)?;
            // broad-audit P0-7: `@shared @optional` — the present value is the
            // pointee of the inner shared_ptr, serialized by value.
            if shared {
                let rf = emit_shared_encode_ref(
                    out,
                    &m.type_spec,
                    &format!("(*zd_v.{name}())"),
                    "            ",
                )?;
                emit_mutable_value_emit(
                    out,
                    &m.type_spec,
                    &rf,
                    &id_expr,
                    mu_lit,
                    endian,
                    "            ",
                )?;
            } else {
                emit_mutable_value_emit(
                    out,
                    &m.type_spec,
                    &format!("(*zd_v.{name}())"),
                    &id_expr,
                    mu_lit,
                    endian,
                    "            ",
                )?;
            }
            writeln!(out, "        }}").map_err(fmt_err)?;
        } else if shared {
            // broad-audit P0-7: @shared member — serialize the referenced value BY
            // VALUE (deref the shared_ptr) with the identical EMHEADER framing as a
            // non-@shared member (XTypes 1.3 §7.3.1.2.1.9).
            let rf =
                emit_shared_encode_ref(out, &m.type_spec, &format!("zd_v.{name}()"), "        ")?;
            emit_mutable_value_emit(out, &m.type_spec, &rf, &id_expr, mu_lit, endian, "        ")?;
        } else {
            emit_mutable_value_emit(
                out,
                &m.type_spec,
                &format!("zd_v.{name}()"),
                &id_expr,
                mu_lit,
                endian,
                "        ",
            )?;
        }
    }
    Ok(())
}

/// Emits a single `@mutable` member under XCDR1 / PL_CDR1: a PID-framed
/// parameter whose body is the member's plain (positional) field encoding,
/// origin-relative to the parameter body start (max_align 8 under XCDR1).
/// Mirrors `emit_mutable_member_encode`'s id assignment so the parameter ids
/// equal the EMHEADER ids of the XCDR2 path. `base_id` is the central-resolved
/// member id (P0-3). LE only — PL_CDR1 IS the XCDR1 framing; `encode_be` is
/// always XCDR2.
fn emit_pl_cdr1_member_encode(
    out: &mut String,
    m: &Member,
    base_id: u32,
    endian: &str,
) -> Result<(), CppGenError> {
    let beb = if endian == "be" { "true" } else { "false" };
    let m = &normalize_member(m);
    let is_optional = has_optional_annotation(&m.annotations);
    let shared = has_shared_annotation(&m.annotations);
    for (idx, decl) in m.declarators.iter().enumerate() {
        // broad-audit P0-7: reject unsupported @shared shapes loudly.
        reject_unsupported_shared(m, decl)?;
        let name = escape_cpp_ident(&decl.name().text);
        // Central-resolved id (lockstep with the XCDR2 EMHEADER ids); further
        // declarators of the same member step up from it.
        let this_id = base_id + idx as u32;
        // broad-audit P0-6: an array member in an @mutable struct under XCDR1 /
        // PL_CDR1. The member is a PID-framed parameter whose body is the plain
        // array encoding (origin = the parameter body start); `pl_cdr1_member_end`
        // records the unpadded body length. Symmetric to the XCDR2 LC=4 path — the
        // member is no longer silently dropped.
        if let Declarator::Array(arr) = decl {
            writeln!(out, "            {{").map_err(fmt_err)?;
            writeln!(
                out,
                "                auto zd_pm = ::dds::topic::xcdr2::pl_cdr1_member_begin(zd_out, 0x{this_id:x}u, {beb});"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                const size_t zd_origin = zd_pm.body_start; (void)zd_origin;"
            )
            .map_err(fmt_err)?;
            let ok = emit_array_body_encode(
                out,
                &m.type_spec,
                arr.sizes.len(),
                &format!("zd_v.{name}()"),
                endian,
                "zd_origin",
                "                ",
            )?;
            if !ok {
                return Err(CppGenError::UnsupportedConstruct {
                    construct: "array member in @mutable struct".into(),
                    context: Some(name),
                });
            }
            writeln!(
                out,
                "                ::dds::topic::xcdr2::pl_cdr1_member_end(zd_out, zd_pm, {beb});"
            )
            .map_err(fmt_err)?;
            writeln!(out, "            }}").map_err(fmt_err)?;
            continue;
        }
        if !matches!(decl, Declarator::Simple(_)) {
            writeln!(
                out,
                "            // xcdr1: non-array/non-simple member '{name}' not supported (skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        if !typespec_supported(&m.type_spec) {
            writeln!(
                out,
                "            // xcdr1: member '{name}' not supported (skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        let access = if is_optional {
            format!("(*zd_v.{name}())")
        } else {
            format!("zd_v.{name}()")
        };
        let ind = if is_optional {
            "                "
        } else {
            "            "
        };
        if is_optional {
            // PL_CDR1 optional: present -> emit the parameter; absent -> omit it
            // entirely (no present-flag; absence = the parameter is not in the
            // list), exactly like the XCDR2 EMHEADER-skip.
            writeln!(out, "            if (zd_v.{name}().has_value()) {{").map_err(fmt_err)?;
        }
        writeln!(out, "{ind}{{").map_err(fmt_err)?;
        writeln!(
            out,
            "{ind}    auto zd_pm = ::dds::topic::xcdr2::pl_cdr1_member_begin(zd_out, 0x{this_id:x}u, {beb});"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "{ind}    const size_t zd_origin = zd_pm.body_start; (void)zd_origin;"
        )
        .map_err(fmt_err)?;
        // broad-audit P0-7: @shared member — the PID-framed parameter body is the
        // referenced value serialized BY VALUE (deref the shared_ptr; for
        // `@shared @optional` `access` is already the `*optional` shared_ptr), byte-
        // identical to the same non-@shared member (XTypes 1.3 §7.3.1.2.1.9).
        let access = if shared {
            emit_shared_encode_ref(out, &m.type_spec, &access, &format!("{ind}    "))?
        } else {
            access
        };
        emit_value_write(out, &m.type_spec, &access, endian, "zd_origin", ind)?;
        writeln!(
            out,
            "{ind}    ::dds::topic::xcdr2::pl_cdr1_member_end(zd_out, zd_pm, {beb});"
        )
        .map_err(fmt_err)?;
        writeln!(out, "{ind}}}").map_err(fmt_err)?;
        if is_optional {
            writeln!(out, "            }}").map_err(fmt_err)?;
        }
    }
    Ok(())
}

fn emit_mutable_value_emit(
    out: &mut String,
    ts: &TypeSpec,
    access: &str,
    id_expr: &str,
    mu_lit: &str,
    endian: &str,
    indent: &str,
) -> Result<(), CppGenError> {
    let beb = if endian == "be" { "true" } else { "false" };
    match ts {
        TypeSpec::Primitive(PrimitiveType::Boolean) => {
            writeln!(
                out,
                "{indent}::dds::topic::xcdr2::emheader_u8(zd_out, zd_origin, {id_expr}, {mu_lit}, static_cast<uint8_t>({access} ? 1 : 0), {beb});"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(PrimitiveType::Octet) => {
            writeln!(
                out,
                "{indent}::dds::topic::xcdr2::emheader_u8(zd_out, zd_origin, {id_expr}, {mu_lit}, {access}, {beb});"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(p) => {
            let cpp_ty = primitive_to_cpp(*p);
            let size = primitive_size(*p);
            // XTypes 1.3 §7.4.3.4.2 (Bug XV-mut): a fixed-size primitive @mutable
            // member uses the COMPACT length code by wire size — NOT the universal
            // LC=4 NEXTINT frame. 2-byte → LC=1 (`emheader_2`), 4-byte → LC=2
            // (`emheader_4`), 8-byte → LC=3 (`emheader_8`); none of them serialize
            // a NEXTINT. This matches the Rust reference (`MutableStructEncoder`
            // via `LengthCode`) and is cross-vendor-validated against
            // CycloneDDS/RTI/FastDDS. BE: the helper writes little-endian member-id
            // EMHEADER (ambient-LE per §7.4.3.4.5) but the body endian must follow
            // the stream — for BE we emit the EMHEADER + raw body inline.
            let helper = match size {
                2 => Some("emheader_2"),
                4 => Some("emheader_4"),
                8 => Some("emheader_8"),
                _ => None,
            };
            match helper {
                Some(h) if endian != "be" => {
                    writeln!(
                        out,
                        "{indent}::dds::topic::xcdr2::{h}<{cpp_ty}>(zd_out, zd_origin, {id_expr}, {mu_lit}, {access});"
                    )
                    .map_err(fmt_err)?;
                }
                Some(_) => {
                    // BE: replicate the compact-EMHEADER helper inline so the
                    // primitive body is big-endian (the helpers are LE-only).
                    let lc = match size {
                        2 => 1,
                        4 => 2,
                        _ => 3,
                    };
                    writeln!(
                        out,
                        "{indent}{{ ::dds::topic::xcdr2::pad_to_from_origin(zd_out, zd_origin, 4);"
                    )
                    .map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{indent}    ::dds::topic::xcdr2::emheader_write(zd_out, ::dds::topic::xcdr2::emheader_make({lc}u, {id_expr}, {mu_lit}), {beb});"
                    )
                    .map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{indent}    ::dds::topic::xcdr2::write_be_raw<{cpp_ty}>(zd_out, {access}); }}"
                    )
                    .map_err(fmt_err)?;
                }
                None => {
                    writeln!(
                        out,
                        "{indent}// xcdr2: unexpected primitive size {size} (skip)"
                    )
                    .map_err(fmt_err)?;
                }
            }
        }
        TypeSpec::String(s) if !s.wide => {
            // Bounded narrow `string<N>` (DDS-XTypes §7.4.3): byte-length check.
            if let Some(b) = &s.bound {
                let bv = bound_to_cpp(b);
                writeln!(
                    out,
                    "{indent}if ({access}.size() > {bv}) throw std::length_error(\"bounded string length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            // EMHEADER LC=5 (Bug XV-mut): a string member REUSES its own leading
            // uint32 length prefix as the NEXTINT — XTypes 1.3 §7.4.3.4.2, EMHEADER
            // bits 30-28 = 101. So we write the EMHEADER with LC=5 and then the
            // string body (length prefix + bytes) directly; NO separate NEXTINT is
            // serialized (the prefix doubles as it). Cross-vendor-validated against
            // the Rust reference (`LengthCode::Lc5` / `reuses_leading_len`).
            writeln!(
                out,
                "{indent}{{ ::dds::topic::xcdr2::pad_to_from_origin(zd_out, zd_origin, 4);"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    ::dds::topic::xcdr2::emheader_write(zd_out, ::dds::topic::xcdr2::emheader_make(5u, {id_expr}, {mu_lit}), {beb});"
            )
            .map_err(fmt_err)?;
            // The length prefix sits at the (4-aligned) EMHEADER body start; use it
            // as the origin for the string-len alignment count.
            writeln!(
                out,
                "{indent}    {{ const auto zd_body_origin = zd_out.size(); (void)zd_body_origin;"
            )
            .map_err(fmt_err)?;
            if endian == "be" {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_string_be(zd_out, {access});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_string_origin(zd_out, zd_body_origin, {access}, zd_max_align);"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{indent}    }} }}").map_err(fmt_err)?;
        }
        TypeSpec::String(s) if s.wide => {
            // Bounded `wstring<N>` (DDS-XTypes §7.4.3): wide-char-length check.
            if let Some(b) = &s.bound {
                let bv = bound_to_cpp(b);
                writeln!(
                    out,
                    "{indent}if ({access}.size() > {bv}) throw std::length_error(\"bounded wstring length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            // EMHEADER LC=5 (Bug XV-mut): like a narrow string, a `wstring`
            // serializes a leading uint32 octet-length prefix, which LC=5 reuses as
            // the NEXTINT (no separate NEXTINT). Mirrors the Rust reference
            // (`TypeSpec::String(_) => Lc5`).
            writeln!(
                out,
                "{indent}{{ ::dds::topic::xcdr2::pad_to_from_origin(zd_out, zd_origin, 4);"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    ::dds::topic::xcdr2::emheader_write(zd_out, ::dds::topic::xcdr2::emheader_make(5u, {id_expr}, {mu_lit}), {beb});"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    {{ const auto zd_body_origin = zd_out.size(); (void)zd_body_origin;"
            )
            .map_err(fmt_err)?;
            if endian == "be" {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_wstring_be(zd_out, {access});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_wstring_origin(zd_out, zd_body_origin, {access}, zd_max_align);"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{indent}    }} }}").map_err(fmt_err)?;
        }
        TypeSpec::Sequence(seq) => {
            // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = throw.
            if let Some(b) = &seq.bound {
                let bv = bound_to_cpp(b);
                writeln!(
                    out,
                    "{indent}if ({access}.size() > {bv}) throw std::length_error(\"bounded sequence length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            // FINDING T1b: a non-primitive-element sequence's body BEGINS with
            // its own DHEADER (a 4-byte length word). XTypes 1.3 §7.4.3.4.2: such
            // a member uses EMHEADER LengthCode-5, which REUSES that leading
            // DHEADER as the NEXTINT — NO separate NEXTINT is serialized. A
            // `sequence<primitive>` has a bare element count (not a byte length)
            // and so stays on the universal LC=4 NEXTINT frame. This mirrors the
            // Rust reference (`member_body_has_leading_dheader` → `LengthCode::Lc5`)
            // and CycloneDDS / RTI / FastDDS byte-for-byte.
            let seq_inner_dh = !matches!(&*seq.elem, TypeSpec::Primitive(_));
            if seq_inner_dh {
                // LC=5: EMHEADER with no NEXTINT; the seq DHEADER below doubles
                // as the NEXTINT (length word). body_origin = start of that word.
                writeln!(
                    out,
                    "{indent}{{ ::dds::topic::xcdr2::pad_to_from_origin(zd_out, zd_origin, 4);"
                )
                .map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}    ::dds::topic::xcdr2::emheader_write(zd_out, ::dds::topic::xcdr2::emheader_make(5u, {id_expr}, {mu_lit}), {beb});"
                )
                .map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}    {{ const auto zd_body_origin = zd_out.size(); (void)zd_body_origin;"
                )
                .map_err(fmt_err)?;
            } else {
                // LC=4: universal NEXTINT frame (bare element count body).
                writeln!(
                    out,
                    "{indent}{{ const auto zd_sub = ::dds::topic::xcdr2::emheader_nextint_begin(zd_out, zd_origin, {id_expr}, {mu_lit}, {beb});"
                )
                .map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}    {{ const auto zd_body_origin = zd_sub.body_start; (void)zd_body_origin;"
                )
                .map_err(fmt_err)?;
            }
            if seq_inner_dh {
                writeln!(
                    out,
                    "{indent}      const auto zd_seq_dh = ::dds::topic::xcdr2::dheader_begin_r(zd_out, zd_repr);"
                )
                .map_err(fmt_err)?;
            }
            if endian == "be" {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_be<uint32_t>(zd_out, static_cast<uint32_t>({access}.size()));"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_le_origin<uint32_t>(zd_out, zd_body_origin, static_cast<uint32_t>({access}.size()), zd_max_align);"
                )
                .map_err(fmt_err)?;
            }
            if matches!(&*seq.elem, TypeSpec::Primitive(PrimitiveType::Octet)) {
                // sequence<octet>: raw byte block instead of a per-byte loop.
                writeln!(
                    out,
                    "{indent}      zd_out.insert(zd_out.end(), {access}.begin(), {access}.end());"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(out, "{indent}      for (const auto& zd_e : {access}) {{")
                    .map_err(fmt_err)?;
                match &*seq.elem {
                    TypeSpec::Primitive(PrimitiveType::Boolean) => {
                        writeln!(
                            out,
                            "{indent}        ::dds::topic::xcdr2::write_bool(zd_out, zd_e);"
                        )
                        .map_err(fmt_err)?;
                    }
                    TypeSpec::Primitive(PrimitiveType::Octet) => {
                        writeln!(
                            out,
                            "{indent}        ::dds::topic::xcdr2::write_u8(zd_out, zd_e);"
                        )
                        .map_err(fmt_err)?;
                    }
                    TypeSpec::Primitive(p) => {
                        let cpp_ty = primitive_to_cpp(*p);
                        if endian == "be" {
                            writeln!(
                            out,
                            "{indent}        ::dds::topic::xcdr2::write_be<{cpp_ty}>(zd_out, zd_e);"
                        )
                        .map_err(fmt_err)?;
                        } else {
                            writeln!(out, "{indent}        ::dds::topic::xcdr2::write_le_origin<{cpp_ty}>(zd_out, zd_body_origin, zd_e, zd_max_align);").map_err(fmt_err)?;
                        }
                    }
                    TypeSpec::String(s) if !s.wide => {
                        if endian == "be" {
                            writeln!(
                                out,
                                "{indent}        ::dds::topic::xcdr2::write_string_be(zd_out, zd_e);"
                            )
                            .map_err(fmt_err)?;
                        } else {
                            writeln!(out, "{indent}        ::dds::topic::xcdr2::write_string_origin(zd_out, zd_body_origin, zd_e, zd_max_align);").map_err(fmt_err)?;
                        }
                    }
                    // wstring / enum / nested struct (any extensibility) elements:
                    // recurse with the EMHEADER body-origin (identical to the
                    // plain-path arms; non-final elements pad-to-4 + splice).
                    TypeSpec::String(_) => {
                        emit_value_write(
                            out,
                            &seq.elem,
                            "zd_e",
                            endian,
                            "zd_body_origin",
                            &format!("{indent}        "),
                        )?;
                    }
                    TypeSpec::Scoped(sc)
                        if scoped_is_enum(sc)
                            || scoped_struct(sc).is_some()
                            || scoped_union(sc).is_some() =>
                    {
                        emit_value_write(
                            out,
                            &seq.elem,
                            "zd_e",
                            endian,
                            "zd_body_origin",
                            &format!("{indent}        "),
                        )?;
                    }
                    // nested sequence / map element (each emits its own DHEADER).
                    TypeSpec::Sequence(_) | TypeSpec::Map(_) => {
                        emit_value_write(
                            out,
                            &seq.elem,
                            "zd_e",
                            endian,
                            "zd_body_origin",
                            &format!("{indent}        "),
                        )?;
                    }
                    _ => {
                        writeln!(
                            out,
                            "{indent}        // xcdr2: nested seq-elem not supported"
                        )
                        .map_err(fmt_err)?;
                    }
                }
                writeln!(out, "{indent}      }}").map_err(fmt_err)?;
            }
            if seq_inner_dh {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::dheader_end_r(zd_out, zd_seq_dh, {beb}, zd_repr);"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{indent}    }}").map_err(fmt_err)?;
            if seq_inner_dh {
                // LC=5: no NEXTINT to patch (the seq DHEADER was the NEXTINT).
                writeln!(out, "{indent}}}").map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{indent}    ::dds::topic::xcdr2::emheader_nextint_end(zd_out, zd_sub, {beb}); }}"
                )
                .map_err(fmt_err)?;
            }
        }
        // enum member: 4-byte int32 -> compact LC=2 EMHEADER (no NEXTINT).
        TypeSpec::Scoped(s) if scoped_is_enum(s) => {
            let ec = enum_wire_ctype(scoped_enum_bytes(s));
            writeln!(
                out,
                "{indent}::dds::topic::xcdr2::emheader_4<{ec}>(zd_out, zd_origin, {id_expr}, {mu_lit}, static_cast<{ec}>({access}));"
            )
            .map_err(fmt_err)?;
        }
        // nested struct member as a @mutable member. FINDING T1b: a @final
        // nested struct has NO inner DHEADER → its body does NOT begin with a
        // length word, so it stays on the universal LC=4 NEXTINT frame. A
        // nested @appendable/@mutable struct's encoding BEGINS with its own
        // DHEADER (a 4-byte length word); XTypes 1.3 §7.4.3.4.2 LengthCode-5
        // REUSES that leading DHEADER as the NEXTINT — NO separate NEXTINT is
        // serialized. This mirrors the Rust reference
        // (`member_body_has_leading_dheader` → `LengthCode::Lc5`) and
        // CycloneDDS / RTI / FastDDS byte-for-byte.
        TypeSpec::Scoped(sc) if scoped_struct(sc).is_some() => {
            let Some((def, ext)) = scoped_struct(sc) else {
                return Ok(());
            };
            match ext {
                Extensibility::Final => {
                    // Inline @final member reads resolve in the nested type's
                    // own module scope (P0-2).
                    let _sg = enter_ref_scope(sc);
                    // LC=4: NEXTINT frame around the tight-packed (no DHEADER) body.
                    writeln!(
                        out,
                        "{indent}{{ const auto zd_sub = ::dds::topic::xcdr2::emheader_nextint_begin(zd_out, zd_origin, {id_expr}, {mu_lit}, {beb});"
                    )
                    .map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{indent}    {{ const auto zd_body_origin = zd_sub.body_start; (void)zd_body_origin;"
                    )
                    .map_err(fmt_err)?;
                    for sm in &def.members {
                        let sm_name = escape_cpp_ident(&sm.declarators[0].name().text);
                        emit_value_write(
                            out,
                            &sm.type_spec,
                            &format!("{access}.{sm_name}()"),
                            endian,
                            "zd_body_origin",
                            &format!("{indent}      "),
                        )?;
                    }
                    writeln!(out, "{indent}    }}").map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{indent}    ::dds::topic::xcdr2::emheader_nextint_end(zd_out, zd_sub, {beb}); }}"
                    )
                    .map_err(fmt_err)?;
                }
                Extensibility::Appendable | Extensibility::Mutable => {
                    // LC=5: EMHEADER with no NEXTINT; the spliced nested encoding
                    // BEGINS with its own DHEADER, which doubles as the NEXTINT.
                    let cpp = scoped_to_cpp(sc);
                    let id = next_nest_id();
                    writeln!(
                        out,
                        "{indent}{{ ::dds::topic::xcdr2::pad_to_from_origin(zd_out, zd_origin, 4);"
                    )
                    .map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{indent}    ::dds::topic::xcdr2::emheader_write(zd_out, ::dds::topic::xcdr2::emheader_make(5u, {id_expr}, {mu_lit}), {beb});"
                    )
                    .map_err(fmt_err)?;
                    if endian == "be" {
                        writeln!(
                            out,
                            "{indent}    auto zd_nsb{id} = ::dds::topic::topic_type_support<{cpp}>::encode_be({access}, zd_repr);"
                        )
                        .map_err(fmt_err)?;
                    } else {
                        writeln!(
                            out,
                            "{indent}    auto zd_nsb{id} = ::dds::topic::topic_type_support<{cpp}>::encode({access}, zd_repr);"
                        )
                        .map_err(fmt_err)?;
                    }
                    writeln!(
                        out,
                        "{indent}    zd_out.insert(zd_out.end(), zd_nsb{id}.begin(), zd_nsb{id}.end()); }}"
                    )
                    .map_err(fmt_err)?;
                }
            }
        }
        // map<K,V> member: FINDING T1b. A map is always a non-primitive
        // collection whose body BEGINS with its own DHEADER (a 4-byte length
        // word), so per XTypes 1.3 §7.4.3.4.2 it uses LengthCode-5, REUSING that
        // leading DHEADER as the NEXTINT — NO separate NEXTINT. Mirrors the Rust
        // reference (`TypeSpec::Map(_)` → `LengthCode::Lc5`) and the vendors.
        TypeSpec::Map(m) => {
            // FINDING (primitive-map): a `map<primitive,primitive>` body has NO
            // leading DHEADER (XTypes 1.3 §7.4.3.5) — like a `sequence<primitive>`
            // it carries a bare element count, so it stays on the universal LC=4
            // NEXTINT frame. A `map<long,Pt>` (non-primitive element) DOES begin
            // with its own DHEADER and uses LengthCode-5, REUSING that leading
            // DHEADER as the NEXTINT. Mirrors the @mutable sequence arm above.
            let map_inner_dh = !map_pair_is_primitive(&m.key, &m.value);
            if map_inner_dh {
                writeln!(
                    out,
                    "{indent}{{ ::dds::topic::xcdr2::pad_to_from_origin(zd_out, zd_origin, 4);"
                )
                .map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}    ::dds::topic::xcdr2::emheader_write(zd_out, ::dds::topic::xcdr2::emheader_make(5u, {id_expr}, {mu_lit}), {beb});"
                )
                .map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}    {{ const auto zd_body_origin = zd_out.size(); (void)zd_body_origin;"
                )
                .map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}      const auto zd_map_dh = ::dds::topic::xcdr2::dheader_begin_r(zd_out, zd_repr);"
                )
                .map_err(fmt_err)?;
            } else {
                // LC=4: universal NEXTINT frame (bare element count body).
                writeln!(
                    out,
                    "{indent}{{ const auto zd_sub = ::dds::topic::xcdr2::emheader_nextint_begin(zd_out, zd_origin, {id_expr}, {mu_lit}, {beb});"
                )
                .map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}    {{ const auto zd_body_origin = zd_sub.body_start; (void)zd_body_origin;"
                )
                .map_err(fmt_err)?;
            }
            if endian == "be" {
                writeln!(out, "{indent}      ::dds::topic::xcdr2::write_be<uint32_t>(zd_out, static_cast<uint32_t>({access}.size()));").map_err(fmt_err)?;
            } else {
                writeln!(out, "{indent}      ::dds::topic::xcdr2::write_le_origin<uint32_t>(zd_out, zd_body_origin, static_cast<uint32_t>({access}.size()), zd_max_align);").map_err(fmt_err)?;
            }
            writeln!(out, "{indent}      for (const auto& zd_kv : {access}) {{")
                .map_err(fmt_err)?;
            let kv_indent = format!("{indent}        ");
            emit_value_write(
                out,
                &m.key,
                "zd_kv.first",
                endian,
                "zd_body_origin",
                &kv_indent,
            )?;
            emit_value_write(
                out,
                &m.value,
                "zd_kv.second",
                endian,
                "zd_body_origin",
                &kv_indent,
            )?;
            writeln!(out, "{indent}      }}").map_err(fmt_err)?;
            if map_inner_dh {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::dheader_end_r(zd_out, zd_map_dh, {beb}, zd_repr);"
                )
                .map_err(fmt_err)?;
                writeln!(out, "{indent}    }}").map_err(fmt_err)?;
                // LC=5: no NEXTINT to patch (the map DHEADER was the NEXTINT).
                writeln!(out, "{indent}}}").map_err(fmt_err)?;
            } else {
                writeln!(out, "{indent}    }}").map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}    ::dds::topic::xcdr2::emheader_nextint_end(zd_out, zd_sub, {beb}); }}"
                )
                .map_err(fmt_err)?;
            }
        }
        TypeSpec::Fixed(_) => {
            // fixed<P,S> @mutable member: raw BCD body (no leading length word),
            // so the universal LC=4 NEXTINT frame (matching the Rust reference's
            // `encode_member` default — see `mutable_member_length_code` → None).
            writeln!(
                out,
                "{indent}{{ const auto zd_sub = ::dds::topic::xcdr2::emheader_nextint_begin(zd_out, zd_origin, {id_expr}, {mu_lit}, {beb});"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    {{ const auto& zd_bcd = {access}.bcd_bytes(); zd_out.insert(zd_out.end(), zd_bcd.begin(), zd_bcd.end()); }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    ::dds::topic::xcdr2::emheader_nextint_end(zd_out, zd_sub, {beb}); }}"
            )
            .map_err(fmt_err)?;
        }
        _ => {
            writeln!(out, "{indent}// xcdr2: unsupported member type").map_err(fmt_err)?;
        }
    }
    Ok(())
}

/// `true` when BOTH the map key and value are XCDR `IS_PRIMITIVE` scalars, so the
/// map carries NO collection DHEADER (XTypes 1.3 §7.4.3.5; mirrors cdr-core
/// `needs_collection_dheader(.., K::IS_PRIMITIVE && V::IS_PRIMITIVE)` and the
/// PARRAY rule already applied to primitive arrays above). `map<long,Pt>` keeps
/// its DHEADER; `map<long,long>` omits it (FastDDS/OpenDDS-confirmed).
fn map_pair_is_primitive(key: &TypeSpec, value: &TypeSpec) -> bool {
    matches!(key, TypeSpec::Primitive(_)) && matches!(value, TypeSpec::Primitive(_))
}

fn primitive_size(p: PrimitiveType) -> usize {
    use zerodds_idl::ast::{FloatingType, IntegerType};
    match p {
        PrimitiveType::Boolean => 1,
        PrimitiveType::Octet => 1,
        PrimitiveType::Char => 1,
        PrimitiveType::WideChar => 2,
        PrimitiveType::Integer(i) => match i {
            IntegerType::Int8 | IntegerType::UInt8 => 1,
            IntegerType::Short | IntegerType::UShort | IntegerType::Int16 | IntegerType::UInt16 => {
                2
            }
            IntegerType::Long | IntegerType::ULong | IntegerType::Int32 | IntegerType::UInt32 => 4,
            IntegerType::LongLong
            | IntegerType::ULongLong
            | IntegerType::Int64
            | IntegerType::UInt64 => 8,
        },
        PrimitiveType::Floating(f) => match f {
            FloatingType::Float => 4,
            FloatingType::Double => 8,
            FloatingType::LongDouble => 16,
        },
    }
}

fn emit_decode_fn(
    out: &mut String,
    cpp_fqn: &str,
    s: &StructDef,
    ext: Extensibility,
    phase: TtsPhase,
) -> Result<(), CppGenError> {
    if phase == TtsPhase::Decl {
        writeln!(
            out,
            "    static {cpp_fqn} decode(const uint8_t* zd_buf, size_t zd_len, \
             ::dds::topic::xcdr2::XcdrVersion zd_repr, bool zd_be = false);"
        )
        .map_err(fmt_err)?;
        return Ok(());
    }
    writeln!(
        out,
        "inline {cpp_fqn} topic_type_support<{cpp_fqn}>::decode(const uint8_t* zd_buf, size_t zd_len, \
         ::dds::topic::xcdr2::XcdrVersion zd_repr, bool zd_be) {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        size_t zd_pos = 0;").map_err(fmt_err)?;
    writeln!(out, "        {cpp_fqn} zd_v;").map_err(fmt_err)?;
    // The XCDR version controls alignment: XCDR2 caps 8-byte primitives
    // to 4-byte boundaries (XTypes 1.3 §7.4.3.4.2), XCDR1 does not. `zd_be`
    // selects the wire byte order (false = little-endian, the canonical wire).
    writeln!(
        out,
        "        const size_t zd_max_align = ::dds::topic::xcdr2::xcdr_max_align(zd_repr);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "        (void)zd_buf; (void)zd_len; (void)zd_pos; (void)zd_max_align; (void)zd_be;"
    )
    .map_err(fmt_err)?;

    match ext {
        Extensibility::Final => {
            writeln!(out, "        const size_t zd_origin = 0;").map_err(fmt_err)?;
            writeln!(out, "        (void)zd_origin;").map_err(fmt_err)?;
            for m in &resolved_wire_members(s) {
                emit_plain_member_decode(out, m, "zd_origin")?;
            }
        }
        Extensibility::Appendable => {
            // XCDR1 has NO DHEADER (origin = stream start, max_align 8); XCDR2
            // reads the 4-byte DHEADER first (origin after, max_align 4). The
            // member reads are identical; only the DHEADER framing differs.
            writeln!(
                out,
                "        const bool zd_x1 = (zd_repr == ::dds::topic::xcdr2::XcdrVersion::Xcdr1);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        size_t zd_end = zd_len;").map_err(fmt_err)?;
            writeln!(out, "        if (!zd_x1) {{").map_err(fmt_err)?;
            writeln!(
                out,
                "            const auto zd_dh = ::dds::topic::xcdr2::dheader_read(zd_buf, zd_pos, zd_len, zd_be);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "            zd_end = zd_pos + zd_dh;").map_err(fmt_err)?;
            writeln!(out, "        }}").map_err(fmt_err)?;
            writeln!(out, "        const size_t zd_origin = zd_pos;").map_err(fmt_err)?;
            writeln!(out, "        (void)zd_end;").map_err(fmt_err)?;
            for m in &resolved_wire_members(s) {
                emit_plain_member_decode(out, m, "zd_origin")?;
            }
            // Skip trailing bytes (forward-compat with appendable extension);
            // only meaningful under XCDR2 where the DHEADER bounded the body.
            writeln!(
                out,
                "        if (!zd_x1 && zd_pos < zd_end) zd_pos = zd_end;"
            )
            .map_err(fmt_err)?;
        }
        Extensibility::Mutable => {
            // XCDR1 -> PL_CDR1 (no DHEADER, PID-framed parameters to a sentinel);
            // XCDR2 -> PL_CDR2 (DHEADER + EMHEADER per member). LE and BE share
            // the PL_CDR2 path; only XCDR1-LE takes PL_CDR1.
            writeln!(
                out,
                "        if (zd_repr == ::dds::topic::xcdr2::XcdrVersion::Xcdr1) {{"
            )
            .map_err(fmt_err)?;
            writeln!(out, "            while (zd_pos + 4 <= zd_len) {{").map_err(fmt_err)?;
            writeln!(
                out,
                "                const auto zd_ph = ::dds::topic::xcdr2::pl_cdr1_read_header(zd_buf, zd_pos, zd_len, zd_be);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "                if (zd_ph.is_end) break;").map_err(fmt_err)?;
            writeln!(
                out,
                "                const size_t zd_pl_origin = zd_pos; (void)zd_pl_origin;"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                const size_t zd_pl_end = zd_pos + zd_ph.body_len;"
            )
            .map_err(fmt_err)?;
            writeln!(out, "                switch (zd_ph.member_id) {{").map_err(fmt_err)?;
            for (m, id) in resolved_wire_members(s).iter().zip(resolved_member_ids(s)) {
                emit_pl_cdr1_member_decode_case(out, m, id)?;
            }
            writeln!(out, "                    default: break;").map_err(fmt_err)?;
            writeln!(out, "                }}").map_err(fmt_err)?;
            writeln!(
                out,
                "                if (zd_pos < zd_pl_end) zd_pos = zd_pl_end;"
            )
            .map_err(fmt_err)?;
            writeln!(out, "                ::dds::topic::xcdr2::pl_cdr1_skip_pad(zd_pos, zd_len, zd_ph.body_len);").map_err(fmt_err)?;
            writeln!(out, "            }}").map_err(fmt_err)?;
            writeln!(out, "            return zd_v;").map_err(fmt_err)?;
            writeln!(out, "        }}").map_err(fmt_err)?;
            writeln!(
                out,
                "        const auto zd_dh = ::dds::topic::xcdr2::dheader_read(zd_buf, zd_pos, zd_len, zd_be);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        const size_t zd_origin = zd_pos;").map_err(fmt_err)?;
            writeln!(out, "        const size_t zd_end = zd_origin + zd_dh;").map_err(fmt_err)?;
            writeln!(out, "        while (zd_pos + 4 <= zd_end) {{").map_err(fmt_err)?;
            writeln!(
                out,
                "            const auto zd_h = ::dds::topic::xcdr2::emheader_read(zd_buf, zd_pos, zd_len, zd_origin, zd_be);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "            switch (zd_h.member_id) {{").map_err(fmt_err)?;
            for (m, id) in resolved_wire_members(s).iter().zip(resolved_member_ids(s)) {
                emit_mutable_member_decode_case(out, m, id)?;
            }
            writeln!(out, "                default: {{").map_err(fmt_err)?;
            writeln!(
                out,
                "                    // Unknown member: per-LC skip per XTypes 1.3"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    // §7.4.3.4.2 (LengthCode::body_len). LC0..3 are"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    // fixed 1/2/4/8-byte bodies WITHOUT NEXTINT; LC4/5 NEXTINT="
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    // byte length; LC6/7 NEXTINT=element count (4 + 4n / 4 + 8n)."
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    if (zd_h.lc == 0) {{ zd_pos += 1; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else if (zd_h.lc == 1) {{ zd_pos += 2; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else if (zd_h.lc == 2) {{ zd_pos += 4; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else if (zd_h.lc == 3) {{ zd_pos += 8; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else if (zd_h.lc == 4 || zd_h.lc == 5) {{ auto zd_n = ::dds::topic::xcdr2::emheader_nextint_read(zd_buf, zd_pos, zd_len, zd_be); zd_pos += zd_n; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else if (zd_h.lc == 6) {{ auto zd_c = ::dds::topic::xcdr2::emheader_nextint_read(zd_buf, zd_pos, zd_len, zd_be); zd_pos += 4 + 4 * static_cast<size_t>(zd_c); }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else {{ auto zd_c = ::dds::topic::xcdr2::emheader_nextint_read(zd_buf, zd_pos, zd_len, zd_be); zd_pos += 4 + 8 * static_cast<size_t>(zd_c); }}"
            )
            .map_err(fmt_err)?;
            writeln!(out, "                    break;").map_err(fmt_err)?;
            writeln!(out, "                }}").map_err(fmt_err)?;
            writeln!(out, "            }}").map_err(fmt_err)?;
            writeln!(out, "        }}").map_err(fmt_err)?;
            writeln!(out, "        if (zd_pos < zd_end) zd_pos = zd_end;").map_err(fmt_err)?;
        }
    }

    writeln!(out, "        return zd_v;").map_err(fmt_err)?;
    writeln!(out, "    }}").map_err(fmt_err)?;
    Ok(())
}

fn emit_plain_member_decode(out: &mut String, m: &Member, origin: &str) -> Result<(), CppGenError> {
    let m = &normalize_member(m);
    let is_optional = has_optional_annotation(&m.annotations);
    for decl in &m.declarators {
        // broad-audit P0-7: reject unsupported @shared shapes loudly. Plain @shared
        // decodes the value BY VALUE and assigns it through the value-typed setter
        // overload (see `emit_struct_member_accessors`), which wraps it in a fresh
        // shared_ptr (XTypes 1.3 §7.3.1.2.1.9 — @shared is in-memory sharing only).
        reject_unsupported_shared(m, decl)?;
        let name = escape_cpp_ident(&decl.name().text);
        // Fixed array member (any dims): read the shared array body (symmetric to
        // the plain-encode array path — 1-D leaf tight-packed, multi-dim primitive
        // PARRAY tight-packed, non-primitive elements DHEADER-wrapped).
        if let Declarator::Array(arr) = decl {
            if !emit_array_body_decode(
                out,
                &m.type_spec,
                arr.sizes.len(),
                &name,
                origin,
                "        ",
            )? {
                writeln!(
                    out,
                    "        // xcdr2: array member '{name}' (1-D string only / unsupported elem) not supported (skip)"
                )
                .map_err(fmt_err)?;
            }
            continue;
        }
        if !typespec_supported(&m.type_spec) {
            writeln!(
                out,
                "        // xcdr2: member '{name}' not supported (skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        if is_optional {
            writeln!(out, "        {{").map_err(fmt_err)?;
            writeln!(
                out,
                "            uint8_t zd_present = ::dds::topic::xcdr2::read_u8(zd_buf, zd_pos, zd_len);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "            if (zd_present) {{").map_err(fmt_err)?;
            emit_value_read(
                out,
                &m.type_spec,
                &format!("zd_v.{name}"),
                origin,
                "                ",
                true,
            )?;
            writeln!(out, "            }} else {{").map_err(fmt_err)?;
            writeln!(out, "                zd_v.{name}(std::nullopt);").map_err(fmt_err)?;
            writeln!(out, "            }}").map_err(fmt_err)?;
            writeln!(out, "        }}").map_err(fmt_err)?;
        } else {
            emit_value_read(
                out,
                &m.type_spec,
                &format!("zd_v.{name}"),
                origin,
                "        ",
                false,
            )?;
        }
    }
    Ok(())
}

/// zerodds-lint: recursion-depth 64 (nested type emit; bounded by IDL nesting)
fn emit_value_read(
    out: &mut String,
    ts: &TypeSpec,
    setter: &str,
    origin: &str,
    indent: &str,
    is_opt: bool,
) -> Result<(), CppGenError> {
    let wrap_opt = |v: String| -> String {
        if is_opt {
            format!("std::optional<decltype({v})>({v})")
        } else {
            v
        }
    };
    let _ = wrap_opt;
    match ts {
        TypeSpec::Primitive(PrimitiveType::Boolean) => {
            writeln!(
                out,
                "{indent}{setter}(::dds::topic::xcdr2::read_bool(zd_buf, zd_pos, zd_len));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(PrimitiveType::Octet) => {
            writeln!(
                out,
                "{indent}{setter}(::dds::topic::xcdr2::read_u8(zd_buf, zd_pos, zd_len));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(p) => {
            let cpp_ty = primitive_to_cpp(*p);
            writeln!(
                out,
                "{indent}{setter}(::dds::topic::xcdr2::read_le_origin<{cpp_ty}>(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::String(s) if !s.wide => {
            // B1 follow-up (#22 decode-side parity): mirror the encode-side
            // bound check (`emit_value_write` above) on decode — XTypes 1.3
            // §7.4.3 requires the IDL bound enforced on BOTH sides, not just
            // the wire's remaining-buffer check that `read_string_origin`
            // already does.
            if let Some(b) = &s.bound {
                let bv = bound_to_cpp(b);
                let id = next_nest_id();
                let tmp = format!("zd_bc{id}");
                writeln!(
                    out,
                    "{indent}{{ auto {tmp} = ::dds::topic::xcdr2::read_string_origin(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be); if ({tmp}.size() > {bv}) throw std::length_error(\"decoded string length exceeds its IDL bound ({bv})\"); {setter}(std::move({tmp})); }}"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{indent}{setter}(::dds::topic::xcdr2::read_string_origin(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be));"
                )
                .map_err(fmt_err)?;
            }
        }
        TypeSpec::String(s) if s.wide => {
            if let Some(b) = &s.bound {
                let bv = bound_to_cpp(b);
                let id = next_nest_id();
                let tmp = format!("zd_bc{id}");
                writeln!(
                    out,
                    "{indent}{{ auto {tmp} = ::dds::topic::xcdr2::read_wstring_origin(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be); if ({tmp}.size() > {bv}) throw std::length_error(\"decoded wstring length exceeds its IDL bound ({bv})\"); {setter}(std::move({tmp})); }}"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{indent}{setter}(::dds::topic::xcdr2::read_wstring_origin(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be));"
                )
                .map_err(fmt_err)?;
            }
        }
        TypeSpec::Sequence(seq) => {
            if matches!(&*seq.elem, TypeSpec::Primitive(PrimitiveType::Octet)) {
                // sequence<octet>: raw byte block directly from the buffer.
                writeln!(out, "{indent}{{").map_err(fmt_err)?;
                writeln!(out, "{indent}    auto zd_cnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be);").map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}    ::dds::topic::xcdr2::check_avail(zd_pos, zd_cnt, zd_len);"
                )
                .map_err(fmt_err)?;
                // B1 follow-up (#22 decode-side parity): mirror the encode-side
                // bound check — XTypes 1.3 §7.4.3 requires the IDL bound
                // enforced on decode too, not just the wire's remaining-buffer
                // check `check_avail` already does above.
                if let Some(b) = &seq.bound {
                    let bv = bound_to_cpp(b);
                    writeln!(
                        out,
                        "{indent}    if (zd_cnt > {bv}) throw std::length_error(\"decoded sequence length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                }
                writeln!(
                    out,
                    "{indent}    std::vector<uint8_t> zd_seq(zd_buf + zd_pos, zd_buf + zd_pos + zd_cnt);"
                )
                .map_err(fmt_err)?;
                writeln!(out, "{indent}    zd_pos += zd_cnt;").map_err(fmt_err)?;
                writeln!(out, "{indent}    {setter}(std::move(zd_seq));").map_err(fmt_err)?;
                writeln!(out, "{indent}}}").map_err(fmt_err)?;
                return Ok(());
            }
            let elem_cpp_ty: String = match &*seq.elem {
                TypeSpec::Primitive(PrimitiveType::Boolean) => "bool".to_string(),
                TypeSpec::Primitive(p) => primitive_to_cpp(*p).to_string(),
                TypeSpec::String(s) if !s.wide => "std::string".to_string(),
                // wide string element -> std::wstring (narrow caught above).
                TypeSpec::String(_) => "std::wstring".to_string(),
                // enum (-> underlying int32, but the vector holds the enum) and
                // nested struct elements (any extensibility) use their C++ type.
                TypeSpec::Scoped(s) if scoped_is_enum(s) => scoped_to_cpp(s),
                TypeSpec::Scoped(s) if scoped_struct(s).is_some() => scoped_to_cpp(s),
                // union element (sequence<union>, Bug R3) -> the union's C++ type;
                // each element is spliced/sub-decoded by its own TypeSupport.
                TypeSpec::Scoped(s) if scoped_union(s).is_some() => scoped_to_cpp(s),
                // nested sequence element -> std::vector<inner>.
                TypeSpec::Sequence(_) => typespec_to_cpp(&seq.elem)?,
                // map element -> std::map<K,V>.
                TypeSpec::Map(_) => typespec_to_cpp(&seq.elem)?,
                _ => {
                    writeln!(
                        out,
                        "{indent}// xcdr2: nested seq-elem not supported (skip)"
                    )
                    .map_err(fmt_err)?;
                    return Ok(());
                }
            };
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            // XCDR2 §7.4.3.5: for non-primitive elements skip the DHEADER (4-aligned).
            if !matches!(&*seq.elem, TypeSpec::Primitive(_)) {
                writeln!(
                    out,
                    "{indent}    ::dds::topic::xcdr2::skip_pad_from_origin(zd_pos, {origin}, 4);"
                )
                .map_err(fmt_err)?;
                writeln!(out, "{indent}    const auto zd_seq_dh = ::dds::topic::xcdr2::dheader_read_r(zd_buf, zd_pos, zd_len, zd_be, zd_repr); (void)zd_seq_dh;").map_err(fmt_err)?;
            }
            writeln!(out, "{indent}    auto zd_cnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be);").map_err(fmt_err)?;
            // B1 follow-up (#22 decode-side parity): mirror the encode-side
            // bound check — XTypes 1.3 §7.4.3.
            if let Some(b) = &seq.bound {
                let bv = bound_to_cpp(b);
                writeln!(
                    out,
                    "{indent}    if (zd_cnt > {bv}) throw std::length_error(\"decoded sequence length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{indent}    std::vector<{elem_cpp_ty}> zd_seq;").map_err(fmt_err)?;
            writeln!(out, "{indent}    zd_seq.reserve(zd_cnt);").map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    for (uint32_t zd_i = 0; zd_i < zd_cnt; ++zd_i) {{"
            )
            .map_err(fmt_err)?;
            match &*seq.elem {
                TypeSpec::Primitive(PrimitiveType::Boolean) => {
                    writeln!(out, "{indent}        zd_seq.push_back(::dds::topic::xcdr2::read_bool(zd_buf, zd_pos, zd_len));").map_err(fmt_err)?;
                }
                TypeSpec::Primitive(PrimitiveType::Octet) => {
                    writeln!(out, "{indent}        zd_seq.push_back(::dds::topic::xcdr2::read_u8(zd_buf, zd_pos, zd_len));").map_err(fmt_err)?;
                }
                TypeSpec::Primitive(p) => {
                    let cpp_ty = primitive_to_cpp(*p);
                    writeln!(out, "{indent}        zd_seq.push_back(::dds::topic::xcdr2::read_le_origin<{cpp_ty}>(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be));").map_err(fmt_err)?;
                }
                TypeSpec::String(s) if !s.wide => {
                    writeln!(out, "{indent}        zd_seq.push_back(::dds::topic::xcdr2::read_string_origin(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be));").map_err(fmt_err)?;
                }
                // wide string element.
                TypeSpec::String(_) => {
                    writeln!(out, "{indent}        zd_seq.push_back(::dds::topic::xcdr2::read_wstring_origin(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be));").map_err(fmt_err)?;
                }
                // enum element: read int32, cast back to the enum type.
                TypeSpec::Scoped(s) if scoped_is_enum(s) => {
                    let cpp_ty = scoped_to_cpp(s);
                    let ec = enum_wire_ctype(scoped_enum_bytes(s));
                    writeln!(out, "{indent}        zd_seq.push_back(static_cast<{cpp_ty}>(::dds::topic::xcdr2::read_le_origin<{ec}>(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be)));").map_err(fmt_err)?;
                }
                // nested @final struct element: read each sub-member into a fresh
                // temp, push the whole object (symmetric to the inline encode).
                TypeSpec::Scoped(sc) if scoped_final_struct(sc).is_some() => {
                    if let Some(def) = scoped_final_struct(sc) {
                        let cpp_ty = scoped_to_cpp(sc);
                        let _sg = enter_ref_scope(sc);
                        let var = format!("zd_se{}", next_nest_id());
                        let binner = format!("{indent}        ");
                        writeln!(out, "{binner}{cpp_ty} {var}{{}};").map_err(fmt_err)?;
                        for sm in &def.members {
                            let sm_name = escape_cpp_ident(&sm.declarators[0].name().text);
                            emit_value_read(
                                out,
                                &sm.type_spec,
                                &format!("{var}.{sm_name}"),
                                origin,
                                &binner,
                                false,
                            )?;
                        }
                        writeln!(out, "{binner}zd_seq.push_back({var});").map_err(fmt_err)?;
                    }
                }
                // nested @appendable/@mutable struct element: 4-align, read the
                // element's own DHEADER, sub-decode the [DHEADER+body] slice via
                // the nested type's `decode`, advance the cursor past it, push it.
                TypeSpec::Scoped(sc) if scoped_struct(sc).is_some() => {
                    let cpp_ty = scoped_to_cpp(sc);
                    let id = next_nest_id();
                    let var = format!("zd_se{id}");
                    let binner = format!("{indent}        ");
                    writeln!(
                        out,
                        "{binner}::dds::topic::xcdr2::skip_pad_from_origin(zd_pos, {origin}, 4);"
                    )
                    .map_err(fmt_err)?;
                    emit_nested_span_decode(out, &cpp_ty, &var, id, &binner, true)?;
                    writeln!(out, "{binner}zd_seq.push_back(std::move({var}));")
                        .map_err(fmt_err)?;
                }
                // union element (sequence<union>, Bug R3): 4-align, read the
                // union's own DHEADER, sub-decode the [DHEADER+body] slice via the
                // union's own `decode`, advance the cursor, push it (symmetric to
                // the appendable-struct splice above; previously dropped → data
                // loss / empty sequence).
                TypeSpec::Scoped(sc) if scoped_union(sc).is_some() => {
                    let Some(u) = scoped_union(sc) else {
                        return Ok(());
                    };
                    let cpp_ty = scoped_to_cpp(sc);
                    let id = next_nest_id();
                    let var = format!("zd_se{id}");
                    let binner = format!("{indent}        ");
                    match union_extensibility(&u.annotations) {
                        // @final union element: inline disc + selected member,
                        // NO per-element DHEADER (rule (26), symmetric to the
                        // inline encode).
                        Extensibility::Final => {
                            let disc_ts = switch_type_spec(&u.switch_type);
                            let disc_cpp = switch_type_to_cpp(&u.switch_type)?;
                            writeln!(out, "{binner}{cpp_ty} {var};").map_err(fmt_err)?;
                            writeln!(out, "{binner}{disc_cpp} zd_disc{id}{{}};")
                                .map_err(fmt_err)?;
                            emit_value_read(
                                out,
                                &disc_ts,
                                &format!("zd_disc{id} ="),
                                origin,
                                &binner,
                                false,
                            )?;
                            writeln!(out, "{binner}{var}._d(zd_disc{id});").map_err(fmt_err)?;
                            emit_union_branch_switch_at(
                                out, &u, &disc_cpp, /*decode=*/ true, "le", &var, origin,
                            )?;
                            writeln!(out, "{binner}zd_seq.push_back(std::move({var}));")
                                .map_err(fmt_err)?;
                        }
                        // @appendable/@mutable union element: 4-align, read its
                        // own DHEADER, sub-decode the [DHEADER+body] slice.
                        Extensibility::Appendable | Extensibility::Mutable => {
                            writeln!(
                                out,
                                "{binner}::dds::topic::xcdr2::skip_pad_from_origin(zd_pos, {origin}, 4);"
                            )
                            .map_err(fmt_err)?;
                            emit_nested_span_decode(out, &cpp_ty, &var, id, &binner, true)?;
                            writeln!(out, "{binner}zd_seq.push_back(std::move({var}));")
                                .map_err(fmt_err)?;
                        }
                    }
                }
                // nested sequence element: read the inner sequence into a temp
                // via the assignment-setter form, then push it.
                TypeSpec::Sequence(_) => {
                    let inner_ty = typespec_to_cpp(&seq.elem)?;
                    let var = format!("zd_se{}", next_nest_id());
                    let binner = format!("{indent}        ");
                    writeln!(out, "{binner}{inner_ty} {var}{{}};").map_err(fmt_err)?;
                    emit_value_read(out, &seq.elem, &format!("{var} ="), origin, &binner, false)?;
                    writeln!(out, "{binner}zd_seq.push_back(std::move({var}));")
                        .map_err(fmt_err)?;
                }
                // map element: read the inner map into a temp, then push it.
                TypeSpec::Map(_) => {
                    let inner_ty = typespec_to_cpp(&seq.elem)?;
                    let var = format!("zd_se{}", next_nest_id());
                    let binner = format!("{indent}        ");
                    writeln!(out, "{binner}{inner_ty} {var}{{}};").map_err(fmt_err)?;
                    emit_value_read(out, &seq.elem, &format!("{var} ="), origin, &binner, false)?;
                    writeln!(out, "{binner}zd_seq.push_back(std::move({var}));")
                        .map_err(fmt_err)?;
                }
                _ => {}
            }
            writeln!(out, "{indent}    }}").map_err(fmt_err)?;
            writeln!(out, "{indent}    {setter}(std::move(zd_seq));").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
        // map<K,V> member: read DHEADER, count, then count interleaved key/value
        // pairs (symmetric to the encode); insert into a std::map. Key/value are
        // read into fresh temps via the assignment-setter form `zd_k =(...)`
        // (emit_value_read always ends in `{setter}(final)`, so `zd_k =` yields a
        // plain assignment that works for primitive/string/enum/struct values).
        TypeSpec::Map(m) => {
            let k_ty = typespec_to_cpp(&m.key)?;
            let v_ty = typespec_to_cpp(&m.value)?;
            let id = next_nest_id();
            let mapv = format!("zd_map{id}");
            let kv = format!("zd_mk{id}");
            let vv = format!("zd_mv{id}");
            let inner = format!("{indent}    ");
            let li = format!("{inner}    ");
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            // Symmetric to the encoder: the map DHEADER is 4-aligned to origin,
            // and present ONLY for a non-primitive (key,value) element.
            writeln!(
                out,
                "{inner}::dds::topic::xcdr2::skip_pad_from_origin(zd_pos, {origin}, 4);"
            )
            .map_err(fmt_err)?;
            if !map_pair_is_primitive(&m.key, &m.value) {
                writeln!(out, "{inner}const auto zd_map_dh = ::dds::topic::xcdr2::dheader_read_r(zd_buf, zd_pos, zd_len, zd_be, zd_repr); (void)zd_map_dh;").map_err(fmt_err)?;
            }
            writeln!(out, "{inner}auto zd_mcnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be);").map_err(fmt_err)?;
            // B1 follow-up (#22 decode-side parity): mirror the encode-side
            // bound check — XTypes 1.3 §7.4.3.
            if let Some(b) = &m.bound {
                let bv = bound_to_cpp(b);
                writeln!(
                    out,
                    "{inner}if (zd_mcnt > {bv}) throw std::length_error(\"decoded map length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{inner}std::map<{k_ty}, {v_ty}> {mapv};").map_err(fmt_err)?;
            writeln!(
                out,
                "{inner}for (uint32_t zd_i = 0; zd_i < zd_mcnt; ++zd_i) {{"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{li}{k_ty} {kv}{{}};").map_err(fmt_err)?;
            writeln!(out, "{li}{v_ty} {vv}{{}};").map_err(fmt_err)?;
            emit_value_read(out, &m.key, &format!("{kv} ="), origin, &li, false)?;
            emit_value_read(out, &m.value, &format!("{vv} ="), origin, &li, false)?;
            writeln!(out, "{li}{mapv}.emplace(std::move({kv}), std::move({vv}));")
                .map_err(fmt_err)?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
            writeln!(out, "{inner}{setter}(std::move({mapv}));").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
        // bitmask / bitset member: read the holder integer (cdr-core holder
        // width) then reconstruct. Bitmask -> `enum class : uintN` cast; bitset
        // -> `Flags{ value }` aggregate init. XTypes §7.4.x.
        TypeSpec::Scoped(s) if scoped_bitholder(s).is_some() => {
            let bytes = scoped_bitholder(s).unwrap_or(1);
            let holder = holder_uint_for_bytes(bytes);
            let cpp_ty = scoped_to_cpp(s);
            let read = format!(
                "::dds::topic::xcdr2::read_le_origin<{holder}>(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be)"
            );
            let built = if scoped_is_bitset(s) {
                format!("{cpp_ty}{{ static_cast<uint64_t>({read}) }}")
            } else {
                format!("static_cast<{cpp_ty}>({read})")
            };
            writeln!(out, "{indent}{setter}({});", wrap_opt(built)).map_err(fmt_err)?;
        }
        // enum member: read its @bit_bound-width underlying value, cast to enum.
        TypeSpec::Scoped(s) if scoped_is_enum(s) => {
            let cpp_ty = scoped_to_cpp(s);
            let ec = enum_wire_ctype(scoped_enum_bytes(s));
            writeln!(
                out,
                "{indent}{setter}(static_cast<{cpp_ty}>(::dds::topic::xcdr2::read_le_origin<{ec}>(zd_buf, zd_pos, zd_len, {origin}, zd_max_align, zd_be)));"
            )
            .map_err(fmt_err)?;
        }
        // nested struct member. @final: read each sub-member into a fresh temp
        // (symmetric to the inline encode). @appendable/@mutable: read the
        // nested DHEADER length, then sub-decode the [DHEADER+body] slice via the
        // nested type's own `decode` and advance the cursor past it.
        TypeSpec::Scoped(sc) if scoped_struct(sc).is_some() => {
            let Some((def, ext)) = scoped_struct(sc) else {
                return Ok(());
            };
            let cpp_ty = scoped_to_cpp(sc);
            // Switch to the nested type's module scope for the inline (@final)
            // member reads (P0-2); `cpp_ty` was resolved in the outer scope.
            let _sg = enter_ref_scope(sc);
            let id = next_nest_id();
            let var = format!("zd_ns{id}");
            let inner = format!("{indent}    ");
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            writeln!(out, "{inner}{cpp_ty} {var}{{}};").map_err(fmt_err)?;
            match ext {
                Extensibility::Final => {
                    for sm in &def.members {
                        let sm_name = escape_cpp_ident(&sm.declarators[0].name().text);
                        emit_value_read(
                            out,
                            &sm.type_spec,
                            &format!("{var}.{sm_name}"),
                            origin,
                            &inner,
                            false,
                        )?;
                    }
                }
                Extensibility::Appendable | Extensibility::Mutable => {
                    writeln!(
                        out,
                        "{inner}::dds::topic::xcdr2::skip_pad_from_origin(zd_pos, {origin}, 4);"
                    )
                    .map_err(fmt_err)?;
                    emit_nested_span_decode(out, &cpp_ty, &var, id, &inner, false)?;
                }
            }
            writeln!(out, "{inner}{setter}({var});").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
        // Bug R3: union member — 4-align, read the union's own DHEADER, sub-decode
        // the [DHEADER+body] slice via `topic_type_support<Union>::decode`, then
        // advance the cursor past it (symmetric to the appendable-struct splice).
        TypeSpec::Scoped(sc) if scoped_union(sc).is_some() => {
            let Some(u) = scoped_union(sc) else {
                return Ok(());
            };
            let cpp_ty = scoped_to_cpp(sc);
            // The union's own cases resolve in the union's module scope (P0-2).
            let _sg = enter_ref_scope(sc);
            let id = next_nest_id();
            let var = format!("zd_nu{id}");
            let inner = format!("{indent}    ");
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            match union_extensibility(&u.annotations) {
                // @final union: inline disc + selected member, NO DHEADER
                // (rule (26) FUNION_TYPE, §7.4.3.4.1). Read into a local union
                // built at the OUTER origin (8-byte members align to 4 relative
                // to the top-level DHEADER), then hand it to the setter.
                Extensibility::Final => {
                    let disc_ts = switch_type_spec(&u.switch_type);
                    let disc_cpp = switch_type_to_cpp(&u.switch_type)?;
                    writeln!(out, "{inner}{cpp_ty} {var};").map_err(fmt_err)?;
                    writeln!(out, "{inner}{disc_cpp} zd_disc{id}{{}};").map_err(fmt_err)?;
                    emit_value_read(
                        out,
                        &disc_ts,
                        &format!("zd_disc{id} ="),
                        origin,
                        &inner,
                        false,
                    )?;
                    writeln!(out, "{inner}{var}._d(zd_disc{id});").map_err(fmt_err)?;
                    emit_union_branch_switch_at(
                        out, &u, &disc_cpp, /*decode=*/ true, "le", &var, origin,
                    )?;
                    writeln!(out, "{inner}{setter}({var});").map_err(fmt_err)?;
                }
                // @appendable/@mutable union: 4-align, read its own DHEADER,
                // sub-decode the [DHEADER+body] slice via the union's `decode`.
                Extensibility::Appendable | Extensibility::Mutable => {
                    writeln!(
                        out,
                        "{inner}::dds::topic::xcdr2::skip_pad_from_origin(zd_pos, {origin}, 4);"
                    )
                    .map_err(fmt_err)?;
                    emit_nested_span_decode(out, &cpp_ty, &var, id, &inner, true)?;
                    writeln!(out, "{inner}{setter}({var});").map_err(fmt_err)?;
                }
            }
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
        TypeSpec::Fixed(_) => {
            // fixed<P,S>: read exactly (P+2)/2 raw BCD octets (no align/endian),
            // construct a `::dds::core::Fixed<P,S>` from them.
            let cpp_ty = typespec_to_cpp(ts)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    ::dds::topic::xcdr2::check_avail(zd_pos, {cpp_ty}::kByteCount, zd_len);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{indent}    {cpp_ty} zd_fx(zd_buf + zd_pos);").map_err(fmt_err)?;
            writeln!(out, "{indent}    zd_pos += {cpp_ty}::kByteCount;").map_err(fmt_err)?;
            writeln!(out, "{indent}    {setter}(std::move(zd_fx));").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
        _ => {}
    }
    Ok(())
}

/// Emits one PL_CDR1 (`@mutable` XCDR1) decode `case`: dispatch on the parameter
/// member id and decode the body as a plain positional value, origin-relative to
/// the parameter body start (`zd_pl_origin`, max_align 8). Optional presence is
/// implied by the parameter being in the list, so there is no present-flag — an
/// absent optional simply never matches a case and stays default-constructed.
/// Mirrors `emit_pl_cdr1_member_encode`'s id assignment.
fn emit_pl_cdr1_member_decode_case(
    out: &mut String,
    m: &Member,
    base_id: u32,
) -> Result<(), CppGenError> {
    let m = &normalize_member(m);
    let is_optional = has_optional_annotation(&m.annotations);
    for (idx, decl) in m.declarators.iter().enumerate() {
        // broad-audit P0-7: reject unsupported @shared shapes loudly. Plain @shared
        // assigns the decoded value through the value-typed setter overload (wraps
        // it in a fresh shared_ptr — XTypes 1.3 §7.3.1.2.1.9).
        reject_unsupported_shared(m, decl)?;
        let name = escape_cpp_ident(&decl.name().text);
        let this_id = base_id + idx as u32;
        // broad-audit P0-6: array member decode under XCDR1 / PL_CDR1. The PID
        // header was already read by the outer loop (no NEXTINT under XCDR1); read
        // the array body directly from the parameter body origin. Symmetric to the
        // PL_CDR1 array encode.
        if let Declarator::Array(arr) = decl {
            writeln!(out, "                    case 0x{this_id:x}u: {{").map_err(fmt_err)?;
            let ok = emit_array_body_decode(
                out,
                &m.type_spec,
                arr.sizes.len(),
                &name,
                "zd_pl_origin",
                "                        ",
            )?;
            if !ok {
                return Err(CppGenError::UnsupportedConstruct {
                    construct: "array member in @mutable struct".into(),
                    context: Some(name),
                });
            }
            writeln!(out, "                        break;").map_err(fmt_err)?;
            writeln!(out, "                    }}").map_err(fmt_err)?;
            continue;
        }
        if !matches!(decl, Declarator::Simple(_)) {
            continue;
        }
        if !typespec_supported(&m.type_spec) {
            continue;
        }
        writeln!(out, "                    case 0x{this_id:x}u: {{").map_err(fmt_err)?;
        emit_value_read(
            out,
            &m.type_spec,
            &format!("zd_v.{name}"),
            "zd_pl_origin",
            "                        ",
            is_optional,
        )?;
        writeln!(out, "                        break;").map_err(fmt_err)?;
        writeln!(out, "                    }}").map_err(fmt_err)?;
    }
    Ok(())
}

/// Emits the repr-aware decode of a nested `@appendable`/`@mutable` element
/// (sequence / array / map element) that advances `zd_pos` past it. Under XCDR2
/// the element is DHEADER-delimited, so the cursor jumps `4 + dheader`. Under
/// XCDR1 (classic CDR) there is NO delimiter — decode from the cursor to the
/// buffer end and advance by the element's re-encoded byte length (XCDR1 encode
/// is byte-identical to the wire for canonical data). `declare_var` emits the
/// `cpp_ty var;` slot (default-constructed); pass false when the caller already
/// declared it. Requires `cpp_ty` to have a repr-aware `encode(v, repr)` (true
/// for structs; union elements use the dedicated union path).
fn emit_nested_span_decode(
    out: &mut String,
    cpp_ty: &str,
    var: &str,
    id: u32,
    indent: &str,
    declare_var: bool,
) -> Result<(), CppGenError> {
    writeln!(out, "{indent}const size_t zd_nss{id} = zd_pos;").map_err(fmt_err)?;
    if declare_var {
        writeln!(out, "{indent}{cpp_ty} {var};").map_err(fmt_err)?;
    }
    writeln!(
        out,
        "{indent}if (zd_repr == ::dds::topic::xcdr2::XcdrVersion::Xcdr1) {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{indent}    {var} = ::dds::topic::topic_type_support<{cpp_ty}>::decode(zd_buf + zd_nss{id}, zd_len - zd_nss{id}, zd_repr, zd_be);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{indent}    zd_pos = zd_nss{id} + ::dds::topic::topic_type_support<{cpp_ty}>::encode({var}, zd_repr).size();"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{indent}}} else {{").map_err(fmt_err)?;
    writeln!(out, "{indent}    size_t zd_npk{id} = zd_pos;").map_err(fmt_err)?;
    writeln!(
        out,
        "{indent}    const uint32_t zd_nl{id} = ::dds::topic::xcdr2::dheader_read(zd_buf, zd_npk{id}, zd_len, zd_be);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{indent}    {var} = ::dds::topic::topic_type_support<{cpp_ty}>::decode(zd_buf + zd_nss{id}, 4u + zd_nl{id}, zd_repr, zd_be);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{indent}    zd_pos = zd_nss{id} + 4u + zd_nl{id};").map_err(fmt_err)?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    Ok(())
}

fn emit_mutable_member_decode_case(
    out: &mut String,
    m: &Member,
    base_id: u32,
) -> Result<(), CppGenError> {
    let m = &normalize_member(m);
    let is_optional = has_optional_annotation(&m.annotations);
    let _ = is_optional; // mutable optional: same path; absent member just skips this case.
    for (idx, decl) in m.declarators.iter().enumerate() {
        // broad-audit P0-7: reject unsupported @shared shapes loudly. Plain @shared
        // assigns the decoded value through the value-typed setter overload (wraps
        // it in a fresh shared_ptr — XTypes 1.3 §7.3.1.2.1.9).
        reject_unsupported_shared(m, decl)?;
        let name = escape_cpp_ident(&decl.name().text);
        let this_id = base_id + idx as u32;
        // broad-audit P0-6: array member decode. Symmetric to the LC=4 encode —
        // the EMHEADER was already read by the outer loop; consume the separately-
        // serialized NEXTINT (total array-body byte length, discarded here since
        // the fixed dimensions bound the read), then read the array body from the
        // NEXTINT-relative origin (tight-packed primitives / DHEADER-wrapped
        // non-primitive elements).
        if let Declarator::Array(arr) = decl {
            let id_expr = format!("0x{this_id:x}u");
            writeln!(out, "                case {id_expr}: {{").map_err(fmt_err)?;
            writeln!(out, "                    auto zd_n = ::dds::topic::xcdr2::emheader_nextint_read(zd_buf, zd_pos, zd_len, zd_be); (void)zd_n;").map_err(fmt_err)?;
            writeln!(
                out,
                "                    const size_t zd_body_origin = zd_pos; (void)zd_body_origin;"
            )
            .map_err(fmt_err)?;
            let ok = emit_array_body_decode(
                out,
                &m.type_spec,
                arr.sizes.len(),
                &name,
                "zd_body_origin",
                "                    ",
            )?;
            if !ok {
                return Err(CppGenError::UnsupportedConstruct {
                    construct: "array member in @mutable struct".into(),
                    context: Some(name),
                });
            }
            writeln!(out, "                    break;").map_err(fmt_err)?;
            writeln!(out, "                }}").map_err(fmt_err)?;
            continue;
        }
        if !matches!(decl, Declarator::Simple(_)) {
            continue;
        }
        if !typespec_supported(&m.type_spec) {
            continue;
        }
        let id_expr = format!("0x{this_id:x}u");
        writeln!(out, "                case {id_expr}: {{").map_err(fmt_err)?;
        match &m.type_spec {
            TypeSpec::Primitive(PrimitiveType::Boolean) => {
                // boolean/octet stay compact LC=0 (1-byte body, no NEXTINT).
                writeln!(out, "                    uint8_t zd_b = ::dds::topic::xcdr2::read_u8(zd_buf, zd_pos, zd_len);").map_err(fmt_err)?;
                writeln!(
                    out,
                    "                    zd_v.{name}(static_cast<bool>(zd_b));"
                )
                .map_err(fmt_err)?;
            }
            TypeSpec::Primitive(PrimitiveType::Octet) => {
                writeln!(out, "                    zd_v.{name}(::dds::topic::xcdr2::read_u8(zd_buf, zd_pos, zd_len));").map_err(fmt_err)?;
            }
            TypeSpec::Primitive(p) => {
                // 2/4/8-byte @mutable primitives are framed with the COMPACT
                // length code (LC=1/2/3) by the encoder (Bug XV-mut) — NO NEXTINT
                // precedes the body. Read the value directly. (XTypes §7.4.3.4.2.)
                let cpp_ty = primitive_to_cpp(*p);
                writeln!(out, "                    zd_v.{name}(::dds::topic::xcdr2::read_le_raw<{cpp_ty}>(zd_buf, zd_pos, zd_len, zd_be));").map_err(fmt_err)?;
            }
            TypeSpec::String(s) if !s.wide => {
                // EMHEADER LC=5 (Bug XV-mut): the string's own uint32 length prefix
                // doubled as the NEXTINT — there is NO separate NEXTINT to skip.
                // Read the string body directly from the EMHEADER body origin.
                writeln!(out, "                    auto zd_body_origin = zd_pos;")
                    .map_err(fmt_err)?;
                // B1 follow-up (#22 decode-side parity): mirror the encode-side
                // bound check — XTypes 1.3 §7.4.3.
                if let Some(b) = &s.bound {
                    let bv = bound_to_cpp(b);
                    writeln!(out, "                    auto zd_bcs = ::dds::topic::xcdr2::read_string_origin(zd_buf, zd_pos, zd_len, zd_body_origin, zd_max_align, zd_be);").map_err(fmt_err)?;
                    writeln!(out, "                    if (zd_bcs.size() > {bv}) throw std::length_error(\"decoded string length exceeds its IDL bound ({bv})\");").map_err(fmt_err)?;
                    writeln!(out, "                    zd_v.{name}(std::move(zd_bcs));")
                        .map_err(fmt_err)?;
                } else {
                    writeln!(out, "                    zd_v.{name}(::dds::topic::xcdr2::read_string_origin(zd_buf, zd_pos, zd_len, zd_body_origin, zd_max_align, zd_be));").map_err(fmt_err)?;
                }
            }
            TypeSpec::String(s) if s.wide => {
                // EMHEADER LC=5 (Bug XV-mut): wstring octet-length prefix = NEXTINT.
                writeln!(out, "                    auto zd_body_origin = zd_pos;")
                    .map_err(fmt_err)?;
                if let Some(b) = &s.bound {
                    let bv = bound_to_cpp(b);
                    writeln!(out, "                    auto zd_bcw = ::dds::topic::xcdr2::read_wstring_origin(zd_buf, zd_pos, zd_len, zd_body_origin, zd_max_align, zd_be);").map_err(fmt_err)?;
                    writeln!(out, "                    if (zd_bcw.size() > {bv}) throw std::length_error(\"decoded wstring length exceeds its IDL bound ({bv})\");").map_err(fmt_err)?;
                    writeln!(out, "                    zd_v.{name}(std::move(zd_bcw));")
                        .map_err(fmt_err)?;
                } else {
                    writeln!(out, "                    zd_v.{name}(::dds::topic::xcdr2::read_wstring_origin(zd_buf, zd_pos, zd_len, zd_body_origin, zd_max_align, zd_be));").map_err(fmt_err)?;
                }
            }
            TypeSpec::Sequence(seq) => {
                // FINDING T1b: a sequence<primitive> is LC=4 (separate NEXTINT to
                // consume); a non-primitive-element sequence is LC=5 — its body
                // BEGINS with the seq DHEADER which doubled as the NEXTINT, so
                // there is NO separate NEXTINT. The DHEADER itself is read below.
                if matches!(&*seq.elem, TypeSpec::Primitive(_)) {
                    writeln!(out, "                    auto zd_n = ::dds::topic::xcdr2::emheader_nextint_read(zd_buf, zd_pos, zd_len, zd_be);").map_err(fmt_err)?;
                    writeln!(out, "                    (void)zd_n;").map_err(fmt_err)?;
                }
                writeln!(out, "                    auto zd_body_origin = zd_pos;")
                    .map_err(fmt_err)?;
                // LC=5 non-primitive-element sequence: read the inner DHEADER
                // (= the reused NEXTINT length word) before the count.
                if !matches!(&*seq.elem, TypeSpec::Primitive(_)) {
                    writeln!(out, "                    {{ const auto zd_seq_dh = ::dds::topic::xcdr2::dheader_read_r(zd_buf, zd_pos, zd_len, zd_be, zd_repr); (void)zd_seq_dh; }}").map_err(fmt_err)?;
                }
                writeln!(out, "                    auto zd_cnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(zd_buf, zd_pos, zd_len, zd_body_origin, zd_max_align, zd_be);").map_err(fmt_err)?;
                // B1 follow-up (#22 decode-side parity): mirror the encode-side
                // bound check — XTypes 1.3 §7.4.3.
                if let Some(b) = &seq.bound {
                    let bv = bound_to_cpp(b);
                    writeln!(
                        out,
                        "                    if (zd_cnt > {bv}) throw std::length_error(\"decoded sequence length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                }
                if matches!(&*seq.elem, TypeSpec::Primitive(PrimitiveType::Octet)) {
                    // sequence<octet>: raw byte block directly from the buffer.
                    writeln!(
                        out,
                        "                    ::dds::topic::xcdr2::check_avail(zd_pos, zd_cnt, zd_len);"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "                    std::vector<uint8_t> zd_seq(zd_buf + zd_pos, zd_buf + zd_pos + zd_cnt);").map_err(fmt_err)?;
                    writeln!(out, "                    zd_pos += zd_cnt;").map_err(fmt_err)?;
                    writeln!(out, "                    zd_v.{name}(std::move(zd_seq));")
                        .map_err(fmt_err)?;
                } else {
                    let elem_cpp_ty: String = match &*seq.elem {
                        TypeSpec::Primitive(PrimitiveType::Boolean) => "bool".to_string(),
                        TypeSpec::Primitive(p) => primitive_to_cpp(*p).to_string(),
                        TypeSpec::String(s) if !s.wide => "std::string".to_string(),
                        TypeSpec::String(_) => "std::wstring".to_string(),
                        TypeSpec::Scoped(s) if scoped_is_enum(s) => scoped_to_cpp(s),
                        TypeSpec::Scoped(s) if scoped_struct(s).is_some() => scoped_to_cpp(s),
                        TypeSpec::Sequence(_) | TypeSpec::Map(_) => typespec_to_cpp(&seq.elem)?,
                        _ => "uint8_t".to_string(),
                    };
                    writeln!(
                        out,
                        "                    std::vector<{elem_cpp_ty}> zd_seq;"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "                    zd_seq.reserve(zd_cnt);")
                        .map_err(fmt_err)?;
                    writeln!(
                        out,
                        "                    for (uint32_t zd_i = 0; zd_i < zd_cnt; ++zd_i) {{"
                    )
                    .map_err(fmt_err)?;
                    match &*seq.elem {
                        TypeSpec::Primitive(PrimitiveType::Boolean) => {
                            writeln!(out, "                        zd_seq.push_back(::dds::topic::xcdr2::read_bool(zd_buf, zd_pos, zd_len));").map_err(fmt_err)?;
                        }
                        TypeSpec::Primitive(p) => {
                            let cpp_ty = primitive_to_cpp(*p);
                            writeln!(out, "                        zd_seq.push_back(::dds::topic::xcdr2::read_le_origin<{cpp_ty}>(zd_buf, zd_pos, zd_len, zd_body_origin, zd_max_align, zd_be));").map_err(fmt_err)?;
                        }
                        TypeSpec::String(s) if !s.wide => {
                            writeln!(out, "                        zd_seq.push_back(::dds::topic::xcdr2::read_string_origin(zd_buf, zd_pos, zd_len, zd_body_origin, zd_max_align, zd_be));").map_err(fmt_err)?;
                        }
                        TypeSpec::String(_) => {
                            writeln!(out, "                        zd_seq.push_back(::dds::topic::xcdr2::read_wstring_origin(zd_buf, zd_pos, zd_len, zd_body_origin, zd_max_align, zd_be));").map_err(fmt_err)?;
                        }
                        TypeSpec::Scoped(s) if scoped_is_enum(s) => {
                            let cpp_ty = scoped_to_cpp(s);
                            let ec = enum_wire_ctype(scoped_enum_bytes(s));
                            writeln!(out, "                        zd_seq.push_back(static_cast<{cpp_ty}>(::dds::topic::xcdr2::read_le_origin<{ec}>(zd_buf, zd_pos, zd_len, zd_body_origin, zd_max_align, zd_be)));").map_err(fmt_err)?;
                        }
                        TypeSpec::Scoped(sc) if scoped_final_struct(sc).is_some() => {
                            if let Some(def) = scoped_final_struct(sc) {
                                let cpp_ty = scoped_to_cpp(sc);
                                let _sg = enter_ref_scope(sc);
                                let var = format!("zd_se{}", next_nest_id());
                                writeln!(out, "                        {cpp_ty} {var}{{}};")
                                    .map_err(fmt_err)?;
                                for sm in &def.members {
                                    let sm_name = escape_cpp_ident(&sm.declarators[0].name().text);
                                    emit_value_read(
                                        out,
                                        &sm.type_spec,
                                        &format!("{var}.{sm_name}"),
                                        "zd_body_origin",
                                        "                        ",
                                        false,
                                    )?;
                                }
                                writeln!(out, "                        zd_seq.push_back({var});")
                                    .map_err(fmt_err)?;
                            }
                        }
                        // nested @appendable/@mutable struct element: 4-align,
                        // read the element DHEADER, sub-decode the [DHEADER+body]
                        // slice via the nested type's `decode`, advance, push.
                        TypeSpec::Scoped(sc) if scoped_struct(sc).is_some() => {
                            let cpp_ty = scoped_to_cpp(sc);
                            let id = next_nest_id();
                            let var = format!("zd_se{id}");
                            writeln!(out, "                        ::dds::topic::xcdr2::skip_pad_from_origin(zd_pos, zd_body_origin, 4);").map_err(fmt_err)?;
                            writeln!(
                                out,
                                "                        const size_t zd_nss{id} = zd_pos;"
                            )
                            .map_err(fmt_err)?;
                            writeln!(out, "                        size_t zd_npk{id} = zd_pos;")
                                .map_err(fmt_err)?;
                            writeln!(out, "                        const uint32_t zd_nl{id} = ::dds::topic::xcdr2::dheader_read(zd_buf, zd_npk{id}, zd_len, zd_be);").map_err(fmt_err)?;
                            writeln!(out, "                        {cpp_ty} {var} = ::dds::topic::topic_type_support<{cpp_ty}>::decode(zd_buf + zd_nss{id}, 4u + zd_nl{id}, zd_repr, zd_be);").map_err(fmt_err)?;
                            writeln!(
                                out,
                                "                        zd_pos = zd_nss{id} + 4u + zd_nl{id};"
                            )
                            .map_err(fmt_err)?;
                            writeln!(
                                out,
                                "                        zd_seq.push_back(std::move({var}));"
                            )
                            .map_err(fmt_err)?;
                        }
                        // nested sequence / map element: read into a temp, push.
                        TypeSpec::Sequence(_) | TypeSpec::Map(_) => {
                            let inner_ty = typespec_to_cpp(&seq.elem)?;
                            let var = format!("zd_se{}", next_nest_id());
                            writeln!(out, "                        {inner_ty} {var}{{}};")
                                .map_err(fmt_err)?;
                            emit_value_read(
                                out,
                                &seq.elem,
                                &format!("{var} ="),
                                "zd_body_origin",
                                "                        ",
                                false,
                            )?;
                            writeln!(
                                out,
                                "                        zd_seq.push_back(std::move({var}));"
                            )
                            .map_err(fmt_err)?;
                        }
                        _ => {}
                    }
                    writeln!(out, "                    }}").map_err(fmt_err)?;
                    writeln!(out, "                    zd_v.{name}(std::move(zd_seq));")
                        .map_err(fmt_err)?;
                }
            }
            // enum member: 4-byte int32 read directly (encoded via compact LC=2).
            TypeSpec::Scoped(s) if scoped_is_enum(s) => {
                let cpp_ty = scoped_to_cpp(s);
                let ec = enum_wire_ctype(scoped_enum_bytes(s));
                writeln!(out, "                    zd_v.{name}(static_cast<{cpp_ty}>(::dds::topic::xcdr2::read_le_raw<{ec}>(zd_buf, zd_pos, zd_len, zd_be)));").map_err(fmt_err)?;
            }
            // nested struct member. FINDING T1b: a @final nested struct is LC=4
            // (a separate NEXTINT precedes its tight-packed body). A nested
            // @appendable/@mutable struct is LC=5 — its body BEGINS with its own
            // DHEADER which doubled as the NEXTINT, so there is NO separate
            // NEXTINT (the DHEADER is read by the sub-decode below).
            TypeSpec::Scoped(sc) if scoped_struct(sc).is_some() => {
                if let Some((def, ext)) = scoped_struct(sc) {
                    let cpp_ty = scoped_to_cpp(sc);
                    // Inline @final member reads resolve in the nested type's own
                    // module scope (P0-2); `cpp_ty` was resolved in the outer scope.
                    let _sg = enter_ref_scope(sc);
                    let id = next_nest_id();
                    let var = format!("zd_ns{id}");
                    if matches!(ext, Extensibility::Final) {
                        writeln!(out, "                    auto zd_n = ::dds::topic::xcdr2::emheader_nextint_read(zd_buf, zd_pos, zd_len, zd_be); (void)zd_n;").map_err(fmt_err)?;
                    }
                    writeln!(
                        out,
                        "                    auto zd_body_origin = zd_pos; (void)zd_body_origin;"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "                    {cpp_ty} {var}{{}};").map_err(fmt_err)?;
                    match ext {
                        Extensibility::Final => {
                            for sm in &def.members {
                                let sm_name = escape_cpp_ident(&sm.declarators[0].name().text);
                                emit_value_read(
                                    out,
                                    &sm.type_spec,
                                    &format!("{var}.{sm_name}"),
                                    "zd_body_origin",
                                    "                    ",
                                    false,
                                )?;
                            }
                        }
                        Extensibility::Appendable | Extensibility::Mutable => {
                            writeln!(out, "                    const size_t zd_nss{id} = zd_pos;")
                                .map_err(fmt_err)?;
                            writeln!(out, "                    size_t zd_npk{id} = zd_pos;")
                                .map_err(fmt_err)?;
                            writeln!(out, "                    const uint32_t zd_nl{id} = ::dds::topic::xcdr2::dheader_read(zd_buf, zd_npk{id}, zd_len, zd_be);").map_err(fmt_err)?;
                            writeln!(out, "                    {var} = ::dds::topic::topic_type_support<{cpp_ty}>::decode(zd_buf + zd_nss{id}, 4u + zd_nl{id}, zd_repr, zd_be);").map_err(fmt_err)?;
                            writeln!(
                                out,
                                "                    zd_pos = zd_nss{id} + 4u + zd_nl{id};"
                            )
                            .map_err(fmt_err)?;
                        }
                    }
                    writeln!(out, "                    zd_v.{name}({var});").map_err(fmt_err)?;
                }
            }
            // map<K,V> member: FINDING T1b — LC=5. The body BEGINS with the map
            // DHEADER which doubled as the NEXTINT, so there is NO separate
            // NEXTINT; the DHEADER is read below before the count + entries
            // (symmetric to the mutable encode).
            TypeSpec::Map(m) => {
                let k_ty = typespec_to_cpp(&m.key)?;
                let v_ty = typespec_to_cpp(&m.value)?;
                let id = next_nest_id();
                let mapv = format!("zd_map{id}");
                let kv = format!("zd_mk{id}");
                let vv = format!("zd_mv{id}");
                writeln!(out, "                    auto zd_body_origin = zd_pos;")
                    .map_err(fmt_err)?;
                // Every @mutable member carries a 4-byte length word right after its
                // EMHEADER (the dispatcher reads only the EMHEADER). For a non-prim
                // map (LC=5) it is the map's own DHEADER; for a primitive map (LC=4)
                // it is the NEXTINT byte length. Positionally identical — consume it
                // unconditionally, then read the count. (See encode arm above.)
                writeln!(out, "                    {{ const auto zd_map_dh = ::dds::topic::xcdr2::dheader_read_r(zd_buf, zd_pos, zd_len, zd_be, zd_repr); (void)zd_map_dh; }}").map_err(fmt_err)?;
                writeln!(out, "                    auto zd_mcnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(zd_buf, zd_pos, zd_len, zd_body_origin, zd_max_align, zd_be);").map_err(fmt_err)?;
                // B1 follow-up (#22 decode-side parity): mirror the encode-side
                // bound check — XTypes 1.3 §7.4.3.
                if let Some(b) = &m.bound {
                    let bv = bound_to_cpp(b);
                    writeln!(
                        out,
                        "                    if (zd_mcnt > {bv}) throw std::length_error(\"decoded map length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                }
                writeln!(out, "                    std::map<{k_ty}, {v_ty}> {mapv};")
                    .map_err(fmt_err)?;
                writeln!(
                    out,
                    "                    for (uint32_t zd_i = 0; zd_i < zd_mcnt; ++zd_i) {{"
                )
                .map_err(fmt_err)?;
                writeln!(out, "                        {k_ty} {kv}{{}};").map_err(fmt_err)?;
                writeln!(out, "                        {v_ty} {vv}{{}};").map_err(fmt_err)?;
                emit_value_read(
                    out,
                    &m.key,
                    &format!("{kv} ="),
                    "zd_body_origin",
                    "                        ",
                    false,
                )?;
                emit_value_read(
                    out,
                    &m.value,
                    &format!("{vv} ="),
                    "zd_body_origin",
                    "                        ",
                    false,
                )?;
                writeln!(
                    out,
                    "                        {mapv}.emplace(std::move({kv}), std::move({vv}));"
                )
                .map_err(fmt_err)?;
                writeln!(out, "                    }}").map_err(fmt_err)?;
                writeln!(out, "                    zd_v.{name}(std::move({mapv}));")
                    .map_err(fmt_err)?;
            }
            TypeSpec::Fixed(_) => {
                // @mutable fixed member: encoder framed it LC=4, so a separate
                // NEXTINT (byte length) precedes the raw BCD body — consume it,
                // then read (P+2)/2 octets into a `::dds::core::Fixed<P,S>`.
                let cpp_ty = typespec_to_cpp(&m.type_spec)?;
                writeln!(out, "                    {{ auto zd_n = ::dds::topic::xcdr2::emheader_nextint_read(zd_buf, zd_pos, zd_len, zd_be); (void)zd_n; }}").map_err(fmt_err)?;
                writeln!(out, "                    ::dds::topic::xcdr2::check_avail(zd_pos, {cpp_ty}::kByteCount, zd_len);").map_err(fmt_err)?;
                writeln!(out, "                    {{ {cpp_ty} zd_fx(zd_buf + zd_pos); zd_pos += {cpp_ty}::kByteCount; zd_v.{name}(std::move(zd_fx)); }}").map_err(fmt_err)?;
            }
            _ => {}
        }
        writeln!(out, "                    break;").map_err(fmt_err)?;
        writeln!(out, "                }}").map_err(fmt_err)?;
    }
    Ok(())
}

fn emit_key_hash_fn(
    out: &mut String,
    cpp_fqn: &str,
    s: &StructDef,
    is_keyed: bool,
    phase: TtsPhase,
) -> Result<(), CppGenError> {
    if phase == TtsPhase::Decl {
        writeln!(
            out,
            "    static std::array<uint8_t, 16> key_hash(const {cpp_fqn}& zd_v);"
        )
        .map_err(fmt_err)?;
        return Ok(());
    }
    writeln!(
        out,
        "inline std::array<uint8_t, 16> topic_type_support<{cpp_fqn}>::key_hash(const {cpp_fqn}& zd_v) {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        (void)zd_v;").map_err(fmt_err)?;
    if !is_keyed {
        writeln!(
            out,
            "        return std::array<uint8_t, 16>{{{{0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0}}}};"
        )
        .map_err(fmt_err)?;
        writeln!(out, "    }}").map_err(fmt_err)?;
        return Ok(());
    }
    writeln!(out, "        std::vector<uint8_t> zd_out;").map_err(fmt_err)?;
    writeln!(out, "        const size_t zd_origin = 0;").map_err(fmt_err)?;
    writeln!(out, "        (void)zd_origin;").map_err(fmt_err)?;
    // KeyHash serializes @key members as big-endian XCDR1 (RTPS §9.6.3.8 / XTypes
    // §7.6.8) — full natural alignment, NO XCDR2 8->4 cap. xcdr_max_align(Xcdr1)
    // = 8, so capped_align(sizeof(T), 8) == sizeof(T): byte-identical to the
    // previous uncapped key serialization.
    writeln!(
        out,
        "        const size_t zd_max_align = ::dds::topic::xcdr2::xcdr_max_align(::dds::topic::xcdr2::XcdrVersion::Xcdr1);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        (void)zd_max_align;").map_err(fmt_err)?;
    // XTypes 1.3 §7.6.8.3.1.b: KeyHolder members in ascending member-id order
    // (explicit `@id(N)`, else positional index among the `@key` members) —
    // NOT declaration order.
    let zd_all_members = resolved_wire_members(s);
    let key_members: Vec<&Member> = zd_all_members
        .iter()
        .filter(|m| has_key_annotation(&m.annotations))
        .collect();
    for m in sort_members_by_id(&key_members) {
        let dealiased = normalize_member(m);
        // A `@key` member whose (typedef-dealiased) type is a nested struct
        // is NOT delegated to `emit_plain_member_encode` (whose Scoped-struct
        // arm — via `emit_value_write` — encodes the WHOLE nested struct, and
        // for @appendable/@mutable splices the struct's own DHEADER-framed
        // encode). A KeyHolder is always the FLAT concatenation of that
        // struct's own `@key` subset (XTypes 1.3 §7.6.8), independent of the
        // struct's own extensibility — expand it with `emit_key_value_write`
        // instead. `struct_def_raw` (unlike `scoped_struct`) has no
        // "all-members-generically-encodable" gate, so a nested struct that
        // itself contains a typedef-aliased member (previously making the
        // whole outer member silently vanish from the KeyHash) is still
        // found and expanded here.
        let is_struct_key =
            matches!(&dealiased.type_spec, TypeSpec::Scoped(sc) if struct_def_raw(sc).is_some());
        if is_struct_key {
            for decl in &dealiased.declarators {
                let name = escape_cpp_ident(&decl.name().text);
                match decl {
                    Declarator::Simple(_) => {
                        emit_key_value_write(
                            out,
                            &dealiased.type_spec,
                            &format!("zd_v.{name}()"),
                            "be",
                            "zd_origin",
                        )?;
                    }
                    // Array-of-struct `@key` member (e.g. `@key Inner arr[3]`):
                    // NOT delegated to `emit_plain_member_encode` (whose
                    // array-of-struct branch wraps a DHEADER and encodes each
                    // element's WHOLE struct via `emit_value_write` — wrong on
                    // both counts for a KeyHolder, which is always the FLAT,
                    // un-framed concatenation of key bytes, XTypes 1.3
                    // §7.6.8). N nested range-for loops (row-major, same
                    // shape as the primitive multi-dim array path above) feed
                    // each element straight into `emit_key_value_write`,
                    // which expands it to just its own `@key` subset — same
                    // fix as the Simple-declarator case, extended over the
                    // array's elements instead of skipping it.
                    Declarator::Array(arr) => {
                        let n = arr.sizes.len();
                        let mut acc = format!("zd_v.{name}()");
                        for d in 0..n {
                            let lv = format!("zd_akey{d}");
                            writeln!(out, "        for (const auto& {lv} : {acc}) {{")
                                .map_err(fmt_err)?;
                            acc = lv;
                        }
                        emit_key_value_write(out, &dealiased.type_spec, &acc, "be", "zd_origin")?;
                        for _ in 0..n {
                            writeln!(out, "        }}").map_err(fmt_err)?;
                        }
                    }
                }
            }
            continue;
        }
        // Primitive / string / enum / union / bitholder / sequence / array
        // `@key` members: the investigation found the existing generic
        // per-field encoder already correct for these shapes — reuse it.
        emit_plain_member_encode(out, m, "be", "zd_origin")?;
    }
    // XTypes 1.3 §7.6.8.4: holder ≤ 16 octets -> zero-pad; otherwise MD5.
    writeln!(out, "        std::array<uint8_t, 16> zd_h{{}};").map_err(fmt_err)?;
    writeln!(out, "        if (zd_out.size() <= 16) {{").map_err(fmt_err)?;
    writeln!(
        out,
        "            std::memcpy(zd_h.data(), zd_out.data(), zd_out.size());"
    )
    .map_err(fmt_err)?;
    writeln!(out, "            return zd_h;").map_err(fmt_err)?;
    writeln!(out, "        }}").map_err(fmt_err)?;
    writeln!(out, "        return ::dds::topic::xcdr2_md5::md5(zd_out);").map_err(fmt_err)?;
    writeln!(out, "    }}").map_err(fmt_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers: fmt-Error-Bridge
// ---------------------------------------------------------------------------

fn fmt_err(_: core::fmt::Error) -> CppGenError {
    CppGenError::Internal("string formatting failed".into())
}

#[allow(dead_code)]
fn _ensure_used() {
    // is_reserved is used by check_identifier — a compiler hint.
    let _ = is_reserved("int");
}
