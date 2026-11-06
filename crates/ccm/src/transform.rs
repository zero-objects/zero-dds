// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Equivalent-IDL transformation — Spec §6.3.2 / §6.4.1 / §6.5.1 /
//! §6.6.x / §6.7.1.
//!
//! Input and output are the same model. Spec §6.2 (p. 11): "A component
//! definition in IDL implicitly defines an interface that supports the
//! features defined in the component definition body." In the common AST
//! a component and an interface are one node kind that differ in
//! [`InterfaceKind`] and in the CORBA facet, so the equivalent interface
//! *is* the component declaration with its kind changed, its bases
//! rewritten per §6.3.2 and its ports turned into the operations §6.4.1
//! to §6.6.x name. The transformation rewrites it in place and returns
//! the handles; a caller that needs the component declaration afterwards
//! snapshots the model first.
//!
//! Every name the specification introduces — `Components::CCMObject`,
//! `Components::Cookie`, `Components::CreateFailure` — enters as a
//! pending reference on its scoped name. The model is built for partial
//! knowledge: whether a `Components.idl` has been parsed into the same
//! model is the caller's business, and a later resolution pass wires
//! them up if it has.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{Ast, AstError, ComponentRole, InterfaceKind, InterfaceNode, TypeKind, TypeNode};
use zerodds_ast_core::{Id, PendingRef, Ref, ScopedName};

/// Why a transformation could not be carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformError {
    /// The handle names no live node, or the model rejected an edit.
    Model(AstError),
    /// The declaration is not the form the transformation applies to.
    NotA {
        /// What the transformation needed.
        expected: &'static str,
        /// What it was given.
        found: &'static str,
    },
    /// A `home` carried no `manages` clause, which IDL 4.2 §7.4.9
    /// requires and CCM 4.0 §6.7.1 maps.
    HomeManagesNothing(String),
}

impl From<AstError> for TransformError {
    fn from(error: AstError) -> Self {
        Self::Model(error)
    }
}

impl core::fmt::Display for TransformError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Model(error) => write!(f, "the model rejected the transformation: {error}"),
            Self::NotA { expected, found } => {
                write!(f, "expected {expected}, found {found}")
            }
            Self::HomeManagesNothing(name) => {
                write!(f, "home `{name}` has no `manages` clause")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TransformError {}

/// Result of the component transformation: the equivalent interface
/// (Spec §6.3.2) plus every implied event-consumer interface (Spec
/// §6.6.6.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEquivalent {
    /// The component equivalent interface (Spec §6.3.2). The same node
    /// the component was, with its kind and body rewritten.
    pub equivalent_interface: Id<InterfaceNode>,
    /// Implied `<event_type>Consumer` interfaces for the
    /// `consumes`/`emits`/`publishes` ports (Spec §6.6.6.2).
    pub event_consumer_interfaces: Vec<Id<InterfaceNode>>,
}

/// Result of the home transformation: the three interfaces from Spec
/// §6.7.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeEquivalent {
    /// `<home_name>Explicit : Components::CCMHome [, supported]`.
    pub explicit: Id<InterfaceNode>,
    /// `<home_name>Implicit : Components::KeylessCCMHome`, or the keyed
    /// variant without inheritance.
    pub implicit: Id<InterfaceNode>,
    /// `<home_name> : <home_name>Explicit, <home_name>Implicit { }`. The
    /// same node the home was.
    pub equivalent: Id<InterfaceNode>,
}

/// Result of the event-type transformation — Spec §6.6.1.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventTypeEquivalent {
    /// The equivalent valuetype. The same node the eventtype was, with
    /// `Components::EventBase` added to its bases when it is the first
    /// of its chain.
    pub valuetype: Id<TypeNode>,
    /// `<event_type>Consumer` interface (Spec §6.6.1.1).
    pub consumer_interface: Id<InterfaceNode>,
}

/// A pending reference to a name the specification introduces.
///
/// `Components::CCMObject` and the rest are declared in the
/// specification's own IDL, which may or may not be in this model. A
/// pending reference says "this names `::Components::CCMObject`" without
/// claiming to have found it.
fn spec_ref<T>(parts: &[&str]) -> Ref<T> {
    Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(
        parts.iter().map(|part| (*part).to_string()).collect(),
    )))
}

/// A pending reference to a name in the model's own root scope.
fn named_ref<T>(name: &str) -> Ref<T> {
    Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(alloc::vec![
        name.to_string()
    ])))
}

/// A reference to `name` declared in `module`: the module's scoped name
/// with `name` appended. The equivalent declarations are made in the
/// module of the component, home or event type they derive from, and a
/// reference anchored at the root reached them only when that module was
/// the root.
fn sibling_ref<T>(ast: &Ast, module: Id<zerodds_ast::ModuleNode>, name: &str) -> Ref<T> {
    let mut segments: Vec<String> = module_path(ast, module);
    segments.push(name.to_string());
    Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(segments)))
}

