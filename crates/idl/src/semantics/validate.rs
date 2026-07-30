// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The shared semantic gate.
//!
//! [`resolve_and_validate`] runs the name resolver
//! ([`crate::semantics::resolver`]), the 19 spec-constraint validators
//! ([`crate::semantics::spec_validators`]) and a type-reference resolution
//! pass over a parsed [`Specification`]. It is the single semantic barrier
//! that both `zerodds-idlc` (the `check` *and* `generate` paths) and
//! `zerodds-idl-compose` call directly after parsing + default-patching, so
//! neither can emit code for IDL with duplicate declarations, duplicate /
//! case-colliding members, unresolved type names or violated spec
//! constraints.
//!
//! The reference-resolution pass exists because the resolver's scope build
//! only checks *definition-site* constraints — it never walks member type
//! references. Without it, an unknown member type (`DoesNotExist value;`)
//! reaches the backends verbatim. That pass runs here, independent of
//! XTypes TypeObject lowering, so `--no-typeobject` cannot bypass the gate.

use crate::ast::{
    Annotation, AnnotationParams, AttrDecl, ComponentDcl, ConstExpr, ConstrTypeDecl, Definition,
    EventDcl, Export, HomeDcl, IntegerType, InterfaceDcl, InterfaceDef, LiteralKind, Member,
    OpDecl, ParamDecl, PrimitiveType, ScopedName, Specification, StateMember, StructDcl,
    SwitchTypeSpec, TypeDecl, TypeSpec, UnaryOp, UnionDcl, ValueDef, ValueElement,
};
use crate::errors::Span;
use crate::semantics::annotations::{LowerError, lower_single};
use crate::semantics::bitfield_validation::{BitfieldValidationError, validate_bitfields};
use crate::semantics::resolver::{Resolver, ResolverError};
use crate::semantics::spec_validators::{SpecValidationError, validate_all_with_pragmas};

/// A rejected annotation found by the gate's annotation pass: a recognized
/// builtin annotation carried an argument of the wrong type/shape (broad-audit
/// P1, e.g. `@autoid(1)`, `@extensibility(1)`, `@position("x")`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationError {
    /// The lowering error describing why the argument is invalid.
    pub error: LowerError,
    /// Source location of the offending annotation.
    pub span: Span,
}

impl core::fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "invalid annotation at byte {}..{}: {}",
            self.span.start, self.span.end, self.error
        )
    }
}

/// Aggregate of every semantic error found by [`resolve_and_validate`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SemanticErrors {
    /// Resolver + reference-resolution errors (duplicate / case-colliding
    /// declarations and members, keyword collisions, unfinished forward
    /// decls, oneway violations, unresolved type names).
    pub resolver: Vec<ResolverError>,
    /// The 19 spec-constraint validator findings.
    pub spec: Vec<SpecValidationError>,
    /// Builtin-annotation lowering errors (broad-audit P1): a known builtin
    /// annotation with a wrong-typed/invalid argument. Previously swallowed
    /// as `Ok(None)`/`unwrap_or(0)` so codegen ran with a default value.
    pub annotations: Vec<AnnotationError>,
    /// Bitset/bitmask constraint findings (§7.4.13.4.3): out-of-range /
    /// duplicate / colliding `@position`, bit_bound over 64, bitfield width
    /// over the dest_type cap, bitset total over 64. Previously the
    /// [`validate_bitfields`] pass existed but was never wired into the gate,
    /// so a bitset with two bitfields on colliding bit ranges reached the
    /// backends verbatim.
    pub bitfields: Vec<BitfieldValidationError>,
    /// `@default(value)` type-conversion findings: a `@default` literal whose
    /// type does not match the member type (a string default on an integer
    /// member, an out-of-range integer literal, a boolean default on a numeric
    /// member, ...). Previously `@default` lowered to an opaque string
    /// (`BuiltinAnnotation::Default(String)`) with no type check, so a
    /// mismatched literal was silently mis-converted downstream.
    pub defaults: Vec<DefaultValueError>,
}

impl SemanticErrors {
    /// `true` when no error was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resolver.is_empty()
            && self.spec.is_empty()
            && self.annotations.is_empty()
            && self.bitfields.is_empty()
            && self.defaults.is_empty()
    }

    /// Total number of recorded errors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.resolver.len()
            + self.spec.len()
            + self.annotations.len()
            + self.bitfields.len()
            + self.defaults.len()
    }

    /// One human-readable message per error, resolver findings first.
    #[must_use]
    pub fn messages(&self) -> Vec<String> {
        self.resolver
            .iter()
            .map(ToString::to_string)
            .chain(self.spec.iter().map(ToString::to_string))
            .chain(self.annotations.iter().map(ToString::to_string))
            .chain(self.bitfields.iter().map(ToString::to_string))
            .chain(self.defaults.iter().map(ToString::to_string))
            .collect()
    }
}

