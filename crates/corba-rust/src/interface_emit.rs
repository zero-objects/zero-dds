// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL `interface` → Rust trait + stub + skeleton.
//!
//! Mapping:
//! - `interface I { ... };` → `pub trait I { fn op(...) -> Result<...>; }`
//!   plus `pub struct IStub` (client) and `pub fn dispatch_<i>(...)`
//!   (server skeleton).
//! - `attribute T x` → trait method `fn x(&self)` and, when not
//!   `readonly`, `fn set_x(&self, value: T)`.
//! - `op(in T x, out T y, inout T z)` → a trait method taking `in` by
//!   value and `out`/`inout` as `&mut`.
//! - `oneway op(...)` → a trait method with no reply body.
//! - `raises (E1, E2)` → a per-interface `{I}Error` enum.
//!
//! ## What the marshalling looks like now
//!
//! The generated stub used to call `<T as zerodds_cdr::CdrEncode>::encode`
//! for every argument, because `zerodds-cdr` had a trait with an
//! implementation for each primitive. `zerodds-wire` has no such trait
//! — it has a `Writer` with a method per primitive — and the model says
//! which one an argument needs. So a `long` argument is
//! `__w.write_i32(a);` and a declared type is `a.encode(&mut __w)?`,
//! which is the inherent method `zerodds-lang-rules` generates for it.
//!
//! Two consequences, both visible in the output: a primitive write
//! carries no error path, because `zerodds-wire`'s writer cannot refuse
//! one; and the generated code names one crate fewer.

use std::collections::BTreeSet;

use zerodds_ast::{AnyId, AttributeNode, InterfaceNode, OperationNode, ParamDirection};
use zerodds_ast_core::Id;

use crate::emitter::Ctx;
use crate::error::{CorbaRustError, Result};
use crate::type_map::{read_of, rust_type_for, write_of};

/// Inline Rust literal for a CORBA MARSHAL system exception.
pub(crate) const MARSHAL_EXC: &str = "zerodds_corba_rust::CorbaException::SystemException { minor: 0u32, message: \"CORBA MARSHAL: CDR error\" }";

/// Emits the trait, the stub and the skeleton of one interface.
pub fn emit_interface(out: &mut String, ctx: &Ctx<'_>, id: Id<InterfaceNode>) -> Result<()> {
    emit_interface_trait(out, ctx, id)?;
    out.push('\n');
    emit_interface_stub(out, ctx, id)?;
    out.push('\n');
    // Client AMI (CORBA Messaging §22): only for `@ami` interfaces and
    // operations.
    crate::ami_emit::emit_ami(out, ctx, id)?;
    emit_interface_skeleton(out, ctx, id)?;
    Ok(())
}

/// The interface node behind a handle, or the error that names it.
pub(crate) fn interface_of<'a>(ctx: &Ctx<'a>, id: Id<InterfaceNode>) -> Result<&'a InterfaceNode> {
    ctx.ast
        .interface(id)
        .map_err(|_| CorbaRustError::StaleHandle {
            what: "an interface",
        })
}

/// The operation node behind a handle.
fn operation_of<'a>(ctx: &Ctx<'a>, id: Id<OperationNode>) -> Result<&'a OperationNode> {
    ctx.ast
        .operation(id)
        .map_err(|_| CorbaRustError::StaleHandle {
            what: "an operation",
        })
}

/// The attribute node behind a handle.
fn attribute_of<'a>(ctx: &Ctx<'a>, id: Id<AttributeNode>) -> Result<&'a AttributeNode> {
    ctx.ast
        .attribute(id)
        .map_err(|_| CorbaRustError::StaleHandle {
            what: "an attribute",
        })
}

/// The simple names of the exceptions an operation raises, in
/// declaration order.
fn raises_of(ctx: &Ctx<'_>, op: &OperationNode) -> Vec<String> {
    op.raises
        .iter()
        .filter_map(|reference| reference.resolved())
        .filter_map(|id| ctx.ast.exception(id).ok())
        .map(|node| node.name.clone())
        .collect()
}

