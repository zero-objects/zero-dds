// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The executor equivalent IDL of the CCM Implementation Framework —
//! CORBA 3.3 Part 3 §9.3 (CCM 4.0 §8.3).
//!
//! "The language mapping, like the mapping for the client side, is based
//! on equivalent IDL. For components and homes, local interfaces are
//! defined. The user then implements these local interfaces using
//! existing language mapping rules." (§9.3.1)
//!
//! | Declaration | Local interfaces (§9.3.3) |
//! |---|---|
//! | `interface I` (not abstract, not local) | `CCM_I : I` |
//! | `eventtype E` | `CCM_EConsumer { void push(in E ev); }` |
//! | `component C` | `CCM_C_Executor` (main), `CCM_C` (monolithic), `CCM_C_Context` |
//! | `home H` | `CCM_HExplicit`, `CCM_HImplicit`, `CCM_H` |
//!
//! The ports add operations (§9.3.3.5): a facet `CCM_<type> get_<name>()`
//! and a sink `void push_<name>(in <type> ev)` on the monolithic executor;
//! a simplex receptacle `<type> get_connection_<name>()`, a multiplex
//! receptacle `<name>Connections get_connections_<name>()` and a source
//! `void push_<name>(in <type> ev)` on the context. The home's factories
//! and finders return `Components::EnterpriseComponent` (§9.3.3.6.6,
//! §9.3.3.6.7); the implicit executor of a keyless home has
//! `create()`, that of a keyed home — or of a keyless home derived from
//! one — `create`, `find_by_primary_key` and `remove` with the key
//! (§9.3.3.6.3, §9.3.3.6.4), all raising `Components::CCMException`.
//!
//! The local interfaces are declared beside what they derive from and
//! leave the declarations themselves as they are, so the client-side
//! transformation of [`crate::transform`] can follow. Extended ports are
//! replaced by their basic ports first (§7.2.2.2,
//! [`crate::expand_extended_ports`]).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_ast::{
    Ast, Completeness, ComponentRole, InterfaceKind, InterfaceNode, ParamDirection, TypeKind,
    TypeNode,
};
use zerodds_ast_core::{Id, PendingRef, Ref, ScopedName};

use crate::transform::{
    TransformError, expand_extended_ports, module_path, object_reference, sibling_ref, spec_ref,
};

/// The three local interfaces of a component (§9.3.3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentExecutors {
    /// `CCM_<C>_Executor`, the main executor of the locator strategy.
    pub main: Id<InterfaceNode>,
    /// `CCM_<C>`, the monolithic executor.
    pub monolithic: Id<InterfaceNode>,
    /// `CCM_<C>_Context`, the context the container implements.
    pub context: Id<InterfaceNode>,
}

/// The three local interfaces of a home (§9.3.3.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeExecutors {
    /// `CCM_<H>Explicit`.
    pub explicit: Id<InterfaceNode>,
    /// `CCM_<H>Implicit`.
    pub implicit: Id<InterfaceNode>,
    /// `CCM_<H> : CCM_<H>Explicit, CCM_<H>Implicit`.
    pub main: Id<InterfaceNode>,
}

/// Everything [`executor_idl`] declared.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutorIdl {
    /// The facet executors `CCM_<I>` (§9.3.3.1).
    pub facets: Vec<Id<InterfaceNode>>,
    /// The consumer executors `CCM_<E>Consumer` (§9.3.3.2).
    pub consumers: Vec<Id<InterfaceNode>>,
    /// The executors of each component (§9.3.3.3).
    pub components: Vec<ComponentExecutors>,
    /// The executors of each home (§9.3.3.6).
    pub homes: Vec<HomeExecutors>,
}

