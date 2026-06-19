// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Client AMI codegen (CORBA Messaging §22): the *implied IDL* for asynchronous
//! invocations. For an interface marked with `@ami` (§8.3.6.3) (or individual
//! `@ami` operations) this pass emits, in addition to the synchronous stub:
//!
//! * **Callback model** (§22.5): an `<Iface>AmiHandler` trait (`<op>` for the
//!   reply, `<op>_excep` for faults) + `sendc_<op>(channel, handler, in-args…)`
//!   on the stub.
//! * **Polling model** (§22.6): `sendp_<op>(channel, in-args…) -> <Iface><Op>Poller`
//!   + a typed poller with `get_reply(channel)`.
//!
//! Both models run over the abstract [`AsyncCorbaChannel`](crate::AsyncCorbaChannel),
//! which the transport (`AmiClient`) implements.

use zerodds_idl::ast::types::{Export, InterfaceDef, OpDecl, ParamAttribute};

use crate::error::Result;

/// Inline Rust literal for a CORBA MARSHAL system exception (CDR error).
const MARSHAL_EXC: &str = "zerodds_corba_rust::CorbaException::SystemException { minor: 0u32, message: \"CORBA MARSHAL: CDR error\" }";

/// Whether an operation carries `@ami`.
fn op_has_ami(op: &OpDecl) -> bool {
    op.annotations
        .iter()
        .any(|a| a.name.parts.last().is_some_and(|p| p.text == "ami"))
}

/// Whether the interface is AMI-enabled (`@ami` on the interface or on an operation).
fn iface_has_ami(i: &InterfaceDef) -> bool {
    let on_iface = i
        .annotations
        .iter()
        .any(|a| a.name.parts.last().is_some_and(|p| p.text == "ami"));
    on_iface
        || i.exports.iter().any(|e| match e {
            Export::Op(op) => op_has_ami(op),
            _ => false,
        })
}

/// AMI operations of an interface (oneway is excluded from AMI, §22).
fn ami_ops(i: &InterfaceDef) -> Vec<&OpDecl> {
    let iface_ami = i
        .annotations
        .iter()
        .any(|a| a.name.parts.last().is_some_and(|p| p.text == "ami"));
    i.exports
        .iter()
        .filter_map(|e| match e {
            Export::Op(op) if !op.oneway && (iface_ami || op_has_ami(op)) => Some(op),
            _ => None,
        })
        .collect()
}

/// In and InOut params as stub method parameters (`name: Ty`, by value — the
/// InOut input value is marshalled on send, the output comes via handler/poller).
fn in_params(op: &OpDecl) -> Result<String> {
    let mut out = String::new();
    for p in &op.params {
        if matches!(p.attribute, ParamAttribute::In | ParamAttribute::InOut) {
            let ty = zerodds_idl_rust::type_map::rust_type_for(&p.type_spec)?;
            out.push_str(&format!(", {}: {ty}", p.name.text));
        }
    }
    Ok(out)
}

/// Out and InOut params (those that come back in the reply): `(name, rust_type)`.
fn out_params(op: &OpDecl) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for p in &op.params {
        if matches!(p.attribute, ParamAttribute::Out | ParamAttribute::InOut) {
            let ty = zerodds_idl_rust::type_map::rust_type_for(&p.type_spec)?;
            out.push((p.name.text.clone(), ty));
        }
    }
    Ok(out)
}

/// Emits in-arg marshalling (in/inout) into `__w`. `err` = error expression.
fn emit_in_marshal(out: &mut String, op: &OpDecl, indent: &str) {
    for p in &op.params {
        if matches!(p.attribute, ParamAttribute::In | ParamAttribute::InOut) {
            out.push_str(&format!(
                "{indent}<_ as zerodds_cdr::CdrEncode>::encode(&{}, &mut __w).map_err(|_| {MARSHAL_EXC})?;\n",
                p.name.text
            ));
        }
    }
}

