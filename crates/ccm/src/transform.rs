// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Equivalent-IDL transformation — Spec §6.3.2 / §6.4.1 / §6.5.1 /
//! §6.6.x / §6.7.1.
//!
//! Input: `zerodds_idl::ast::ComponentDef` / `HomeDef` / `EventDef`.
//! Output: one or more `InterfaceDef` (all spec-conformant) that
//! represent the "implicitly defined equivalent interface" of the CCM
//! spec.
//!
//! Spec §6.2 (S. 11): "A component definition in IDL implicitly defines
//! an interface that supports the features defined in the component
//! definition body."

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_idl::ast::{
    AttrDecl, ComponentDef, ComponentExport, EventDef, Export, HomeDef, Identifier, InterfaceDef,
    InterfaceKind, OpDecl, ParamAttribute, ParamDecl, PrimitiveType, ScopedName, StringType,
    TypeSpec, ValueKind,
};
use zerodds_idl::errors::Span;

/// Result of the component transformation: the equivalent interface
/// (Spec §6.3.2) plus all implied event-consumer interfaces (Spec
/// §6.6.6.2 "EventConsumers" module).
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentEquivalent {
    /// The component equivalent interface (Spec §6.3.2).
    pub equivalent_interface: InterfaceDef,
    /// Implied `<event_type>Consumer` interfaces for the
    /// `consumes`/`emits`/`publishes` ports (Spec §6.6.6.2 "EventConsumers"
    /// scope).
    pub event_consumer_interfaces: Vec<InterfaceDef>,
}

/// Result of the home transformation: the three interfaces from Spec
/// §6.7.1 (Explicit + Implicit + Equivalent).
#[derive(Debug, Clone, PartialEq)]
pub struct HomeEquivalent {
    /// `<home_name>Explicit : Components::CCMHome [, supported]`.
    pub explicit: InterfaceDef,
    /// `<home_name>Implicit : Components::KeylessCCMHome` OR the
    /// keyed variant without inheritance.
    pub implicit: InterfaceDef,
    /// `<home_name> : <home_name>Explicit, <home_name>Implicit { }`.
    pub equivalent: InterfaceDef,
}

/// Result of the event-type transformation — Spec §6.6.1.1.
///
/// From `eventtype E { state-members }`, two IDL definitions are
/// derived:
/// 1. `valuetype E { state-members }` (with additional inheritance from
///    `Components::EventBase` for the first eventtype in a chain).
/// 2. `interface EConsumer : Components::EventConsumerBase { void
///    push_E(in E the_e); };`.
#[derive(Debug, Clone, PartialEq)]
pub struct EventTypeEquivalent {
    /// The equivalent valuetype name (with `: Components::EventBase` for
    /// the first in the chain).
    pub valuetype_name: Identifier,
    /// Inheritance bases of the valuetype (per Spec §6.6.1.1: first in
    /// the chain `Components::EventBase`, derived eventtype `BaseValueType,
    /// :: Components::EventBase`).
    pub valuetype_bases: Vec<ScopedName>,
    /// `<event_type>Consumer` interface (Spec §6.6.1.1).
    pub consumer_interface: InterfaceDef,
}

/// Spec §6.3.2 (p. 12) + §6.4.1 / §6.5.1 / §6.6.x — generates the
/// component equivalent interface including all port operations.
///
/// **Inheritance (§6.3.2):**
/// * Without `supports` and without `base`: `: Components::CCMObject`.
/// * With `base`: `: <base>` (CCMObject is transitive).
/// * With `supports`: additionally the supported interfaces.
///
/// **Body (§6.4.1 / §6.5.1 / §6.6.x):** all port decls are translated
/// into operations on the equivalent interface.
#[must_use]
pub fn transform_component(comp: &ComponentDef) -> ComponentEquivalent {
    let span = Span::SYNTHETIC;

    // Inheritance: §6.3.2.
    let bases = component_bases(comp, span);

    let mut exports: Vec<Export> = Vec::new();
    let mut event_consumer_ifaces: Vec<InterfaceDef> = Vec::new();
    let mut emitted_consumers: Vec<String> = Vec::new();

    for export in &comp.body {
        match export {
            ComponentExport::Provides {
                type_spec, name, ..
            } => {
                exports.push(Export::Op(provide_facet_op(name, type_spec, span)));
            }
            ComponentExport::Uses {
                type_spec,
                name,
                multiple,
                ..
            } => {
                exports.extend(uses_ops(name, type_spec, *multiple, span));
            }
            ComponentExport::Attribute(attr) => {
                exports.push(Export::Attr(attr_to_attr_decl(attr, span)));
            }
            ComponentExport::Emits {
                type_spec, name, ..
            } => {
                ensure_consumer_interface(
                    type_spec,
                    span,
                    &mut event_consumer_ifaces,
                    &mut emitted_consumers,
                );
                exports.extend(emits_ops(name, type_spec, span));
            }
            ComponentExport::Publishes {
                type_spec, name, ..
            } => {
                ensure_consumer_interface(
                    type_spec,
                    span,
                    &mut event_consumer_ifaces,
                    &mut emitted_consumers,
                );
                exports.extend(publishes_ops(name, type_spec, span));
            }
            ComponentExport::Consumes {
                type_spec, name, ..
            } => {
                ensure_consumer_interface(
                    type_spec,
                    span,
                    &mut event_consumer_ifaces,
                    &mut emitted_consumers,
                );
                exports.push(Export::Op(consumes_op(name, type_spec, span)));
            }
            ComponentExport::Port { .. } => {
                // Spec §7.4.11 (IDL4) — a `port` decl is a user-defined
                // port type. The equivalent-IDL mapping is defined via the
                // porttype (the spec refers to the connector mapping).
                // In CCM 4.0 this is not directly §6 but rather §7.4.11
                // IDL4 (add-on); here we do not invoke it and leave it to
                // the caller if desired.
            }
        }
    }

    let equivalent_interface = InterfaceDef {
        kind: InterfaceKind::Plain,
        name: comp.name.clone(),
        bases,
        exports,
        annotations: Vec::new(),
        span,
    };

    ComponentEquivalent {
        equivalent_interface,
        event_consumer_interfaces: event_consumer_ifaces,
    }
}

