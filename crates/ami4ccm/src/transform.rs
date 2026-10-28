// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Implied-IDL transformation — Spec §7.3 + §7.5.
//!
//! Input is an interface in the model; output is two `local interface`
//! declarations added beside it:
//!
//! 1. `AMI4CCM_<Iface>ReplyHandler` (Spec §7.5) — the type-specific
//!    reply handler, with its normal-reply and `_excep` operations.
//! 2. `AMI4CCM_<Iface>` (Spec §7.3) — the asynchronous interface, with
//!    the `sendc_` operations.
//!
//! Spec §7.3.1 (p. 7) — naming-conflict resolution: where a generated
//! `sendc_<op>` name already exists in the interface, `ami_` is inserted
//! between `sendc_` and `<op>` until the name is unique. The same rule
//! applies to `<op>_excep` in the reply handler (Spec §7.5.2, p. 10),
//! with `_ami` inserted before `_excep` instead.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{Ast, AstError, InterfaceKind, InterfaceNode, ParamDirection, TypeNode};
use zerodds_ast_core::{Id, PendingRef, Ref, ScopedName};

/// Why a transformation could not be carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransformError {
    /// The handle names no live node, or the model rejected an edit.
    Model(AstError),
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
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TransformError {}

/// The two local interfaces the transformation adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ami4CcmInterfaces {
    /// `AMI4CCM_<Iface>ReplyHandler` (Spec §7.5).
    pub reply_handler: Id<InterfaceNode>,
    /// `AMI4CCM_<Iface>` (Spec §7.3 + §7.6 `ami4ccm_provides`).
    pub ami_interface: Id<InterfaceNode>,
}

/// Compilation context for the AMI4CCM transform.
///
/// Spec references:
/// * §7.5 — a derived interface's reply handler inherits from the base
///   interface's reply handler, so the transformer needs to know which
///   bases have already been transformed (`known_bases`).
/// * §7.5 / §7.3.1 — where the plain `AMI4CCM_<Iface>` or
///   `…ReplyHandler` name collides with an identifier in the compilation
///   scope, `AMI_` is prepended until it does not (`known_symbols`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransformContext {
    /// Interfaces whose reply handler has already been generated. An
    /// interface whose base is in here gets
    /// `AMI4CCM_<Base>ReplyHandler` as its handler's base instead of the
    /// generic `CCM_AMI::ReplyHandler`.
    pub known_bases: BTreeSet<String>,
    /// Every identifier in the compilation scope, checked before the
    /// AMI4CCM names are emitted.
    pub known_symbols: BTreeSet<String>,
}

impl TransformContext {
    /// A context that knows nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark an interface as already transformed, so that interfaces
    /// derived from it inherit their reply handler correctly.
    pub fn mark_transformed(&mut self, name: &str) {
        let _ = self.known_bases.insert(name.to_string());
        let _ = self.known_symbols.insert(format!("AMI4CCM_{name}"));
        let _ = self
            .known_symbols
            .insert(format!("AMI4CCM_{name}ReplyHandler"));
    }

    /// Register an identifier the naming resolver must avoid.
    pub fn add_known_symbol(&mut self, name: &str) {
        let _ = self.known_symbols.insert(name.to_string());
    }
}

/// Transform an interface into the implied AMI4CCM pair. Spec §7.3 +
/// §7.5.
///
/// The convenience form, with no inheritance lookup and no symbol table.
/// For a fully conformant transformation use
/// [`transform_interface_in_context`].
///
/// # Errors
/// [`TransformError::Model`] when the model rejects an edit.
pub fn transform_interface(
    ast: &mut Ast,
    interface: Id<InterfaceNode>,
) -> Result<Ami4CcmInterfaces, TransformError> {
    transform_interface_in_context(ast, interface, &TransformContext::default())
}