/// A reference to `<prefix><name><suffix>` beside what `reference`
/// names: `CCM_<base>`, `CCM_<base>_Context`, `CCM_<base home>Explicit`.
fn renamed_ref<T>(ast: &Ast, reference: &Ref<InterfaceNode>, prefix: &str, suffix: &str) -> Ref<T> {
    match reference {
        Ref::Resolved(id) => match ast.interface(*id) {
            Ok(node) => sibling_ref(ast, node.module, &format!("{prefix}{}{suffix}", node.name)),
            Err(_) => spec_ref(&[&format!("{prefix}{suffix}")]),
        },
        Ref::Pending(PendingRef::ScopedName(name)) => {
            let mut segments = name.segments().to_vec();
            if let Some(last) = segments.last_mut() {
                *last = format!("{prefix}{last}{suffix}");
            }
            Ref::Pending(PendingRef::ScopedName(if name.is_absolute() {
                ScopedName::absolute(segments)
            } else {
                ScopedName::relative(segments)
            }))
        }
        Ref::Pending(key) => Ref::Pending(key.clone()),
        Ref::Broken(cause) => Ref::Broken(cause.clone()),
    }
}

/// The object reference type of the local interface `CCM_<I>` of the
/// interface `interface` refers to: resolved when the model declares it,
/// by name otherwise.
fn facet_executor_type(
    ast: &mut Ast,
    interface: &Ref<InterfaceNode>,
) -> Result<Ref<TypeNode>, TransformError> {
    if let Ref::Resolved(id) = interface {
        let (module, name) = {
            let node = ast.interface(*id)?;
            (node.module, format!("CCM_{}", node.name))
        };
        if let Some(local) = declared_interface(ast, module, &name) {
            return Ok(Ref::Resolved(ast.object_reference_to(local)?));
        }
    }
    Ok(renamed_ref(ast, interface, "CCM_", ""))
}

fn declared_interface(
    ast: &Ast,
    module: Id<zerodds_ast::ModuleNode>,
    name: &str,
) -> Option<Id<InterfaceNode>> {
    ast.interfaces()
        .find(|(_, node)| {
            node.module == module && node.name == name && node.completeness != Completeness::Forward
        })
        .map(|(id, _)| id)
}

/// Adds a local interface unless the model already declares one of that
/// name in that module (a second call for the same declaration).
fn local_interface(
    ast: &mut Ast,
    module: Id<zerodds_ast::ModuleNode>,
    name: &str,
    bases: Vec<Ref<InterfaceNode>>,
) -> Result<Id<InterfaceNode>, TransformError> {
    if let Some(existing) = declared_interface(ast, module, name) {
        return Ok(existing);
    }
    Ok(ast.add_interface(module, name, InterfaceKind::Local, bases)?)
}

/// §9.3.3.1 — `local interface CCM_<I> : <I> { };` for a non-abstract,
/// non-local interface.
///
/// # Errors
/// [`TransformError::NotA`] for an abstract or local interface or a
/// component-model declaration, [`TransformError::Model`] for a model
/// edit refused.
pub fn facet_executor(
    ast: &mut Ast,
    interface: Id<InterfaceNode>,
) -> Result<Id<InterfaceNode>, TransformError> {
    let node = ast.interface(interface)?;
    if node.kind != InterfaceKind::Interface {
        return Err(TransformError::NotA {
            expected: "an interface that is neither abstract nor local",
            found: node.kind.as_str(),
        });
    }
    let (module, name) = (node.module, node.name.clone());
    local_interface(
        ast,
        module,
        &format!("CCM_{name}"),
        alloc::vec![Ref::Resolved(interface)],
    )
}

/// §9.3.3.2 — `local interface CCM_<E>Consumer { void push(in <E> ev); };`
///
/// # Errors
/// [`TransformError::NotA`] for a type that is no event type.
pub fn consumer_executor(
    ast: &mut Ast,
    event: Id<TypeNode>,
) -> Result<Id<InterfaceNode>, TransformError> {
    let node = ast.type_(event)?;
    let is_event = node
        .facets
        .corba
        .as_ref()
        .is_some_and(|corba| corba.is_event_type);
    if !is_event {
        return Err(TransformError::NotA {
            expected: "an eventtype",
            found: node.kind.as_str(),
        });
    }
    let (module, name) = (node.module, format!("CCM_{}Consumer", node.name));
    if let Some(existing) = declared_interface(ast, module, &name) {
        return Ok(existing);
    }
    let consumer = ast.add_interface(module, &name, InterfaceKind::Local, Vec::new())?;
    let push = ast.add_operation(consumer, "push", None, false)?;
    let _ = ast.add_param(push, "ev", ParamDirection::In, Ref::Resolved(event))?;
    Ok(consumer)
}