/// The members of an exception, by simple name, in declaration order.
///
/// A user exception's body is `string repository_id` followed by its
/// members, contiguously, from the start of the reply body — CORBA 3.3
/// part 2 §15.4.3. The members are marshalled **here** rather than
/// through the exception type's own `encode` and `decode`, because it
/// has none: `zerodds-lang-rules` declares an exception as a Rust
/// struct and `zerodds_lang_rules::encode::rust_codec` takes an
/// `Id<TypeNode>`, which an `ExceptionNode` is not. Calling a method
/// that does not exist is what `tests/compile_check.rs` reported for
/// `raises (NotFound)`.
///
/// Generating the two here is also where they belong: the framing is
/// the CORBA reply body's and the data column has no notion of one.
fn exception_members(
    ctx: &Ctx<'_>,
    name: &str,
) -> Result<Vec<(String, zerodds_ast_core::Ref<zerodds_ast::TypeNode>)>> {
    let Some((_, node)) = ctx.ast.exceptions().find(|(_, node)| node.name == name) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for member in &node.members {
        let member = ctx
            .ast
            .member(*member)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "an exception member",
            })?;
        out.push((member.name.clone(), member.type_.clone()));
    }
    Ok(out)
}

fn emit_interface_trait(out: &mut String, ctx: &Ctx<'_>, id: Id<InterfaceNode>) -> Result<()> {
    let node = interface_of(ctx, id)?;
    let name = &node.name;
    out.push_str("/// Generated by `zerodds-corba-rust` from IDL interface.\n");
    emit_interface_exceptions_enum(out, ctx, id)?;

    // §10.7.3.1: the repository identifier is the model's, which
    // carries `#pragma prefix`, `#pragma version` and `#pragma ID`.
    let repo_id = repository_id_of_node(ctx, id, node);

    let mut bases = node
        .bases
        .iter()
        .filter_map(|reference| reference.resolved())
        .filter_map(|base| ctx.ast.interface(base).ok())
        .map(|base| base.name.clone());
    let bases_clause = if let Some(first) = bases.next() {
        let mut clause = format!(": {first} + ::core::marker::Send + ::core::marker::Sync");
        for base in bases {
            clause.push_str(&format!(" + {base}"));
        }
        clause
    } else {
        String::from(": ::core::marker::Send + ::core::marker::Sync")
    };

    // A free `pub const` and not a trait constant: the skeleton
    // dispatches over `&dyn {name}`, and a trait with an associated
    // constant is not dyn-compatible.
    let repo_const = name.to_ascii_uppercase();
    out.push_str(&format!(
        "/// CORBA Repository-ID for `{name}` (spec §10.7.3.1).\npub const {repo_const}_REPOSITORY_ID: &str = \"{repo_id}\";\n"
    ));
    out.push_str(&format!("pub trait {name}{bases_clause} {{\n"));
    for op in &node.operations {
        emit_op_trait_method(out, ctx, name, *op)?;
    }
    for attr in &node.attributes {
        emit_attr_trait_method(out, ctx, *attr)?;
    }
    out.push_str("}\n");
    Ok(())
}

/// One exception enum per interface, over every `raises` list it has.
/// Spec §3.5.
fn emit_interface_exceptions_enum(
    out: &mut String,
    ctx: &Ctx<'_>,
    id: Id<InterfaceNode>,
) -> Result<()> {
    let node = interface_of(ctx, id)?;
    let name = &node.name;
    let mut all: BTreeSet<String> = BTreeSet::new();
    for op in &node.operations {
        all.extend(raises_of(ctx, operation_of(ctx, *op)?));
    }
    if all.is_empty() {
        return Ok(());
    }
    out.push_str(&format!(
        "/// Exception enum for `{name}` — unifies all user exceptions referenced in `raises` lists plus the system exception (spec §3.5).\n"
    ));
    out.push_str(&format!("pub enum {name}Error {{\n"));
    out.push_str(
        "    /// CORBA-System-Exception (BAD_OPERATION, MARSHAL, OBJECT_NOT_EXIST, ...).\n",
    );
    out.push_str("    System(zerodds_corba_rust::CorbaException),\n");
    for raise in &all {
        out.push_str(&format!(
            "    /// User-Exception `{raise}` (raised by `{name}`-Operations).\n"
        ));
        out.push_str(&format!("    {raise}({raise}),\n"));
    }
    out.push_str("}\n\n");
    // `Debug` written out rather than derived, and no `Clone` at all.
    // The exception types are the data column's, and
    // `zerodds-lang-rules` derives nothing on what it declares —
    // `adapter::cst` sets `derives: Vec::new()` on every struct and
    // enum, because a derive is a decision about the target language.
    // `#[derive(Debug, Clone)]` here therefore named two traits the
    // payload does not implement, which is what
    // `tests/compile_check.rs` reported for `raises (NotFound)`.
    //
    // The variant name and nothing else: printing the payload would
    // need the payload to be `Debug`, which is the same requirement
    // one level down.
    out.push_str(&format!(
        "impl ::core::fmt::Debug for {name}Error {{\n    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {{\n        match self {{\n"
    ));
    out.push_str(&format!(
        "            Self::System(__e) => write!(f, \"{name}Error::System({{__e}})\"),\n"
    ));
    for raise in &all {
        out.push_str(&format!(
            "            Self::{raise}(_) => f.write_str(\"{name}Error::{raise}\"),\n"
        ));
    }
    out.push_str("        }\n    }\n}\n\n");
    Ok(())
}

