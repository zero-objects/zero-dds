// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Implied-IDL transformation — Spec §7.3 + §7.5.
//!
//! Input: [`InterfaceDef`] of a normal IDL interface.
//! Output: two `InterfaceKind::Local` interfaces (Spec §7.3 + §7.5):
//!
//! 1. `AMI4CCM_<Iface>ReplyHandler` (Spec §7.5) — type-specific reply
//!    handler with normal-reply + `_excep` operations.
//! 2. `AMI4CCM_<Iface>` (Spec §7.3) — async operations with the
//!    `sendc_` prefix.
//!
//! Spec §7.3.1 (p. 7) — naming-conflict resolution: if the generated
//! `sendc_<op>` name already exists in the interface, `ami_` prefixes
//! are inserted between `sendc_` and `<op>` until the name is unique.
//! The same rule applies to `<op>_excep` in the ReplyHandler (Spec
//! §7.5.2, p. 10).

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_idl::ast::{
    AttrDecl, Export, Identifier, InterfaceDef, InterfaceKind, OpDecl, ParamAttribute, ParamDecl,
    ScopedName, TypeSpec,
};
use zerodds_idl::errors::Span;

/// Result of the transformation: two additional local interfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct Ami4CcmInterfaces {
    /// `AMI4CCM_<Iface>ReplyHandler` (Spec §7.5).
    pub reply_handler: InterfaceDef,
    /// `AMI4CCM_<Iface>` (Spec §7.3 + §7.6 ami4ccm_provides).
    pub ami_interface: InterfaceDef,
}

/// Compilation context for the AMI4CCM transform.
///
/// Spec references:
/// * §7.5 — derived interfaces inherit their ReplyHandler from the
///   ReplyHandler of the base interface (`AMI4CCM_<Base>ReplyHandler`).
///   For this, the transformer needs a map of the already-transformed
///   bases (`known_bases`).
/// * §7.5 / §7.3.1 — if the naive `AMI4CCM_<Iface>`/`ReplyHandler` name
///   collides with an existing identifier in the compilation unit,
///   `AMI_` prefixes must be added until the name is unique
///   (`known_symbols`).
#[derive(Debug, Clone, Default)]
pub struct TransformContext {
    /// Set of original interface names whose ReplyHandler has already
    /// been generated. Consulted in `transform_interface` for an
    /// interface with `iface.bases = [Base]`: if `Base.text` is
    /// present, the new ReplyHandler inherits from
    /// `AMI4CCM_<Base>ReplyHandler` instead of the generic
    /// `CCM_AMI::ReplyHandler`.
    pub known_bases: BTreeSet<String>,
    /// Set of all identifiers in the current compilation scope. Checked
    /// against the conflict resolver before emitting the AMI4CCM
    /// interface names (`AMI_AMI4CCM_<Iface>` etc.).
    pub known_symbols: BTreeSet<String>,
}

impl TransformContext {
    /// New empty context (equivalent to `default()`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks an interface name as already-transformed, so that derived
    /// interfaces inherit their ReplyHandler correctly.
    pub fn mark_transformed(&mut self, iface_name: &str) {
        self.known_bases.insert(iface_name.to_string());
        self.known_symbols.insert(format!("AMI4CCM_{iface_name}"));
        self.known_symbols
            .insert(format!("AMI4CCM_{iface_name}ReplyHandler"));
    }

    /// Registers an already-existing identifier that the AMI4CCM naming
    /// resolver must check against.
    pub fn add_known_symbol(&mut self, name: &str) {
        self.known_symbols.insert(name.to_string());
    }
}

/// Transforms a normal [`InterfaceDef`] into the implied AMI4CCM pair.
/// Spec §7.3 + §7.5.
///
/// Convenience wrapper without a context (no inheritance lookup, no
/// symbol table). For a fully spec-compliant transformation, call
/// `transform_interface_in_context`.
#[must_use]
pub fn transform_interface(iface: &InterfaceDef) -> Ami4CcmInterfaces {
    transform_interface_in_context(iface, &TransformContext::default())
}

