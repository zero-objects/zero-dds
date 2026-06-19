// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL service → Java RPC codegen (DDS-RPC 1.0 §7.11.2 — Java PSM).
//!
//! This module is the bridge between the typed RPC data model
//! from `zerodds-rpc` (`ServiceDef`/`MethodDef`/`ParamDef`) and the
//! Java source codegen. Per service we emit four classes:
//!
//! * `<Service>.java`         — synchronous interface with all methods
//!                              (`throws RemoteException` + user exceptions).
//! * `<Service>Async.java`    — asynchronous interface with
//!                              `CompletableFuture<TOut>` returns
//!                              (spec §7.11.2.2.4 maps `Future<T>` to
//!                              `java.util.concurrent.Future<T>`; we
//!                              use `CompletableFuture` as a
//!                              full implementation).
//! * `<Service>Requester.java` — client side: wraps a
//!                              `org.zerodds.rpc.Requester<TIn,TOut>`
//!                              and implements `<Service>` +
//!                              `<Service>Async`.
//! * `<Service>Replier.java`   — server side: wraps a
//!                              `org.zerodds.rpc.Replier<TIn,TOut>` and
//!                              dispatches incoming requests to a
//!                              `<Service>Service` handler.
//! * `<Service>Service.java`   — server-side handler interface: the
//!                              service implementor implements this
//!                              interface.
//!
//! # Out parameters
//! Java has no `out`/`inout` concept. We map `out`/`inout` via the
//! **holder pattern** (spec §7.11.2.3 / IDL-to-Java 1.3 §1.5):
//!
//! ```java
//! public final class IntHolder {
//!     public int value;
//!     public IntHolder() {}
//!     public IntHolder(int v) { this.value = v; }
//! }
//! ```
//!
//! Holders are **not** emitted per service — we reference the
//! generic holders from `org.zerodds.rpc.Holder<T>` (see
//! `runtime/rpc/Holder.java`). Rationale: less boilerplate than
//! per-type holders, fully compatible with the spec pattern (the
//! caller writes to `holder.value` before the call, the reply decoder
//! overwrites it on return). The alternative `Object[]` wrapper would be
//! type-unsafe and is therefore ruled out.
//!
//! # Exception mapping (spec §7.11.2.1)
//! * IDL `exception E { ... }` → we delegate to the existing
//!   `emit_exception_file` path (`E extends RuntimeException`); the RPC
//!   codegen only adds the `throws E1, E2` clause on the method.
//! * `org.zerodds.rpc.RemoteException` is a RuntimeException
//!   subclass that every RPC method may throw implicitly — we therefore
//!   do **not** place it explicitly in the `throws` lists, because
//!   RuntimeException is not checked in Java.
//!
//! # Marshalling convention
//! The requester/replier marshal a type-erased tuple
//! through the runtime `Requester<Object,Object>` / `Replier<Object,Object>`:
//! the request payload is an `Object[]` of the IN + INOUT values (declaration
//! order); the reply payload is an `Object[] { returnValue-or-null, INOUT+OUT
//! values… }`. The requester writes the INOUT/OUT holders back from the reply
//! and casts the return value; the replier decodes the request tuple, builds
//! holders, calls the handler, and packs the reply. The generated code is
//! verified to compile against the real runtime by
//! `tests/compile_check.rs` (real `javac`, no stubs).
//!
//! # What this stage does NOT do
//! * No `mvn` packaging — callers drop the `runtime/` sources (or a jar)
//!   alongside the generated code (see `runtime/README.md`).
//! * No reflection TypeRep (a stretch in idl4-java §8).

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;

use zerodds_idl::ast::{IntegerType, PrimitiveType, ScopedName, TypeSpec};
use zerodds_rpc::service_mapping::{MethodDef, ParamDef, ParamDirection, ServiceDef};

use crate::JavaGenOptions;
use crate::emitter::{JavaFile, fmt_err, indent_unit, wrap_compilation_unit_default};
use crate::error::JavaGenError;
use crate::keywords::sanitize_identifier;
use crate::type_map::{
    floating_to_java, floating_to_java_boxed, integer_to_java, integer_to_java_boxed,
    primitive_to_java, primitive_to_java_boxed,
};