/// The Rust parameter list of an operation.
fn render_params(ctx: &Ctx<'_>, op: &OperationNode) -> Result<String> {
    let mut out = String::new();
    for (index, param) in op.params.iter().enumerate() {
        let param = ctx
            .ast
            .param(*param)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a parameter",
            })?;
        if index > 0 {
            out.push_str(", ");
        }
        let type_ = rust_type_for(ctx.ast, &param.type_)?;
        let name = &param.name;
        match param.direction {
            ParamDirection::In => out.push_str(&format!("{name}: {type_}")),
            ParamDirection::Out | ParamDirection::InOut => {
                out.push_str(&format!("{name}: &mut {type_}"));
            }
        }
    }
    Ok(out)
}

/// The error type of an operation: the typed per-interface enum when it
/// raises anything, and `CorbaException` when only a system exception
/// is possible. Spec §3.5.
fn op_error_type(owner: &str, raises: &[String]) -> String {
    if raises.is_empty() {
        String::from("zerodds_corba_rust::CorbaException")
    } else {
        format!("{owner}Error")
    }
}

fn render_return(ctx: &Ctx<'_>, owner: &str, op: &OperationNode) -> Result<String> {
    let inner = match &op.return_type {
        None => String::from("()"),
        Some(type_) => rust_type_for(ctx.ast, type_)?,
    };
    let error = op_error_type(owner, &raises_of(ctx, op));
    Ok(format!("::core::result::Result<{inner}, {error}>"))
}

/// A trait method for an operation. Public so the valuetype emitter can
/// reuse it for an `operation` exported by a valuetype.
pub fn emit_op_trait_method(
    out: &mut String,
    ctx: &Ctx<'_>,
    owner: &str,
    id: Id<OperationNode>,
) -> Result<()> {
    let op = operation_of(ctx, id)?;
    let method = &op.name;
    // The receiver is `&self` throughout: the skeleton dispatches over
    // `&dyn Iface`, and `out`/`inout` are `&mut` *parameters*. A
    // servant with state uses interior mutability.
    let params = render_params(ctx, op)?;
    let returns = render_return(ctx, owner, op)?;
    let comma = if params.is_empty() { "" } else { ", " };
    out.push_str(&format!(
        "    /// IDL operation `{method}`.\n    fn {method}(&self{comma}{params}) -> {returns};\n"
    ));
    Ok(())
}

/// A trait method pair for an attribute.
pub fn emit_attr_trait_method(
    out: &mut String,
    ctx: &Ctx<'_>,
    id: Id<AttributeNode>,
) -> Result<()> {
    let attr = attribute_of(ctx, id)?;
    let name = &attr.name;
    let type_ = rust_type_for(ctx.ast, &attr.type_)?;

    out.push_str(&format!("    /// IDL attribute `{name}` getter.\n"));
    out.push_str(&format!(
        "    fn {name}(&self) -> ::core::result::Result<{type_}, zerodds_corba_rust::CorbaException>;\n"
    ));
    if !attr.readonly {
        out.push_str(&format!("    /// IDL attribute `{name}` setter.\n"));
        out.push_str(&format!(
            "    fn set_{name}(&self, value: {type_}) -> ::core::result::Result<(), zerodds_corba_rust::CorbaException>;\n"
        ));
    }
    Ok(())
}

