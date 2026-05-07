// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! AST-Walker, der C++17-Header emittiert.
//!
//! Block-A: Header-Layout (`#pragma once`, includes, namespaces).
//! Block-B: Primitive-Mapping (delegiert an [`crate::type_map`]).
//! Block-C: struct/enum/union/typedef/sequence/array/inheritance.
//! Block-D: Exception → `class X : public std::exception`.
//! Block-E: Time/Duration über DDS::Time_t / DDS::Duration_t.
//!
//! Die Emission ist single-pass: zuerst werden alle benoetigten
//! Standard-Includes durch einen pre-walk gesammelt, dann der Body
//! emittiert. So bleibt die Header-Praeambel deterministisch
//! (alphabetisch sortiert).

use std::collections::BTreeSet;
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

/// Bekannte Header-Includes. Reihenfolge ist stabil-alphabetisch.
#[derive(Debug, Default, Clone)]
struct Includes {
    headers: BTreeSet<&'static str>,
}

impl Includes {
    fn add(&mut self, h: &'static str) {
        self.headers.insert(h);
    }
}

/// Haupt-Entry: erzeugt den vollstaendigen C++-Header.
pub(crate) fn emit_header(
    spec: &Specification,
    opts: &CppGenOptions,
) -> Result<String, CppGenError> {
    // 1. Kollidierende Inheritance-Cycles erkennen (vor Emission).
    detect_inheritance_cycles(spec)?;

    // 2. Walk pre-pass: includes sammeln.
    let mut includes = Includes::default();
    includes.add("<cstdint>"); // stdint.h ist immer praesent (uint8_t etc.).
    collect_includes(spec, &mut includes);

    // 3. Output-Buffer.
    let mut out = String::new();
    write_header_preamble(&mut out, opts, &includes)?;

    // 4. Optionale Top-Level-Namespace-Einbindung (`namespace_prefix`).
    let mut ctx = EmitCtx::new(opts);
    let outer_prefix: Option<&str> = opts.namespace_prefix.as_deref().filter(|p| !p.is_empty());
    if let Some(prefix) = outer_prefix {
        ctx.open_namespace(&mut out, prefix)?;
    }

    // 4b. §7.2.2.4.8 — `@verbatim(placement=BEGIN_FILE)` aus allen
    // Top-Level-Defs. Spec sagt nichts ueber Reihenfolge bei mehreren —
    // wir nehmen Source-Order.
    for d in &spec.definitions {
        if let Some(anns) = top_level_annotations(d) {
            emit_verbatim_at(&mut out, "", anns, PlacementKind::BeginFile)?;
        }
    }

    // 4c. Wenn das Spec mindestens eine Top-Level- oder Modul-nested
    //     `struct`-Definition enthaelt, emittieren wir nach den
    //     Standard-Includes auch `dds/topic/TopicTraits.hpp` — dort lebt
    //     `topic_type_support<T>` und ByteSeq/string-Defaults — sowie
    //     `dds/topic/xcdr2.hpp` und `dds/topic/xcdr2_md5.hpp` fuer die
    //     XCDR2 Wire-Encoder/MD5-Helpers, auf denen die spaeter
    //     emittierten Spezialisierungen aufsetzen.
    let mut probe_structs: Vec<(String, &StructDef)> = Vec::new();
    collect_topic_structs(&spec.definitions, "", &mut probe_structs);
    if !probe_structs.is_empty() {
        // <array> wird von key_hash() benoetigt, <vector>/<string> von
        // encode/decode(). Falls die Standard-Walks die nicht schon eingezogen
        // haben, ueber TopicTraits.hpp transitiv abgedeckt — aber wir
        // emittieren sie hier explizit, damit der Header ohne Topic-Helpers
        // noch syntaktisch valid bleibt.
        writeln!(&mut out, "#include \"dds/topic/TopicTraits.hpp\"").map_err(fmt_err)?;
        writeln!(&mut out, "#include \"dds/topic/xcdr2.hpp\"").map_err(fmt_err)?;
        writeln!(&mut out, "#include \"dds/topic/xcdr2_md5.hpp\"").map_err(fmt_err)?;
        writeln!(&mut out).map_err(fmt_err)?;
    }

    // 5. Definitions emittieren.
    for d in &spec.definitions {
        emit_definition(&mut out, &mut ctx, d)?;
    }

    // 5b. §7.2.2.4.8 — `@verbatim(placement=END_FILE)` aus allen
    // Top-Level-Defs.
    for d in &spec.definitions {
        if let Some(anns) = top_level_annotations(d) {
            emit_verbatim_at(&mut out, "", anns, PlacementKind::EndFile)?;
        }
    }

    // 6. Optional: Top-Level-Namespace schliessen.
    if let Some(prefix) = outer_prefix {
        ctx.close_namespace(&mut out, prefix)?;
    }

    // 7. `topic_type_support<T>`-Spezialisierungen fuer alle struct-Defs.
    //    Lebt im `::dds::topic`-Namespace (globaler Scope), daher nach
    //    `outer_prefix`-Close emittiert mit voll-qualifiziertem T.
    if !probe_structs.is_empty() {
        emit_topic_type_support_specs(&mut out, opts, &probe_structs)?;
    }

    Ok(out)
}