// ---------------------------------------------------------------------------
// Public API — see the crate doc for the call schema
// ---------------------------------------------------------------------------

/// Emits the synchronous service interface (`<Service>.java`).
///
/// # Errors
/// `JavaGenError::InvalidName` for a non-sanitizable service name.
pub fn emit_service_interface(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&svc.name)?;
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(body, "/** Synchronous service interface for {class}. */").map_err(fmt_err)?;
    writeln!(body, "@org.zerodds.rpc.Service(\"{}\")", svc.name).map_err(fmt_err)?;
    writeln!(body, "public interface {class} {{").map_err(fmt_err)?;
    for m in &svc.methods {
        emit_sync_method_signature(&mut body, m, &ind)?;
    }
    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Emits the asynchronous service interface (`<Service>Async.java`).
///
/// # Errors
/// Like [`emit_service_interface`].
pub fn emit_service_interface_async(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let svc_class = sanitize_identifier(&svc.name)?;
    let class = format!("{svc_class}Async");
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(
        body,
        "/** Asynchronous service interface for {svc_class}. */"
    )
    .map_err(fmt_err)?;
    writeln!(body, "public interface {class} {{").map_err(fmt_err)?;
    for m in &svc.methods {
        emit_async_method_signature(&mut body, m, &ind)?;
    }
    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Emits the client-side requester class (`<Service>Requester.java`).
///
/// # Errors
/// Like [`emit_service_interface`].
pub fn emit_requester_class(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let svc_class = sanitize_identifier(&svc.name)?;
    let class = format!("{svc_class}Requester");
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(
        body,
        "/** Client-side proxy for {svc_class}. Implements both the \
         synchronous {svc_class} interface and the {svc_class}Async \
         interface. */",
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "public final class {class} implements {svc_class}, {svc_class}Async {{",
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}private final org.zerodds.rpc.Requester<Object, Object> requester;",
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class}(org.zerodds.rpc.Requester<Object, Object> requester) {{",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}this.requester = requester;").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // Sync methods: call the async variant and block.
    for m in &svc.methods {
        emit_requester_sync_impl(&mut body, m, &ind)?;
    }
    writeln!(body).map_err(fmt_err)?;
    // Async methods: send the request, return a CompletableFuture.
    for m in &svc.methods {
        emit_requester_async_impl(&mut body, m, &ind)?;
    }

    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Emits the server-side replier class (`<Service>Replier.java`).
///
/// # Errors
/// Like [`emit_service_interface`].
pub fn emit_replier_class(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let svc_class = sanitize_identifier(&svc.name)?;
    let class = format!("{svc_class}Replier");
    let handler_iface = format!("{svc_class}Service");
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(
        body,
        "/** Server-side replier for {svc_class}. Wires a {handler_iface} \
         implementation to the underlying RPC runtime. */",
    )
    .map_err(fmt_err)?;
    writeln!(body, "public final class {class} {{").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}private final org.zerodds.rpc.Replier<Object, Object> replier;",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}private final {handler_iface} handler;").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class}(org.zerodds.rpc.Replier<Object, Object> replier, {handler_iface} handler) {{",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}this.replier = replier;").map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}this.handler = handler;").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // Dispatch stub: takes a request and calls the right
    // the handler method by method ID.
    writeln!(
        body,
        "{ind}/** Dispatches an incoming request by method id. */",
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public Object dispatch(int methodId, Object args) {{",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}switch (methodId) {{").map_err(fmt_err)?;
    for (idx, m) in svc.methods.iter().enumerate() {
        let mname = sanitize_identifier(&m.name)?;
        let case_id = idx + 1;
        // Decode the request tuple, call the handler, pack the reply tuple —
        // mirroring the requester marshalling convention:
        //   args  = Object[] of IN + INOUT values (declaration order)
        //   reply = Object[] { returnValue-or-null, INOUT+OUT values… }
        writeln!(body, "{ind}{ind}{ind}case {case_id}: {{").map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}{ind}{ind}Object[] __a = (Object[]) args;").map_err(fmt_err)?;
        let mut req_idx = 0usize;
        let mut call_args: Vec<String> = Vec::new();
        for p in &m.params {
            let pname = sanitize_identifier(&p.name)?;
            let boxed = typespec_to_java_boxed(&p.type_ref)?;
            match p.direction {
                ParamDirection::In => {
                    call_args.push(format!("({boxed}) __a[{req_idx}]"));
                    req_idx += 1;
                }
                ParamDirection::InOut => {
                    writeln!(
                        body,
                        "{ind}{ind}{ind}{ind}org.zerodds.rpc.Holder<{boxed}> {pname} = \
                         new org.zerodds.rpc.Holder<>(({boxed}) __a[{req_idx}]);",
                    )
                    .map_err(fmt_err)?;
                    call_args.push(pname);
                    req_idx += 1;
                }
                ParamDirection::Out => {
                    writeln!(
                        body,
                        "{ind}{ind}{ind}{ind}org.zerodds.rpc.Holder<{boxed}> {pname} = \
                         new org.zerodds.rpc.Holder<>();",
                    )
                    .map_err(fmt_err)?;
                    call_args.push(pname);
                }
            }
        }
        let call = format!("handler.{mname}({})", call_args.join(", "));
        if m.return_type.is_some() {
            let ret_unboxed = sync_return_type(m)?;
            writeln!(body, "{ind}{ind}{ind}{ind}{ret_unboxed} __ret = {call};").map_err(fmt_err)?;
        } else {
            writeln!(body, "{ind}{ind}{ind}{ind}{call};").map_err(fmt_err)?;
        }
        // Reply tuple: [0] = return value (or null), then inout/out values.
        let mut reply_parts: Vec<String> = Vec::new();
        reply_parts.push(if m.return_type.is_some() {
            "__ret".to_string()
        } else {
            "null".to_string()
        });
        for p in &m.params {
            if p.direction != ParamDirection::In {
                reply_parts.push(format!("{}.value", sanitize_identifier(&p.name)?));
            }
        }
        writeln!(
            body,
            "{ind}{ind}{ind}{ind}return new Object[] {{ {} }};",
            reply_parts.join(", ")
        )
        .map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}{ind}}}").map_err(fmt_err)?;
    }
    writeln!(
        body,
        "{ind}{ind}{ind}default: throw new org.zerodds.rpc.RemoteException(\
         \"unknown method id: \" + methodId, \
         org.zerodds.rpc.RemoteExceptionCode.UNKNOWN_OPERATION);",
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}}}").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;

    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Emits the server-side handler interface (`<Service>Service.java`).