/// Copies the non-port attributes of `from` onto `to`, with their
/// exceptions.
fn copy_attributes(
    ast: &mut Ast,
    from: Id<InterfaceNode>,
    to: Id<InterfaceNode>,
) -> Result<(), TransformError> {
    for attribute in ast.attributes_of(from).to_vec() {
        let node = ast.attribute(attribute)?.clone();
        let is_port = node
            .facets
            .corba
            .as_ref()
            .is_some_and(|corba| corba.component_role.is_some());
        if is_port {
            continue;
        }
        let copy = ast.add_attribute(to, &node.name, node.type_.clone(), node.readonly)?;
        for raised in node.get_raises {
            ast.add_attribute_raises(copy, raised, false)?;
        }
        for raised in node.set_raises {
            ast.add_attribute_raises(copy, raised, true)?;
        }
    }
    Ok(())
}

/// A copy of operation `operation` on `to`, returning `returns` when
/// given (a factory or finder), its own return type otherwise.
fn copy_operation(
    ast: &mut Ast,
    operation: Id<zerodds_ast::OperationNode>,
    to: Id<InterfaceNode>,
    returns: Option<Option<Ref<TypeNode>>>,
) -> Result<(), TransformError> {
    let node = ast.operation(operation)?.clone();
    let return_type = returns.unwrap_or(node.return_type.clone());
    let copy = ast.add_operation(to, &node.name, return_type, node.oneway)?;
    for param in &node.params {
        let p = ast.param(*param)?.clone();
        let _ = ast.add_param(copy, &p.name, p.direction, p.type_)?;
    }
    for raised in node.raises {
        ast.add_raises(copy, raised)?;
    }
    Ok(())
}

fn enterprise_component() -> Ref<TypeNode> {
    spec_ref(&["Components", "EnterpriseComponent"])
}

