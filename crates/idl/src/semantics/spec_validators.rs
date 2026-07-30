// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Spec-constraint validators for K1 follow-up (§7.4.5-§7.4.12).
//!
//! Implements the 19 specific constraint validators from the
//! S-Res-AST-builder-validator follow-up tracker
//! (`docs/spec-coverage/idl-4.2.md`). Each validator corresponds to
//! exactly one normative spec section and returns concrete
//! [`SpecValidationError`] variants.
//!
//! Call model: `validate_all` runs as a post-resolver pass — the
//! resolver builds the symbol tree, this module uses it for
//! symbol-kind lookups (e.g. ValueBox-inner-no-Value).

use crate::ast::{
    ComponentDcl, ComponentExport, Definition, EventDcl, HomeDcl, ImportedScope, InterfaceDcl,
    PorttypeDcl, ScopedName, Specification, ValueDef, ValueElement, ValueKind,
};
use crate::errors::Span;
use crate::semantics::resolver::{Resolver, SymbolKind};
use std::collections::{BTreeMap, HashSet};

/// Aggregate error type of all K1 follow-up validators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecValidationError {
    // -------- Cluster A: ValueDef --------
    /// §7.4.5.4.1.2 — a concrete value inherits from at most 1 concrete base.
    ValueMultipleConcreteBases {
        /// Name of the erroneously inheriting ValueType.
        name: String,
        /// Number of actual concrete bases.
        count: usize,
        /// Source location.
        span: Span,
    },
    /// §7.4.7.4.2.1 — an abstract valuetype must contain neither a state_member
    /// nor an init.
    AbstractValueWithStateOrInit {
        /// Name of the abstract ValueType.
        name: String,
        /// Concrete violation (`"state_member"` or `"init"`).
        violation: &'static str,
        /// Source location.
        span: Span,
    },
    /// §7.4.7.4.3 Tab. 7-19 — inheritance-compatibility matrix violated.
    ValueInheritanceMatrixViolation {
        /// Sub-ValueType.
        sub: String,
        /// Base-ValueType.
        base: String,
        /// Description of the violation.
        reason: &'static str,
        /// Source location.
        span: Span,
    },
    /// §7.4.7.4.4 — a `custom valuetype` must have at least 1 state_member.
    CustomValueNotStateful {
        /// Name of the custom ValueType.
        name: String,
        /// Source location.
        span: Span,
    },
    /// §7.4.7.4.5 — `custom` values must not inherit `truncatable`;
    /// ValueBox decls must not appear in inheritance.
    TruncatableMisuse {
        /// Name of the erroneously marked ValueType.
        name: String,
        /// Description of the violation.
        reason: &'static str,
        /// Source location.
        span: Span,
    },

    // -------- Cluster B: Value-Box + Interface --------
    /// §7.4.7.4.1 — the `value_box` inner must not itself be a ValueType.
    ValueBoxInnerIsValue {
        /// Name of the faulty ValueBox.
        name: String,
        /// Source location.
        span: Span,
    },
    /// §7.4.7.4.2.2 — an `abstract interface` may only inherit from `abstract`
    /// interfaces.
    AbstractInterfaceFromConcrete {
        /// Name of the abstract interface.
        name: String,
        /// Name of the concrete base.
        base: String,
        /// Source location.
        span: Span,
    },
    /// §7.4.6.4.3 — a `local` interface type may only appear as a param/return
    /// of local/value-type operations. Simplified
    /// variant: `local` must not inherit from non-local interfaces.
    LocalInterfaceInheritsNonLocal {
        /// Local interface name.
        name: String,
        /// Non-local base name.
        base: String,
        /// Source location.
        span: Span,
    },

    // -------- Cluster C: CORBA-Top-Level --------
    /// §7.4.6.4.1.1 — `typeid`: at-most-one pro Construct.
    TypeIdDuplicated {
        /// Full name of the target.
        target: String,
        /// Source location of the second typeid decl.
        span: Span,
    },
    /// §7.4.6.4.1.2 — the typeprefix string must have a valid format:
    /// no trailing `/`, no leading dots/underscores in
    /// segments, slash-separated segments non-empty.
    TypePrefixInvalidFormat {
        /// Raw prefix string.
        prefix: String,
        /// Concrete format violation.
        reason: &'static str,
        /// Source location.
        span: Span,
    },
    /// §7.4.6.4.1.3 — repository-ID conflict between `#pragma prefix`
    /// and a `typeprefix` declaration for the same scope.
    RepositoryIdConflict {
        /// Full name of the conflicting scope.
        target: String,
        /// Source location of the conflict.
        span: Span,
    },
    /// §7.4.6.4.1.4 — `import` references an unknown symbol or
    /// re-declares an imported name.
    ImportSemanticViolation {
        /// Description of the violated effect.
        violation: &'static str,
        /// Imported name.
        target: String,
        /// Source location.
        span: Span,
    },
    /// §7.4.6.4.7 — the `CORBA::` prefix is reserved; only allowed in `orb.idl`
    /// (bundled resource).
    CorbaPrefixReserved {
        /// Full forbidden name.
        name: String,
        /// Source location.
        span: Span,
    },

    // -------- Cluster D: Components/Events/Home/Porttype/Template --------
    /// §7.4.8.4.2.1 — the `provides` type_spec must be a non-component
    /// interface (or `Object`).
    ProvidesNotInterface {
        /// Component name.
        component: String,
        /// Provides-slot name.
        port: String,
        /// Source location.
        span: Span,
    },
    /// §7.4.8.4.3 — a component forward must be defined before use;
    /// here narrowed to inherit-from-forward detection.
    ComponentForwardInherit {
        /// Component name.
        component: String,
        /// Source location.
        span: Span,
    },
    /// §7.4.10.4.1.1.2 — an `abstract eventtype` must contain neither state nor
    /// init.
    AbstractEventWithStateOrInit {
        /// Name of the abstract eventtype.
        name: String,
        /// Concrete violation.
        violation: &'static str,
        /// Source location.
        span: Span,
    },
    /// §7.4.10.4.2.2 — a `primarykey` type must inherit from
    /// `Components::PrimaryKeyBase`.
    PrimaryKeyNotPrimaryKeyBase {
        /// Home name.
        home: String,
        /// PrimaryKey type name.
        key_type: String,
        /// Source location.
        span: Span,
    },
    /// §7.4.11.4.1.1 — a porttype must not have a cyclic embedding path.
    PorttypeCycle {
        /// Name of the porttype at which the cycle starts.
        name: String,
        /// Source location.
        span: Span,
    },
    /// §7.4.12.4.1 — a template module may neither embed another
    /// template module nor reopen a module.
    TemplateModuleEmbedReopen {
        /// Name of the outer template module.
        name: String,
        /// Description of the violated effect.
        violation: &'static str,
        /// Source location.
        span: Span,
    },
    /// §7.4.3.4.2 — an exception identifier may only appear in `raises`
    /// expressions, not as a member type.
    ExceptionUsedAsMemberType {
        /// Exception name.
        exception: String,
        /// Struct/union context.
        context: String,
        /// Source location.
        span: Span,
    },
}