///
/// Users implement this interface and pass their
/// implementation to the [`emit_replier_class`] output.
///
/// # Errors
/// Like [`emit_service_interface`].
pub fn emit_service_handler_interface(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let svc_class = sanitize_identifier(&svc.name)?;
    let class = format!("{svc_class}Service");
    let ind = indent_unit(opts);
    let mut body = String::new();
    writeln!(
        body,
        "/** Server-side handler interface for {svc_class}. Implementors \
         provide the actual business logic; a {svc_class}Replier wires \
         them to the RPC runtime. */",
    )
    .map_err(fmt_err)?;
    writeln!(body, "public interface {class} {{").map_err(fmt_err)?;
    for m in &svc.methods {
        emit_handler_method_signature(&mut body, m, &ind)?;
    }
    writeln!(body, "}}").map_err(fmt_err)?;
    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: class,
        source,
    })
}

/// Convenience wrapper: emits all 5 Java files for one service.
///
/// Order: interface, async interface, handler interface, requester,
/// replier.
///
/// # Errors
/// Like [`emit_service_interface`].
pub fn emit_service_files(
    svc: &ServiceDef,
    pkg: &str,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    Ok(alloc::vec![
        emit_service_interface(svc, pkg, opts)?,
        emit_service_interface_async(svc, pkg, opts)?,
        emit_service_handler_interface(svc, pkg, opts)?,
        emit_requester_class(svc, pkg, opts)?,
        emit_replier_class(svc, pkg, opts)?,
    ])
}

