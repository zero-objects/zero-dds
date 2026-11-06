// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The IDL rules the model cannot express structurally. Spec §6.
//!
//! The model's own invariants run at every commit and are not repeated
//! here: this is what is legal to build and illegal to write.
//!
//! Every diagnostic carries a byte range, a code and the paragraph it
//! comes from. A rule with no paragraph is ZeroDDS policy and says so.

use crate::lower::Lowering;
use alloc::string::String;
use alloc::vec::Vec;
use zerodds_ast::{AnyId, Bound, MemberNode, PrimitiveKind, TryConstruct, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};
use zerodds_idl_cst::Span;
use zerodds_idl_parse::DiagnosticCode;

/// The largest number of digits a `fixed` may have. IDL 4.2 §7.4.1.4.4.
const MAX_FIXED_DIGITS: u8 = 31;

/// How a forbidden type was reached from a key member.
///
/// Only the diagnostic's wording depends on it: "`m` is a `map`" reads
/// as a mistake in one line, and "the key of `m` reaches a `map`" tells
/// the reader to look at another declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyReach {
    /// The member's own type, after aliases.
    Direct,
    /// Through a collection element or a nested type's own key.
    Nested,
}

/// The values `@default`, `@range`, `@min` and `@max` give an element, each
/// with the words a diagnostic names it by.
type AnnotatedValues = Vec<(&'static str, Id<zerodds_ast::ConstExprNode>)>;

/// How far a `typedef` chain is followed before it is taken for a cycle.
/// The cycle itself is a resolution fault with its own report.
const MAX_ALIAS_DEPTH: usize = 64;

impl Lowering<'_> {
    /// Check every rule §6 states, over the finished model.
    ///
    /// Runs after completion, because a rule about member ids needs the
    /// derived ones to exist.
    pub(crate) fn validate(&mut self) {
        let types: Vec<Id<TypeNode>> = self.ast.types().map(|(id, _)| id).collect();
        for id in types {
            self.validate_type(id);
        }
        self.annotation_member_types();
        self.forward_declarations_are_defined();
        self.constant_values_fit_their_type();
        self.oneway_operations_return_nothing();
        self.ranges_are_ordered();
        self.interface_inheritance_rules();
        self.value_inheritance_rules();
        self.native_parameters_stay_local();
        self.context_expressions_are_well_formed();
        self.annotated_values_fit_their_element();
        self.component_model_rules();
    }

    /// What the declarations of the component model may name.
    ///
    /// OMG IDL 4.2:
    /// - §7.4.8.4.1, §7.4.9.4.1, §7.4.11.4.3: a component, a home and a
    ///   connector inherit from "a previously defined" component, home and
    ///   connector respectively;
    /// - §7.4.9.4.1: "The manages keyword followed by a scoped name that
    ///   denotes the previously defined component type";
    /// - §7.4.8.4.2.1, §7.4.8.4.2.2: the interface of a facet or a
    ///   receptacle "must denote a previously-defined non-component
    ///   interface type" — or `Object`, rule (157);
    /// - §7.4.10.4.4.1: `supports` names interfaces;
    /// - §7.4.10.4.5: `emits`, `publishes` and `consumes` name event types;
    /// - §7.4.11.4.2: `port` and `mirrorport` name "a previously declared
    ///   port type";
    /// - §7.4.11.4.1.1 and §7.4.11.4.3: a port type holds "At least one
    ///   facet … or receptacle … or port declaration", a connector "At
    ///   least one port", and "no cycles are allowed among port type
    ///   definitions".
    fn component_model_rules(&mut self) {
        use zerodds_ast::{ComponentRole, InterfaceKind};
        let containers: Vec<_> = self
            .ast
            .interfaces()
            .filter(|(_, node)| {
                matches!(
                    node.kind,
                    InterfaceKind::Component
                        | InterfaceKind::Home
                        | InterfaceKind::PortType
                        | InterfaceKind::Connector
                )
            })
            .map(|(id, node)| {
                (
                    id,
                    node.node_id,
                    node.name.clone(),
                    node.kind,
                    node.bases.clone(),
                )
            })
            .collect();
        for (id, node_id, name, kind, bases) in containers {
            let span = self.span_of(AnyId::Interface(id, node_id));
            let kind_of = |this: &Self, target: Id<zerodds_ast::InterfaceNode>| {
                this.ast
                    .interface(target)
                    .ok()
                    .map(|node| (node.kind, node.name.clone()))
            };
            // Bases of the same kind.
            for base in bases.iter().filter_map(Ref::resolved) {
                let Some((base_kind, base_name)) = kind_of(self, base) else {
                    continue;
                };
                if base_kind != kind {
                    let message = alloc::format!(
                        "the {} `{name}` derives from `{base_name}`, a {}; a {} derives from a {}",
                        kind.as_str(),
                        base_kind.as_str(),
                        kind.as_str(),
                        kind.as_str()
                    );
                    self.diagnose_at(
                        DiagnosticCode::InvalidInheritance,
                        span,
                        &message,
                        "OMG IDL 4.2 §7.4.8.4.1, §7.4.9.4.1, §7.4.11.4.3",
                    );
                }
            }
            let facets = self
                .ast
                .interface(id)
                .ok()
                .map(|node| node.facets.clone())
                .unwrap_or_default();
            // `supports` names interfaces.
            if let Some(corba) = facets.corba.as_ref() {
                for supported in corba.supports.iter().filter_map(Ref::resolved) {
                    let Some((supported_kind, supported_name)) = kind_of(self, supported) else {
                        continue;
                    };
                    if !matches!(
                        supported_kind,
                        InterfaceKind::Interface | InterfaceKind::Abstract | InterfaceKind::Local
                    ) {
                        let message = alloc::format!(
                            "`{name}` supports `{supported_name}`, a {}; `supports` names \
                             interfaces",
                            supported_kind.as_str()
                        );
                        self.diagnose_at(
                            DiagnosticCode::InvalidInheritance,
                            span,
                            &message,
                            "OMG IDL 4.2 §7.4.10.4.4.1",
                        );
                    }
                }
                // `manages` names a component.
                if let Some(manages) = corba.manages.as_ref()
                    && let Some(component) = manages.component.resolved()
                    && let Some((managed_kind, managed_name)) = kind_of(self, component)
                    && managed_kind != InterfaceKind::Component
                {
                    let message = alloc::format!(
                        "the home `{name}` manages `{managed_name}`, a {}; a home manages a \
                         component",
                        managed_kind.as_str()
                    );
                    self.diagnose_at(
                        DiagnosticCode::TypeMismatch,
                        span,
                        &message,
                        "OMG IDL 4.2 §7.4.9.4.1",
                    );
                }
            }
            // The ports.
            let mut ports = 0;
            for attribute in self.ast.attributes_of(id).to_vec() {
                let Ok(node) = self.ast.attribute(attribute) else {
                    continue;
                };
                let at = self.span_of(AnyId::Attribute(attribute, node.node_id));
                let port = node.name.clone();
                let role = node
                    .facets
                    .corba
                    .as_ref()
                    .and_then(|corba| corba.component_role.clone());
                let target = self.port_target(&node.type_);
                match role {
                    Some(
                        ComponentRole::Facet { interface, .. }
                        | ComponentRole::Receptacle { interface, .. },
                    ) => {
                        ports += 1;
                        // `Object`, rule (157), has no interface node.
                        let named = interface.resolved().or(target);
                        let Some((found, found_name)) = named.and_then(|i| kind_of(self, i)) else {
                            continue;
                        };
                        if !matches!(
                            found,
                            InterfaceKind::Interface
                                | InterfaceKind::Local
                                | InterfaceKind::Abstract
                        ) {
                            let message = alloc::format!(
                                "the port `{port}` of `{name}` names `{found_name}`, a {}; a facet \
                                 or a receptacle names a non-component interface or `Object`",
                                found.as_str()
                            );
                            self.diagnose_at(
                                DiagnosticCode::TypeMismatch,
                                at,
                                &message,
                                "OMG IDL 4.2 §7.4.8.4.2.1, §7.4.8.4.2.2",
                            );
                        }
                    }
                    Some(ComponentRole::PortTypeRole | ComponentRole::ConnectorRole) => {
                        ports += 1;
                        let found = target.and_then(|i| kind_of(self, i));
                        let wrong = match found {
                            Some((InterfaceKind::PortType, _)) => None,
                            Some((other, other_name)) => {
                                Some(alloc::format!("`{other_name}`, a {}", other.as_str()))
                            }
                            None => node
                                .type_
                                .resolved()
                                .and_then(|t| self.ast.type_(t).ok())
                                .map(|t| alloc::format!("a {}", t.kind.as_str())),
                        };
                        if let Some(found) = wrong {
                            let message = alloc::format!(
                                "the port `{port}` of `{name}` names {found}; `port` and \
                                 `mirrorport` name a port type"
                            );
                            self.diagnose_at(
                                DiagnosticCode::TypeMismatch,
                                at,
                                &message,
                                "OMG IDL 4.2 §7.4.11.4.2",
                            );
                        }
                    }
                    Some(
                        ComponentRole::EventSource { event, .. }
                        | ComponentRole::EventSink { event, .. },
                    ) => {
                        let event_type =
                            event
                                .resolved()
                                .and_then(|t| self.ast.type_(t).ok())
                                .map(|n| {
                                    (
                                        n.name.clone(),
                                        n.facets.corba.as_ref().is_some_and(|c| c.is_event_type),
                                    )
                                });
                        if let Some((event_name, false)) = event_type {
                            let message = alloc::format!(
                                "the port `{port}` of `{name}` names `{event_name}`, which is no \
                                 event type; `emits`, `publishes` and `consumes` name event types"
                            );
                            self.diagnose_at(
                                DiagnosticCode::TypeMismatch,
                                at,
                                &message,
                                "OMG IDL 4.2 §7.4.10.4.5",
                            );
                        }
                    }
                    _ => {}
                }
            }
            // Rule (175): a port type's body starts with a `<port_ref>`;
            // rule (180): a connector's body holds at least one
            // `<connector_export>`, which rule (183) makes a port or an
            // attribute.
            let empty = match kind {
                InterfaceKind::PortType => ports == 0,
                InterfaceKind::Connector => self.ast.attributes_of(id).is_empty(),
                _ => false,
            };
            if empty {
                let what = if kind == InterfaceKind::PortType {
                    "declares no port; a port type holds at least one facet, receptacle or port"
                } else {
                    "declares nothing; a connector holds at least one port or attribute"
                };
                let message = alloc::format!("`{name}` {what}");
                self.diagnose_at(
                    DiagnosticCode::UnexpectedToken,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.4.11.4.1.1, §7.4.11.4.3",
                );
            }
        }
        self.port_types_have_no_cycle();
    }

    /// The interface a port's type names: the object reference the name
    /// resolves to, or — before the names are resolved, which is when
    /// lowering validates — the declaration its pending scoped name finds.
    fn port_target(&self, type_: &Ref<TypeNode>) -> Option<Id<zerodds_ast::InterfaceNode>> {
        match type_ {
            Ref::Resolved(id) => match &self.ast.type_(*id).ok()?.kind {
                TypeKind::ObjectRef { interface: Some(r) } => r.resolved(),
                _ => None,
            },
            Ref::Pending(zerodds_ast_core::PendingRef::ScopedName(name)) => {
                match self.ast.find_by_scoped_name(name).ok()? {
                    AnyId::Interface(id, _) => Some(id),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// "no cycles are allowed among port type definitions" (§7.4.11.4.1.1):
    /// a port type that, through the extended ports it embeds, reaches
    /// itself.
    fn port_types_have_no_cycle(&mut self) {
        use zerodds_ast::InterfaceKind;
        let port_types: Vec<_> = self
            .ast
            .interfaces()
            .filter(|(_, node)| node.kind == InterfaceKind::PortType)
            .map(|(id, node)| (id, node.node_id, node.name.clone()))
            .collect();
        let embedded = |this: &Self,
                        id: Id<zerodds_ast::InterfaceNode>|
         -> Vec<Id<zerodds_ast::InterfaceNode>> {
            this.ast
                .attributes_of(id)
                .iter()
                .filter_map(|attribute| this.ast.attribute(*attribute).ok())
                .filter_map(|node| this.port_target(&node.type_))
                .filter(|i| {
                    this.ast
                        .interface(*i)
                        .is_ok_and(|node| node.kind == InterfaceKind::PortType)
                })
                .collect()
        };
        for (id, node_id, name) in port_types {
            let mut queue = embedded(self, id);
            let mut seen: Vec<Id<zerodds_ast::InterfaceNode>> = Vec::new();
            let mut cyclic = false;
            while let Some(next) = (!queue.is_empty()).then(|| queue.remove(0)) {
                if next == id {
                    cyclic = true;
                    break;
                }
                if seen.contains(&next) || seen.len() > MAX_ALIAS_DEPTH {
                    continue;
                }
                seen.push(next);
                queue.extend(embedded(self, next));
            }
            if cyclic {
                let span = self.span_of(AnyId::Interface(id, node_id));
                let message = alloc::format!(
                    "the port type `{name}` embeds itself through its extended ports; no cycles \
                     are allowed among port type definitions"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidInheritance,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.4.11.4.1.1",
                );
            }
        }
    }

    /// `@default`, `@range`, `@min` and `@max` give values of the type of
    /// the element they annotate.
    ///
    /// OMG IDL 4.2 §8.3.3.1: "The provided value must be compliant with the
    /// type of the annotated element", and the same sentence for `@range`
    /// (§8.3.3.2), `@min` (§8.3.3.3) and `@max` (§8.3.3.4). A member is
    /// held to its own type, a `typedef` to the type it names. Compliant
    /// means the form (`value_disagrees_with`, the reading
    /// §7.4.1.4.3 gives a constant) and, for an integer type, the range of
    /// Table 7-26.
    fn annotated_values_fit_their_element(&mut self) {
        let mut annotated: Vec<(AnyId, Ref<TypeNode>, AnnotatedValues)> = Vec::new();
        let values_of = |facets: &zerodds_ast::Facets| {
            let mut values = Vec::new();
            if let Some(x) = facets.xtypes.as_ref() {
                if let Some(value) = x.default_value {
                    values.push(("@default", value));
                }
                if let Some(range) = x.range {
                    if let Some(min) = range.min {
                        values.push(("the min of @range", min));
                    }
                    if let Some(max) = range.max {
                        values.push(("the max of @range", max));
                    }
                }
            }
            values
        };
        for (id, node) in self.ast.members() {
            let values = values_of(&node.facets);
            if !values.is_empty() {
                annotated.push((AnyId::Member(id, node.node_id), node.type_.clone(), values));
            }
        }
        for (id, node) in self.ast.types() {
            let TypeKind::Alias { aliased } = &node.kind else {
                continue;
            };
            let values = values_of(&node.facets);
            if !values.is_empty() {
                annotated.push((AnyId::Type(id, node.node_id), aliased.clone(), values));
            }
        }
        for (any, declared, values) in annotated {
            for (what, value) in values {
                let complaint = self.value_disagrees_with(&declared, value).or_else(|| {
                    let target = declared.resolved().and_then(|t| self.through_aliases(t))?;
                    let TypeKind::Primitive(kind) = self.ast.type_(target).ok()?.kind.clone() else {
                        return None;
                    };
                    let (low, high) = primitive_range(kind)?;
                    let zerodds_ast::ConstValue::Integer(number) = self.ast.eval_const(value).ok()?
                    else {
                        return None;
                    };
                    (!(low..=high).contains(&number)).then(|| {
                        alloc::format!(
                            "is {number}, outside the {low} to {high} the element's type `{}` holds",
                            kind.as_str()
                        )
                    })
                });
                let Some(complaint) = complaint else {
                    continue;
                };
                let span = self.span_of(any);
                let message = alloc::format!(
                    "{what} {complaint}; the value must be compliant with the type of the \
                     annotated element"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    span,
                    &message,
                    "OMG IDL 4.2 §8.3.3.1, §8.3.3.2",
                );
            }
        }
    }

    /// A native type is no parameter of an operation of a remote interface.
    ///
    /// OMG IDL 4.2 §7.4.6.4.4: "native type parameters are not permitted in
    /// operations of remote interfaces. Any attempt to transmit a value of a
    /// native type in a remote invocation may raise the MARSHAL standard
    /// system exception." A local interface (§7.4.6.4.3) is not remote; an
    /// interface, an abstract interface, a component and a home are.
    fn native_parameters_stay_local(&mut self) {
        if self.options.native_in_remote_interfaces == crate::NativeInRemoteInterface::Admitted {
            return;
        }
        let operations: Vec<_> = self
            .ast
            .operations()
            .map(|(id, node)| {
                (
                    id,
                    node.node_id,
                    node.name.clone(),
                    node.params.clone(),
                    node.parent,
                )
            })
            .collect();
        for (id, node_id, name, params, parent) in operations {
            let AnyId::Interface(interface, _) = parent else {
                continue;
            };
            let Ok(owner) = self.ast.interface(interface) else {
                continue;
            };
            if owner.kind == zerodds_ast::InterfaceKind::Local {
                continue;
            }
            let owner_name = owner.name.clone();
            for param in params {
                let Ok(param) = self.ast.param(param) else {
                    continue;
                };
                let native = param
                    .type_
                    .resolved()
                    .and_then(|target| self.through_aliases(target))
                    .and_then(|target| self.ast.type_(target).ok())
                    .is_some_and(|node| matches!(node.kind, TypeKind::Native));
                if !native {
                    continue;
                }
                let span = self.span_of(AnyId::Operation(id, node_id));
                let message = alloc::format!(
                    "the parameter `{}` of `{owner_name}::{name}` is of a native type, and \
                     `{owner_name}` is a remote interface; a native value is not transmitted, so \
                     a native parameter belongs to a local interface",
                    param.name
                );
                self.diagnose_at(
                    DiagnosticCode::TypeMismatch,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.4.6.4.4",
                );
            }
        }
    }

    /// The string literals of a `context` clause.
    ///
    /// OMG IDL 4.2 §7.4.6.4.6: "Each <string_literal> is a non-empty
    /// string. If the character * appears in a <string_literal>, it must
    /// appear only once, as the last character of the <string_literal>,
    /// and must be preceded by one or more characters other than *."
    fn context_expressions_are_well_formed(&mut self) {
        let operations: Vec<_> = self
            .ast
            .operations()
            .filter(|(_, node)| !node.contexts.is_empty())
            .map(|(id, node)| (id, node.node_id, node.name.clone(), node.contexts.clone()))
            .collect();
        for (id, node_id, name, contexts) in operations {
            for context in contexts {
                let stars = context.matches('*').count();
                let well_formed = !context.is_empty()
                    && (stars == 0 || (stars == 1 && context.len() > 1 && context.ends_with('*')));
                if well_formed {
                    continue;
                }
                let span = self.span_of(AnyId::Operation(id, node_id));
                let message = alloc::format!(
                    "the context \"{context}\" of `{name}` is not one: a context string is not \
                     empty, and a `*` in it stands once, last, after one or more other characters"
                );
                self.diagnose_at(
                    DiagnosticCode::UnexpectedToken,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.4.6.4.6",
                );
            }
        }
    }

    /// The inheritance rules of a value type, OMG IDL 4.2 §7.4.7.4.3 and
    /// Table 7-19, and of truncation and custom marshaling, §7.4.7.4.5.
    ///
    /// - "all derived value types (which must of course implement the
    ///   state) may only derive from a single (concrete) value type";
    /// - "The single immediate base concrete value type, if present, must
    ///   be the first element specified in the inheritance list";
    /// - an abstract value type inherits abstract value types only, and a
    ///   boxed value type is never a base (Table 7-19);
    /// - a value type supports any number of abstract interfaces and at
    ///   most one other ("supports single"), and that one "must be
    ///   derived, directly or indirectly, from each interface that the
    ///   value type supports through inheritance";
    /// - truncation needs a stateful value deriving from a stateful value,
    ///   and "Non-custom value types may not (transitively) inherit from
    ///   custom value types."
    fn value_inheritance_rules(&mut self) {
        use zerodds_ast::ValuetypeKind;
        let values: Vec<_> = self
            .ast
            .types()
            .filter_map(|(id, node)| match &node.kind {
                TypeKind::Valuetype {
                    kind,
                    bases,
                    supported,
                    ..
                } => Some((
                    id,
                    node.node_id,
                    node.name.clone(),
                    *kind,
                    bases.clone(),
                    supported.clone(),
                )),
                _ => None,
            })
            .collect();
        for (id, node_id, name, kind, bases, supported) in values {
            let span = self.span_of(AnyId::Type(id, node_id));
            let mut concrete_seen = false;
            for (index, base) in bases.iter().enumerate() {
                let Some(base) = base.resolved().and_then(|b| self.through_aliases(b)) else {
                    continue;
                };
                let Ok(base_node) = self.ast.type_(base) else {
                    continue;
                };
                let base_name = base_node.name.clone();
                let base_kind = match &base_node.kind {
                    TypeKind::Valuetype { kind, .. } => *kind,
                    other => {
                        let what = other.as_str();
                        let message = alloc::format!(
                            "`{name}` derives from `{base_name}`, a {what}; a value type derives \
                             from value types only, and a boxed value type is never a base"
                        );
                        self.diagnose_at(
                            DiagnosticCode::InvalidInheritance,
                            span,
                            &message,
                            "OMG IDL 4.2 §7.4.7.4.3, Table 7-19",
                        );
                        continue;
                    }
                };
                if base_kind == ValuetypeKind::Abstract {
                    continue;
                }
                let complaint = if kind == ValuetypeKind::Abstract {
                    Some("an abstract value type inherits abstract value types only")
                } else if concrete_seen {
                    Some("a value type derives from a single concrete value type")
                } else if index != 0 {
                    Some(
                        "the concrete base of a value type is the first element of its \
                         inheritance list",
                    )
                } else {
                    None
                };
                concrete_seen = true;
                if let Some(complaint) = complaint {
                    let message = alloc::format!(
                        "`{name}` names the concrete value type `{base_name}` as a base; {complaint}"
                    );
                    self.diagnose_at(
                        DiagnosticCode::InvalidInheritance,
                        span,
                        &message,
                        "OMG IDL 4.2 §7.4.7.4.3, Table 7-19",
                    );
                }
            }
            if kind == ValuetypeKind::Truncatable && !concrete_seen {
                let message = alloc::format!(
                    "`{name}` is truncatable and has no stateful base to be truncated to; \
                     truncatable is for a stateful value deriving from another stateful value"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidInheritance,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.4.7.4.5",
                );
            }
            if kind != ValuetypeKind::Custom
                && let Some(custom) = self.custom_ancestor_of(id)
            {
                let message = alloc::format!(
                    "`{name}` is not custom and inherits from the custom value type `{custom}`; \
                     a non-custom value type may not inherit from a custom one"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidInheritance,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.4.7.4.5",
                );
            }

            // Supported interfaces: any number of abstract ones, one other.
            let concrete: Vec<Id<zerodds_ast::InterfaceNode>> = supported
                .iter()
                .filter_map(Ref::resolved)
                .filter(|interface| {
                    self.ast
                        .interface(*interface)
                        .is_ok_and(|node| node.kind != zerodds_ast::InterfaceKind::Abstract)
                })
                .collect();
            if concrete.len() > 1 {
                let message = alloc::format!(
                    "`{name}` supports {} interfaces that are not abstract; a value type \
                     supports a single one",
                    concrete.len()
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidInheritance,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.4.7.4.3, Table 7-19",
                );
            }
            if let Some(own) = concrete.first() {
                let own_bases = self.all_bases_of(*own);
                for inherited in self.supported_through_bases(id) {
                    if inherited == *own || own_bases.contains(&inherited) {
                        continue;
                    }
                    let (own_name, inherited_name) = (
                        self.ast
                            .interface(*own)
                            .map_or_else(|_| String::new(), |n| n.name.clone()),
                        self.ast
                            .interface(inherited)
                            .map_or_else(|_| String::new(), |n| n.name.clone()),
                    );
                    let message = alloc::format!(
                        "`{name}` supports `{own_name}`, which does not derive from \
                         `{inherited_name}`, an interface it supports through its bases"
                    );
                    self.diagnose_at(
                        DiagnosticCode::InvalidInheritance,
                        span,
                        &message,
                        "OMG IDL 4.2 §7.4.7.4.3",
                    );
                }
            }
        }
    }

    /// The first custom value type among the bases of a value type,
    /// followed transitively.
    fn custom_ancestor_of(&self, value: Id<TypeNode>) -> Option<String> {
        let mut queue = self.value_bases(value);
        let mut seen: Vec<Id<TypeNode>> = Vec::new();
        while let Some(next) = (!queue.is_empty()).then(|| queue.remove(0)) {
            if next == value || seen.contains(&next) || seen.len() > MAX_ALIAS_DEPTH {
                continue;
            }
            seen.push(next);
            if let Ok(node) = self.ast.type_(next)
                && let TypeKind::Valuetype { kind, .. } = &node.kind
            {
                if *kind == zerodds_ast::ValuetypeKind::Custom {
                    return Some(node.name.clone());
                }
                queue.extend(self.value_bases(next));
            }
        }
        None
    }

    /// The value types a value type names as its bases, aliases followed.
    fn value_bases(&self, value: Id<TypeNode>) -> Vec<Id<TypeNode>> {
        match self.ast.type_(value).map(|node| &node.kind) {
            Ok(TypeKind::Valuetype { bases, .. }) => bases
                .iter()
                .filter_map(Ref::resolved)
                .filter_map(|base| self.through_aliases(base))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The interfaces, not abstract, that the bases of a value type
    /// support, followed transitively.
    fn supported_through_bases(&self, value: Id<TypeNode>) -> Vec<Id<zerodds_ast::InterfaceNode>> {
        let mut out = Vec::new();
        let mut queue = self.value_bases(value);
        let mut seen: Vec<Id<TypeNode>> = Vec::new();
        while let Some(next) = (!queue.is_empty()).then(|| queue.remove(0)) {
            if next == value || seen.contains(&next) || seen.len() > MAX_ALIAS_DEPTH {
                continue;
            }
            seen.push(next);
            if let Ok(TypeKind::Valuetype { supported, .. }) = self.ast.type_(next).map(|n| &n.kind)
            {
                for interface in supported.iter().filter_map(Ref::resolved) {
                    let abstract_ = self
                        .ast
                        .interface(interface)
                        .is_ok_and(|node| node.kind == zerodds_ast::InterfaceKind::Abstract);
                    if !abstract_ && !out.contains(&interface) {
                        out.push(interface);
                    }
                }
            }
            queue.extend(self.value_bases(next));
        }
        out
    }

    /// The inheritance rules of an interface.
    ///
    /// OMG IDL 4.2 §7.4.3.4.3.2.1: "An interface may not be specified as a
    /// direct base interface of a derived interface more than once; it may
    /// be an indirect base interface more than once" — the diamond of
    /// Figure 7-1 stays legal — and "It is forbidden to redefine an
    /// operation or an attribute in a derived interface, as well as
    /// inheriting two different operations or attributes with the same
    /// name." §7.4.4.4 repeats the last two for interfaces with embedded
    /// declarations. An operation or attribute reached along two paths of
    /// a diamond is one declaration and not a conflict.
    fn interface_inheritance_rules(&mut self) {
        let interfaces: Vec<_> = self
            .ast
            .interfaces()
            .map(|(id, node)| (id, node.node_id, node.name.clone(), node.bases.clone()))
            .collect();
        for (id, node_id, name, bases) in interfaces {
            let span = self.span_of(AnyId::Interface(id, node_id));
            let direct: Vec<_> = bases.iter().filter_map(Ref::resolved).collect();
            for (index, base) in direct.iter().enumerate() {
                if !direct[..index].contains(base) {
                    continue;
                }
                let base_name = self
                    .ast
                    .interface(*base)
                    .map_or_else(|_| String::from("?"), |node| node.name.clone());
                let message = alloc::format!(
                    "`{name}` names `{base_name}` as a direct base twice; an interface may be a \
                     direct base only once"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidInheritance,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.4.3.4.3.2.1",
                );
            }

            // Table 7-19: "An abstract interface may only inherit from
            // abstract interfaces" (§7.4.7.4.2 says it in words).
            let is_abstract = self
                .ast
                .interface(id)
                .is_ok_and(|node| node.kind == zerodds_ast::InterfaceKind::Abstract);
            if is_abstract {
                for base in &direct {
                    let Ok(base_node) = self.ast.interface(*base) else {
                        continue;
                    };
                    if base_node.kind == zerodds_ast::InterfaceKind::Abstract {
                        continue;
                    }
                    let message = alloc::format!(
                        "the abstract interface `{name}` derives from `{}`, which is not \
                         abstract; an abstract interface may only inherit from abstract interfaces",
                        base_node.name
                    );
                    self.diagnose_at(
                        DiagnosticCode::InvalidInheritance,
                        span,
                        &message,
                        "OMG IDL 4.2 §7.4.7.4.2, Table 7-19",
                    );
                }
            }

            // Every operation and attribute of every base, direct or
            // indirect, each declaration once.
            let mut inherited: Vec<(String, AnyId, String)> = Vec::new();
            for base in self.all_bases_of(id) {
                let Ok(node) = self.ast.interface(base) else {
                    continue;
                };
                let base_name = node.name.clone();
                for (element, any) in self.elements_of(base) {
                    if !inherited.iter().any(|(_, known, _)| *known == any) {
                        inherited.push((element, any, base_name.clone()));
                    }
                }
            }
            for (index, (element, any, from)) in inherited.iter().enumerate() {
                let Some((_, _, other)) = inherited[..index]
                    .iter()
                    .find(|(known, other, _)| known.eq_ignore_ascii_case(element) && other != any)
                else {
                    continue;
                };
                let message = alloc::format!(
                    "`{name}` inherits two different operations or attributes named `{element}`, \
                     from `{other}` and from `{from}`"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidInheritance,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.4.3.4.3.2.1",
                );
            }
            for (element, any) in self.elements_of(id) {
                let Some((_, _, from)) = inherited
                    .iter()
                    .find(|(known, _, _)| known.eq_ignore_ascii_case(&element))
                else {
                    continue;
                };
                let at = self.span_of(any);
                let message = alloc::format!(
                    "`{element}` redefines the operation or attribute `{name}` inherits from \
                     `{from}`; a derived interface may not redefine one"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidInheritance,
                    at,
                    &message,
                    "OMG IDL 4.2 §7.4.3.4.3.2.1",
                );
            }
        }
    }

    /// The direct and indirect bases of an interface, each once.
    fn all_bases_of(
        &self,
        interface: Id<zerodds_ast::InterfaceNode>,
    ) -> Vec<Id<zerodds_ast::InterfaceNode>> {
        let mut out = Vec::new();
        let mut queue: Vec<Id<zerodds_ast::InterfaceNode>> = self
            .ast
            .interface(interface)
            .map(|node| node.bases.iter().filter_map(Ref::resolved).collect())
            .unwrap_or_default();
        while let Some(next) = (!queue.is_empty()).then(|| queue.remove(0)) {
            if next == interface || out.contains(&next) || out.len() > MAX_ALIAS_DEPTH {
                continue;
            }
            if let Ok(node) = self.ast.interface(next) {
                queue.extend(node.bases.iter().filter_map(Ref::resolved));
            }
            out.push(next);
        }
        out
    }

    /// The operations and attributes an interface declares itself, with
    /// their names.
    fn elements_of(&self, interface: Id<zerodds_ast::InterfaceNode>) -> Vec<(String, AnyId)> {
        let mut out = Vec::new();
        for operation in self.ast.operations_of(interface) {
            if let Ok(node) = self.ast.operation(*operation) {
                out.push((
                    node.name.clone(),
                    AnyId::Operation(*operation, node.node_id),
                ));
            }
        }
        for attribute in self.ast.attributes_of(interface) {
            if let Ok(node) = self.ast.attribute(*attribute) {
                out.push((
                    node.name.clone(),
                    AnyId::Attribute(*attribute, node.node_id),
                ));
            }
        }
        out
    }

    /// A constant's value lies in the range of the integer type it is
    /// declared with.
    ///
    /// OMG IDL 4.2 §7.4.1.4.3: "If the value of an integer constant
    /// declaration is too large to fit in the actual type of the constant on
    /// the left hand side (for example `const short s = 655592;`) or is
    /// inappropriate for the actual type of the constant (for example
    /// `const octet o = -54;`) it shall be treated as an error." The ranges
    /// are the ones Table 7-26 (§7.4.13.4.6) gives every integer type,
    /// [`primitive_range`]. The form check in lowering
    /// (`value_disagrees_with`) answers whether the value is an integer at
    /// all; this one, run over the finished model so that a reference to a
    /// constant declared later is resolved, answers whether it fits.
    fn constant_values_fit_their_type(&mut self) {
        let constants: Vec<_> = self
            .ast
            .constants()
            .map(|(id, node)| {
                (
                    id,
                    node.node_id,
                    node.name.clone(),
                    node.type_.clone(),
                    node.expr,
                )
            })
            .collect();
        for (id, node_id, name, declared, expr) in constants {
            let Some(target) = declared.resolved().and_then(|t| self.through_aliases(t)) else {
                continue;
            };
            let Ok(TypeKind::Primitive(kind)) =
                self.ast.type_(target).map(|node| node.kind.clone())
            else {
                continue;
            };
            if matches!(
                kind,
                PrimitiveKind::Boolean | PrimitiveKind::Char | PrimitiveKind::WChar
            ) {
                continue;
            }
            let Some((low, high)) = primitive_range(kind) else {
                continue;
            };
            let Ok(zerodds_ast::ConstValue::Integer(value)) = self.ast.eval_const(expr) else {
                continue;
            };
            if (low..=high).contains(&value) {
                continue;
            }
            let span = self.span_of(AnyId::Const(id, node_id));
            let message = alloc::format!(
                "the constant `{name}` has the value {value}, and its type `{}` holds {low} to \
                 {high}; a value that does not fit its type is an error",
                kind.as_str()
            );
            self.diagnose_at(
                DiagnosticCode::TypeMismatch,
                span,
                &message,
                "OMG IDL 4.2 §7.4.1.4.3, Table 7-26",
            );
        }
    }

    /// A one-way operation returns nothing and passes nothing back.
    ///
    /// OMG IDL 4.2 §8.3.6.2 on `@oneway`: "This annotation may only concern
    /// operations without any return value (void return type) and without
    /// any out or inout parameters." Rule (120) states the same of the
    /// keyword form, `"oneway" "void" <identifier> "(" [ <in_parameter_dcls> ] ")"`.
    fn oneway_operations_return_nothing(&mut self) {
        let operations: Vec<_> = self
            .ast
            .operations()
            .filter(|(_, node)| node.oneway)
            .map(|(id, node)| {
                (
                    id,
                    node.node_id,
                    node.name.clone(),
                    node.return_type.is_some(),
                    node.params.clone(),
                    !node.raises.is_empty(),
                )
            })
            .collect();
        for (id, node_id, name, returns, params, raises) in operations {
            let span = self.span_of(AnyId::Operation(id, node_id));
            // §7.4.6.4.5: one-way operations "Cannot trigger exceptions."
            if raises {
                let message = alloc::format!(
                    "the one-way operation `{name}` has a `raises` clause; a one-way operation \
                     cannot trigger exceptions"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    span,
                    &message,
                    "OMG IDL 4.2 §7.4.6.4.5",
                );
            }
            if returns {
                let message = alloc::format!(
                    "the one-way operation `{name}` returns a value; a one-way operation's \
                     return type is `void`"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    span,
                    &message,
                    "OMG IDL 4.2 §8.3.6.2",
                );
            }
            for param in params {
                let Ok(param) = self.ast.param(param) else {
                    continue;
                };
                if param.direction == zerodds_ast::ParamDirection::In {
                    continue;
                }
                let message = alloc::format!(
                    "the one-way operation `{name}` has the {} parameter `{}`; a one-way \
                     operation takes `in` parameters only",
                    param.direction.as_str(),
                    param.name
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    span,
                    &message,
                    "OMG IDL 4.2 §8.3.6.2",
                );
            }
        }
    }

    /// The upper end of a range is not below its lower end.
    ///
    /// OMG IDL 4.2 §8.3.3.2: "The max value must be greater or equal to the
    /// min value." `@min` and `@max` written separately state the same two
    /// ends (§8.3.3.3, §8.3.3.4) and are held to the same order.
    fn ranges_are_ordered(&mut self) {
        let mut ranged: Vec<(AnyId, zerodds_ast::RangeConstraint)> = Vec::new();
        for (id, node) in self.ast.types() {
            if let Some(range) = node.facets.xtypes.as_ref().and_then(|x| x.range) {
                ranged.push((AnyId::Type(id, node.node_id), range));
            }
        }
        for (id, node) in self.ast.members() {
            if let Some(range) = node.facets.xtypes.as_ref().and_then(|x| x.range) {
                ranged.push((AnyId::Member(id, node.node_id), range));
            }
        }
        for (any, range) in ranged {
            let (Some(min), Some(max)) = (range.min, range.max) else {
                continue;
            };
            let (Ok(low), Ok(high)) = (self.ast.eval_const(min), self.ast.eval_const(max)) else {
                continue;
            };
            let Some(order) = numeric_order(&low, &high) else {
                continue;
            };
            if order != core::cmp::Ordering::Greater {
                continue;
            }
            let span = self.span_of(any);
            let message = alloc::format!(
                "the range runs from {} down to {}; the max value must be greater than or \
                 equal to the min value",
                value_text(&low),
                value_text(&high)
            );
            self.diagnose_at(
                DiagnosticCode::InvalidAnnotation,
                span,
                &message,
                "OMG IDL 4.2 §8.3.3.2",
            );
        }
    }

    fn validate_type(&mut self, id: Id<TypeNode>) {
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        let span = self.span_of(AnyId::Type(id, node.node_id));
        self.external_is_a_member_annotation(id, span);
        self.topic_is_on_a_topic_type(id, span);
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        match &node.kind {
            TypeKind::Union { discriminator, .. } => {
                let discriminator = discriminator.clone();
                self.union_rules(id, &discriminator, span);
                self.union_member_rules(id);
                let empty = self.ast.cases_of(id).is_empty();
                self.body_is_not_empty(
                    empty,
                    span,
                    "a `union` needs at least one `case`",
                    "OMG IDL 4.2 §7.4.1.3 rule (52)",
                );
            }
            TypeKind::Enumeration { enumerators } => {
                let empty = enumerators.is_empty();
                let enumerators = enumerators.clone();
                self.body_is_not_empty(
                    empty,
                    span,
                    "an `enum` needs at least one enumerator",
                    "OMG IDL 4.2 §7.4.1.3 rule (57)",
                );
                self.enumerator_values_fit_int32(&enumerators, span);
                self.enumerator_values_are_unique(&enumerators, span);
                self.extensibility_is_final_or_appendable(id, "enum", span);
            }
            TypeKind::Bitmask { bits } => {
                let empty = bits.is_empty();
                self.body_is_not_empty(
                    empty,
                    span,
                    "a `bitmask` needs at least one bit value",
                    "OMG IDL 4.2 §7.4.13.3 rule (204)",
                );
                self.extensibility_is_final_or_appendable(id, "bitmask", span);
            }
            TypeKind::Fixed { digits, scale } => {
                let (digits, scale) = (*digits, *scale);
                self.fixed_rules(digits, scale, span);
            }
            TypeKind::String { bound, .. } => {
                let bound = *bound;
                self.bound_is_positive(bound, span, "a bound");
            }
            TypeKind::Sequence { bound, .. } => {
                let bound = *bound;
                self.bound_is_positive(bound, span, "a bound");
                self.trim_is_defined(id, span, "a sequence element");
            }
            TypeKind::Map { bound, .. } => {
                let bound = *bound;
                self.bound_is_positive(bound, span, "a map bound");
                self.trim_is_defined(id, span, "a map value");
            }
            // Array dimensions and duplicate default cases are
            // structural invariants the model refuses outright; checking
            // them again here would give one rule two answers.
            TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
                let derives = matches!(node.kind, TypeKind::Structure { base: Some(_), .. });
                self.member_rules(id);
                if derives {
                    self.inheritance_rules(id, span);
                }
            }
            _ => {}
        }
    }

    /// `@external` written on a type declaration is refused by name.
    ///
    /// DDS-XTypes 1.3 Table 21 applies `@external` to members; on a
    /// `typedef`, a `struct` or any other type declaration the model has
    /// no consumer for it — the layout planner, the codec walks and the
    /// language adapters all read it off the member. Before this rule,
    /// `@external typedef long Handle;` lowered without a diagnostic and
    /// the flag sat on the alias node where nothing ever read it — an
    /// annotation that vanished between the source and every consumer,
    /// which is the silent-substitution class this project refuses.
    fn external_is_a_member_annotation(&mut self, id: Id<TypeNode>, span: Span) {
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        let external = node
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|xtypes| xtypes.is_external);
        if !external {
            return;
        }
        let what = node.kind.as_str();
        let message = alloc::format!(
            "`@external` applies to the members of an aggregated type, not to a {what} \
             declaration; nothing downstream reads it here, so it is refused rather than \
             silently dropped"
        );
        self.diagnose_at(
            DiagnosticCode::InvalidAnnotation,
            span,
            &message,
            "DDS-XTypes 1.3 Table 21",
        );
    }

    /// An enumerated type is `@final` or `@appendable`, never `@mutable`.
    ///
    /// DDS-XTypes 1.3 Table 12 (§7.2.3): "Enumerated Types: ENUMERATION_TYPE,
    /// BITMASK_TYPE — These types may be final or appendable on a
    /// type-by-type basis." §7.2.2.4.1 repeats it for enumerations. A
    /// `@mutable` enumeration reached the TypeObject as an `IS_MUTABLE`
    /// bit in the enumeration's flag word, a kind of enumeration the
    /// specification does not have.
    fn extensibility_is_final_or_appendable(&mut self, id: Id<TypeNode>, what: &str, span: Span) {
        let mutable = self
            .ast
            .type_(id)
            .ok()
            .and_then(|node| node.facets.xtypes.as_ref())
            .is_some_and(|xtypes| {
                xtypes.extensibility == Some(zerodds_ast::Extensibility::Mutable)
            });
        if mutable {
            let message = alloc::format!(
                "an `{what}` is `@final` or `@appendable`; `@mutable` applies to structures and \
                 unions"
            );
            self.diagnose_at(
                DiagnosticCode::InvalidAnnotation,
                span,
                &message,
                "DDS-XTypes 1.3 §7.2.2.4.1, Table 12",
            );
        }
    }

    /// The value of an enumerated literal is an Int32.
    ///
    /// DDS-XTypes 1.3 Table 4: an enumerated literal has "a name and an
    /// Int32 value". `@value` takes any integer constant, and the
    /// TypeObject writes `CommonEnumeratedLiteral.value` as an `int32`
    /// (§7.3.4.5.5), so a value outside the range has no representation
    /// and `zerodds_types::from_ast` reports `OutOfRange` for it.
    fn enumerator_values_fit_int32(
        &mut self,
        enumerators: &[Id<zerodds_ast::EnumeratorNode>],
        span: Span,
    ) {
        for id in enumerators {
            let Ok(node) = self.ast.enumerator(*id) else {
                continue;
            };
            let name = node.name.clone();
            let Some(value) = self.ast.effective_enumerator_value(*id) else {
                continue;
            };
            if i32::try_from(value).is_err() {
                let message = alloc::format!(
                    "the enumerator `{name}` has the value `{value}`, which is no Int32"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    span,
                    &message,
                    "DDS-XTypes 1.3 Table 4, §7.2.2.4.1",
                );
            }
        }
    }

    /// The literals of an enumeration have unique values.
    ///
    /// DDS-XTypes 1.3 Table 4: an enumerated literal has "a name and an
    /// Int32 value that are each unique within that type". Two literals
    /// of one value reached the TypeObject as two literals the Table 18
    /// rule ("any literals that have the same value in T1 and T2 also have
    /// the same name") can not tell apart.
    fn enumerator_values_are_unique(
        &mut self,
        enumerators: &[Id<zerodds_ast::EnumeratorNode>],
        span: Span,
    ) {
        let mut seen: Vec<(i128, String)> = Vec::new();
        for id in enumerators {
            let Ok(node) = self.ast.enumerator(*id) else {
                continue;
            };
            let name = node.name.clone();
            let Some(value) = self.ast.effective_enumerator_value(*id) else {
                continue;
            };
            if let Some((_, first)) = seen.iter().find(|(v, _)| *v == value) {
                let message = alloc::format!(
                    "the enumerators `{first}` and `{name}` both have the value `{value}`"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    span,
                    &message,
                    "DDS-XTypes 1.3 Table 4, §7.2.2.4.1",
                );
            } else {
                seen.push((value, name));
            }
        }
    }

    /// The type of an annotation member is one §7.2.2.6 permits.
    ///
    /// "The types permitted for an AnnotationParameter are: Primitive
    /// types, String types of Char8 or Char16 elements, Enumerated types."
    /// An alias resolves to its target first.
    fn annotation_member_types(&mut self) {
        let defs: Vec<_> = self
            .ast
            .annotation_defs()
            .map(|(id, node)| (id, node.node_id, node.name.clone(), node.members.clone()))
            .collect();
        for (id, node_id, name, members) in defs {
            let at = self.span_of(AnyId::AnnotationDef(id, node_id));
            for member in members {
                let Ok(node) = self.ast.annotation_member(member) else {
                    continue;
                };
                let member_name = node.name.clone();
                let Some(target) = node.type_.resolved() else {
                    continue;
                };
                if self.is_any(target, 0) {
                    // OMG IDL 4.2 rules (223) and (224) give an annotation
                    // member the type `any` (`<any_const_type>`), and the
                    // note of §7.4.15.4.2 says what its value has to match.
                    // `any` is the Any building block (§7.4.2), which the
                    // Extensible DDS Profile of DDS-XTypes 1.3 §7.3.1.1.1
                    // does not hold: "These constructs remain legal for use
                    // in IDL files provided to IDL compilers compliant with
                    // this specification. [...] Compilers that do not
                    // support them shall ignore them or issue a warning
                    // rather than halting with an error." §7.2.2.6 admits
                    // no such annotation parameter, so the declaration is
                    // read and warned about.
                    let message = alloc::format!(
                        "`@{name}`'s member `{member_name}` is any, which DDS-XTypes 1.3 \
                         §7.2.2.6 does not admit for an annotation parameter; the declaration \
                         has no representation in the type system"
                    );
                    self.warn_at(
                        DiagnosticCode::InvalidAnnotation,
                        at,
                        &message,
                        "DDS-XTypes 1.3 §7.3.1.1.1",
                    );
                    continue;
                }
                if !self.is_legal_annotation_member_type(target, 0) {
                    let found = self
                        .ast
                        .type_(target)
                        .map_or("an unknown type", |node| node.kind.as_str());
                    let message = alloc::format!(
                        "`@{name}`'s member `{member_name}` is {found}; an annotation member is a \
                         primitive, a string or wstring, or an enumeration"
                    );
                    self.diagnose_at(
                        DiagnosticCode::InvalidAnnotation,
                        at,
                        &message,
                        "DDS-XTypes 1.3 §7.2.2.6",
                    );
                }
            }
        }
    }

    /// Whether `target` is `any`, through any number of aliases.
    fn is_any(&self, target: Id<TypeNode>, depth: usize) -> bool {
        if depth > MAX_ALIAS_DEPTH {
            return false;
        }
        let Ok(node) = self.ast.type_(target) else {
            return false;
        };
        match &node.kind {
            TypeKind::Any => true,
            TypeKind::Alias { aliased } => aliased
                .resolved()
                .is_some_and(|target| self.is_any(target, depth + 1)),
            _ => false,
        }
    }

    fn is_legal_annotation_member_type(&self, target: Id<TypeNode>, depth: usize) -> bool {
        if depth > MAX_ALIAS_DEPTH {
            return false;
        }
        let Ok(node) = self.ast.type_(target) else {
            return false;
        };
        match &node.kind {
            TypeKind::Primitive(_) | TypeKind::Enumeration { .. } | TypeKind::String { .. } => true,
            TypeKind::Alias { aliased } => aliased
                .resolved()
                .is_some_and(|target| self.is_legal_annotation_member_type(target, depth + 1)),
            _ => false,
        }
    }

    /// `@topic` is written on a type that may be a topic type.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.1.16: "The @topic annotation can only be
    /// used on types that are legal as DDS Topic types, see 7.6.2", and
    /// §7.6.2: "The only types that may be associated with a DDS Topic
    /// are the Aggregated Types" — structures and unions.
    fn topic_is_on_a_topic_type(&mut self, id: Id<TypeNode>, span: Span) {
        let Ok(node) = self.ast.type_(id) else {
            return;
        };
        if node
            .facets
            .xtypes
            .as_ref()
            .is_none_or(|xtypes| xtypes.topic.is_none())
        {
            return;
        }
        if matches!(
            node.kind,
            TypeKind::Structure { .. } | TypeKind::Union { .. } | TypeKind::Valuetype { .. }
        ) {
            return;
        }
        let what = node.kind.as_str();
        let message = alloc::format!(
            "`@topic` applies to the aggregated types that DDS allows as topic types, not to a \
             {what} declaration"
        );
        self.diagnose_at(
            DiagnosticCode::InvalidAnnotation,
            span,
            &message,
            "DDS-XTypes 1.3 §7.3.1.2.1.16, §7.6.2",
        );
    }

    // -- §6: bodies the grammar makes one-or-more ------------------------

    /// A repetition IDL 4.2 writes as one-or-more, written as none.
    ///
    /// Three rules say it and all three were read here as zero-or-more:
    /// (52) `<switch_body> ::= <case>+`, (57)'s `<enumerator> { ","
    /// <enumerator> }*` and (204)'s `<bit_value> { "," <bit_value> }*`.
    /// `enum E { };` reached the model as an enumeration with no value
    /// it could ever hold and nothing said about it; all seven foreign
    /// front ends measured refuse the file.
    ///
    /// Two empty bodies next door are legal and stay legal, which is why
    /// this is a check per rule and not a rule about empty bodies: (198)
    /// adds `"struct" <identifier> "{" "}"` to Extended Data-Types
    /// outright, and (200) writes a bitset's body as `<bitfield>*`.
    fn body_is_not_empty(&mut self, empty: bool, at: Span, message: &str, spec: &'static str) {
        if empty {
            self.diagnose_at(DiagnosticCode::MissingToken, at, message, spec);
        }
    }

    // -- §6: unions ------------------------------------------------------

    /// A discriminator must be integral, boolean, character or an enum,
    /// and every case label must be unique. IDL 4.2 §7.4.1.4.4.3.
    fn union_rules(&mut self, id: Id<TypeNode>, discriminator: &Ref<TypeNode>, span: Span) {
        let Some(target) = discriminator.resolved() else {
            // An unresolved discriminator is a resolution failure, which
            // the resolve pass reports; reporting it again here would
            // say the same thing twice.
            return;
        };
        if !self.is_legal_discriminator(target) {
            let found = self
                .ast
                .type_(target)
                .map_or("an unknown type", |node| node.kind.as_str());
            let message = alloc::format!(
                "a union discriminator must be an integer, boolean, character or enum, not {found}"
            );
            self.diagnose_at(
                DiagnosticCode::InvalidDiscriminator,
                span,
                &message,
                "OMG IDL 4.2 §7.4.1.4.4.3",
            );
        }

        // Labels are unique across the whole union.
        let mut seen: Vec<i128> = Vec::new();
        for case in self.ast.cases_of(id).to_vec() {
            let Ok(node) = self.ast.case(case) else {
                continue;
            };
            let labels = node.labels.clone();
            let at = self.span_of(AnyId::Case(case, node.node_id));
            for label in labels {
                match label {
                    // A second `default:` is a structural invariant the
                    // model already refuses.
                    zerodds_ast::CaseLabel::Default => {}
                    zerodds_ast::CaseLabel::Value(expr) => {
                        let Ok(value) = self.ast.eval_integral(expr) else {
                            continue;
                        };
                        if seen.contains(&value) {
                            let message =
                                alloc::format!("the case label `{value}` appears more than once");
                            self.diagnose_at(
                                DiagnosticCode::InvalidCaseLabel,
                                at,
                                &message,
                                "OMG IDL 4.2 §7.4.1.4.4.3",
                            );
                        } else {
                            seen.push(value);
                        }
                        self.label_is_in_range(target, value, at);
                    }
                }
            }
        }
    }

    fn is_legal_discriminator(&self, target: Id<TypeNode>) -> bool {
        let Ok(node) = self.ast.type_(target) else {
            return false;
        };
        match &node.kind {
            TypeKind::Enumeration { .. } => true,
            TypeKind::Primitive(kind) => matches!(
                kind,
                PrimitiveKind::Boolean
                    | PrimitiveKind::Char
                    | PrimitiveKind::WChar
                    | PrimitiveKind::Octet
                    | PrimitiveKind::Int8
                    | PrimitiveKind::UInt8
                    | PrimitiveKind::Int16
                    | PrimitiveKind::UInt16
                    | PrimitiveKind::Int32
                    | PrimitiveKind::UInt32
                    | PrimitiveKind::Int64
                    | PrimitiveKind::UInt64
            ),
            // A typedef to a legal discriminator is one too.
            TypeKind::Alias { aliased } => aliased
                .resolved()
                .is_some_and(|target| self.is_legal_discriminator(target)),
            _ => false,
        }
    }

    /// A label outside the discriminator's range can never be selected.
    fn label_is_in_range(&mut self, discriminator: Id<TypeNode>, value: i128, at: Span) {
        let Ok(node) = self.ast.type_(discriminator) else {
            return;
        };
        let range = match &node.kind {
            TypeKind::Primitive(kind) => primitive_range(*kind),
            TypeKind::Enumeration { enumerators } => {
                // An enum label must name one of its own enumerators.
                let enumerators = enumerators.clone();
                let known = enumerators.iter().any(|enumerator| {
                    self.ast.effective_enumerator_value(*enumerator) == Some(value)
                });
                if !known {
                    let message = alloc::format!(
                        "the case label `{value}` is not one of the discriminator's enumerators"
                    );
                    self.diagnose_at(
                        DiagnosticCode::InvalidCaseLabel,
                        at,
                        &message,
                        "OMG IDL 4.2 §7.4.1.4.4.3",
                    );
                }
                return;
            }
            TypeKind::Alias { aliased } => {
                if let Some(target) = aliased.resolved() {
                    self.label_is_in_range(target, value, at);
                }
                return;
            }
            _ => return,
        };
        let Some((low, high)) = range else {
            return;
        };
        if value < low || value > high {
            let message = alloc::format!(
                "the case label `{value}` is outside the discriminator's range {low}..={high}"
            );
            self.diagnose_at(
                DiagnosticCode::InvalidCaseLabel,
                at,
                &message,
                "OMG IDL 4.2 §7.4.1.4.4.3",
            );
        }
    }

    // -- §6: members -----------------------------------------------------

    /// `@key` must not be `@optional`, and two members must not end up
    /// with the same id.
    fn member_rules(&mut self, id: Id<TypeNode>) {
        let members = self.ast.members_of(id).to_vec();
        let mut seen: Vec<(u32, String, &'static str)> = Vec::new();

        for member in members {
            let Ok(node) = self.ast.member(member) else {
                continue;
            };
            let member_any = AnyId::Member(member, node.node_id);
            let name = node.name.clone();
            let declared_ref = node.type_.clone();
            let at = self.span_of(member_any);
            // `try_construct` is checked before the facet is unwrapped:
            // a member with no annotations of its own can still inherit
            // `TRIM` from the alias it is declared with, and skipping it
            // here let `@try_construct(TRIM) typedef long L;` through.
            self.member_trim_is_defined(&declared_ref, member_any, &name, at);
            let Some(xtypes) = self.ast.facets(member_any).and_then(|f| f.xtypes.clone()) else {
                continue;
            };

            if xtypes.is_key && xtypes.is_optional {
                let message = alloc::format!("`{name}` cannot be both `@key` and `@optional`");
                self.diagnose_at(
                    DiagnosticCode::ConflictingAnnotations,
                    at,
                    &message,
                    "DDS-XTypes 1.3 §7.2.2.4.4.4.8, §7.3.1.2.1.3",
                );
            }
            if xtypes.is_optional && xtypes.default_value.is_some() {
                let message = alloc::format!(
                    "`{name}` is `@optional` and has a `@default`; an optional member has no \
                     explicit default value"
                );
                self.diagnose_at(
                    DiagnosticCode::ConflictingAnnotations,
                    at,
                    &message,
                    "DDS-XTypes 1.3 §7.2.2.4.4.4.9",
                );
            }
            if xtypes.is_key && xtypes.is_non_serialized {
                let message =
                    alloc::format!("`{name}` cannot be both `@key` and `@non_serialized`");
                self.diagnose_at(
                    DiagnosticCode::ConflictingAnnotations,
                    at,
                    &message,
                    "DDS-XTypes 1.3 §7.2.4.4.2",
                );
            }
            if xtypes.is_key {
                self.key_member_type_rules(member, &name, at);
            }

            // The model refuses a commit that would collide, but ids are
            // written onto members after they exist, so a collision
            // between the last two members reaches no commit. Both paths
            // word it the same way and the duplicate is dropped, so one
            // fact stays one diagnostic.
            if let Some(assignment) = xtypes.member_id {
                let origin = assignment.origin.as_str();
                if let Some((_, other, other_origin)) = seen
                    .iter()
                    .find(|(id, _, _)| *id == assignment.effective)
                    .cloned()
                {
                    let message = crate::lower::member_id_collision(
                        &name,
                        origin,
                        &other,
                        other_origin,
                        assignment.effective,
                    );
                    self.diagnose_at(
                        DiagnosticCode::DuplicateMemberId,
                        at,
                        &message,
                        "DDS-XTypes 1.3 §7.3.1.2.1.1",
                    );
                } else {
                    seen.push((assignment.effective, name.clone(), origin));
                }
            }
        }
    }

    // -- DDS-XTypes 1.3 §7.2.2.4.5: inheritance of structures ---------------

    /// The three conditions under which a structure may derive from
    /// another: its extensibility is unspecified or the base's, no member
    /// repeats a name or member id of the base chain, and it defines no key
    /// fields of its own.
    ///
    /// Measured before these rules (`zerodds-idlc check`): all three were
    /// accepted without a diagnostic. Cyclone DDS 11.0.1 refuses a key in a
    /// derived structure ("Derived struct is not allowed to expand on
    /// keys"), and so does Fast DDS; every ZeroDDS column of the
    /// `key-in-base-struct` corpus row wrote bytes for it.
    fn inheritance_rules(&mut self, id: Id<TypeNode>, span: Span) {
        const SPEC: &str = "DDS-XTypes 1.3 §7.2.2.4.5";
        let Some(base) = self.ast.base_of(id) else {
            return;
        };
        let name = self
            .ast
            .type_(id)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let base_name = self
            .ast
            .type_(base)
            .map(|n| n.name.clone())
            .unwrap_or_default();

        let declared = self
            .ast
            .type_(id)
            .ok()
            .and_then(|n| n.facets.extensibility());
        let inherited = self.ast.effective_extensibility(base);
        if let Some(declared) = declared.filter(|d| *d != inherited) {
            let message = alloc::format!(
                "`{name}` is {} and derives from `{base_name}`, which is {}; a derived \
                 structure states its base's extensibility or none",
                extensibility_word(declared),
                extensibility_word(inherited),
            );
            self.diagnose_at(DiagnosticCode::InvalidInheritance, span, &message, SPEC);
        }

        // Name and member id of every member the base chain contributes.
        let inherited_members: Vec<(String, Option<u32>)> = self
            .ast
            .all_members_of(base)
            .iter()
            .filter_map(|m| {
                let node = self.ast.member(*m).ok()?;
                let id = self
                    .ast
                    .facets(AnyId::Member(*m, node.node_id))
                    .and_then(|f| f.xtypes.as_ref())
                    .and_then(|x| x.member_id.as_ref())
                    .map(|a| a.effective);
                Some((node.name.clone(), id))
            })
            .collect();

        for member in self.ast.members_of(id).to_vec() {
            let Ok(node) = self.ast.member(member) else {
                continue;
            };
            let member_any = AnyId::Member(member, node.node_id);
            let member_name = node.name.clone();
            let at = self.span_of(member_any);
            let xtypes = self.ast.facets(member_any).and_then(|f| f.xtypes.clone());

            if inherited_members
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case(&member_name))
            {
                let message = alloc::format!(
                    "`{member_name}` is declared in `{name}` and inherited from `{base_name}`; \
                     the members of a structure and of all its bases share one set of names"
                );
                self.diagnose_at(DiagnosticCode::DuplicateName, at, &message, SPEC);
            }
            if let Some(effective) = xtypes
                .as_ref()
                .and_then(|x| x.member_id.as_ref())
                .map(|a| a.effective)
                && let Some((other, _)) = inherited_members
                    .iter()
                    .find(|(_, other_id)| *other_id == Some(effective))
            {
                let message = alloc::format!(
                    "`{member_name}` of `{name}` has member id {effective}, which `{other}` \
                         inherited from `{base_name}` already has"
                );
                self.diagnose_at(DiagnosticCode::DuplicateMemberId, at, &message, SPEC);
            }
            if xtypes.as_ref().is_some_and(|x| x.is_key) {
                let message = alloc::format!(
                    "`{member_name}` is a key of `{name}`, which derives from `{base_name}`; a \
                     derived structure defines no key fields, its keys are its root base's"
                );
                self.diagnose_at(DiagnosticCode::InvalidInheritance, at, &message, SPEC);
            }
        }
    }

    // -- §6: try_construct -----------------------------------------------

    /// `TRIM` on a member, where nothing at that position can be
    /// trimmed.
    ///
    /// An array member's annotation governs its **elements** rather than
    /// the array: Table 17's `ARRAY_TYPE` row knows no member-level
    /// behaviour at all, and an array whose bounds must match the
    /// sender's has no length to trim. Every other member is checked
    /// against its own type.
    fn member_trim_is_defined(
        &mut self,
        declared_ref: &Ref<TypeNode>,
        member: AnyId,
        name: &str,
        at: Span,
    ) {
        // The *unresolved* reference, so that a `TRIM` written on the
        // alias the member is declared with is seen. `refuse_trim`
        // resolves the chain again for the type it names.
        let declared = declared_ref.resolved();
        let written = self
            .ast
            .facets(member)
            .and_then(|f| f.xtypes.as_ref())
            .and_then(|x| x.try_construct);
        let array = declared
            .and_then(|id| self.through_aliases(id))
            .filter(|id| {
                matches!(
                    self.ast.type_(*id).map(|node| &node.kind),
                    Ok(TypeKind::Array { .. })
                )
            });
        if let Some(array) = array {
            if self.ast.element_try_construct(array, written) == TryConstruct::Trim {
                let element = match self.ast.type_(array).map(|node| node.kind.clone()) {
                    Ok(TypeKind::Array { element, .. }) => element.resolved(),
                    _ => None,
                };
                self.refuse_trim(element, at, "an array element");
            }
            return;
        }
        if self.ast.effective_try_construct(written, declared) == TryConstruct::Trim {
            let what = alloc::format!("the member `{name}`");
            self.refuse_trim(declared, at, &what);
        }
    }

    /// `TRIM` on a collection's element, where the element is something
    /// that cannot be trimmed.
    fn trim_is_defined(&mut self, collection: Id<TypeNode>, at: Span, what: &str) {
        if self.ast.element_try_construct(collection, None) != TryConstruct::Trim {
            return;
        }
        let element = match &self.ast.type_(collection).map(|node| node.kind.clone()) {
            Ok(TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. }) => {
                element.resolved()
            }
            Ok(TypeKind::Map { value, .. }) => value.resolved(),
            _ => None,
        };
        self.refuse_trim(element, at, what);
    }

    /// Report `TRIM` where Table 11 does not define it.
    ///
    /// Table 11: "This option only applies to elements or members of
    /// type string, wide string, sequence, or map. The behavior when
    /// applied to other element/member types is unspecified and may be
    /// treated as an error."
    ///
    /// Treated as an error, and not quietly as `DISCARD`. A declaration
    /// that asks for `TRIM` on a `long` is asking for something no
    /// receiver can do, and the two implementations that accept it —
    /// RTI, which drops the annotation entirely, and OpenDDS, which
    /// generates its `DISCARD` branch — give the author the opposite of
    /// what was written without saying so. Cyclone refuses it in the
    /// front end and names the reason;
    /// `crates/ast-layout/tests/fixtures/try_construct_behaviour.md`
    /// has the message.
    ///
    /// An unresolved type is not refused: the resolve pass reports it,
    /// and guessing here would put a second diagnostic on one fault.
    fn refuse_trim(&mut self, target: Option<Id<TypeNode>>, at: Span, what: &str) {
        let Some(target) = target else {
            return;
        };
        let Some(resolved) = self.through_aliases(target) else {
            return;
        };
        let Ok(node) = self.ast.type_(resolved) else {
            return;
        };
        if TryConstruct::trim_is_defined_for(&node.kind) {
            return;
        }
        let kind = node.kind.as_str();
        let message = alloc::format!(
            "`@try_construct(TRIM)` on {what} of type `{kind}` is not defined: only a \
             string, wstring, sequence or map can be trimmed"
        );
        self.diagnose_at(
            DiagnosticCode::InvalidAnnotation,
            at,
            &message,
            "DDS-XTypes 1.3 Table 11",
        );
    }

    /// The type an alias chain ends at. §7.2.4.4.3 resolves an alias to
    /// its base type for every rule in the clause.
    pub(crate) fn through_aliases(&self, start: Id<TypeNode>) -> Option<Id<TypeNode>> {
        let mut current = start;
        // Bounded: a `typedef` cycle is a separate fault with its own
        // report, and spinning here would hide it behind a hang.
        for _ in 0..MAX_ALIAS_DEPTH {
            let node = self.ast.type_(current).ok()?;
            let TypeKind::Alias { aliased } = &node.kind else {
                return Some(current);
            };
            current = aliased.resolved()?;
        }
        None
    }

    // -- §7.2.2.4.4.4.8: what may be part of a key ------------------------

    /// A `map` may not be part of a key, however deeply it is reached.
    ///
    /// §7.2.2.4.4.4.8: "Members of type map cannot be included as part
    /// of the key." Nothing in the tree enforced it, and a `map` inside
    /// a key holder is not a byte question but an *instance* question:
    /// the holder is what the KeyHash is computed over, and two samples
    /// whose maps hold the same pairs in a different order hash
    /// differently, so one instance gets two handles.
    ///
    /// # Where the walk goes, and where it stops
    ///
    /// The clause says which members are part of the key, so the walk
    /// follows exactly that:
    ///
    /// * an alias is transparent — "Aliases to one of the previous
    ///   types can also be used as key members";
    /// * an array or sequence carries its elements in — "If a member of
    ///   type array or sequence is marked as a key member of an
    ///   aggregated type T, all the elements in the array or sequence
    ///   shall be considered part of the key of T";
    /// * an aggregate carries only its **own** key members in — "In the
    ///   event that the type K of a key member of a given type T itself
    ///   defines key members, only the key of K, and not any other of
    ///   its members, shall be considered part of the key of T. This
    ///   relationship is recursive".
    ///
    /// It stops at an aggregate that declares no key member of its own.
    /// Read with §7.6.8 Step 1 — "Otherwise do not remove any members" —
    /// every member of such a type is in the key, which would make a
    /// `map` anywhere inside it a refusal here. That reading was the open
    /// question "A keyed member whose type declares no `@key` of its own"
    /// (closed: `keyhash::member_key_for`), it is not what
    /// `zerodds_ast_layout::keyhash::collect_key_members` implements, and
    /// no vendor has been asked. Refusing on a reading this project does
    /// not itself follow would reject IDL whose key holds no map at all.
    ///
    /// # What is not refused
    ///
    /// The clause also lists the types a key *may* include — "primitive,
    /// aggregation, enumeration, bitmask, array, and sequence" — and
    /// `string` is not among them. Its own Example 2 four pages later is
    /// `@key string<12> label`, `ShapeType`'s key in the OMG interop test
    /// is `@key string<128> color`, and Cyclone, RTI and Fast DDS all
    /// accept a string key. The list is incomplete, not exhaustive, and
    /// only the sentence that forbids something is enforced.
    fn key_member_type_rules(&mut self, member: Id<MemberNode>, name: &str, at: Span) {
        let Some(target) = self
            .ast
            .member(member)
            .ok()
            .and_then(|node| node.type_.resolved())
        else {
            return;
        };
        let mut seen = Vec::new();
        let Some(reached) = self.map_in_key(target, &mut seen) else {
            return;
        };
        let message = if reached == KeyReach::Direct {
            alloc::format!("`{name}` is a `map`, which cannot be part of a key")
        } else {
            alloc::format!("the key of `{name}` reaches a `map`, which cannot be part of a key")
        };
        self.diagnose_at(
            DiagnosticCode::InvalidAnnotation,
            at,
            &message,
            "DDS-XTypes 1.3 §7.2.2.4.4.4.8",
        );
    }

    /// Whether the key of `target` reaches a `map`, and how directly.
    ///
    /// `seen` is the cycle guard: a keyed member whose type reaches
    /// itself through a sequence is legal IDL, and a walk without one
    /// would not return.
    fn map_in_key(&self, target: Id<TypeNode>, seen: &mut Vec<Id<TypeNode>>) -> Option<KeyReach> {
        if seen.contains(&target) {
            return None;
        }
        seen.push(target);
        let node = self.ast.type_(target).ok()?;
        match &node.kind {
            TypeKind::Map { .. } => Some(KeyReach::Direct),
            TypeKind::Alias { aliased } => aliased
                .resolved()
                .and_then(|next| self.map_in_key(next, seen)),
            TypeKind::Sequence { element, .. } | TypeKind::Array { element, .. } => element
                .resolved()
                .and_then(|next| self.map_in_key(next, seen))
                .map(|_| KeyReach::Nested),
            TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
                let members = self.ast.all_members_of(target);
                for member in &members {
                    let Ok(member_node) = self.ast.member(*member) else {
                        continue;
                    };
                    if !member_node.facets.is_key() {
                        continue;
                    }
                    let Some(member_type) = member_node.type_.resolved() else {
                        continue;
                    };
                    if self.map_in_key(member_type, seen).is_some() {
                        return Some(KeyReach::Nested);
                    }
                }
                None
            }
            // A union's key is its discriminator and nothing else
            // (§7.2.2.4.4.4.8), and a discriminator is integral,
            // character, boolean or an enumeration — never a map.
            _ => None,
        }
    }

    /// In a union only the discriminator may be a key, and no union
    /// member may be optional.
    ///
    /// Table 21 lists `@key` against "Structure Members, Union
    /// discriminator" and `@optional` against "Structure Members" alone;
    /// §7.2.2.4.4.4.7 says it in prose — "Union members, including the
    /// discriminator, shall never be optional."
    ///
    /// Both were accepted and both change bytes. `@key` on a case member
    /// puts that member in the key holder, so an instance handle depends
    /// on which branch is selected; `@optional` on a case member plans a
    /// presence marker no implementation writes, so the discriminator's
    /// value and the member behind it come out of different offsets than
    /// any peer put them at.
    fn union_member_rules(&mut self, id: Id<TypeNode>) {
        for case in self.ast.cases_of(id).to_vec() {
            let Ok(case_node) = self.ast.case(case) else {
                continue;
            };
            let case_any = AnyId::Case(case, case_node.node_id);
            let member = case_node.member;
            let name = self
                .ast
                .member(member)
                .map_or_else(|_| String::from("a case member"), |node| node.name.clone());
            if name.eq_ignore_ascii_case("discriminator") {
                let span = self
                    .ast
                    .member(member)
                    .map(|node| self.span_of(AnyId::Member(member, node.node_id)))
                    .unwrap_or_default();
                let at = if span.end > span.start {
                    span
                } else {
                    self.span_of(case_any)
                };
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    at,
                    "a union member cannot be named `discriminator`; the name is reserved for \
                     the union's discriminator",
                    "DDS-XTypes 1.3 §7.2.2.4.4.3",
                );
            }
            // A case's annotations are applied to the case node, which
            // is where `lower.rs` puts them, and the member behind it
            // carries none of its own. Both are read so the rule does
            // not depend on which side a future change writes to.
            //
            // The span comes from whichever of the two has one. A
            // diagnostic with an empty range points at byte zero of the
            // file, which is the one place the mistake is not.
            let member_span = self
                .ast
                .member(member)
                .map(|node| self.span_of(AnyId::Member(member, node.node_id)))
                .unwrap_or_default();
            let case_span = self.span_of(case_any);
            let at = if case_span.end > case_span.start {
                case_span
            } else {
                member_span
            };
            let member_xtypes = self
                .ast
                .member(member)
                .ok()
                .and_then(|node| {
                    self.ast
                        .facets(AnyId::Member(member, node.node_id))
                        .cloned()
                })
                .and_then(|facets| facets.xtypes);
            let Some(xtypes) = self
                .ast
                .facets(case_any)
                .and_then(|f| f.xtypes.clone())
                .or(member_xtypes)
            else {
                continue;
            };
            if xtypes.is_key {
                let message = alloc::format!(
                    "`{name}` is a union member, and only a union's discriminator may be a key"
                );
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    at,
                    &message,
                    "DDS-XTypes 1.3 §7.2.2.4.4.4.8, Table 21",
                );
            }
            if xtypes.is_optional {
                let message =
                    alloc::format!("`{name}` is a union member, and no union member is optional");
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    at,
                    &message,
                    "DDS-XTypes 1.3 §7.2.2.4.4.4.7, Table 21",
                );
            }
        }
    }

    // -- §6: bounds, dimensions and precision ----------------------------

    fn bound_is_positive(&mut self, bound: Bound, at: Span, what: &str) {
        let Bound::Const(expr) = bound else {
            return;
        };
        let Ok(value) = self.ast.eval_integral(expr) else {
            return;
        };
        if value <= 0 {
            let message = alloc::format!("{what} must be positive, not `{value}`");
            self.diagnose_at(
                DiagnosticCode::InvalidBound,
                at,
                &message,
                "OMG IDL 4.2 §7.4.1.4.4",
            );
        }
    }

    /// `fixed<d,s>` requires `0 < d <= 31` and `0 <= s <= d`.
    /// IDL 4.2 §7.4.1.4.4.
    fn fixed_rules(&mut self, digits: u8, scale: u8, at: Span) {
        if digits == 0 || digits > MAX_FIXED_DIGITS {
            let message = alloc::format!(
                "`fixed` needs between 1 and {MAX_FIXED_DIGITS} digits, not `{digits}`"
            );
            self.diagnose_at(
                DiagnosticCode::InvalidBound,
                at,
                &message,
                "OMG IDL 4.2 §7.4.1.4.4",
            );
        }
        if scale > digits {
            let message = alloc::format!(
                "a `fixed` scale must not exceed its digits: `{scale}` of `{digits}`"
            );
            self.diagnose_at(
                DiagnosticCode::InvalidBound,
                at,
                &message,
                "OMG IDL 4.2 §7.4.1.4.4",
            );
        }
    }

    // -- §6: forward declarations ---------------------------------------

    /// A forward declaration must be defined in the same translation
    /// unit. IDL 4.2 §7.4.1.4.5.
    ///
    /// Over both arenas. An interface forward now makes an interface
    /// node rather than a `Structure` placeholder — rule (75) — and a
    /// rule that only walked the types would have stopped reporting the
    /// case it was written for.
    fn forward_declarations_are_defined(&mut self) {
        let unfinished: Vec<(AnyId, String)> = self
            .ast
            .types()
            .filter(|(_, node)| node.completeness == zerodds_ast::Completeness::Forward)
            .map(|(id, node)| (AnyId::Type(id, node.node_id), node.name.clone()))
            .chain(
                self.ast
                    .interfaces()
                    .filter(|(_, node)| node.completeness == zerodds_ast::Completeness::Forward)
                    .map(|(id, node)| (AnyId::Interface(id, node.node_id), node.name.clone())),
            )
            .collect();

        for (any, name) in unfinished {
            let at = self.span_of(any);
            let message = alloc::format!("`{name}` was declared but never defined in this file");
            self.diagnose_at(
                DiagnosticCode::UndefinedForward,
                at,
                &message,
                "OMG IDL 4.2 §7.4.1.4.5",
            );
        }
    }

    /// The byte range a node came from, empty when it has none.
    fn span_of(&self, any: AnyId) -> Span {
        self.ast
            .facets(any)
            .and_then(|facets| facets.source.span)
            .map_or(Span::new(0, 0), |span| Span::new(span.start, span.end))
    }
}

