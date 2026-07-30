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
    Annotation, ComponentDcl, ConstrTypeDecl, Definition, EventDcl, Export, HomeDcl, InterfaceDcl,
    ScopedName, Specification, StructDcl, SwitchTypeSpec, TypeDecl, TypeSpec, UnionDcl,
    ValueElement,
};
use crate::errors::Span;
use crate::semantics::annotations::{LowerError, lower_single};
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
}

impl SemanticErrors {
    /// `true` when no error was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.resolver.is_empty() && self.spec.is_empty() && self.annotations.is_empty()
    }

    /// Total number of recorded errors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.resolver.len() + self.spec.len() + self.annotations.len()
    }

    /// One human-readable message per error, resolver findings first.
    #[must_use]
    pub fn messages(&self) -> Vec<String> {
        self.resolver
            .iter()
            .map(ToString::to_string)
            .chain(self.spec.iter().map(ToString::to_string))
            .chain(self.annotations.iter().map(ToString::to_string))
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

    if resolver_errors.is_empty() && spec_errors.is_empty() && annotation_errors.is_empty() {
        Ok(())
    } else {
        Err(SemanticErrors {
            resolver: resolver_errors,
            spec: spec_errors,
            annotations: annotation_errors,
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
            _ => {}
        }
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::config::ParserConfig;
    use crate::parser::parse;

    fn parse_ok(src: &str) -> Specification {
        parse(src, &ParserConfig::default()).expect("parse ok")
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
}
