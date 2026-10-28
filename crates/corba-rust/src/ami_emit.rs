// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Client AMI codegen (CORBA Messaging §22): the *implied IDL* for
//! asynchronous invocations. For an interface marked `@ami` (IDL 4.2
//! §8.3.5), or for individual `@ami` operations, this emits alongside
//! the synchronous stub:
//!
//! * **Callback model** (§22.5): an `<Iface>AmiHandler` trait (`<op>`
//!   for the reply, `<op>_excep` for a fault) plus
//!   `sendc_<op>(channel, handler, in-args…)` on the stub.
//! * **Polling model** (§22.6): `sendp_<op>(channel, in-args…)`
//!   returning an `<Iface><Op>Poller` with `get_reply(channel)`.
//!
//! Both run over [`AsyncCorbaChannel`](crate::AsyncCorbaChannel), which
//! the transport implements.
//!
//! ## Where `@ami` is read from
//!
//! [`CorbaFacet::is_ami`]. The previous version matched the last
//! segment of every applied annotation's name against the string
//! `"ami"`, which also answered `true` for a user-declared
//! `@mypkg::ami` that means something else. The frontend decides which
//! annotation an `@ami` is; a generator that re-decided it would be a
//! second answer.
//!
//! [`CorbaFacet::is_ami`]: zerodds_ast::CorbaFacet::is_ami

use zerodds_ast::{InterfaceNode, OperationNode, ParamDirection};
use zerodds_ast_core::Id;

use crate::emitter::Ctx;
use crate::error::{CorbaRustError, Result};
use crate::interface_emit::MARSHAL_EXC;
use crate::type_map::{read_of, rust_type_for, write_of};

/// Whether a node's CORBA facet carries `@ami`.
fn is_ami(facets: &zerodds_ast::Facets) -> bool {
    facets.corba.as_ref().is_some_and(|corba| corba.is_ami)
}

/// The operations an interface offers asynchronously.
///
/// `oneway` is excluded: §22 has no reply to deliver.
fn ami_ops(ctx: &Ctx<'_>, node: &InterfaceNode) -> Result<Vec<Id<OperationNode>>> {
    let on_interface = is_ami(&node.facets);
    let mut out = Vec::new();
    for id in &node.operations {
        let op = ctx
            .ast
            .operation(*id)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "an operation",
            })?;
        if !op.oneway && (on_interface || is_ami(&op.facets)) {
            out.push(*id);
        }
    }
    Ok(out)
}

/// `in` and `inout` parameters as stub arguments, by value: the `inout`
/// input is marshalled on send and its output comes back through the
/// handler or the poller.
fn in_params(ctx: &Ctx<'_>, op: &OperationNode) -> Result<String> {
    let mut out = String::new();
    for id in &op.params {
        let param = ctx
            .ast
            .param(*id)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a parameter",
            })?;
        if matches!(param.direction, ParamDirection::In | ParamDirection::InOut) {
            let type_ = rust_type_for(ctx.ast, &param.type_)?;
            out.push_str(&format!(", {}: {type_}", param.name));
        }
    }
    Ok(out)
}

/// The parameters that come back in the reply, as `(name, rust_type)`.
fn out_params(ctx: &Ctx<'_>, op: &OperationNode) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for id in &op.params {
        let param = ctx
            .ast
            .param(*id)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a parameter",
            })?;
        if matches!(param.direction, ParamDirection::Out | ParamDirection::InOut) {
            out.push((param.name.clone(), rust_type_for(ctx.ast, &param.type_)?));
        }
    }
    Ok(out)
}

/// Marshals the `in` and `inout` arguments into `__w`.
fn emit_in_marshal(
    out: &mut String,
    ctx: &Ctx<'_>,
    op: &OperationNode,
    indent: &str,
) -> Result<()> {
    for id in &op.params {
        let param = ctx
            .ast
            .param(*id)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a parameter",
            })?;
        if matches!(param.direction, ParamDirection::In | ParamDirection::InOut) {
            let write = write_of(ctx.ast, &param.type_, "__w", &param.name)?;
            out.push_str(&format!("{indent}{}\n", write.statement(MARSHAL_EXC)));
        }
    }
    Ok(())
}