impl core::fmt::Display for SemanticErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msgs = self.messages();
        for (i, m) in msgs.iter().enumerate() {
            if i + 1 == msgs.len() {
                write!(f, "{m}")?;
            } else {
                writeln!(f, "{m}")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for SemanticErrors {}

/// Runs the full semantic gate on a parsed spec.
///
/// `pragma_prefixes` carries `(prefix, line)` tuples from the preprocessor
/// so §7.4.6.4.1.3 repository-id conflicts can be diagnosed; pass `&[]`
/// when no preprocessor output is available.
///
/// # Errors
/// Returns [`SemanticErrors`] when any resolver finding, unresolved type
/// reference or spec-constraint violation is present. On a clean spec it
/// returns `Ok(())`.
pub fn resolve_and_validate(
    spec: &Specification,
    pragma_prefixes: &[(String, usize)],
) -> Result<(), SemanticErrors> {
    let mut resolver = Resolver::new();
    resolver.build(spec);

    let mut resolver_errors = core::mem::take(&mut resolver.errors);
    resolver_errors.extend(resolver.forward_decl_errors());
    collect_unresolved_type_refs(spec, &resolver, &mut resolver_errors);

    let spec_errors = validate_all_with_pragmas(spec, &resolver, pragma_prefixes);

    let mut annotation_errors = Vec::new();
    collect_annotation_errors(spec, &mut annotation_errors);

    let bitfield_errors = validate_bitfields(spec);

    let mut default_errors = Vec::new();
    collect_default_type_errors(spec, &mut default_errors);

    if resolver_errors.is_empty()
        && spec_errors.is_empty()
        && annotation_errors.is_empty()
        && bitfield_errors.is_empty()
        && default_errors.is_empty()
    {
        Ok(())
    } else {
        Err(SemanticErrors {
            resolver: resolver_errors,
            spec: spec_errors,
            annotations: annotation_errors,
            bitfields: bitfield_errors,
            defaults: default_errors,
        })
    }
}

// ============================================================================
// Annotation-lowering pass (broad-audit P1)
// ============================================================================

/// Walks every annotation-bearing node in the spec and lowers each annotation.
/// A recognized builtin annotation with a wrong-typed/invalid argument
/// (`@autoid(1)`, `@extensibility(1)`, `@position("x")`, `@id("x")`,
/// `@bit_bound("x")`, …) is reported as an [`AnnotationError`]. Unknown /
/// vendor-specific annotations lower to `Ok(None)` and are ignored here — only
/// KNOWN builtins with a bad argument fail the gate.
fn collect_annotation_errors(spec: &Specification, out: &mut Vec<AnnotationError>) {
    for def in &spec.definitions {
        walk_def_anns(def, out);
    }
}

/// Lowers one annotation list; records a wrong-typed builtin argument.
fn check_anns(anns: &[Annotation], out: &mut Vec<AnnotationError>) {
    for a in anns {
        if let Err(error) = lower_single(a) {
            out.push(AnnotationError {
                error,
                span: a.span,
            });
        }
    }
}

/// zerodds-lint: recursion-depth 64 (module hierarchy; bounded by IDL nesting)
fn walk_def_anns(def: &Definition, out: &mut Vec<AnnotationError>) {
    match def {
        Definition::Module(m) => {
            check_anns(&m.annotations, out);
            for d in &m.definitions {
                walk_def_anns(d, out);
            }
        }
        Definition::TemplateModule(t) => {
            for d in &t.definitions {
                walk_def_anns(d, out);
            }
        }
        Definition::Type(t) => walk_type_decl_anns(t, out),
        Definition::Const(c) => check_anns(&c.annotations, out),
        Definition::Except(e) => {
            check_anns(&e.annotations, out);
            for m in &e.members {
                check_anns(&m.annotations, out);
            }
        }
        Definition::Interface(InterfaceDcl::Def(i)) => {
            check_anns(&i.annotations, out);
            for ex in &i.exports {
                walk_export_anns(ex, out);
            }
        }
        Definition::ValueBox(v) => check_anns(&v.annotations, out),
        Definition::ValueDef(v) => {
            check_anns(&v.annotations, out);
            for el in &v.elements {
                walk_value_element_anns(el, out);
            }
        }
        Definition::Event(EventDcl::Def(e)) => {
            check_anns(&e.annotations, out);
            for el in &e.elements {
                walk_value_element_anns(el, out);
            }
        }
        Definition::Component(ComponentDcl::Def(c)) => check_anns(&c.annotations, out),
        Definition::Home(HomeDcl::Def(h)) => check_anns(&h.annotations, out),
        // No annotation-bearing surface: forwards, imports, typeid/typeprefix,
        // porttype/connector, template-module instances, annotation decls,
        // vendor extensions.
        _ => {}
    }
}

/// zerodds-lint: recursion-depth 64 (nested type decls; bounded by IDL nesting)
fn walk_type_decl_anns(t: &TypeDecl, out: &mut Vec<AnnotationError>) {
    match t {
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            check_anns(&s.annotations, out);
            for m in &s.members {
                check_anns(&m.annotations, out);
            }
        }
        TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u))) => {
            check_anns(&u.annotations, out);
            for c in &u.cases {
                check_anns(&c.annotations, out);
                check_anns(&c.element.annotations, out);
            }
        }
        TypeDecl::Constr(ConstrTypeDecl::Enum(e)) => {
            check_anns(&e.annotations, out);
            for en in &e.enumerators {
                check_anns(&en.annotations, out);
            }
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitset(b)) => {
            check_anns(&b.annotations, out);
            for bf in &b.bitfields {
                check_anns(&bf.annotations, out);
            }
        }
        TypeDecl::Constr(ConstrTypeDecl::Bitmask(b)) => {
            check_anns(&b.annotations, out);
            for v in &b.values {
                check_anns(&v.annotations, out);
            }
        }
        TypeDecl::Typedef(td) => check_anns(&td.annotations, out),
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Forward(_)))
        | TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Forward(_)))
        | TypeDecl::Native(_) => {}
    }
}

