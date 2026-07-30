// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL `@service` interface → C# RPC codegen (DDS-RPC 1.0 §7.11.2, C# PSM).
//!
//! Restores C# RPC to parity with the Java PSM (`idl-java/src/rpc.rs`): a
//! `@service`-annotated interface no longer degrades to a bare signature stub.
//! Per service we emit five members into the current namespace/compilation
//! unit:
//!
//! * `<Service>`          — synchronous interface (blocking methods).
//! * `<Service>Async`     — asynchronous interface (`Task`/`Task<T>` returns).
//! * `<Service>Service`   — server-side handler interface the implementor fills.
//! * `<Service>Requester` — client proxy: implements both interfaces, marshals
//!                          a type-erased `object[]` tuple through the runtime
//!                          `Zerodds.Rpc.IRequester`.
//! * `<Service>Replier`   — server dispatcher: decodes the request tuple, calls
//!                          the handler, packs the reply tuple.
//!
//! # Out / inout parameters
//! Mirroring the Java PSM's holder pattern, `out`/`inout` map to
//! `Zerodds.Rpc.Holder<T>` (a mutable single-field box) so the sync and async
//! signatures stay symmetric and the marshalling convention is identical.
//!
//! # Marshalling convention (identical to the Java PSM)
//!   * request payload = `object[]` of IN + INOUT values (declaration order)
//!   * reply payload   = `object[] { returnValue-or-null, INOUT+OUT values… }`
//!
//! The generated requester and replier are proven to compile against the real
//! `Zerodds.Rpc.*` runtime and to round-trip a request→dispatch→reply cycle by
//! `tests/rpc_codegen.rs` and `tests/rpc_marshalling.rs` (real `dotnet`).

use core::fmt::Write;

use zerodds_idl::ast::{InterfaceDef, ScopedName, TypeSpec};
use zerodds_rpc::annotations::lower_rpc_annotations;
use zerodds_rpc::service_mapping::{MethodDef, ParamDef, ParamDirection, lower_service};

use crate::emitter::{fmt_err, scoped_to_cs, typespec_to_cs};
use crate::error::CsGenError;
use crate::keywords::escape_identifier;