/// Emits the AMI code for an interface, and nothing when it has none.
pub fn emit_ami(out: &mut String, ctx: &Ctx<'_>, id: Id<InterfaceNode>) -> Result<()> {
    let node = crate::interface_emit::interface_of(ctx, id)?;
    let ops = ami_ops(ctx, node)?;
    if ops.is_empty() {
        return Ok(());
    }
    let iface = node.name.clone();
    let stub = format!("{iface}Stub");

    // 1. The reply handler (callback model, §22.5).
    out.push_str(&format!(
        "/// AMI ReplyHandler for `{iface}` (CORBA Messaging §22.5): per operation a\n/// success callback `<op>` and a fault callback `<op>_excep`.\n"
    ));
    out.push_str(&format!(
        "pub trait {iface}AmiHandler: ::core::marker::Send + ::core::marker::Sync {{\n"
    ));
    for op_id in &ops {
        let op = ctx
            .ast
            .operation(*op_id)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "an operation",
            })?;
        let method = &op.name;
        let mut signature = String::new();
        if let Some(type_) = &op.return_type {
            signature.push_str(&format!(", __return: {}", rust_type_for(ctx.ast, type_)?));
        }
        for (name, type_) in out_params(ctx, op)? {
            signature.push_str(&format!(", {name}: {type_}"));
        }
        out.push_str(&format!(
            "    /// Reply for `{method}` (return value + out/inout args, decl order).\n"
        ));
        out.push_str(&format!("    fn {method}(&self{signature});\n"));
        out.push_str(&format!(
            "    /// Fault for `{method}` (system or user exception via ExceptionHolder).\n"
        ));
        out.push_str(&format!(
            "    fn {method}_excep(&self, __excep: zerodds_corba_rust::CorbaException);\n"
        ));
    }
    out.push_str("}\n\n");

    // 2. One poller per operation (polling model, §22.6).
    for op_id in &ops {
        let op = ctx
            .ast
            .operation(*op_id)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "an operation",
            })?;
        let method = &op.name;
        let poller = format!("{iface}{}Poller", up_first(method));
        out.push_str(&format!(
            "/// AMI poller for `{iface}::{method}` (§22.6): holds the `request_id`; `get_reply`\n/// drives the channel until the reply and decodes it typed.\n"
        ));
        out.push_str(&format!(
            "pub struct {poller} {{\n    /// GIOP `request_id` of the pending invocation.\n    pub request_id: u32,\n}}\n\n"
        ));
        out.push_str(&format!("impl {poller} {{\n"));
        let returns = poller_return_type(ctx, op)?;
        out.push_str(&format!(
            "    /// Fetches the reply (blocking until it arrives) and decodes it.\n    ///\n    /// # Errors\n    /// CORBA exception (system/user) or transport error.\n    pub fn get_reply(&self, __channel: &mut dyn zerodds_corba_rust::AsyncCorbaChannel) -> ::core::result::Result<{returns}, zerodds_corba_rust::CorbaException> {{\n"
        ));
        out.push_str("        let (__b, __re) = __channel.get_reply(self.request_id)??;\n");
        out.push_str("        let mut __r = zerodds_wire::Reader::new(&__b, __re);\n");
        emit_reply_decode(out, ctx, op, "        ", false)?;
        out.push_str(&format!(
            "        ::core::result::Result::Ok({})\n",
            poller_return_expr(ctx, op)?
        ));
        out.push_str("    }\n}\n\n");
    }

    // 3. `sendc_` and `sendp_` on the stub.
    out.push_str(&format!(
        "/// AMI invocations on `{stub}` (CORBA Messaging §22).\nimpl {stub} {{\n"
    ));
    for op_id in &ops {
        let op = ctx
            .ast
            .operation(*op_id)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "an operation",
            })?;
        let method = &op.name;
        let inputs = in_params(ctx, op)?;

        // sendc_<op> — callback.
        out.push_str(&format!(
            "    /// Asynchronous callback call of `{method}` (§22.5). Returns the `request_id`.\n    ///\n    /// # Errors\n    /// Marshalling/transport error on send.\n"
        ));
        out.push_str(&format!(
            "    pub fn sendc_{method}(&self, __channel: &mut dyn zerodds_corba_rust::AsyncCorbaChannel, __handler: ::std::sync::Arc<dyn {iface}AmiHandler>{inputs}) -> ::core::result::Result<u32, zerodds_corba_rust::CorbaException> {{\n"
        ));
        out.push_str(
            "        let mut __w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Big);\n",
        );
        emit_in_marshal(out, ctx, op, "        ")?;
        out.push_str("        let __body = __w.finish();\n");
        out.push_str(&format!(
            "        __channel.send(\"{method}\", &__body, ::std::boxed::Box::new(move |__reply| {{\n"
        ));
        out.push_str("            match __reply {\n");
        out.push_str("                ::core::result::Result::Ok((__b, __re)) => {\n");
        out.push_str("                    let mut __r = zerodds_wire::Reader::new(&__b, __re);\n");
        emit_reply_decode(out, ctx, op, "                    ", true)?;
        out.push_str(&format!(
            "                    __handler.{method}({});\n",
            handler_call_args(ctx, op)?
        ));
        out.push_str("                }\n");
        out.push_str(&format!(
            "                ::core::result::Result::Err(__exc) => __handler.{method}_excep(__exc),\n"
        ));
        out.push_str("            }\n");
        out.push_str("        }))\n");
        out.push_str("    }\n\n");

        // sendp_<op> — polling.
        let poller = format!("{iface}{}Poller", up_first(method));
        out.push_str(&format!(
            "    /// Asynchronous polling call of `{method}` (§22.6). Returns a poller.\n    ///\n    /// # Errors\n    /// Marshalling/transport error on send.\n"
        ));
        out.push_str(&format!(
            "    pub fn sendp_{method}(&self, __channel: &mut dyn zerodds_corba_rust::AsyncCorbaChannel{inputs}) -> ::core::result::Result<{poller}, zerodds_corba_rust::CorbaException> {{\n"
        ));
        out.push_str(
            "        let mut __w = zerodds_wire::Writer::new(zerodds_wire::Endianness::Big);\n",
        );
        emit_in_marshal(out, ctx, op, "        ")?;
        out.push_str(&format!(
            "        let __id = __channel.send_poll(\"{method}\", &__w.finish())?;\n        ::core::result::Result::Ok({poller} {{ request_id: __id }})\n    }}\n\n"
        ));
    }
    out.push_str("}\n\n");
    Ok(())
}