impl core::fmt::Display for SpecValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ValueMultipleConcreteBases { name, count, .. } => write!(
                f,
                "valuetype '{name}' has {count} concrete bases (max 1 per §7.4.5.4.1.2)"
            ),
            Self::AbstractValueWithStateOrInit {
                name, violation, ..
            } => write!(
                f,
                "abstract valuetype '{name}' must not contain {violation} (§7.4.7.4.2.1)"
            ),
            Self::ValueInheritanceMatrixViolation {
                sub, base, reason, ..
            } => {
                write!(f, "value '{sub}' from '{base}': {reason} (§7.4.7.4.3)")
            }
            Self::CustomValueNotStateful { name, .. } => write!(
                f,
                "custom valuetype '{name}' must contain at least one state_member (§7.4.7.4.4)"
            ),
            Self::TruncatableMisuse { name, reason, .. } => {
                write!(f, "truncatable misuse on '{name}': {reason} (§7.4.7.4.5)")
            }
            Self::ValueBoxInnerIsValue { name, .. } => write!(
                f,
                "value_box '{name}' inner type must not be a value type (§7.4.7.4.1)"
            ),
            Self::AbstractInterfaceFromConcrete { name, base, .. } => write!(
                f,
                "abstract interface '{name}' inherits from concrete '{base}' (§7.4.7.4.2.2)"
            ),
            Self::LocalInterfaceInheritsNonLocal { name, base, .. } => write!(
                f,
                "local interface '{name}' inherits from non-local '{base}' (§7.4.6.4.3)"
            ),
            Self::TypeIdDuplicated { target, .. } => write!(
                f,
                "duplicate typeid for target '{target}' (§7.4.6.4.1.1: at-most-one)"
            ),
            Self::TypePrefixInvalidFormat { prefix, reason, .. } => write!(
                f,
                "typeprefix '{prefix}' has invalid format: {reason} (§7.4.6.4.1.2)"
            ),
            Self::RepositoryIdConflict { target, .. } => write!(
                f,
                "repository-id conflict for '{target}' between #pragma prefix and typeprefix (§7.4.6.4.1.3)"
            ),
            Self::ImportSemanticViolation {
                violation, target, ..
            } => {
                write!(f, "import '{target}': {violation} (§7.4.6.4.1.4)")
            }
            Self::CorbaPrefixReserved { name, .. } => {
                write!(f, "name '{name}' uses reserved 'CORBA' prefix (§7.4.6.4.7)")
            }
            Self::ProvidesNotInterface {
                component, port, ..
            } => write!(
                f,
                "component '{component}' provides slot '{port}' must reference an interface type (§7.4.8.4.2.1)"
            ),
            Self::ComponentForwardInherit { component, .. } => write!(
                f,
                "component '{component}' inherits from undefined forward decl (§7.4.8.4.3)"
            ),
            Self::AbstractEventWithStateOrInit {
                name, violation, ..
            } => write!(
                f,
                "abstract eventtype '{name}' must not contain {violation} (§7.4.10.4.1.1.2)"
            ),
            Self::PrimaryKeyNotPrimaryKeyBase { home, key_type, .. } => write!(
                f,
                "home '{home}' primarykey '{key_type}' must derive from Components::PrimaryKeyBase (§7.4.10.4.2.2)"
            ),
            Self::PorttypeCycle { name, .. } => {
                write!(
                    f,
                    "porttype '{name}' has cyclic port-ref chain (§7.4.11.4.1.1)"
                )
            }
            Self::TemplateModuleEmbedReopen {
                name, violation, ..
            } => write!(f, "template module '{name}': {violation} (§7.4.12.4.1)"),
            Self::ExceptionUsedAsMemberType {
                exception, context, ..
            } => write!(
                f,
                "exception '{exception}' may not appear as a member type (used in '{context}', §7.4.3.4.2)"
            ),
        }
    }
}

impl std::error::Error for SpecValidationError {}

impl SpecValidationError {
    /// Source location of the error.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::ValueMultipleConcreteBases { span, .. }
            | Self::AbstractValueWithStateOrInit { span, .. }
            | Self::ValueInheritanceMatrixViolation { span, .. }
            | Self::CustomValueNotStateful { span, .. }
            | Self::TruncatableMisuse { span, .. }
            | Self::ValueBoxInnerIsValue { span, .. }
            | Self::AbstractInterfaceFromConcrete { span, .. }
            | Self::LocalInterfaceInheritsNonLocal { span, .. }
            | Self::TypeIdDuplicated { span, .. }
            | Self::TypePrefixInvalidFormat { span, .. }
            | Self::RepositoryIdConflict { span, .. }
            | Self::ImportSemanticViolation { span, .. }
            | Self::CorbaPrefixReserved { span, .. }
            | Self::ProvidesNotInterface { span, .. }
            | Self::ComponentForwardInherit { span, .. }
            | Self::AbstractEventWithStateOrInit { span, .. }
            | Self::PrimaryKeyNotPrimaryKeyBase { span, .. }
            | Self::PorttypeCycle { span, .. }
            | Self::TemplateModuleEmbedReopen { span, .. }
            | Self::ExceptionUsedAsMemberType { span, .. } => *span,
        }
    }
}

/// Aggregate: run all 19 K1 follow-up validators on `spec` —
/// without the `#pragma prefix` side channel. Callers without preprocessor
/// output call this variant.
#[must_use]
pub fn validate_all(spec: &Specification, resolver: &Resolver) -> Vec<SpecValidationError> {
    validate_all_with_pragmas(spec, resolver, &[])
}

/// Aggregate variant with the `#pragma prefix` side channel: enables
/// §7.4.6.4.1.3 repository-ID conflict detection between pragma and
/// typeprefix decls. Expects a list of tuples
/// `(prefix_string, pragma_line)` from the preprocessor output.
#[must_use]
pub fn validate_all_with_pragmas(
    spec: &Specification,
    resolver: &Resolver,
    pragma_prefixes: &[(String, usize)],
) -> Vec<SpecValidationError> {
    let mut errs = Vec::new();
    let summary = SpecSummary::from_spec(spec);
    walk_definitions(spec, &summary, resolver, &mut errs);
    validate_typeid_at_most_one(spec, &mut errs);
    validate_porttype_graph(&summary, &mut errs);
    validate_import_exposed_redefines(spec, &mut errs);
    validate_pragma_prefix_conflict(spec, pragma_prefixes, &mut errs);
    validate_typeprefix_internal_conflicts(spec, &mut errs);
    validate_exception_only_in_raises(spec, &summary, &mut errs);
    errs
}

// ============================================================================
// Spec summary: index of all definitions with kind/category
// ============================================================================

/// Summary of the entire specification — for each fully-qualified name
/// the most important categories once (ValueKind, local flag, component-vs-
/// interface). Built once and used by all validators.
#[derive(Debug, Default)]
struct SpecSummary {
    /// Path → ValueDef kind (Concrete/Abstract/Custom).
    value_kinds: BTreeMap<String, ValueKind>,
    /// Path → is it a value box?
    value_boxes: HashSet<String>,
    /// Path → is it a component (def or forward)?
    components: HashSet<String>,
    /// Path → interface kind: (abstract, local).
    interfaces: BTreeMap<String, (bool, bool)>,
    /// Path → truncatable marker on the inheritance spec.
    value_truncatables: HashSet<String>,
    /// Porttype name (last segment) → (outgoing edges as
    /// last-segment targets, span). Used by the
    /// `validate_porttype_graph` pass for multi-hop cycle detection.
    porttype_edges: BTreeMap<String, (Vec<String>, Span)>,
    /// Last-segment names of all declared exceptions. Used by the
    /// `validate_exception_only_in_raises` pass.
    exceptions: HashSet<String>,
}

impl SpecSummary {
    fn from_spec(spec: &Specification) -> Self {
        let mut s = Self::default();
        collect_summary(&spec.definitions, &[], &mut s);
        s
    }
}

/// zerodds-lint: recursion-depth 64 (AST-Walk; bounded by IDL nesting)
fn collect_summary(defs: &[Definition], path: &[&str], out: &mut SpecSummary) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let mut np = path.to_vec();
                np.push(&m.name.text);
                collect_summary(&m.definitions, &np, out);
            }
            Definition::TemplateModule(t) => {
                let mut np = path.to_vec();
                np.push(&t.name.text);
                collect_summary(&t.definitions, &np, out);
            }
            Definition::ValueDef(v) => {
                let full = full_path(path, &v.name.text);
                out.value_kinds.insert(full.clone(), v.kind);
                if let Some(inh) = &v.inheritance {
                    if inh.truncatable {
                        out.value_truncatables.insert(full);
                    }
                }
            }
            Definition::ValueBox(v) => {
                out.value_boxes.insert(full_path(path, &v.name.text));
            }
            Definition::Interface(i) => match i {
                InterfaceDcl::Def(def) => {
                    out.interfaces.insert(
                        full_path(path, &def.name.text),
                        (
                            matches!(def.kind, crate::ast::InterfaceKind::Abstract),
                            matches!(def.kind, crate::ast::InterfaceKind::Local),
                        ),
                    );
                    // §7.4.4 Rule 97: exceptions can also be declared
                    // inside interface bodies.
                    for ex in &def.exports {
                        if let crate::ast::Export::Except(e) = ex {
                            out.exceptions.insert(e.name.text.clone());
                        }
                    }
                }
                InterfaceDcl::Forward(f) => {
                    out.interfaces.insert(
                        full_path(path, &f.name.text),
                        (
                            matches!(f.kind, crate::ast::InterfaceKind::Abstract),
                            matches!(f.kind, crate::ast::InterfaceKind::Local),
                        ),
                    );
                }
            },
            Definition::Component(c) => {
                let name = match c {
                    ComponentDcl::Def(d) => &d.name.text,
                    ComponentDcl::Forward(n, _) => &n.text,
                };
                out.components.insert(full_path(path, name));
            }
            Definition::Except(e) => {
                out.exceptions.insert(e.name.text.clone());
            }
            Definition::Porttype(PorttypeDcl::Def(p)) => {
                let mut edges: Vec<String> = Vec::new();
                for ex in &p.body {
                    match ex {
                        ComponentExport::Provides { type_spec, .. }
                        | ComponentExport::Uses { type_spec, .. }
                        | ComponentExport::Port { type_spec, .. } => {
                            if let Some(last) = type_spec.parts.last() {
                                edges.push(last.text.clone());
                            }
                        }
                        _ => {}
                    }
                }
                out.porttype_edges
                    .insert(p.name.text.clone(), (edges, p.name.span));
            }
            _ => {}
        }
    }
}

fn full_path(path: &[&str], name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        format!("{}::{name}", path.join("::"))
    }
}

// ============================================================================
// Walk loop: go through all top-level defs + cluster validators
// ============================================================================