/// Emits the five RPC members for one `@service` interface into `out`.
///
/// `base` is the indentation of the enclosing scope; `unit` is one indent step.
///
/// # Errors
/// Propagates codegen / lowering failures (`CsGenError`).
pub(crate) fn emit_service(
    out: &mut String,
    iface: &InterfaceDef,
    base: &str,
    unit: &str,
) -> Result<(), CsGenError> {
    let lowered = lower_rpc_annotations(&iface.annotations);
    let svc = lower_service(iface, &lowered).map_err(|e| CsGenError::Internal(e.to_string()))?;
    let class = escape_identifier(&svc.name)?;

    emit_sync_interface(out, &svc.name, &class, &svc.methods, base, unit)?;
    emit_async_interface(out, &class, &svc.methods, base, unit)?;
    emit_handler_interface(out, &class, &svc.methods, base, unit)?;
    emit_requester(out, &class, &svc.methods, base, unit)?;
    emit_replier(out, &class, &svc.methods, base, unit)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Interfaces
// ---------------------------------------------------------------------------

fn emit_sync_interface(
    out: &mut String,
    svc_name: &str,
    class: &str,
    methods: &[MethodDef],
    base: &str,
    unit: &str,
) -> Result<(), CsGenError> {
    writeln!(
        out,
        "{base}/// <summary>Synchronous service interface for {class}.</summary>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{base}[Zerodds.Rpc.Service(\"{svc_name}\")]").map_err(fmt_err)?;
    writeln!(out, "{base}public interface {class}").map_err(fmt_err)?;
    writeln!(out, "{base}{{").map_err(fmt_err)?;
    for m in methods {
        let name = pascal_method(&m.name)?;
        if m.oneway {
            writeln!(out, "{base}{unit}[Zerodds.Rpc.Oneway]").map_err(fmt_err)?;
        }
        writeln!(
            out,
            "{base}{unit}{} {name}({});",
            sync_return_type(m)?,
            render_params(m)?
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{base}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_async_interface(
    out: &mut String,
    class: &str,
    methods: &[MethodDef],
    base: &str,
    unit: &str,
) -> Result<(), CsGenError> {
    writeln!(
        out,
        "{base}/// <summary>Asynchronous service interface for {class}.</summary>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{base}public interface {class}Async").map_err(fmt_err)?;
    writeln!(out, "{base}{{").map_err(fmt_err)?;
    for m in methods {
        let name = pascal_method(&m.name)?;
        writeln!(
            out,
            "{base}{unit}{} {name}Async({});",
            async_return_type(m)?,
            render_params(m)?
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{base}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_handler_interface(
    out: &mut String,
    class: &str,
    methods: &[MethodDef],
    base: &str,
    unit: &str,
) -> Result<(), CsGenError> {
    writeln!(
        out,
        "{base}/// <summary>Server-side handler interface for {class}; the service \
         implementor fills this in.</summary>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{base}public interface {class}Service").map_err(fmt_err)?;
    writeln!(out, "{base}{{").map_err(fmt_err)?;
    for m in methods {
        let name = pascal_method(&m.name)?;
        writeln!(
            out,
            "{base}{unit}{} {name}({});",
            sync_return_type(m)?,
            render_params(m)?
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{base}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Requester (client proxy)
// ---------------------------------------------------------------------------

fn emit_requester(
    out: &mut String,
    class: &str,
    methods: &[MethodDef],
    base: &str,
    unit: &str,
) -> Result<(), CsGenError> {
    let i1 = format!("{base}{unit}");
    let i2 = format!("{base}{unit}{unit}");
    let i3 = format!("{base}{unit}{unit}{unit}");
    writeln!(
        out,
        "{base}/// <summary>Client-side proxy for {class}: implements {class} and {class}Async.</summary>"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{base}public sealed class {class}Requester : {class}, {class}Async"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{base}{{").map_err(fmt_err)?;
    writeln!(
        out,
        "{i1}private readonly Zerodds.Rpc.IRequester requester;"
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(
        out,
        "{i1}public {class}Requester(Zerodds.Rpc.IRequester requester)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{i1}{{").map_err(fmt_err)?;
    writeln!(out, "{i2}this.requester = requester;").map_err(fmt_err)?;
    writeln!(out, "{i1}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    for (idx, m) in methods.iter().enumerate() {
        let method_id = idx + 1;
        emit_requester_sync(out, m, &i1, &i2)?;
        emit_requester_async(out, m, method_id, &i1, &i2, &i3)?;
    }

    writeln!(out, "{base}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

fn emit_requester_sync(
    out: &mut String,
    m: &MethodDef,
    i1: &str,
    i2: &str,
) -> Result<(), CsGenError> {
    let name = pascal_method(&m.name)?;
    let ret = sync_return_type(m)?;
    let call_args = render_call_args(m)?;
    writeln!(out, "{i1}public {ret} {name}({})", render_params(m)?).map_err(fmt_err)?;
    writeln!(out, "{i1}{{").map_err(fmt_err)?;
    if m.oneway {
        writeln!(out, "{i2}{name}Async({call_args});").map_err(fmt_err)?;
    } else if m.return_type.is_none() {
        writeln!(
            out,
            "{i2}try {{ {name}Async({call_args}).GetAwaiter().GetResult(); }} \
             catch (System.AggregateException __e) {{ throw new Zerodds.Rpc.RemoteException(__e.InnerException ?? __e); }}"
        )
        .map_err(fmt_err)?;
    } else {
        writeln!(
            out,
            "{i2}try {{ return {name}Async({call_args}).GetAwaiter().GetResult(); }} \
             catch (System.AggregateException __e) {{ throw new Zerodds.Rpc.RemoteException(__e.InnerException ?? __e); }}"
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{i1}}}").map_err(fmt_err)?;
    Ok(())
}

fn emit_requester_async(
    out: &mut String,
    m: &MethodDef,
    method_id: usize,
    i1: &str,
    i2: &str,
    i3: &str,
) -> Result<(), CsGenError> {
    let name = pascal_method(&m.name)?;
    let ret = async_return_type(m)?;
    let req = request_tuple(m)?;
    writeln!(out, "{i1}public {ret} {name}Async({})", render_params(m)?).map_err(fmt_err)?;
    writeln!(out, "{i1}{{").map_err(fmt_err)?;

    if m.oneway {
        writeln!(out, "{i2}requester.SendOneway({method_id}, {req});").map_err(fmt_err)?;
        writeln!(out, "{i2}return System.Threading.Tasks.Task.CompletedTask;").map_err(fmt_err)?;
        writeln!(out, "{i1}}}").map_err(fmt_err)?;
        return Ok(());
    }

    let has_writeback = m.params.iter().any(|p| p.direction != ParamDirection::In);
    writeln!(
        out,
        "{i2}return requester.SendRequest({method_id}, {req}).ContinueWith(__t =>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{i2}{{").map_err(fmt_err)?;
    if m.return_type.is_none() && !has_writeback {
        // void, no holders: nothing to unpack; touch Result to surface faults.
        writeln!(out, "{i3}_ = __t.Result;").map_err(fmt_err)?;
    } else {
        writeln!(out, "{i3}object[] __out = (object[]) __t.Result;").map_err(fmt_err)?;
        let mut k = 1usize;
        for p in &m.params {
            if p.direction == ParamDirection::In {
                continue;
            }
            let pname = escape_identifier(&p.name)?;
            let ty = typespec_to_cs(&p.type_ref)?;
            writeln!(out, "{i3}{pname}.Value = ({ty}) __out[{k}];").map_err(fmt_err)?;
            k += 1;
        }
        if let Some(ts) = &m.return_type {
            let ty = typespec_to_cs(ts)?;
            writeln!(out, "{i3}return ({ty}) __out[0];").map_err(fmt_err)?;
        }
    }
    writeln!(out, "{i2}}});").map_err(fmt_err)?;
    writeln!(out, "{i1}}}").map_err(fmt_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Replier (server dispatch)
// ---------------------------------------------------------------------------

fn emit_replier(
    out: &mut String,
    class: &str,
    methods: &[MethodDef],
    base: &str,
    unit: &str,
) -> Result<(), CsGenError> {
    let i1 = format!("{base}{unit}");
    let i2 = format!("{base}{unit}{unit}");
    let i3 = format!("{base}{unit}{unit}{unit}");
    let i4 = format!("{base}{unit}{unit}{unit}{unit}");
    writeln!(
        out,
        "{base}/// <summary>Server-side dispatcher for {class}: routes requests to a {class}Service.</summary>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{base}public sealed class {class}Replier").map_err(fmt_err)?;
    writeln!(out, "{base}{{").map_err(fmt_err)?;
    writeln!(out, "{i1}private readonly Zerodds.Rpc.IReplier replier;").map_err(fmt_err)?;
    writeln!(out, "{i1}private readonly {class}Service handler;").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(
        out,
        "{i1}public {class}Replier(Zerodds.Rpc.IReplier replier, {class}Service handler)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{i1}{{").map_err(fmt_err)?;
    writeln!(out, "{i2}this.replier = replier;").map_err(fmt_err)?;
    writeln!(out, "{i2}this.handler = handler;").map_err(fmt_err)?;
    writeln!(out, "{i1}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    writeln!(
        out,
        "{i1}/// <summary>Dispatches an incoming request by method id.</summary>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{i1}public object Dispatch(int methodId, object args)").map_err(fmt_err)?;
    writeln!(out, "{i1}{{").map_err(fmt_err)?;
    writeln!(out, "{i2}switch (methodId)").map_err(fmt_err)?;
    writeln!(out, "{i2}{{").map_err(fmt_err)?;
    for (idx, m) in methods.iter().enumerate() {
        let case_id = idx + 1;
        let mname = pascal_method(&m.name)?;
        writeln!(out, "{i3}case {case_id}:").map_err(fmt_err)?;
        writeln!(out, "{i3}{{").map_err(fmt_err)?;
        writeln!(out, "{i4}object[] __a = (object[]) args;").map_err(fmt_err)?;
        let mut req_idx = 0usize;
        let mut call_args: Vec<String> = Vec::new();
        for p in &m.params {
            let pname = escape_identifier(&p.name)?;
            let ty = typespec_to_cs(&p.type_ref)?;
            match p.direction {
                ParamDirection::In => {
                    call_args.push(format!("({ty}) __a[{req_idx}]"));
                    req_idx += 1;
                }
                ParamDirection::InOut => {
                    writeln!(
                        out,
                        "{i4}var {pname} = new Zerodds.Rpc.Holder<{ty}>(({ty}) __a[{req_idx}]);"
                    )
                    .map_err(fmt_err)?;
                    call_args.push(pname);
                    req_idx += 1;
                }
                ParamDirection::Out => {
                    writeln!(out, "{i4}var {pname} = new Zerodds.Rpc.Holder<{ty}>();")
                        .map_err(fmt_err)?;
                    call_args.push(pname);
                }
            }
        }
        let call = format!("handler.{mname}({})", call_args.join(", "));
        if m.return_type.is_some() {
            let ret_ty = sync_return_type(m)?;
            writeln!(out, "{i4}{ret_ty} __ret = {call};").map_err(fmt_err)?;
        } else {
            writeln!(out, "{i4}{call};").map_err(fmt_err)?;
        }
        let mut reply_parts: Vec<String> = Vec::new();
        reply_parts.push(if m.return_type.is_some() {
            "__ret".to_string()
        } else {
            "null".to_string()
        });
        for p in &m.params {
            if p.direction != ParamDirection::In {
                reply_parts.push(format!("{}.Value", escape_identifier(&p.name)?));
            }
        }
        writeln!(
            out,
            "{i4}return new object?[] {{ {} }};",
            reply_parts.join(", ")
        )
        .map_err(fmt_err)?;
        writeln!(out, "{i3}}}").map_err(fmt_err)?;
    }
    writeln!(out, "{i3}default:").map_err(fmt_err)?;
    writeln!(
        out,
        "{i4}throw new Zerodds.Rpc.RemoteException(\"unknown method id: \" + methodId, \
         Zerodds.Rpc.RemoteExceptionCode.UnknownOperation);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{i2}}}").map_err(fmt_err)?;
    writeln!(out, "{i1}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Pushes a dispatched reply back through the runtime replier — this also
    // keeps the `replier` field live (mirrors the Java PSM `sendReply`).
    writeln!(
        out,
        "{i1}/// <summary>Sends a dispatched reply tuple back to the client.</summary>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{i1}public void SendReply(object reply)").map_err(fmt_err)?;
    writeln!(out, "{i1}{{").map_err(fmt_err)?;
    writeln!(out, "{i2}replier.SendReply(reply);").map_err(fmt_err)?;
    writeln!(out, "{i1}}}").map_err(fmt_err)?;
    writeln!(out, "{base}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Type / parameter helpers
// ---------------------------------------------------------------------------

/// Sync return type: `void` for oneway/void, otherwise the C# type.
fn sync_return_type(m: &MethodDef) -> Result<String, CsGenError> {
    if m.oneway {
        return Ok("void".to_string());
    }
    match &m.return_type {
        None => Ok("void".to_string()),
        Some(ts) => typespec_to_cs(ts),
    }
}

/// Async return type: `Task` for oneway/void, otherwise `Task<T>`.
fn async_return_type(m: &MethodDef) -> Result<String, CsGenError> {
    match (m.oneway, &m.return_type) {
        (false, Some(ts)) => Ok(format!(
            "System.Threading.Tasks.Task<{}>",
            typespec_to_cs(ts)?
        )),
        _ => Ok("System.Threading.Tasks.Task".to_string()),
    }
}

/// Method parameter list. `out`/`inout` render as `Holder<T>` (holder pattern).
fn render_params(m: &MethodDef) -> Result<String, CsGenError> {
    let mut parts: Vec<String> = Vec::new();
    for p in &m.params {
        parts.push(render_param(p)?);
    }
    Ok(parts.join(", "))
}

fn render_param(p: &ParamDef) -> Result<String, CsGenError> {
    let name = escape_identifier(&p.name)?;
    let ty = typespec_to_cs(&p.type_ref)?;
    let rendered = match p.direction {
        ParamDirection::In => ty,
        ParamDirection::Out | ParamDirection::InOut => format!("Zerodds.Rpc.Holder<{ty}>"),
    };
    Ok(format!("{rendered} {name}"))
}

/// Call argument list (all params by name, in declaration order).
fn render_call_args(m: &MethodDef) -> Result<String, CsGenError> {
    let mut parts: Vec<String> = Vec::new();
    for p in &m.params {
        parts.push(escape_identifier(&p.name)?);
    }
    Ok(parts.join(", "))
}

/// `new object[] { … }` request tuple: IN by value, INOUT by `.Value`, OUT omitted.
fn request_tuple(m: &MethodDef) -> Result<String, CsGenError> {
    let mut parts: Vec<String> = Vec::new();
    for p in &m.params {
        let pname = escape_identifier(&p.name)?;
        match p.direction {
            ParamDirection::In => parts.push(pname),
            ParamDirection::InOut => parts.push(format!("{pname}.Value")),
            ParamDirection::Out => {}
        }
    }
    Ok(format!("new object[] {{ {} }}", parts.join(", ")))
}

/// PascalCase method name (C# convention), keyword-escaped.
fn pascal_method(name: &str) -> Result<String, CsGenError> {
    let mut chars = name.chars();
    let pascal = match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    };
    escape_identifier(&pascal)
}

// Keep the `ScopedName` / `TypeSpec` imports live for future resolved-type work
// without a dead-import warning (mirrors the Java PSM's helper marker).
#[allow(dead_code)]
fn _keep_imports(_s: &ScopedName, _t: &TypeSpec) {
    let _ = scoped_to_cs;
}