/// zerodds-lint: recursion-depth 64 (nested type decls; bounded by IDL nesting)
fn walk_export_anns(ex: &Export, out: &mut Vec<AnnotationError>) {
    match ex {
        Export::Op(op) => {
            check_anns(&op.annotations, out);
            for p in &op.params {
                check_anns(&p.annotations, out);
            }
        }
        Export::Attr(a) => check_anns(&a.annotations, out),
        Export::Type(t) => walk_type_decl_anns(t, out),
        Export::Const(c) => check_anns(&c.annotations, out),
        Export::Except(e) => {
            check_anns(&e.annotations, out);
            for m in &e.members {
                check_anns(&m.annotations, out);
            }
        }
    }
}

fn walk_value_element_anns(el: &ValueElement, out: &mut Vec<AnnotationError>) {
    match el {
        ValueElement::Export(ex) => walk_export_anns(ex, out),
        ValueElement::State(sm) => check_anns(&sm.annotations, out),
        ValueElement::Init(_) => {}
    }
}

// ============================================================================
// Reference-resolution pass
// ============================================================================

/// Walks the data-type surface (struct / union / exception members, typedef
/// targets, struct base + union discriminant) and reports every named type
/// reference the resolver cannot resolve. Only `UnresolvedName` findings are
/// surfaced; use-site case mismatches are left to the resolver's own
/// definition-side checks so this pass does not newly reject IDL that
/// merely differs in casing.
///
/// zerodds-lint: recursion-depth 64 (AST walk; bounded by IDL nesting)
fn collect_unresolved_type_refs(
    spec: &Specification,
    resolver: &Resolver,
    out: &mut Vec<ResolverError>,
) {
    walk_defs(&spec.definitions, &[], resolver, out);
}

/// zerodds-lint: recursion-depth 64 (module hierarchy; bounded by IDL nesting)
fn walk_defs(
    defs: &[Definition],
    path: &[String],
    resolver: &Resolver,
    out: &mut Vec<ResolverError>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let mut np = path.to_vec();
                np.push(m.name.text.clone());
                walk_defs(&m.definitions, &np, resolver, out);
            }
            Definition::TemplateModule(t) => {
                let mut np = path.to_vec();
                np.push(t.name.text.clone());
                walk_defs(&t.definitions, &np, resolver, out);
            }
            Definition::Type(t) => walk_type_decl(t, path, resolver, out),
            Definition::Except(e) => {
                for member in &e.members {
                    check_type_spec(&member.type_spec, path, resolver, out);
                }
            }
            Definition::Interface(InterfaceDcl::Def(i)) => {
                walk_interface_refs(i, path, resolver, out);
            }
            Definition::ValueDef(v) => {
                walk_value_refs(v, path, resolver, out);
            }
            Definition::ValueBox(v) => {
                // The value-box inner type must be a declared type; the
                // separate spec validator only checks it is not *itself* a
                // value type, not that it resolves at all.
                check_type_spec(&v.type_spec, path, resolver, out);
            }
            _ => {}
        }
    }
}

/// Walks an interface body: base references, operation return + parameter
/// types and attribute types. Nested type/exception exports are walked so a
/// struct declared inside the interface is covered too. This is the
/// valuetype/interface half of the reference surface that the data-type walk
/// (`walk_type_decl`) does not reach.
fn walk_interface_refs(
    i: &InterfaceDef,
    path: &[String],
    resolver: &Resolver,
    out: &mut Vec<ResolverError>,
) {
    for base in &i.bases {
        check_scoped(base, path, resolver, out);
    }
    for ex in &i.exports {
        walk_export_refs(ex, path, resolver, out);
    }
}

/// Walks a valuetype body: base references, state-member types and the
/// operation/attribute exports. `supports`/`raises` clauses are intentionally
/// left to the resolver's own definition-side checks.
fn walk_value_refs(
    v: &ValueDef,
    path: &[String],
    resolver: &Resolver,
    out: &mut Vec<ResolverError>,
) {
    if let Some(inh) = &v.inheritance {
        for base in &inh.bases {
            check_scoped(base, path, resolver, out);
        }
    }
    for el in &v.elements {
        match el {
            ValueElement::State(StateMember { type_spec, .. }) => {
                check_type_spec(type_spec, path, resolver, out);
            }
            ValueElement::Export(ex) => walk_export_refs(ex, path, resolver, out),
            ValueElement::Init(_) => {}
        }
    }
}