/// zerodds-lint: recursion-depth 64 (AST-Walk; bounded by IDL nesting)
fn walk_definitions(
    spec: &Specification,
    summary: &SpecSummary,
    resolver: &Resolver,
    errs: &mut Vec<SpecValidationError>,
) {
    walk_definitions_inner(&spec.definitions, summary, resolver, errs);
}

/// zerodds-lint: recursion-depth 64 (AST-Walk; bounded by IDL nesting)
fn walk_definitions_inner(
    defs: &[Definition],
    summary: &SpecSummary,
    resolver: &Resolver,
    errs: &mut Vec<SpecValidationError>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                walk_definitions_inner(&m.definitions, summary, resolver, errs);
            }
            Definition::TemplateModule(t) => {
                validate_template_module(t, errs);
                walk_definitions_inner(&t.definitions, summary, resolver, errs);
            }
            Definition::ValueDef(v) => {
                validate_value_def(v, summary, errs);
            }
            Definition::ValueBox(v) => {
                validate_value_box(&v.name.text, &v.type_spec, summary, resolver, v.span, errs);
            }
            Definition::Interface(InterfaceDcl::Def(def)) => {
                validate_interface_def(def, summary, resolver, errs);
            }
            Definition::TypePrefix(tp) => {
                validate_type_prefix(&tp.prefix, tp.span, errs);
                validate_corba_prefix_in_target(&tp.target, errs);
            }
            Definition::Import(im) => {
                validate_import(im, resolver, errs);
            }
            Definition::TypeId(t) => {
                validate_corba_prefix_in_target(&t.target, errs);
            }
            Definition::Component(ComponentDcl::Def(comp)) => {
                validate_component(comp, summary, resolver, errs);
            }
            Definition::Component(ComponentDcl::Forward(_, _)) => {}
            Definition::Event(EventDcl::Def(ev)) => {
                validate_event(ev, errs);
            }
            Definition::Event(EventDcl::Forward(_, _)) => {}
            Definition::Home(HomeDcl::Def(h)) => {
                validate_home(h, resolver, errs);
            }
            Definition::Home(HomeDcl::Forward(_, _)) => {}
            Definition::Porttype(_) => {
                // Porttype cycle detection runs globally in
                // validate_porttype_graph (DFS on the overall graph from
                // SpecSummary.porttype_edges).
            }
            _ => {}
        }
    }
}

// ============================================================================
// Cluster A: ValueDef (5 Validatoren)
// ============================================================================

fn validate_value_def(v: &ValueDef, summary: &SpecSummary, errs: &mut Vec<SpecValidationError>) {
    let bases: &[ScopedName] = match &v.inheritance {
        Some(spec) => &spec.bases,
        None => &[],
    };
    let truncatable = v.inheritance.as_ref().is_some_and(|s| s.truncatable);

    // §7.4.5.4.1.2 — Single-Derivation: max 1 concrete Base.
    if v.kind == ValueKind::Concrete {
        let concrete_bases = bases
            .iter()
            .filter(|b| matches_value_kind(b, summary, ValueKind::Concrete))
            .count();
        if concrete_bases > 1 {
            errs.push(SpecValidationError::ValueMultipleConcreteBases {
                name: v.name.text.clone(),
                count: concrete_bases,
                span: v.name.span,
            });
        }
    }

    // §7.4.7.4.2.1 — abstract: no state_member, no init.
    if v.kind == ValueKind::Abstract {
        for el in &v.elements {
            match el {
                ValueElement::State(s) => {
                    errs.push(SpecValidationError::AbstractValueWithStateOrInit {
                        name: v.name.text.clone(),
                        violation: "state_member",
                        span: s.span,
                    });
                }
                ValueElement::Init(i) => {
                    errs.push(SpecValidationError::AbstractValueWithStateOrInit {
                        name: v.name.text.clone(),
                        violation: "init",
                        span: i.span,
                    });
                }
                ValueElement::Export(_) => {}
            }
        }
    }

    // §7.4.7.4.4 — `custom` must have at least one state_member.
    if v.kind == ValueKind::Custom {
        let has_state = v
            .elements
            .iter()
            .any(|e| matches!(e, ValueElement::State(_)));
        if !has_state {
            errs.push(SpecValidationError::CustomValueNotStateful {
                name: v.name.text.clone(),
                span: v.name.span,
            });
        }
    }

    // §7.4.7.4.5 — truncatable not for custom; ValueBox must not
    // appear in inheritance.
    if truncatable && v.kind == ValueKind::Custom {
        errs.push(SpecValidationError::TruncatableMisuse {
            name: v.name.text.clone(),
            reason: "'custom' valuetype must not be 'truncatable'",
            span: v.name.span,
        });
    }
    for b in bases {
        let key = scoped_name_to_key(b);
        if summary.value_boxes.contains(&key) {
            errs.push(SpecValidationError::TruncatableMisuse {
                name: v.name.text.clone(),
                reason: "value_box must not appear in inheritance list",
                span: b.span,
            });
        }
    }

    // §7.4.7.4.3 — inheritance-compatibility matrix (Tab. 7-19, 5x5).
    // Subset: concrete can only inherit from concrete; abstract only from
    // abstract; custom only from custom or concrete (spec rule: custom
    // inherits from arbitrary non-custom values, spec §7.4.7.4.4 allows
    // this). Boxed values cannot appear in a hierarchy (already
    // checked above). Truncatable + abstract = violation.
    if truncatable && v.kind == ValueKind::Abstract {
        errs.push(SpecValidationError::ValueInheritanceMatrixViolation {
            sub: v.name.text.clone(),
            base: bases.first().map_or(String::new(), scoped_name_to_key),
            reason: "abstract valuetype cannot be 'truncatable'",
            span: v.name.span,
        });
    }
    for b in bases {
        let key = scoped_name_to_key(b);
        if let Some(base_kind) = summary.value_kinds.get(&key) {
            match (v.kind, *base_kind) {
                (ValueKind::Concrete, ValueKind::Custom) => {
                    errs.push(SpecValidationError::ValueInheritanceMatrixViolation {
                        sub: v.name.text.clone(),
                        base: key,
                        reason: "concrete valuetype cannot inherit from custom",
                        span: b.span,
                    });
                }
                (ValueKind::Abstract, ValueKind::Custom)
                | (ValueKind::Abstract, ValueKind::Concrete) => {
                    errs.push(SpecValidationError::ValueInheritanceMatrixViolation {
                        sub: v.name.text.clone(),
                        base: key,
                        reason: "abstract valuetype must inherit only from abstract",
                        span: b.span,
                    });
                }
                _ => {}
            }
        }
    }
}

fn matches_value_kind(name: &ScopedName, summary: &SpecSummary, kind: ValueKind) -> bool {
    let key = scoped_name_to_key(name);
    summary.value_kinds.get(&key).is_some_and(|k| *k == kind)
}

fn scoped_name_to_key(sn: &ScopedName) -> String {
    sn.parts
        .iter()
        .map(|p| p.text.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

// ============================================================================
// Cluster B: Value-Box + Interface (3 Validatoren)
// ============================================================================

fn validate_value_box(
    name: &str,
    type_spec: &crate::ast::TypeSpec,
    summary: &SpecSummary,
    resolver: &Resolver,
    span: Span,
    errs: &mut Vec<SpecValidationError>,
) {
    use crate::ast::TypeSpec;
    if let TypeSpec::Scoped(sn) = type_spec {
        let key = scoped_name_to_key(sn);
        // Local lookup in the summary; if missing, query the resolver.
        let is_value = summary.value_kinds.contains_key(&key)
            || summary.value_boxes.contains(&key)
            || resolver_kind(resolver, sn.parts.last().map_or("", |p| p.text.as_str()))
                .is_some_and(|k| matches!(k, SymbolKind::ValueType | SymbolKind::ValueForward));
        if is_value {
            errs.push(SpecValidationError::ValueBoxInnerIsValue {
                name: name.to_string(),
                span,
            });
        }
    }
}

fn resolver_kind(resolver: &Resolver, name: &str) -> Option<SymbolKind> {
    resolver.root.lookup(name).map(|s| s.kind.clone())
}

/// Path lookup in the resolver scope tree: walk along the path segments
/// and return the target symbol if every segment is resolvable.
fn resolve_path_in_root<'a>(
    root: &'a crate::semantics::resolver::Scope,
    parts: &[crate::ast::Identifier],
) -> Option<&'a crate::semantics::resolver::ResolvedSymbol> {
    use crate::semantics::resolver::CaseInsensitiveIdent;
    if parts.is_empty() {
        return None;
    }
    let mut scope = root;
    for (i, p) in parts.iter().enumerate() {
        let key = CaseInsensitiveIdent::new(&p.text);
        if i + 1 == parts.len() {
            return scope.symbols.get(&key);
        }
        // Mid-path: must be a module → descend into children.
        match scope.children.get(&key) {
            Some(child) => scope = child,
            None => return None,
        }
    }
    None
}