/// Spec-compliant variant with a compilation context.
///
/// **Naming Convention (Spec §7.3.1 + §7.5):**
/// * Reply-handler name: `AMI4CCM_<original-iface-name>ReplyHandler`,
///   on conflict with `ctx.known_symbols`: `AMI_AMI4CCM_<...>` etc.
/// * Async-interface name: `AMI4CCM_<original-iface-name>`.
///
/// **Reply-Handler Inheritance (Spec §7.5):**
/// If `iface.bases` has an entry whose `last` identifier is contained
/// in `ctx.known_bases`, the ReplyHandler inherits from
/// `AMI4CCM_<Base>ReplyHandler`. Otherwise from
/// `CCM_AMI::ReplyHandler` (default).
///
/// **`InterfaceKind::Local`** (Spec §7.3 + Annex A): both generated
/// interfaces are `local interface` (not remotable).
#[must_use]
pub fn transform_interface_in_context(
    iface: &InterfaceDef,
    ctx: &TransformContext,
) -> Ami4CcmInterfaces {
    let span = Span::SYNTHETIC;
    let original_name = iface.name.text.clone();

    // Collect all original operation names + attribute setter/getter
    // names to resolve naming conflicts (Spec §7.3.1).
    let mut original_op_names: Vec<String> = Vec::new();
    for export in &iface.exports {
        match export {
            Export::Op(op) => original_op_names.push(op.name.text.clone()),
            Export::Attr(attr) => {
                original_op_names.push(format!("get_{}", attr.name.text));
                if !attr.readonly {
                    original_op_names.push(format!("set_{}", attr.name.text));
                }
            }
            _ => {}
        }
    }

    // Spec §7.5 — Reply-handler inheritance for derived interfaces.
    // We consult ctx.known_bases: the first base in iface.bases whose
    // last identifier part is contained in ctx.known_bases determines
    // the parent ReplyHandler.
    let reply_handler_base = iface
        .bases
        .iter()
        .find_map(|b| {
            b.parts
                .last()
                .map(|i| &i.text)
                .filter(|n| ctx.known_bases.contains(n.as_str()))
                .cloned()
        })
        .map_or_else(
            || scoped_name(&["CCM_AMI", "ReplyHandler"], span),
            |base| ScopedName::single(Identifier::new(format!("AMI4CCM_{base}ReplyHandler"), span)),
        );

    // Spec §7.3.1 / §7.5 — naming conflict with the compilation scope:
    // if AMI4CCM_<Iface>[ReplyHandler] is already in ctx.known_symbols,
    // prefix with AMI_ (recursively).
    let ami_iface_name =
        resolve_unique_iface_name(&format!("AMI4CCM_{original_name}"), &ctx.known_symbols);
    let reply_handler_name = resolve_unique_iface_name(
        &format!("AMI4CCM_{original_name}ReplyHandler"),
        &ctx.known_symbols,
    );

    let reply_handler = build_reply_handler(
        &original_name,
        &reply_handler_name,
        reply_handler_base,
        &iface.exports,
        &original_op_names,
        span,
    );
    let ami_interface = build_ami_interface(
        &original_name,
        &ami_iface_name,
        &iface.exports,
        &original_op_names,
        span,
    );

    Ami4CcmInterfaces {
        reply_handler,
        ami_interface,
    }
}

/// Resolver for the naming conflict from Spec §7.3.1 / §7.5.
///
/// If `name` is contained in `known`, prepends an `AMI_` prefix and
/// tries again. In the pathological case (all variants taken) the
/// function returns the last candidate — the caller then falls back to
/// the unavoidable duplicate.
fn resolve_unique_iface_name(base: &str, known: &BTreeSet<String>) -> String {
    let mut candidate = base.to_string();
    for _ in 0..16 {
        if !known.contains(&candidate) {
            return candidate;
        }
        candidate = format!("AMI_{candidate}");
    }
    candidate
}

/// Spec §7.3 (p. 6-7) — `AMI4CCM_<Iface>` async interface with
/// `sendc_<op>` operations.
fn build_ami_interface(
    original_name: &str,
    iface_name: &str,
    exports: &[Export],
    original_op_names: &[String],
    span: Span,
) -> InterfaceDef {
    let handler_type = handler_type_spec(original_name);
    // Track already-emitted names to avoid internal collisions
    // (e.g. two "ami_..." escalations).
    let mut emitted: Vec<String> = Vec::new();
    let mut new_exports: Vec<Export> = Vec::new();

    for export in exports {
        match export {
            Export::Op(op) => {
                let name = resolve_sendc_name(&op.name.text, original_op_names, &emitted);
                emitted.push(name.clone());
                new_exports.push(Export::Op(build_sendc_op(&name, &handler_type, op, span)));
            }
            Export::Attr(attr) => {
                // Getter — Spec §7.3.1.2 (p. 7).
                let getter_name = resolve_sendc_name(
                    &format!("get_{}", attr.name.text),
                    original_op_names,
                    &emitted,
                );
                emitted.push(getter_name.clone());
                new_exports.push(Export::Op(build_sendc_attr_get(
                    &getter_name,
                    &handler_type,
                    span,
                )));
                if !attr.readonly {
                    let setter_name = resolve_sendc_name(
                        &format!("set_{}", attr.name.text),
                        original_op_names,
                        &emitted,
                    );
                    emitted.push(setter_name.clone());
                    new_exports.push(Export::Op(build_sendc_attr_set(
                        &setter_name,
                        &handler_type,
                        attr,
                        span,
                    )));
                }
            }
            // Spec §7.3 — types/consts/exceptions are not relevant in
            // the async interface; they remain visible in the original
            // interface.
            _ => {}
        }
    }

    InterfaceDef {
        kind: InterfaceKind::Local,
        name: Identifier::new(iface_name, span),
        bases: Vec::new(),
        exports: new_exports,
        annotations: Vec::new(),
        span,
    }
}