/// Spec §6.7.1 (p. 33) — generates the three interfaces (Explicit,
/// Implicit, Equivalent) from a `home` decl.
#[must_use]
pub fn transform_home(home: &HomeDef) -> HomeEquivalent {
    let span = Span::SYNTHETIC;
    let h = &home.name.text;

    // Spec §6.7.1.3 (p. 35): the Explicit iface inherits CCMHome + supported.
    let mut explicit_bases = alloc::vec![scoped(&["Components", "CCMHome"], span)];
    explicit_bases.extend(home.supports.iter().cloned());
    if let Some(base) = &home.base {
        // Spec §6.7.4 (p. 37) — derived home: `<home>Explicit :
        // <base_home>Explicit`.
        explicit_bases = alloc::vec![base_explicit_name(base, span)];
        explicit_bases.extend(home.supports.iter().cloned());
    }

    let explicit = InterfaceDef {
        kind: InterfaceKind::Plain,
        name: Identifier::new(format!("{h}Explicit"), span),
        bases: explicit_bases,
        // Spec §6.7.3 — the Explicit iface contains the explicitly
        // declared operations + attributes; factory/finder are converted
        // (see `factory_op_to_explicit` / `finder_op_to_explicit`); here
        // we return an empty list because the body is not further modeled
        // in HomeDef (completing it is the caller's responsibility for
        // factory/finder decls).
        exports: Vec::new(),
        annotations: Vec::new(),
        span,
    };

    // Spec §6.7.1.1 (no primary key) vs §6.7.1.2 (with primary key).
    let implicit = if let Some(pk) = &home.primary_key {
        build_keyed_implicit(h, &home.manages, pk, span)
    } else {
        build_keyless_implicit(h, &home.manages, span)
    };

    // Spec §6.7.1.x — the Equivalent iface inherits Explicit + Implicit.
    let equivalent = InterfaceDef {
        kind: InterfaceKind::Plain,
        name: home.name.clone(),
        bases: alloc::vec![
            ScopedName::single(Identifier::new(format!("{h}Explicit"), span)),
            ScopedName::single(Identifier::new(format!("{h}Implicit"), span)),
        ],
        exports: Vec::new(),
        annotations: Vec::new(),
        span,
    };

    HomeEquivalent {
        explicit,
        implicit,
        equivalent,
    }
}

/// Spec §6.6.1.1 (p. 24) — eventtype → valuetype + consumer interface.
#[must_use]
pub fn transform_event_type(et: &EventDef) -> EventTypeEquivalent {
    let span = Span::SYNTHETIC;

    // Spec §6.6.1.1 (p. 25): "the first event type in the inheritance
    // chain introduces the inheritance from Components::EventBase".
    // If EventDef already has an inheritance, EventBase is NOT added
    // again.
    let mut bases: Vec<ScopedName> = Vec::new();
    let mut has_inheritance = false;
    if let Some(inherit) = &et.inheritance {
        if !inherit.bases.is_empty() {
            has_inheritance = true;
            bases.extend(inherit.bases.iter().cloned());
        }
    }
    if !has_inheritance {
        // First eventtype in the chain → EventBase as inheritance.
        bases.push(scoped(&["Components", "EventBase"], span));
    }

    // Consumer interface (Spec §6.6.1.1 p. 25).
    let consumer_iface_name = format!("{}Consumer", et.name.text);
    let push_op = OpDecl {
        name: Identifier::new(format!("push_{}", et.name.text), span),
        oneway: false,
        context: Vec::new(),
        return_type: None,
        params: alloc::vec![ParamDecl {
            attribute: ParamAttribute::In,
            type_spec: TypeSpec::Scoped(ScopedName::single(et.name.clone())),
            name: Identifier::new(format!("the_{}", lowercase_first(&et.name.text)), span),
            annotations: Vec::new(),
            span,
        }],
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    };
    let consumer_bases = if has_inheritance {
        // Spec §6.6.1.1 (p. 25): "Consumer interfaces are in the same
        // inheritance relation as the event types".
        et.inheritance.as_ref().map_or_else(
            || alloc::vec![scoped(&["Components", "EventConsumerBase"], span)],
            |inh| {
                inh.bases
                    .iter()
                    .map(|b| {
                        let last = b
                            .parts
                            .last()
                            .map_or("Base", |i| i.text.as_str())
                            .to_string();
                        ScopedName::single(Identifier::new(format!("{last}Consumer"), span))
                    })
                    .collect()
            },
        )
    } else {
        alloc::vec![scoped(&["Components", "EventConsumerBase"], span)]
    };

    let consumer_iface = InterfaceDef {
        kind: InterfaceKind::Plain,
        name: Identifier::new(consumer_iface_name, span),
        bases: consumer_bases,
        exports: alloc::vec![Export::Op(push_op)],
        annotations: Vec::new(),
        span,
    };

    let _ = ValueKind::Concrete; // Mark unused-import safe.

    EventTypeEquivalent {
        valuetype_name: et.name.clone(),
        valuetype_bases: bases,
        consumer_interface: consumer_iface,
    }
}