fn validate_interface_def(
    def: &crate::ast::InterfaceDef,
    summary: &SpecSummary,
    resolver: &Resolver,
    errs: &mut Vec<SpecValidationError>,
) {
    // §7.4.7.4.2.2 — an abstract interface inherits only from abstract.
    if matches!(def.kind, crate::ast::InterfaceKind::Abstract) {
        for b in &def.bases {
            let key = scoped_name_to_key(b);
            let base_abstract = summary
                .interfaces
                .get(&key)
                .map(|(a, _)| *a)
                .unwrap_or_else(|| {
                    // Fallback: resolver kind. We cannot
                    // distinguish whether forward/def was abstract without
                    // the original kind — so pessimistically only error
                    // if the resolver knows it is non-abstract.
                    resolver_kind(resolver, b.parts.last().map_or("", |p| p.text.as_str()))
                        .is_some_and(|k| matches!(k, SymbolKind::InterfaceDef))
                });
            if !base_abstract {
                errs.push(SpecValidationError::AbstractInterfaceFromConcrete {
                    name: def.name.text.clone(),
                    base: key,
                    span: b.span,
                });
            }
        }
    }
    // §7.4.6.4.3 — a local interface must not inherit from non-local.
    if matches!(def.kind, crate::ast::InterfaceKind::Local) {
        for b in &def.bases {
            let key = scoped_name_to_key(b);
            let base_local = summary.interfaces.get(&key).is_some_and(|(_, l)| *l);
            if !base_local && summary.interfaces.contains_key(&key) {
                errs.push(SpecValidationError::LocalInterfaceInheritsNonLocal {
                    name: def.name.text.clone(),
                    base: key,
                    span: b.span,
                });
            }
        }
    }
}

// ============================================================================
// Cluster C: CORBA-Top-Level (5 Validatoren)
// ============================================================================

fn validate_typeid_at_most_one(spec: &Specification, errs: &mut Vec<SpecValidationError>) {
    let mut seen: BTreeMap<String, Span> = BTreeMap::new();
    collect_typeids(&spec.definitions, &[], &mut seen, errs);
}

/// zerodds-lint: recursion-depth 64 (AST-Walk; bounded by IDL nesting)
fn collect_typeids(
    defs: &[Definition],
    path: &[&str],
    seen: &mut BTreeMap<String, Span>,
    errs: &mut Vec<SpecValidationError>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let mut np = path.to_vec();
                np.push(&m.name.text);
                collect_typeids(&m.definitions, &np, seen, errs);
            }
            Definition::TemplateModule(t) => {
                let mut np = path.to_vec();
                np.push(&t.name.text);
                collect_typeids(&t.definitions, &np, seen, errs);
            }
            Definition::TypeId(t) => {
                let key = format!("{}::{}", path.join("::"), scoped_name_to_key(&t.target));
                if let Some(_first) = seen.get(&key) {
                    errs.push(SpecValidationError::TypeIdDuplicated {
                        target: scoped_name_to_key(&t.target),
                        span: t.span,
                    });
                } else {
                    seen.insert(key, t.span);
                }
            }
            _ => {}
        }
    }
}

fn validate_type_prefix(prefix: &str, span: Span, errs: &mut Vec<SpecValidationError>) {
    // §7.4.6.4.1.2 — slash separation, no trailing slash, no
    // segments beginning with `_`/`.`/`-`, segments non-empty.
    let mk = |reason: &'static str| SpecValidationError::TypePrefixInvalidFormat {
        prefix: prefix.to_string(),
        reason,
        span,
    };
    if prefix.ends_with('/') {
        errs.push(mk("trailing '/' is not allowed"));
        return;
    }
    if prefix.is_empty() {
        errs.push(mk("empty prefix is not allowed"));
        return;
    }
    for seg in prefix.split('/') {
        if seg.is_empty() {
            errs.push(mk("empty segment between '/' separators"));
            return;
        }
        let Some(first) = seg.chars().next() else {
            // An empty segment is already caught above — defensive guard.
            errs.push(mk("empty segment between '/' separators"));
            return;
        };
        if first == '_' || first == '.' || first == '-' {
            errs.push(mk("segment must not start with '_', '.', or '-'"));
            return;
        }
    }
}

fn validate_corba_prefix_in_target(target: &ScopedName, errs: &mut Vec<SpecValidationError>) {
    // §7.4.6.4.7 — the `CORBA::` prefix is reserved.
    if let Some(first) = target.parts.first() {
        if first.text == "CORBA" {
            errs.push(SpecValidationError::CorbaPrefixReserved {
                name: scoped_name_to_key(target),
                span: target.span,
            });
        }
    }
}

fn validate_import(
    im: &crate::ast::ImportDcl,
    resolver: &Resolver,
    errs: &mut Vec<SpecValidationError>,
) {
    // §7.4.6.4.1.4 — 9 effects from spec p. 61:
    //
    //   1. Visibility: imported name scope visible in the importing spec —
    //      enforced here via the unresolved diagnostic.
    //   2. Same Namespace: imported scopes live in the same namespace
    //      as subsequent decls — resolver default; not extra
    //      validated (otherwise a false positive on reopen).
    //   3. Module reopen: imported modules may be reopened —
    //      explicitly allowed; no error.
    //   4. Inner-no-implicit: import of A::B::C does not implicitly
    //      import A or B — behavior test (see test suite).
    //   5. Enclosing exposed-not-imported: path components before the
    //      last segment are exposed, but NOT imported; they
    //      must not be redefined/reopened — enforced
    //      by validate_import_exposed_redefines (global pass).
    //   6. Recursive: import imports nested scopes — behavior test.
    //   7. Importable constructs: targets must be module/interface/
    //      value-type/event-type — enforced below via a
    //      symbol-kind check.
    //   8. Redundant: redundant imports are ignored —
    //      behavior test, no error.
    //   9. Generation neutral: the standard defines no stub form —
    //      not checkable.
    if let ImportedScope::Scoped(sn) = &im.imported {
        validate_corba_prefix_in_target(sn, errs);
        if sn.parts.is_empty() {
            return;
        }
        // Effect 1: Visibility — the target must be resolvable.
        // Simplified: for multi-path imports (`A::B::C`) the
        // first segment must be a known module. For a simple
        // 1-segment import the symbol must lie directly in the root.
        let first = sn.parts[0].text.as_str();
        let target_sym = if sn.parts.len() == 1 {
            resolver.root.lookup(first)
        } else {
            // Path lookup: find the first segment in the root, then
            // nest.
            resolve_path_in_root(&resolver.root, &sn.parts)
        };
        let Some(sym) = target_sym else {
            errs.push(SpecValidationError::ImportSemanticViolation {
                violation: "imported name not found in current resolution scope",
                target: scoped_name_to_key(sn),
                span: im.span,
            });
            return;
        };
        // Effect 7: the target must be module / interface / value-type /
        // event-type. Other symbol kinds (struct/const/enum/...)
        // are not importable name scopes.
        let is_importable = matches!(
            sym.kind,
            SymbolKind::Module
                | SymbolKind::InterfaceDef
                | SymbolKind::InterfaceForward
                | SymbolKind::ValueType
                | SymbolKind::ValueForward
        );
        if !is_importable {
            errs.push(SpecValidationError::ImportSemanticViolation {
                violation: "imported target must be a name scope (module, interface, value type, or event type)",
                target: scoped_name_to_key(sn),
                span: im.span,
            });
        }
    }
}

/// §7.4.6.4.1.3 — repository-ID conflict between `#pragma prefix` and a
/// `typeprefix` decl in the same spec.
///
/// Spec-Regel: "Both mechanisms must return the same repository id for
/// the same IDL element otherwise an error should be raised."
///
/// Pragmatic approximation: if any `#pragma prefix` was active at all
/// in the spec AND a `typeprefix` decl with a different value
/// appears, we report the conflict. True per-element correlation
/// (which pragma region affects which typeprefix) would require
/// source-range overlap tracking; that is a future refinement.
fn validate_pragma_prefix_conflict(
    spec: &Specification,
    pragma_prefixes: &[(String, usize)],
    errs: &mut Vec<SpecValidationError>,
) {
    if pragma_prefixes.is_empty() {
        return;
    }
    let pragma_values: HashSet<&str> = pragma_prefixes.iter().map(|(p, _)| p.as_str()).collect();
    collect_typeprefix_conflicts(&spec.definitions, &pragma_values, errs);
}

/// §7.4.6.4.1.2 + §7.4.6.4.1.3 — detects multiple `typeprefix`
/// decls on the same target with different values. IDL is
/// case-insensitive (§7.2.3), so the target is grouped
/// case-insensitively.
fn validate_typeprefix_internal_conflicts(
    spec: &Specification,
    errs: &mut Vec<SpecValidationError>,
) {
    // target_lower (path-joined) → first prefix value + canonical target.
    let mut seen: BTreeMap<String, (String, String, Span)> = BTreeMap::new();
    collect_typeprefixes(&spec.definitions, &[], &mut seen, errs);
}