/// Decodes the return value and the `out`/`inout` arguments into local
/// bindings. Inside a callback a refused decode reaches
/// `__handler.<op>_excep` and returns; elsewhere it returns the error.
fn emit_reply_decode(
    out: &mut String,
    ctx: &Ctx<'_>,
    op: &OperationNode,
    indent: &str,
    in_callback: bool,
) -> Result<()> {
    let method = &op.name;
    let on_error = if in_callback {
        format!("{{ __handler.{method}_excep({MARSHAL_EXC}); return; }}")
    } else {
        format!("return ::core::result::Result::Err({MARSHAL_EXC})")
    };
    if let Some(type_) = &op.return_type {
        let read = read_of(ctx.ast, type_, "__r")?;
        out.push_str(&format!(
            "{indent}let __ret = match {read} {{ ::core::result::Result::Ok(__v) => __v, ::core::result::Result::Err(_) => {on_error} }};\n"
        ));
    }
    for id in &op.params {
        let param = ctx
            .ast
            .param(*id)
            .map_err(|_| CorbaRustError::StaleHandle {
                what: "a parameter",
            })?;
        if matches!(param.direction, ParamDirection::Out | ParamDirection::InOut) {
            let read = read_of(ctx.ast, &param.type_, "__r")?;
            let name = &param.name;
            out.push_str(&format!(
                "{indent}let {name} = match {read} {{ ::core::result::Result::Ok(__v) => __v, ::core::result::Result::Err(_) => {on_error} }};\n"
            ));
        }
    }
    Ok(())
}

/// The argument list of the `<op>` handler call: the return value
/// first, then `out` and `inout` in declaration order.
fn handler_call_args(ctx: &Ctx<'_>, op: &OperationNode) -> Result<String> {
    let mut parts = Vec::new();
    if op.return_type.is_some() {
        parts.push(String::from("__ret"));
    }
    for (name, _) in out_params(ctx, op)? {
        parts.push(name);
    }
    Ok(parts.join(", "))
}

/// What `Poller::get_reply` returns: `()`, the return type, or a tuple.
fn poller_return_type(ctx: &Ctx<'_>, op: &OperationNode) -> Result<String> {
    let mut parts = Vec::new();
    if let Some(type_) = &op.return_type {
        parts.push(rust_type_for(ctx.ast, type_)?);
    }
    for (_, type_) in out_params(ctx, op)? {
        parts.push(type_);
    }
    Ok(join_tuple(parts))
}

/// The expression matching [`poller_return_type`].
fn poller_return_expr(ctx: &Ctx<'_>, op: &OperationNode) -> Result<String> {
    let mut parts = Vec::new();
    if op.return_type.is_some() {
        parts.push(String::from("__ret"));
    }
    for (name, _) in out_params(ctx, op)? {
        parts.push(name);
    }
    Ok(join_tuple(parts))
}

/// Nothing, one thing, or a tuple of things.
fn join_tuple(parts: Vec<String>) -> String {
    match parts.len() {
        0 => String::from("()"),
        1 => parts.into_iter().next().unwrap_or_default(),
        _ => format!("({})", parts.join(", ")),
    }
}

/// First character uppercased, for `<Iface><Op>Poller`.
fn up_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