/// Liefert die Annotation-Liste eines Top-Level-`Definition`s, falls
/// die Variante eine traegt. Wird fuer File-Level-Verbatim genutzt.
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

/// Schreibt `// Generated...`, `#pragma once` und Standard-Includes.
fn write_header_preamble(
    out: &mut String,
    opts: &CppGenOptions,
    includes: &Includes,
) -> Result<(), CppGenError> {
    writeln!(out, "// Generated by zerodds idl-cpp. Do not edit.").map_err(fmt_err)?;
    writeln!(out, "#pragma once").map_err(fmt_err)?;
    if let Some(guard) = opts.include_guard_prefix.as_deref() {
        if !guard.is_empty() {
            // Optionaler Guard-Kommentar fuer Tools, die nur klassische
            // include-Guards akzeptieren. `#pragma once` bleibt primaer.
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

/// Walkt den AST und sammelt benoetigte `<...>`-Includes.
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
            // Im emit_definition als
            // UnsupportedConstruct erkannt; hier kein Include sammeln.
        }
    }
}

fn collect_in_typedecl(td: &TypeDecl, inc: &mut Includes) {
    match td {
        TypeDecl::Constr(c) => match c {
            ConstrTypeDecl::Struct(StructDcl::Def(s)) => {
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
            // sowohl `string` als auch `wstring` sind in `<string>` definiert.
            inc.add("<string>");
        }
        TypeSpec::Map(m) => {
            inc.add("<map>");
            collect_in_typespec(&m.key, inc);
            collect_in_typespec(&m.value, inc);
        }
        TypeSpec::Fixed(_) => {
            // §7.2.4.2.4 — `fixed<digits, scale>` -> `dds::core::Fixed`.
            // Wir emittieren eine forward-deklarierte Wrapper-Klasse
            // (Runtime-Implementierung kommt mit `dds-core`-Crate).
            inc.add("<cstdint>");
        }
        TypeSpec::Any => {
            // §7.3 — `any` -> `dds::core::Any`. Runtime in dds-core.
            inc.add("<cstdint>");
        }
    }
}

/// Emit-Kontext (Indentation, Output).
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
            // `@service`-Interfaces gehen weiter ueber den RPC-Codegen-
            // Pfad (siehe `crate::rpc`); hier ist der Legacy-CORBA-Stub-
            // Pfad fuer non-service Interfaces.
            emit_interface_stub(out, ctx, iface)
        }
        Definition::Interface(InterfaceDcl::Forward(f)) => {
            check_identifier(&f.name.text)?;
            writeln!(out, "{}class {};", ctx.indent(), f.name.text).map_err(fmt_err)?;
            Ok(())
        }
        Definition::ValueDef(v) => emit_value_type(out, ctx, v),
        Definition::ValueBox(_) | Definition::ValueForward(_) => {
            // ValueBox + ValueForward sind seltene CORBA-Konstrukte;
            // Foundation laesst sie als no-op (Spec §7.6.x erlaubt
            // forward-decl mit spaeterer Resolution).
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
            // §7.4.15 Annotation-Defs werden nicht direkt zu C++-Code —
            // Anwendungen werden bei den annotierten Mitgliedern via
            // Annotation-Bridges (z.B. @key) emittiert.
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
    }
}