/// Emits the AMI code for an interface (nothing if not AMI-enabled).
pub fn emit_ami(out: &mut String, i: &InterfaceDef) -> Result<()> {
    if !iface_has_ami(i) {
        return Ok(());
    }
    let ops = ami_ops(i);
    if ops.is_empty() {
        return Ok(());
    }
    let iface = &i.name.text;
    let stub = format!("{iface}Stub");

    // 1. ReplyHandler trait (callback model, §22.5).
    out.push_str(&format!(
        "/// AMI ReplyHandler for `{iface}` (CORBA Messaging §22.5): per operation a\n/// success callback `<op>` and a fault callback `<op>_excep`.\n"
    ));
    out.push_str(&format!(
        "pub trait {iface}AmiHandler: ::core::marker::Send + ::core::marker::Sync {{\n"
    ));
    for op in &ops {
        let m = &op.name.text;
        let mut sig = String::new();
        if let Some(ts) = &op.return_type {
            let ty = zerodds_idl_rust::type_map::rust_type_for(ts)?;
            sig.push_str(&format!(", __return: {ty}"));
        }
        for (name, ty) in out_params(op)? {
            sig.push_str(&format!(", {name}: {ty}"));
        }
        out.push_str(&format!(
            "    /// Reply for `{m}` (return value + out/inout args, decl order).\n"
        ));
        out.push_str(&format!("    fn {m}(&self{sig});\n"));
        out.push_str(&format!(
            "    /// Fault for `{m}` (system or user exception via ExceptionHolder).\n"
        ));
        out.push_str(&format!(
            "    fn {m}_excep(&self, __excep: zerodds_corba_rust::CorbaException);\n"
        ));
    }
    out.push_str("}\n\n");

    // 2. Poller structs per operation (polling model, §22.6).
    for op in &ops {
        let m = &op.name.text;
        let poller = format!("{iface}{}Poller", up_first(m));
        out.push_str(&format!(
            "/// AMI poller for `{iface}::{m}` (§22.6): holds the `request_id`; `get_reply`\n/// drives the channel until the reply and decodes it typed.\n"
        ));
        out.push_str(&format!(
            "pub struct {poller} {{\n    /// GIOP `request_id` of the pending invocation.\n    pub request_id: u32,\n}}\n\n"
        ));
        out.push_str(&format!("impl {poller} {{\n"));
        let ret_ty = poller_return_type(iface, op)?;
        out.push_str(&format!(
            "    /// Fetches the reply (blocking until it arrives) and decodes it.\n    ///\n    /// # Errors\n    /// CORBA exception (system/user) or transport error.\n    pub fn get_reply(&self, __channel: &mut dyn zerodds_corba_rust::AsyncCorbaChannel) -> ::core::result::Result<{ret_ty}, zerodds_corba_rust::CorbaException> {{\n"
        ));
        out.push_str("        let (__b, __re) = __channel.get_reply(self.request_id)??;\n");
        out.push_str("        let mut __r = zerodds_cdr::BufferReader::new(&__b, __re);\n");
        emit_reply_decode(out, op, "        ", "        ", false)?;
        out.push_str(&format!(
            "        ::core::result::Result::Ok({})\n",
            poller_return_expr(op)?
        ));
        out.push_str("    }\n}\n\n");
    }

    // 3. sendc_/sendp_ on the stub.
    out.push_str(&format!(
        "/// AMI invocations on `{stub}` (CORBA Messaging §22).\nimpl {stub} {{\n"
    ));
    for op in &ops {
        let m = &op.name.text;
        let inp = in_params(op)?;
        // --- sendc_<op> (Callback) ---
        out.push_str(&format!(
            "    /// Asynchronous callback call of `{m}` (§22.5). Returns the `request_id`.\n    ///\n    /// # Errors\n    /// Marshalling/transport error on send.\n"
        ));
        out.push_str(&format!(
            "    pub fn sendc_{m}(&self, __channel: &mut dyn zerodds_corba_rust::AsyncCorbaChannel, __handler: ::std::sync::Arc<dyn {iface}AmiHandler>{inp}) -> ::core::result::Result<u32, zerodds_corba_rust::CorbaException> {{\n"
        ));
        out.push_str(
            "        let mut __w = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Big);\n",
        );
        emit_in_marshal(out, op, "        ");
        out.push_str("        let __body = __w.into_bytes();\n");
        out.push_str(&format!(
            "        __channel.send(\"{m}\", &__body, ::std::boxed::Box::new(move |__reply| {{\n"
        ));
        out.push_str("            match __reply {\n");
        out.push_str("                ::core::result::Result::Ok((__b, __re)) => {\n");
        out.push_str(
            "                    let mut __r = zerodds_cdr::BufferReader::new(&__b, __re);\n",
        );
        // Decode with error → <op>_excep + return.
        emit_reply_decode(
            out,
            op,
            "                    ",
            "                    ",
            true,
        )?;
        let call_args = handler_call_args(op)?;
        out.push_str(&format!(
            "                    __handler.{m}({call_args});\n"
        ));
        out.push_str("                }\n");
        out.push_str(&format!(
            "                ::core::result::Result::Err(__exc) => __handler.{m}_excep(__exc),\n"
        ));
        out.push_str("            }\n");
        out.push_str("        }))\n");
        out.push_str("    }\n\n");

        // --- sendp_<op> (Polling) ---
        let poller = format!("{iface}{}Poller", up_first(m));
        out.push_str(&format!(
            "    /// Asynchronous polling call of `{m}` (§22.6). Returns a poller.\n    ///\n    /// # Errors\n    /// Marshalling/transport error on send.\n"
        ));
        out.push_str(&format!(
            "    pub fn sendp_{m}(&self, __channel: &mut dyn zerodds_corba_rust::AsyncCorbaChannel{inp}) -> ::core::result::Result<{poller}, zerodds_corba_rust::CorbaException> {{\n"
        ));
        out.push_str(
            "        let mut __w = zerodds_cdr::BufferWriter::new(zerodds_cdr::Endianness::Big);\n",
        );
        emit_in_marshal(out, op, "        ");
        out.push_str(&format!(
            "        let __id = __channel.send_poll(\"{m}\", &__w.into_bytes())?;\n        ::core::result::Result::Ok({poller} {{ request_id: __id }})\n    }}\n\n"
        ));
    }
    out.push_str("}\n\n");
    Ok(())
}