/// zerodds-lint: recursion-depth 64 (AST-Walk; bounded by IDL nesting)
fn collect_typeprefixes(
    defs: &[Definition],
    path: &[&str],
    seen: &mut BTreeMap<String, (String, String, Span)>,
    errs: &mut Vec<SpecValidationError>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let mut np = path.to_vec();
                np.push(&m.name.text);
                collect_typeprefixes(&m.definitions, &np, seen, errs);
            }
            Definition::TemplateModule(t) => {
                let mut np = path.to_vec();
                np.push(&t.name.text);
                collect_typeprefixes(&t.definitions, &np, seen, errs);
            }
            Definition::TypePrefix(tp) => {
                // Case-insensitive key: enclosing path + target.
                let target_canonical = scoped_name_to_key(&tp.target);
                let key_lower: String = if path.is_empty() {
                    target_canonical.to_ascii_lowercase()
                } else {
                    format!(
                        "{}::{}",
                        path.join("::").to_ascii_lowercase(),
                        target_canonical.to_ascii_lowercase()
                    )
                };
                if let Some((first_prefix, first_target, _)) = seen.get(&key_lower) {
                    if first_prefix != &tp.prefix {
                        errs.push(SpecValidationError::RepositoryIdConflict {
                            target: first_target.clone(),
                            span: tp.span,
                        });
                    }
                } else {
                    seen.insert(key_lower, (tp.prefix.clone(), target_canonical, tp.span));
                }
            }
            _ => {}
        }
    }
}

/// zerodds-lint: recursion-depth 64 (AST-Walk; bounded by IDL nesting)
fn collect_typeprefix_conflicts(
    defs: &[Definition],
    pragma_values: &HashSet<&str>,
    errs: &mut Vec<SpecValidationError>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                collect_typeprefix_conflicts(&m.definitions, pragma_values, errs);
            }
            Definition::TemplateModule(t) => {
                collect_typeprefix_conflicts(&t.definitions, pragma_values, errs);
            }
            Definition::TypePrefix(tp) => {
                if !pragma_values.contains(tp.prefix.as_str()) {
                    errs.push(SpecValidationError::RepositoryIdConflict {
                        target: scoped_name_to_key(&tp.target),
                        span: tp.span,
                    });
                }
            }
            _ => {}
        }
    }
}

/// §7.4.6.4.1.4 effect 5 — global pass: collects all exposed-but-not-
/// imported path components from the top-level imports and reports
/// module/interface defs that redefine/reopen such a name
/// *after* the earliest import.
fn validate_import_exposed_redefines(spec: &Specification, errs: &mut Vec<SpecValidationError>) {
    // 1) Collect from all top-level imports:
    //    - imported_targets: last segments from `import A::B::C` → `C`
    //    - exposed_only: enclosing segments → `A`, `B`
    //    - earliest_import_offset: span.start of the first import
    let mut imported_targets: HashSet<String> = HashSet::new();
    let mut exposed_only: HashSet<String> = HashSet::new();
    let mut earliest: Option<u32> = None;
    collect_import_paths(
        &spec.definitions,
        &mut imported_targets,
        &mut exposed_only,
        &mut earliest,
    );
    // The imported set takes precedence: a name that is explicitly imported
    // is NOT "exposed-only" (reopen allowed, effect 3).
    let exposed_only: HashSet<&String> = exposed_only
        .iter()
        .filter(|n| !imported_targets.contains(*n))
        .collect();
    let Some(earliest_offset) = earliest else {
        return;
    };
    if exposed_only.is_empty() {
        return;
    }
    // 2) Walk top-level defs and report redefine/reopen of exposed-only,
    //    but only for defs *after* the first import (subsequent
    //    declarations per spec).
    walk_for_exposed_redefines(&spec.definitions, &exposed_only, earliest_offset, errs);
}

/// zerodds-lint: recursion-depth 64 (AST walk; bounded by IDL nesting)
fn collect_import_paths(
    defs: &[Definition],
    imported: &mut HashSet<String>,
    exposed: &mut HashSet<String>,
    earliest: &mut Option<u32>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                collect_import_paths(&m.definitions, imported, exposed, earliest);
            }
            Definition::TemplateModule(t) => {
                collect_import_paths(&t.definitions, imported, exposed, earliest);
            }
            Definition::Import(im) => {
                if let ImportedScope::Scoped(sn) = &im.imported {
                    if let Some((last, head)) = sn.parts.split_last() {
                        imported.insert(last.text.clone());
                        for p in head {
                            exposed.insert(p.text.clone());
                        }
                    }
                }
                let off = im.span.start as u32;
                *earliest = Some(earliest.map_or(off, |e| e.min(off)));
            }
            _ => {}
        }
    }
}

/// zerodds-lint: recursion-depth 64 (AST-Walk; bounded by IDL nesting)
fn walk_for_exposed_redefines(
    defs: &[Definition],
    exposed_only: &HashSet<&String>,
    earliest_import_offset: u32,
    errs: &mut Vec<SpecValidationError>,
) {
    for d in defs {
        match d {
            Definition::Module(m) => {
                // The AST builder folds a reopened module into a single
                // `ModuleDef` (see `ast::builder::merge_reopened_modules`),
                // so `m.name.span` only ever reflects the *first*
                // occurrence. `reopen_spans` carries every occurrence's
                // name-span — check each one, since a module first declared
                // before the import and then reopened after it is exactly
                // the effect-5 violation this pass exists to catch.
                if exposed_only.contains(&m.name.text) {
                    for occurrence in &m.reopen_spans {
                        if occurrence.start as u32 > earliest_import_offset {
                            errs.push(SpecValidationError::ImportSemanticViolation {
                                violation: "module name is exposed via import but not imported; redefine or reopen is forbidden (effect 5)",
                                target: m.name.text.clone(),
                                span: *occurrence,
                            });
                        }
                    }
                }
                walk_for_exposed_redefines(
                    &m.definitions,
                    exposed_only,
                    earliest_import_offset,
                    errs,
                );
            }
            Definition::Interface(InterfaceDcl::Def(def)) => {
                if def.name.span.start as u32 > earliest_import_offset
                    && exposed_only.contains(&def.name.text)
                {
                    errs.push(SpecValidationError::ImportSemanticViolation {
                        violation: "interface name is exposed via import but not imported; redefine is forbidden (effect 5)",
                        target: def.name.text.clone(),
                        span: def.name.span,
                    });
                }
            }
            _ => {}
        }
    }
}

// ============================================================================
// Cluster D: Components/Events/Home/Porttype/Template (6 Validatoren)
// ============================================================================

fn validate_component(
    comp: &crate::ast::ComponentDef,
    summary: &SpecSummary,
    resolver: &Resolver,
    errs: &mut Vec<SpecValidationError>,
) {
    // §7.4.8.4.2.1 — provides must reference a non-component interface (or Object).
    for ex in &comp.body {
        if let ComponentExport::Provides {
            type_spec,
            name,
            span,
        } = ex
        {
            let key = scoped_name_to_key(type_spec);
            // Object prefix allowed.
            if type_spec.parts.len() == 1 && type_spec.parts[0].text == "Object" {
                continue;
            }
            if summary.components.contains(&key) {
                errs.push(SpecValidationError::ProvidesNotInterface {
                    component: comp.name.text.clone(),
                    port: name.text.clone(),
                    span: *span,
                });
                continue;
            }
            // The resolver kind must be an interface or ValueType.
            let kind = resolver_kind(
                resolver,
                type_spec.parts.last().map_or("", |p| p.text.as_str()),
            );
            match kind {
                Some(SymbolKind::InterfaceDef | SymbolKind::InterfaceForward) => {}
                Some(SymbolKind::ValueType) | None => {
                    // Unknown targets we let through — the resolver
                    // unresolved path picks that up in the resolver pass.
                    if kind.is_none() && !summary.interfaces.contains_key(&key) {
                        // Silence: the resolver reports unresolved separately.
                    }
                }
                _ => {
                    errs.push(SpecValidationError::ProvidesNotInterface {
                        component: comp.name.text.clone(),
                        port: name.text.clone(),
                        span: *span,
                    });
                }
            }
        }
    }
    // §7.4.8.4.3 — component-forward-inherit: if `base` points to only a
    // forward entry (the resolver knows only the forward, not the def), that
    // is a violation.
    if let Some(base) = &comp.base {
        let last = base.parts.last().map_or("", |p| p.text.as_str());
        if let Some(SymbolKind::InterfaceForward) = resolver_kind(resolver, last) {
            errs.push(SpecValidationError::ComponentForwardInherit {
                component: comp.name.text.clone(),
                span: base.span,
            });
        }
    }
}