/// §9.3.3.3 and §9.3.3.5 — the main executor `CCM_<C>_Executor`, the
/// monolithic executor `CCM_<C>` and the context `CCM_<C>_Context` of a
/// component, with the operations of its ports.
///
/// # Errors
/// [`TransformError::NotA`] for a declaration that is no component;
/// as [`expand_extended_ports`].
pub fn component_executors(
    ast: &mut Ast,
    component: Id<InterfaceNode>,
) -> Result<ComponentExecutors, TransformError> {
    let node = ast.interface(component)?;
    if node.kind != InterfaceKind::Component {
        return Err(TransformError::NotA {
            expected: "a component",
            found: node.kind.as_str(),
        });
    }
    expand_extended_ports(ast, component)?;
    let node = ast.interface(component)?;
    let (module, name) = (node.module, node.name.clone());
    let base = node.bases.first().cloned();
    let supported = node
        .facets
        .corba
        .as_ref()
        .map(|corba| corba.supports.clone())
        .unwrap_or_default();
    let ports: Vec<ComponentRole> = node
        .attributes
        .iter()
        .filter_map(|a| {
            ast.attribute(*a)
                .ok()?
                .facets
                .corba
                .as_ref()?
                .component_role
                .clone()
        })
        .collect();

    // Rules 3 and 4 of the main and monolithic executors, rule 2 of the
    // context.
    let bases_for = |ast: &Ast, suffix: &str, root: &[&str]| -> Vec<Ref<InterfaceNode>> {
        let mut bases = match &base {
            Some(b) => alloc::vec![renamed_ref(ast, b, "CCM_", suffix)],
            None => alloc::vec![spec_ref(root)],
        };
        bases.extend(supported.iter().cloned());
        bases
    };
    let main_bases = bases_for(ast, "_Executor", &["Components", "EnterpriseComponent"]);
    let main = local_interface(ast, module, &format!("CCM_{name}_Executor"), main_bases)?;
    copy_attributes(ast, component, main)?;

    let monolithic_bases = bases_for(ast, "", &["Components", "EnterpriseComponent"]);
    let monolithic = local_interface(ast, module, &format!("CCM_{name}"), monolithic_bases)?;
    copy_attributes(ast, component, monolithic)?;

    let context_bases = match &base {
        Some(b) => alloc::vec![renamed_ref(ast, b, "CCM_", "_Context")],
        None => alloc::vec![spec_ref(&["Components", "CCMContext"])],
    };
    let context = local_interface(ast, module, &format!("CCM_{name}_Context"), context_bases)?;

    for port in ports {
        match port {
            ComponentRole::Facet {
                name: port,
                interface,
            } => {
                let returns = facet_executor_type(ast, &interface)?;
                let _ =
                    ast.add_operation(monolithic, &format!("get_{port}"), Some(returns), false)?;
            }
            ComponentRole::EventSink { name: port, event } => {
                let push = ast.add_operation(monolithic, &format!("push_{port}"), None, false)?;
                let _ = ast.add_param(push, "ev", ParamDirection::In, event)?;
            }
            ComponentRole::Receptacle {
                name: port,
                interface,
                multiple,
            } => {
                if multiple {
                    // `<name>Connections` is declared in the component's
                    // scope by the client-side equivalent IDL (§6.5.1).
                    let mut segments = module_path(ast, module);
                    segments.push(name.clone());
                    segments.push(format!("{port}Connections"));
                    let connections: Ref<TypeNode> =
                        Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(segments)));
                    let _ = ast.add_operation(
                        context,
                        &format!("get_connections_{port}"),
                        Some(connections),
                        false,
                    )?;
                } else {
                    let returns = object_reference(ast, &interface)?;
                    let _ = ast.add_operation(
                        context,
                        &format!("get_connection_{port}"),
                        Some(returns),
                        false,
                    )?;
                }
            }
            ComponentRole::EventSource {
                name: port, event, ..
            } => {
                let push = ast.add_operation(context, &format!("push_{port}"), None, false)?;
                let _ = ast.add_param(push, "ev", ParamDirection::In, event)?;
            }
            ComponentRole::PortTypeRole
            | ComponentRole::ConnectorRole
            | ComponentRole::HomeFactory
            | ComponentRole::HomeFinder => {}
        }
    }
    Ok(ComponentExecutors {
        main,
        monolithic,
        context,
    })
}

/// The primary key of a home or of the first keyed home it derives
/// from (§9.3.3.6.4: "a keyless home that derives from a keyed home").
fn primary_key_of(ast: &Ast, home: Id<InterfaceNode>) -> Option<Ref<TypeNode>> {
    let mut current = home;
    for _ in 0..64 {
        let node = ast.interface(current).ok()?;
        if let Some(key) = node
            .facets
            .corba
            .as_ref()
            .and_then(|corba| corba.manages.as_ref())
            .and_then(|m| m.primary_key.clone())
        {
            return Some(key);
        }
        match node.bases.first() {
            Some(Ref::Resolved(base)) => current = *base,
            _ => return None,
        }
    }
    None
}