/// The scoped name of a module, outermost segment first.
fn module_path(ast: &Ast, module: Id<zerodds_ast::ModuleNode>) -> Vec<String> {
    ast.module(module)
        .ok()
        .and_then(|node| ast.scoped_name_of(zerodds_ast::AnyId::Module(module, node.node_id)))
        .map(|name| {
            name.segments()
                .iter()
                .filter(|segment| !segment.is_empty())
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// A reference to the declaration named like the one `reference` names,
/// with `suffix` appended to its last segment and in the same scope:
/// `<base home>Explicit`, `<event type>Consumer`.
fn suffixed_ref<T, U>(
    ast: &Ast,
    reference: &Ref<U>,
    module_of: impl Fn(&Ast, Id<U>) -> Option<Id<zerodds_ast::ModuleNode>>,
    name_of: impl Fn(&Ast, Id<U>) -> Option<String>,
    suffix: &str,
) -> Ref<T> {
    match reference {
        Ref::Resolved(id) => match (module_of(ast, *id), name_of(ast, *id)) {
            (Some(module), Some(name)) => sibling_ref(ast, module, &format!("{name}{suffix}")),
            _ => named_ref(suffix),
        },
        Ref::Pending(PendingRef::ScopedName(name)) => {
            let mut segments = name.segments().to_vec();
            if let Some(last) = segments.last_mut() {
                last.push_str(suffix);
            }
            let renamed = if name.is_absolute() {
                ScopedName::absolute(segments)
            } else {
                ScopedName::relative(segments)
            };
            Ref::Pending(PendingRef::ScopedName(renamed))
        }
        Ref::Pending(key) => Ref::Pending(key.clone()),
        Ref::Broken(cause) => Ref::Broken(cause.clone()),
    }
}

/// `<base home>Explicit`.
fn explicit_of(ast: &Ast, base: &Ref<InterfaceNode>) -> Ref<InterfaceNode> {
    suffixed_ref(
        ast,
        base,
        |ast, id| ast.interface(id).ok().map(|node| node.module),
        |ast, id| ast.interface(id).ok().map(|node| node.name.clone()),
        "Explicit",
    )
}

/// `<event type>Consumer`.
fn consumer_of<T>(ast: &Ast, event: &Ref<TypeNode>) -> Ref<T> {
    suffixed_ref(
        ast,
        event,
        |ast, id| ast.type_(id).ok().map(|node| node.module),
        |ast, id| ast.type_(id).ok().map(|node| node.name.clone()),
        "Consumer",
    )
}

/// Spec §6.3.2 (p. 12) + §6.4.1 / §6.5.1 / §6.6.x — rewrites a component
/// into its equivalent interface, ports and all.
///
/// **Inheritance (§6.3.2):**
/// * Without `supports` and without a base: `: Components::CCMObject`.
/// * With a base: `: <base>` — `CCMObject` is transitive through it.
/// * With `supports`: the supported interfaces on top of either.
///
/// **Body (§6.4.1 / §6.5.1 / §6.6.x):** every port becomes operations on
/// the equivalent interface.
///
/// # Errors
/// [`TransformError::NotA`] when the handle does not name a `component`,
/// [`TransformError::Model`] when the model rejects an edit.
pub fn transform_component(
    ast: &mut Ast,
    component: Id<InterfaceNode>,
) -> Result<ComponentEquivalent, TransformError> {
    let node = ast.interface(component)?;
    if node.kind != InterfaceKind::Component {
        return Err(TransformError::NotA {
            expected: "a component",
            found: node.kind.as_str(),
        });
    }
    // An extended port is the basic ports of its port type (IDL 4.2
    // §7.4.11.4.1); those are what §6.4.1 and §6.5.1 map.
    expand_extended_ports(ast, component)?;
    let ports: Vec<ComponentRole> = ast
        .interface(component)?
        .attributes
        .clone()
        .into_iter()
        .filter_map(|attribute| {
            ast.attribute(attribute)
                .ok()?
                .facets
                .corba
                .as_ref()?
                .component_role
                .clone()
        })
        .collect();

    // §6.3.2 — the base list, then the supported interfaces on top.
    let declared_bases = ast.interface(component)?.bases.clone();
    let supported = ast
        .interface(component)?
        .facets
        .corba
        .as_ref()
        .map(|corba| corba.supports.clone())
        .unwrap_or_default();
    let mut bases = if declared_bases.is_empty() {
        alloc::vec![spec_ref(&["Components", "CCMObject"])]
    } else {
        declared_bases
    };
    bases.extend(supported);

    ast.set_interface_form(component, InterfaceKind::Interface, bases)?;
    // The ports are gone once they are operations: leaving them in place
    // would put each one into the equivalent interface twice, once as
    // the attribute a port never was and once as the operations §6.4.1
    // names.
    for attribute in ast.interface(component)?.attributes.clone() {
        let is_port = ast
            .attribute(attribute)
            .ok()
            .and_then(|node| node.facets.corba.as_ref())
            .is_some_and(|corba| corba.component_role.is_some());
        if is_port {
            let node_id = ast.attribute(attribute)?.node_id;
            ast.remove(zerodds_ast::AnyId::Attribute(attribute, node_id))?;
        }
    }

    let module = ast.interface(component)?.module;
    let mut consumers = Vec::new();
    let mut emitted: Vec<String> = Vec::new();

    for port in ports {
        match port {
            ComponentRole::Facet { name, interface } => {
                // §6.4.1 (p. 13) — `provides T name` → `T provide_name();`
                let returns = object_reference(ast, &interface)?;
                let _ =
                    ast.add_operation(component, &format!("provide_{name}"), Some(returns), false)?;
            }
            ComponentRole::Receptacle {
                name,
                interface,
                multiple,
            } => {
                uses_ops(ast, component, &name, &interface, multiple)?;
            }
            ComponentRole::EventSource {
                name,
                event,
                publisher,
            } => {
                let consumer = ensure_consumer_interface(ast, module, &event, &mut emitted)?;
                if let Some(consumer) = consumer {
                    consumers.push(consumer);
                }
                if publisher {
                    publishes_ops(ast, component, &name, &event)?;
                } else {
                    emits_ops(ast, component, &name, &event)?;
                }
            }
            ComponentRole::EventSink { name, event } => {
                let consumer = ensure_consumer_interface(ast, module, &event, &mut emitted)?;
                if let Some(consumer) = consumer {
                    consumers.push(consumer);
                }
                // §6.6.7.1 (p. 29) — `consumes T name` →
                // `TConsumer get_consumer_name();`
                let returns = consumer_type_ref(ast, &event)?;
                let _ = ast.add_operation(
                    component,
                    &format!("get_consumer_{name}"),
                    Some(returns),
                    false,
                )?;
            }
            // A `port` or `mirrorport` was replaced by its basic ports
            // above (`expand_extended_ports`).
            ComponentRole::PortTypeRole
            | ComponentRole::ConnectorRole
            | ComponentRole::HomeFactory
            | ComponentRole::HomeFinder => {}
        }
    }

    Ok(ComponentEquivalent {
        equivalent_interface: component,
        event_consumer_interfaces: consumers,
    })
}

/// Spec §6.5.1 (p. 19) — `uses [multiple] T name` → connect / disconnect
/// / get_connection(s).
fn uses_ops(
    ast: &mut Ast,
    interface: Id<InterfaceNode>,
    name: &str,
    target: &Ref<InterfaceNode>,
    multiple: bool,
) -> Result<(), TransformError> {
    if multiple {
        // §6.5.1 (p. 19-20) — multiplex.
        let connect = ast.add_operation(
            interface,
            &format!("connect_{name}"),
            Some(spec_ref(&["Components", "Cookie"])),
            false,
        )?;
        let connection = object_reference(ast, target)?;
        let _ = ast.add_param(
            connect,
            "connection",
            zerodds_ast::ParamDirection::In,
            connection,
        )?;
        ast.add_raises(
            connect,
            spec_ref(&["Components", "ExceededConnectionLimit"]),
        )?;
        ast.add_raises(connect, spec_ref(&["Components", "InvalidConnection"]))?;

        let returns = object_reference(ast, target)?;
        let disconnect = ast.add_operation(
            interface,
            &format!("disconnect_{name}"),
            Some(returns),
            false,
        )?;
        let _ = ast.add_param(
            disconnect,
            "ck",
            zerodds_ast::ParamDirection::In,
            spec_ref(&["Components", "Cookie"]),
        )?;
        ast.add_raises(disconnect, spec_ref(&["Components", "InvalidConnection"]))?;

        // §6.5.1 (p. 19) declares the two types the operation returns,
        // `struct <name>Connection { <interface_type> objref;
        // Components::Cookie ck; }` and the sequence of them
        // `<name>Connections`, beside the operations: in the scope of the
        // equivalent interface.
        let connections = connection_types(ast, interface, name, target)?;
        let _ = ast.add_operation(
            interface,
            &format!("get_connections_{name}"),
            Some(connections),
            false,
        )?;
        return Ok(());
    }

    // §6.5.1 (p. 19) — simplex.
    let connect = ast.add_operation(interface, &format!("connect_{name}"), None, false)?;
    let connection = object_reference(ast, target)?;
    let _ = ast.add_param(
        connect,
        "conxn",
        zerodds_ast::ParamDirection::In,
        connection,
    )?;
    ast.add_raises(connect, spec_ref(&["Components", "AlreadyConnected"]))?;
    ast.add_raises(connect, spec_ref(&["Components", "InvalidConnection"]))?;

    let returns = object_reference(ast, target)?;
    let disconnect = ast.add_operation(
        interface,
        &format!("disconnect_{name}"),
        Some(returns),
        false,
    )?;
    ast.add_raises(disconnect, spec_ref(&["Components", "NoConnection"]))?;

    let returns = object_reference(ast, target)?;
    let _ = ast.add_operation(
        interface,
        &format!("get_connection_{name}"),
        Some(returns),
        false,
    )?;
    Ok(())
}

/// Spec §6.6.6.1 (p. 28) — `emits T name` → `void connect_<name>(in
/// TConsumer)` + `TConsumer disconnect_<name>()`.
fn emits_ops(
    ast: &mut Ast,
    interface: Id<InterfaceNode>,
    name: &str,
    event: &Ref<TypeNode>,
) -> Result<(), TransformError> {
    let consumer = consumer_type_ref(ast, event)?;
    let connect = ast.add_operation(interface, &format!("connect_{name}"), None, false)?;
    let _ = ast.add_param(
        connect,
        "consumer",
        zerodds_ast::ParamDirection::In,
        consumer.clone(),
    )?;
    ast.add_raises(connect, spec_ref(&["Components", "AlreadyConnected"]))?;

    let disconnect = ast.add_operation(
        interface,
        &format!("disconnect_{name}"),
        Some(consumer),
        false,
    )?;
    ast.add_raises(disconnect, spec_ref(&["Components", "NoConnection"]))?;
    Ok(())
}

/// Spec §6.6.5.1 (p. 27) — `publishes T name` → `Cookie
/// subscribe_<name>(in TConsumer)` + `TConsumer unsubscribe_<name>(in
/// Cookie)`.
fn publishes_ops(
    ast: &mut Ast,
    interface: Id<InterfaceNode>,
    name: &str,
    event: &Ref<TypeNode>,
) -> Result<(), TransformError> {
    let consumer = consumer_type_ref(ast, event)?;
    let subscribe = ast.add_operation(
        interface,
        &format!("subscribe_{name}"),
        Some(spec_ref(&["Components", "Cookie"])),
        false,
    )?;
    let _ = ast.add_param(
        subscribe,
        "consumer",
        zerodds_ast::ParamDirection::In,
        consumer.clone(),
    )?;
    ast.add_raises(
        subscribe,
        spec_ref(&["Components", "ExceededConnectionLimit"]),
    )?;

    let unsubscribe = ast.add_operation(
        interface,
        &format!("unsubscribe_{name}"),
        Some(consumer),
        false,
    )?;
    let _ = ast.add_param(
        unsubscribe,
        "ck",
        zerodds_ast::ParamDirection::In,
        spec_ref(&["Components", "Cookie"]),
    )?;
    ast.add_raises(unsubscribe, spec_ref(&["Components", "InvalidConnection"]))?;
    Ok(())
}

/// The `<event_type>Consumer` interface for an event type, created once
/// per type however many ports name it. Spec §6.6.
///
/// Returns `None` when the interface was already made for an earlier
/// port of the same component.
fn ensure_consumer_interface(
    ast: &mut Ast,
    module: Id<zerodds_ast::ModuleNode>,
    event: &Ref<TypeNode>,
    emitted: &mut Vec<String>,
) -> Result<Option<Id<InterfaceNode>>, TransformError> {
    let event_name = type_name(ast, event);
    let consumer_name = format!("{event_name}Consumer");
    if emitted.iter().any(|seen| seen == &consumer_name) {
        return Ok(None);
    }
    emitted.push(consumer_name.clone());
    // The event type's own transform declares the interface beside the
    // event type (§6.6.1.1); a second one of the same name would be a
    // second declaration of it.
    let event_module = match event {
        Ref::Resolved(id) => Some(ast.type_(*id)?.module),
        _ => None,
    };
    let declared = ast.interfaces().any(|(_, node)| {
        node.name == consumer_name
            && (node.module == module || Some(node.module) == event_module)
            && node.completeness != zerodds_ast::Completeness::Forward
    });
    if declared {
        return Ok(None);
    }
    let interface = ast.add_interface(
        module,
        &consumer_name,
        InterfaceKind::Interface,
        alloc::vec![spec_ref(&["Components", "EventConsumerBase"])],
    )?;
    let push = ast.add_operation(interface, &format!("push_{event_name}"), None, false)?;
    let _ = ast.add_param(
        push,
        &format!("the_{}", lowercase_first(&event_name)),
        zerodds_ast::ParamDirection::In,
        event.clone(),
    )?;
    Ok(Some(interface))
}

/// Spec §6.7.1 (p. 33) — rewrites a home into the three interfaces
/// Explicit, Implicit and Equivalent.
///
/// The home node becomes the equivalent interface; `<h>Explicit` and
/// `<h>Implicit` are added beside it.
///
/// # Errors
/// [`TransformError::NotA`] when the handle does not name a `home`,
/// [`TransformError::HomeManagesNothing`] when it carries no `manages`
/// clause, [`TransformError::Model`] when the model rejects an edit.
pub fn transform_home(
    ast: &mut Ast,
    home: Id<InterfaceNode>,
) -> Result<HomeEquivalent, TransformError> {
    let node = ast.interface(home)?;
    if node.kind != InterfaceKind::Home {
        return Err(TransformError::NotA {
            expected: "a home",
            found: node.kind.as_str(),
        });
    }
    let name = node.name.clone();
    let module = node.module;
    let declared_bases = node.bases.clone();
    let corba = node.facets.corba.clone().unwrap_or_default();
    let supported = corba.supports.clone();
    let Some(manages) = corba.manages.clone() else {
        return Err(TransformError::HomeManagesNothing(name));
    };

    // §6.7.1.3 (p. 35) — the Explicit interface inherits `CCMHome` and
    // the supported interfaces; §6.7.4 (p. 37) — a derived home inherits
    // the base home's Explicit instead, which already anchors `CCMHome`.
    let mut explicit_bases = if declared_bases.is_empty() {
        alloc::vec![spec_ref(&["Components", "CCMHome"])]
    } else {
        declared_bases
            .iter()
            .map(|base| explicit_of(ast, base))
            .collect()
    };
    explicit_bases.extend(supported);
    let explicit = ast.add_interface(
        module,
        &format!("{name}Explicit"),
        InterfaceKind::Interface,
        explicit_bases,
    )?;

    // §6.7.1.1 (no primary key) versus §6.7.1.2 (with one).
    let component = object_reference(ast, &manages.component)?;

    // §6.7.2: the explicit interface holds the operations and attributes
    // the home body declares; a `factory` returns the component and raises
    // `Components::CreateFailure` (§6.7.2.1), a `finder` returns it and
    // raises `Components::FinderFailure` (§6.7.2.2), each besides what it
    // raises itself.
    for operation in ast.operations_of(home).to_vec() {
        let kind = ast
            .operation(operation)?
            .facets
            .corba
            .as_ref()
            .and_then(|corba| corba.home_operation);
        match kind {
            Some(zerodds_ast::HomeOperation::Factory) => {
                ast.set_return_type(operation, Some(component.clone()))?;
                ast.add_raises(operation, spec_ref(&["Components", "CreateFailure"]))?;
            }
            Some(zerodds_ast::HomeOperation::Finder) => {
                ast.set_return_type(operation, Some(component.clone()))?;
                ast.add_raises(operation, spec_ref(&["Components", "FinderFailure"]))?;
            }
            None => {}
        }
        ast.move_operation(operation, explicit)?;
    }
    for attribute in ast.attributes_of(home).to_vec() {
        ast.move_attribute(attribute, explicit)?;
    }
    let implicit = match manages.primary_key.clone() {
        Some(key) => build_keyed_implicit(ast, module, &name, &component, &key)?,
        None => build_keyless_implicit(ast, module, &name, &component)?,
    };

    // The home itself becomes the equivalent interface.
    ast.set_interface_form(
        home,
        InterfaceKind::Interface,
        alloc::vec![
            sibling_ref(ast, module, &format!("{name}Explicit")),
            sibling_ref(ast, module, &format!("{name}Implicit")),
        ],
    )?;

    Ok(HomeEquivalent {
        explicit,
        implicit,
        equivalent: home,
    })
}

/// Spec §6.7.1.1 (p. 33) — `interface <h>Implicit :
/// Components::KeylessCCMHome { <component> create() raises
/// (CreateFailure); };`
fn build_keyless_implicit(
    ast: &mut Ast,
    module: Id<zerodds_ast::ModuleNode>,
    home: &str,
    component: &Ref<TypeNode>,
) -> Result<Id<InterfaceNode>, TransformError> {
    let implicit = ast.add_interface(
        module,
        &format!("{home}Implicit"),
        InterfaceKind::Interface,
        alloc::vec![spec_ref(&["Components", "KeylessCCMHome"])],
    )?;
    let create = ast.add_operation(implicit, "create", Some(component.clone()), false)?;
    ast.add_raises(create, spec_ref(&["Components", "CreateFailure"]))?;
    Ok(implicit)
}

/// Spec §6.7.1.2 (p. 34) — the keyed `<h>Implicit` carries create,
/// find_by_primary_key, remove and get_primary_key, and inherits
/// nothing.
fn build_keyed_implicit(
    ast: &mut Ast,
    module: Id<zerodds_ast::ModuleNode>,
    home: &str,
    component: &Ref<TypeNode>,
    key: &Ref<TypeNode>,
) -> Result<Id<InterfaceNode>, TransformError> {
    let implicit = ast.add_interface(
        module,
        &format!("{home}Implicit"),
        InterfaceKind::Interface,
        Vec::new(),
    )?;

    let create = ast.add_operation(implicit, "create", Some(component.clone()), false)?;
    let _ = ast.add_param(create, "key", zerodds_ast::ParamDirection::In, key.clone())?;
    for exception in ["CreateFailure", "DuplicateKeyValue", "InvalidKey"] {
        ast.add_raises(create, spec_ref(&["Components", exception]))?;
    }

    let find = ast.add_operation(
        implicit,
        "find_by_primary_key",
        Some(component.clone()),
        false,
    )?;
    let _ = ast.add_param(find, "key", zerodds_ast::ParamDirection::In, key.clone())?;
    for exception in ["FinderFailure", "UnknownKeyValue", "InvalidKey"] {
        ast.add_raises(find, spec_ref(&["Components", exception]))?;
    }

    let remove = ast.add_operation(implicit, "remove", None, false)?;
    let _ = ast.add_param(remove, "key", zerodds_ast::ParamDirection::In, key.clone())?;
    for exception in ["RemoveFailure", "UnknownKeyValue", "InvalidKey"] {
        ast.add_raises(remove, spec_ref(&["Components", exception]))?;
    }

    let get = ast.add_operation(implicit, "get_primary_key", Some(key.clone()), false)?;
    let _ = ast.add_param(
        get,
        "comp",
        zerodds_ast::ParamDirection::In,
        component.clone(),
    )?;
    Ok(implicit)
}

/// Spec §6.6.1.1 (p. 24) — eventtype → valuetype + consumer interface.
///
/// The eventtype node becomes the equivalent valuetype: §6.6.1.1 (p. 25)
/// says the first event type of an inheritance chain introduces the
/// inheritance from `Components::EventBase`, so a chain's root gains it
/// and a derived event type does not.
///
/// # Errors
/// [`TransformError::NotA`] when the handle does not name an eventtype,
/// [`TransformError::Model`] when the model rejects an edit.
pub fn transform_event_type(
    ast: &mut Ast,
    event: Id<TypeNode>,
) -> Result<EventTypeEquivalent, TransformError> {
    let node = ast.type_(event)?;
    let is_event = node
        .facets
        .corba
        .as_ref()
        .is_some_and(|corba| corba.is_event_type);
    let TypeKind::Valuetype { bases, .. } = &node.kind else {
        return Err(TransformError::NotA {
            expected: "an eventtype",
            found: node.kind.as_str(),
        });
    };
    if !is_event {
        return Err(TransformError::NotA {
            expected: "an eventtype",
            found: "a valuetype",
        });
    }
    let name = node.name.clone();
    let module = node.module;
    let inherited: Vec<Ref<TypeNode>> = bases.clone();

    // §6.6.1.1 (p. 25) — `Components::EventBase` only at the root of the
    // chain, and the consumer interfaces stand in the same inheritance
    // relation as the event types themselves.
    let consumer_bases = if inherited.is_empty() {
        alloc::vec![spec_ref(&["Components", "EventConsumerBase"])]
    } else {
        inherited
            .iter()
            .map(|base| consumer_of(ast, base))
            .collect()
    };
    if inherited.is_empty() {
        ast.add_valuetype_base(event, spec_ref(&["Components", "EventBase"]))?;
    }

    let consumer = ast.add_interface(
        module,
        &format!("{name}Consumer"),
        InterfaceKind::Interface,
        consumer_bases,
    )?;
    let push = ast.add_operation(consumer, &format!("push_{name}"), None, false)?;
    let _ = ast.add_param(
        push,
        &format!("the_{}", lowercase_first(&name)),
        zerodds_ast::ParamDirection::In,
        Ref::Resolved(event),
    )?;

    Ok(EventTypeEquivalent {
        valuetype: event,
        consumer_interface: consumer,
    })
}

/// The type that stands for a reference to an interface.
///
/// A resolved interface gets the object-reference type the model derives
/// from it; an unresolved one keeps its name, so the reference resolves
/// once the interface arrives.
/// `struct <name>Connection` and `<name>Connections`, the sequence of
/// it, declared in the scope of `interface` (CCM 4.0 §6.5.1, a `uses
/// multiple` receptacle).
fn connection_types(
    ast: &mut Ast,
    interface: Id<InterfaceNode>,
    name: &str,
    target: &Ref<InterfaceNode>,
) -> Result<Ref<TypeNode>, TransformError> {
    let (module, owner) = {
        let node = ast.interface(interface)?;
        (node.module, node.name.clone())
    };
    let objref = object_reference(ast, target)?;
    let connection = ast.add_type(
        module,
        &format!("{name}Connection"),
        TypeKind::Structure {
            base: None,
            members: Vec::new(),
        },
    )?;
    let _ = ast.add_member(connection, zerodds_ast::MemberSpec::new("objref", objref))?;
    let _ = ast.add_member(
        connection,
        zerodds_ast::MemberSpec::new("ck", spec_ref(&["Components", "Cookie"])),
    )?;
    let root = ast.root_module();
    let sequence = ast.add_type(
        root,
        "",
        TypeKind::Sequence {
            element: Ref::Resolved(connection),
            bound: zerodds_ast::Bound::Unbounded,
        },
    )?;
    let connections = ast.add_type(
        module,
        &format!("{name}Connections"),
        TypeKind::Alias {
            aliased: Ref::Resolved(sequence),
        },
    )?;
    for declared in [connection, connections] {
        let node_id = ast.type_(declared)?.node_id;
        ast.set_declaration_scope(zerodds_ast::AnyId::Type(declared, node_id), &owner)?;
    }
    Ok(Ref::Resolved(connections))
}

fn object_reference(
    ast: &mut Ast,
    interface: &Ref<InterfaceNode>,
) -> Result<Ref<TypeNode>, TransformError> {
    match interface {
        Ref::Resolved(id) => Ok(Ref::Resolved(ast.object_reference_to(*id)?)),
        Ref::Pending(key) => Ok(Ref::Pending(key.clone())),
        Ref::Broken(cause) => Ok(Ref::Broken(cause.clone())),
    }
}

/// A reference to the `<T>Consumer` interface of an event type, as a
/// type: the object reference to the interface when the model declares
/// it (the event type's own transform, or `ensure_consumer_interface`, put
/// it beside the event type), the name otherwise.
fn consumer_type_ref(
    ast: &mut Ast,
    event: &Ref<TypeNode>,
) -> Result<Ref<TypeNode>, TransformError> {
    if let Ref::Resolved(id) = event {
        let (module, name) = {
            let node = ast.type_(*id)?;
            (node.module, format!("{}Consumer", node.name))
        };
        let declared = ast
            .interfaces()
            .find(|(_, node)| {
                node.module == module
                    && node.name == name
                    && node.completeness != zerodds_ast::Completeness::Forward
            })
            .map(|(interface, _)| interface);
        if let Some(interface) = declared {
            return Ok(Ref::Resolved(ast.object_reference_to(interface)?));
        }
    }
    Ok(consumer_of(ast, event))
}

/// The simple name behind a type reference, resolved or not.
fn type_name(ast: &Ast, reference: &Ref<TypeNode>) -> String {
    match reference {
        Ref::Resolved(id) => ast
            .type_(*id)
            .map(|node| node.name.clone())
            .unwrap_or_default(),
        Ref::Pending(PendingRef::ScopedName(name)) => name.last().cloned().unwrap_or_default(),
        Ref::Pending(_) | Ref::Broken(_) => String::new(),
    }
}

fn lowercase_first(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or_else(String::new, |first| {
        let mut out: String = first.to_lowercase().collect();
        out.push_str(chars.as_str());
        out
    })
}

// -- extended ports and connectors, OMG IDL 4.2 §7.4.11 -----------------------

/// The port type a `port` or `mirrorport` attribute names.
fn porttype_of(ast: &Ast, port: Id<zerodds_ast::AttributeNode>) -> Option<Id<InterfaceNode>> {
    let node = ast.attribute(port).ok()?;
    match &node.type_ {
        Ref::Resolved(id) => match &ast.type_(*id).ok()?.kind {
            TypeKind::ObjectRef { interface } => interface.as_ref()?.resolved(),
            _ => None,
        },
        Ref::Pending(PendingRef::ScopedName(name)) => match ast.find_by_scoped_name(name).ok()? {
            zerodds_ast::AnyId::Interface(id, _) => Some(id),
            _ => None,
        },
        _ => None,
    }
}

/// Replace every extended port of a component or connector by the basic
/// ports of its port type: OMG IDL 4.2 §7.4.11.4.1 — "An Extended Port is
/// a grouping of basic ports (facets and/or receptacles)" — and
/// §7.4.11.4.2 — a `mirrorport` is the port "where all its facets are
/// turned into receptacles, all its receptacles turned into facets, all
/// its extended ports turned into reverse extended ports".
///
/// IDL 4.2 names no spelling for the basic ports one extended port puts on
/// a component, and the language mappings that refer to an intermediate
/// IDL for ports (IDL4 to Java 1.0 §7.12, IDL4 to C# 1.0 §7.12, IDL4 to
/// C++ 1.0 §7.12) do not either. The basic port `f` of port `p` is named
/// `p_f`, a nested extended port's ports `p_q_f`, and an attribute `a` of
/// the port type the attribute `p_a` of the component — the names are
/// unique in the component because a component's port names are, and §6
/// of CCM 4.0 then maps the basic ports to operations.
///
/// # Errors
/// [`TransformError::Model`] for a stale handle, [`TransformError::NotA`]
/// for a port whose type is no port type.
pub fn expand_extended_ports(
    ast: &mut Ast,
    owner: Id<InterfaceNode>,
) -> Result<(), TransformError> {
    let ports: Vec<(Id<zerodds_ast::AttributeNode>, bool)> = ast
        .interface(owner)?
        .attributes
        .iter()
        .filter_map(|attribute| {
            let role = ast
                .attribute(*attribute)
                .ok()?
                .facets
                .corba
                .as_ref()?
                .component_role
                .clone()?;
            match role {
                ComponentRole::PortTypeRole => Some((*attribute, false)),
                ComponentRole::ConnectorRole => Some((*attribute, true)),
                _ => None,
            }
        })
        .collect();
    for (port, mirrored) in ports {
        let name = ast.attribute(port)?.name.clone();
        let porttype = porttype_of(ast, port).ok_or(TransformError::NotA {
            expected: "a port type",
            found: "another type",
        })?;
        add_basic_ports(ast, owner, &name, porttype, mirrored, 0)?;
        let node_id = ast.attribute(port)?.node_id;
        ast.remove(zerodds_ast::AnyId::Attribute(port, node_id))?;
    }
    Ok(())
}

/// The basic ports and attributes of `porttype`, prefixed `<prefix>_`, on
/// `owner`.
///
/// zerodds-lint: recursion-depth 32 — the `depth` argument; past 32 the
/// model has a cycle of port types and it refuses.
fn add_basic_ports(
    ast: &mut Ast,
    owner: Id<InterfaceNode>,
    prefix: &str,
    porttype: Id<InterfaceNode>,
    mirrored: bool,
    depth: usize,
) -> Result<(), TransformError> {
    // Port types form no cycle (IDL 4.2 §7.4.11.4.1.1, refused by the
    // front end); the bound keeps a malformed model from looping.
    if depth > 32 {
        return Err(TransformError::NotA {
            expected: "port types without a cycle",
            found: "a cycle of port types",
        });
    }
    let members: Vec<Id<zerodds_ast::AttributeNode>> = ast.interface(porttype)?.attributes.clone();
    for member in members {
        let node = ast.attribute(member)?.clone();
        let name = format!("{prefix}_{}", node.name);
        let role = node
            .facets
            .corba
            .as_ref()
            .and_then(|c| c.component_role.clone());
        let role = match role {
            Some(ComponentRole::Facet { interface, .. }) => Some(if mirrored {
                ComponentRole::Receptacle {
                    name: name.clone(),
                    interface,
                    multiple: false,
                }
            } else {
                ComponentRole::Facet {
                    name: name.clone(),
                    interface,
                }
            }),
            Some(ComponentRole::Receptacle {
                interface,
                multiple,
                ..
            }) => Some(if mirrored {
                ComponentRole::Facet {
                    name: name.clone(),
                    interface,
                }
            } else {
                ComponentRole::Receptacle {
                    name: name.clone(),
                    interface,
                    multiple,
                }
            }),
            Some(ComponentRole::PortTypeRole) | Some(ComponentRole::ConnectorRole) => {
                let nested = porttype_of(ast, member).ok_or(TransformError::NotA {
                    expected: "a port type",
                    found: "another type",
                })?;
                let reverse = matches!(
                    node.facets
                        .corba
                        .as_ref()
                        .and_then(|c| c.component_role.clone()),
                    Some(ComponentRole::ConnectorRole)
                );
                add_basic_ports(ast, owner, &name, nested, mirrored != reverse, depth + 1)?;
                continue;
            }
            _ => None,
        };
        let added = ast.add_attribute(owner, &name, node.type_.clone(), node.readonly)?;
        let added_node = ast.attribute(added)?.node_id;
        if let Some(facets) = ast.facets_mut(zerodds_ast::AnyId::Attribute(added, added_node)) {
            facets.source = node.facets.source.clone();
            if let Some(role) = role {
                facets.corba_mut().component_role = Some(role);
            }
        }
    }
    Ok(())
}

/// A connector as the component its ports make it: its extended ports
/// replaced by their basic ports ([`expand_extended_ports`]), then the
/// equivalent interface of CCM 4.0 §6.3.2 to §6.5.1, as a component's.
///
/// # Errors
/// As [`expand_extended_ports`] and [`transform_component`].
pub fn transform_connector(
    ast: &mut Ast,
    connector: Id<InterfaceNode>,
) -> Result<ComponentEquivalent, TransformError> {
    let node = ast.interface(connector)?;
    if node.kind != InterfaceKind::Connector {
        return Err(TransformError::NotA {
            expected: "a connector",
            found: node.kind.as_str(),
        });
    }
    let bases = node.bases.clone();
    expand_extended_ports(ast, connector)?;
    ast.set_interface_form(connector, InterfaceKind::Component, bases)?;
    transform_component(ast, connector)
}