/// Spec §7.5 (p. 9-10) — `AMI4CCM_<Iface>ReplyHandler` with normal
/// reply + `_excep` operations.
fn build_reply_handler(
    original_name: &str,
    handler_name: &str,
    handler_base: ScopedName,
    exports: &[Export],
    original_op_names: &[String],
    span: Span,
) -> InterfaceDef {
    let _ = original_name; // unused now; reserved for future extensions
    let exc_holder_type = exception_holder_type_spec();
    let mut emitted: Vec<String> = Vec::new();
    let mut new_exports: Vec<Export> = Vec::new();

    for export in exports {
        match export {
            Export::Op(op) => {
                // Normal reply (Spec §7.5.1).
                let normal_name = op.name.text.clone();
                emitted.push(normal_name.clone());
                new_exports.push(Export::Op(build_handler_normal_op(&normal_name, op, span)));
                // Exception reply (Spec §7.5.2).
                let excep_name = resolve_excep_name(&op.name.text, original_op_names, &emitted);
                emitted.push(excep_name.clone());
                new_exports.push(Export::Op(build_handler_excep_op(
                    &excep_name,
                    &exc_holder_type,
                    span,
                )));
            }
            Export::Attr(attr) => {
                // Getter normal — `void get_<attr>(in <type> ami_return_val);`.
                let get_name = format!("get_{}", attr.name.text);
                emitted.push(get_name.clone());
                new_exports.push(Export::Op(build_handler_attr_get(
                    &get_name,
                    &attr.type_spec,
                    span,
                )));
                // Getter excep.
                let get_excep = resolve_excep_name(&get_name, original_op_names, &emitted);
                emitted.push(get_excep.clone());
                new_exports.push(Export::Op(build_handler_excep_op(
                    &get_excep,
                    &exc_holder_type,
                    span,
                )));
                if !attr.readonly {
                    // Setter normal — `void set_<attr>();` (no args).
                    let set_name = format!("set_{}", attr.name.text);
                    emitted.push(set_name.clone());
                    new_exports.push(Export::Op(build_handler_attr_set_ack(&set_name, span)));
                    // Setter excep.
                    let set_excep = resolve_excep_name(&set_name, original_op_names, &emitted);
                    emitted.push(set_excep.clone());
                    new_exports.push(Export::Op(build_handler_excep_op(
                        &set_excep,
                        &exc_holder_type,
                        span,
                    )));
                }
            }
            _ => {}
        }
    }

    InterfaceDef {
        kind: InterfaceKind::Local,
        name: Identifier::new(handler_name, span),
        // Spec §7.5 (p. 9): "is derived from the generic CCM_AMI::ReplyHandler"
        // — for derived interfaces with a known base, the parent handler
        // of the base interface is used (see TransformContext).
        bases: alloc::vec![handler_base],
        exports: new_exports,
        annotations: Vec::new(),
        span,
    }
}

/// Spec §7.3.1.1 (p. 7) — `void sendc_<op>(in handler, in/inout-args)`.
fn build_sendc_op(name: &str, handler_type: &TypeSpec, op: &OpDecl, span: Span) -> OpDecl {
    let mut params = Vec::new();
    params.push(handler_param(handler_type, span));
    for p in &op.params {
        // Spec §7.3.1.1: "Each of the in and inout arguments [...] all
        // with a parameter attribute of in [...]. out arguments are
        // ignored."
        if matches!(p.attribute, ParamAttribute::In | ParamAttribute::InOut) {
            params.push(ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: p.type_spec.clone(),
                name: p.name.clone(),
                annotations: Vec::new(),
                span,
            });
        }
    }
    OpDecl {
        name: Identifier::new(name, span),
        oneway: false,
        context: Vec::new(),
        return_type: None,
        params,
        // Spec §7.3 (p. 5/6) — async operations raise no user
        // exceptions; only the `INV_OBJREF` system exception, which does
        // not appear in `raises()`.
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    }
}

/// Spec §7.3.1.2 (p. 7) — `void sendc_get_<attr>(in handler);`.
fn build_sendc_attr_get(name: &str, handler_type: &TypeSpec, span: Span) -> OpDecl {
    OpDecl {
        name: Identifier::new(name, span),
        oneway: false,
        context: Vec::new(),
        return_type: None,
        params: alloc::vec![handler_param(handler_type, span)],
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    }
}

/// Spec §7.3.1.2 (p. 7) — `void sendc_set_<attr>(in handler, in
/// <attrType> attr_<attrName>);`.
fn build_sendc_attr_set(
    name: &str,
    handler_type: &TypeSpec,
    attr: &AttrDecl,
    span: Span,
) -> OpDecl {
    let arg_name = format!("attr_{}", attr.name.text);
    OpDecl {
        name: Identifier::new(name, span),
        oneway: false,
        context: Vec::new(),
        return_type: None,
        params: alloc::vec![
            handler_param(handler_type, span),
            ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: attr.type_spec.clone(),
                name: Identifier::new(arg_name, span),
                annotations: Vec::new(),
                span,
            },
        ],
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    }
}

