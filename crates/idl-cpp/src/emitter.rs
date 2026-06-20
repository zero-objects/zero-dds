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

use zerodds_idl::semantics::annotations::PlacementKind;

use crate::bitset::{emit_bitmask, emit_bitset};
use crate::error::CppGenError;
use crate::type_map::{check_identifier, is_reserved, primitive_to_cpp};
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
    if !probe_structs.is_empty() {
        // <array> is needed by key_hash(), <vector>/<string> by
        // encode/decode(). If the standard walks have not already pulled
        // them in, they are covered transitively via TopicTraits.hpp —
        // but we emit them explicitly here so the header remains
        // syntactically valid without the topic helpers.
        writeln!(&mut out, "#include \"dds/topic/TopicTraits.hpp\"").map_err(fmt_err)?;
        writeln!(&mut out, "#include \"dds/topic/xcdr2.hpp\"").map_err(fmt_err)?;
        writeln!(&mut out, "#include \"dds/topic/xcdr2_md5.hpp\"").map_err(fmt_err)?;
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
    if !probe_structs.is_empty() {
        emit_topic_type_support_specs(&mut out, opts, &probe_structs)?;
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
        Definition::Const(_) => {}
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
            check_identifier(&f.name.text)?;
            writeln!(out, "{}class {};", ctx.indent(), f.name.text).map_err(fmt_err)?;
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
    check_identifier(&m.name.text)?;
    ctx.open_namespace(out, &m.name.text)?;
    for d in &m.definitions {
        emit_definition(out, ctx, d)?;
    }
    ctx.close_namespace(out, &m.name.text)?;
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
                check_identifier(&f.name.text)?;
                writeln!(out, "{}class {};", ctx.indent(), f.name.text).map_err(fmt_err)?;
                Ok(())
            }
            ConstrTypeDecl::Union(UnionDcl::Def(u)) => emit_union(out, ctx, u),
            ConstrTypeDecl::Union(UnionDcl::Forward(f)) => {
                check_identifier(&f.name.text)?;
                writeln!(out, "{}class {};", ctx.indent(), f.name.text).map_err(fmt_err)?;
                Ok(())
            }
            ConstrTypeDecl::Enum(e) => emit_enum(out, ctx, e),
            ConstrTypeDecl::Bitset(b) => {
                check_identifier(&b.name.text)?;
                let ind = ctx.indent();
                let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);
                emit_bitset(out, &ind, &inner, b)
            }
            ConstrTypeDecl::Bitmask(b) => {
                check_identifier(&b.name.text)?;
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
    check_identifier(&s.name.text)?;
    let ind = ctx.indent();

    // §7.2.2.4.8 — `@verbatim(placement=BEFORE_DECLARATION)` before the
    // class header line.
    emit_verbatim_at(out, &ind, &s.annotations, PlacementKind::BeforeDeclaration)?;

    // Class header with an optional inheritance clause.
    if let Some(base) = &s.base {
        let base_str = scoped_to_cpp(base);
        writeln!(out, "{ind}class {} : public {} {{", s.name.text, base_str).map_err(fmt_err)?;
    } else {
        writeln!(out, "{ind}class {} {{", s.name.text).map_err(fmt_err)?;
    }
    writeln!(out, "{ind}public:").map_err(fmt_err)?;

    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_DECLARATION)` as the first
    // line inside the `public:` block.
    emit_verbatim_at(out, &inner, &s.annotations, PlacementKind::BeginDeclaration)?;

    // Default constructor.
    writeln!(out, "{inner}{}() = default;", s.name.text).map_err(fmt_err)?;
    writeln!(out, "{inner}~{}() = default;", s.name.text).map_err(fmt_err)?;

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

    writeln!(out).map_err(fmt_err)?;
    Ok(())
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
    // Flatten members -> (storage_type, field_name) in declaration order.
    let mut fields: Vec<(String, String)> = Vec::new();
    for m in &s.members {
        for decl in &m.declarators {
            let name = decl.name();
            check_identifier(&name.text)?;
            fields.push((member_storage_type(m, decl)?, name.text.clone()));
        }
    }
    if fields.is_empty() {
        return Ok(());
    }

    let params = fields
        .iter()
        .map(|(ty, name)| format!("{ty} {name}"))
        .collect::<Vec<_>>()
        .join(", ");
    let inits = fields
        .iter()
        .map(|(_, name)| format!("{name}_(std::move({name}))"))
        .collect::<Vec<_>>()
        .join(", ");

    writeln!(
        out,
        "{inner}{}({params})\n{inner}    : {inits} {{}}",
        s.name.text
    )
    .map_err(fmt_err)?;
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
        let name = decl.name();
        check_identifier(&name.text)?;
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
            writeln!(
                out,
                "{inner}std::optional<{core_ty}> {}_;{key_marker}",
                name.text
            )
            .map_err(fmt_err)?;
        } else {
            writeln!(out, "{inner}{core_ty} {}_;{key_marker}", name.text).map_err(fmt_err)?;
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
        let name = &decl.name().text;
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
        writeln!(out, "{inner}{storage_ty}& {name}() {{ return {name}_; }}").map_err(fmt_err)?;
        writeln!(
            out,
            "{inner}const {storage_ty}& {name}() const {{ return {name}_; }}"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "{inner}void {name}(const {storage_ty}& value) {{ {name}_ = value; }}"
        )
        .map_err(fmt_err)?;
    }
    Ok(())
}

fn emit_union(out: &mut String, ctx: &mut EmitCtx<'_>, u: &UnionDef) -> Result<(), CppGenError> {
    check_identifier(&u.name.text)?;
    let ind = ctx.indent();
    emit_verbatim_at(out, &ind, &u.annotations, PlacementKind::BeforeDeclaration)?;
    writeln!(out, "{ind}class {} {{", u.name.text).map_err(fmt_err)?;
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
    writeln!(out, "{inner}{}() = default;", u.name.text).map_err(fmt_err)?;
    writeln!(out, "{inner}~{}() = default;", u.name.text).map_err(fmt_err)?;
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
    check_identifier(&e.name.text)?;
    let ind = ctx.indent();
    emit_verbatim_at(out, &ind, &e.annotations, PlacementKind::BeforeDeclaration)?;
    writeln!(out, "{ind}enum class {} : int32_t {{", e.name.text).map_err(fmt_err)?;
    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);
    emit_verbatim_at(out, &inner, &e.annotations, PlacementKind::BeginDeclaration)?;
    for en in &e.enumerators {
        check_identifier(&en.name.text)?;
        writeln!(out, "{inner}{},", en.name.text).map_err(fmt_err)?;
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
    let name = &iface.name.text;
    check_identifier(name)?;
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
    check_identifier(&op.name.text)?;
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
            Ok(format!("{qual} {}", p.name.text))
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
        "{inner}virtual {ret} {}({}) = 0;{raises_comment}",
        op.name.text,
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
    check_identifier(&attr.name.text)?;
    let ty = typespec_to_cpp(&attr.type_spec)?;
    // Getter (every attribute has one).
    writeln!(out, "{inner}virtual {ty} {}() const = 0;", attr.name.text).map_err(fmt_err)?;
    // Setter only for non-readonly.
    if !attr.readonly {
        writeln!(
            out,
            "{inner}virtual void {}(const {ty}& value) = 0;",
            attr.name.text
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
    let name = &v.name.text;
    check_identifier(name)?;
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
                    let n = &d.name().text;
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
                        let n = &d.name().text;
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
            check_identifier(&f.name.text)?;
            let params: Vec<String> = f
                .params
                .iter()
                .map(|p| -> Result<String, CppGenError> {
                    let ty = typespec_to_cpp(&p.type_spec)?;
                    let qual = match p.attribute {
                        ParamAttribute::In => format!("const {ty}&"),
                        ParamAttribute::Out | ParamAttribute::InOut => format!("{ty}&"),
                    };
                    Ok(format!("{qual} {}", p.name.text))
                })
                .collect::<Result<_, _>>()?;
            writeln!(
                out,
                "{inner}virtual std::shared_ptr<{name}> {}({}) = 0;",
                f.name.text,
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
        let alias = &decl.name().text;
        check_identifier(alias)?;
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
    check_identifier(&c.name.text)?;
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
        zerodds_idl::ast::ConstType::String { wide: false } => "std::string".into(),
        zerodds_idl::ast::ConstType::String { wide: true } => "std::wstring".into(),
        zerodds_idl::ast::ConstType::Scoped(s) => scoped_to_cpp(s),
        zerodds_idl::ast::ConstType::Fixed => {
            // §7.2.4.2.4 — fixed constant without a digits/scale annotation;
            // we emit it as an opaque wrapper (the caller annotates the type
            // via a separate `typedef fixed<D,S> Name;`).
            "::dds::core::Fixed<31, 0>".into()
        }
    };
    let val = const_expr_to_cpp(&c.value);
    writeln!(out, "{ind}constexpr {cpp_ty} {} = {val};", c.name.text).map_err(fmt_err)?;
    Ok(())
}

fn emit_exception(
    out: &mut String,
    ctx: &mut EmitCtx<'_>,
    e: &ExceptDecl,
) -> Result<(), CppGenError> {
    check_identifier(&e.name.text)?;
    let ind = ctx.indent();
    writeln!(out, "{ind}class {} : public std::exception {{", e.name.text).map_err(fmt_err)?;
    writeln!(out, "{ind}public:").map_err(fmt_err)?;
    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);
    writeln!(out, "{inner}{}() = default;", e.name.text).map_err(fmt_err)?;
    writeln!(out, "{inner}~{}() override = default;", e.name.text).map_err(fmt_err)?;
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
            let name = &decl.name().text;
            check_identifier(name)?;
            writeln!(out, "{inner}{cpp_ty} {name}_;").map_err(fmt_err)?;
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
                let n = const_expr_to_usize(size).unwrap_or_default();
                out = format!("std::array<{out}, {n}>");
            }
            Ok(out)
        }
    }
}

/// zerodds-lint: recursion-depth 64 (Parser/AST-Walk; bounded by IDL nesting)
pub(crate) fn typespec_to_cpp(ts: &TypeSpec) -> Result<String, CppGenError> {
    match ts {
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
            // Spec idl4-cpp §7.3: `any` -> `omg::types::Any`. We emit it
            // as the ZeroDDS-equivalent `dds::core::Any` class
            // (a reflective container, runtime implementation in the
            // `dds-core` crate).
            Ok("::dds::core::Any".into())
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
    let parts: Vec<String> = s.parts.iter().map(|p| p.text.clone()).collect();
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
        LiteralKind::Boolean => l.raw.clone(),
        LiteralKind::Integer | LiteralKind::Floating => l.raw.clone(),
        LiteralKind::Char => l.raw.clone(),
        LiteralKind::WideChar => l.raw.clone(),
        LiteralKind::String => l.raw.clone(),
        LiteralKind::WideString => l.raw.clone(),
        LiteralKind::Fixed => l.raw.clone(),
    }
}

fn const_expr_to_usize(e: &ConstExpr) -> Option<usize> {
    match e {
        ConstExpr::Literal(l) if l.kind == LiteralKind::Integer => l.raw.parse::<usize>().ok(),
        _ => None,
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
    if has_named_annotation(anns, "final") {
        Extensibility::Final
    } else if has_named_annotation(anns, "mutable") {
        Extensibility::Mutable
    } else if has_named_annotation(anns, "appendable") {
        Extensibility::Appendable
    } else {
        // Default per zerodds-xcdr2-cpp-1.0 §6: appendable.
        Extensibility::Appendable
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

fn emit_topic_type_support_specs(
    out: &mut String,
    opts: &CppGenOptions,
    structs: &[(String, &StructDef)],
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
    for (fqn, s) in structs {
        let cpp_fqn = if user_prefix.is_empty() {
            format!("::{fqn}")
        } else {
            format!("::{user_prefix}::{fqn}")
        };
        emit_topic_type_support_for(out, &cpp_fqn, fqn, s)?;
    }

    writeln!(out, "}} // namespace topic").map_err(fmt_err)?;
    writeln!(out, "}} // namespace dds").map_err(fmt_err)?;
    Ok(())
}

/// Returns true if the member annotations are encode-safe (the codegen
/// can produce wire bytes). False for `@shared` (heap indirection, not
/// yet supported). `@optional` is allowed.
fn member_codegen_supported(m: &Member) -> bool {
    !has_shared_annotation(&m.annotations)
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
            TypeSpec::Scoped(s) => scoped_is_enum(s) || scoped_struct(s).is_some(),
            TypeSpec::Sequence(_) => typespec_supported(&seq.elem),
            TypeSpec::Map(m) => typespec_supported(&m.key) && typespec_supported(&m.value),
            _ => false,
        },
        // A `Scoped` member resolving to an enum (→ int32) or to a directly-
        // encodable struct of ANY extensibility (@final → recursed inline;
        // @appendable/@mutable → spliced, see `scoped_struct`) is supported.
        // The Sequence-element arm above mirrors this (each non-final element is
        // 4-aligned + spliced/sub-decoded per its own DHEADER).
        TypeSpec::Scoped(s) => scoped_is_enum(s) || scoped_struct(s).is_some(),
        // map<K,V>: supported iff both key and value are themselves supported
        // (encode/decode recurse through emit_value_write/read per entry).
        TypeSpec::Map(m) => typespec_supported(&m.key) && typespec_supported(&m.value),
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
    // Monotonic counter for unique nested-struct decode temp-var names
    // (`__ns<N>`), so nested-nested decodes do not shadow each other.
    static NEST_CTR: core::cell::Cell<u32> = const { core::cell::Cell::new(0) };
}

fn next_nest_id() -> u32 {
    NEST_CTR.with(|c| {
        let v = c.get();
        c.set(v.wrapping_add(1));
        v
    })
}

fn set_type_registry(spec: &Specification) {
    let mut enums = BTreeSet::new();
    let mut structs = BTreeSet::new();
    let mut defs = BTreeMap::new();
    collect_type_names(&spec.definitions, &mut enums, &mut structs, &mut defs);
    ENUM_NAMES.with(|r| *r.borrow_mut() = enums);
    STRUCT_NAMES.with(|r| *r.borrow_mut() = structs);
    STRUCT_DEFS.with(|r| *r.borrow_mut() = defs);
}

/// zerodds-lint: recursion-depth 64 (module/type tree; bounded by IDL nesting)
fn collect_type_names(
    defs: &[Definition],
    enums: &mut BTreeSet<String>,
    structs: &mut BTreeSet<String>,
    struct_defs: &mut BTreeMap<String, StructDef>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                collect_type_names(&m.definitions, enums, structs, struct_defs)
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                enums.insert(e.name.text.clone());
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                structs.insert(s.name.text.clone());
                struct_defs.insert(s.name.text.clone(), s.clone());
            }
            _ => {}
        }
    }
}

/// `true` if `s` (a member's scoped type name) unambiguously names an enum.
fn scoped_is_enum(s: &ScopedName) -> bool {
    let Some(last) = s.parts.last().map(|p| p.text.clone()) else {
        return false;
    };
    let is_enum = ENUM_NAMES.with(|r| r.borrow().contains(&last));
    let is_struct = STRUCT_NAMES.with(|r| r.borrow().contains(&last));
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
    let last = s.parts.last()?.text.clone();
    let def = STRUCT_DEFS.with(|r| r.borrow().get(&last).cloned())?;
    let ext = struct_extensibility(&def.annotations);
    let all_encodable = def.members.iter().all(|m| {
        m.declarators.len() == 1
            && matches!(m.declarators.first(), Some(Declarator::Simple(_)))
            && typespec_supported(&m.type_spec)
    });
    if all_encodable {
        Some((def, ext))
    } else {
        None
    }
}

fn emit_topic_type_support_for(
    out: &mut String,
    cpp_fqn: &str,
    type_name: &str,
    s: &StructDef,
) -> Result<(), CppGenError> {
    let ext = struct_extensibility(&s.annotations);

    writeln!(out, "template <>").map_err(fmt_err)?;
    writeln!(out, "struct topic_type_support<{cpp_fqn}> {{").map_err(fmt_err)?;
    writeln!(
        out,
        "    static const char* type_name() {{ return \"{type_name}\"; }}"
    )
    .map_err(fmt_err)?;

    // is_keyed
    let is_keyed = s.members.iter().any(|m| has_key_annotation(&m.annotations));
    writeln!(
        out,
        "    static constexpr bool is_keyed() {{ return {}; }}",
        if is_keyed { "true" } else { "false" }
    )
    .map_err(fmt_err)?;

    // extensibility
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

    // encode (LE)
    emit_encode_fn(out, cpp_fqn, s, ext, /*be=*/ false)?;
    // encode_be (BE)
    emit_encode_fn(out, cpp_fqn, s, ext, /*be=*/ true)?;
    // decode (LE)
    emit_decode_fn(out, cpp_fqn, s, ext)?;
    // key_hash (BE Plain-CDR2 of @key members + MD5)
    emit_key_hash_fn(out, cpp_fqn, s, is_keyed)?;

    writeln!(out, "}};").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_encode_fn(
    out: &mut String,
    cpp_fqn: &str,
    s: &StructDef,
    ext: Extensibility,
    be: bool,
) -> Result<(), CppGenError> {
    // Suffix for write helpers: write_le or write_be, write_string or write_string_be.
    let endian_suffix = if be { "be" } else { "le" };

    if be {
        writeln!(
            out,
            "    static std::vector<uint8_t> encode_be(const {cpp_fqn}& __v) {{"
        )
        .map_err(fmt_err)?;
    } else {
        // XCDR2 default delegator + version-aware encode. XCDR2 caps
        // 8-byte primitive alignment to 4 (XTypes 1.3 §7.4.3.4.2)
        // — symmetric to `decode(.., XcdrVersion)`. XCDR2 is the
        // ZeroDDS system default (= dcps DEFAULT_OFFER [XCDR2], encap
        // 0x07/0x09/0x0b); for legacy XCDR1 call
        // `encode(__v, XcdrVersion::Xcdr1)` explicitly.
        writeln!(
            out,
            "    static std::vector<uint8_t> encode(const {cpp_fqn}& __v) {{"
        )
        .map_err(fmt_err)?;
        writeln!(
            out,
            "        return encode(__v, ::dds::topic::xcdr2::XcdrVersion::Xcdr2);"
        )
        .map_err(fmt_err)?;
        writeln!(out, "    }}").map_err(fmt_err)?;
        writeln!(
            out,
            "    static std::vector<uint8_t> encode(const {cpp_fqn}& __v, \
             ::dds::topic::xcdr2::XcdrVersion __repr) {{"
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "        std::vector<uint8_t> __out;").map_err(fmt_err)?;
    writeln!(out, "        (void)__v;").map_err(fmt_err)?;
    if !be {
        writeln!(
            out,
            "        const size_t __max_align = ::dds::topic::xcdr2::xcdr_max_align(__repr);"
        )
        .map_err(fmt_err)?;
        writeln!(out, "        (void)__max_align;").map_err(fmt_err)?;
    }

    match ext {
        Extensibility::Final => {
            // Plain-CDR2, no DHEADER, alignment relative to buffer start.
            // origin = 0.
            writeln!(out, "        const size_t __origin = 0;").map_err(fmt_err)?;
            writeln!(out, "        (void)__origin;").map_err(fmt_err)?;
            for m in &s.members {
                emit_plain_member_encode(out, m, endian_suffix, "__origin")?;
            }
        }
        Extensibility::Appendable => {
            writeln!(
                out,
                "        const auto __dh = ::dds::topic::xcdr2::dheader_begin(__out);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        const size_t __origin = __out.size();").map_err(fmt_err)?;
            writeln!(out, "        (void)__origin;").map_err(fmt_err)?;
            for m in &s.members {
                emit_plain_member_encode(out, m, endian_suffix, "__origin")?;
            }
            writeln!(
                out,
                "        ::dds::topic::xcdr2::dheader_end(__out, __dh);"
            )
            .map_err(fmt_err)?;
        }
        Extensibility::Mutable => {
            writeln!(
                out,
                "        const auto __scope = ::dds::topic::xcdr2::mutable_begin(__out);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        const size_t __origin = __scope.origin;").map_err(fmt_err)?;
            for m in &s.members {
                emit_mutable_member_encode(out, m, endian_suffix)?;
            }
            writeln!(
                out,
                "        ::dds::topic::xcdr2::mutable_end(__out, __scope);"
            )
            .map_err(fmt_err)?;
        }
    }

    writeln!(out, "        return __out;").map_err(fmt_err)?;
    writeln!(out, "    }}").map_err(fmt_err)?;
    Ok(())
}

/// Emit Plain-CDR2 (LE/BE) encoding for one member at the current
/// position; alignment relative to `origin`.
fn emit_plain_member_encode(
    out: &mut String,
    m: &Member,
    endian: &str,
    origin: &str,
) -> Result<(), CppGenError> {
    if !member_codegen_supported(m) {
        for decl in &m.declarators {
            let name = &decl.name().text;
            writeln!(
                out,
                "        // xcdr2: @shared member '{name}' not supported (skip)"
            )
            .map_err(fmt_err)?;
        }
        return Ok(());
    }
    let is_optional = has_optional_annotation(&m.annotations);
    for decl in &m.declarators {
        let name = &decl.name().text;
        // 1-D fixed array of a leaf type (primitive / string / wstring): XCDR2
        // encodes N contiguous elements, no length prefix. Multi-dim arrays and
        // arrays of struct/sequence remain a follow-up (need the type registry /
        // recursion — see idl-cpp-xcdr2-encoder-gaps.md).
        if let Declarator::Array(arr) = decl {
            let prim = matches!(m.type_spec, TypeSpec::Primitive(_));
            let leaf_1d = arr.sizes.len() == 1
                && matches!(m.type_spec, TypeSpec::Primitive(_) | TypeSpec::String(_));
            if leaf_1d {
                writeln!(out, "        for (const auto& __ae : __v.{name}()) {{")
                    .map_err(fmt_err)?;
                emit_value_write(out, &m.type_spec, "__ae", endian, origin, "        ")?;
                writeln!(out, "        }}").map_err(fmt_err)?;
            } else if prim && arr.sizes.len() >= 2 {
                // Multi-dim array of a primitive (XTypes §7.4.3): row-major, fixed
                // size, NO DHEADER (primitive elements). The C++ type is a nested
                // std::array, so N nested range-for loops reach the innermost cell.
                let n = arr.sizes.len();
                let mut acc = format!("__v.{name}()");
                let mut ind = String::from("        ");
                for d in 0..n {
                    let lv = format!("__a{d}");
                    writeln!(out, "{ind}for (const auto& {lv} : {acc}) {{").map_err(fmt_err)?;
                    acc = lv;
                    ind.push_str("    ");
                }
                emit_value_write(out, &m.type_spec, &acc, endian, origin, &ind)?;
                for _ in 0..n {
                    ind.truncate(ind.len() - 4);
                    writeln!(out, "{ind}}}").map_err(fmt_err)?;
                }
            } else if matches!(&m.type_spec, TypeSpec::Scoped(s) if scoped_is_enum(s) || scoped_final_struct(s).is_some())
                || (matches!(m.type_spec, TypeSpec::String(_)) && arr.sizes.len() >= 2)
            {
                // Array of NON-primitive elements (enum / @final struct, any dims;
                // string only for >=2 dims — 1-D string keeps the legacy no-DHEADER
                // leaf path above): one DHEADER (XTypes §7.4.3.5) wrapping N
                // elements inline, row-major, NO count. N nested range-for loops.
                let n = arr.sizes.len();
                writeln!(out, "        {{").map_err(fmt_err)?;
                writeln!(
                    out,
                    "        const auto __arr_dh = ::dds::topic::xcdr2::dheader_begin(__out);"
                )
                .map_err(fmt_err)?;
                let mut acc = format!("__v.{name}()");
                let mut ind = String::from("        ");
                for d in 0..n {
                    let lv = format!("__a{d}");
                    writeln!(out, "{ind}for (const auto& {lv} : {acc}) {{").map_err(fmt_err)?;
                    acc = lv;
                    ind.push_str("    ");
                }
                emit_value_write(out, &m.type_spec, &acc, endian, origin, &ind)?;
                for _ in 0..n {
                    ind.truncate(ind.len() - 4);
                    writeln!(out, "{ind}}}").map_err(fmt_err)?;
                }
                writeln!(
                    out,
                    "        ::dds::topic::xcdr2::dheader_end(__out, __arr_dh);"
                )
                .map_err(fmt_err)?;
                writeln!(out, "        }}").map_err(fmt_err)?;
            } else {
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
                "        // xcdr2: member '{name}' not supported (nested/enum/map/fixed; skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        if is_optional {
            // Final/appendable: 1-byte present-flag, then the value if present.
            writeln!(out, "        if (__v.{name}().has_value()) {{").map_err(fmt_err)?;
            writeln!(out, "            __out.push_back(uint8_t{{1}});").map_err(fmt_err)?;
            emit_value_write(
                out,
                &m.type_spec,
                &format!("(*__v.{name}())"),
                endian,
                origin,
                "        ",
            )?;
            writeln!(out, "        }} else {{").map_err(fmt_err)?;
            writeln!(out, "            __out.push_back(uint8_t{{0}});").map_err(fmt_err)?;
            writeln!(out, "        }}").map_err(fmt_err)?;
        } else {
            emit_value_write(
                out,
                &m.type_spec,
                &format!("__v.{name}()"),
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
    match ts {
        TypeSpec::Primitive(PrimitiveType::Boolean) => {
            writeln!(
                out,
                "{pre}::dds::topic::xcdr2::write_bool(__out, {access});"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(PrimitiveType::Octet) => {
            writeln!(out, "{pre}::dds::topic::xcdr2::write_u8(__out, {access});")
                .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(p) => {
            let cpp_ty = primitive_to_cpp(*p);
            if endian == "be" {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_be_origin<{cpp_ty}>(__out, {origin}, {access});"
                )
                .map_err(fmt_err)?;
            } else {
                // LE: representation-aware (XCDR2 deckelt 8-Byte-Align auf 4).
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_le_origin<{cpp_ty}>(__out, {origin}, {access}, __max_align);"
                )
                .map_err(fmt_err)?;
            }
        }
        TypeSpec::String(s) if !s.wide => {
            // Bounded narrow `string<N>` (DDS-XTypes §7.4.3): byte-length check
            // (std::string::size = bytes = CDR wire length).
            if let Some(b) = &s.bound {
                let bv = const_expr_to_cpp(b);
                writeln!(
                    out,
                    "{pre}if ({access}.size() > {bv}) throw std::length_error(\"bounded string length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            if endian == "be" {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_string_be(__out, {access});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_string_origin(__out, {origin}, {access}, __max_align);"
                )
                .map_err(fmt_err)?;
            }
        }
        TypeSpec::String(s) if s.wide => {
            // Bounded `wstring<N>` (DDS-XTypes §7.4.3): bound is in wide chars
            // (std::wstring::size). Wire = UTF-16 (conformance §9.1).
            if let Some(b) = &s.bound {
                let bv = const_expr_to_cpp(b);
                writeln!(
                    out,
                    "{pre}if ({access}.size() > {bv}) throw std::length_error(\"bounded wstring length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            if endian == "be" {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_wstring_be(__out, {access});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_wstring_origin(__out, {origin}, {access}, __max_align);"
                )
                .map_err(fmt_err)?;
            }
        }
        TypeSpec::Sequence(seq) => {
            // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = encode
            // error. The encode returns a vector (no Result channel), so this
            // throws — strict vendors (OpenDDS) reject on the wire likewise.
            if let Some(b) = &seq.bound {
                let bv = const_expr_to_cpp(b);
                writeln!(
                    out,
                    "{pre}if ({access}.size() > {bv}) throw std::length_error(\"bounded sequence length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            if matches!(&*seq.elem, TypeSpec::Primitive(PrimitiveType::Octet)) {
                // sequence<octet>: u32 length + raw byte block, no per-byte loop.
                if endian == "be" {
                    writeln!(out, "{pre}::dds::topic::xcdr2::write_be<uint32_t>(__out, static_cast<uint32_t>({access}.size()));").map_err(fmt_err)?;
                } else {
                    writeln!(out, "{pre}::dds::topic::xcdr2::write_le_origin<uint32_t>(__out, {origin}, static_cast<uint32_t>({access}.size()), __max_align);").map_err(fmt_err)?;
                }
                writeln!(
                    out,
                    "{pre}__out.insert(__out.end(), {access}.begin(), {access}.end());"
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
                writeln!(
                    out,
                    "{pre}const auto __seq_dh = ::dds::topic::xcdr2::dheader_begin(__out);"
                )
                .map_err(fmt_err)?;
            }
            let count_call = if endian == "be" {
                format!(
                    "{pre}::dds::topic::xcdr2::write_be<uint32_t>(__out, static_cast<uint32_t>({access}.size()));"
                )
            } else {
                format!(
                    "{pre}::dds::topic::xcdr2::write_le_origin<uint32_t>(__out, {origin}, static_cast<uint32_t>({access}.size()), __max_align);"
                )
            };
            writeln!(out, "{count_call}").map_err(fmt_err)?;
            writeln!(out, "{pre}for (const auto& __e : {access}) {{").map_err(fmt_err)?;
            let elem_indent = format!("{pre}    ");
            match &*seq.elem {
                TypeSpec::Primitive(PrimitiveType::Boolean) => {
                    writeln!(
                        out,
                        "{elem_indent}::dds::topic::xcdr2::write_bool(__out, __e);"
                    )
                    .map_err(fmt_err)?;
                }
                TypeSpec::Primitive(PrimitiveType::Octet) => {
                    writeln!(
                        out,
                        "{elem_indent}::dds::topic::xcdr2::write_u8(__out, __e);"
                    )
                    .map_err(fmt_err)?;
                }
                TypeSpec::Primitive(p) => {
                    let cpp_ty = primitive_to_cpp(*p);
                    if endian == "be" {
                        writeln!(
                            out,
                            "{elem_indent}::dds::topic::xcdr2::write_be<{cpp_ty}>(__out, __e);"
                        )
                        .map_err(fmt_err)?;
                    } else {
                        writeln!(
                            out,
                            "{elem_indent}::dds::topic::xcdr2::write_le_origin<{cpp_ty}>(__out, {origin}, __e, __max_align);"
                        )
                        .map_err(fmt_err)?;
                    }
                }
                TypeSpec::String(s) if !s.wide => {
                    if endian == "be" {
                        writeln!(
                            out,
                            "{elem_indent}::dds::topic::xcdr2::write_string_be(__out, __e);"
                        )
                        .map_err(fmt_err)?;
                    } else {
                        writeln!(
                            out,
                            "{elem_indent}::dds::topic::xcdr2::write_string_origin(__out, {origin}, __e, __max_align);"
                        )
                        .map_err(fmt_err)?;
                    }
                }
                // wide string (wstring): recurse for the BOM/octet-length wire form.
                TypeSpec::String(_) => {
                    emit_value_write(out, &seq.elem, "__e", endian, origin, &elem_indent)?;
                }
                // enum (-> int32) and nested struct of ANY extensibility: recurse
                // through emit_value_write, identical to member-level encoding —
                // @final inlines (no DHEADER), @appendable/@mutable pad-to-4 +
                // splice the element's own [DHEADER+body] (XTypes §7.4.3.5).
                TypeSpec::Scoped(sc) if scoped_is_enum(sc) || scoped_struct(sc).is_some() => {
                    emit_value_write(out, &seq.elem, "__e", endian, origin, &elem_indent)?;
                }
                // nested sequence (sequence<sequence<...>>): recurse — the inner
                // sequence emits its own DHEADER (XTypes §7.4.3.5).
                TypeSpec::Sequence(_) => {
                    emit_value_write(out, &seq.elem, "__e", endian, origin, &elem_indent)?;
                }
                // map element (sequence<map<K,V>>): recurse — the map emits its
                // own DHEADER.
                TypeSpec::Map(_) => {
                    emit_value_write(out, &seq.elem, "__e", endian, origin, &elem_indent)?;
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
                    "{pre}::dds::topic::xcdr2::dheader_end(__out, __seq_dh);"
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
                let bv = const_expr_to_cpp(b);
                writeln!(
                    out,
                    "{pre}if ({access}.size() > {bv}) throw std::length_error(\"bounded map length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{pre}{{").map_err(fmt_err)?;
            writeln!(
                out,
                "{pre}const auto __map_dh = ::dds::topic::xcdr2::dheader_begin(__out);"
            )
            .map_err(fmt_err)?;
            if endian == "be" {
                writeln!(out, "{pre}::dds::topic::xcdr2::write_be<uint32_t>(__out, static_cast<uint32_t>({access}.size()));").map_err(fmt_err)?;
            } else {
                writeln!(out, "{pre}::dds::topic::xcdr2::write_le_origin<uint32_t>(__out, {origin}, static_cast<uint32_t>({access}.size()), __max_align);").map_err(fmt_err)?;
            }
            writeln!(out, "{pre}for (const auto& __kv : {access}) {{").map_err(fmt_err)?;
            let kv_indent = format!("{pre}    ");
            emit_value_write(out, &m.key, "__kv.first", endian, origin, &kv_indent)?;
            emit_value_write(out, &m.value, "__kv.second", endian, origin, &kv_indent)?;
            writeln!(out, "{pre}}}").map_err(fmt_err)?;
            writeln!(
                out,
                "{pre}::dds::topic::xcdr2::dheader_end(__out, __map_dh);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{pre}}}").map_err(fmt_err)?;
        }
        // enum member: encode as its int32 underlying type (Spec §7.4.1.4.2).
        TypeSpec::Scoped(s) if scoped_is_enum(s) => {
            if endian == "be" {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_be<int32_t>(__out, static_cast<int32_t>({access}));"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_le_origin<int32_t>(__out, {origin}, static_cast<int32_t>({access}), __max_align);"
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
            match ext {
                Extensibility::Final => {
                    for sm in &def.members {
                        let sm_name = &sm.declarators[0].name().text;
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
                    let cpp = scoped_to_cpp(sc);
                    let id = next_nest_id();
                    writeln!(out, "{pre}{{").map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{pre}    ::dds::topic::xcdr2::pad_to_from_origin(__out, {origin}, 4);"
                    )
                    .map_err(fmt_err)?;
                    if endian == "be" {
                        writeln!(
                            out,
                            "{pre}    auto __nsb{id} = ::dds::topic::topic_type_support<{cpp}>::encode_be({access});"
                        )
                        .map_err(fmt_err)?;
                    } else {
                        writeln!(
                            out,
                            "{pre}    auto __nsb{id} = ::dds::topic::topic_type_support<{cpp}>::encode({access}, __repr);"
                        )
                        .map_err(fmt_err)?;
                    }
                    writeln!(
                        out,
                        "{pre}    __out.insert(__out.end(), __nsb{id}.begin(), __nsb{id}.end());"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "{pre}}}").map_err(fmt_err)?;
                }
            }
        }
        _ => {
            writeln!(out, "{pre}// xcdr2: member type not supported (skip)").map_err(fmt_err)?;
        }
    }
    Ok(())
}

/// Emit Mutable-EMHEADER + body for one member.
fn emit_mutable_member_encode(
    out: &mut String,
    m: &Member,
    endian: &str,
) -> Result<(), CppGenError> {
    if !member_codegen_supported(m) {
        for decl in &m.declarators {
            let name = &decl.name().text;
            writeln!(
                out,
                "        // xcdr2: @shared member '{name}' not supported (skip)"
            )
            .map_err(fmt_err)?;
        }
        return Ok(());
    }
    let is_optional = has_optional_annotation(&m.annotations);
    let must_understand = has_named_annotation(&m.annotations, "must_understand");
    let id_override = find_uint_annotation(&m.annotations, "id");
    let mu_lit = if must_understand { "true" } else { "false" };

    for (idx, decl) in m.declarators.iter().enumerate() {
        let name = &decl.name().text;
        if !matches!(decl, Declarator::Simple(_)) {
            writeln!(
                out,
                "        // xcdr2: array member '{name}' not supported (skip)"
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
        // Member-id: explicit @id override; otherwise auto-id (declaration-order).
        // For positional: same id_override applies to all declarators in this Member.
        // (IDL convention: @id() applies to the whole declaration; we replicate.)
        let _ = idx;
        let id_expr = match id_override {
            Some(id) => id.to_string(),
            None => format!("0x{:x}u", auto_id_for(name)),
        };
        if is_optional {
            // Mutable + optional: skip EMHEADER if absent.
            writeln!(out, "        if (__v.{name}().has_value()) {{").map_err(fmt_err)?;
            emit_mutable_value_emit(
                out,
                &m.type_spec,
                &format!("(*__v.{name}())"),
                &id_expr,
                mu_lit,
                endian,
                "            ",
            )?;
            writeln!(out, "        }}").map_err(fmt_err)?;
        } else {
            emit_mutable_value_emit(
                out,
                &m.type_spec,
                &format!("__v.{name}()"),
                &id_expr,
                mu_lit,
                endian,
                "        ",
            )?;
        }
    }
    Ok(())
}

/// Auto-id from member name (XTypes "auto" id mode: name-hash truncated to 28 bits).
/// Default mode is "sequential" but we use name-hash for stability across re-orderings;
/// caller should normally provide @id(N) explicitly.
fn auto_id_for(name: &str) -> u32 {
    // FNV-1a 32-bit; truncate to 28 bits to fit EMHEADER member-id.
    let mut h: u32 = 0x811C9DC5;
    for b in name.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x01000193);
    }
    h & 0x0FFF_FFFF
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
    match ts {
        TypeSpec::Primitive(PrimitiveType::Boolean) => {
            writeln!(
                out,
                "{indent}::dds::topic::xcdr2::emheader_u8(__out, __origin, {id_expr}, {mu_lit}, static_cast<uint8_t>({access} ? 1 : 0));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(PrimitiveType::Octet) => {
            writeln!(
                out,
                "{indent}::dds::topic::xcdr2::emheader_u8(__out, __origin, {id_expr}, {mu_lit}, {access});"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(p) => {
            let cpp_ty = primitive_to_cpp(*p);
            // Decide LC by size.
            let size = primitive_size(*p);
            match size {
                2 => {
                    writeln!(
                        out,
                        "{indent}::dds::topic::xcdr2::emheader_2<{cpp_ty}>(__out, __origin, {id_expr}, {mu_lit}, {access});"
                    )
                    .map_err(fmt_err)?;
                }
                4 => {
                    writeln!(
                        out,
                        "{indent}::dds::topic::xcdr2::emheader_4<{cpp_ty}>(__out, __origin, {id_expr}, {mu_lit}, {access});"
                    )
                    .map_err(fmt_err)?;
                }
                8 => {
                    writeln!(
                        out,
                        "{indent}::dds::topic::xcdr2::emheader_8<{cpp_ty}>(__out, __origin, {id_expr}, {mu_lit}, {access});"
                    )
                    .map_err(fmt_err)?;
                }
                _ => {
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
                let bv = const_expr_to_cpp(b);
                writeln!(
                    out,
                    "{indent}if ({access}.size() > {bv}) throw std::length_error(\"bounded string length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            // EMHEADER LC=3 with NEXTINT, then string body inline.
            writeln!(
                out,
                "{indent}{{ const auto __sub = ::dds::topic::xcdr2::emheader_nextint_begin(__out, __origin, {id_expr}, {mu_lit});"
            )
            .map_err(fmt_err)?;
            // Inside the NEXTINT block, the body itself uses origin = __sub.body_start
            // (string-len align is 4, count starts at body_start which is 4-aligned).
            let body_endian = if endian == "be" { "be" } else { "le" };
            let _ = body_endian;
            writeln!(
                out,
                "{indent}    {{ const auto __body_origin = __sub.body_start; (void)__body_origin;"
            )
            .map_err(fmt_err)?;
            if endian == "be" {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_string_be(__out, {access});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_string_origin(__out, __body_origin, {access}, __max_align);"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{indent}    }}").map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    ::dds::topic::xcdr2::emheader_nextint_end(__out, __sub); }}"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::String(s) if s.wide => {
            // Bounded `wstring<N>` (DDS-XTypes §7.4.3): wide-char-length check.
            if let Some(b) = &s.bound {
                let bv = const_expr_to_cpp(b);
                writeln!(
                    out,
                    "{indent}if ({access}.size() > {bv}) throw std::length_error(\"bounded wstring length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            // EMHEADER LC=3 with NEXTINT, then wstring body inline (UTF-16).
            writeln!(
                out,
                "{indent}{{ const auto __sub = ::dds::topic::xcdr2::emheader_nextint_begin(__out, __origin, {id_expr}, {mu_lit});"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    {{ const auto __body_origin = __sub.body_start; (void)__body_origin;"
            )
            .map_err(fmt_err)?;
            if endian == "be" {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_wstring_be(__out, {access});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_wstring_origin(__out, __body_origin, {access}, __max_align);"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{indent}    }}").map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    ::dds::topic::xcdr2::emheader_nextint_end(__out, __sub); }}"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Sequence(seq) => {
            // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = throw.
            if let Some(b) = &seq.bound {
                let bv = const_expr_to_cpp(b);
                writeln!(
                    out,
                    "{indent}if ({access}.size() > {bv}) throw std::length_error(\"bounded sequence length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            writeln!(
                out,
                "{indent}{{ const auto __sub = ::dds::topic::xcdr2::emheader_nextint_begin(__out, __origin, {id_expr}, {mu_lit});"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    {{ const auto __body_origin = __sub.body_start; (void)__body_origin;"
            )
            .map_err(fmt_err)?;
            // XTypes §7.4.3.5: a non-primitive-element sequence carries its OWN
            // DHEADER even inside a mutable EMHEADER NEXTINT frame (the Rust
            // reference encoder writes EMHEADER+NEXTINT+DHEADER+count+elements;
            // Cyclone-interop-verified). Primitive-element sequences carry none.
            let seq_inner_dh = !matches!(&*seq.elem, TypeSpec::Primitive(_));
            if seq_inner_dh {
                writeln!(
                    out,
                    "{indent}      const auto __seq_dh = ::dds::topic::xcdr2::dheader_begin(__out);"
                )
                .map_err(fmt_err)?;
            }
            if endian == "be" {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_be<uint32_t>(__out, static_cast<uint32_t>({access}.size()));"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_le_origin<uint32_t>(__out, __body_origin, static_cast<uint32_t>({access}.size()), __max_align);"
                )
                .map_err(fmt_err)?;
            }
            if matches!(&*seq.elem, TypeSpec::Primitive(PrimitiveType::Octet)) {
                // sequence<octet>: raw byte block instead of a per-byte loop.
                writeln!(
                    out,
                    "{indent}      __out.insert(__out.end(), {access}.begin(), {access}.end());"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(out, "{indent}      for (const auto& __e : {access}) {{")
                    .map_err(fmt_err)?;
                match &*seq.elem {
                    TypeSpec::Primitive(PrimitiveType::Boolean) => {
                        writeln!(
                            out,
                            "{indent}        ::dds::topic::xcdr2::write_bool(__out, __e);"
                        )
                        .map_err(fmt_err)?;
                    }
                    TypeSpec::Primitive(PrimitiveType::Octet) => {
                        writeln!(
                            out,
                            "{indent}        ::dds::topic::xcdr2::write_u8(__out, __e);"
                        )
                        .map_err(fmt_err)?;
                    }
                    TypeSpec::Primitive(p) => {
                        let cpp_ty = primitive_to_cpp(*p);
                        if endian == "be" {
                            writeln!(
                            out,
                            "{indent}        ::dds::topic::xcdr2::write_be<{cpp_ty}>(__out, __e);"
                        )
                        .map_err(fmt_err)?;
                        } else {
                            writeln!(out, "{indent}        ::dds::topic::xcdr2::write_le_origin<{cpp_ty}>(__out, __body_origin, __e, __max_align);").map_err(fmt_err)?;
                        }
                    }
                    TypeSpec::String(s) if !s.wide => {
                        if endian == "be" {
                            writeln!(
                                out,
                                "{indent}        ::dds::topic::xcdr2::write_string_be(__out, __e);"
                            )
                            .map_err(fmt_err)?;
                        } else {
                            writeln!(out, "{indent}        ::dds::topic::xcdr2::write_string_origin(__out, __body_origin, __e, __max_align);").map_err(fmt_err)?;
                        }
                    }
                    // wstring / enum / nested struct (any extensibility) elements:
                    // recurse with the EMHEADER body-origin (identical to the
                    // plain-path arms; non-final elements pad-to-4 + splice).
                    TypeSpec::String(_) => {
                        emit_value_write(
                            out,
                            &seq.elem,
                            "__e",
                            endian,
                            "__body_origin",
                            &format!("{indent}        "),
                        )?;
                    }
                    TypeSpec::Scoped(sc) if scoped_is_enum(sc) || scoped_struct(sc).is_some() => {
                        emit_value_write(
                            out,
                            &seq.elem,
                            "__e",
                            endian,
                            "__body_origin",
                            &format!("{indent}        "),
                        )?;
                    }
                    // nested sequence / map element (each emits its own DHEADER).
                    TypeSpec::Sequence(_) | TypeSpec::Map(_) => {
                        emit_value_write(
                            out,
                            &seq.elem,
                            "__e",
                            endian,
                            "__body_origin",
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
                    "{indent}      ::dds::topic::xcdr2::dheader_end(__out, __seq_dh);"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{indent}    }}").map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    ::dds::topic::xcdr2::emheader_nextint_end(__out, __sub); }}"
            )
            .map_err(fmt_err)?;
        }
        // enum member: 4-byte int32 -> compact LC=2 EMHEADER (no NEXTINT).
        TypeSpec::Scoped(s) if scoped_is_enum(s) => {
            writeln!(
                out,
                "{indent}::dds::topic::xcdr2::emheader_4<int32_t>(__out, __origin, {id_expr}, {mu_lit}, static_cast<int32_t>({access}));"
            )
            .map_err(fmt_err)?;
        }
        // nested struct member as a @mutable member: LC=4 NEXTINT frame. @final:
        // wrap the inline (no inner DHEADER) body. @appendable/@mutable: splice
        // the nested type's own encoding (it carries its own inner DHEADER) into
        // the NEXTINT body — byte-identical to the Rust reference (a non-final
        // nested struct contributes its DHEADER inside the member frame).
        TypeSpec::Scoped(sc) if scoped_struct(sc).is_some() => {
            let Some((def, ext)) = scoped_struct(sc) else {
                return Ok(());
            };
            writeln!(
                out,
                "{indent}{{ const auto __sub = ::dds::topic::xcdr2::emheader_nextint_begin(__out, __origin, {id_expr}, {mu_lit});"
            )
            .map_err(fmt_err)?;
            match ext {
                Extensibility::Final => {
                    writeln!(
                        out,
                        "{indent}    {{ const auto __body_origin = __sub.body_start; (void)__body_origin;"
                    )
                    .map_err(fmt_err)?;
                    for sm in &def.members {
                        let sm_name = &sm.declarators[0].name().text;
                        emit_value_write(
                            out,
                            &sm.type_spec,
                            &format!("{access}.{sm_name}()"),
                            endian,
                            "__body_origin",
                            &format!("{indent}      "),
                        )?;
                    }
                    writeln!(out, "{indent}    }}").map_err(fmt_err)?;
                }
                Extensibility::Appendable | Extensibility::Mutable => {
                    let cpp = scoped_to_cpp(sc);
                    let id = next_nest_id();
                    writeln!(
                        out,
                        "{indent}    {{ ::dds::topic::xcdr2::pad_to_from_origin(__out, __sub.body_start, 4);"
                    )
                    .map_err(fmt_err)?;
                    if endian == "be" {
                        writeln!(
                            out,
                            "{indent}      auto __nsb{id} = ::dds::topic::topic_type_support<{cpp}>::encode_be({access});"
                        )
                        .map_err(fmt_err)?;
                    } else {
                        writeln!(
                            out,
                            "{indent}      auto __nsb{id} = ::dds::topic::topic_type_support<{cpp}>::encode({access}, __repr);"
                        )
                        .map_err(fmt_err)?;
                    }
                    writeln!(
                        out,
                        "{indent}      __out.insert(__out.end(), __nsb{id}.begin(), __nsb{id}.end()); }}"
                    )
                    .map_err(fmt_err)?;
                }
            }
            writeln!(
                out,
                "{indent}    ::dds::topic::xcdr2::emheader_nextint_end(__out, __sub); }}"
            )
            .map_err(fmt_err)?;
        }
        // map<K,V> member: LC=4 NEXTINT frame wrapping [DHEADER + count +
        // interleaved entries]. A map is always a non-primitive collection, so —
        // like the mutable Sequence arm and the Rust reference encoder — it
        // carries its own inner DHEADER inside the NEXTINT frame (Finding 6,
        // resolved against the Rust/Cyclone wire).
        TypeSpec::Map(m) => {
            writeln!(
                out,
                "{indent}{{ const auto __sub = ::dds::topic::xcdr2::emheader_nextint_begin(__out, __origin, {id_expr}, {mu_lit});"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    {{ const auto __body_origin = __sub.body_start; (void)__body_origin;"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}      const auto __map_dh = ::dds::topic::xcdr2::dheader_begin(__out);"
            )
            .map_err(fmt_err)?;
            if endian == "be" {
                writeln!(out, "{indent}      ::dds::topic::xcdr2::write_be<uint32_t>(__out, static_cast<uint32_t>({access}.size()));").map_err(fmt_err)?;
            } else {
                writeln!(out, "{indent}      ::dds::topic::xcdr2::write_le_origin<uint32_t>(__out, __body_origin, static_cast<uint32_t>({access}.size()), __max_align);").map_err(fmt_err)?;
            }
            writeln!(out, "{indent}      for (const auto& __kv : {access}) {{").map_err(fmt_err)?;
            let kv_indent = format!("{indent}        ");
            emit_value_write(
                out,
                &m.key,
                "__kv.first",
                endian,
                "__body_origin",
                &kv_indent,
            )?;
            emit_value_write(
                out,
                &m.value,
                "__kv.second",
                endian,
                "__body_origin",
                &kv_indent,
            )?;
            writeln!(out, "{indent}      }}").map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}      ::dds::topic::xcdr2::dheader_end(__out, __map_dh);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{indent}    }}").map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    ::dds::topic::xcdr2::emheader_nextint_end(__out, __sub); }}"
            )
            .map_err(fmt_err)?;
        }
        _ => {
            writeln!(out, "{indent}// xcdr2: unsupported member type").map_err(fmt_err)?;
        }
    }
    Ok(())
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
) -> Result<(), CppGenError> {
    writeln!(
        out,
        "    static {cpp_fqn} decode(const uint8_t* __buf, size_t __len, \
         ::dds::topic::xcdr2::XcdrVersion __repr) {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        size_t __pos = 0;").map_err(fmt_err)?;
    writeln!(out, "        {cpp_fqn} __v;").map_err(fmt_err)?;
    // The XCDR version controls alignment: XCDR2 caps 8-byte primitives
    // to 4-byte boundaries (XTypes 1.3 §7.4.3.4.2), XCDR1 does not.
    writeln!(
        out,
        "        const size_t __max_align = ::dds::topic::xcdr2::xcdr_max_align(__repr);"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "        (void)__buf; (void)__len; (void)__pos; (void)__max_align;"
    )
    .map_err(fmt_err)?;

    match ext {
        Extensibility::Final => {
            writeln!(out, "        const size_t __origin = 0;").map_err(fmt_err)?;
            writeln!(out, "        (void)__origin;").map_err(fmt_err)?;
            for m in &s.members {
                emit_plain_member_decode(out, m, "__origin")?;
            }
        }
        Extensibility::Appendable => {
            writeln!(
                out,
                "        const auto __dh = ::dds::topic::xcdr2::dheader_read(__buf, __pos, __len);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        const size_t __origin = __pos;").map_err(fmt_err)?;
            writeln!(out, "        const size_t __end = __origin + __dh;").map_err(fmt_err)?;
            writeln!(out, "        (void)__end;").map_err(fmt_err)?;
            for m in &s.members {
                emit_plain_member_decode(out, m, "__origin")?;
            }
            // Skip trailing bytes (forward-compat with appendable extension).
            writeln!(out, "        if (__pos < __end) __pos = __end;").map_err(fmt_err)?;
        }
        Extensibility::Mutable => {
            writeln!(
                out,
                "        const auto __dh = ::dds::topic::xcdr2::dheader_read(__buf, __pos, __len);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "        const size_t __origin = __pos;").map_err(fmt_err)?;
            writeln!(out, "        const size_t __end = __origin + __dh;").map_err(fmt_err)?;
            writeln!(out, "        while (__pos + 4 <= __end) {{").map_err(fmt_err)?;
            writeln!(
                out,
                "            const auto __h = ::dds::topic::xcdr2::emheader_read(__buf, __pos, __len, __origin);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "            switch (__h.member_id) {{").map_err(fmt_err)?;
            for m in &s.members {
                emit_mutable_member_decode_case(out, m)?;
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
                "                    if (__h.lc == 0) {{ __pos += 1; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else if (__h.lc == 1) {{ __pos += 2; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else if (__h.lc == 2) {{ __pos += 4; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else if (__h.lc == 3) {{ __pos += 8; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else if (__h.lc == 4 || __h.lc == 5) {{ auto __n = ::dds::topic::xcdr2::emheader_nextint_read(__buf, __pos, __len); __pos += __n; }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else if (__h.lc == 6) {{ auto __c = ::dds::topic::xcdr2::emheader_nextint_read(__buf, __pos, __len); __pos += 4 + 4 * static_cast<size_t>(__c); }}"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "                    else {{ auto __c = ::dds::topic::xcdr2::emheader_nextint_read(__buf, __pos, __len); __pos += 4 + 8 * static_cast<size_t>(__c); }}"
            )
            .map_err(fmt_err)?;
            writeln!(out, "                    break;").map_err(fmt_err)?;
            writeln!(out, "                }}").map_err(fmt_err)?;
            writeln!(out, "            }}").map_err(fmt_err)?;
            writeln!(out, "        }}").map_err(fmt_err)?;
            writeln!(out, "        if (__pos < __end) __pos = __end;").map_err(fmt_err)?;
        }
    }

    writeln!(out, "        return __v;").map_err(fmt_err)?;
    writeln!(out, "    }}").map_err(fmt_err)?;
    Ok(())
}

fn emit_plain_member_decode(out: &mut String, m: &Member, origin: &str) -> Result<(), CppGenError> {
    if !member_codegen_supported(m) {
        for decl in &m.declarators {
            let name = &decl.name().text;
            writeln!(
                out,
                "        // xcdr2: @shared member '{name}' not supported (skip)"
            )
            .map_err(fmt_err)?;
        }
        return Ok(());
    }
    let is_optional = has_optional_annotation(&m.annotations);
    for decl in &m.declarators {
        let name = &decl.name().text;
        // 1-D fixed array of a leaf type — read N elements in place (symmetric to
        // the plain-encode array path). Multi-dim / array-of-struct: follow-up.
        if let Declarator::Array(arr) = decl {
            let prim = matches!(m.type_spec, TypeSpec::Primitive(_));
            let leaf_1d = arr.sizes.len() == 1
                && matches!(m.type_spec, TypeSpec::Primitive(_) | TypeSpec::String(_));
            let prim_read_expr = || -> String {
                match &m.type_spec {
                    TypeSpec::Primitive(PrimitiveType::Boolean) => {
                        "::dds::topic::xcdr2::read_bool(__buf, __pos, __len)".to_string()
                    }
                    TypeSpec::Primitive(PrimitiveType::Octet) => {
                        "::dds::topic::xcdr2::read_u8(__buf, __pos, __len)".to_string()
                    }
                    TypeSpec::Primitive(p) => format!(
                        "::dds::topic::xcdr2::read_le_origin<{}>(__buf, __pos, __len, {origin}, __max_align)",
                        primitive_to_cpp(*p)
                    ),
                    TypeSpec::String(s) if s.wide => format!(
                        "::dds::topic::xcdr2::read_wstring_origin(__buf, __pos, __len, {origin}, __max_align)"
                    ),
                    _ => format!(
                        "::dds::topic::xcdr2::read_string_origin(__buf, __pos, __len, {origin}, __max_align)"
                    ),
                }
            };
            if leaf_1d {
                let read_expr = prim_read_expr();
                writeln!(out, "        {{").map_err(fmt_err)?;
                writeln!(out, "            auto __arr = __v.{name}();").map_err(fmt_err)?;
                writeln!(
                    out,
                    "            for (auto& __ae : __arr) {{ __ae = {read_expr}; }}"
                )
                .map_err(fmt_err)?;
                writeln!(out, "            __v.{name}(__arr);").map_err(fmt_err)?;
                writeln!(out, "        }}").map_err(fmt_err)?;
            } else if prim && arr.sizes.len() >= 2 {
                // Multi-dim primitive array: read row-major into the nested
                // std::array via N nested loops (symmetric to the encode).
                let read_expr = prim_read_expr();
                let n = arr.sizes.len();
                writeln!(out, "        {{").map_err(fmt_err)?;
                writeln!(out, "            auto __arr = __v.{name}();").map_err(fmt_err)?;
                let mut acc = String::from("__arr");
                let mut ind = String::from("            ");
                for d in 0..n {
                    let lv = format!("__a{d}");
                    writeln!(out, "{ind}for (auto& {lv} : {acc}) {{").map_err(fmt_err)?;
                    acc = lv;
                    ind.push_str("    ");
                }
                writeln!(out, "{ind}{acc} = {read_expr};").map_err(fmt_err)?;
                for _ in 0..n {
                    ind.truncate(ind.len() - 4);
                    writeln!(out, "{ind}}}").map_err(fmt_err)?;
                }
                writeln!(out, "            __v.{name}(__arr);").map_err(fmt_err)?;
                writeln!(out, "        }}").map_err(fmt_err)?;
            } else if matches!(&m.type_spec, TypeSpec::Scoped(s) if scoped_is_enum(s) || scoped_final_struct(s).is_some())
                || (matches!(m.type_spec, TypeSpec::String(_)) && arr.sizes.len() >= 2)
            {
                // Array of non-primitive elements (any dims; string only >=2-D):
                // skip the DHEADER, read N elements in place via N nested loops
                // (symmetric to the encode; fixed size, no count).
                let n = arr.sizes.len();
                writeln!(out, "        {{").map_err(fmt_err)?;
                writeln!(out, "        const auto __arr_dh = ::dds::topic::xcdr2::dheader_read(__buf, __pos, __len); (void)__arr_dh;").map_err(fmt_err)?;
                writeln!(out, "        auto __arr = __v.{name}();").map_err(fmt_err)?;
                let mut acc = String::from("__arr");
                let mut ind = String::from("        ");
                for d in 0..n {
                    let lv = format!("__a{d}");
                    writeln!(out, "{ind}for (auto& {lv} : {acc}) {{").map_err(fmt_err)?;
                    acc = lv;
                    ind.push_str("    ");
                }
                emit_value_read(out, &m.type_spec, &format!("{acc} ="), origin, &ind, false)?;
                for _ in 0..n {
                    ind.truncate(ind.len() - 4);
                    writeln!(out, "{ind}}}").map_err(fmt_err)?;
                }
                writeln!(out, "        __v.{name}(__arr);").map_err(fmt_err)?;
                writeln!(out, "        }}").map_err(fmt_err)?;
            } else {
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
                "            uint8_t __present = ::dds::topic::xcdr2::read_u8(__buf, __pos, __len);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "            if (__present) {{").map_err(fmt_err)?;
            emit_value_read(
                out,
                &m.type_spec,
                &format!("__v.{name}"),
                origin,
                "                ",
                true,
            )?;
            writeln!(out, "            }} else {{").map_err(fmt_err)?;
            writeln!(out, "                __v.{name}(std::nullopt);").map_err(fmt_err)?;
            writeln!(out, "            }}").map_err(fmt_err)?;
            writeln!(out, "        }}").map_err(fmt_err)?;
        } else {
            emit_value_read(
                out,
                &m.type_spec,
                &format!("__v.{name}"),
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
                "{indent}{setter}(::dds::topic::xcdr2::read_bool(__buf, __pos, __len));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(PrimitiveType::Octet) => {
            writeln!(
                out,
                "{indent}{setter}(::dds::topic::xcdr2::read_u8(__buf, __pos, __len));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Primitive(p) => {
            let cpp_ty = primitive_to_cpp(*p);
            writeln!(
                out,
                "{indent}{setter}(::dds::topic::xcdr2::read_le_origin<{cpp_ty}>(__buf, __pos, __len, {origin}, __max_align));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::String(s) if !s.wide => {
            writeln!(
                out,
                "{indent}{setter}(::dds::topic::xcdr2::read_string_origin(__buf, __pos, __len, {origin}, __max_align));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::String(s) if s.wide => {
            writeln!(
                out,
                "{indent}{setter}(::dds::topic::xcdr2::read_wstring_origin(__buf, __pos, __len, {origin}, __max_align));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Sequence(seq) => {
            if matches!(&*seq.elem, TypeSpec::Primitive(PrimitiveType::Octet)) {
                // sequence<octet>: raw byte block directly from the buffer.
                writeln!(out, "{indent}{{").map_err(fmt_err)?;
                writeln!(out, "{indent}    auto __cnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(__buf, __pos, __len, {origin}, __max_align);").map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}    ::dds::topic::xcdr2::check_avail(__pos, __cnt, __len);"
                )
                .map_err(fmt_err)?;
                writeln!(
                    out,
                    "{indent}    std::vector<uint8_t> __seq(__buf + __pos, __buf + __pos + __cnt);"
                )
                .map_err(fmt_err)?;
                writeln!(out, "{indent}    __pos += __cnt;").map_err(fmt_err)?;
                writeln!(out, "{indent}    {setter}(std::move(__seq));").map_err(fmt_err)?;
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
            // XCDR2 §7.4.3.5: for non-primitive elements skip the DHEADER.
            if !matches!(&*seq.elem, TypeSpec::Primitive(_)) {
                writeln!(out, "{indent}    const auto __seq_dh = ::dds::topic::xcdr2::dheader_read(__buf, __pos, __len); (void)__seq_dh;").map_err(fmt_err)?;
            }
            writeln!(out, "{indent}    auto __cnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(__buf, __pos, __len, {origin}, __max_align);").map_err(fmt_err)?;
            writeln!(out, "{indent}    std::vector<{elem_cpp_ty}> __seq;").map_err(fmt_err)?;
            writeln!(out, "{indent}    __seq.reserve(__cnt);").map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    for (uint32_t __i = 0; __i < __cnt; ++__i) {{"
            )
            .map_err(fmt_err)?;
            match &*seq.elem {
                TypeSpec::Primitive(PrimitiveType::Boolean) => {
                    writeln!(out, "{indent}        __seq.push_back(::dds::topic::xcdr2::read_bool(__buf, __pos, __len));").map_err(fmt_err)?;
                }
                TypeSpec::Primitive(PrimitiveType::Octet) => {
                    writeln!(out, "{indent}        __seq.push_back(::dds::topic::xcdr2::read_u8(__buf, __pos, __len));").map_err(fmt_err)?;
                }
                TypeSpec::Primitive(p) => {
                    let cpp_ty = primitive_to_cpp(*p);
                    writeln!(out, "{indent}        __seq.push_back(::dds::topic::xcdr2::read_le_origin<{cpp_ty}>(__buf, __pos, __len, {origin}, __max_align));").map_err(fmt_err)?;
                }
                TypeSpec::String(s) if !s.wide => {
                    writeln!(out, "{indent}        __seq.push_back(::dds::topic::xcdr2::read_string_origin(__buf, __pos, __len, {origin}, __max_align));").map_err(fmt_err)?;
                }
                // wide string element.
                TypeSpec::String(_) => {
                    writeln!(out, "{indent}        __seq.push_back(::dds::topic::xcdr2::read_wstring_origin(__buf, __pos, __len, {origin}, __max_align));").map_err(fmt_err)?;
                }
                // enum element: read int32, cast back to the enum type.
                TypeSpec::Scoped(s) if scoped_is_enum(s) => {
                    let cpp_ty = scoped_to_cpp(s);
                    writeln!(out, "{indent}        __seq.push_back(static_cast<{cpp_ty}>(::dds::topic::xcdr2::read_le_origin<int32_t>(__buf, __pos, __len, {origin}, __max_align)));").map_err(fmt_err)?;
                }
                // nested @final struct element: read each sub-member into a fresh
                // temp, push the whole object (symmetric to the inline encode).
                TypeSpec::Scoped(sc) if scoped_final_struct(sc).is_some() => {
                    if let Some(def) = scoped_final_struct(sc) {
                        let cpp_ty = scoped_to_cpp(sc);
                        let var = format!("__se{}", next_nest_id());
                        let binner = format!("{indent}        ");
                        writeln!(out, "{binner}{cpp_ty} {var}{{}};").map_err(fmt_err)?;
                        for sm in &def.members {
                            let sm_name = &sm.declarators[0].name().text;
                            emit_value_read(
                                out,
                                &sm.type_spec,
                                &format!("{var}.{sm_name}"),
                                origin,
                                &binner,
                                false,
                            )?;
                        }
                        writeln!(out, "{binner}__seq.push_back({var});").map_err(fmt_err)?;
                    }
                }
                // nested @appendable/@mutable struct element: 4-align, read the
                // element's own DHEADER, sub-decode the [DHEADER+body] slice via
                // the nested type's `decode`, advance the cursor past it, push it.
                TypeSpec::Scoped(sc) if scoped_struct(sc).is_some() => {
                    let cpp_ty = scoped_to_cpp(sc);
                    let id = next_nest_id();
                    let var = format!("__se{id}");
                    let binner = format!("{indent}        ");
                    writeln!(
                        out,
                        "{binner}::dds::topic::xcdr2::skip_pad_from_origin(__pos, {origin}, 4);"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "{binner}const size_t __nss{id} = __pos;").map_err(fmt_err)?;
                    writeln!(out, "{binner}size_t __npk{id} = __pos;").map_err(fmt_err)?;
                    writeln!(out, "{binner}const uint32_t __nl{id} = ::dds::topic::xcdr2::dheader_read(__buf, __npk{id}, __len);").map_err(fmt_err)?;
                    writeln!(out, "{binner}{cpp_ty} {var} = ::dds::topic::topic_type_support<{cpp_ty}>::decode(__buf + __nss{id}, 4u + __nl{id}, __repr);").map_err(fmt_err)?;
                    writeln!(out, "{binner}__pos = __nss{id} + 4u + __nl{id};").map_err(fmt_err)?;
                    writeln!(out, "{binner}__seq.push_back(std::move({var}));").map_err(fmt_err)?;
                }
                // nested sequence element: read the inner sequence into a temp
                // via the assignment-setter form, then push it.
                TypeSpec::Sequence(_) => {
                    let inner_ty = typespec_to_cpp(&seq.elem)?;
                    let var = format!("__se{}", next_nest_id());
                    let binner = format!("{indent}        ");
                    writeln!(out, "{binner}{inner_ty} {var}{{}};").map_err(fmt_err)?;
                    emit_value_read(out, &seq.elem, &format!("{var} ="), origin, &binner, false)?;
                    writeln!(out, "{binner}__seq.push_back(std::move({var}));").map_err(fmt_err)?;
                }
                // map element: read the inner map into a temp, then push it.
                TypeSpec::Map(_) => {
                    let inner_ty = typespec_to_cpp(&seq.elem)?;
                    let var = format!("__se{}", next_nest_id());
                    let binner = format!("{indent}        ");
                    writeln!(out, "{binner}{inner_ty} {var}{{}};").map_err(fmt_err)?;
                    emit_value_read(out, &seq.elem, &format!("{var} ="), origin, &binner, false)?;
                    writeln!(out, "{binner}__seq.push_back(std::move({var}));").map_err(fmt_err)?;
                }
                _ => {}
            }
            writeln!(out, "{indent}    }}").map_err(fmt_err)?;
            writeln!(out, "{indent}    {setter}(std::move(__seq));").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
        // map<K,V> member: read DHEADER, count, then count interleaved key/value
        // pairs (symmetric to the encode); insert into a std::map. Key/value are
        // read into fresh temps via the assignment-setter form `__k =(...)`
        // (emit_value_read always ends in `{setter}(final)`, so `__k =` yields a
        // plain assignment that works for primitive/string/enum/struct values).
        TypeSpec::Map(m) => {
            let k_ty = typespec_to_cpp(&m.key)?;
            let v_ty = typespec_to_cpp(&m.value)?;
            let id = next_nest_id();
            let mapv = format!("__map{id}");
            let kv = format!("__mk{id}");
            let vv = format!("__mv{id}");
            let inner = format!("{indent}    ");
            let li = format!("{inner}    ");
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            writeln!(out, "{inner}const auto __map_dh = ::dds::topic::xcdr2::dheader_read(__buf, __pos, __len); (void)__map_dh;").map_err(fmt_err)?;
            writeln!(out, "{inner}auto __mcnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(__buf, __pos, __len, {origin}, __max_align);").map_err(fmt_err)?;
            writeln!(out, "{inner}std::map<{k_ty}, {v_ty}> {mapv};").map_err(fmt_err)?;
            writeln!(out, "{inner}for (uint32_t __i = 0; __i < __mcnt; ++__i) {{")
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
        // enum member: read its int32 underlying value, cast back to the enum.
        TypeSpec::Scoped(s) if scoped_is_enum(s) => {
            let cpp_ty = scoped_to_cpp(s);
            writeln!(
                out,
                "{indent}{setter}(static_cast<{cpp_ty}>(::dds::topic::xcdr2::read_le_origin<int32_t>(__buf, __pos, __len, {origin}, __max_align)));"
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
            let id = next_nest_id();
            let var = format!("__ns{id}");
            let inner = format!("{indent}    ");
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            writeln!(out, "{inner}{cpp_ty} {var}{{}};").map_err(fmt_err)?;
            match ext {
                Extensibility::Final => {
                    for sm in &def.members {
                        let sm_name = &sm.declarators[0].name().text;
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
                        "{inner}::dds::topic::xcdr2::skip_pad_from_origin(__pos, {origin}, 4);"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "{inner}const size_t __nss{id} = __pos;").map_err(fmt_err)?;
                    writeln!(out, "{inner}size_t __npk{id} = __pos;").map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{inner}const uint32_t __nl{id} = ::dds::topic::xcdr2::dheader_read(__buf, __npk{id}, __len);"
                    )
                    .map_err(fmt_err)?;
                    writeln!(
                        out,
                        "{inner}{var} = ::dds::topic::topic_type_support<{cpp_ty}>::decode(__buf + __nss{id}, 4u + __nl{id}, __repr);"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "{inner}__pos = __nss{id} + 4u + __nl{id};").map_err(fmt_err)?;
                }
            }
            writeln!(out, "{inner}{setter}({var});").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
        _ => {}
    }
    Ok(())
}

fn emit_mutable_member_decode_case(out: &mut String, m: &Member) -> Result<(), CppGenError> {
    if !member_codegen_supported(m) {
        return Ok(());
    }
    let id_override = find_uint_annotation(&m.annotations, "id");
    let is_optional = has_optional_annotation(&m.annotations);
    let _ = is_optional; // mutable optional: same path; absent member just skips this case.
    for decl in &m.declarators {
        let name = &decl.name().text;
        if !matches!(decl, Declarator::Simple(_)) {
            continue;
        }
        if !typespec_supported(&m.type_spec) {
            continue;
        }
        let id_expr = match id_override {
            Some(id) => id.to_string(),
            None => format!("0x{:x}u", auto_id_for(name)),
        };
        writeln!(out, "                case {id_expr}: {{").map_err(fmt_err)?;
        match &m.type_spec {
            TypeSpec::Primitive(PrimitiveType::Boolean) => {
                writeln!(out, "                    uint8_t __b = ::dds::topic::xcdr2::read_u8(__buf, __pos, __len);").map_err(fmt_err)?;
                if has_optional_annotation(&m.annotations) {
                    writeln!(
                        out,
                        "                    __v.{name}(static_cast<bool>(__b));"
                    )
                    .map_err(fmt_err)?;
                } else {
                    writeln!(
                        out,
                        "                    __v.{name}(static_cast<bool>(__b));"
                    )
                    .map_err(fmt_err)?;
                }
            }
            TypeSpec::Primitive(PrimitiveType::Octet) => {
                writeln!(out, "                    __v.{name}(::dds::topic::xcdr2::read_u8(__buf, __pos, __len));").map_err(fmt_err)?;
            }
            TypeSpec::Primitive(p) => {
                let cpp_ty = primitive_to_cpp(*p);
                writeln!(out, "                    __v.{name}(::dds::topic::xcdr2::read_le_raw<{cpp_ty}>(__buf, __pos, __len));").map_err(fmt_err)?;
            }
            TypeSpec::String(s) if !s.wide => {
                writeln!(out, "                    auto __n = ::dds::topic::xcdr2::emheader_nextint_read(__buf, __pos, __len);").map_err(fmt_err)?;
                writeln!(out, "                    (void)__n;").map_err(fmt_err)?;
                writeln!(out, "                    auto __body_origin = __pos;")
                    .map_err(fmt_err)?;
                writeln!(out, "                    __v.{name}(::dds::topic::xcdr2::read_string_origin(__buf, __pos, __len, __body_origin, __max_align));").map_err(fmt_err)?;
            }
            TypeSpec::String(s) if s.wide => {
                writeln!(out, "                    auto __n = ::dds::topic::xcdr2::emheader_nextint_read(__buf, __pos, __len);").map_err(fmt_err)?;
                writeln!(out, "                    (void)__n;").map_err(fmt_err)?;
                writeln!(out, "                    auto __body_origin = __pos;")
                    .map_err(fmt_err)?;
                writeln!(out, "                    __v.{name}(::dds::topic::xcdr2::read_wstring_origin(__buf, __pos, __len, __body_origin, __max_align));").map_err(fmt_err)?;
            }
            TypeSpec::Sequence(seq) => {
                writeln!(out, "                    auto __n = ::dds::topic::xcdr2::emheader_nextint_read(__buf, __pos, __len);").map_err(fmt_err)?;
                writeln!(out, "                    (void)__n;").map_err(fmt_err)?;
                writeln!(out, "                    auto __body_origin = __pos;")
                    .map_err(fmt_err)?;
                // Non-primitive-element sequence carries an inner DHEADER inside
                // the NEXTINT frame (symmetric to the encode; Finding 6).
                if !matches!(&*seq.elem, TypeSpec::Primitive(_)) {
                    writeln!(out, "                    {{ const auto __seq_dh = ::dds::topic::xcdr2::dheader_read(__buf, __pos, __len); (void)__seq_dh; }}").map_err(fmt_err)?;
                }
                writeln!(out, "                    auto __cnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(__buf, __pos, __len, __body_origin, __max_align);").map_err(fmt_err)?;
                if matches!(&*seq.elem, TypeSpec::Primitive(PrimitiveType::Octet)) {
                    // sequence<octet>: raw byte block directly from the buffer.
                    writeln!(
                        out,
                        "                    ::dds::topic::xcdr2::check_avail(__pos, __cnt, __len);"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "                    std::vector<uint8_t> __seq(__buf + __pos, __buf + __pos + __cnt);").map_err(fmt_err)?;
                    writeln!(out, "                    __pos += __cnt;").map_err(fmt_err)?;
                    writeln!(out, "                    __v.{name}(std::move(__seq));")
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
                    writeln!(out, "                    std::vector<{elem_cpp_ty}> __seq;")
                        .map_err(fmt_err)?;
                    writeln!(out, "                    __seq.reserve(__cnt);").map_err(fmt_err)?;
                    writeln!(
                        out,
                        "                    for (uint32_t __i = 0; __i < __cnt; ++__i) {{"
                    )
                    .map_err(fmt_err)?;
                    match &*seq.elem {
                        TypeSpec::Primitive(PrimitiveType::Boolean) => {
                            writeln!(out, "                        __seq.push_back(::dds::topic::xcdr2::read_bool(__buf, __pos, __len));").map_err(fmt_err)?;
                        }
                        TypeSpec::Primitive(p) => {
                            let cpp_ty = primitive_to_cpp(*p);
                            writeln!(out, "                        __seq.push_back(::dds::topic::xcdr2::read_le_origin<{cpp_ty}>(__buf, __pos, __len, __body_origin, __max_align));").map_err(fmt_err)?;
                        }
                        TypeSpec::String(s) if !s.wide => {
                            writeln!(out, "                        __seq.push_back(::dds::topic::xcdr2::read_string_origin(__buf, __pos, __len, __body_origin, __max_align));").map_err(fmt_err)?;
                        }
                        TypeSpec::String(_) => {
                            writeln!(out, "                        __seq.push_back(::dds::topic::xcdr2::read_wstring_origin(__buf, __pos, __len, __body_origin, __max_align));").map_err(fmt_err)?;
                        }
                        TypeSpec::Scoped(s) if scoped_is_enum(s) => {
                            let cpp_ty = scoped_to_cpp(s);
                            writeln!(out, "                        __seq.push_back(static_cast<{cpp_ty}>(::dds::topic::xcdr2::read_le_origin<int32_t>(__buf, __pos, __len, __body_origin, __max_align)));").map_err(fmt_err)?;
                        }
                        TypeSpec::Scoped(sc) if scoped_final_struct(sc).is_some() => {
                            if let Some(def) = scoped_final_struct(sc) {
                                let cpp_ty = scoped_to_cpp(sc);
                                let var = format!("__se{}", next_nest_id());
                                writeln!(out, "                        {cpp_ty} {var}{{}};")
                                    .map_err(fmt_err)?;
                                for sm in &def.members {
                                    let sm_name = &sm.declarators[0].name().text;
                                    emit_value_read(
                                        out,
                                        &sm.type_spec,
                                        &format!("{var}.{sm_name}"),
                                        "__body_origin",
                                        "                        ",
                                        false,
                                    )?;
                                }
                                writeln!(out, "                        __seq.push_back({var});")
                                    .map_err(fmt_err)?;
                            }
                        }
                        // nested @appendable/@mutable struct element: 4-align,
                        // read the element DHEADER, sub-decode the [DHEADER+body]
                        // slice via the nested type's `decode`, advance, push.
                        TypeSpec::Scoped(sc) if scoped_struct(sc).is_some() => {
                            let cpp_ty = scoped_to_cpp(sc);
                            let id = next_nest_id();
                            let var = format!("__se{id}");
                            writeln!(out, "                        ::dds::topic::xcdr2::skip_pad_from_origin(__pos, __body_origin, 4);").map_err(fmt_err)?;
                            writeln!(
                                out,
                                "                        const size_t __nss{id} = __pos;"
                            )
                            .map_err(fmt_err)?;
                            writeln!(out, "                        size_t __npk{id} = __pos;")
                                .map_err(fmt_err)?;
                            writeln!(out, "                        const uint32_t __nl{id} = ::dds::topic::xcdr2::dheader_read(__buf, __npk{id}, __len);").map_err(fmt_err)?;
                            writeln!(out, "                        {cpp_ty} {var} = ::dds::topic::topic_type_support<{cpp_ty}>::decode(__buf + __nss{id}, 4u + __nl{id}, __repr);").map_err(fmt_err)?;
                            writeln!(
                                out,
                                "                        __pos = __nss{id} + 4u + __nl{id};"
                            )
                            .map_err(fmt_err)?;
                            writeln!(
                                out,
                                "                        __seq.push_back(std::move({var}));"
                            )
                            .map_err(fmt_err)?;
                        }
                        // nested sequence / map element: read into a temp, push.
                        TypeSpec::Sequence(_) | TypeSpec::Map(_) => {
                            let inner_ty = typespec_to_cpp(&seq.elem)?;
                            let var = format!("__se{}", next_nest_id());
                            writeln!(out, "                        {inner_ty} {var}{{}};")
                                .map_err(fmt_err)?;
                            emit_value_read(
                                out,
                                &seq.elem,
                                &format!("{var} ="),
                                "__body_origin",
                                "                        ",
                                false,
                            )?;
                            writeln!(
                                out,
                                "                        __seq.push_back(std::move({var}));"
                            )
                            .map_err(fmt_err)?;
                        }
                        _ => {}
                    }
                    writeln!(out, "                    }}").map_err(fmt_err)?;
                    writeln!(out, "                    __v.{name}(std::move(__seq));")
                        .map_err(fmt_err)?;
                }
            }
            // enum member: 4-byte int32 read directly (encoded via compact LC=2).
            TypeSpec::Scoped(s) if scoped_is_enum(s) => {
                let cpp_ty = scoped_to_cpp(s);
                writeln!(out, "                    __v.{name}(static_cast<{cpp_ty}>(::dds::topic::xcdr2::read_le_raw<int32_t>(__buf, __pos, __len)));").map_err(fmt_err)?;
            }
            // nested struct member: skip NEXTINT to the EMHEADER body-origin.
            // @final: read inline members. @appendable/@mutable: sub-decode the
            // nested type from its own DHEADER inside the body.
            TypeSpec::Scoped(sc) if scoped_struct(sc).is_some() => {
                if let Some((def, ext)) = scoped_struct(sc) {
                    let cpp_ty = scoped_to_cpp(sc);
                    let id = next_nest_id();
                    let var = format!("__ns{id}");
                    writeln!(out, "                    auto __n = ::dds::topic::xcdr2::emheader_nextint_read(__buf, __pos, __len); (void)__n;").map_err(fmt_err)?;
                    writeln!(
                        out,
                        "                    auto __body_origin = __pos; (void)__body_origin;"
                    )
                    .map_err(fmt_err)?;
                    writeln!(out, "                    {cpp_ty} {var}{{}};").map_err(fmt_err)?;
                    match ext {
                        Extensibility::Final => {
                            for sm in &def.members {
                                let sm_name = &sm.declarators[0].name().text;
                                emit_value_read(
                                    out,
                                    &sm.type_spec,
                                    &format!("{var}.{sm_name}"),
                                    "__body_origin",
                                    "                    ",
                                    false,
                                )?;
                            }
                        }
                        Extensibility::Appendable | Extensibility::Mutable => {
                            writeln!(out, "                    const size_t __nss{id} = __pos;")
                                .map_err(fmt_err)?;
                            writeln!(out, "                    size_t __npk{id} = __pos;")
                                .map_err(fmt_err)?;
                            writeln!(out, "                    const uint32_t __nl{id} = ::dds::topic::xcdr2::dheader_read(__buf, __npk{id}, __len);").map_err(fmt_err)?;
                            writeln!(out, "                    {var} = ::dds::topic::topic_type_support<{cpp_ty}>::decode(__buf + __nss{id}, 4u + __nl{id}, __repr);").map_err(fmt_err)?;
                            writeln!(
                                out,
                                "                    __pos = __nss{id} + 4u + __nl{id};"
                            )
                            .map_err(fmt_err)?;
                        }
                    }
                    writeln!(out, "                    __v.{name}({var});").map_err(fmt_err)?;
                }
            }
            // map<K,V> member: skip NEXTINT, read count + interleaved entries
            // relative to the body-origin (symmetric to the mutable encode).
            TypeSpec::Map(m) => {
                let k_ty = typespec_to_cpp(&m.key)?;
                let v_ty = typespec_to_cpp(&m.value)?;
                let id = next_nest_id();
                let mapv = format!("__map{id}");
                let kv = format!("__mk{id}");
                let vv = format!("__mv{id}");
                writeln!(out, "                    auto __mn = ::dds::topic::xcdr2::emheader_nextint_read(__buf, __pos, __len); (void)__mn;").map_err(fmt_err)?;
                writeln!(out, "                    auto __body_origin = __pos;")
                    .map_err(fmt_err)?;
                // map is non-primitive -> inner DHEADER inside the NEXTINT frame.
                writeln!(out, "                    {{ const auto __map_dh = ::dds::topic::xcdr2::dheader_read(__buf, __pos, __len); (void)__map_dh; }}").map_err(fmt_err)?;
                writeln!(out, "                    auto __mcnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(__buf, __pos, __len, __body_origin, __max_align);").map_err(fmt_err)?;
                writeln!(out, "                    std::map<{k_ty}, {v_ty}> {mapv};")
                    .map_err(fmt_err)?;
                writeln!(
                    out,
                    "                    for (uint32_t __i = 0; __i < __mcnt; ++__i) {{"
                )
                .map_err(fmt_err)?;
                writeln!(out, "                        {k_ty} {kv}{{}};").map_err(fmt_err)?;
                writeln!(out, "                        {v_ty} {vv}{{}};").map_err(fmt_err)?;
                emit_value_read(
                    out,
                    &m.key,
                    &format!("{kv} ="),
                    "__body_origin",
                    "                        ",
                    false,
                )?;
                emit_value_read(
                    out,
                    &m.value,
                    &format!("{vv} ="),
                    "__body_origin",
                    "                        ",
                    false,
                )?;
                writeln!(
                    out,
                    "                        {mapv}.emplace(std::move({kv}), std::move({vv}));"
                )
                .map_err(fmt_err)?;
                writeln!(out, "                    }}").map_err(fmt_err)?;
                writeln!(out, "                    __v.{name}(std::move({mapv}));")
                    .map_err(fmt_err)?;
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
) -> Result<(), CppGenError> {
    writeln!(
        out,
        "    static std::array<uint8_t, 16> key_hash(const {cpp_fqn}& __v) {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        (void)__v;").map_err(fmt_err)?;
    if !is_keyed {
        writeln!(
            out,
            "        return std::array<uint8_t, 16>{{{{0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0}}}};"
        )
        .map_err(fmt_err)?;
        writeln!(out, "    }}").map_err(fmt_err)?;
        return Ok(());
    }
    writeln!(out, "        std::vector<uint8_t> __out;").map_err(fmt_err)?;
    writeln!(out, "        const size_t __origin = 0;").map_err(fmt_err)?;
    writeln!(out, "        (void)__origin;").map_err(fmt_err)?;
    for m in &s.members {
        if !has_key_annotation(&m.annotations) {
            continue;
        }
        emit_plain_member_encode(out, m, "be", "__origin")?;
    }
    // XTypes 1.3 §7.6.8.4: holder ≤ 16 octets -> zero-pad; otherwise MD5.
    writeln!(out, "        std::array<uint8_t, 16> __h{{}};").map_err(fmt_err)?;
    writeln!(out, "        if (__out.size() <= 16) {{").map_err(fmt_err)?;
    writeln!(
        out,
        "            std::memcpy(__h.data(), __out.data(), __out.size());"
    )
    .map_err(fmt_err)?;
    writeln!(out, "            return __h;").map_err(fmt_err)?;
    writeln!(out, "        }}").map_err(fmt_err)?;
    writeln!(out, "        return ::dds::topic::xcdr2_md5::md5(__out);").map_err(fmt_err)?;
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