fn emit_struct(out: &mut String, ctx: &mut EmitCtx<'_>, s: &StructDef) -> Result<(), CppGenError> {
    check_identifier(&s.name.text)?;
    let ind = ctx.indent();

    // §7.2.2.4.8 — `@verbatim(placement=BEFORE_DECLARATION)` vor der
    // Class-Header-Zeile.
    emit_verbatim_at(out, &ind, &s.annotations, PlacementKind::BeforeDeclaration)?;

    // Class-Header mit optionaler Inheritance-Klausel.
    if let Some(base) = &s.base {
        let base_str = scoped_to_cpp(base);
        writeln!(out, "{ind}class {} : public {} {{", s.name.text, base_str).map_err(fmt_err)?;
    } else {
        writeln!(out, "{ind}class {} {{", s.name.text).map_err(fmt_err)?;
    }
    writeln!(out, "{ind}public:").map_err(fmt_err)?;

    let inner = " ".repeat((ctx.indent_level + 1) * ctx.opts.indent_width);

    // §7.2.2.4.8 — `@verbatim(placement=BEGIN_DECLARATION)` als erste
    // Zeile innerhalb des `public:`-Blocks.
    emit_verbatim_at(out, &inner, &s.annotations, PlacementKind::BeginDeclaration)?;

    // Default-Konstruktor.
    writeln!(out, "{inner}{}() = default;", s.name.text).map_err(fmt_err)?;
    writeln!(out, "{inner}~{}() = default;", s.name.text).map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Member-Felder als private Storage.
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

    // §7.2.2.4.8 — `@verbatim(placement=END_DECLARATION)` als letzte
    // Zeile vor `};`.
    emit_verbatim_at(out, &inner, &s.annotations, PlacementKind::EndDeclaration)?;

    writeln!(out, "{ind}}};").map_err(fmt_err)?;

    // §7.2.2.4.8 — `@verbatim(placement=AFTER_DECLARATION)` direkt
    // hinter dem schliessenden `};`.
    emit_verbatim_at(out, &ind, &s.annotations, PlacementKind::AfterDeclaration)?;

    writeln!(out).map_err(fmt_err)?;
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
        // §8.1.5 `@shared` -> `std::shared_ptr<T>`. Kombination mit
        // `@optional` ergibt `std::optional<std::shared_ptr<T>>`.
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

    // Variant-Liste aus distinct Element-Typen aufbauen.
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

    // Branch-Marker als Kommentare (Discriminator-Werte).
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

    // Bases via public virtual inheritance (CORBA-Pattern fuer Diamond).
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
            // Spec §7.4.5: in -> const T& (oder T fuer primitives),
            // out/inout -> T&. Fuer Foundation: const T&/T& konsistent.
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
    // Getter (alle Attribute haben einen).
    writeln!(out, "{inner}virtual {ty} {}() const = 0;", attr.name.text).map_err(fmt_err)?;
    // Setter nur fuer non-readonly.
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

    // Spec idl4-cpp §7.6: valuetype -> C++ class mit pure-virtual
    // public/protected accessors (state) + factory-Class.
    // Inheritance: public virtual fuer alle bases + supports-Interfaces.
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

    // Public-State + Methoden.
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
            // §7.2.4.2.4 — Fixed-Constant ohne Digits/Scale-Annotation;
            // wir emittieren als opaker Wrapper (Caller annotiert den Typ
            // ueber separates `typedef fixed<D,S> Name;`).
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

/// Liefert den C++-Type-Ausdruck fuer ein Member (TypeSpec + Declarator).
/// Array-Declaratoren werden zu `std::array<T, N>` (mehrdimensional ge-nestet).
pub(crate) fn type_for_declarator(ts: &TypeSpec, decl: &Declarator) -> Result<String, CppGenError> {
    let base = typespec_to_cpp(ts)?;
    match decl {
        Declarator::Simple(_) => Ok(base),
        Declarator::Array(arr) => {
            // Innen-zu-aussen wickeln: arr.sizes[0] ist die aeusserste Dimension.
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
            // `omg::types::fixed<D, S>` (Spec) bzw. `dds::core::Fixed<D,S>`
            // (ZeroDDS-aequivalente Form). Digits/Scale aus AST.
            let digits = const_expr_to_u32(&f.digits).unwrap_or(0);
            let scale = const_expr_to_u32(&f.scale).unwrap_or(0);
            Ok(format!("::dds::core::Fixed<{digits}, {scale}>"))
        }
        TypeSpec::Any => {
            // Spec idl4-cpp §7.3: `any` -> `omg::types::Any`. Wir
            // emittieren als ZeroDDS-aequivalente `dds::core::Any`-Klasse
            // (Reflective-Container, Runtime-Implementation in
            // `dds-core`-Crate).
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
    // Mapping fuer Time/Duration (Block E).
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
// ConstExpr → C++-Literal-String (best-effort fuer Foundation)
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

/// Sucht ein `@<name>(N)` und liefert den uint32-Wert; sonst None.
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
/// Versucht, einen ConstExpr als positiven u32 zu deuten (nur Integer-Literal
/// oder Unary-Plus auf Integer-Literal).
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

/// Parser fuer integer-Literale (dezimal, hex `0x`, oktal `0...`).
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

/// Extensibility-Modus eines Structs aus seinen Annotations.
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

/// Walkt das AST und sammelt `child → parent`-Edges (FQN-Strings).
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

    // Cycle-Detection per visited-Set pro Knoten.
    for start in parents.keys() {
        let mut current = start.clone();
        let mut visited: BTreeSet<String> = BTreeSet::new();
        visited.insert(current.clone());
        while let Some(p) = parents.get(&current) {
            // Match flexibel: voller Schluessel oder Suffix-Match.
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
// Sammelt alle Top-Level- und Modul-nested-Structs und emittiert pro Struct
// eine `dds::topic::topic_type_support<FQN>`-Spezialisierung mit type_name(),
// encode(), encode_be(), decode(), key_hash(), is_keyed() und extensibility().
//
// Wire-Format ist voll-XCDR2 (XTypes 1.3 §7.4):
//   * Plain-CDR2 LE mit Alignment relativ zum Encapsulation-Start.
//   * `@final`           -> kein DHEADER.
//   * `@appendable`(def) -> DHEADER (4 Byte body-size) prefixed.
//   * `@mutable`         -> DHEADER + EMHEADER pro Member (PL_CDR2).
//   * `@key`             -> Member geht in Key-Hash (MD5 ueber BE-Plain-CDR2).
//   * `@id(N)`           -> EMHEADER member-id.
//   * `@optional`        -> EMHEADER skip falls absent (Mutable);
//                            1-Byte Present-Flag fuer Final/Appendable.
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

/// Liefert true falls die Member-Annotations encode-fest sind (Codegen
/// kann Wire-Bytes erzeugen). False bei `@shared` (Heap-Indirection,
/// noch nicht unterstuetzt). `@optional` ist erlaubt.
fn member_codegen_supported(m: &Member) -> bool {
    !has_shared_annotation(&m.annotations)
}

/// Liefert true wenn ein Type-Spec vom XCDR2-Codegen verstanden wird.
fn typespec_supported(ts: &TypeSpec) -> bool {
    match ts {
        TypeSpec::Primitive(_) => true,
        TypeSpec::String(s) => !s.wide,
        TypeSpec::Sequence(seq) => match &*seq.elem {
            TypeSpec::Primitive(_) => true,
            TypeSpec::String(s) => !s.wide,
            _ => false,
        },
        _ => false,
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
    let fn_name = if be { "encode_be" } else { "encode" };
    writeln!(
        out,
        "    static std::vector<uint8_t> {fn_name}(const {cpp_fqn}& __v) {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        std::vector<uint8_t> __out;").map_err(fmt_err)?;
    writeln!(out, "        (void)__v;").map_err(fmt_err)?;

    // Suffix for write helpers: write_le or write_be, write_string or write_string_be.
    let endian_suffix = if be { "be" } else { "le" };

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
                "        // xcdr2: @shared member '{name}' nicht unterstuetzt (skip)"
            )
            .map_err(fmt_err)?;
        }
        return Ok(());
    }
    let is_optional = has_optional_annotation(&m.annotations);
    for decl in &m.declarators {
        let name = &decl.name().text;
        if !matches!(decl, Declarator::Simple(_)) {
            writeln!(
                out,
                "        // xcdr2: array member '{name}' nicht unterstuetzt (skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        if !typespec_supported(&m.type_spec) {
            writeln!(
                out,
                "        // xcdr2: member '{name}' nicht unterstuetzt (nested/enum/wstring/map/fixed; skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        if is_optional {
            // Final/Appendable: 1-Byte present-flag, dann Wert wenn vorhanden.
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
            writeln!(
                out,
                "{pre}::dds::topic::xcdr2::write_{endian}_origin<{cpp_ty}>(__out, {origin}, {access});"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::String(s) if !s.wide => {
            if endian == "be" {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_string_be(__out, {access});"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{pre}::dds::topic::xcdr2::write_string_origin(__out, {origin}, {access});"
                )
                .map_err(fmt_err)?;
            }
        }
        TypeSpec::Sequence(seq) => {
            let count_call = if endian == "be" {
                format!(
                    "{pre}::dds::topic::xcdr2::write_be<uint32_t>(__out, static_cast<uint32_t>({access}.size()));"
                )
            } else {
                format!(
                    "{pre}::dds::topic::xcdr2::write_le_origin<uint32_t>(__out, {origin}, static_cast<uint32_t>({access}.size()));"
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
                            "{elem_indent}::dds::topic::xcdr2::write_le_origin<{cpp_ty}>(__out, {origin}, __e);"
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
                            "{elem_indent}::dds::topic::xcdr2::write_string_origin(__out, {origin}, __e);"
                        )
                        .map_err(fmt_err)?;
                    }
                }
                _ => {
                    writeln!(
                        out,
                        "{elem_indent}// xcdr2: nested/wstring sequence-element nicht unterstuetzt"
                    )
                    .map_err(fmt_err)?;
                }
            }
            writeln!(out, "{pre}}}").map_err(fmt_err)?;
        }
        _ => {
            writeln!(out, "{pre}// xcdr2: member type nicht unterstuetzt (skip)")
                .map_err(fmt_err)?;
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
                "        // xcdr2: @shared member '{name}' nicht unterstuetzt (skip)"
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
                "        // xcdr2: array member '{name}' nicht unterstuetzt (skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        if !typespec_supported(&m.type_spec) {
            writeln!(
                out,
                "        // xcdr2: member '{name}' nicht unterstuetzt (skip)"
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
                    "{indent}      ::dds::topic::xcdr2::write_string_origin(__out, __body_origin, {access});"
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
                    "{indent}      ::dds::topic::xcdr2::write_be<uint32_t>(__out, static_cast<uint32_t>({access}.size()));"
                )
                .map_err(fmt_err)?;
            } else {
                writeln!(
                    out,
                    "{indent}      ::dds::topic::xcdr2::write_le_origin<uint32_t>(__out, __body_origin, static_cast<uint32_t>({access}.size()));"
                )
                .map_err(fmt_err)?;
            }
            writeln!(out, "{indent}      for (const auto& __e : {access}) {{").map_err(fmt_err)?;
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
                        writeln!(out, "{indent}        ::dds::topic::xcdr2::write_le_origin<{cpp_ty}>(__out, __body_origin, __e);").map_err(fmt_err)?;
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
                        writeln!(out, "{indent}        ::dds::topic::xcdr2::write_string_origin(__out, __body_origin, __e);").map_err(fmt_err)?;
                    }
                }
                _ => {
                    writeln!(
                        out,
                        "{indent}        // xcdr2: nested seq-elem nicht unterstuetzt"
                    )
                    .map_err(fmt_err)?;
                }
            }
            writeln!(out, "{indent}      }}").map_err(fmt_err)?;
            writeln!(out, "{indent}    }}").map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}    ::dds::topic::xcdr2::emheader_nextint_end(__out, __sub); }}"
            )
            .map_err(fmt_err)?;
        }
        _ => {
            writeln!(out, "{indent}// xcdr2: nicht unterstuetzter member-type").map_err(fmt_err)?;
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
        "    static {cpp_fqn} decode(const uint8_t* __buf, size_t __len) {{"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        size_t __pos = 0;").map_err(fmt_err)?;
    writeln!(out, "        {cpp_fqn} __v;").map_err(fmt_err)?;
    writeln!(out, "        (void)__buf; (void)__len; (void)__pos;").map_err(fmt_err)?;

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
                "                    // unbekannter Member: NEXTINT-Skip falls LC>=3 ohne primitive-mapping."
            )
            .map_err(fmt_err)?;
            writeln!(out, "                    if (__h.lc == 0) {{ ++__pos; }}")
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
                "                    else {{ auto __n = ::dds::topic::xcdr2::emheader_nextint_read(__buf, __pos, __len); __pos += __n; }}"
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
                "        // xcdr2: @shared member '{name}' nicht unterstuetzt (skip)"
            )
            .map_err(fmt_err)?;
        }
        return Ok(());
    }
    let is_optional = has_optional_annotation(&m.annotations);
    for decl in &m.declarators {
        let name = &decl.name().text;
        if !matches!(decl, Declarator::Simple(_)) {
            writeln!(
                out,
                "        // xcdr2: array member '{name}' nicht unterstuetzt (skip)"
            )
            .map_err(fmt_err)?;
            continue;
        }
        if !typespec_supported(&m.type_spec) {
            writeln!(
                out,
                "        // xcdr2: member '{name}' nicht unterstuetzt (skip)"
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
                "{indent}{setter}(::dds::topic::xcdr2::read_le_origin<{cpp_ty}>(__buf, __pos, __len, {origin}));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::String(s) if !s.wide => {
            writeln!(
                out,
                "{indent}{setter}(::dds::topic::xcdr2::read_string_origin(__buf, __pos, __len, {origin}));"
            )
            .map_err(fmt_err)?;
        }
        TypeSpec::Sequence(seq) => {
            let elem_cpp_ty: String = match &*seq.elem {
                TypeSpec::Primitive(PrimitiveType::Boolean) => "bool".to_string(),
                TypeSpec::Primitive(p) => primitive_to_cpp(*p).to_string(),
                TypeSpec::String(s) if !s.wide => "std::string".to_string(),
                _ => {
                    writeln!(
                        out,
                        "{indent}// xcdr2: nested seq-elem nicht unterstuetzt (skip)"
                    )
                    .map_err(fmt_err)?;
                    return Ok(());
                }
            };
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            writeln!(out, "{indent}    auto __cnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(__buf, __pos, __len, {origin});").map_err(fmt_err)?;
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
                    writeln!(out, "{indent}        __seq.push_back(::dds::topic::xcdr2::read_le_origin<{cpp_ty}>(__buf, __pos, __len, {origin}));").map_err(fmt_err)?;
                }
                TypeSpec::String(s) if !s.wide => {
                    writeln!(out, "{indent}        __seq.push_back(::dds::topic::xcdr2::read_string_origin(__buf, __pos, __len, {origin}));").map_err(fmt_err)?;
                }
                _ => {}
            }
            writeln!(out, "{indent}    }}").map_err(fmt_err)?;
            writeln!(out, "{indent}    {setter}(std::move(__seq));").map_err(fmt_err)?;
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
                writeln!(out, "                    __v.{name}(::dds::topic::xcdr2::read_string_origin(__buf, __pos, __len, __body_origin));").map_err(fmt_err)?;
            }
            TypeSpec::Sequence(seq) => {
                let elem_cpp_ty: String = match &*seq.elem {
                    TypeSpec::Primitive(PrimitiveType::Boolean) => "bool".to_string(),
                    TypeSpec::Primitive(p) => primitive_to_cpp(*p).to_string(),
                    TypeSpec::String(s) if !s.wide => "std::string".to_string(),
                    _ => "uint8_t".to_string(),
                };
                writeln!(out, "                    auto __n = ::dds::topic::xcdr2::emheader_nextint_read(__buf, __pos, __len);").map_err(fmt_err)?;
                writeln!(out, "                    (void)__n;").map_err(fmt_err)?;
                writeln!(out, "                    auto __body_origin = __pos;")
                    .map_err(fmt_err)?;
                writeln!(out, "                    auto __cnt = ::dds::topic::xcdr2::read_le_origin<uint32_t>(__buf, __pos, __len, __body_origin);").map_err(fmt_err)?;
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
                    TypeSpec::Primitive(PrimitiveType::Octet) => {
                        writeln!(out, "                        __seq.push_back(::dds::topic::xcdr2::read_u8(__buf, __pos, __len));").map_err(fmt_err)?;
                    }
                    TypeSpec::Primitive(p) => {
                        let cpp_ty = primitive_to_cpp(*p);
                        writeln!(out, "                        __seq.push_back(::dds::topic::xcdr2::read_le_origin<{cpp_ty}>(__buf, __pos, __len, __body_origin));").map_err(fmt_err)?;
                    }
                    TypeSpec::String(s) if !s.wide => {
                        writeln!(out, "                        __seq.push_back(::dds::topic::xcdr2::read_string_origin(__buf, __pos, __len, __body_origin));").map_err(fmt_err)?;
                    }
                    _ => {}
                }
                writeln!(out, "                    }}").map_err(fmt_err)?;
                writeln!(out, "                    __v.{name}(std::move(__seq));")
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
    // XTypes 1.3 §7.6.8.4: Holder ≤ 16 octets -> zero-pad; sonst MD5.
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
    // is_reserved wird von check_identifier genutzt — Compiler-Hint.
    let _ = is_reserved("int");
}