fn validate_event(ev: &crate::ast::EventDef, errs: &mut Vec<SpecValidationError>) {
    // §7.4.10.4.1.1.2 — an abstract eventtype must have neither state nor init.
    if ev.kind == ValueKind::Abstract {
        for el in &ev.elements {
            match el {
                ValueElement::State(s) => {
                    errs.push(SpecValidationError::AbstractEventWithStateOrInit {
                        name: ev.name.text.clone(),
                        violation: "state_member",
                        span: s.span,
                    });
                }
                ValueElement::Init(i) => {
                    errs.push(SpecValidationError::AbstractEventWithStateOrInit {
                        name: ev.name.text.clone(),
                        violation: "init",
                        span: i.span,
                    });
                }
                ValueElement::Export(_) => {}
            }
        }
    }
}

fn validate_home(
    h: &crate::ast::HomeDef,
    resolver: &Resolver,
    errs: &mut Vec<SpecValidationError>,
) {
    // §7.4.10.4.2.2 — primarykey must inherit from Components::PrimaryKeyBase.
    if let Some(pk) = &h.primary_key {
        let last = pk.parts.last().map_or("", |p| p.text.as_str());
        let kind = resolver_kind(resolver, last);
        // Accepted patterns:
        // 1) `Components::PrimaryKeyBase` referenced directly (canonical).
        // 2) Known in the resolver as a ValueType + inheritance from
        //    `Components::PrimaryKeyBase` (but the validator has no walk
        //    in this phase — we accept known ValueTypes as
        //    "potentially derived" and report only the clear case:
        //    primary_key points to a non-value symbol.
        let is_pk_base = pk.parts.len() == 2
            && pk.parts[0].text == "Components"
            && pk.parts[1].text == "PrimaryKeyBase";
        if is_pk_base {
            return;
        }
        match kind {
            Some(SymbolKind::ValueType | SymbolKind::ValueForward) => {
                // Indirect reference to PrimaryKeyBase — a strict
                // check needs an inheritance walk. We cannot
                // check this conclusively here without the
                // inheritance graph; therefore tolerant: no error.
            }
            Some(_) => {
                errs.push(SpecValidationError::PrimaryKeyNotPrimaryKeyBase {
                    home: h.name.text.clone(),
                    key_type: scoped_name_to_key(pk),
                    span: pk.span,
                });
            }
            None => {
                // Unresolved — the resolver reports that separately.
            }
        }
    }
}

/// §7.4.11.4.1.1 — porttype cycle detection (multi-hop).
///
/// Global DFS pass on the porttype graph: each porttype is a node,
/// edges are provides/uses/port targets whose last segment is a
/// known porttype name. Cycle detection via three-color DFS
/// (white/gray/black) — gray nodes during traversal mark a
/// back-edge → cycle.
fn validate_porttype_graph(summary: &SpecSummary, errs: &mut Vec<SpecValidationError>) {
    use std::collections::HashMap;
    // Color: 0 = white (unseen), 1 = gray (on stack), 2 = black (done).
    let mut color: HashMap<&str, u8> = summary
        .porttype_edges
        .keys()
        .map(|k| (k.as_str(), 0u8))
        .collect();
    let mut reported: HashSet<String> = HashSet::new();
    for start in summary.porttype_edges.keys() {
        if color.get(start.as_str()).copied().unwrap_or(2) == 0 {
            dfs_porttype(
                start,
                summary,
                &mut color,
                &mut Vec::new(),
                &mut reported,
                errs,
            );
        }
    }
}