// ---------------------------------------------------------------------------
// Method-Signature-Emitter
// ---------------------------------------------------------------------------

fn emit_sync_method_signature(
    out: &mut String,
    m: &MethodDef,
    ind: &str,
) -> Result<(), JavaGenError> {
    let name = sanitize_identifier(&m.name)?;
    let ret = sync_return_type(m)?;
    let params = render_method_params(m)?;
    let throws = String::new();
    if m.oneway {
        writeln!(out, "{ind}@org.zerodds.rpc.Oneway").map_err(fmt_err)?;
    }
    writeln!(out, "{ind}{ret} {name}({params}){throws};").map_err(fmt_err)?;
    Ok(())
}

fn emit_async_method_signature(
    out: &mut String,
    m: &MethodDef,
    ind: &str,
) -> Result<(), JavaGenError> {
    let name = sanitize_identifier(&m.name)?;
    let async_name = format!("{name}Async");
    let ret = async_return_type(m)?;
    let params = render_method_params_async(m)?;
    if m.oneway {
        writeln!(out, "{ind}@org.zerodds.rpc.Oneway").map_err(fmt_err)?;
    }
    writeln!(out, "{ind}{ret} {async_name}({params});").map_err(fmt_err)?;
    Ok(())
}

fn emit_handler_method_signature(
    out: &mut String,
    m: &MethodDef,
    ind: &str,
) -> Result<(), JavaGenError> {
    // The handler signature is identical to the sync signature — the
    // Replier-Dispatch ruft sie direkt auf.
    emit_sync_method_signature(out, m, ind)
}