fn emit_interface_stub(out: &mut String, ctx: &Ctx<'_>, id: Id<InterfaceNode>) -> Result<()> {
    let node = interface_of(ctx, id)?;
    let name = &node.name;
    let stub = format!("{name}Stub");
    out.push_str(&format!(
        "/// Client stub for `{name}` (sends GIOP requests to a remote object).\n"
    ));
    out.push_str(&format!("pub struct {stub} {{\n"));
    out.push_str("    /// Object reference (IOR) of the remote servant.\n");
    out.push_str("    pub object_ref: zerodds_corba_rust::ObjectReference,\n");
    out.push_str("    /// Connection handle for sending the GIOP request.\n");
    out.push_str(
        "    pub connection: ::std::sync::Arc<dyn zerodds_corba_rust::CorbaConnection + ::core::marker::Send + ::core::marker::Sync>,\n",
    );
    out.push_str("}\n\n");

    out.push_str(&format!("impl {stub} {{\n"));
    out.push_str("    /// Constructs a stub from an ObjectReference + connection.\n");
    out.push_str("    #[must_use]\n");
    out.push_str(
        "    pub fn new(object_ref: zerodds_corba_rust::ObjectReference, connection: ::std::sync::Arc<dyn zerodds_corba_rust::CorbaConnection + ::core::marker::Send + ::core::marker::Sync>) -> Self {\n",
    );
    out.push_str("        Self { object_ref, connection }\n");
    out.push_str("    }\n");
    out.push_str("}\n\n");

    out.push_str(&format!("impl {name} for {stub} {{\n"));
    for op in &node.operations {
        emit_op_stub_impl(out, ctx, name, *op)?;
    }
    for attr in &node.attributes {
        emit_attr_stub_impl(out, ctx, *attr)?;
    }
    out.push_str("}\n");

    // §3.6: a `pub trait Derived: Base` bound is satisfied on the stub
    // only if the stub also implements every transitive base.
    let mut visited = BTreeSet::new();
    for base in node
        .bases
        .iter()
        .filter_map(zerodds_ast_core::Ref::resolved)
    {
        emit_base_impls_recursive(out, ctx, &stub, base, &mut visited)?;
    }
    Ok(())
}

/// zerodds-lint: recursion-depth 16 (interface inheritance; cycle-guarded)
fn emit_base_impls_recursive(
    out: &mut String,
    ctx: &Ctx<'_>,
    stub: &str,
    base: Id<InterfaceNode>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    let node = interface_of(ctx, base)?;
    if node.name.is_empty() || !visited.insert(node.name.clone()) {
        return Ok(());
    }
    let base_name = node.name.clone();
    out.push_str(&format!(
        "\n/// Base inheritance: `{base_name}` methods on `{stub}` (Spec §3.6).\n"
    ));
    out.push_str(&format!("impl {base_name} for {stub} {{\n"));
    for op in &node.operations {
        emit_op_stub_impl(out, ctx, &base_name, *op)?;
    }
    for attr in &node.attributes {
        emit_attr_stub_impl(out, ctx, *attr)?;
    }
    out.push_str("}\n");
    let grandbases: Vec<Id<InterfaceNode>> = node
        .bases
        .iter()
        .filter_map(zerodds_ast_core::Ref::resolved)
        .collect();
    for grandbase in grandbases {
        emit_base_impls_recursive(out, ctx, stub, grandbase, visited)?;
    }
    Ok(())
}