/// Spec §7.5.1 (p. 9-10) — `void <op>(in <ret> ami_return_val,
/// in/inout-args)`.
fn build_handler_normal_op(name: &str, op: &OpDecl, span: Span) -> OpDecl {
    let mut params = Vec::new();
    if let Some(ret) = &op.return_type {
        params.push(ParamDecl {
            attribute: ParamAttribute::In,
            type_spec: ret.clone(),
            name: Identifier::new("ami_return_val", span),
            annotations: Vec::new(),
            span,
        });
    }
    for p in &op.params {
        // Spec §7.5.1: "Each inout/out type name and argument name as
        // they were declared in IDL." — all as `in`.
        if matches!(p.attribute, ParamAttribute::InOut | ParamAttribute::Out) {
            params.push(ParamDecl {
                attribute: ParamAttribute::In,
                type_spec: p.type_spec.clone(),
                name: p.name.clone(),
                annotations: Vec::new(),
                span,
            });
        }
    }
    OpDecl {
        name: Identifier::new(name, span),
        oneway: false,
        context: Vec::new(),
        return_type: None,
        params,
        // Spec §7.5.1 (p. 10): "These operations do not raise any
        // exceptions because they are never invoked by a client and
        // have no client to respond to such an exception."
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    }
}

/// Spec §7.5.1 (p. 10) — `void get_<attr>(in <attrType> ami_return_val);`.
fn build_handler_attr_get(name: &str, attr_type: &TypeSpec, span: Span) -> OpDecl {
    OpDecl {
        name: Identifier::new(name, span),
        oneway: false,
        context: Vec::new(),
        return_type: None,
        params: alloc::vec![ParamDecl {
            attribute: ParamAttribute::In,
            type_spec: attr_type.clone(),
            name: Identifier::new("ami_return_val", span),
            annotations: Vec::new(),
            span,
        }],
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    }
}

/// Spec §7.5.1 (p. 10) — `void set_<attr>();` (setter acknowledgement,
/// no arguments).
fn build_handler_attr_set_ack(name: &str, span: Span) -> OpDecl {
    OpDecl {
        name: Identifier::new(name, span),
        oneway: false,
        context: Vec::new(),
        return_type: None,
        params: Vec::new(),
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    }
}

/// Spec §7.5.2 (p. 10) — `void <op>_excep(in CCM_AMI::ExceptionHolder
/// excep_holder);`.
fn build_handler_excep_op(name: &str, exc_holder_type: &TypeSpec, span: Span) -> OpDecl {
    OpDecl {
        name: Identifier::new(name, span),
        oneway: false,
        context: Vec::new(),
        return_type: None,
        params: alloc::vec![ParamDecl {
            attribute: ParamAttribute::In,
            type_spec: exc_holder_type.clone(),
            name: Identifier::new("excep_holder", span),
            annotations: Vec::new(),
            span,
        }],
        raises: Vec::new(),
        annotations: Vec::new(),
        span,
    }
}

/// Spec §7.3.1 (p. 7) — naming-conflict resolution for `sendc_<op>`:
/// "If this implied-IDL operation name conflicts with existing
/// operations on the interface or any of the interface's base
/// interfaces, `ami_` strings are inserted between `sendc_` and the
/// original operation name until the implied-IDL operation name is
/// unique."
fn resolve_sendc_name(
    original_op_name: &str,
    forbidden_in_iface: &[String],
    already_emitted: &[String],
) -> String {
    let mut prefix = String::from("sendc_");
    loop {
        let candidate = format!("{prefix}{original_op_name}");
        if !forbidden_in_iface.iter().any(|n| n == &candidate)
            && !already_emitted.iter().any(|n| n == &candidate)
        {
            return candidate;
        }
        prefix.push_str("ami_");
    }
}

/// Spec §7.5.2 (p. 10) — analogous for `<op>_excep` in the ReplyHandler:
/// "If the name generated by the method described above clashes with
/// a name that already exists in the interface, `_ami` strings are
/// inserted immediately preceding the `_excep` repeatedly, until
/// generated IDL operation name is unique in the interface."
fn resolve_excep_name(
    original_op_name: &str,
    forbidden_in_iface: &[String],
    already_emitted: &[String],
) -> String {
    let mut suffix = String::from("_excep");
    loop {
        let candidate = format!("{original_op_name}{suffix}");
        if !forbidden_in_iface.iter().any(|n| n == &candidate)
            && !already_emitted.iter().any(|n| n == &candidate)
        {
            return candidate;
        }
        // Spec says "_ami" (not "ami_" as for sendc) — kept faithful to
        // the spec here.
        suffix = format!("_ami{suffix}");
    }
}

fn handler_param(handler_type: &TypeSpec, span: Span) -> ParamDecl {
    ParamDecl {
        attribute: ParamAttribute::In,
        type_spec: handler_type.clone(),
        name: Identifier::new("ami_handler", span),
        annotations: Vec::new(),
        span,
    }
}

fn handler_type_spec(original_name: &str) -> TypeSpec {
    TypeSpec::Scoped(scoped_name(
        &[&format!("AMI4CCM_{original_name}ReplyHandler")],
        Span::SYNTHETIC,
    ))
}

fn exception_holder_type_spec() -> TypeSpec {
    TypeSpec::Scoped(scoped_name(
        &["CCM_AMI", "ExceptionHolder"],
        Span::SYNTHETIC,
    ))
}