// ============================================================================
// Internal helpers — Spec-by-Spec.
// ============================================================================

fn component_bases(comp: &ComponentDef, span: Span) -> Vec<ScopedName> {
    // Spec §6.3.2 — inheritance + supports.
    let mut bases = Vec::new();
    if let Some(b) = &comp.base {
        bases.push(b.clone());
    } else {
        bases.push(scoped(&["Components", "CCMObject"], span));
    }
    bases.extend(comp.supports.iter().cloned());
    bases
}

/// Spec §6.4.1 (p. 13) — `provides T name` → `T provide_name();`.
fn provide_facet_op(name: &Identifier, iface_type: &ScopedName, span: Span) -> OpDecl {
    OpDecl {
        name: Identifier::new(format!("provide_{}", name.text), span),
        oneway: false,
        context: Vec::new(),
        return_type: Some(TypeSpec::Scoped(iface_type.clone())),
        params: Vec::new(),
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    }
}

/// Spec §6.5.1 (p. 19) — `uses [multiple] T name` → connect/disconnect/
/// get_connection(s) operations.
fn uses_ops(name: &Identifier, iface_type: &ScopedName, multiple: bool, span: Span) -> Vec<Export> {
    let n = &name.text;
    let mut out = Vec::new();
    if multiple {
        // Spec §6.5.1 (p. 19-20) — multiplex.
        // Cookie connect_<name>(in <T> connection) raises (...);
        out.push(Export::Op(OpDecl {
            name: Identifier::new(format!("connect_{n}"), span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(scoped(&["Components", "Cookie"], span))),
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Scoped(iface_type.clone()),
                name: Identifier::new("connection", span),
                annotations: Vec::new(),
                span,
            }],
            raises: alloc::vec![
                scoped(&["Components", "ExceededConnectionLimit"], span),
                scoped(&["Components", "InvalidConnection"], span),
            ],
            annotations: Vec::new(),
            span,
        }));
        // <T> disconnect_<name>(in Components::Cookie ck) raises (...);
        out.push(Export::Op(OpDecl {
            name: Identifier::new(format!("disconnect_{n}"), span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(iface_type.clone())),
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Scoped(scoped(&["Components", "Cookie"], span)),
                name: Identifier::new("ck", span),
                annotations: Vec::new(),
                span,
            }],
            raises: alloc::vec![scoped(&["Components", "InvalidConnection"], span)],
            annotations: Vec::new(),
            span,
        }));
        // <name>Connections get_connections_<name>();
        out.push(Export::Op(OpDecl {
            name: Identifier::new(format!("get_connections_{n}"), span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(ScopedName::single(Identifier::new(
                format!("{n}Connections"),
                span,
            )))),
            params: Vec::new(),
            raises: Vec::new(),
            annotations: Vec::new(),
            span,
        }));
    } else {
        // Spec §6.5.1 (p. 19) — simplex.
        out.push(Export::Op(OpDecl {
            name: Identifier::new(format!("connect_{n}"), span),
            oneway: false,
            context: Vec::new(),
            return_type: None,
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Scoped(iface_type.clone()),
                name: Identifier::new("conxn", span),
                annotations: Vec::new(),
                span,
            }],
            raises: alloc::vec![
                scoped(&["Components", "AlreadyConnected"], span),
                scoped(&["Components", "InvalidConnection"], span),
            ],
            annotations: Vec::new(),
            span,
        }));
        out.push(Export::Op(OpDecl {
            name: Identifier::new(format!("disconnect_{n}"), span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(iface_type.clone())),
            params: Vec::new(),
            raises: alloc::vec![scoped(&["Components", "NoConnection"], span)],
            annotations: Vec::new(),
            span,
        }));
        out.push(Export::Op(OpDecl {
            name: Identifier::new(format!("get_connection_{n}"), span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(iface_type.clone())),
            params: Vec::new(),
            raises: Vec::new(),
            annotations: Vec::new(),
            span,
        }));
    }
    out
}

/// Spec §6.6.6.1 (p. 28) — `emits T name` → `void connect_<name>(in
/// TConsumer)` + `TConsumer disconnect_<name>()`.
fn emits_ops(name: &Identifier, event_type: &ScopedName, span: Span) -> Vec<Export> {
    let n = &name.text;
    let consumer_type = consumer_type_of(event_type, span);
    alloc::vec![
        Export::Op(OpDecl {
            name: Identifier::new(format!("connect_{n}"), span),
            oneway: false,
            context: Vec::new(),
            return_type: None,
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Scoped(consumer_type.clone()),
                name: Identifier::new("consumer", span),
                annotations: Vec::new(),
                span,
            }],
            raises: alloc::vec![scoped(&["Components", "AlreadyConnected"], span)],
            annotations: Vec::new(),
            span,
        }),
        Export::Op(OpDecl {
            name: Identifier::new(format!("disconnect_{n}"), span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(consumer_type)),
            params: Vec::new(),
            raises: alloc::vec![scoped(&["Components", "NoConnection"], span)],
            annotations: Vec::new(),
            span,
        }),
    ]
}