/// Walks a single interface/valuetype export for unresolved type references:
/// operation return + parameter types, attribute types, nested type decls and
/// nested exception members.
fn walk_export_refs(
    ex: &Export,
    path: &[String],
    resolver: &Resolver,
    out: &mut Vec<ResolverError>,
) {
    match ex {
        Export::Op(OpDecl {
            return_type,
            params,
            ..
        }) => {
            if let Some(rt) = return_type {
                check_type_spec(rt, path, resolver, out);
            }
            for ParamDecl { type_spec, .. } in params {
                check_type_spec(type_spec, path, resolver, out);
            }
        }
        Export::Attr(AttrDecl { type_spec, .. }) => {
            check_type_spec(type_spec, path, resolver, out);
        }
        Export::Type(t) => walk_type_decl(t, path, resolver, out),
        Export::Except(e) => {
            for member in &e.members {
                check_type_spec(&member.type_spec, path, resolver, out);
            }
        }
        Export::Const(_) => {}
    }
}

fn walk_type_decl(
    t: &TypeDecl,
    path: &[String],
    resolver: &Resolver,
    out: &mut Vec<ResolverError>,
) {
    match t {
        TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s))) => {
            if let Some(base) = &s.base {
                check_scoped(base, path, resolver, out);
            }
            for member in &s.members {
                check_type_spec(&member.type_spec, path, resolver, out);
            }
        }
        TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u))) => {
            if let SwitchTypeSpec::Scoped(sn) = &u.switch_type {
                check_scoped(sn, path, resolver, out);
            }
            for case in &u.cases {
                check_type_spec(&case.element.type_spec, path, resolver, out);
            }
        }
        TypeDecl::Typedef(td) => {
            check_type_spec(&td.type_spec, path, resolver, out);
        }
        _ => {}
    }
}

/// zerodds-lint: recursion-depth 32 (nested sequence/map; bounded by IDL nesting)
fn check_type_spec(
    ts: &TypeSpec,
    path: &[String],
    resolver: &Resolver,
    out: &mut Vec<ResolverError>,
) {
    match ts {
        TypeSpec::Scoped(sn) => check_scoped(sn, path, resolver, out),
        TypeSpec::Sequence(seq) => check_type_spec(&seq.elem, path, resolver, out),
        TypeSpec::Map(map) => {
            check_type_spec(&map.key, path, resolver, out);
            check_type_spec(&map.value, path, resolver, out);
        }
        TypeSpec::Primitive(_) | TypeSpec::String(_) | TypeSpec::Fixed(_) | TypeSpec::Any => {}
    }
}

fn check_scoped(
    sn: &ScopedName,
    path: &[String],
    resolver: &Resolver,
    out: &mut Vec<ResolverError>,
) {
    if is_builtin_pseudo(sn) {
        return;
    }
    if let Err(ResolverError::UnresolvedName { name, span }) = resolver.resolve(sn, path) {
        out.push(ResolverError::UnresolvedName { name, span });
    }
}

/// CORBA / built-in pseudo object types that are legal member/return types
/// without an in-IDL declaration. The resolver has no symbol for these, so
/// the reference pass must not flag them.
fn is_builtin_pseudo(sn: &ScopedName) -> bool {
    let Some(last) = sn.parts.last() else {
        return true;
    };
    matches!(
        last.text.as_str(),
        "Object" | "ValueBase" | "TypeCode" | "any" | "Any"
    )
}

// ============================================================================
// @default type-conversion pass
// ============================================================================

/// How a `@default(value)` literal is incompatible with the member type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultMismatch {
    /// The literal is of the wrong kind for the member type (e.g. a string
    /// default on an integer member).
    TypeMismatch {
        /// Human-readable name of the expected value category.
        expected: &'static str,
        /// Human-readable name of the literal that was found.
        found: &'static str,
    },
    /// The literal is of the right kind but does not fit the member type's
    /// value range (e.g. `@default(70000)` on a `short`).
    OutOfRange {
        /// The offending literal, as written.
        value: String,
        /// Name of the member's integer type.
        type_name: &'static str,
    },
}

/// A `@default(value)` whose literal does not match the annotated member type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultValueError {
    /// Name of the member carrying the `@default`.
    pub member: String,
    /// Why the default is rejected.
    pub mismatch: DefaultMismatch,
    /// Source location of the `@default` annotation.
    pub span: Span,
}

impl core::fmt::Display for DefaultValueError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.mismatch {
            DefaultMismatch::TypeMismatch { expected, found } => write!(
                f,
                "@default on member '{}' has a {found} value but the member type expects a {expected}",
                self.member
            ),
            DefaultMismatch::OutOfRange { value, type_name } => write!(
                f,
                "@default({value}) on member '{}' is out of range for {type_name}",
                self.member
            ),
        }
    }
}

impl std::error::Error for DefaultValueError {}

/// Walks every member-bearing construct (struct, union case, exception,
/// valuetype state member) and type-checks each `@default(value)` against the
/// member type. A `@default` whose literal type does not match the member type
/// — a string default on an integer member, an out-of-range integer literal, a
/// boolean default on a numeric member — is reported. `@default` values given
/// as a named constant / enum literal (`ConstExpr::Scoped`) or as an arithmetic
/// expression are left alone; only a concrete literal (optionally sign-prefixed)
/// is checked, so a legitimate `@default(TRUE)` / `@default(SomeEnumerator)`
/// still passes.
///
/// zerodds-lint: recursion-depth 64 (AST walk; bounded by IDL nesting)
fn collect_default_type_errors(spec: &Specification, out: &mut Vec<DefaultValueError>) {
    walk_default_defs(&spec.definitions, out);
}