fn scoped_name(parts: &[&str], span: Span) -> ScopedName {
    ScopedName {
        absolute: false,
        parts: parts
            .iter()
            .map(|p| Identifier::new((*p).to_string(), span))
            .collect(),
        span,
    }
}

/// Helper for `void`-type checks in tests.
#[cfg(test)]
fn assert_void_return(op: &OpDecl) {
    assert!(
        op.return_type.is_none(),
        "expected void return on {}",
        op.name.text
    );
}

#[cfg(test)]
#[allow(clippy::panic, clippy::expect_used)]
mod tests {
    use super::*;
    use zerodds_idl::ast::{IntegerType, PrimitiveType, StringType};

    fn primitive(p: PrimitiveType) -> TypeSpec {
        TypeSpec::Primitive(p)
    }

    fn op(
        name: &str,
        return_type: Option<TypeSpec>,
        params: Vec<(ParamAttribute, TypeSpec, &str)>,
    ) -> Export {
        let span = Span::SYNTHETIC;
        Export::Op(OpDecl {
            name: Identifier::new(name, span),
            oneway: false,
            context: Vec::new(),
            return_type,
            params: params
                .into_iter()
                .map(|(attr, ty, n)| ParamDecl {
                    attribute: attr,
                    type_spec: ty,
                    name: Identifier::new(n, span),
                    annotations: Vec::new(),
                    span,
                })
                .collect(),
            raises: Vec::new(),
            annotations: Vec::new(),
            span,
        })
    }

    fn attr(name: &str, ty: TypeSpec, readonly: bool) -> Export {
        let span = Span::SYNTHETIC;
        Export::Attr(AttrDecl {
            name: Identifier::new(name, span),
            type_spec: ty,
            readonly,
            get_raises: Vec::new(),
            set_raises: Vec::new(),
            annotations: Vec::new(),
            span,
        })
    }

    fn iface(name: &str, exports: Vec<Export>) -> InterfaceDef {
        InterfaceDef {
            kind: InterfaceKind::Plain,
            name: Identifier::new(name, Span::SYNTHETIC),
            bases: Vec::new(),
            exports,
            annotations: Vec::new(),
            span: Span::SYNTHETIC,
        }
    }

    fn string_ty() -> TypeSpec {
        TypeSpec::String(StringType {
            wide: false,
            bound: None,
            span: Span::SYNTHETIC,
        })
    }

    fn double_ty() -> TypeSpec {
        primitive(PrimitiveType::Floating(
            zerodds_idl::ast::FloatingType::Double,
        ))
    }

    fn boolean_ty() -> TypeSpec {
        primitive(PrimitiveType::Boolean)
    }

    fn long_ty() -> TypeSpec {
        primitive(PrimitiveType::Integer(IntegerType::Long))
    }

    #[test]
    fn produces_two_local_interfaces_with_correct_names() {
        // Spec §7.3 + §7.5 — Naming.
        let i = iface("StockManager", alloc::vec![]);
        let out = transform_interface(&i);
        assert_eq!(out.ami_interface.name.text, "AMI4CCM_StockManager");
        assert_eq!(
            out.reply_handler.name.text,
            "AMI4CCM_StockManagerReplyHandler"
        );
        assert_eq!(out.ami_interface.kind, InterfaceKind::Local);
        assert_eq!(out.reply_handler.kind, InterfaceKind::Local);
    }

    #[test]
    fn reply_handler_inherits_from_ccm_ami_replyhandler() {
        // Spec §7.5 (p. 9): "is derived from the generic CCM_AMI::
        // ReplyHandler".
        let i = iface("Foo", alloc::vec![]);
        let out = transform_interface(&i);
        assert_eq!(out.reply_handler.bases.len(), 1);
        let base = &out.reply_handler.bases[0];
        assert_eq!(
            base.parts
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>(),
            alloc::vec!["CCM_AMI", "ReplyHandler"]
        );
    }

    #[test]
    fn sendc_op_has_handler_first_then_in_inout_only() {
        // Spec §7.3.1.1 — out args are ignored; inout becomes in.
        let i = iface(
            "I",
            alloc::vec![op(
                "remove_stock",
                None,
                alloc::vec![
                    (ParamAttribute::In, string_ty(), "symbol"),
                    (ParamAttribute::Out, double_ty(), "quote"),
                ],
            )],
        );
        let out = transform_interface(&i);
        let Export::Op(o) = &out.ami_interface.exports[0] else {
            panic!("expected op");
        };
        assert_eq!(o.name.text, "sendc_remove_stock");
        assert_void_return(o);
        // Expected: ami_handler + symbol; out arg "quote" is dropped.
        assert_eq!(o.params.len(), 2);
        assert_eq!(o.params[0].name.text, "ami_handler");
        assert_eq!(o.params[0].attribute, ParamAttribute::In);
        assert_eq!(o.params[1].name.text, "symbol");
        assert_eq!(o.params[1].attribute, ParamAttribute::In);
    }