/// The conformant form, with a compilation context.
///
/// **Naming (§7.3.1 + §7.5):** the reply handler is
/// `AMI4CCM_<name>ReplyHandler` and the asynchronous interface
/// `AMI4CCM_<name>`; on a collision with `ctx.known_symbols`, `AMI_` is
/// prepended repeatedly.
///
/// **Reply-handler inheritance (§7.5):** if a base's name is in
/// `ctx.known_bases`, the handler inherits `AMI4CCM_<Base>ReplyHandler`;
/// otherwise `CCM_AMI::ReplyHandler`.
///
/// **`local interface` (§7.3 + Annex A):** both are local and not
/// remotable.
///
/// # Errors
/// [`TransformError::Model`] when the model rejects an edit.
pub fn transform_interface_in_context(
    ast: &mut Ast,
    interface: Id<InterfaceNode>,
    ctx: &TransformContext,
) -> Result<Ami4CcmInterfaces, TransformError> {
    let node = ast.interface(interface)?;
    let original = node.name.clone();
    let module = node.module;
    let operations = node.operations.clone();
    let attributes = node.attributes.clone();
    let bases = node.bases.clone();

    // §7.3.1 — the names the original interface already carries, which
    // the generated ones must not collide with.
    let mut taken: Vec<String> = Vec::new();
    for id in &operations {
        if let Ok(operation) = ast.operation(*id) {
            taken.push(operation.name.clone());
        }
    }
    for id in &attributes {
        if let Ok(attribute) = ast.attribute(*id) {
            taken.push(format!("get_{}", attribute.name));
            if !attribute.readonly {
                taken.push(format!("set_{}", attribute.name));
            }
        }
    }

    // §7.5 — the parent reply handler, for a derived interface.
    let handler_base = bases
        .iter()
        .filter_map(|base| base_name(ast, base))
        .find(|name| ctx.known_bases.contains(name.as_str()))
        .map_or_else(
            || scoped_ref(&["CCM_AMI", "ReplyHandler"]),
            |base| named_ref(&format!("AMI4CCM_{base}ReplyHandler")),
        );

    let handler_name = unique_name(
        &format!("AMI4CCM_{original}ReplyHandler"),
        &ctx.known_symbols,
    );
    let ami_name = unique_name(&format!("AMI4CCM_{original}"), &ctx.known_symbols);

    let reply_handler = ast.add_interface(
        module,
        &handler_name,
        InterfaceKind::Local,
        alloc::vec![handler_base],
    )?;
    build_reply_handler(ast, reply_handler, &operations, &attributes, &taken)?;

    let ami_interface = ast.add_interface(module, &ami_name, InterfaceKind::Local, Vec::new())?;
    let handler_type = Ref::Resolved(ast.object_reference_to(reply_handler)?);
    build_ami_interface(
        ast,
        ami_interface,
        &handler_type,
        &operations,
        &attributes,
        &taken,
    )?;

    Ok(Ami4CcmInterfaces {
        reply_handler,
        ami_interface,
    })
}

/// Spec §7.3 (p. 6-7) — the `sendc_<op>` operations.
fn build_ami_interface(
    ast: &mut Ast,
    target: Id<InterfaceNode>,
    handler_type: &Ref<TypeNode>,
    operations: &[Id<zerodds_ast::OperationNode>],
    attributes: &[Id<zerodds_ast::AttributeNode>],
    taken: &[String],
) -> Result<(), TransformError> {
    let mut emitted: Vec<String> = Vec::new();

    for id in operations {
        let Ok(source) = ast.operation(*id) else {
            continue;
        };
        let original = source.name.clone();
        // §7.3.1.1: "Each of the in and inout arguments [...] all with a
        // parameter attribute of in [...]. out arguments are ignored."
        let carried: Vec<(String, Ref<TypeNode>)> = source
            .params
            .clone()
            .into_iter()
            .filter_map(|param| {
                let node = ast.param(param).ok()?;
                matches!(node.direction, ParamDirection::In | ParamDirection::InOut)
                    .then(|| (node.name.clone(), node.type_.clone()))
            })
            .collect();

        let name = sendc_name(&original, taken, &emitted);
        emitted.push(name.clone());
        // §7.3 (p. 5-6) — an asynchronous operation raises no user
        // exception; only `INV_OBJREF`, which no `raises` clause names.
        let op = ast.add_operation(target, &name, None, false)?;
        let _ = ast.add_param(op, "ami_handler", ParamDirection::In, handler_type.clone())?;
        for (param, type_) in carried {
            let _ = ast.add_param(op, &param, ParamDirection::In, type_)?;
        }
    }

    for id in attributes {
        let Ok(attribute) = ast.attribute(*id) else {
            continue;
        };
        let name = attribute.name.clone();
        let type_ = attribute.type_.clone();
        let readonly = attribute.readonly;

        // §7.3.1.2 (p. 7) — the getter.
        let getter = sendc_name(&format!("get_{name}"), taken, &emitted);
        emitted.push(getter.clone());
        let op = ast.add_operation(target, &getter, None, false)?;
        let _ = ast.add_param(op, "ami_handler", ParamDirection::In, handler_type.clone())?;

        if readonly {
            continue;
        }
        let setter = sendc_name(&format!("set_{name}"), taken, &emitted);
        emitted.push(setter.clone());
        let op = ast.add_operation(target, &setter, None, false)?;
        let _ = ast.add_param(op, "ami_handler", ParamDirection::In, handler_type.clone())?;
        let _ = ast.add_param(op, &format!("attr_{name}"), ParamDirection::In, type_)?;
    }
    Ok(())
}