/// zerodds-lint: recursion-depth 64 (module hierarchy; bounded by IDL nesting)
fn walk_default_defs(defs: &[Definition], out: &mut Vec<DefaultValueError>) {
    for d in defs {
        match d {
            Definition::Module(m) => walk_default_defs(&m.definitions, out),
            Definition::TemplateModule(t) => walk_default_defs(&t.definitions, out),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                for m in &s.members {
                    check_member_default(m, out);
                }
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                for c in &u.cases {
                    check_default_on(
                        &c.element.type_spec,
                        &c.element.annotations,
                        &declarator_name(&c.element.declarator),
                        out,
                    );
                }
            }
            Definition::Except(e) => {
                for m in &e.members {
                    check_member_default(m, out);
                }
            }
            Definition::ValueDef(v) => {
                for el in &v.elements {
                    if let ValueElement::State(sm) = el {
                        for decl in &sm.declarators {
                            check_default_on(
                                &sm.type_spec,
                                &sm.annotations,
                                decl.name().text.as_str(),
                                out,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn declarator_name(d: &crate::ast::Declarator) -> String {
    d.name().text.clone()
}

/// Type-checks a struct/exception member's `@default` against its type, once
/// per declarator so the diagnostic names the right field.
fn check_member_default(m: &Member, out: &mut Vec<DefaultValueError>) {
    for decl in &m.declarators {
        check_default_on(&m.type_spec, &m.annotations, decl.name().text.as_str(), out);
    }
}

/// The single-value literal argument of a `@default(...)` annotation in `anns`,
/// if exactly one such annotation with a single positional argument is present.
fn default_arg(anns: &[Annotation]) -> Option<&ConstExpr> {
    anns.iter().find_map(|a| {
        let is_default = a.name.parts.last().map(|p| p.text.as_str()) == Some("default");
        match (&a.params, is_default) {
            (AnnotationParams::Single(e), true) => Some(e),
            _ => None,
        }
    })
}

/// Classifies a `@default` argument expression as a checkable literal:
/// returns `(literal_kind, raw_text, negated)`. A sign-prefixed numeric literal
/// (`-5`, `+3.0`) is unwrapped and its sign recorded. Named constants / enum
/// literals (`ConstExpr::Scoped`) and arithmetic expressions yield `None` — they
/// are not literal type mismatches and are left for later stages.
fn classify_default(expr: &ConstExpr) -> Option<(LiteralKind, &str, bool)> {
    match expr {
        ConstExpr::Literal(l) => Some((l.kind, l.raw.as_str(), false)),
        ConstExpr::Unary { op, operand, .. } => match (op, operand.as_ref()) {
            (UnaryOp::Minus, ConstExpr::Literal(l)) => Some((l.kind, l.raw.as_str(), true)),
            (UnaryOp::Plus, ConstExpr::Literal(l)) => Some((l.kind, l.raw.as_str(), false)),
            _ => None,
        },
        _ => None,
    }
}

fn literal_kind_name(kind: LiteralKind) -> &'static str {
    match kind {
        LiteralKind::Integer => "integer",
        LiteralKind::Floating => "floating-point",
        LiteralKind::Fixed => "fixed-point",
        LiteralKind::Char => "character",
        LiteralKind::WideChar => "wide character",
        LiteralKind::String => "string",
        LiteralKind::WideString => "wide string",
        LiteralKind::Boolean => "boolean",
    }
}

/// Type-checks one `@default` literal against a member `type_spec`.
fn check_default_on(
    type_spec: &TypeSpec,
    anns: &[Annotation],
    member: &str,
    out: &mut Vec<DefaultValueError>,
) {
    let Some(expr) = default_arg(anns) else {
        return;
    };
    let Some((kind, raw, negated)) = classify_default(expr) else {
        return;
    };
    let span = expr.span();
    let mismatch = |expected: &'static str| DefaultValueError {
        member: member.to_string(),
        mismatch: DefaultMismatch::TypeMismatch {
            expected,
            found: literal_kind_name(kind),
        },
        span,
    };
    match type_spec {
        TypeSpec::Primitive(PrimitiveType::Integer(it)) => {
            if kind == LiteralKind::Integer {
                if let Some(err) = int_range_error(*it, raw, negated, member, span) {
                    out.push(err);
                }
            } else {
                out.push(mismatch("integer"));
            }
        }
        TypeSpec::Primitive(PrimitiveType::Octet) => {
            if kind == LiteralKind::Integer {
                if let Some(err) = octet_range_error(raw, negated, member, span) {
                    out.push(err);
                }
            } else {
                out.push(mismatch("octet"));
            }
        }
        TypeSpec::Primitive(PrimitiveType::Floating(_)) => {
            if !matches!(
                kind,
                LiteralKind::Integer | LiteralKind::Floating | LiteralKind::Fixed
            ) {
                out.push(mismatch("floating-point number"));
            }
        }
        TypeSpec::Primitive(PrimitiveType::Boolean) => {
            if kind != LiteralKind::Boolean {
                out.push(mismatch("boolean"));
            }
        }
        TypeSpec::Primitive(PrimitiveType::Char) => {
            if kind != LiteralKind::Char {
                out.push(mismatch("character"));
            }
        }
        TypeSpec::Primitive(PrimitiveType::WideChar) => {
            if !matches!(kind, LiteralKind::Char | LiteralKind::WideChar) {
                out.push(mismatch("character"));
            }
        }
        TypeSpec::String(s) if !s.wide => {
            if kind != LiteralKind::String {
                out.push(mismatch("string"));
            }
        }
        TypeSpec::String(_) => {
            if !matches!(kind, LiteralKind::String | LiteralKind::WideString) {
                out.push(mismatch("wide string"));
            }
        }
        // Scoped/sequence/map/fixed/any members: a `@default` here would carry
        // a named value or is not a scalar literal target — out of scope for
        // the literal type check.
        _ => {}
    }
}

/// Integer value bounds `[min, max]` for one `IntegerType`.
fn int_bounds(it: IntegerType) -> (i128, i128, &'static str) {
    match it {
        IntegerType::Int8 => (-128, 127, "int8"),
        IntegerType::UInt8 => (0, 255, "uint8"),
        IntegerType::Short | IntegerType::Int16 => (-32_768, 32_767, "short/int16"),
        IntegerType::UShort | IntegerType::UInt16 => (0, 65_535, "unsigned short/uint16"),
        IntegerType::Long | IntegerType::Int32 => {
            (i128::from(i32::MIN), i128::from(i32::MAX), "long/int32")
        }
        IntegerType::ULong | IntegerType::UInt32 => {
            (0, i128::from(u32::MAX), "unsigned long/uint32")
        }
        IntegerType::LongLong | IntegerType::Int64 => (
            i128::from(i64::MIN),
            i128::from(i64::MAX),
            "long long/int64",
        ),
        IntegerType::ULongLong | IntegerType::UInt64 => {
            (0, i128::from(u64::MAX), "unsigned long long/uint64")
        }
    }
}

fn int_range_error(
    it: IntegerType,
    raw: &str,
    negated: bool,
    member: &str,
    span: Span,
) -> Option<DefaultValueError> {
    let value = parse_int_literal(raw, negated)?;
    let (min, max, type_name) = int_bounds(it);
    if value < min || value > max {
        return Some(DefaultValueError {
            member: member.to_string(),
            mismatch: DefaultMismatch::OutOfRange {
                value: signed_raw(raw, negated),
                type_name,
            },
            span,
        });
    }
    None
}

fn octet_range_error(
    raw: &str,
    negated: bool,
    member: &str,
    span: Span,
) -> Option<DefaultValueError> {
    let value = parse_int_literal(raw, negated)?;
    if !(0..=255).contains(&value) {
        return Some(DefaultValueError {
            member: member.to_string(),
            mismatch: DefaultMismatch::OutOfRange {
                value: signed_raw(raw, negated),
                type_name: "octet",
            },
            span,
        });
    }
    None
}

fn signed_raw(raw: &str, negated: bool) -> String {
    if negated {
        format!("-{}", raw.trim())
    } else {
        raw.trim().to_string()
    }
}

/// Parses an IDL integer literal (`raw`, plus an external sign from a unary
/// operator) into an `i128`. Decimal, `0x`/`0X` hex and leading-zero octal are
/// handled; an inline `+`/`-` sign in `raw` is honored too. Returns `None` on a
/// form we cannot parse — the range check is then skipped rather than reporting
/// a false positive.
fn parse_int_literal(raw: &str, external_neg: bool) -> Option<i128> {
    let t = raw.trim();
    let (inline_neg, body) = if let Some(r) = t.strip_prefix('-') {
        (true, r.trim_start())
    } else if let Some(r) = t.strip_prefix('+') {
        (false, r.trim_start())
    } else {
        (false, t)
    };
    let neg = inline_neg ^ external_neg;
    let magnitude: i128 =
        if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
            i128::from_str_radix(hex, 16).ok()?
        } else if body.len() > 1
            && body.starts_with('0')
            && body.bytes().all(|b| (b'0'..=b'7').contains(&b))
        {
            i128::from_str_radix(body, 8).ok()?
        } else {
            body.parse::<i128>().ok()?
        };
    Some(if neg { -magnitude } else { magnitude })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn parse_ok(src: &str) -> Specification {
        parse(src, &ParserConfig::default()).expect("parse ok")
    }

    /// CORBA-full profile (valuetypes/interfaces enabled) for the
    /// valuetype reference tests.
    fn parse_full(src: &str) -> Specification {
        parse(src, &ParserConfig::full_4_2()).expect("parse ok")
    }

    #[test]
    fn clean_struct_passes() {
        let spec = parse_ok("struct Good { long a; string b; };");
        assert!(resolve_and_validate(&spec, &[]).is_ok());
    }

    #[test]
    fn duplicate_member_is_rejected() {
        let spec = parse_ok("struct S { long value; long value; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.resolver
                .iter()
                .any(|e| matches!(e, ResolverError::CaseConflict { .. })),
            "got {:?}",
            err.resolver
        );
    }

    #[test]
    fn unknown_member_type_is_rejected() {
        let spec = parse_ok("struct S { DoesNotExist value; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.resolver.iter().any(|e| matches!(
                e,
                ResolverError::UnresolvedName { name, .. } if name == "DoesNotExist"
            )),
            "got {:?}",
            err.resolver
        );
    }

    #[test]
    fn duplicate_type_definition_is_rejected() {
        let spec = parse_ok("struct Foo { long x; }; struct Foo { long y; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.resolver
                .iter()
                .any(|e| matches!(e, ResolverError::DuplicateDefinition { .. }))
        );
    }

    #[test]
    fn known_cross_reference_resolves() {
        let spec = parse_ok("struct Inner { long a; }; struct Outer { Inner nested; };");
        assert!(resolve_and_validate(&spec, &[]).is_ok());
    }

    #[test]
    fn scoped_cross_module_reference_resolves() {
        let spec =
            parse_ok("module M { struct Inner { long a; }; }; struct Outer { M::Inner nested; };");
        assert!(resolve_and_validate(&spec, &[]).is_ok());
    }

    #[test]
    fn unknown_type_inside_sequence_is_rejected() {
        let spec = parse_ok("struct S { sequence<Missing> items; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.resolver.iter().any(|e| matches!(
                e,
                ResolverError::UnresolvedName { name, .. } if name == "Missing"
            )),
            "got {:?}",
            err.resolver
        );
    }

    #[test]
    fn typedef_to_unknown_type_is_rejected() {
        let spec = parse_ok("typedef Nope MyAlias;");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(err.resolver.iter().any(|e| matches!(
            e,
            ResolverError::UnresolvedName { name, .. } if name == "Nope"
        )));
    }

    // ---- broad-audit P1: wrong-typed builtin annotation arguments ---------

    fn ann_err_variant(spec_src: &str) -> LowerError {
        let spec = parse_ok(spec_src);
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert_eq!(
            err.annotations.len(),
            1,
            "expected exactly one annotation error, got {:?}",
            err.annotations
        );
        err.annotations[0].error.clone()
    }

    #[test]
    fn autoid_with_integer_argument_is_rejected() {
        // `@autoid(1)` — integer where SEQUENTIAL|HASH is required.
        assert_eq!(
            ann_err_variant("@autoid(1) struct S { long x; };"),
            LowerError::WrongAnnotationArgument {
                annotation: "autoid".into(),
                expected: "SEQUENTIAL or HASH".into(),
            }
        );
    }

    #[test]
    fn extensibility_with_integer_argument_is_rejected() {
        // `@extensibility(1)` — integer where FINAL|APPENDABLE|MUTABLE required.
        assert_eq!(
            ann_err_variant("@extensibility(1) struct S { long x; };"),
            LowerError::WrongAnnotationArgument {
                annotation: "extensibility".into(),
                expected: "FINAL, APPENDABLE or MUTABLE".into(),
            }
        );
    }

    #[test]
    fn position_with_string_argument_is_rejected() {
        // `@position("x")` — string where a non-negative integer is required.
        assert_eq!(
            ann_err_variant("bitmask Flags { @position(\"x\") F0 };"),
            LowerError::WrongAnnotationArgument {
                annotation: "position".into(),
                expected: "a non-negative integer (0..=65535)".into(),
            }
        );
    }

    #[test]
    fn wrong_annotation_argument_on_member_is_rejected() {
        // The pass reaches member-level annotations too, not just the type.
        let spec = parse_ok("struct S { @id(\"x\") long a; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.annotations
                .iter()
                .any(|e| e.error == LowerError::InvalidIdArgument),
            "got {:?}",
            err.annotations
        );
    }

    #[test]
    fn valid_builtin_annotations_pass_the_gate() {
        // Counter-corpus: the legitimate forms must still validate cleanly.
        assert!(
            resolve_and_validate(&parse_ok("@autoid(HASH) struct S { long x; };"), &[]).is_ok()
        );
        assert!(
            resolve_and_validate(
                &parse_ok("@extensibility(MUTABLE) struct S { long x; };"),
                &[]
            )
            .is_ok()
        );
        assert!(resolve_and_validate(&parse_ok("bitmask Flags { @position(3) F0 };"), &[]).is_ok());
        // A bare @autoid (no argument) is not a wrong-typed argument.
        assert!(resolve_and_validate(&parse_ok("@autoid struct S { long x; };"), &[]).is_ok());
    }

    // ---- Class (a): valuetype / interface unresolved references -----------

    #[test]
    fn interface_op_return_of_unknown_type_is_rejected() {
        let spec = parse_ok("interface I { Missing get(); };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.resolver.iter().any(|e| matches!(
                e,
                ResolverError::UnresolvedName { name, .. } if name == "Missing"
            )),
            "got {:?}",
            err.resolver
        );
    }

    #[test]
    fn interface_op_param_of_unknown_type_is_rejected() {
        let spec = parse_ok("interface I { void set(in Missing v); };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(err.resolver.iter().any(|e| matches!(
            e,
            ResolverError::UnresolvedName { name, .. } if name == "Missing"
        )));
    }

    #[test]
    fn interface_base_of_unknown_type_is_rejected() {
        let spec = parse_ok("interface I : Missing { };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(err.resolver.iter().any(|e| matches!(
            e,
            ResolverError::UnresolvedName { name, .. } if name == "Missing"
        )));
    }

    #[test]
    fn valuetype_state_member_of_unknown_type_is_rejected() {
        let spec = parse_full("valuetype V { public Missing field; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(err.resolver.iter().any(|e| matches!(
            e,
            ResolverError::UnresolvedName { name, .. } if name == "Missing"
        )));
    }

    #[test]
    fn valuetype_base_of_unknown_type_is_rejected() {
        let spec = parse_full("valuetype V : Missing { };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(err.resolver.iter().any(|e| matches!(
            e,
            ResolverError::UnresolvedName { name, .. } if name == "Missing"
        )));
    }

    #[test]
    fn valuetype_and_interface_with_known_refs_pass() {
        // Counter-corpus: legitimate cross-references must still resolve.
        assert!(
            resolve_and_validate(
                &parse_full(
                    "struct Payload { long a; };\n\
                     interface Base { };\n\
                     interface I : Base { Payload fetch(in Payload p); };\n\
                     valuetype V { public Payload field; };"
                ),
                &[]
            )
            .is_ok()
        );
    }

    // ---- Class (c): @default type conversion ------------------------------

    #[test]
    fn default_string_on_integer_member_is_rejected() {
        let spec = parse_ok("struct S { @default(\"oops\") long x; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.defaults.iter().any(|e| matches!(
                &e.mismatch,
                DefaultMismatch::TypeMismatch {
                    found: "string",
                    ..
                }
            )),
            "got {:?}",
            err.defaults
        );
    }

    #[test]
    fn default_out_of_range_on_short_is_rejected() {
        let spec = parse_ok("struct S { @default(70000) short x; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.defaults
                .iter()
                .any(|e| matches!(&e.mismatch, DefaultMismatch::OutOfRange { .. })),
            "got {:?}",
            err.defaults
        );
    }

    #[test]
    fn default_boolean_on_integer_member_is_rejected() {
        let spec = parse_ok("struct S { @default(TRUE) long x; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.defaults.iter().any(|e| matches!(
                &e.mismatch,
                DefaultMismatch::TypeMismatch {
                    found: "boolean",
                    ..
                }
            )),
            "got {:?}",
            err.defaults
        );
    }

    #[test]
    fn default_integer_on_string_member_is_rejected() {
        let spec = parse_ok("struct S { @default(5) string x; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.defaults
                .iter()
                .any(|e| matches!(&e.mismatch, DefaultMismatch::TypeMismatch { .. })),
            "got {:?}",
            err.defaults
        );
    }

    #[test]
    fn legitimate_defaults_pass_the_gate() {
        // Counter-corpus: matching defaults must still validate cleanly.
        assert!(resolve_and_validate(&parse_ok("struct S { @default(7) long x; };"), &[]).is_ok());
        assert!(
            resolve_and_validate(&parse_ok("struct S { @default(-5) short x; };"), &[]).is_ok()
        );
        assert!(
            resolve_and_validate(&parse_ok("struct S { @default(255) octet x; };"), &[]).is_ok()
        );
        assert!(
            resolve_and_validate(&parse_ok("struct S { @default(TRUE) boolean x; };"), &[]).is_ok()
        );
        assert!(
            resolve_and_validate(&parse_ok("struct S { @default(\"hi\") string x; };"), &[])
                .is_ok()
        );
        assert!(
            resolve_and_validate(&parse_ok("struct S { @default(1.5) double x; };"), &[]).is_ok()
        );
        // Enum-literal default is a named value (Scoped), left unchecked.
        assert!(
            resolve_and_validate(
                &parse_ok("enum Color { RED, GREEN }; struct S { @default(GREEN) Color c; };"),
                &[]
            )
            .is_ok()
        );
    }

    // ---- Class (b): bitset @position collision reaches the gate -----------

    #[test]
    fn bitset_colliding_position_is_rejected_by_gate() {
        let spec =
            parse_ok("bitset BS { @position(0) bitfield<4> a; @position(2) bitfield<4> b; };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.bitfields
                .iter()
                .any(|e| matches!(e, BitfieldValidationError::BitfieldPositionCollision { .. })),
            "got {:?}",
            err.bitfields
        );
    }

    #[test]
    fn bitmask_duplicate_position_is_rejected_by_gate() {
        // The pre-existing bitmask duplicate-position check now runs in the gate.
        let spec = parse_ok("bitmask Flags { @position(2) F0, @position(2) F1 };");
        let err = resolve_and_validate(&spec, &[]).unwrap_err();
        assert!(
            err.bitfields
                .iter()
                .any(|e| matches!(e, BitfieldValidationError::DuplicatePosition { .. })),
            "got {:?}",
            err.bitfields
        );
    }

    #[test]
    fn well_packed_bitset_passes_the_gate() {
        // Counter-corpus: sequential bitfields without overlap must pass.
        assert!(
            resolve_and_validate(
                &parse_ok(
                    "@bit_bound(16) bitset TypeFlag {\n\
                        bitfield<1> is_final;\n\
                        bitfield<1> is_appendable;\n\
                        bitfield<1> is_mutable;\n\
                        bitfield<11>;\n\
                     };"
                ),
                &[]
            )
            .is_ok()
        );
    }
}