/// How two numeric constant values compare: exactly for two integers,
/// through `f64` once a floating or fixed-point value takes part. `None`
/// when either is not a number.
fn numeric_order(
    low: &zerodds_ast::ConstValue,
    high: &zerodds_ast::ConstValue,
) -> Option<core::cmp::Ordering> {
    use zerodds_ast::ConstValue;
    if let (ConstValue::Integer(a), ConstValue::Integer(b)) = (low, high) {
        return Some(a.cmp(b));
    }
    let as_f64 = |value: &ConstValue| -> Option<f64> {
        match value {
            ConstValue::Integer(number) => value_text(&ConstValue::Integer(*number)).parse().ok(),
            ConstValue::Float { .. } | ConstValue::Fixed { .. } => value_text(value).parse().ok(),
            _ => None,
        }
    };
    as_f64(low)?.partial_cmp(&as_f64(high)?)
}

/// A numeric constant value as decimal text, as a diagnostic quotes it.
fn value_text(value: &zerodds_ast::ConstValue) -> String {
    use zerodds_ast::ConstValue;
    match value {
        ConstValue::Integer(number) => alloc::format!("{number}"),
        ConstValue::Float { mantissa, exponent } => match exponent {
            Some(exponent) => alloc::format!("{mantissa}e{exponent}"),
            None => mantissa.clone(),
        },
        ConstValue::Fixed { digits, scale } => {
            let (sign, digits) = digits
                .strip_prefix('-')
                .map_or(("", digits.as_str()), |rest| ("-", rest));
            let scale = usize::from(*scale);
            if scale == 0 {
                alloc::format!("{sign}{digits}")
            } else if digits.len() > scale {
                let (whole, fraction) = digits.split_at(digits.len() - scale);
                alloc::format!("{sign}{whole}.{fraction}")
            } else {
                alloc::format!("{sign}0.{}{digits}", "0".repeat(scale - digits.len()))
            }
        }
        ConstValue::Boolean(flag) => alloc::format!("{flag}"),
        ConstValue::Char(c) => alloc::format!("'{c}'"),
        ConstValue::WChar(code) => alloc::format!("U+{code:04X}"),
        ConstValue::String(text) => alloc::format!("{text:?}"),
        ConstValue::WString(_) => String::from("a wide string"),
        _ => String::from("a value"),
    }
}