fn emit_requester_sync_impl(
    out: &mut String,
    m: &MethodDef,
    ind: &str,
) -> Result<(), JavaGenError> {
    let name = sanitize_identifier(&m.name)?;
    let async_name = format!("{name}Async");
    let ret_ty = sync_return_type(m)?;
    let params = render_method_params(m)?;
    let arg_list = render_call_arglist(m)?;
    writeln!(out, "{ind}@Override").map_err(fmt_err)?;
    writeln!(out, "{ind}public {ret_ty} {name}({params}) {{").map_err(fmt_err)?;
    if m.oneway {
        writeln!(out, "{ind}{ind}{async_name}({arg_list});").map_err(fmt_err)?;
        writeln!(out, "{ind}}}").map_err(fmt_err)?;
        return Ok(());
    }
    if m.return_type.is_none() {
        // void return (with or without out/inout holders): block on the
        // future but do not `return` its (Void) value.
        writeln!(
            out,
            "{ind}{ind}try {{ {async_name}({arg_list}).get(); }} catch (Exception e) {{ \
             throw new org.zerodds.rpc.RemoteException(e); }}",
        )
        .map_err(fmt_err)?;
    } else {
        writeln!(
            out,
            "{ind}{ind}try {{ return {async_name}({arg_list}).get(); }} catch (Exception e) {{ \
             throw new org.zerodds.rpc.RemoteException(e); }}",
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    Ok(())
}

fn emit_requester_async_impl(
    out: &mut String,
    m: &MethodDef,
    ind: &str,
) -> Result<(), JavaGenError> {
    let name = sanitize_identifier(&m.name)?;
    let async_name = format!("{name}Async");
    let ret_ty = async_return_type(m)?;
    let params = render_method_params_async(m)?;
    writeln!(out, "{ind}@Override").map_err(fmt_err)?;
    writeln!(out, "{ind}public {ret_ty} {async_name}({params}) {{").map_err(fmt_err)?;

    // Type-erased marshalling convention (matches the replier dispatch):
    //   request payload  = Object[] of IN + INOUT values (declaration order)
    //   reply payload    = Object[] { returnValue-or-null, INOUT+OUT values… }
    let req_args = request_args_array(m)?;
    if m.oneway {
        writeln!(out, "{ind}{ind}requester.sendOneway({req_args});").map_err(fmt_err)?;
        writeln!(
            out,
            "{ind}{ind}return java.util.concurrent.CompletableFuture.completedFuture(null);",
        )
        .map_err(fmt_err)?;
    } else {
        let has_writeback = m.params.iter().any(|p| p.direction != ParamDirection::In);
        if m.return_type.is_none() && !has_writeback {
            // void, no holders: complete with null once the reply arrives.
            writeln!(
                out,
                "{ind}{ind}return requester.sendRequest({req_args}).thenApply(__reply -> null);",
            )
            .map_err(fmt_err)?;
        } else {
            writeln!(
                out,
                "{ind}{ind}return requester.sendRequest({req_args}).thenApply(__reply -> {{",
            )
            .map_err(fmt_err)?;
            writeln!(out, "{ind}{ind}{ind}Object[] __out = (Object[]) __reply;",)
                .map_err(fmt_err)?;
            // Write back inout/out holders from reply[1..] in declaration order.
            let mut k = 1usize;
            for p in &m.params {
                if p.direction == ParamDirection::In {
                    continue;
                }
                let pname = sanitize_identifier(&p.name)?;
                let boxed = typespec_to_java_boxed(&p.type_ref)?;
                writeln!(out, "{ind}{ind}{ind}{pname}.value = ({boxed}) __out[{k}];",)
                    .map_err(fmt_err)?;
                k += 1;
            }
            match &m.return_type {
                None => {
                    writeln!(out, "{ind}{ind}{ind}return null;").map_err(fmt_err)?;
                }
                Some(ts) => {
                    let boxed = typespec_to_java_boxed(ts)?;
                    writeln!(out, "{ind}{ind}{ind}return ({boxed}) __out[0];").map_err(fmt_err)?;
                }
            }
            writeln!(out, "{ind}{ind}}});").map_err(fmt_err)?;
        }
    }
    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Type-Mapping-Helpers
// ---------------------------------------------------------------------------

/// Returns the Java return type for the synchronous method signature.
/// `oneway` and `void` → `void`. `out` parameters are mapped in the holder
/// pattern — they remain part of the parameter list.
fn sync_return_type(m: &MethodDef) -> Result<String, JavaGenError> {
    if m.oneway {
        return Ok("void".to_string());
    }
    match &m.return_type {
        None => Ok("void".to_string()),
        Some(ts) => typespec_to_java_unboxed(ts),
    }
}

/// Returns the Java async return type. `oneway` yields
/// `CompletableFuture<Void>` (the caller may still wait on "complete"
/// even if the reply will be skipped), `void` → `Void`,
/// otherwise `CompletableFuture<TBoxed>`.
fn async_return_type(m: &MethodDef) -> Result<String, JavaGenError> {
    if m.oneway {
        return Ok("java.util.concurrent.CompletableFuture<Void>".to_string());
    }
    match &m.return_type {
        None => Ok("java.util.concurrent.CompletableFuture<Void>".to_string()),
        Some(ts) => Ok(format!(
            "java.util.concurrent.CompletableFuture<{}>",
            typespec_to_java_boxed(ts)?
        )),
    }
}

/// Parameter list for the sync variant. `out`/`inout` are
/// Holder gerendert.
fn render_method_params(m: &MethodDef) -> Result<String, JavaGenError> {
    let mut parts: Vec<String> = Vec::new();
    for p in &m.params {
        parts.push(render_param(p)?);
    }
    Ok(parts.join(", "))
}

/// Parameter list for the async variant; the holder pattern is preserved.
fn render_method_params_async(m: &MethodDef) -> Result<String, JavaGenError> {
    render_method_params(m)
}

fn render_param(p: &ParamDef) -> Result<String, JavaGenError> {
    let name = sanitize_identifier(&p.name)?;
    let ty = match p.direction {
        ParamDirection::In => typespec_to_java_unboxed(&p.type_ref)?,
        ParamDirection::Out | ParamDirection::InOut => {
            // Holder-Pattern (Spec §7.11.2 / IDL2Java 1.3 §1.5).
            format!(
                "org.zerodds.rpc.Holder<{}>",
                typespec_to_java_boxed(&p.type_ref)?
            )
        }
    };
    Ok(format!("{ty} {name}"))
}

fn render_call_arglist(m: &MethodDef) -> Result<String, JavaGenError> {
    let mut parts: Vec<String> = Vec::new();
    for p in &m.params {
        parts.push(sanitize_identifier(&p.name)?);
    }
    Ok(parts.join(", "))
}

/// `new Object[] { … }` request payload sent to the runtime `Requester`:
/// IN params contribute their value, INOUT their `.value`, OUT nothing (the
/// server allocates an empty holder). Order = declaration order.
fn request_args_array(m: &MethodDef) -> Result<String, JavaGenError> {
    let mut parts: Vec<String> = Vec::new();
    for p in &m.params {
        let pname = sanitize_identifier(&p.name)?;
        match p.direction {
            ParamDirection::In => parts.push(pname),
            ParamDirection::InOut => parts.push(format!("{pname}.value")),
            ParamDirection::Out => {}
        }
    }
    Ok(format!("new Object[] {{ {} }}", parts.join(", ")))
}

// ---------------------------------------------------------------------------
// TypeSpec → Java
// ---------------------------------------------------------------------------

fn typespec_to_java_unboxed(ts: &TypeSpec) -> Result<String, JavaGenError> {
    match ts {
        TypeSpec::Primitive(p) => Ok(primitive_to_java(*p).to_string()),
        TypeSpec::Scoped(s) => Ok(scoped_to_java(s)),
        TypeSpec::String(_) => Ok("String".to_string()),
        TypeSpec::Sequence(s) => Ok(format!(
            "java.util.List<{}>",
            typespec_to_java_boxed(&s.elem)?
        )),
        TypeSpec::Map(mm) => Ok(format!(
            "java.util.Map<{}, {}>",
            typespec_to_java_boxed(&mm.key)?,
            typespec_to_java_boxed(&mm.value)?,
        )),
        TypeSpec::Fixed(_) => Err(JavaGenError::UnsupportedConstruct {
            construct: "fixed".into(),
            context: None,
        }),
        TypeSpec::Any => Err(JavaGenError::UnsupportedConstruct {
            construct: "any".into(),
            context: None,
        }),
    }
}

fn typespec_to_java_boxed(ts: &TypeSpec) -> Result<String, JavaGenError> {
    match ts {
        TypeSpec::Primitive(p) => Ok(primitive_to_java_boxed(*p).to_string()),
        TypeSpec::Scoped(s) => Ok(scoped_to_java(s)),
        TypeSpec::String(_) => Ok("String".to_string()),
        TypeSpec::Sequence(s) => Ok(format!(
            "java.util.List<{}>",
            typespec_to_java_boxed(&s.elem)?
        )),
        TypeSpec::Map(mm) => Ok(format!(
            "java.util.Map<{}, {}>",
            typespec_to_java_boxed(&mm.key)?,
            typespec_to_java_boxed(&mm.value)?,
        )),
        TypeSpec::Fixed(_) => Err(JavaGenError::UnsupportedConstruct {
            construct: "fixed".into(),
            context: None,
        }),
        TypeSpec::Any => Err(JavaGenError::UnsupportedConstruct {
            construct: "any".into(),
            context: None,
        }),
    }
}

fn scoped_to_java(s: &ScopedName) -> String {
    s.parts
        .iter()
        .map(|p| p.text.clone())
        .collect::<Vec<_>>()
        .join(".")
}

// Marker so the linter does not report the "maybe-later" helpers
// as unused.
#[allow(dead_code)]
fn _unused_marker(_i: IntegerType) {
    let _ = integer_to_java;
    let _ = floating_to_java;
    let _ = integer_to_java_boxed;
    let _ = floating_to_java_boxed;
    let _ = PrimitiveType::Boolean;
}