/// §9.3.3.6 — the explicit, implicit and main home executors of a home.
///
/// # Errors
/// [`TransformError::NotA`] for a declaration that is no home,
/// [`TransformError::HomeManagesNothing`].
pub fn home_executors(
    ast: &mut Ast,
    home: Id<InterfaceNode>,
) -> Result<HomeExecutors, TransformError> {
    let node = ast.interface(home)?;
    if node.kind != InterfaceKind::Home {
        return Err(TransformError::NotA {
            expected: "a home",
            found: node.kind.as_str(),
        });
    }
    let (module, name) = (node.module, node.name.clone());
    let corba = node.facets.corba.clone().unwrap_or_default();
    if corba.manages.is_none() {
        return Err(TransformError::HomeManagesNothing(name));
    }
    let mut explicit_bases = match node.bases.first() {
        Some(b) => alloc::vec![renamed_ref(ast, b, "CCM_", "Explicit")],
        None => alloc::vec![spec_ref(&["Components", "HomeExecutorBase"])],
    };
    explicit_bases.extend(corba.supports.iter().cloned());
    let key = primary_key_of(ast, home);

    let explicit = local_interface(ast, module, &format!("CCM_{name}Explicit"), explicit_bases)?;
    // Rule 2: all attributes and operations of the home; §9.3.3.6.6 and
    // §9.3.3.6.7: a factory or finder returns EnterpriseComponent.
    for operation in ast.operations_of(home).to_vec() {
        let is_factory_or_finder = ast
            .operation(operation)?
            .facets
            .corba
            .as_ref()
            .and_then(|corba| corba.home_operation)
            .is_some();
        let returns = is_factory_or_finder.then(|| Some(enterprise_component()));
        copy_operation(ast, operation, explicit, returns)?;
    }
    copy_attributes(ast, home, explicit)?;

    let implicit = local_interface(ast, module, &format!("CCM_{name}Implicit"), Vec::new())?;
    let ccm_exception = || spec_ref(&["Components", "CCMException"]);
    match key {
        None => {
            let create =
                ast.add_operation(implicit, "create", Some(enterprise_component()), false)?;
            ast.add_raises(create, ccm_exception())?;
        }
        Some(key) => {
            for (op, returns) in [
                ("create", Some(enterprise_component())),
                ("find_by_primary_key", Some(enterprise_component())),
                ("remove", None),
            ] {
                let id = ast.add_operation(implicit, op, returns, false)?;
                let _ = ast.add_param(id, "key", ParamDirection::In, key.clone())?;
                ast.add_raises(id, ccm_exception())?;
            }
        }
    }

    let main = local_interface(
        ast,
        module,
        &format!("CCM_{name}"),
        alloc::vec![
            sibling_ref(ast, module, &format!("CCM_{name}Explicit")),
            sibling_ref(ast, module, &format!("CCM_{name}Implicit")),
        ],
    )?;
    Ok(HomeExecutors {
        explicit,
        implicit,
        main,
    })
}

/// §9.3 for a whole model: the facet executor of every interface that is
/// neither abstract nor local, the consumer executor of every event
/// type, the executors of every component and every home. Run it on the
/// declarations, before the client-side transformation rewrites
/// components and homes in place.
///
/// # Errors
/// As [`component_executors`] and [`home_executors`].
pub fn executor_idl(ast: &mut Ast) -> Result<ExecutorIdl, TransformError> {
    let mut out = ExecutorIdl::default();
    let declared: Vec<(Id<InterfaceNode>, InterfaceKind, String)> = ast
        .interfaces()
        .filter(|(_, node)| node.completeness != Completeness::Forward)
        .map(|(id, node)| (id, node.kind, node.name.clone()))
        .collect();
    for (id, kind, name) in &declared {
        if *kind == InterfaceKind::Interface && !name.starts_with("CCM_") {
            out.facets.push(facet_executor(ast, *id)?);
        }
    }
    let events: Vec<Id<TypeNode>> = ast
        .types()
        .filter(|(_, node)| {
            matches!(node.kind, TypeKind::Valuetype { .. })
                && node
                    .facets
                    .corba
                    .as_ref()
                    .is_some_and(|corba| corba.is_event_type)
        })
        .map(|(id, _)| id)
        .collect();
    for event in events {
        out.consumers.push(consumer_executor(ast, event)?);
    }
    for (id, kind, _) in &declared {
        if *kind == InterfaceKind::Component {
            out.components.push(component_executors(ast, *id)?);
        }
    }
    for (id, kind, _) in &declared {
        if *kind == InterfaceKind::Home {
            out.homes.push(home_executors(ast, *id)?);
        }
    }
    Ok(out)
}

/// The name of the local interface the container narrows an executor to,
/// for a component `name` and the strategy: `CCM_<name>` (monolithic) or
/// `CCM_<name>_Executor` (the main executor a locator returns).
#[must_use]
pub fn executor_interface_name(component: &str, monolithic: bool) -> String {
    if monolithic {
        format!("CCM_{component}")
    } else {
        format!("CCM_{component}_Executor")
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn corba_p3_9_3_3_3_executor_interface_names() {
        assert_eq!(executor_interface_name("Toon", true), "CCM_Toon");
        assert_eq!(executor_interface_name("Toon", false), "CCM_Toon_Executor");
    }
}