/// Spec §6.6.5.1 (p. 27) — `publishes T name` → `Cookie subscribe_<name>
/// (in TConsumer consumer)` + `TConsumer unsubscribe_<name>(in
/// Cookie ck)`.
fn publishes_ops(name: &Identifier, event_type: &ScopedName, span: Span) -> Vec<Export> {
    let n = &name.text;
    let consumer_type = consumer_type_of(event_type, span);
    alloc::vec![
        Export::Op(OpDecl {
            name: Identifier::new(format!("subscribe_{n}"), span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(scoped(&["Components", "Cookie"], span))),
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Scoped(consumer_type.clone()),
                name: Identifier::new("consumer", span),
                annotations: Vec::new(),
                span,
            }],
            raises: alloc::vec![scoped(&["Components", "ExceededConnectionLimit"], span)],
            annotations: Vec::new(),
            span,
        }),
        Export::Op(OpDecl {
            name: Identifier::new(format!("unsubscribe_{n}"), span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(consumer_type)),
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Scoped(scoped(&["Components", "Cookie"], span)),
                name: Identifier::new("ck", span),
                annotations: Vec::new(),
                span,
            }],
            raises: alloc::vec![scoped(&["Components", "InvalidConnection"], span)],
            annotations: Vec::new(),
            span,
        }),
    ]
}

/// Spec §6.6.7.1 (p. 29) — `consumes T name` → `TConsumer
/// get_consumer_<name>();`.
fn consumes_op(name: &Identifier, event_type: &ScopedName, span: Span) -> OpDecl {
    let n = &name.text;
    let consumer_type = consumer_type_of(event_type, span);
    OpDecl {
        name: Identifier::new(format!("get_consumer_{n}"), span),
        oneway: false,
        context: Vec::new(),
        return_type: Some(TypeSpec::Scoped(consumer_type)),
        params: Vec::new(),
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    }
}

fn ensure_consumer_interface(
    event_type: &ScopedName,
    span: Span,
    out: &mut Vec<InterfaceDef>,
    emitted: &mut Vec<String>,
) {
    let consumer_name = consumer_simple_name(event_type);
    if emitted.iter().any(|n| n == &consumer_name) {
        return;
    }
    emitted.push(consumer_name.clone());
    let push_op = OpDecl {
        name: Identifier::new(
            format!(
                "push_{}",
                event_type.parts.last().map_or("E", |i| i.text.as_str())
            ),
            span,
        ),
        oneway: false,
        context: Vec::new(),
        return_type: None,
        params: alloc::vec![ParamDecl {
            attribute: ParamAttribute::In,
            type_spec: TypeSpec::Scoped(event_type.clone()),
            name: Identifier::new(
                format!(
                    "the_{}",
                    lowercase_first(event_type.parts.last().map_or("e", |i| i.text.as_str()))
                ),
                span,
            ),
            annotations: Vec::new(),
            span,
        }],
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    };
    out.push(InterfaceDef {
        kind: InterfaceKind::Plain,
        name: Identifier::new(consumer_name, span),
        bases: alloc::vec![scoped(&["Components", "EventConsumerBase"], span)],
        exports: alloc::vec![Export::Op(push_op)],
        annotations: Vec::new(),
        span,
    });
}

fn consumer_type_of(event_type: &ScopedName, span: Span) -> ScopedName {
    let mut parts = event_type.parts.clone();
    if let Some(last) = parts.last_mut() {
        last.text.push_str("Consumer");
    } else {
        parts.push(Identifier::new("EConsumer", span));
    }
    ScopedName {
        absolute: event_type.absolute,
        parts,
        span,
    }
}

fn consumer_simple_name(event_type: &ScopedName) -> String {
    event_type.parts.last().map_or_else(
        || String::from("EConsumer"),
        |i| format!("{}Consumer", i.text),
    )
}

/// Converts the `zerodds_idl::ast::AttrDcl` (CCM component attribute with
/// 4 fields) into the full `zerodds_idl::ast::AttrDecl` with 8 fields.
fn attr_to_attr_decl(attr: &zerodds_idl::ast::AttrDcl, span: Span) -> AttrDecl {
    AttrDecl {
        name: attr.name.clone(),
        type_spec: attr.type_spec.clone(),
        readonly: attr.readonly,
        get_raises: Vec::new(),
        set_raises: Vec::new(),
        annotations: Vec::new(),
        span,
    }
}

fn build_keyless_implicit(
    home_name: &str,
    component_type: &ScopedName,
    span: Span,
) -> InterfaceDef {
    // Spec §6.7.1.1 (p. 33) — `interface <h>Implicit :
    // Components::KeylessCCMHome { <component> create() raises
    // (CreateFailure); };`.
    InterfaceDef {
        kind: InterfaceKind::Plain,
        name: Identifier::new(format!("{home_name}Implicit"), span),
        bases: alloc::vec![scoped(&["Components", "KeylessCCMHome"], span)],
        exports: alloc::vec![Export::Op(OpDecl {
            name: Identifier::new("create", span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(component_type.clone())),
            params: Vec::new(),
            raises: alloc::vec![scoped(&["Components", "CreateFailure"], span)],
            annotations: Vec::new(),
            span,
        })],
        annotations: Vec::new(),
        span,
    }
}