/// The annotation that states `extensibility`, as a diagnostic names it.
fn extensibility_word(extensibility: zerodds_ast::Extensibility) -> &'static str {
    match extensibility {
        zerodds_ast::Extensibility::Final => "`@final`",
        zerodds_ast::Extensibility::Appendable => "`@appendable`",
        zerodds_ast::Extensibility::Mutable => "`@mutable`",
    }
}

/// The value range of an integer type, Table 7-26 of OMG IDL 4.2
/// (§7.4.13.4.6), and the code range of the character types.
///
/// `None` for kinds a value cannot be compared against numerically.
fn primitive_range(kind: PrimitiveKind) -> Option<(i128, i128)> {
    Some(match kind {
        PrimitiveKind::Boolean => (0, 1),
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => (0, 255),
        PrimitiveKind::Char => (0, 255),
        PrimitiveKind::WChar => (0, 0xFFFF),
        PrimitiveKind::Int8 => (-128, 127),
        PrimitiveKind::Int16 => (-32_768, 32_767),
        PrimitiveKind::UInt16 => (0, 65_535),
        PrimitiveKind::Int32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
        PrimitiveKind::UInt32 => (0, i128::from(u32::MAX)),
        PrimitiveKind::Int64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
        PrimitiveKind::UInt64 => (0, i128::from(u64::MAX)),
        PrimitiveKind::Float32 | PrimitiveKind::Float64 | PrimitiveKind::Float128 => return None,
    })
}