    #[test]
    fn sendc_inout_becomes_in() {
        // Spec §7.3.1.1 — "all with a parameter attribute of in".
        let i = iface(
            "I",
            alloc::vec![op(
                "find_closest_symbol",
                Some(boolean_ty()),
                alloc::vec![(ParamAttribute::InOut, string_ty(), "symbol")],
            )],
        );
        let out = transform_interface(&i);
        let Export::Op(o) = &out.ami_interface.exports[0] else {
            panic!()
        };
        assert_eq!(o.params.len(), 2);
        // Original inout -> in.
        assert_eq!(o.params[1].attribute, ParamAttribute::In);
        assert_eq!(o.params[1].name.text, "symbol");
    }

    #[test]
    fn handler_op_has_return_value_then_inout_out() {
        // Spec §7.5.1 (p. 9-10): ami_return_val first, then inout/out.
        let i = iface(
            "I",
            alloc::vec![op(
                "remove_stock",
                Some(double_ty()),
                alloc::vec![
                    (ParamAttribute::In, string_ty(), "symbol"),
                    (ParamAttribute::Out, double_ty(), "quote"),
                ],
            )],
        );
        let out = transform_interface(&i);
        // First reply-handler op = "remove_stock" (normal), second =
        // "remove_stock_excep".
        let Export::Op(o) = &out.reply_handler.exports[0] else {
            panic!()
        };
        assert_eq!(o.name.text, "remove_stock");
        // ami_return_val + quote (out becomes in in the handler).
        assert_eq!(o.params.len(), 2);
        assert_eq!(o.params[0].name.text, "ami_return_val");
        assert_eq!(o.params[0].attribute, ParamAttribute::In);
        assert_eq!(o.params[1].name.text, "quote");
        assert_eq!(o.params[1].attribute, ParamAttribute::In);
    }

    #[test]
    fn handler_excep_op_takes_exception_holder() {
        // Spec §7.5.2 (p. 10): `void <op>_excep(in CCM_AMI::
        // ExceptionHolder excep_holder);`.
        let i = iface(
            "I",
            alloc::vec![op("get_quote", Some(double_ty()), alloc::vec![])],
        );
        let out = transform_interface(&i);
        // Index 0 = get_quote, Index 1 = get_quote_excep.
        let Export::Op(o) = &out.reply_handler.exports[1] else {
            panic!()
        };
        assert_eq!(o.name.text, "get_quote_excep");
        assert_eq!(o.params.len(), 1);
        assert_eq!(o.params[0].name.text, "excep_holder");
        let TypeSpec::Scoped(sn) = &o.params[0].type_spec else {
            panic!("expected ScopedName for ExceptionHolder")
        };
        assert_eq!(
            sn.parts.iter().map(|p| p.text.as_str()).collect::<Vec<_>>(),
            alloc::vec!["CCM_AMI", "ExceptionHolder"]
        );
    }

    #[test]
    fn attribute_get_set_generated_in_both_interfaces() {
        // Spec §7.3.1.2 (p. 7) + §7.5.1 (p. 10) — getter+setter path
        // for writable attributes.
        let i = iface(
            "I",
            alloc::vec![attr("stock_exchange_name", string_ty(), false)],
        );
        let out = transform_interface(&i);

        // AMI-Iface: sendc_get_..., sendc_set_...
        let names: Vec<String> = out
            .ami_interface
            .exports
            .iter()
            .map(|e| match e {
                Export::Op(o) => o.name.text.clone(),
                _ => String::new(),
            })
            .collect();
        assert!(names.contains(&String::from("sendc_get_stock_exchange_name")));
        assert!(names.contains(&String::from("sendc_set_stock_exchange_name")));

        // Reply-Handler: get_x, get_x_excep, set_x, set_x_excep.
        let h_names: Vec<String> = out
            .reply_handler
            .exports
            .iter()
            .map(|e| match e {
                Export::Op(o) => o.name.text.clone(),
                _ => String::new(),
            })
            .collect();
        assert!(h_names.contains(&String::from("get_stock_exchange_name")));
        assert!(h_names.contains(&String::from("get_stock_exchange_name_excep")));
        assert!(h_names.contains(&String::from("set_stock_exchange_name")));
        assert!(h_names.contains(&String::from("set_stock_exchange_name_excep")));
    }

    #[test]
    fn readonly_attribute_only_generates_getter() {
        // Spec §7.3.1.2 (p. 7): "Setter operations are only generated
        // for attributes that are not defined readonly".
        let i = iface("I", alloc::vec![attr("price", double_ty(), true)]);
        let out = transform_interface(&i);
        let names: Vec<String> = out
            .ami_interface
            .exports
            .iter()
            .map(|e| match e {
                Export::Op(o) => o.name.text.clone(),
                _ => String::new(),
            })
            .collect();
        assert!(names.contains(&String::from("sendc_get_price")));
        assert!(!names.iter().any(|n| n.starts_with("sendc_set_")));
    }

    #[test]
    fn sendc_attr_setter_takes_attr_prefixed_arg() {
        // Spec §7.3.1.2: "in <attrType> attr_<attributeName>".
        let i = iface(
            "I",
            alloc::vec![attr("stock_exchange_name", string_ty(), false)],
        );
        let out = transform_interface(&i);
        // Index 1 = sendc_set_stock_exchange_name (index 0 = getter).
        let Export::Op(o) = &out.ami_interface.exports[1] else {
            panic!()
        };
        assert_eq!(o.name.text, "sendc_set_stock_exchange_name");
        assert_eq!(o.params.len(), 2);
        assert_eq!(o.params[1].name.text, "attr_stock_exchange_name");
    }