fn emit_op_stub_impl(
    out: &mut String,
    ctx: &Ctx<'_>,
    owner: &str,
    id: Id<OperationNode>,
) -> Result<()> {
    let op = operation_of(ctx, id)?;
    let method = &op.name;
    let params = render_params(ctx, op)?;
    let returns = render_return(ctx, owner, op)?;
    let comma = if params.is_empty() { "" } else { ", " };
    let raises = raises_of(ctx, op);
    let has_raises = !raises.is_empty();
    let marshal_err = if has_raises {
        format!("{owner}Error::System({MARSHAL_EXC})")
    } else {
        MARSHAL_EXC.to_string()
    };
    let wrap_sys = |inner: &str| -> String {
        if has_raises {
            format!("{owner}Error::System({inner})")
        } else {
            inner.to_string()
        }
    };
    out.push_str(&format!(
        "    fn {method}(&self{comma}{params}) -> {returns} {{\n"
    ));

    // 1. `in` and `inout` arguments into the request body, in
    //    declaration order.
    out.push_str("        let __e = zerodds_wire::Endianness::Big;\n");
    out.push_str("        let mut __w = zerodds_wire::Writer::new(__e);\n");
    for param in &op.params {
        let param = ctx
            .ast
            .param(*param)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a parameter",
            })?;
        let name = &param.name;
        match param.direction {
            ParamDirection::In => {
                let write = write_of(ctx.ast, &param.type_, "__w", name)?;
                out.push_str(&format!("        {}\n", write.statement(&marshal_err)));
            }
            ParamDirection::InOut => {
                let write = write_of(ctx.ast, &param.type_, "__w", &format!("(*{name})"))?;
                out.push_str(&format!("        {}\n", write.statement(&marshal_err)));
            }
            ParamDirection::Out => {}
        }
    }

    if op.oneway {
        let invocation = wrap_sys("__oe");
        out.push_str(&format!(
            "        self.connection.invoke_oneway(&self.object_ref, \"{method}\", __e, &__w.finish()).map_err(|__oe| {invocation})?;\n"
        ));
        out.push_str("        ::core::result::Result::Ok(())\n        }\n");
        return Ok(());
    }

    if has_raises {
        out.push_str(&format!(
            "        let (__reply, __re) = match self.connection.invoke(&self.object_ref, \"{method}\", __e, &__w.finish()) {{\n"
        ));
        out.push_str("            ::core::result::Result::Ok(__v) => __v,\n");
        out.push_str(
            "            ::core::result::Result::Err(zerodds_corba_rust::CorbaException::UserException { repository_id: __rid, body: __body, endianness: __xe }) => {\n",
        );
        for raise in &raises {
            let repo = ctx.exception_repository_id(raise);
            out.push_str(&format!("                if __rid == \"{repo}\" {{\n"));
            out.push_str(
                "                    let mut __xr = zerodds_wire::Reader::new(&__body, __xe);\n",
            );
            // The repository id is consumed first: it positions the
            // reader at the members, whose alignment counts from the
            // start of the body.
            out.push_str("                    let _ = __xr.read_string();\n");
            let members = exception_members(ctx, raise)?;
            for (member, type_) in &members {
                let read = read_of(ctx.ast, type_, "__xr")?;
                out.push_str(&format!(
                    "                    let __m_{member} = match {read} {{ ::core::result::Result::Ok(__v) => __v, ::core::result::Result::Err(_) => return ::core::result::Result::Err({owner}Error::System({MARSHAL_EXC})) }};\n"
                ));
            }
            let fields = members
                .iter()
                .map(|(member, _)| format!("{member}: __m_{member}"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "                    return ::core::result::Result::Err({owner}Error::{raise}({raise} {{ {fields} }}));\n"
            ));
            out.push_str("                }\n");
        }
        out.push_str(&format!(
            "                return ::core::result::Result::Err({owner}Error::System(zerodds_corba_rust::CorbaException::UserException {{ repository_id: __rid, body: __body, endianness: __xe }}));\n"
        ));
        out.push_str("            }\n");
        out.push_str(&format!(
            "            ::core::result::Result::Err(__other) => return ::core::result::Result::Err({owner}Error::System(__other)),\n"
        ));
        out.push_str("        };\n");
    } else {
        out.push_str(&format!(
            "        let (__reply, __re) = self.connection.invoke(&self.object_ref, \"{method}\", __e, &__w.finish())?;\n"
        ));
    }
    out.push_str("        let mut __r = zerodds_wire::Reader::new(&__reply, __re);\n");

    // 2. The reply: the return value if there is one, then `out` and
    //    `inout` in declaration order.
    if let Some(type_) = &op.return_type {
        let read = read_of(ctx.ast, type_, "__r")?;
        out.push_str(&format!(
            "        let __ret = {read}.map_err(|_| {marshal_err})?;\n"
        ));
    }
    for param in &op.params {
        let param = ctx
            .ast
            .param(*param)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a parameter",
            })?;
        if matches!(param.direction, ParamDirection::Out | ParamDirection::InOut) {
            let read = read_of(ctx.ast, &param.type_, "__r")?;
            let name = &param.name;
            out.push_str(&format!(
                "        *{name} = {read}.map_err(|_| {marshal_err})?;\n"
            ));
        }
    }
    if op.return_type.is_some() {
        out.push_str("        ::core::result::Result::Ok(__ret)\n");
    } else {
        out.push_str("        ::core::result::Result::Ok(())\n");
    }
    out.push_str("    }\n");
    Ok(())
}