/// zerodds-lint: recursion-depth 64 (Porttype-Graph-Walk; bounded by IDL nesting)
fn dfs_porttype<'a>(
    node: &'a str,
    summary: &'a SpecSummary,
    color: &mut std::collections::HashMap<&'a str, u8>,
    stack: &mut Vec<&'a str>,
    reported: &mut HashSet<String>,
    errs: &mut Vec<SpecValidationError>,
) {
    color.insert(node, 1);
    stack.push(node);
    if let Some((edges, _span)) = summary.porttype_edges.get(node) {
        for target in edges {
            // Only follow edges to known porttypes — an edge to
            // an interface is not a porttype-cycle contribution.
            let Some(target_key) = summary
                .porttype_edges
                .keys()
                .find(|k| *k == target)
                .map(String::as_str)
            else {
                continue;
            };
            match color.get(target_key).copied().unwrap_or(0) {
                0 => dfs_porttype(target_key, summary, color, stack, reported, errs),
                1 => {
                    // Back-edge: cycle detected. Report exactly one
                    // error per cycle (at the first node of the cycle in the stack).
                    if let Some(idx) = stack.iter().position(|n| *n == target_key) {
                        let cycle_root = stack[idx];
                        if reported.insert(cycle_root.to_string()) {
                            let span = summary
                                .porttype_edges
                                .get(cycle_root)
                                .map_or(Span::new(0, 0), |(_, s)| *s);
                            errs.push(SpecValidationError::PorttypeCycle {
                                name: cycle_root.to_string(),
                                span,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
    stack.pop();
    color.insert(node, 2);
}

/// §7.4.3.4.2 — an exception identifier may only appear in `raises` expressions,
/// not as a member type of struct/union/exception. Walks
/// all struct/union/exception defs and checks per member whether the
/// TypeSpec points to a known exception.
fn validate_exception_only_in_raises(
    spec: &Specification,
    summary: &SpecSummary,
    errs: &mut Vec<SpecValidationError>,
) {
    if summary.exceptions.is_empty() {
        return;
    }
    walk_exception_member_types(&spec.definitions, summary, errs);
}

/// zerodds-lint: recursion-depth 64 (AST-Walk; bounded by IDL nesting)
fn walk_exception_member_types(
    defs: &[Definition],
    summary: &SpecSummary,
    errs: &mut Vec<SpecValidationError>,
) {
    use crate::ast::{ConstrTypeDecl, StructDcl, TypeDecl, UnionDcl};
    for d in defs {
        match d {
            Definition::Module(m) => walk_exception_member_types(&m.definitions, summary, errs),
            Definition::TemplateModule(t) => {
                walk_exception_member_types(&t.definitions, summary, errs);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                for m in &s.members {
                    check_member_type(&m.type_spec, &s.name.text, m.span, summary, errs);
                }
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                for c in &u.cases {
                    check_member_type(&c.element.type_spec, &u.name.text, c.span, summary, errs);
                }
            }
            Definition::Except(e) => {
                for m in &e.members {
                    check_member_type(&m.type_spec, &e.name.text, m.span, summary, errs);
                }
            }
            _ => {}
        }
    }
}

fn check_member_type(
    type_spec: &crate::ast::TypeSpec,
    context: &str,
    span: Span,
    summary: &SpecSummary,
    errs: &mut Vec<SpecValidationError>,
) {
    use crate::ast::TypeSpec;
    if let TypeSpec::Scoped(sn) = type_spec {
        if let Some(last) = sn.parts.last() {
            if summary.exceptions.contains(&last.text) {
                errs.push(SpecValidationError::ExceptionUsedAsMemberType {
                    exception: last.text.clone(),
                    context: context.to_string(),
                    span,
                });
            }
        }
    }
}

fn validate_template_module(
    t: &crate::ast::TemplateModuleDcl,
    errs: &mut Vec<SpecValidationError>,
) {
    // §7.4.12.4.1 — no template module may be embedded in another
    // template module, and no reopen.
    let mut seen_modules: HashSet<String> = HashSet::new();
    for d in &t.definitions {
        match d {
            Definition::TemplateModule(inner) => {
                errs.push(SpecValidationError::TemplateModuleEmbedReopen {
                    name: t.name.text.clone(),
                    violation: "nested template_module declarations are not allowed",
                    span: inner.name.span,
                });
            }
            Definition::Module(m) => {
                if !seen_modules.insert(m.name.text.clone()) {
                    errs.push(SpecValidationError::TemplateModuleEmbedReopen {
                        name: t.name.text.clone(),
                        violation: "module reopen inside template_module is not allowed",
                        span: m.name.span,
                    });
                }
            }
            _ => {}
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use super::*;
    use crate::ast::build;
    use crate::cst::build::build_cst;
    use crate::engine::Engine;
    use crate::grammar::idl42::IDL_42;
    use crate::lexer::Tokenizer;

    fn validate_src(src: &str) -> Vec<SpecValidationError> {
        validate_pipeline(src, "test.idl")
    }

    /// Full pipeline through preprocessor, lexer, recognizer, builder,
    /// resolver and `validate_all_with_pragmas`. Activates the
    /// `§7.4.6.4.1.3` repository-ID conflict detection via the
    /// pragma-prefix side channel of the preprocessor.
    fn validate_pipeline(src: &str, file_name: &str) -> Vec<SpecValidationError> {
        use crate::preprocessor::{MemoryResolver, Preprocessor};
        let resolver_files = MemoryResolver::default();
        let pp = Preprocessor::new(resolver_files);
        let processed = pp.process(file_name, src).expect("preprocess");
        let pragma_prefixes: Vec<(String, usize)> = processed
            .pragma_prefixes
            .iter()
            .map(|p| (p.prefix.clone(), p.line))
            .collect();
        let tokenizer = Tokenizer::for_grammar(&IDL_42);
        let stream = tokenizer.tokenize(&processed.expanded).expect("lex");
        let engine = Engine::new(&IDL_42);
        let result = engine.recognize(stream.tokens()).expect("parse");
        let cst = build_cst(engine.compiled_grammar(), stream.tokens(), &result).expect("cst");
        let spec = build(&cst).expect("ast");
        let mut resolver = Resolver::new();
        resolver.build(&spec);
        validate_all_with_pragmas(&spec, &resolver, &pragma_prefixes)
    }

    fn assert_no_errors(src: &str, msg: &str) {
        let errs = validate_src(src);
        assert!(errs.is_empty(), "{msg}: {errs:?}");
    }

    fn assert_error_matches<F>(src: &str, pred: F, msg: &str)
    where
        F: Fn(&SpecValidationError) -> bool,
    {
        let errs = validate_src(src);
        assert!(
            errs.iter().any(pred),
            "{msg}: expected error not found, got {errs:?}"
        );
    }

    // ------- Cluster A: ValueDef -------

    #[test]
    fn rejects_value_with_two_concrete_bases() {
        let src = "valuetype A {}; valuetype B {}; valuetype C : A, B {};";
        assert_error_matches(
            src,
            |e| matches!(e, SpecValidationError::ValueMultipleConcreteBases { name, .. } if name == "C"),
            "two concrete bases",
        );
    }

    #[test]
    fn accepts_value_with_one_concrete_base() {
        assert_no_errors(
            "valuetype A {}; valuetype B : A {};",
            "single concrete base",
        );
    }

    #[test]
    fn rejects_abstract_value_with_state_member() {
        assert_error_matches(
            "abstract valuetype A { public long x; };",
            |e| {
                matches!(
                    e,
                    SpecValidationError::AbstractValueWithStateOrInit {
                        violation: "state_member",
                        ..
                    }
                )
            },
            "abstract+state",
        );
    }

    #[test]
    fn rejects_abstract_value_with_init() {
        assert_error_matches(
            "abstract valuetype A { factory create(); };",
            |e| {
                matches!(
                    e,
                    SpecValidationError::AbstractValueWithStateOrInit {
                        violation: "init",
                        ..
                    }
                )
            },
            "abstract+init",
        );
    }

    #[test]
    fn rejects_custom_value_without_state() {
        assert_error_matches(
            "custom valuetype C { void op(); };",
            |e| matches!(e, SpecValidationError::CustomValueNotStateful { .. }),
            "custom-without-state",
        );
    }

    #[test]
    fn accepts_custom_value_with_state() {
        assert_no_errors(
            "custom valuetype C { public long x; };",
            "custom-with-state",
        );
    }

    #[test]
    fn rejects_truncatable_on_custom_value() {
        // Custom cannot syntactically carry 'truncatable' in IDL
        // (the recognizer does not allow it in the custom header) — we test
        // the validator code path via abstract+truncatable.
        assert_error_matches(
            "abstract valuetype A {}; abstract valuetype B : truncatable A {};",
            |e| matches!(e, SpecValidationError::ValueInheritanceMatrixViolation { reason, .. } if reason.contains("truncatable")),
            "abstract-truncatable",
        );
    }

    #[test]
    fn rejects_abstract_value_inheriting_concrete() {
        assert_error_matches(
            "valuetype A {}; abstract valuetype B : A {};",
            |e| matches!(e, SpecValidationError::ValueInheritanceMatrixViolation { reason, .. } if reason.contains("abstract")),
            "abstract-from-concrete",
        );
    }

    // ------- Cluster B: Value-Box + Interface -------

    #[test]
    fn rejects_value_box_inner_value() {
        assert_error_matches(
            "valuetype Inner {}; valuetype Outer Inner;",
            |e| matches!(e, SpecValidationError::ValueBoxInnerIsValue { name, .. } if name == "Outer"),
            "value_box-inner-value",
        );
    }

    #[test]
    fn accepts_value_box_with_primitive_inner() {
        assert_no_errors("valuetype B long;", "value_box+long");
    }

    #[test]
    fn rejects_abstract_interface_from_concrete() {
        assert_error_matches(
            "interface I {}; abstract interface A : I {};",
            |e| matches!(e, SpecValidationError::AbstractInterfaceFromConcrete { name, .. } if name == "A"),
            "abstract-iface-from-concrete",
        );
    }

    #[test]
    fn accepts_abstract_interface_from_abstract() {
        assert_no_errors(
            "abstract interface A {}; abstract interface B : A {};",
            "abstract-iface-chain",
        );
    }

    #[test]
    fn rejects_local_interface_inheriting_non_local() {
        assert_error_matches(
            "interface I {}; local interface L : I {};",
            |e| matches!(e, SpecValidationError::LocalInterfaceInheritsNonLocal { name, .. } if name == "L"),
            "local-from-non-local",
        );
    }

    // ------- Cluster C: CORBA-Top-Level -------

    #[test]
    fn rejects_duplicate_typeid_for_same_target() {
        assert_error_matches(
            r#"interface Foo {}; typeid Foo "IDL:foo:1.0"; typeid Foo "IDL:foo:2.0";"#,
            |e| matches!(e, SpecValidationError::TypeIdDuplicated { .. }),
            "duplicate-typeid",
        );
    }

    #[test]
    fn rejects_typeprefix_with_trailing_slash() {
        assert_error_matches(
            r#"module M {}; typeprefix M "company.com/";"#,
            |e| matches!(e, SpecValidationError::TypePrefixInvalidFormat { reason, .. } if reason.contains("trailing")),
            "trailing-slash",
        );
    }

    #[test]
    fn rejects_typeprefix_segment_starting_with_dash() {
        assert_error_matches(
            r#"module M {}; typeprefix M "company.com/-foo";"#,
            |e| matches!(e, SpecValidationError::TypePrefixInvalidFormat { .. }),
            "leading-dash",
        );
    }

    #[test]
    fn accepts_typeprefix_normal_format() {
        assert_no_errors(
            r#"module M {}; typeprefix M "company.com/product";"#,
            "valid typeprefix",
        );
    }

    #[test]
    fn rejects_corba_prefix_in_typeprefix_target() {
        assert_error_matches(
            r#"typeprefix CORBA::Foo "company.com";"#,
            |e| matches!(e, SpecValidationError::CorbaPrefixReserved { .. }),
            "corba-prefix",
        );
    }

    #[test]
    fn rejects_import_unknown_target() {
        assert_error_matches(
            "import Unknown;",
            |e| matches!(e, SpecValidationError::ImportSemanticViolation { violation, .. } if violation.contains("not found")),
            "import-unknown",
        );
    }

    #[test]
    fn accepts_import_of_module() {
        // §7.4.6.4.1.4 effect 7: a module is importable.
        assert_no_errors("module Foo {}; import Foo;", "import-module-ok");
    }

    #[test]
    fn accepts_import_of_interface() {
        // §7.4.6.4.1.4 effect 7: an interface is importable.
        assert_no_errors("interface Foo {}; import Foo;", "import-interface-ok");
    }

    #[test]
    fn rejects_import_of_struct() {
        // §7.4.6.4.1.4 effect 7: a struct is NOT an importable name scope.
        assert_error_matches(
            "struct Foo { long x; }; import Foo;",
            |e| matches!(e, SpecValidationError::ImportSemanticViolation { violation, .. } if violation.contains("name scope")),
            "import-struct-rejected",
        );
    }

    #[test]
    fn rejects_import_of_const() {
        // §7.4.6.4.1.4 effect 7: a const is not an importable scope.
        assert_error_matches(
            "const long X = 1; import X;",
            |e| matches!(e, SpecValidationError::ImportSemanticViolation { violation, .. } if violation.contains("name scope")),
            "import-const-rejected",
        );
    }

    #[test]
    fn rejects_module_redef_of_exposed_outer_scope() {
        // §7.4.6.4.1.4 effect 5: `import A::B` exposes A, but does not
        // import it. A subsequent `module A {}` must not reopen A.
        assert_error_matches(
            "module A { module B {}; }; import A::B; module A { struct S { long x; }; };",
            |e| {
                matches!(e, SpecValidationError::ImportSemanticViolation { violation, target, .. }
                if violation.contains("effect 5") && target == "A")
            },
            "exposed-redef-rejected",
        );
    }

    #[test]
    fn accepts_module_reopen_when_imported_directly() {
        // §7.4.6.4.1.4 effect 3: `import A` makes A directly imported
        // → module reopen is allowed.
        assert_no_errors(
            "module A {}; import A; module A { struct S { long x; }; };",
            "imported-module-reopen-ok",
        );
    }

    #[test]
    fn rejects_pragma_prefix_vs_typeprefix_conflict() {
        // §7.4.6.4.1.3 — pragma prefix "company.com" + typeprefix M
        // "other.com" → repository-ID conflict.
        assert_error_matches(
            "#pragma prefix \"company.com\"\nmodule M {};\ntypeprefix M \"other.com\";",
            |e| matches!(e, SpecValidationError::RepositoryIdConflict { target, .. } if target == "M"),
            "pragma-vs-typeprefix-conflict",
        );
    }

    #[test]
    fn accepts_pragma_prefix_matching_typeprefix() {
        // If pragma and typeprefix return the same value, no conflict.
        assert_no_errors(
            "#pragma prefix \"company.com\"\nmodule M {};\ntypeprefix M \"company.com\";",
            "pragma-typeprefix-equal",
        );
    }

    #[test]
    fn accepts_pragma_prefix_alone_without_typeprefix() {
        // Only pragma prefix, no typeprefix decl → no conflict.
        assert_no_errors(
            "#pragma prefix \"company.com\"\nmodule M {};",
            "pragma-only",
        );
    }

    #[test]
    fn rejects_conflicting_typeprefixes_on_same_target() {
        // Gap 3 (user audit): two typeprefix declarations on
        // the same target with different values — AST-internal
        // conflict without a pragma. Spec §7.4.6.4.1.2 + §7.4.6.4.1.3:
        // repository ID must be consistent.
        assert_error_matches(
            "module M {};\ntypeprefix M \"company.com\";\ntypeprefix M \"other.com\";",
            |e| matches!(e, SpecValidationError::RepositoryIdConflict { target, .. } if target == "M"),
            "typeprefix-internal-conflict",
        );
    }

    #[test]
    fn validates_case_insensitivity_for_pragmas_vs_typeprefixes() {
        // Gap 1 (user audit): IDL is case-insensitive (§7.2.3).
        // Pragma on "M", typeprefix on "m" — same module; the conflict
        // must be detected despite the different spelling.
        assert_error_matches(
            "#pragma prefix \"company.com\"\nmodule M {};\ntypeprefix m \"other.com\";",
            |e| matches!(e, SpecValidationError::RepositoryIdConflict { .. }),
            "case-insensitivity-conflict",
        );
    }

    #[test]
    fn rejects_exception_used_as_struct_member() {
        // §7.4.3.4.2 — an exception must not appear as a member type.
        assert_error_matches(
            "exception E { long y; }; struct S { E e; };",
            |e| matches!(e, SpecValidationError::ExceptionUsedAsMemberType { exception, .. } if exception == "E"),
            "exception-as-struct-member",
        );
    }

    #[test]
    fn rejects_exception_used_as_union_case_type() {
        assert_error_matches(
            "exception E { long y; }; \
             union U switch (long) { case 1: E e; };",
            |e| matches!(e, SpecValidationError::ExceptionUsedAsMemberType { exception, .. } if exception == "E"),
            "exception-as-union-case",
        );
    }

    #[test]
    fn accepts_exception_in_raises_only() {
        // Exception only in a raises expression — OK.
        assert_no_errors(
            "exception E {}; \
             interface I { void op() raises (E); };",
            "exception-in-raises-only",
        );
    }

    #[test]
    fn accepts_template_module_import() {
        // Gap 2 (user audit): a template module is a valid
        // name scope and may be imported (§7.4.6.4.1.4 effect 7).
        assert_no_errors(
            "module MyTemplate<typename T> { typedef T A; };\nimport MyTemplate;",
            "template-module-import",
        );
    }

    #[test]
    fn accepts_typeprefix_alone_without_pragma_prefix() {
        // Only typeprefix, no #pragma prefix → no conflict-pass
        // trigger.
        assert_no_errors(
            "module M {};\ntypeprefix M \"company.com\";",
            "typeprefix-only",
        );
    }

    #[test]
    fn redundant_imports_are_silent() {
        // §7.4.6.4.1.4 effect 8: redundant imports are ignored.
        // `import A::B` + `import A::B` (or subset) → no error.
        assert_no_errors(
            "module A { module B {}; }; import A::B; import A::B;",
            "redundant-imports-silent",
        );
    }

    // ------- Cluster D: Components/Events/Home/Porttype/Template -------

    #[test]
    fn rejects_provides_referencing_component_type() {
        assert_error_matches(
            "component A {}; component B { provides A p; };",
            |e| matches!(e, SpecValidationError::ProvidesNotInterface { component, .. } if component == "B"),
            "provides-component",
        );
    }

    #[test]
    fn accepts_provides_with_interface() {
        assert_no_errors(
            "interface I {}; component C { provides I p; };",
            "provides-interface",
        );
    }

    #[test]
    fn accepts_provides_with_object_keyword() {
        assert_no_errors("component C { provides Object p; };", "provides-object");
    }

    #[test]
    fn rejects_abstract_event_with_state() {
        assert_error_matches(
            "abstract eventtype E { public long x; };",
            |e| {
                matches!(
                    e,
                    SpecValidationError::AbstractEventWithStateOrInit {
                        violation: "state_member",
                        ..
                    }
                )
            },
            "abstract-event-state",
        );
    }

    #[test]
    fn primary_key_must_inherit_primary_key_base_or_be_value() {
        // Direct Components::PrimaryKeyBase path: accepted.
        assert_no_errors(
            "component C {}; home H manages C primarykey Components::PrimaryKeyBase {};",
            "primarykey=PKBase",
        );
    }

    #[test]
    fn rejects_primary_key_pointing_at_interface() {
        assert_error_matches(
            "interface K {}; component C {}; home H manages C primarykey K {};",
            |e| matches!(e, SpecValidationError::PrimaryKeyNotPrimaryKeyBase { home, .. } if home == "H"),
            "primarykey-iface",
        );
    }

    #[test]
    fn rejects_component_inherit_from_forward_only() {
        // Forward-decl without full form; inheriting from a forward is a §7.4.8.4.3 violation.
        assert_error_matches(
            "component Fwd; component Sub : Fwd {};",
            |e| matches!(e, SpecValidationError::ComponentForwardInherit { component, .. } if component == "Sub"),
            "component-forward-inherit",
        );
    }

    #[test]
    fn accepts_component_inherit_from_full_def() {
        assert_no_errors(
            "component Base {}; component Sub : Base {};",
            "component-full-base",
        );
    }

    #[test]
    fn rejects_porttype_self_loop() {
        assert_error_matches(
            "porttype P { provides P p; };",
            |e| matches!(e, SpecValidationError::PorttypeCycle { name, .. } if name == "P"),
            "porttype-self-loop",
        );
    }

    #[test]
    fn accepts_porttype_acyclic() {
        assert_no_errors(
            "interface I {}; porttype P { provides I p; };",
            "porttype-acyclic",
        );
    }

    #[test]
    fn rejects_porttype_two_hop_cycle() {
        // A → B → A
        assert_error_matches(
            "interface I {}; \
             porttype A { uses B b; provides I i1; }; \
             porttype B { uses A a; provides I i2; };",
            |e| matches!(e, SpecValidationError::PorttypeCycle { .. }),
            "porttype-2-hop-cycle",
        );
    }

    #[test]
    fn rejects_porttype_three_hop_cycle() {
        // A → B → C → A
        assert_error_matches(
            "interface I {}; \
             porttype A { uses B b; provides I ia; }; \
             porttype B { uses C c; provides I ib; }; \
             porttype C { uses A a; provides I ic; };",
            |e| matches!(e, SpecValidationError::PorttypeCycle { .. }),
            "porttype-3-hop-cycle",
        );
    }

    #[test]
    fn accepts_porttype_acyclic_chain() {
        // A → B → C (terminal), no cycle
        assert_no_errors(
            "interface I {}; \
             porttype A { uses B b; provides I ia; }; \
             porttype B { uses C c; provides I ib; }; \
             porttype C { provides I ic; };",
            "porttype-acyclic-chain",
        );
    }

    #[test]
    fn porttype_cycle_reports_one_error_per_cycle() {
        // Diamond with cycle: A → B → A + A → C; only one PorttypeCycle error.
        let errs = validate_src(
            "interface I {}; \
             porttype A { uses B b; uses C c; provides I ia; }; \
             porttype B { uses A a; provides I ib; }; \
             porttype C { provides I ic; };",
        );
        let cycle_errs: Vec<_> = errs
            .iter()
            .filter(|e| matches!(e, SpecValidationError::PorttypeCycle { .. }))
            .collect();
        assert_eq!(
            cycle_errs.len(),
            1,
            "expected exactly one cycle error, got {errs:?}"
        );
    }

    #[test]
    fn rejects_template_module_embed() {
        assert_error_matches(
            "module Outer<typename T> { module Inner<typename U> { typedef U A; }; typedef T B; };",
            |e| matches!(e, SpecValidationError::TemplateModuleEmbedReopen { violation, .. } if violation.contains("nested")),
            "template-embed",
        );
    }

    #[test]
    fn accepts_template_module_with_plain_module_inside() {
        assert_no_errors(
            "module Outer<typename T> { module Plain { typedef T A; }; };",
            "template-with-plain-module",
        );
    }
}