fn build_keyed_implicit(
    home_name: &str,
    component_type: &ScopedName,
    key_type: &ScopedName,
    span: Span,
) -> InterfaceDef {
    // Spec §6.7.1.2 (p. 34): keyed `<h>Implicit` contains create/
    // find_by_primary_key/remove/get_primary_key. No inheritance —
    // the operations are the only body entries.
    let exports = alloc::vec![
        // <comp> create(in <K> key) raises (CreateFailure, DuplicateKeyValue, InvalidKey);
        Export::Op(OpDecl {
            name: Identifier::new("create", span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(component_type.clone())),
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Scoped(key_type.clone()),
                name: Identifier::new("key", span),
                annotations: Vec::new(),
                span,
            }],
            raises: alloc::vec![
                scoped(&["Components", "CreateFailure"], span),
                scoped(&["Components", "DuplicateKeyValue"], span),
                scoped(&["Components", "InvalidKey"], span),
            ],
            annotations: Vec::new(),
            span,
        }),
        // <comp> find_by_primary_key(in <K> key) raises (FinderFailure, UnknownKeyValue, InvalidKey);
        Export::Op(OpDecl {
            name: Identifier::new("find_by_primary_key", span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(component_type.clone())),
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Scoped(key_type.clone()),
                name: Identifier::new("key", span),
                annotations: Vec::new(),
                span,
            }],
            raises: alloc::vec![
                scoped(&["Components", "FinderFailure"], span),
                scoped(&["Components", "UnknownKeyValue"], span),
                scoped(&["Components", "InvalidKey"], span),
            ],
            annotations: Vec::new(),
            span,
        }),
        // void remove(in <K> key) raises (RemoveFailure, UnknownKeyValue, InvalidKey);
        Export::Op(OpDecl {
            name: Identifier::new("remove", span),
            oneway: false,
            context: Vec::new(),
            return_type: None,
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Scoped(key_type.clone()),
                name: Identifier::new("key", span),
                annotations: Vec::new(),
                span,
            }],
            raises: alloc::vec![
                scoped(&["Components", "RemoveFailure"], span),
                scoped(&["Components", "UnknownKeyValue"], span),
                scoped(&["Components", "InvalidKey"], span),
            ],
            annotations: Vec::new(),
            span,
        }),
        // <K> get_primary_key(in <comp> comp);
        Export::Op(OpDecl {
            name: Identifier::new("get_primary_key", span),
            oneway: false,
            context: Vec::new(),
            return_type: Some(TypeSpec::Scoped(key_type.clone())),
            params: alloc::vec![ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: TypeSpec::Scoped(component_type.clone()),
                name: Identifier::new("comp", span),
                annotations: Vec::new(),
                span,
            }],
            raises: Vec::new(),
            annotations: Vec::new(),
            span,
        }),
    ];

    InterfaceDef {
        kind: InterfaceKind::Plain,
        name: Identifier::new(format!("{home_name}Implicit"), span),
        // Spec §6.7.1.2 (p. 34): no direct inheritance; operations are
        // the complete members.
        bases: Vec::new(),
        exports,
        annotations: Vec::new(),
        span,
    }
}

fn base_explicit_name(base: &ScopedName, span: Span) -> ScopedName {
    let last_text = base
        .parts
        .last()
        .map_or_else(|| String::from("BaseHome"), |i| i.text.clone());
    ScopedName {
        absolute: base.absolute,
        parts: alloc::vec![Identifier::new(format!("{last_text}Explicit"), span)],
        span,
    }
}

fn scoped(parts: &[&str], span: Span) -> ScopedName {
    ScopedName {
        absolute: false,
        parts: parts
            .iter()
            .map(|p| Identifier::new((*p).to_string(), span))
            .collect(),
        span,
    }
}

/// Public re-export for `validate.rs` (Phase-B-Cluster-9).
#[must_use]
pub fn scoped_name(parts: &[&str], span: Span) -> ScopedName {
    scoped(parts, span)
}

fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |c| {
        let mut out: String = c.to_lowercase().collect();
        out.push_str(chars.as_str());
        out
    })
}