fn emit_attr_stub_impl(out: &mut String, ctx: &Ctx<'_>, id: Id<AttributeNode>) -> Result<()> {
    let attr = attribute_of(ctx, id)?;
    let name = &attr.name;
    let type_ = rust_type_for(ctx.ast, &attr.type_)?;
    let read = read_of(ctx.ast, &attr.type_, "__r")?;

    out.push_str(&format!(
        "    fn {name}(&self) -> ::core::result::Result<{type_}, zerodds_corba_rust::CorbaException> {{\n"
    ));
    out.push_str("        let __e = zerodds_wire::Endianness::Big;\n");
    out.push_str("        let __w = zerodds_wire::Writer::new(__e);\n");
    out.push_str(&format!(
        "        let (__reply, __re) = self.connection.invoke(&self.object_ref, \"_get_{name}\", __e, &__w.finish())?;\n"
    ));
    out.push_str("        let mut __r = zerodds_wire::Reader::new(&__reply, __re);\n");
    out.push_str(&format!(
        "        ::core::result::Result::Ok({read}.map_err(|_| {MARSHAL_EXC})?)\n"
    ));
    out.push_str("    }\n");

    if !attr.readonly {
        let write = write_of(ctx.ast, &attr.type_, "__w", "value")?;
        out.push_str(&format!(
            "    fn set_{name}(&self, value: {type_}) -> ::core::result::Result<(), zerodds_corba_rust::CorbaException> {{\n"
        ));
        out.push_str("        let __e = zerodds_wire::Endianness::Big;\n");
        out.push_str("        let mut __w = zerodds_wire::Writer::new(__e);\n");
        out.push_str(&format!("        {}\n", write.statement(MARSHAL_EXC)));
        out.push_str(&format!(
            "        self.connection.invoke(&self.object_ref, \"_set_{name}\", __e, &__w.finish())?;\n"
        ));
        out.push_str("        ::core::result::Result::Ok(())\n");
        out.push_str("    }\n");
    }
    Ok(())
}

/// This interface's own repository id, the way `emit_interface_trait`
/// computes the `{NAME}_REPOSITORY_ID` constant (Spec §10.7.3.1).
fn repository_id_of_node(ctx: &Ctx<'_>, id: Id<InterfaceNode>, node: &InterfaceNode) -> String {
    zerodds_corba_codegen::repository_id_of(ctx.ast, AnyId::Interface(id, node.node_id))
        .map(|resolved| resolved.id)
        .unwrap_or_else(|| zerodds_corba_codegen::build_repository_id(&[], &node.name, 1, 0))
}

/// Every ancestor interface's repository id, transitively (CORBA
/// multiple inheritance) — what `_is_a` (CORBA 3.3 Part 2 §15.4.2) must
/// also answer `true` for, besides the interface's own id and
/// `IDL:omg.org/CORBA/Object:1.0` (`zerodds_corba_rust::runtime::
/// dispatch_object_operation` adds the latter itself).
///
/// zerodds-lint: recursion-depth 16 (interface inheritance; cycle-guarded)
fn collect_transitive_base_repository_ids(
    ctx: &Ctx<'_>,
    id: Id<InterfaceNode>,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<String>,
) -> Result<()> {
    let node = interface_of(ctx, id)?;
    for base in node
        .bases
        .iter()
        .filter_map(zerodds_ast_core::Ref::resolved)
    {
        let base_node = interface_of(ctx, base)?;
        if base_node.name.is_empty() || !seen.insert(base_node.name.clone()) {
            continue;
        }
        out.push(repository_id_of_node(ctx, base, base_node));
        collect_transitive_base_repository_ids(ctx, base, seen, out)?;
    }
    Ok(())
}