    #[test]
    fn handler_attr_setter_ack_has_no_args() {
        // Spec §7.5.1 (example p. 10): "void set_stock_exchange_name();".
        let i = iface(
            "I",
            alloc::vec![attr("stock_exchange_name", string_ty(), false)],
        );
        let out = transform_interface(&i);
        // Order: get_x, get_x_excep, set_x, set_x_excep.
        let Export::Op(o) = &out.reply_handler.exports[2] else {
            panic!()
        };
        assert_eq!(o.name.text, "set_stock_exchange_name");
        assert!(o.params.is_empty());
    }

    #[test]
    fn naming_conflict_resolved_with_ami_prefix() {
        // Spec §7.3.1 (p. 7) — if `sendc_<op>` already exists in the
        // interface, insert "ami_" until unique.
        let i = iface(
            "I",
            alloc::vec![
                op("foo", None, alloc::vec![]),
                op("sendc_foo", None, alloc::vec![]),
            ],
        );
        let out = transform_interface(&i);
        let names: Vec<String> = out
            .ami_interface
            .exports
            .iter()
            .map(|e| match e {
                Export::Op(o) => o.name.text.clone(),
                _ => String::new(),
            })
            .collect();
        // "sendc_foo" exists -> "sendc_ami_foo" is used.
        assert!(names.contains(&String::from("sendc_ami_foo")));
        assert!(names.contains(&String::from("sendc_sendc_foo")));
    }

    #[test]
    fn excep_naming_conflict_resolved_with_ami_suffix() {
        // Spec §7.5.2 (p. 10) — `_ami_excep` etc.
        let i = iface(
            "I",
            alloc::vec![
                op("foo", None, alloc::vec![]),
                op("foo_excep", None, alloc::vec![]),
            ],
        );
        let out = transform_interface(&i);
        let h_names: Vec<String> = out
            .reply_handler
            .exports
            .iter()
            .map(|e| match e {
                Export::Op(o) => o.name.text.clone(),
                _ => String::new(),
            })
            .collect();
        // "foo_excep" exists -> "foo_ami_excep" is used.
        assert!(h_names.contains(&String::from("foo_ami_excep")));
        // foo_excep_excep is NOT (that would be an original _excep
        // without conflict for the original foo_excep).
        assert!(h_names.contains(&String::from("foo_excep_excep")));
    }

    #[test]
    fn full_stockmanager_running_example_yields_spec_signatures() {
        // Spec §7.2 (p. 5-6) running example -> §7.3.1.3 (p. 8) +
        // §7.5.3 (p. 11) expected signatures.
        let i = iface(
            "StockManager",
            alloc::vec![
                attr("stock_exchange_name", string_ty(), false),
                op(
                    "set_stock",
                    None,
                    alloc::vec![
                        (ParamAttribute::In, string_ty(), "symbol"),
                        (ParamAttribute::In, double_ty(), "new_quote"),
                    ],
                ),
                op(
                    "remove_stock",
                    None,
                    alloc::vec![
                        (ParamAttribute::In, string_ty(), "symbol"),
                        (ParamAttribute::Out, double_ty(), "quote"),
                    ],
                ),
                op(
                    "find_closest_symbol",
                    Some(boolean_ty()),
                    alloc::vec![(ParamAttribute::InOut, string_ty(), "symbol")],
                ),
                op(
                    "get_quote",
                    Some(double_ty()),
                    alloc::vec![(ParamAttribute::In, string_ty(), "symbol")],
                ),
            ],
        );
        let out = transform_interface(&i);

        // §7.3.1.3 (p. 8) — the AMI interface contains these sendc_ ops.
        let ami_names: Vec<String> = out
            .ami_interface
            .exports
            .iter()
            .map(|e| match e {
                Export::Op(o) => o.name.text.clone(),
                _ => String::new(),
            })
            .collect();
        for expected in [
            "sendc_get_stock_exchange_name",
            "sendc_set_stock_exchange_name",
            "sendc_set_stock",
            "sendc_remove_stock",
            "sendc_find_closest_symbol",
            "sendc_get_quote",
        ] {
            assert!(
                ami_names.contains(&String::from(expected)),
                "missing {expected} in {ami_names:?}"
            );
        }

        // §7.5.3 (p. 11) — ReplyHandler signatures.
        let h_names: Vec<String> = out
            .reply_handler
            .exports
            .iter()
            .map(|e| match e {
                Export::Op(o) => o.name.text.clone(),
                _ => String::new(),
            })
            .collect();
        for expected in [
            "get_stock_exchange_name",
            "get_stock_exchange_name_excep",
            "set_stock_exchange_name",
            "set_stock_exchange_name_excep",
            "set_stock",
            "set_stock_excep",
            "remove_stock",
            "remove_stock_excep",
            "find_closest_symbol",
            "find_closest_symbol_excep",
            "get_quote",
            "get_quote_excep",
        ] {
            assert!(
                h_names.contains(&String::from(expected)),
                "missing {expected} in {h_names:?}"
            );
        }
    }