/// Spec §7.5 (p. 9-10) — the normal-reply and `_excep` operations.
fn build_reply_handler(
    ast: &mut Ast,
    target: Id<InterfaceNode>,
    operations: &[Id<zerodds_ast::OperationNode>],
    attributes: &[Id<zerodds_ast::AttributeNode>],
    taken: &[String],
) -> Result<(), TransformError> {
    let holder: Ref<TypeNode> = scoped_ref(&["CCM_AMI", "ExceptionHolder"]);
    let mut emitted: Vec<String> = Vec::new();

    for id in operations {
        let Ok(source) = ast.operation(*id) else {
            continue;
        };
        let original = source.name.clone();
        let returns = source.return_type.clone();
        // §7.5.1: "Each inout/out type name and argument name as they
        // were declared in IDL" — all as `in`.
        let carried: Vec<(String, Ref<TypeNode>)> = source
            .params
            .clone()
            .into_iter()
            .filter_map(|param| {
                let node = ast.param(param).ok()?;
                matches!(node.direction, ParamDirection::InOut | ParamDirection::Out)
                    .then(|| (node.name.clone(), node.type_.clone()))
            })
            .collect();

        // §7.5.1 (p. 10): these operations raise nothing, because no
        // client is waiting on them.
        emitted.push(original.clone());
        let op = ast.add_operation(target, &original, None, false)?;
        if let Some(returns) = returns {
            let _ = ast.add_param(op, "ami_return_val", ParamDirection::In, returns)?;
        }
        for (param, type_) in carried {
            let _ = ast.add_param(op, &param, ParamDirection::In, type_)?;
        }

        // §7.5.2 — the exception reply.
        let excep = excep_name(&original, taken, &emitted);
        emitted.push(excep.clone());
        let op = ast.add_operation(target, &excep, None, false)?;
        let _ = ast.add_param(op, "excep_holder", ParamDirection::In, holder.clone())?;
    }

    for id in attributes {
        let Ok(attribute) = ast.attribute(*id) else {
            continue;
        };
        let name = attribute.name.clone();
        let type_ = attribute.type_.clone();
        let readonly = attribute.readonly;

        // §7.5.1 (p. 10) — `void get_<attr>(in <attrType> ami_return_val);`
        let getter = format!("get_{name}");
        emitted.push(getter.clone());
        let op = ast.add_operation(target, &getter, None, false)?;
        let _ = ast.add_param(op, "ami_return_val", ParamDirection::In, type_)?;

        let getter_excep = excep_name(&getter, taken, &emitted);
        emitted.push(getter_excep.clone());
        let op = ast.add_operation(target, &getter_excep, None, false)?;
        let _ = ast.add_param(op, "excep_holder", ParamDirection::In, holder.clone())?;

        if readonly {
            continue;
        }
        // §7.5.1 (p. 10) — the setter acknowledgement takes no arguments.
        let setter = format!("set_{name}");
        emitted.push(setter.clone());
        let _ = ast.add_operation(target, &setter, None, false)?;

        let setter_excep = excep_name(&setter, taken, &emitted);
        emitted.push(setter_excep.clone());
        let op = ast.add_operation(target, &setter_excep, None, false)?;
        let _ = ast.add_param(op, "excep_holder", ParamDirection::In, holder.clone())?;
    }
    Ok(())
}

/// Spec §7.3.1 (p. 7) — `sendc_<op>`, with `ami_` inserted until the
/// name is unique.
fn sendc_name(original: &str, taken: &[String], emitted: &[String]) -> String {
    let mut prefix = String::from("sendc_");
    loop {
        let candidate = format!("{prefix}{original}");
        if !taken.iter().any(|name| name == &candidate)
            && !emitted.iter().any(|name| name == &candidate)
        {
            return candidate;
        }
        prefix.push_str("ami_");
    }
}

/// Spec §7.5.2 (p. 10) — `<op>_excep`, with `_ami` inserted immediately
/// before `_excep` until the name is unique. The specification spells
/// this one `_ami` and the `sendc_` one `ami_`; both are kept as
/// written.
fn excep_name(original: &str, taken: &[String], emitted: &[String]) -> String {
    let mut suffix = String::from("_excep");
    loop {
        let candidate = format!("{original}{suffix}");
        if !taken.iter().any(|name| name == &candidate)
            && !emitted.iter().any(|name| name == &candidate)
        {
            return candidate;
        }
        suffix = format!("_ami{suffix}");
    }
}

/// Spec §7.3.1 / §7.5 — prepend `AMI_` until the name is free.
///
/// In the pathological case where every variant is taken the last
/// candidate is returned; the caller then lives with the duplicate,
/// which the model reports at declaration.
fn unique_name(base: &str, known: &BTreeSet<String>) -> String {
    let mut candidate = base.to_string();
    for _ in 0..16 {
        if !known.contains(&candidate) {
            return candidate;
        }
        candidate = format!("AMI_{candidate}");
    }
    candidate
}

fn base_name(ast: &Ast, reference: &Ref<InterfaceNode>) -> Option<String> {
    match reference {
        Ref::Resolved(id) => ast.interface(*id).ok().map(|node| node.name.clone()),
        Ref::Pending(PendingRef::ScopedName(name)) => name.last().cloned(),
        Ref::Pending(_) | Ref::Broken(_) => None,
    }
}

/// A pending reference to a name the specification introduces.
fn scoped_ref<T>(parts: &[&str]) -> Ref<T> {
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