fn emit_interface_skeleton(out: &mut String, ctx: &Ctx<'_>, id: Id<InterfaceNode>) -> Result<()> {
    let node = interface_of(ctx, id)?;
    let name = &node.name;
    let repo_id = repository_id_of_node(ctx, id, node);
    let mut base_ids = Vec::new();
    collect_transitive_base_repository_ids(ctx, id, &mut BTreeSet::new(), &mut base_ids)?;
    let base_ids_literal = base_ids
        .iter()
        .map(|base_id| format!("\"{base_id}\""))
        .collect::<Vec<_>>()
        .join(", ");

    out.push_str(&format!("/// Server skeleton dispatch for `{name}`.\n"));
    out.push_str("/// Decodes the GIOP request body (in `__endianness`), calls the servant,\n");
    out.push_str("/// Encodes return + out/inout into the reply body (same order).\n");
    let lower = name.to_lowercase();
    out.push_str(&format!(
        "pub fn dispatch_{lower}(servant: &dyn {name}, operation: &str, __payload: &[u8], __endianness: zerodds_wire::Endianness) -> zerodds_corba_rust::SkeletonResult {{\n"
    ));
    // CORBA 3.3 Part 2 §15.4.2: `_interface`/`_is_a`/`_non_existent`/
    // `_not_existent`/`_domain_managers`/`_component`/`_repository_id`
    // answer without reaching the interface's own operations at all.
    out.push_str(&format!(
        "    if let Some(__r) = zerodds_corba_rust::runtime::dispatch_object_operation(operation, __payload, __endianness, \"{repo_id}\", &[{base_ids_literal}]) {{\n        return __r;\n    }}\n"
    ));
    out.push_str("    match operation {\n");
    for op in &node.operations {
        emit_op_skeleton_arm(out, ctx, name, *op)?;
    }
    for attr in &node.attributes {
        emit_attr_skeleton_arm(out, ctx, *attr)?;
    }
    out.push_str("        _ => zerodds_corba_rust::SkeletonResult::BadOperation,\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    Ok(())
}

/// The expression a skeleton arm returns when marshalling refuses.
fn marshal_failure() -> String {
    format!("zerodds_corba_rust::SkeletonResult::Exception({MARSHAL_EXC})")
}

fn emit_op_skeleton_arm(
    out: &mut String,
    ctx: &Ctx<'_>,
    owner: &str,
    id: Id<OperationNode>,
) -> Result<()> {
    let op = operation_of(ctx, id)?;
    let method = &op.name;
    let failure = marshal_failure();
    out.push_str(&format!("        \"{method}\" => {{\n"));
    out.push_str("            let mut __r = zerodds_wire::Reader::new(__payload, __endianness);\n");
    let mut call_args: Vec<String> = Vec::new();
    for param in &op.params {
        let param = ctx
            .ast
            .param(*param)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a parameter",
            })?;
        let name = &param.name;
        let type_ = rust_type_for(ctx.ast, &param.type_)?;
        match param.direction {
            ParamDirection::In => {
                let read = read_of(ctx.ast, &param.type_, "__r")?;
                out.push_str(&format!(
                    "            let {name} = match {read} {{ Ok(__v) => __v, Err(_) => return {failure} }};\n"
                ));
                call_args.push(name.clone());
            }
            ParamDirection::InOut => {
                let read = read_of(ctx.ast, &param.type_, "__r")?;
                out.push_str(&format!(
                    "            let mut {name} = match {read} {{ Ok(__v) => __v, Err(_) => return {failure} }};\n"
                ));
                call_args.push(format!("&mut {name}"));
            }
            ParamDirection::Out => {
                // A value of the type, built from the model. Not
                // `Default::default()`: see `type_map::zero_of`.
                let zero = crate::type_map::zero_of(ctx.ast, &param.type_)?;
                out.push_str(&format!("            let mut {name}: {type_} = {zero};\n"));
                call_args.push(format!("&mut {name}"));
            }
        }
    }
    let args = call_args.join(", ");
    out.push_str(&format!("            match servant.{method}({args}) {{\n"));
    out.push_str("                ::core::result::Result::Ok(__ret) => {\n");
    if op.oneway {
        out.push_str("                    let _ = __ret;\n");
        out.push_str("                    zerodds_corba_rust::SkeletonResult::Reply(::std::vec::Vec::new())\n");
    } else {
        out.push_str(
            "                    let mut __w = zerodds_corba_rust::runtime::reply_writer(__endianness);\n",
        );
        if let Some(type_) = &op.return_type {
            let write = write_of(ctx.ast, type_, "__w", "__ret")?;
            out.push_str(&format!(
                "                    {}\n",
                write.guarded(&failure)
            ));
        } else {
            out.push_str("                    let _ = __ret;\n");
        }
        for param in &op.params {
            let param = ctx
                .ast
                .param(*param)
                .map_err(|_| CorbaRustError::StaleHandle {
                    what: "a parameter",
                })?;
            if matches!(param.direction, ParamDirection::Out | ParamDirection::InOut) {
                let write = write_of(ctx.ast, &param.type_, "__w", &param.name)?;
                out.push_str(&format!(
                    "                    {}\n",
                    write.guarded(&failure)
                ));
            }
        }
        out.push_str(
            "                    zerodds_corba_rust::SkeletonResult::Reply(__w.finish())\n",
        );
    }
    out.push_str("                }\n");

    let raises = raises_of(ctx, op);
    if raises.is_empty() {
        out.push_str("                ::core::result::Result::Err(__exc) => zerodds_corba_rust::SkeletonResult::Exception(__exc),\n");
    } else {
        out.push_str("                ::core::result::Result::Err(__err) => match __err {\n");
        out.push_str(&format!(
            "                    {owner}Error::System(__e) => zerodds_corba_rust::SkeletonResult::Exception(__e),\n"
        ));
        for raise in &raises {
            let repo = ctx.exception_repository_id(raise);
            out.push_str(&format!(
                "                    {owner}Error::{raise}(__exc) => {{\n"
            ));
            out.push_str(
                "                        let mut __xw = zerodds_corba_rust::runtime::reply_writer(__endianness);\n",
            );
            // The repository id and the members go into one contiguous
            // body: §15.4.3 puts the whole UserException at the reply
            // body's start, and the members align from there.
            out.push_str(&format!(
                "                        __xw.write_string(\"{repo}\");\n"
            ));
            for (member, type_) in exception_members(ctx, raise)? {
                let write = write_of(ctx.ast, &type_, "__xw", &format!("__exc.{member}"))?;
                out.push_str(&format!(
                    "                        {}\n",
                    write.guarded(&failure)
                ));
            }
            out.push_str(&format!(
                "                        zerodds_corba_rust::SkeletonResult::Exception(zerodds_corba_rust::CorbaException::UserException {{ repository_id: \"{repo}\".to_string(), body: __xw.finish(), endianness: __endianness }})\n"
            ));
            out.push_str("                    }\n");
        }
        // The shared `{owner}Error` enum carries every operation's
        // variants; this one cannot produce the others, and the match
        // still has to be total.
        out.push_str(&format!("                    _ => {failure},\n"));
        out.push_str("                },\n");
    }
    out.push_str("            }\n        }\n");
    Ok(())
}