    #[test]
    fn operation_with_no_return_no_args_yields_handler_op_with_no_params() {
        // Spec §7.5.1 (p. 10) "first case" — void return + no params.
        let i = iface("I", alloc::vec![op("acknowledge", None, alloc::vec![])]);
        let out = transform_interface(&i);
        let Export::Op(o) = &out.reply_handler.exports[0] else {
            panic!()
        };
        assert_eq!(o.name.text, "acknowledge");
        assert!(o.params.is_empty());
    }

    #[test]
    fn long_typed_attribute_propagates_type_to_handler_param() {
        let i = iface("I", alloc::vec![attr("counter", long_ty(), true)]);
        let out = transform_interface(&i);
        let Export::Op(o) = &out.reply_handler.exports[0] else {
            panic!()
        };
        assert_eq!(o.name.text, "get_counter");
        assert_eq!(o.params[0].type_spec, long_ty());
    }

    // ---- Phase-B Cluster 3 — Spec §7.5 ReplyHandler-Inheritance ----

    fn iface_with_base(name: &str, base: &str, exports: Vec<Export>) -> InterfaceDef {
        InterfaceDef {
            kind: InterfaceKind::Plain,
            name: Identifier::new(name, Span::SYNTHETIC),
            bases: alloc::vec![ScopedName::single(Identifier::new(base, Span::SYNTHETIC))],
            exports,
            annotations: Vec::new(),
            span: Span::SYNTHETIC,
        }
    }

    #[test]
    fn derived_iface_handler_inherits_from_base_handler_when_known() {
        // Spec §7.5: the ReplyHandler of a derived interface inherits
        // from AMI4CCM_<Base>ReplyHandler instead of CCM_AMI::ReplyHandler.
        let mut ctx = TransformContext::new();
        ctx.mark_transformed("Base");
        let derived = iface_with_base(
            "Derived",
            "Base",
            alloc::vec![op(
                "ping",
                Some(long_ty()),
                alloc::vec![(ParamAttribute::In, long_ty(), "v")]
            )],
        );
        let out = transform_interface_in_context(&derived, &ctx);
        assert_eq!(out.reply_handler.bases.len(), 1);
        let parts = out.reply_handler.bases[0]
            .parts
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            parts,
            alloc::vec!["AMI4CCM_BaseReplyHandler"],
            "expected derived ReplyHandler to inherit from AMI4CCM_BaseReplyHandler, got {parts:?}"
        );
    }

    #[test]
    fn derived_iface_falls_back_to_ccm_ami_when_base_unknown() {
        // If the base is not recorded in known_bases, the default
        // inheritance rule applies.
        let ctx = TransformContext::new();
        let derived = iface_with_base("Derived", "UnknownBase", alloc::vec![]);
        let out = transform_interface_in_context(&derived, &ctx);
        let parts = out.reply_handler.bases[0]
            .parts
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(parts, alloc::vec!["CCM_AMI", "ReplyHandler"]);
    }

    // ---- Phase-B Cluster 3 — Spec §7.5 AMI_-Prefix Naming-Resolver ----

    #[test]
    fn ami4ccm_prefix_collision_inserts_ami_prefix() {
        // Spec §7.5 / §7.3.1: if AMI4CCM_<Iface> or AMI4CCM_<Iface>
        // ReplyHandler already exists as an identifier in the
        // compilation scope, prefix with AMI_ recursively.
        let mut ctx = TransformContext::new();
        ctx.add_known_symbol("AMI4CCM_Order");
        ctx.add_known_symbol("AMI4CCM_OrderReplyHandler");
        let i = iface("Order", alloc::vec![op("submit", None, alloc::vec![])]);
        let out = transform_interface_in_context(&i, &ctx);
        assert_eq!(out.ami_interface.name.text, "AMI_AMI4CCM_Order");
        assert_eq!(out.reply_handler.name.text, "AMI_AMI4CCM_OrderReplyHandler");
    }

    #[test]
    fn ami4ccm_prefix_collision_recurses_until_unique() {
        // If even the first prefix levels are taken, keep prefixing
        // recursively.
        let mut ctx = TransformContext::new();
        for layer in [
            "AMI4CCM_Order",
            "AMI_AMI4CCM_Order",
            "AMI_AMI_AMI4CCM_Order",
        ] {
            ctx.add_known_symbol(layer);
        }
        let i = iface("Order", alloc::vec![op("submit", None, alloc::vec![])]);
        let out = transform_interface_in_context(&i, &ctx);
        assert_eq!(out.ami_interface.name.text, "AMI_AMI_AMI_AMI4CCM_Order");
    }

    #[test]
    fn transform_context_mark_transformed_sets_known_symbols_and_bases() {
        let mut ctx = TransformContext::new();
        ctx.mark_transformed("Foo");
        assert!(ctx.known_bases.contains("Foo"));
        assert!(ctx.known_symbols.contains("AMI4CCM_Foo"));
        assert!(ctx.known_symbols.contains("AMI4CCM_FooReplyHandler"));
    }
}