/// Decodes return + out/inout into local `__ret`/`<name>` bindings. With
/// `in_callback`, a decode error leads to `__handler.<op>_excep(MARSHAL)+return`,
/// otherwise to `return Err(MARSHAL)`.
fn emit_reply_decode(
    out: &mut String,
    op: &OpDecl,
    indent: &str,
    _x: &str,
    in_callback: bool,
) -> Result<()> {
    let m = &op.name.text;
    let on_err = if in_callback {
        format!("{{ __handler.{m}_excep({MARSHAL_EXC}); return; }}")
    } else {
        format!("return ::core::result::Result::Err({MARSHAL_EXC})")
    };
    if let Some(ts) = &op.return_type {
        let ty = zerodds_idl_rust::type_map::rust_type_for(ts)?;
        out.push_str(&format!(
            "{indent}let __ret = match <{ty} as zerodds_cdr::CdrDecode>::decode(&mut __r) {{ ::core::result::Result::Ok(__v) => __v, ::core::result::Result::Err(_) => {on_err} }};\n"
        ));
    }
    for (name, ty) in out_params(op)? {
        out.push_str(&format!(
            "{indent}let {name} = match <{ty} as zerodds_cdr::CdrDecode>::decode(&mut __r) {{ ::core::result::Result::Ok(__v) => __v, ::core::result::Result::Err(_) => {on_err} }};\n"
        ));
    }
    Ok(())
}

/// Argument list for the `<op>` handler call (return first, then out/inout).
fn handler_call_args(op: &OpDecl) -> Result<String> {
    let mut parts = Vec::new();
    if op.return_type.is_some() {
        parts.push("__ret".to_string());
    }
    for (name, _) in out_params(op)? {
        parts.push(name);
    }
    Ok(parts.join(", "))
}

/// Return type of `Poller::get_reply`: `()` / `RET` / tuple `(RET, out…)`.
fn poller_return_type(_iface: &str, op: &OpDecl) -> Result<String> {
    let mut parts = Vec::new();
    if let Some(ts) = &op.return_type {
        parts.push(zerodds_idl_rust::type_map::rust_type_for(ts)?);
    }
    for (_, ty) in out_params(op)? {
        parts.push(ty);
    }
    Ok(match parts.len() {
        0 => "()".to_string(),
        1 => parts.into_iter().next().unwrap_or_default(),
        _ => format!("({})", parts.join(", ")),
    })
}

/// Return expression of `Poller::get_reply` matching [`poller_return_type`].
fn poller_return_expr(op: &OpDecl) -> Result<String> {
    let mut parts = Vec::new();
    if op.return_type.is_some() {
        parts.push("__ret".to_string());
    }
    for (name, _) in out_params(op)? {
        parts.push(name);
    }
    Ok(match parts.len() {
        0 => "()".to_string(),
        1 => parts.into_iter().next().unwrap_or_default(),
        _ => format!("({})", parts.join(", ")),
    })
}

/// First character uppercased (for `<Iface><Op>Poller`).
fn up_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