fn emit_attr_skeleton_arm(out: &mut String, ctx: &Ctx<'_>, id: Id<AttributeNode>) -> Result<()> {
    let attr = attribute_of(ctx, id)?;
    let name = &attr.name;
    let failure = marshal_failure();
    let write = write_of(ctx.ast, &attr.type_, "__w", "__ret")?;

    out.push_str(&format!("        \"_get_{name}\" => {{\n"));
    out.push_str(&format!("            match servant.{name}() {{\n"));
    out.push_str("                ::core::result::Result::Ok(__ret) => {\n");
    out.push_str("                    let mut __w = zerodds_corba_rust::runtime::reply_writer(__endianness);\n");
    out.push_str(&format!(
        "                    {}\n",
        write.guarded(&failure)
    ));
    out.push_str("                    zerodds_corba_rust::SkeletonResult::Reply(__w.finish())\n");
    out.push_str("                }\n");
    out.push_str("                ::core::result::Result::Err(__exc) => zerodds_corba_rust::SkeletonResult::Exception(__exc),\n");
    out.push_str("            }\n        }\n");

    if !attr.readonly {
        let read = read_of(ctx.ast, &attr.type_, "__r")?;
        out.push_str(&format!("        \"_set_{name}\" => {{\n"));
        out.push_str(
            "            let mut __r = zerodds_wire::Reader::new(__payload, __endianness);\n",
        );
        out.push_str(&format!(
            "            let __value = match {read} {{ Ok(__v) => __v, Err(_) => return {failure} }};\n"
        ));
        out.push_str(&format!(
            "            match servant.set_{name}(__value) {{\n"
        ));
        out.push_str("                ::core::result::Result::Ok(()) => zerodds_corba_rust::SkeletonResult::Reply(::std::vec::Vec::new()),\n");
        out.push_str("                ::core::result::Result::Err(__exc) => zerodds_corba_rust::SkeletonResult::Exception(__exc),\n");
        out.push_str("            }\n        }\n");
    }
    Ok(())
}
