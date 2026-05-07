// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Spec-Constraint-Validatoren fuer K1-Followup (§7.4.5-§7.4.12).
//!
//! Implementiert die 19 spezifischen Constraint-Validatoren aus dem
//! S-Res-AST-Builder-Validator-Followup-Tracker
//! (`docs/spec-coverage/idl-4.2.md`). Jeder Validator entspricht
//! genau einer normativen Spec-Section und liefert konkrete
//! [`SpecValidationError`]-Varianten.
//!
//! Aufruf-Modell: `validate_all` laeuft als Post-Resolver-Pass — der
//! Resolver baut das Symbol-Tree, dieses Modul nutzt es fuer
//! Symbol-Kind-Lookups (z.B. ValueBox-inner-no-Value).

use crate::ast::{
    ComponentDcl, ComponentExport, Definition, EventDcl, HomeDcl, ImportedScope, InterfaceDcl,
    PorttypeDcl, ScopedName, Specification, ValueDef, ValueElement, ValueKind,
};
use crate::errors::Span;
use crate::semantics::resolver::{Resolver, SymbolKind};
use std::collections::{BTreeMap, HashSet};

/// Aggregat-Fehler-Typ aller K1-Followup-Validatoren.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecValidationError {
    // -------- Cluster A: ValueDef --------
    /// §7.4.5.4.1.2 — concrete Value erbt von max. 1 concrete Base.
    ValueMultipleConcreteBases {
        /// Name des fehlerhaft erbenden ValueType.
        name: String,
        /// Anzahl tatsaechlicher concrete Bases.
        count: usize,
        /// Quellort.
        span: Span,
    },
    /// §7.4.7.4.2.1 — abstract valuetype darf weder state_member noch
    /// init enthalten.
    AbstractValueWithStateOrInit {
        /// Name des abstract ValueType.
        name: String,
        /// Konkrete Verletzung (`"state_member"` oder `"init"`).
        violation: &'static str,
        /// Quellort.
        span: Span,
    },
    /// §7.4.7.4.3 Tab. 7-19 — Inheritance-Compat-Matrix verletzt.
    ValueInheritanceMatrixViolation {
        /// Sub-ValueType.
        sub: String,
        /// Base-ValueType.
        base: String,
        /// Beschreibung der Verletzung.
        reason: &'static str,
        /// Quellort.
        span: Span,
    },
    /// §7.4.7.4.4 — `custom valuetype` muss mind. 1 state_member haben.
    CustomValueNotStateful {
        /// Name des custom ValueType.
        name: String,
        /// Quellort.
        span: Span,
    },
    /// §7.4.7.4.5 — `custom`-Values duerfen nicht `truncatable` erben;
    /// ValueBox-Decls duerfen nicht in Inheritance auftreten.
    TruncatableMisuse {
        /// Name des fehlerhaft markierten ValueType.
        name: String,
        /// Beschreibung der Verletzung.
        reason: &'static str,
        /// Quellort.
        span: Span,
    },

    // -------- Cluster B: Value-Box + Interface --------
    /// §7.4.7.4.1 — `value_box`-inner darf nicht selbst ValueType sein.
    ValueBoxInnerIsValue {
        /// Name des fehlerhaften ValueBox.
        name: String,
        /// Quellort.
        span: Span,
    },
    /// §7.4.7.4.2.2 — `abstract interface` darf nur von `abstract`-
    /// Interfaces erben.
    AbstractInterfaceFromConcrete {
        /// Name des abstract Interfaces.
        name: String,
        /// Name der konkreten Base.
        base: String,
        /// Quellort.
        span: Span,
    },
    /// §7.4.6.4.3 — `local`-Interface-Type darf nur als Param/Return
    /// von Local-/Value-Type-Operationen auftreten. Vereinfachte
    /// Variante: `local` darf nicht von non-local Interfaces erben.
    LocalInterfaceInheritsNonLocal {
        /// Local-Interface-Name.
        name: String,
        /// Non-local Base-Name.
        base: String,
        /// Quellort.
        span: Span,
    },

    // -------- Cluster C: CORBA-Top-Level --------
    /// §7.4.6.4.1.1 — `typeid`: at-most-one pro Construct.
    TypeIdDuplicated {
        /// Voller Name des Targets.
        target: String,
        /// Quellort der zweiten typeid-Decl.
        span: Span,
    },
    /// §7.4.6.4.1.2 — Typeprefix-String muss gueltiges Format haben:
    /// kein trailing `/`, keine fuehrenden Punkte/Underscores in
    /// Segmenten, slash-getrennte Segmente non-leer.
    TypePrefixInvalidFormat {
        /// Roher Prefix-String.
        prefix: String,
        /// Konkrete Format-Verletzung.
        reason: &'static str,
        /// Quellort.
        span: Span,
    },
    /// §7.4.6.4.1.3 — Repository-ID-Konflikt zwischen `#pragma prefix`
    /// und `typeprefix`-Deklaration fuer denselben Scope.
    RepositoryIdConflict {
        /// Voller Name des Konflikt-Scopes.
        target: String,
        /// Quellort des Konflikts.
        span: Span,
    },
    /// §7.4.6.4.1.4 — `import` referenziert unbekanntes Symbol oder
    /// re-deklariert einen importierten Namen.
    ImportSemanticViolation {
        /// Beschreibung des verletzten Effekts.
        violation: &'static str,
        /// Importierter Name.
        target: String,
        /// Quellort.
        span: Span,
    },
    /// §7.4.6.4.7 — `CORBA::`-Praefix ist reserved; nur in `orb.idl`
    /// (bundled Resource) erlaubt.
    CorbaPrefixReserved {
        /// Voller verbotener Name.
        name: String,
        /// Quellort.
        span: Span,
    },

    // -------- Cluster D: Components/Events/Home/Porttype/Template --------
    /// §7.4.8.4.2.1 — `provides`-type_spec muss ein non-Component
    /// Interface (oder `Object`) sein.
    ProvidesNotInterface {
        /// Component-Name.
        component: String,
        /// Provides-Slot-Name.
        port: String,
        /// Quellort.
        span: Span,
    },
    /// §7.4.8.4.3 — Component-Forward muss vor Verwendung definiert
    /// werden; hier eingegrenzt auf Inherit-from-Forward-Detection.
    ComponentForwardInherit {
        /// Component-Name.
        component: String,
        /// Quellort.
        span: Span,
    },
    /// §7.4.10.4.1.1.2 — `abstract eventtype` darf weder state noch
    /// init enthalten.
    AbstractEventWithStateOrInit {
        /// Name des abstract Eventtype.
        name: String,
        /// Konkrete Verletzung.
        violation: &'static str,
        /// Quellort.
        span: Span,
    },
    /// §7.4.10.4.2.2 — `primarykey`-Type muss von
    /// `Components::PrimaryKeyBase` erben.
    PrimaryKeyNotPrimaryKeyBase {
        /// Home-Name.
        home: String,
        /// PrimaryKey-Type-Name.
        key_type: String,
        /// Quellort.
        span: Span,
    },
    /// §7.4.11.4.1.1 — Porttype darf keinen zyklischen Embedding-Pfad
    /// haben.
    PorttypeCycle {
        /// Name des Porttype, ueber dem der Zyklus startet.
        name: String,
        /// Quellort.
        span: Span,
    },
    /// §7.4.12.4.1 — Template-Module darf weder ein anderes
    /// Template-Module embedden noch Module-Reopen.
    TemplateModuleEmbedReopen {
        /// Name des outer Template-Module.
        name: String,
        /// Beschreibung des verletzten Effekts.
        violation: &'static str,
        /// Quellort.
        span: Span,
    },
    /// §7.4.3.4.2 — Exception-Identifier darf nur in `raises`-
    /// Expressions auftreten, nicht als Member-Type.
    ExceptionUsedAsMemberType {
        /// Exception-Name.
        exception: String,
        /// Struct/Union-Kontext.
        context: String,
        /// Quellort.
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
    /// Quellort des Fehlers.
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

/// Aggregat: alle 19 K1-Followup-Validatoren auf `spec` ausfuehren —
/// ohne `#pragma prefix`-Side-Channel. Aufrufer ohne Preprocessor-
/// Output rufen diese Variant auf.
#[must_use]
pub fn validate_all(spec: &Specification, resolver: &Resolver) -> Vec<SpecValidationError> {
    validate_all_with_pragmas(spec, resolver, &[])
}

/// Aggregat-Variant mit `#pragma prefix`-Side-Channel: ermoeglicht
/// §7.4.6.4.1.3 Repository-ID-Konflikt-Detection zwischen Pragma- und
/// Typeprefix-Decls. Erwartet eine Liste von Tuples
/// `(prefix_string, pragma_line)` aus dem Preprocessor-Output.
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
// Spec-Summary: Index aller Definitionen mit Kind/Kategorie
// ============================================================================

/// Zusammenfassung der gesamten Specification — pro fully-qualified-Name
/// einmal die wichtigsten Kategorien (ValueKind, Local-Flag, Component-vs-
/// Interface). Wird einmalig aufgebaut und von allen Validatoren genutzt.
#[derive(Debug, Default)]
struct SpecSummary {
    /// Pfad → ValueDef-Kind (Concrete/Abstract/Custom).
    value_kinds: BTreeMap<String, ValueKind>,
    /// Pfad → ist Value-Box?
    value_boxes: HashSet<String>,
    /// Pfad → ist Component (Def oder Forward)?
    components: HashSet<String>,
    /// Pfad → Interface-Kind: (abstract, local).
    interfaces: BTreeMap<String, (bool, bool)>,
    /// Pfad → Truncatable-Marker auf der Inheritance-Spec.
    value_truncatables: HashSet<String>,
    /// Porttype-Name (Last-Segment) → (outgoing Edges als
    /// Last-Segment-Targets, Span). Wird vom
    /// `validate_porttype_graph`-Pass fuer Multi-Hop-Cycle-Detection
    /// verwendet.
    porttype_edges: BTreeMap<String, (Vec<String>, Span)>,
    /// Last-Segment-Namen aller deklarierten Exceptions. Wird vom
    /// `validate_exception_only_in_raises`-Pass verwendet.
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
                    // §7.4.4 Rule 97: Exceptions koennen auch innerhalb
                    // von Interface-Bodies deklariert werden.
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
// Walk-Loop: alle Top-Level-Defs durchgehen + Cluster-Validatoren
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
                // Porttype-Cycle-Detection laeuft global in
                // validate_porttype_graph (DFS auf dem Gesamt-Graph aus
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

    // §7.4.7.4.2.1 — Abstract: kein state_member, kein init.
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

    // §7.4.7.4.4 — `custom` muss mind. ein state_member haben.
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

    // §7.4.7.4.5 — Truncatable nicht fuer custom; ValueBox darf nicht
    // in Inheritance auftreten.
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

    // §7.4.7.4.3 — Inheritance-Compat-Matrix (Tab. 7-19, 5x5).
    // Subset: concrete kann nur von concrete erben; abstract nur von
    // abstract; custom nur von custom oder concrete (Spec-Regel: custom
    // erbt von beliebigen non-custom Values, Spec §7.4.7.4.4 erlaubt
    // dies). Boxed-Values koennen nicht in Hierarchie auftreten (oben
    // bereits geprueft). Truncatable + abstract = Verstoss.
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
        // Lokales Lookup im Summary; falls fehlt, Resolver befragen.
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

/// Path-Lookup im Resolver-Scope-Tree: walke entlang der Path-Segmente
/// und liefere das Ziel-Symbol, falls jedes Segment auflösbar ist.
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
        // Mid-Path: muss ein Module sein → in children absteigen.
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
    // §7.4.7.4.2.2 — abstract interface erbt nur von abstract.
    if matches!(def.kind, crate::ast::InterfaceKind::Abstract) {
        for b in &def.bases {
            let key = scoped_name_to_key(b);
            let base_abstract = summary
                .interfaces
                .get(&key)
                .map(|(a, _)| *a)
                .unwrap_or_else(|| {
                    // Fallback: Resolver-Kind. Wir koennen nicht
                    // unterscheiden ob Forward/Def abstract war ohne
                    // den Original-Kind — also pessimistisch nur Error
                    // wenn Resolver weiss dass es non-abstract ist.
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
    // §7.4.6.4.3 — local-Interface darf nicht von non-local erben.
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
    // §7.4.6.4.1.2 — slash-Separation, kein trailing slash, keine
    // Segmente die mit `_`/`.`/`-` beginnen, Segmente non-empty.
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
            // Leeres Segment ist oben bereits abgefangen — defensive guard.
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
    // §7.4.6.4.7 — `CORBA::`-Praefix ist reserved.
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
    // §7.4.6.4.1.4 — 9 Effekte aus Spec S. 61:
    //
    //   1. Visibility: imported name-scope sichtbar im importing-spec —
    //      hier durchgesetzt durch unresolved-Diagnose.
    //   2. Same Namespace: imported scopes leben im selben Namespace
    //      wie subsequent Decls — Resolver-Default; nicht extra
    //      validiert (sonst false-positive bei Reopen).
    //   3. Module-Reopen: importierte Modules duerfen reopened werden —
    //      explizit erlaubt; kein Error.
    //   4. Inner-no-Implicit: import von A::B::C importiert nicht
    //      implizit A oder B — Behavior-Test (siehe Test-Suite).
    //   5. Enclosing exposed-not-imported: Path-Komponenten vor dem
    //      letzten Segment sind exposed, aber NICHT importiert; sie
    //      duerfen nicht redefined/reopened werden — durchgesetzt
    //      durch validate_import_exposed_redefines (globaler Pass).
    //   6. Recursive: import importiert nested scopes — Behavior-Test.
    //   7. Importable Constructs: Targets muessen Module/Interface/
    //      Value-Type/Event-Type sein — durchgesetzt unten via
    //      Symbol-Kind-Check.
    //   8. Redundant: redundante imports werden ignoriert —
    //      Behavior-Test, kein Error.
    //   9. Generation neutral: Standard definiert keine Stub-Form —
    //      nicht prueffaehig.
    if let ImportedScope::Scoped(sn) = &im.imported {
        validate_corba_prefix_in_target(sn, errs);
        if sn.parts.is_empty() {
            return;
        }
        // Effekt 1: Visibility — Target muss resolvable sein.
        // Vereinfacht: bei Multi-Path-Imports (`A::B::C`) muss das
        // erste Segment ein bekanntes Module sein. Beim einfachen
        // 1-Segment-Import muss das Symbol direkt im Root liegen.
        let first = sn.parts[0].text.as_str();
        let target_sym = if sn.parts.len() == 1 {
            resolver.root.lookup(first)
        } else {
            // Path-Lookup: erstes Segment im Root finden, dann
            // verschachteln.
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
        // Effekt 7: Target muss Module / Interface / Value-Type /
        // Event-Type sein. Andere Symbol-Kinds (Struct/Const/Enum/...)
        // sind keine importable Name-Scopes.
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

/// §7.4.6.4.1.3 — Repository-ID-Konflikt zwischen `#pragma prefix` und
/// `typeprefix`-Decl im selben Spec.
///
/// Spec-Regel: "Both mechanisms must return the same repository id for
/// the same IDL element otherwise an error should be raised."
///
/// Pragmatische Approximation: wenn ueberhaupt ein `#pragma prefix`
/// im Spec aktiv war UND eine `typeprefix`-Decl mit anderem Wert
/// auftaucht, melden wir den Konflikt. Echte per-element-Korrelation
/// (welche Pragma-Region welchen typeprefix beeinflusst) wuerde
/// Source-Range-Overlap-Tracking erfordern; das ist eine künftige Verfeinerung.
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

/// §7.4.6.4.1.2 + §7.4.6.4.1.3 — Detektiert mehrfache `typeprefix`-
/// Decls auf demselben Target mit unterschiedlichen Werten. IDL ist
/// case-insensitive (§7.2.3), daher wird das Target case-insensitiv
/// gruppiert.
fn validate_typeprefix_internal_conflicts(
    spec: &Specification,
    errs: &mut Vec<SpecValidationError>,
) {
    // target_lower (path-joined) → erster prefix-Wert + canonical target.
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
                // Case-insensitive Schluessel: enclosing-path + target.
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

/// §7.4.6.4.1.4 Effekt 5 — globaler Pass: sammelt alle exposed-but-not-
/// imported Path-Komponenten aus den Top-Level-Imports und meldet
/// Module/Interface-Defs, die *nach* dem fruehesten Import einen
/// solchen Namen redefine/reopen.
fn validate_import_exposed_redefines(spec: &Specification, errs: &mut Vec<SpecValidationError>) {
    // 1) Sammle aus allen Top-Level-Imports:
    //    - imported_targets: Last-Segments aus `import A::B::C` → `C`
    //    - exposed_only: enclosing-Segments → `A`, `B`
    //    - earliest_import_offset: span.start des ersten Imports
    let mut imported_targets: HashSet<String> = HashSet::new();
    let mut exposed_only: HashSet<String> = HashSet::new();
    let mut earliest: Option<u32> = None;
    collect_import_paths(
        &spec.definitions,
        &mut imported_targets,
        &mut exposed_only,
        &mut earliest,
    );
    // Imported-Set hat Vorrang: ein Name, der explizit importiert ist,
    // ist NICHT "exposed-only" (Reopen erlaubt, Effekt 3).
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
    // 2) Walk Top-Level-Defs und melde Redefine/Reopen von exposed-only,
    //    aber nur fuer Defs *nach* dem ersten Import (subsequent
    //    declarations gemaess Spec).
    walk_for_exposed_redefines(&spec.definitions, &exposed_only, earliest_offset, errs);
}

/// zerodds-lint: recursion-depth 64 (AST-Walk; bounded by IDL nesting)
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
                if m.name.span.start as u32 > earliest_import_offset
                    && exposed_only.contains(&m.name.text)
                {
                    errs.push(SpecValidationError::ImportSemanticViolation {
                        violation: "module name is exposed via import but not imported; redefine or reopen is forbidden (effect 5)",
                        target: m.name.text.clone(),
                        span: m.name.span,
                    });
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
    // §7.4.8.4.2.1 — provides muss non-Component-Interface (oder Object)
    // referenzieren.
    for ex in &comp.body {
        if let ComponentExport::Provides {
            type_spec,
            name,
            span,
        } = ex
        {
            let key = scoped_name_to_key(type_spec);
            // Object-Praefix erlaubt.
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
            // Resolver-Kind muss Interface oder ValueType sein.
            let kind = resolver_kind(
                resolver,
                type_spec.parts.last().map_or("", |p| p.text.as_str()),
            );
            match kind {
                Some(SymbolKind::InterfaceDef | SymbolKind::InterfaceForward) => {}
                Some(SymbolKind::ValueType) | None => {
                    // Unbekannte Targets lassen wir durch — den Resolver-
                    // unresolved-Pfad pickt das im Resolver-Pass auf.
                    if kind.is_none() && !summary.interfaces.contains_key(&key) {
                        // Schweigen: Resolver meldet unresolved separat.
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
    // §7.4.8.4.3 — Component-Forward-Inherit: Wenn `base` auf nur einen
    // Forward-Eintrag zeigt (Resolver kennt nur Forward, nicht Def), ist
    // das ein Verstoss.
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
    // §7.4.10.4.1.1.2 — abstract eventtype darf weder state noch init.
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
    // §7.4.10.4.2.2 — primarykey muss von Components::PrimaryKeyBase erben.
    if let Some(pk) = &h.primary_key {
        let last = pk.parts.last().map_or("", |p| p.text.as_str());
        let kind = resolver_kind(resolver, last);
        // Akzeptierte Patterns:
        // 1) `Components::PrimaryKeyBase` direkt referenziert (canonical).
        // 2) Im Resolver bekannt als ValueType + Inheritance auf
        //    `Components::PrimaryKeyBase` (Validator hat aber keinen Walk
        //    in dieser Phase — wir akzeptieren bekannte ValueTypes als
        //    "potentially derived" und melden nur den klaren Fall:
        //    primary_key zeigt auf Non-Value-Symbol.
        let is_pk_base = pk.parts.len() == 2
            && pk.parts[0].text == "Components"
            && pk.parts[1].text == "PrimaryKeyBase";
        if is_pk_base {
            return;
        }
        match kind {
            Some(SymbolKind::ValueType | SymbolKind::ValueForward) => {
                // Nicht-direkte Referenz auf PrimaryKeyBase — strenge
                // Pruefung benoetigt Inheritance-Walk. Wir koennen das
                // hier nicht abschliessend pruefen ohne den
                // Inheritance-Graph; deshalb tolerant: kein Error.
            }
            Some(_) => {
                errs.push(SpecValidationError::PrimaryKeyNotPrimaryKeyBase {
                    home: h.name.text.clone(),
                    key_type: scoped_name_to_key(pk),
                    span: pk.span,
                });
            }
            None => {
                // Unresolved — Resolver meldet das separat.
            }
        }
    }
}

/// §7.4.11.4.1.1 — Porttype-Cycle-Detection (Multi-Hop).
///
/// Globaler DFS-Pass auf dem Porttype-Graph: jeder Porttype ist Knoten,
/// Edges sind Provides/Uses/Port-Targets, deren Last-Segment ein
/// bekannter Porttype-Name ist. Cycle-Detection via Three-Color-DFS
/// (white/gray/black) — gray-Knoten waehrend Traversal markiert
/// Back-Edge → Cycle.
fn validate_porttype_graph(summary: &SpecSummary, errs: &mut Vec<SpecValidationError>) {
    use std::collections::HashMap;
    // Color: 0 = white (ungesehen), 1 = gray (auf Stack), 2 = black (fertig).
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
            // Nur Edges zu bekannten Porttypes verfolgen — ein Edge zu
            // einem Interface ist kein Porttype-Cycle-Beitrag.
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
                    // Back-Edge: Cycle entdeckt. Pro Zyklus genau einen
                    // Error melden (am ersten Knoten des Cycles im Stack).
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

/// §7.4.3.4.2 — Exception-Identifier darf nur in `raises`-Expressions
/// auftreten, nicht als Member-Type von Struct/Union/Exception. Walke
/// alle Struct/Union/Exception-Defs und pruefe pro Member, ob der
/// TypeSpec auf eine bekannte Exception zeigt.
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
    // §7.4.12.4.1 — kein Template-Module darf in einem anderen
    // Template-Module embeded werden, und kein Reopen.
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

    /// Voll-Pipeline durch Preprocessor, Lexer, Recognizer, Builder,
    /// Resolver und `validate_all_with_pragmas`. Aktiviert die
    /// `§7.4.6.4.1.3`-Repository-ID-Konflikt-Detection ueber den
    /// Pragma-prefix-Side-Channel des Preprocessors.
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
        // Custom kann syntaktisch in IDL nicht 'truncatable' tragen
        // (Recognizer erlaubt es nicht im custom-header) — wir testen
        // den Validator-Code-Path ueber abstract+truncatable.
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
        // §7.4.6.4.1.4 Effekt 7: Module ist importable.
        assert_no_errors("module Foo {}; import Foo;", "import-module-ok");
    }

    #[test]
    fn accepts_import_of_interface() {
        // §7.4.6.4.1.4 Effekt 7: Interface ist importable.
        assert_no_errors("interface Foo {}; import Foo;", "import-interface-ok");
    }

    #[test]
    fn rejects_import_of_struct() {
        // §7.4.6.4.1.4 Effekt 7: Struct ist KEIN importable name scope.
        assert_error_matches(
            "struct Foo { long x; }; import Foo;",
            |e| matches!(e, SpecValidationError::ImportSemanticViolation { violation, .. } if violation.contains("name scope")),
            "import-struct-rejected",
        );
    }

    #[test]
    fn rejects_import_of_const() {
        // §7.4.6.4.1.4 Effekt 7: Const ist kein importable scope.
        assert_error_matches(
            "const long X = 1; import X;",
            |e| matches!(e, SpecValidationError::ImportSemanticViolation { violation, .. } if violation.contains("name scope")),
            "import-const-rejected",
        );
    }

    #[test]
    fn rejects_module_redef_of_exposed_outer_scope() {
        // §7.4.6.4.1.4 Effekt 5: `import A::B` exposes A, but does not
        // import it. Subsequent `module A {}` darf A nicht reopen.
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
        // §7.4.6.4.1.4 Effekt 3: `import A` macht A direkt importiert
        // → Module-Reopen ist erlaubt.
        assert_no_errors(
            "module A {}; import A; module A { struct S { long x; }; };",
            "imported-module-reopen-ok",
        );
    }

    #[test]
    fn rejects_pragma_prefix_vs_typeprefix_conflict() {
        // §7.4.6.4.1.3 — pragma prefix "company.com" + typeprefix M
        // "other.com" → Repository-ID-Konflikt.
        assert_error_matches(
            "#pragma prefix \"company.com\"\nmodule M {};\ntypeprefix M \"other.com\";",
            |e| matches!(e, SpecValidationError::RepositoryIdConflict { target, .. } if target == "M"),
            "pragma-vs-typeprefix-conflict",
        );
    }

    #[test]
    fn accepts_pragma_prefix_matching_typeprefix() {
        // Wenn Pragma- und Typeprefix denselben Wert liefern, kein Konflikt.
        assert_no_errors(
            "#pragma prefix \"company.com\"\nmodule M {};\ntypeprefix M \"company.com\";",
            "pragma-typeprefix-equal",
        );
    }

    #[test]
    fn accepts_pragma_prefix_alone_without_typeprefix() {
        // Nur pragma prefix, keine typeprefix-Decl → kein Konflikt.
        assert_no_errors(
            "#pragma prefix \"company.com\"\nmodule M {};",
            "pragma-only",
        );
    }

    #[test]
    fn rejects_conflicting_typeprefixes_on_same_target() {
        // Lücke 3 (User-Audit): Zwei typeprefix-Deklarationen auf
        // demselben Target mit unterschiedlichen Werten — AST-interner
        // Konflikt ohne Pragma. Spec §7.4.6.4.1.2 + §7.4.6.4.1.3:
        // Repository-ID muss konsistent sein.
        assert_error_matches(
            "module M {};\ntypeprefix M \"company.com\";\ntypeprefix M \"other.com\";",
            |e| matches!(e, SpecValidationError::RepositoryIdConflict { target, .. } if target == "M"),
            "typeprefix-internal-conflict",
        );
    }

    #[test]
    fn validates_case_insensitivity_for_pragmas_vs_typeprefixes() {
        // Lücke 1 (User-Audit): IDL ist case-insensitive (§7.2.3).
        // Pragma auf "M", typeprefix auf "m" — selbes Modul; Konflikt
        // muss erkannt werden trotz unterschiedlicher Schreibweise.
        assert_error_matches(
            "#pragma prefix \"company.com\"\nmodule M {};\ntypeprefix m \"other.com\";",
            |e| matches!(e, SpecValidationError::RepositoryIdConflict { .. }),
            "case-insensitivity-conflict",
        );
    }

    #[test]
    fn rejects_exception_used_as_struct_member() {
        // §7.4.3.4.2 — Exception darf nicht als Member-Type auftreten.
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
        // Exception nur in raises-Expression — OK.
        assert_no_errors(
            "exception E {}; \
             interface I { void op() raises (E); };",
            "exception-in-raises-only",
        );
    }

    #[test]
    fn accepts_template_module_import() {
        // Lücke 2 (User-Audit): Template-Module ist ein valider
        // Name-Scope und darf importiert werden (§7.4.6.4.1.4 Effekt 7).
        assert_no_errors(
            "module MyTemplate<typename T> { typedef T A; };\nimport MyTemplate;",
            "template-module-import",
        );
    }

    #[test]
    fn accepts_typeprefix_alone_without_pragma_prefix() {
        // Nur typeprefix, keine #pragma prefix → kein Konflikt-Pass-
        // Trigger.
        assert_no_errors(
            "module M {};\ntypeprefix M \"company.com\";",
            "typeprefix-only",
        );
    }

    #[test]
    fn redundant_imports_are_silent() {
        // §7.4.6.4.1.4 Effekt 8: redundante Imports werden ignoriert.
        // `import A::B` + `import A::B` (oder Subset) → kein Error.
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
        // Direkter Components::PrimaryKeyBase-Pfad: akzeptiert.
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
        // Forward-Decl ohne Vollform; Inherit von Forward ist §7.4.8.4.3-Verstoss.
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
        // A → B → C (terminal), kein Cycle
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
        // Diamond mit Zyklus: A → B → A + A → C; nur ein PorttypeCycle-Error.
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