/// The name of a diagnostic's rule, for a caller listing them.
#[must_use]
pub fn rule_name(code: DiagnosticCode) -> &'static str {
    match code {
        DiagnosticCode::DuplicateName => "duplicate-name",
        DiagnosticCode::AmbiguousName => "ambiguous-name",
        DiagnosticCode::UnresolvedName => "unresolved-name",
        DiagnosticCode::InvalidDiscriminator => "union-discriminator",
        DiagnosticCode::InvalidCaseLabel => "union-case-label",
        DiagnosticCode::ConflictingAnnotations => "conflicting-annotations",
        DiagnosticCode::InvalidBound => "bound-range",
        DiagnosticCode::UndefinedForward => "forward-defined",
        DiagnosticCode::DuplicateMemberId => "member-id-collision",
        DiagnosticCode::InvalidAnnotation => "annotation-argument",
        DiagnosticCode::KeywordCollision => "keyword-collision",
        DiagnosticCode::InvalidInheritance => "inheritance",
        DiagnosticCode::OutsideProfile => "profile",
        DiagnosticCode::Preprocessing | DiagnosticCode::ErrorDirective => "preprocessor",
        DiagnosticCode::UnexpectedToken
        | DiagnosticCode::MissingToken
        | DiagnosticCode::UnterminatedDeclaration
        | DiagnosticCode::InvalidToken
        | DiagnosticCode::VendorExtension
        | DiagnosticCode::Unsupported => "syntax",
        _ => "unknown",
    }
}