// Mark unused-import safe (StringType, PrimitiveType used only in tests).
const _: fn() = || {
    let _ = core::marker::PhantomData::<StringType>;
    let _ = core::marker::PhantomData::<PrimitiveType>;
};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_idl::ast::{ComponentDef, ComponentExport, IntegerType};

    fn span() -> Span {
        Span::SYNTHETIC
    }

    fn ident(s: &str) -> Identifier {
        Identifier::new(s, span())
    }

    fn sn(parts: &[&str]) -> ScopedName {
        scoped(parts, span())
    }

    fn comp(name: &str, body: Vec<ComponentExport>) -> ComponentDef {
        ComponentDef {
            name: ident(name),
            base: None,
            supports: Vec::new(),
            body,
            annotations: Vec::new(),
            span: span(),
        }
    }

    fn comp_with_supports(
        name: &str,
        supports: Vec<ScopedName>,
        body: Vec<ComponentExport>,
    ) -> ComponentDef {
        ComponentDef {
            name: ident(name),
            base: None,
            supports,
            body,
            annotations: Vec::new(),
            span: span(),
        }
    }

    fn comp_with_base(name: &str, base: ScopedName, body: Vec<ComponentExport>) -> ComponentDef {
        ComponentDef {
            name: ident(name),
            base: Some(base),
            supports: Vec::new(),
            body,
            annotations: Vec::new(),
            span: span(),
        }
    }

    fn op_names(iface: &InterfaceDef) -> Vec<String> {
        iface
            .exports
            .iter()
            .filter_map(|e| match e {
                Export::Op(o) => Some(o.name.text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn simple_basic_component_inherits_ccmobject() {
        // Spec §6.3.2.1 (p. 12) — `component C {};` →
        // `interface C : Components::CCMObject {};`.
        let c = comp("C", alloc::vec![]);
        let out = transform_component(&c);
        assert_eq!(out.equivalent_interface.name.text, "C");
        assert_eq!(out.equivalent_interface.bases.len(), 1);
        let parts: Vec<&str> = out.equivalent_interface.bases[0]
            .parts
            .iter()
            .map(|p| p.text.as_str())
            .collect();
        assert_eq!(parts, alloc::vec!["Components", "CCMObject"]);
    }

    #[test]
    fn component_with_supports_inherits_ccmobject_plus_supported() {
        // Spec §6.3.2.2 (p. 12) — `component C supports I1, I2 { };` →
        // `interface C : Components::CCMObject, I1, I2 {};`.
        let c = comp_with_supports("C", alloc::vec![sn(&["I1"]), sn(&["I2"])], alloc::vec![]);
        let out = transform_component(&c);
        let names: Vec<String> = out
            .equivalent_interface
            .bases
            .iter()
            .map(|b| b.parts.last().map_or(String::new(), |i| i.text.clone()))
            .collect();
        assert_eq!(names, alloc::vec!["CCMObject", "I1", "I2"]);
    }

    #[test]
    fn component_with_base_inherits_base_not_ccmobject() {
        // Spec §6.3.2.3 (p. 12) — `component C : B { };` →
        // `interface C : B { ... }` (CCMObject is transitive via B).
        let c = comp_with_base("C", sn(&["B"]), alloc::vec![]);
        let out = transform_component(&c);
        let parts: Vec<String> = out.equivalent_interface.bases[0]
            .parts
            .iter()
            .map(|p| p.text.clone())
            .collect();
        assert_eq!(parts, alloc::vec!["B"]);
    }

    #[test]
    fn provides_decl_yields_provide_underscore_name_op() {
        // Spec §6.4.1 (p. 13) — `provides I foo;` → `I provide_foo();`.
        let c = comp(
            "C",
            alloc::vec![ComponentExport::Provides {
                type_spec: sn(&["M", "I"]),
                name: ident("foo"),
                span: span(),
            }],
        );
        let out = transform_component(&c);
        let names = op_names(&out.equivalent_interface);
        assert!(names.contains(&String::from("provide_foo")));
    }

    #[test]
    fn uses_simplex_yields_three_ops_with_correct_signatures() {
        // Spec §6.5.1 (p. 19).
        let c = comp(
            "C",
            alloc::vec![ComponentExport::Uses {
                type_spec: sn(&["I"]),
                name: ident("manager"),
                multiple: false,
                span: span(),
            }],
        );
        let out = transform_component(&c);
        let names = op_names(&out.equivalent_interface);
        assert!(names.contains(&String::from("connect_manager")));
        assert!(names.contains(&String::from("disconnect_manager")));
        assert!(names.contains(&String::from("get_connection_manager")));
    }

    #[test]
    fn uses_multiple_yields_get_connections_plural_op() {
        // Spec §6.5.1 (p. 19-20) — multiplex receptacle.
        let c = comp(
            "C",
            alloc::vec![ComponentExport::Uses {
                type_spec: sn(&["I"]),
                name: ident("managers"),
                multiple: true,
                span: span(),
            }],
        );
        let out = transform_component(&c);
        let names = op_names(&out.equivalent_interface);
        assert!(names.contains(&String::from("connect_managers")));
        assert!(names.contains(&String::from("disconnect_managers")));
        assert!(names.contains(&String::from("get_connections_managers")));
        // Connect returns a Cookie (Spec §6.5.1 p. 20 multiplex).
        let connect = out
            .equivalent_interface
            .exports
            .iter()
            .find_map(|e| match e {
                Export::Op(o) if o.name.text == "connect_managers" => Some(o),
                _ => None,
            })
            .expect("connect");
        let TypeSpec::Scoped(ret) = connect.return_type.as_ref().expect("return") else {
            panic!()
        };
        assert_eq!(
            ret.parts
                .iter()
                .map(|i| i.text.as_str())
                .collect::<Vec<_>>(),
            alloc::vec!["Components", "Cookie"]
        );
    }

    #[test]
    fn emits_decl_yields_connect_disconnect_with_consumer() {
        // Spec §6.6.6.1 (p. 28).
        let c = comp(
            "C",
            alloc::vec![ComponentExport::Emits {
                type_spec: sn(&["Tick"]),
                name: ident("ticker"),
                span: span(),
            }],
        );
        let out = transform_component(&c);
        let names = op_names(&out.equivalent_interface);
        assert!(names.contains(&String::from("connect_ticker")));
        assert!(names.contains(&String::from("disconnect_ticker")));
        // The TickConsumer iface was created implicitly.
        assert_eq!(out.event_consumer_interfaces.len(), 1);
        assert_eq!(out.event_consumer_interfaces[0].name.text, "TickConsumer");
    }

    #[test]
    fn publishes_decl_yields_subscribe_unsubscribe_with_cookie() {
        // Spec §6.6.5.1 (p. 27).
        let c = comp(
            "C",
            alloc::vec![ComponentExport::Publishes {
                type_spec: sn(&["Tick"]),
                name: ident("ticker"),
                span: span(),
            }],
        );
        let out = transform_component(&c);
        let names = op_names(&out.equivalent_interface);
        assert!(names.contains(&String::from("subscribe_ticker")));
        assert!(names.contains(&String::from("unsubscribe_ticker")));
        // subscribe returns a Cookie (Spec §6.6.5.1).
        let sub = out
            .equivalent_interface
            .exports
            .iter()
            .find_map(|e| match e {
                Export::Op(o) if o.name.text == "subscribe_ticker" => Some(o),
                _ => None,
            })
            .expect("subscribe");
        let TypeSpec::Scoped(ret) = sub.return_type.as_ref().expect("return") else {
            panic!()
        };
        assert_eq!(
            ret.parts
                .iter()
                .map(|i| i.text.as_str())
                .collect::<Vec<_>>(),
            alloc::vec!["Components", "Cookie"]
        );
    }

    #[test]
    fn consumes_decl_yields_get_consumer_op() {
        // Spec §6.6.7.1 (p. 29) — `consumes Tick sink;` →
        // `TickConsumer get_consumer_sink();`.
        let c = comp(
            "C",
            alloc::vec![ComponentExport::Consumes {
                type_spec: sn(&["Tick"]),
                name: ident("sink"),
                span: span(),
            }],
        );
        let out = transform_component(&c);
        let names = op_names(&out.equivalent_interface);
        assert!(names.contains(&String::from("get_consumer_sink")));
        // Return type is TickConsumer.
        let op = out
            .equivalent_interface
            .exports
            .iter()
            .find_map(|e| match e {
                Export::Op(o) if o.name.text == "get_consumer_sink" => Some(o),
                _ => None,
            })
            .expect("op");
        let TypeSpec::Scoped(ret) = op.return_type.as_ref().expect("return") else {
            panic!()
        };
        assert_eq!(
            ret.parts.last().expect("last").text.as_str(),
            "TickConsumer"
        );
    }

    #[test]
    fn duplicate_event_type_yields_only_one_consumer_interface() {
        // Spec §6.6 — one consumer iface per EventType, not per port.
        let c = comp(
            "C",
            alloc::vec![
                ComponentExport::Emits {
                    type_spec: sn(&["Tick"]),
                    name: ident("a"),
                    span: span(),
                },
                ComponentExport::Publishes {
                    type_spec: sn(&["Tick"]),
                    name: ident("b"),
                    span: span(),
                },
                ComponentExport::Consumes {
                    type_spec: sn(&["Tick"]),
                    name: ident("c"),
                    span: span(),
                },
            ],
        );
        let out = transform_component(&c);
        assert_eq!(out.event_consumer_interfaces.len(), 1);
    }

    #[test]
    fn attribute_is_propagated_to_equivalent_interface() {
        let c = comp(
            "C",
            alloc::vec![ComponentExport::Attribute(zerodds_idl::ast::AttrDcl {
                readonly: false,
                type_spec: TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::Long)),
                name: ident("rate"),
                span: span(),
            })],
        );
        let out = transform_component(&c);
        let attr = out
            .equivalent_interface
            .exports
            .iter()
            .find_map(|e| match e {
                Export::Attr(a) => Some(a),
                _ => None,
            })
            .expect("attr");
        assert_eq!(attr.name.text, "rate");
        assert!(!attr.readonly);
    }

    #[test]
    fn home_without_primary_key_yields_keyless_implicit() {
        // Spec §6.7.1.1 (p. 33).
        let h = HomeDef {
            name: ident("CManager"),
            base: None,
            supports: Vec::new(),
            manages: sn(&["C"]),
            primary_key: None,
            annotations: Vec::new(),
            span: span(),
        };
        let out = transform_home(&h);
        assert_eq!(out.explicit.name.text, "CManagerExplicit");
        assert_eq!(out.implicit.name.text, "CManagerImplicit");
        assert_eq!(out.equivalent.name.text, "CManager");
        // Explicit inherits CCMHome.
        let parts = out.explicit.bases[0]
            .parts
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(parts, alloc::vec!["Components", "CCMHome"]);
        // Implicit inherits KeylessCCMHome.
        let parts = out.implicit.bases[0]
            .parts
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(parts, alloc::vec!["Components", "KeylessCCMHome"]);
        // Implicit has a create() op.
        let names = op_names(&out.implicit);
        assert_eq!(names, alloc::vec!["create"]);
    }

    #[test]
    fn home_with_primary_key_yields_keyed_implicit_with_four_ops() {
        // Spec §6.7.1.2 (p. 34).
        let h = HomeDef {
            name: ident("CManager"),
            base: None,
            supports: Vec::new(),
            manages: sn(&["C"]),
            primary_key: Some(sn(&["CKey"])),
            annotations: Vec::new(),
            span: span(),
        };
        let out = transform_home(&h);
        let names = op_names(&out.implicit);
        for expected in ["create", "find_by_primary_key", "remove", "get_primary_key"] {
            assert!(
                names.contains(&String::from(expected)),
                "missing {expected} in {names:?}"
            );
        }
        // Keyed-Implicit has NO inheritance (only body ops).
        assert!(out.implicit.bases.is_empty());
    }

    #[test]
    fn derived_home_inherits_base_explicit() {
        // Spec §6.7.4 (p. 37): "Each Explicit Home, derived from another
        // home, MUST inherit from the parent's Explicit Interface".
        // dds-ts-1.0-beta1 cross-ref: omg-ccm-4.0 §6.7.4.
        let h = HomeDef {
            name: ident("CManagerExt"),
            base: Some(sn(&["CManagerBase"])),
            supports: Vec::new(),
            manages: sn(&["C"]),
            primary_key: None,
            annotations: Vec::new(),
            span: span(),
        };
        let out = transform_home(&h);
        // The Explicit iface of CManagerExt inherits from
        // CManagerBaseExplicit, NOT from Components::CCMHome (the
        // inheritance root is already anchored in the base Explicit).
        assert_eq!(out.explicit.name.text, "CManagerExtExplicit");
        assert_eq!(
            out.explicit.bases.len(),
            1,
            "derived home explicit must inherit exactly the base's Explicit"
        );
        let parts = out.explicit.bases[0]
            .parts
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            parts,
            alloc::vec!["CManagerBaseExplicit"],
            "expected CManagerBaseExplicit, got {parts:?}"
        );
    }

    #[test]
    fn derived_home_with_supports_extends_base_explicit() {
        // Spec §6.7.4: derived home MAY add supports clauses on top
        // of the base-explicit inheritance.
        let h = HomeDef {
            name: ident("CManagerExt"),
            base: Some(sn(&["CManagerBase"])),
            supports: alloc::vec![sn(&["IExtraIface"])],
            manages: sn(&["C"]),
            primary_key: None,
            annotations: Vec::new(),
            span: span(),
        };
        let out = transform_home(&h);
        let names: Vec<String> = out
            .explicit
            .bases
            .iter()
            .map(|b| b.parts.last().map_or(String::new(), |i| i.text.clone()))
            .collect();
        assert_eq!(
            names,
            alloc::vec![
                String::from("CManagerBaseExplicit"),
                String::from("IExtraIface")
            ]
        );
    }

    #[test]
    fn equivalent_home_inherits_explicit_and_implicit() {
        let h = HomeDef {
            name: ident("CManager"),
            base: None,
            supports: Vec::new(),
            manages: sn(&["C"]),
            primary_key: None,
            annotations: Vec::new(),
            span: span(),
        };
        let out = transform_home(&h);
        let names: Vec<String> = out
            .equivalent
            .bases
            .iter()
            .map(|b| b.parts.last().map_or(String::new(), |i| i.text.clone()))
            .collect();
        assert_eq!(names, alloc::vec!["CManagerExplicit", "CManagerImplicit"]);
    }

    #[test]
    fn event_type_first_in_chain_inherits_event_base() {
        // Spec §6.6.1.1 (p. 25).
        let et = EventDef {
            name: ident("Tick"),
            kind: ValueKind::Concrete,
            inheritance: None,
            elements: Vec::new(),
            annotations: Vec::new(),
            span: span(),
        };
        let out = transform_event_type(&et);
        // valuetype Tick : Components::EventBase.
        let parts = out.valuetype_bases[0]
            .parts
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(parts, alloc::vec!["Components", "EventBase"]);
        // TickConsumer : Components::EventConsumerBase.
        assert_eq!(out.consumer_interface.name.text, "TickConsumer");
        let cb = out.consumer_interface.bases[0]
            .parts
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(cb, alloc::vec!["Components", "EventConsumerBase"]);
        // push_Tick(in Tick the_tick).
        let push = out.consumer_interface.exports.iter().find_map(|e| match e {
            Export::Op(o) => Some(o),
            _ => None,
        });
        let push = push.expect("push op");
        assert_eq!(push.name.text, "push_Tick");
        assert_eq!(push.params[0].name.text, "the_tick");
    }

    #[test]
    fn full_stockmanager_component_yields_all_expected_ops() {
        // End-to-end: component with all port kinds.
        let c = comp(
            "Trader",
            alloc::vec![
                ComponentExport::Provides {
                    type_spec: sn(&["StockManager"]),
                    name: ident("manager"),
                    span: span(),
                },
                ComponentExport::Uses {
                    type_spec: sn(&["Bank"]),
                    name: ident("bank"),
                    multiple: false,
                    span: span(),
                },
                ComponentExport::Uses {
                    type_spec: sn(&["Feed"]),
                    name: ident("feeds"),
                    multiple: true,
                    span: span(),
                },
                ComponentExport::Emits {
                    type_spec: sn(&["Tick"]),
                    name: ident("ticker"),
                    span: span(),
                },
                ComponentExport::Publishes {
                    type_spec: sn(&["Tick"]),
                    name: ident("public_ticker"),
                    span: span(),
                },
                ComponentExport::Consumes {
                    type_spec: sn(&["Order"]),
                    name: ident("order_sink"),
                    span: span(),
                },
            ],
        );
        let out = transform_component(&c);
        let names = op_names(&out.equivalent_interface);
        for expected in [
            "provide_manager",
            "connect_bank",
            "disconnect_bank",
            "get_connection_bank",
            "connect_feeds",
            "disconnect_feeds",
            "get_connections_feeds",
            "connect_ticker",
            "disconnect_ticker",
            "subscribe_public_ticker",
            "unsubscribe_public_ticker",
            "get_consumer_order_sink",
        ] {
            assert!(
                names.contains(&String::from(expected)),
                "missing {expected} in {names:?}"
            );
        }
        // Tick + Order Consumers (2).
        let consumer_names: Vec<String> = out
            .event_consumer_interfaces
            .iter()
            .map(|i| i.name.text.clone())
            .collect();
        assert!(consumer_names.contains(&String::from("TickConsumer")));
        assert!(consumer_names.contains(&String::from("OrderConsumer")));
    }
}
